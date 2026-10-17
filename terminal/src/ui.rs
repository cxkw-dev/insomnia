//! The TUI: terminal setup, the render loop, and the two layouts — the
//! combined frame (default: the session above one scrollable dashboard) and
//! separate boxes (a status card with cards beside or below it) — floating
//! over the sky rendered by [`crate::sky`].

use std::io;
use std::time::{Duration, Instant};

use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    crossterm::{
        cursor,
        event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Widget},
    Frame, Terminal,
};

use crate::art;
use crate::cards::{self, Feed, SideCard};
use crate::collect::{self, Container, Health, Update};
use crate::config::{FileConfig, Flags};
use crate::dashboard::{Dashboard, Navigation};
use crate::duration::clock;
use crate::power::{Kind, PowerStatus};
use crate::sky;
use crate::text::{pair, truncate, width};
use crate::theme::{
    scale, AMBER, BACKGROUND, BORDER, EMERALD, GOLD, GOLD_RGB, MUTED, RED, SLATE_400, SLATE_600,
    SLATE_700, TEXT, VIOLET_LIGHT,
};

/// Frame interval while the sky moves, and while it's still.
const FRAME: Duration = Duration::from_millis(120);
const STILL_FRAME: Duration = Duration::from_millis(250);
/// How long a card command may run before it's killed.
const CARD_TIMEOUT: Duration = Duration::from_secs(45);

// Combined layout.
const COMBINED_MIN: (u16, u16) = (32, 14);
const PANEL_MAX_W: u16 = 110;
const PANEL_TOP: u16 = 3;
const PANEL_PAD_X: u16 = 4;
const PANEL_PAD_Y: u16 = 2;

// Separate-box layout.
const SEPARATE_MIN: (u16, u16) = (MAIN_W + 2, 18);
const MAIN_W: u16 = 64;
const SIDE_W: u16 = 40;
/// Side cards grow past [`SIDE_W`] to fit their longest row, up to this.
const SIDE_MAX_W: u16 = 64;
const GAP: u16 = 2;
/// Rows a card spends on chrome: borders and the blank line under its title.
const CARD_CHROME: u16 = 4;
/// Rows the status card spends on chrome: borders and a bottom margin.
const STATUS_CHROME: u16 = 3;
/// Rows around the cards: the title and its gap above, the hint below, and
/// breathing room.
const SURROUND: u16 = art::TITLE_H + 1 + 2 + 2;

/// Everything the renderer needs for one frame.
pub struct ViewState<'a> {
    pub tick: u64,
    pub elapsed: Duration,
    pub total: Option<Duration>,
    pub remaining: Option<Duration>,
    pub kinds: &'a [Kind],
    pub power: &'a PowerStatus,
    /// Cards with data, in display order (custom cards, then docker).
    pub cards: &'a [SideCard],
    /// The combined layout's dashboard; `None` draws separate boxes.
    pub dashboard: Option<&'a Dashboard>,
    pub navigation: &'a Navigation,
    /// Per-flyer sky toggles, aligned with [`sky::FLYERS`].
    pub sky: &'a [bool],
    pub motion: bool,
}

impl ViewState<'_> {
    /// How far through a timed session we are, 0–1.
    fn progress(&self) -> Option<f32> {
        let (remaining, total) = (self.remaining?, self.total?);
        Some((1.0 - remaining.as_secs_f32() / total.as_secs_f32().max(1.0)).clamp(0.0, 1.0))
    }

    /// The tick for container badges: frozen while motion is off.
    fn badge_tick(&self) -> u64 {
        if self.motion {
            self.tick
        } else {
            0
        }
    }
}

/// Restores the terminal however the UI exits — including an early `?`.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let guard = Self;
        execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
        let orig_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            orig_hook(info);
        }));
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
}

pub fn run(flags: &Flags, fc: &FileConfig) -> io::Result<Duration> {
    let _guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let start = Instant::now();
    let deadline = flags.timeout.map(|t| start + t);
    let updates = collect::spawn(fc.docker.enabled, fc.cards.clone(), CARD_TIMEOUT);
    let sky_on = sky::resolve(&fc.sky.flyers);
    let mut motion = fc.sky.motion;
    let mut navigation = Navigation::default();
    let mut power = PowerStatus::default();
    let mut docker: Option<Vec<Container>> = None;
    let mut feeds = vec![Feed::default(); fc.cards.len()];
    let mut tick: u64 = 0;

    loop {
        let now = Instant::now();
        if deadline.is_some_and(|d| now >= d) {
            break;
        }
        while let Ok(update) = updates.try_recv() {
            match update {
                Update::Power(p) => power = p,
                Update::Docker(d) => docker = d,
                Update::Card(i, result) => feeds[i].update(result),
            }
        }
        let cards = cards::build_cards(&fc.cards, &docker, &feeds);
        let dashboard = fc.dashboard.combined.then(|| Dashboard::from_cards(&cards));
        if let Some(dashboard) = &dashboard {
            navigation.sync(dashboard);
        }
        let mut viewport = None;
        {
            let state = ViewState {
                tick,
                elapsed: start.elapsed(),
                total: flags.timeout,
                remaining: deadline.map(|d| d.saturating_duration_since(now)),
                kinds: &flags.kinds,
                power: &power,
                cards: &cards,
                dashboard: dashboard.as_ref(),
                navigation: &navigation,
                sky: &sky_on,
                motion,
            };
            terminal.draw(|f| viewport = draw(f, &state))?;
        }
        if event::poll(if motion { FRAME } else { STILL_FRAME })? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let quit = handle_key(
                        key,
                        &mut motion,
                        &mut navigation,
                        dashboard.as_ref(),
                        viewport.as_ref(),
                    );
                    if quit {
                        break;
                    }
                }
            }
        }
        tick += 1;
    }
    Ok(start.elapsed())
}

/// Apply one key press; true means release and quit.
fn handle_key(
    key: KeyEvent,
    motion: &mut bool,
    navigation: &mut Navigation,
    dashboard: Option<&Dashboard>,
    viewport: Option<&Viewport>,
) -> bool {
    match key.code {
        KeyCode::Char('q' | 'Q') | KeyCode::Esc => return true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return true,
        KeyCode::Char('m' | 'M') => *motion = !*motion,
        _ => {}
    }
    let Some(dashboard) = dashboard else {
        return false;
    };
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => navigation.move_selection(dashboard, 1),
        KeyCode::Up | KeyCode::Char('k') => navigation.move_selection(dashboard, -1),
        KeyCode::PageDown | KeyCode::PageUp => {
            if let Some(viewport) = viewport {
                let page = viewport.height.max(1) as isize;
                let step = if key.code == KeyCode::PageDown {
                    page
                } else {
                    -page
                };
                navigation.scroll = viewport.page(step);
            }
        }
        _ => {}
    }
    false
}

/// Draw one frame in whichever layout the state asks for. The combined
/// layout returns its scroll viewport so paging can be computed from it.
pub fn draw(f: &mut Frame, s: &ViewState) -> Option<Viewport> {
    let area = f.area();
    f.buffer_mut()
        .set_style(area, Style::default().bg(BACKGROUND).fg(TEXT));
    match s.dashboard {
        Some(dashboard) => draw_combined(f, s, dashboard),
        None => {
            draw_separate(f, s);
            None
        }
    }
}

fn too_small(area: Rect, (w, h): (u16, u16)) -> bool {
    area.width < w || area.height < h
}

// --- Combined layout ---------------------------------------------------------

/// The dashboard's scroll window: the session stays anchored above while
/// the rows beneath scroll.
pub struct Viewport {
    start: usize,
    base: usize,
    max: usize,
    height: usize,
}

impl Viewport {
    fn page(&self, step: isize) -> isize {
        self.start.saturating_add_signed(step).min(self.max) as isize - self.base as isize
    }
}

fn draw_combined(f: &mut Frame, s: &ViewState, dashboard: &Dashboard) -> Option<Viewport> {
    let area = f.area();
    if too_small(area, COMBINED_MIN) {
        draw_compact(f, s);
        return None;
    }
    let panel_w = area.width.saturating_sub(4).min(PANEL_MAX_W);
    let content_w = panel_w.saturating_sub(2 * PANEL_PAD_X) as usize;
    let rows = dashboard.render(s.navigation, content_w, s.badge_tick());
    let mut header = session_lines(s, content_w);
    if !rows.lines.is_empty() {
        header.push(Line::default());
        header.push(Line::from(Span::styled(
            "─".repeat(content_w),
            Style::default().fg(BORDER),
        )));
        header.push(Line::default());
    }
    let max_height = area.height.saturating_sub(2 * PANEL_TOP) as usize;
    let chrome = 2 * PANEL_PAD_Y as usize;
    let panel_height = (header.len() + rows.lines.len() + chrome).min(max_height) as u16;
    let panel = Rect::new(
        area.x + (area.width - panel_w) / 2,
        area.y + PANEL_TOP,
        panel_w,
        panel_height,
    );
    let body = Rect::new(
        panel.x + PANEL_PAD_X,
        panel.y + PANEL_PAD_Y,
        content_w as u16,
        panel.height.saturating_sub(2 * PANEL_PAD_Y),
    );
    let dashboard_height = (body.height as usize).saturating_sub(header.len());
    let overflow = rows.lines.len() > dashboard_height;
    let visible = dashboard_height.saturating_sub(usize::from(overflow));
    let start = rows.start(visible, s.navigation.scroll);
    let mut lines = header;
    lines.extend(rows.lines.iter().skip(start).take(visible).cloned());
    if overflow && dashboard_height > 0 {
        let end = (start + visible).min(rows.lines.len());
        lines.push(Line::from(Span::styled(
            truncate(
                &format!(
                    "{}–{} OF {} · PGUP/PGDN SCROLL",
                    start + 1,
                    end,
                    rows.lines.len()
                ),
                content_w,
            ),
            Style::default().fg(MUTED),
        )));
    }
    let buf = f.buffer_mut();
    sky::render(&sky::Ctx { tick: s.tick, area }, s.sky, s.motion, buf);
    // A clear margin separates moving sky cells from the frame and title.
    let quiet = Rect::new(
        panel.x.saturating_sub(1),
        panel.y.saturating_sub(1),
        panel.width + 2,
        panel.height + 2,
    )
    .intersection(area);
    Clear.render(quiet, buf);
    buf.set_style(quiet, Style::default().bg(BACKGROUND).fg(TEXT));
    draw_title(Rect::new(area.x, area.y + 1, area.width, 1), buf);
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .render(panel, buf);
    Paragraph::new(lines).render(body, buf);
    let motion = if s.motion { "ON" } else { "OFF" };
    let navigable = dashboard.entries().next().is_some();
    let hint = if navigable && content_w >= 62 {
        format!("↑↓ NAVIGATE   PGUP/PGDN SCROLL   M MOTION:{motion}   Q RELEASE")
    } else if navigable {
        "↑↓ NAVIGATE · PGUP/PGDN · M · Q".into()
    } else {
        format!("M MOTION:{motion}   Q RELEASE & LET IT SLEEP")
    };
    let hint_area = Rect::new(
        area.x + 1,
        area.bottom().saturating_sub(2),
        area.width.saturating_sub(2),
        1,
    );
    Clear.render(hint_area, buf);
    buf.set_style(hint_area, Style::default().bg(BACKGROUND));
    Paragraph::new(Line::from(Span::styled(
        truncate(&hint, hint_area.width as usize),
        Style::default().fg(MUTED),
    )))
    .alignment(Alignment::Center)
    .render(hint_area, buf);
    Some(Viewport {
        start,
        base: rows.start(visible, 0),
        max: rows.lines.len().saturating_sub(visible),
        height: visible,
    })
}

/// The combined layout's session block: Night Watch, the timers and their
/// rail, what's held, and power.
fn session_lines(s: &ViewState, cells: usize) -> Vec<Line<'static>> {
    let mut lines = vec![
        pair("NIGHT WATCH", "● AWAKE", cells, VIOLET_LIGHT, EMERALD),
        Line::default(),
    ];
    let time = match s.remaining {
        Some(remaining) => format!("{} REMAINING", clock(remaining)),
        None => format!("{} AWAKE", clock(s.elapsed)),
    };
    if s.remaining.is_some() && cells >= 48 {
        let elapsed = format!("{} ELAPSED", clock(s.elapsed));
        lines.push(pair(&time, &elapsed, cells, TEXT, MUTED));
    } else {
        lines.push(Line::from(Span::styled(
            time,
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )));
    }
    if let Some(frac) = s.progress() {
        lines.push(Line::from(rail(
            frac,
            cells.saturating_sub(5),
            [VIOLET_LIGHT, VIOLET_LIGHT, BORDER, MUTED],
        )));
    }
    lines.push(Line::default());
    let held = held_label(s.kinds);
    let release = if s.remaining.is_some() {
        "RELEASES AUTOMATICALLY"
    } else {
        "UNTIL YOU RELEASE"
    };
    if width(&held) + width(release) + 2 <= cells {
        lines.push(pair(&held, release, cells, MUTED, MUTED));
    } else {
        for text in [held.as_str(), release] {
            lines.push(Line::from(Span::styled(
                truncate(text, cells),
                Style::default().fg(MUTED),
            )));
        }
    }
    lines.push(Line::from(Span::styled(
        truncate(&s.power.label(), cells),
        Style::default().fg(if s.power.low() { AMBER } else { MUTED }),
    )));
    lines
}

// --- Shared session pieces ---------------------------------------------------

/// A progress rail `━━━◆──── nnn%`: `cells` of rail plus the five-cell value.
/// Colors are [done, marker, remaining, value].
fn rail(frac: f32, cells: usize, [done, marker, rest, value]: [Color; 4]) -> Vec<Span<'static>> {
    let filled = (frac * cells.saturating_sub(1) as f32).round() as usize;
    vec![
        Span::styled("━".repeat(filled), Style::default().fg(done)),
        Span::styled("◆", Style::default().fg(marker)),
        Span::styled(
            "─".repeat(cells.saturating_sub(filled + 1)),
            Style::default().fg(rest),
        ),
        Span::styled(
            format!(" {:>3}%", (frac * 100.0).round() as u8),
            Style::default().fg(value),
        ),
    ]
}

/// `DISPLAY + SYSTEM HELD`
fn held_label(kinds: &[Kind]) -> String {
    let names: Vec<String> = kinds.iter().map(|k| k.label().to_uppercase()).collect();
    format!("{} HELD", names.join(" + "))
}

/// The wordmark, centered in `area` on cleared cells, so a comet passing
/// behind it never shows through the letters.
fn draw_title(area: Rect, buf: &mut Buffer) {
    let x0 = area.x + area.width.saturating_sub(art::TITLE_W) / 2;
    for (dx, ch) in art::TITLE.chars().enumerate() {
        let x = x0 + dx as u16;
        if x >= area.right() {
            break;
        }
        if let Some(cell) = buf.cell_mut((x, area.y)) {
            cell.set_char(ch);
            cell.set_fg(if ch == ' ' {
                Color::Reset
            } else {
                VIOLET_LIGHT
            });
            cell.set_bg(BACKGROUND);
        }
    }
}

/// The single-line layout for terminals too small for anything else: the
/// sky still twinkles behind a centered wordmark and status line.
fn draw_compact(f: &mut Frame, s: &ViewState) {
    let area = f.area();
    let mid = area.y + area.height / 2;
    let mut status = format!("● AWAKE {}", clock(s.elapsed));
    if let Some(r) = s.remaining {
        status.push_str(&format!(" · {} LEFT", clock(r)));
    }
    status.push_str(" · Q RELEASE");

    let buf = f.buffer_mut();
    sky::render(&sky::Ctx { tick: s.tick, area }, s.sky, s.motion, buf);
    let centered = |y: u16, text: String, color: Color, buf: &mut Buffer| {
        Paragraph::new(Line::from(Span::styled(text, Style::default().fg(color))))
            .alignment(Alignment::Center)
            .render(Rect::new(area.x, y, area.width, 1), buf);
    };
    if mid > area.y {
        centered(mid - 1, art::TITLE.into(), VIOLET_LIGHT, buf);
    }
    if mid + 1 < area.bottom() {
        centered(mid + 1, status, SLATE_400, buf);
    }
}

// --- Separate-box layout -----------------------------------------------------

/// Greedy top-down placement of the cards in `idx` into `avail` rows:
/// (card index, card height, rows shown) for each one that fits. Cards in
/// `skip` and cards with no headroom left land in `fold` instead.
fn place_column(
    cards: &[SideCard],
    idx: &[usize],
    mut avail: u16,
    skip: &[usize],
    fold: &mut Vec<usize>,
) -> Vec<(usize, u16, usize)> {
    let mut placed = Vec::new();
    for &i in idx {
        let n = cards[i].rows.len();
        let gap = u16::from(!placed.is_empty());
        let budget = (avail.saturating_sub(gap) as usize).saturating_sub(CARD_CHROME as usize);
        if skip.contains(&i) || budget < 2 {
            fold.push(i);
            continue;
        }
        let shown = if n <= budget { n } else { budget - 1 };
        let h = (shown + usize::from(n > shown)) as u16 + CARD_CHROME;
        avail = avail.saturating_sub(h + gap);
        placed.push((i, h, shown));
    }
    placed
}

/// The status card with cards beside it on a wide terminal (left-side cards
/// in a third column when there's room) or docked beneath it on a narrow one.
/// Cards get as many rows as the height allows; a card with no headroom left
/// folds into a summary row inside the status card.
fn draw_separate(f: &mut Frame, s: &ViewState) {
    let area = f.area();
    if too_small(area, SEPARATE_MIN) {
        draw_compact(f, s);
        return;
    }
    let all: Vec<usize> = (0..s.cards.len()).collect();
    let wide = !all.is_empty() && area.width >= MAIN_W + GAP + SIDE_W + 2;
    let left_idx: Vec<usize> = all.iter().copied().filter(|&i| s.cards[i].left).collect();
    // When *every* card is `left` there is no third column — the single
    // column simply swaps to the other side of the status card.
    let wide3 = wide
        && !left_idx.is_empty()
        && left_idx.len() < all.len()
        && area.width >= MAIN_W + 2 * (GAP + SIDE_W) + 2;
    let right_idx: Vec<usize> = all
        .iter()
        .copied()
        .filter(|&i| !(wide3 && s.cards[i].left))
        .collect();
    let bw = MAIN_W.min(area.width.saturating_sub(2));
    let main_base = status_lines(s, &[], bw).len() as u16 + STATUS_CHROME;

    // Folding a card adds a summary row to the status card, which in the
    // stacked layout shrinks the very budget being divided — so placement
    // reruns until the folded set stops changing.
    let mut placed: Vec<(usize, u16, usize)>;
    let mut placed_left: Vec<(usize, u16, usize)>;
    let mut folded: Vec<usize> = Vec::new();
    loop {
        let growth = if folded.is_empty() {
            0
        } else {
            folded.len() as u16 + 1
        };
        let avail = area
            .height
            .saturating_sub(SURROUND)
            .saturating_sub(if wide { 0 } else { main_base + growth + 1 });
        let mut refold: Vec<usize> = Vec::new();
        placed_left = if wide3 {
            place_column(s.cards, &left_idx, avail, &folded, &mut refold)
        } else {
            Vec::new()
        };
        placed = place_column(s.cards, &right_idx, avail, &folded, &mut refold);
        if refold == folded {
            break;
        }
        folded = refold;
    }

    let lines = status_lines(s, &folded, bw);
    let main_h = lines.len() as u16 + STATUS_CHROME;
    let col_h = |p: &[(usize, u16, usize)]| -> u16 {
        p.iter().map(|(_, h, _)| h).sum::<u16>() + p.len().saturating_sub(1) as u16
    };
    let side_h = col_h(&placed).max(col_h(&placed_left));
    let cards_h = if wide {
        main_h.max(side_h)
    } else {
        main_h + if side_h > 0 { 1 + side_h } else { 0 }
    };
    let content_h = art::TITLE_H + 1 + cards_h + 2;
    let y0 = area.y + area.height.saturating_sub(content_h) / 2;
    let title_rect = Rect::new(area.x, y0, area.width, art::TITLE_H);
    let cards_y = y0 + art::TITLE_H + 1;
    // The side column widens past SIDE_W to hold its longest row whole —
    // capped by SIDE_MAX_W and by the room this layout actually has — so
    // usage details never get clipped just because the box was born narrow.
    let room = if wide3 {
        area.width.saturating_sub(MAIN_W + 2 * GAP + 2) / 2
    } else if wide {
        area.width.saturating_sub(MAIN_W + GAP + 2)
    } else {
        area.width.saturating_sub(2)
    };
    let sw = s
        .cards
        .iter()
        .map(cards::natural_width)
        .max()
        .unwrap_or(SIDE_W)
        .clamp(SIDE_W, SIDE_MAX_W)
        .min(room);
    let (main_x, left_x, side_x, side_y0) = if wide3 && side_h > 0 {
        let gx = area.x + area.width.saturating_sub(MAIN_W + 2 * (GAP + sw)) / 2;
        let mx = gx + sw + GAP;
        (mx, gx, mx + MAIN_W + GAP, cards_y)
    } else if wide && side_h > 0 {
        let gx = area.x + area.width.saturating_sub(MAIN_W + GAP + sw) / 2;
        if !placed.is_empty() && placed.iter().all(|&(i, _, _)| s.cards[i].left) {
            (gx + sw + GAP, gx, gx, cards_y)
        } else {
            (gx, gx, gx + MAIN_W + GAP, cards_y)
        }
    } else {
        let x = area.x + (area.width - bw) / 2;
        let sx = area.x + area.width.saturating_sub(sw) / 2;
        (x, x, sx, cards_y + main_h + 1)
    };
    // Everything below is clamped to the screen: a pathological config (say,
    // a dozen folded cards on an 18-row terminal) clips instead of panicking.
    let brect = Rect::new(main_x, cards_y, bw, main_h).intersection(area);
    let mut side_rects: Vec<(usize, Rect, usize)> = Vec::new();
    for (col_x, col) in [(left_x, &placed_left), (side_x, &placed)] {
        let mut sy = side_y0;
        for &(i, h, shown) in col {
            side_rects.push((i, Rect::new(col_x, sy, sw, h).intersection(area), shown));
            sy += h + 1;
        }
    }
    let hint_y = side_rects
        .iter()
        .map(|(_, r, _)| r.bottom())
        .fold(brect.bottom(), u16::max)
        + 1;

    let buf = f.buffer_mut();
    sky::render(&sky::Ctx { tick: s.tick, area }, s.sky, s.motion, buf);
    draw_title(title_rect, buf);

    Clear.render(brect, buf);
    buf.set_style(brect, Style::default().bg(BACKGROUND));
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Line::from(vec![
            Span::styled(" ● ", Style::default().fg(EMERALD)),
            Span::styled(
                "AWAKE ",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ),
        ]))
        .title_alignment(Alignment::Left);
    let inner = block.inner(brect);
    block.render(brect, buf);
    Paragraph::new(lines).render(inner, buf);

    for (i, rect, shown) in &side_rects {
        cards::render_side_card(&s.cards[*i], *shown, *rect, s.badge_tick(), buf);
    }

    if hint_y < area.bottom() {
        Paragraph::new(Line::from(vec![
            Span::styled(
                " Q ",
                Style::default()
                    .fg(VIOLET_LIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("RELEASE & LET IT SLEEP", Style::default().fg(SLATE_600)),
        ]))
        .alignment(Alignment::Center)
        .render(Rect::new(area.x, hint_y, area.width, 1), buf);
    }
}

/// The status card body. `folded` lists cards that didn't fit as cards and
/// appear here as one-line summaries instead.
fn status_lines(s: &ViewState, folded: &[usize], box_w: u16) -> Vec<Line<'static>> {
    let inner_w = box_w.saturating_sub(2) as usize;
    let indent = || Span::raw("   ");
    let bold = Style::default().fg(TEXT).add_modifier(Modifier::BOLD);
    let mut lines = vec![
        Line::default(),
        split_line(
            vec![indent(), Span::styled("NIGHT WATCH", bold.fg(VIOLET_LIGHT))],
            kind_spans(s.kinds),
            inner_w,
        ),
        Line::default(),
    ];

    match s.remaining {
        Some(r) => {
            lines.push(split_line(
                vec![
                    indent(),
                    Span::styled(format!("{} AWAKE", clock(s.elapsed)), bold),
                ],
                vec![Span::styled(format!("{} REMAINING", clock(r)), bold)],
                inner_w,
            ));
            let mut spans = vec![indent()];
            spans.extend(rail(
                s.progress().unwrap_or(0.0),
                inner_w.saturating_sub(8),
                [VIOLET_LIGHT, GOLD, SLATE_700, SLATE_400],
            ));
            lines.push(Line::from(spans));
        }
        None => {
            lines.push(Line::from(vec![
                indent(),
                Span::styled(clock(s.elapsed), bold),
                Span::styled(" AWAKE", Style::default().fg(SLATE_400)),
            ]));
            let rail_w = inner_w.saturating_sub(8);
            lines.push(Line::from(vec![
                indent(),
                Span::styled("━".repeat(rail_w / 2), Style::default().fg(VIOLET_LIGHT)),
                Span::styled("◆", Style::default().fg(GOLD)),
                Span::styled(
                    "━".repeat(rail_w.saturating_sub(rail_w / 2 + 1)),
                    Style::default().fg(SLATE_700),
                ),
                Span::styled(" ∞", Style::default().fg(SLATE_400)),
            ]));
        }
    }
    let (glyph, power_color) = match (s.power.on_ac, s.power.low()) {
        (true, _) => ("↯", EMERALD),
        (false, true) => ("◐", RED),
        (false, false) => ("◐", AMBER),
    };
    lines.push(split_line(
        vec![
            indent(),
            Span::styled(
                format!("{glyph} {}", s.power.label()),
                Style::default().fg(power_color),
            ),
        ],
        vec![Span::styled(
            if s.remaining.is_some() {
                "AUTO RELEASE ARMED"
            } else {
                "MANUAL RELEASE"
            },
            Style::default().fg(SLATE_600),
        )],
        inner_w,
    ));

    if !folded.is_empty() {
        lines.push(Line::default());
    }
    for &i in folded {
        let card = &s.cards[i];
        let bad = card
            .rows
            .iter()
            .filter(|r| r.health == Some(Health::Bad))
            .count();
        let mut summary = vec![Span::styled(card.summary.to_uppercase(), bold)];
        if bad > 0 {
            summary.push(Span::styled(
                format!(" · {bad} BAD"),
                Style::default().fg(RED),
            ));
        }
        lines.push(split_line(
            vec![
                indent(),
                Span::styled(
                    format!(
                        "{} {}",
                        card.glyph,
                        truncate(&card.title.to_uppercase(), 18)
                    ),
                    Style::default().fg(scale(card.accent, 1.0)),
                ),
            ],
            summary,
            inner_w,
        ));
    }
    lines
}

/// Spans flush left and right across `cells`, keeping a three-cell margin
/// on the right to match the left indent.
fn split_line(
    mut left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    cells: usize,
) -> Line<'static> {
    const RIGHT_PAD: usize = 3;
    let spans_w = |spans: &[Span]| spans.iter().map(|s| width(&s.content)).sum::<usize>();
    let used = spans_w(&left) + spans_w(&right) + RIGHT_PAD;
    left.push(Span::raw(" ".repeat(cells.saturating_sub(used))));
    left.extend(right);
    left.push(Span::raw(" ".repeat(RIGHT_PAD)));
    Line::from(left)
}

/// `● DISPLAY  ● SYSTEM`, each held kind with its own dot color.
fn kind_spans(kinds: &[Kind]) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (i, kind) in kinds.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let dot = match kind {
            Kind::Display => (56, 189, 248),
            Kind::Idle => (52, 211, 153),
            Kind::System => GOLD_RGB,
            Kind::Disk => (192, 132, 252),
        };
        spans.push(Span::styled("●", Style::default().fg(scale(dot, 0.75))));
        spans.push(Span::styled(
            format!(" {}", kind.label().to_uppercase()),
            Style::default().fg(SLATE_400),
        ));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collect::CardRow;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn paging_does_not_accumulate_invisible_overscroll() {
        let bottom = Viewport {
            start: 20,
            base: 5,
            max: 20,
            height: 8,
        };
        assert_eq!(bottom.page(8), 15);
        assert_eq!(bottom.page(-8), 7);
        let top = Viewport {
            start: 0,
            base: 5,
            max: 20,
            height: 8,
        };
        assert_eq!(top.page(-8), -5);
        assert_eq!(top.page(8), 3);
    }

    const KINDS: [Kind; 2] = [Kind::Display, Kind::Idle];
    const POWER: PowerStatus = PowerStatus {
        on_ac: true,
        percent: Some(100),
    };
    const SKY: [bool; 2] = [true; 2];

    fn state<'a>(
        cards: &'a [SideCard],
        dashboard: Option<&'a Dashboard>,
        navigation: &'a Navigation,
    ) -> ViewState<'a> {
        ViewState {
            tick: 7,
            elapsed: Duration::from_secs(2537),
            total: Some(Duration::from_secs(7200)),
            remaining: Some(Duration::from_secs(4663)),
            kinds: &KINDS,
            power: &POWER,
            cards,
            dashboard,
            navigation,
            sky: &SKY,
            motion: true,
        }
    }

    fn meter_card(left: bool) -> SideCard {
        SideCard {
            title: "usage".into(),
            glyph: "✦".into(),
            accent: (148, 163, 184),
            rows: vec![
                CardRow {
                    name: "✻ claude".into(),
                    accent: Some((217, 119, 87)),
                    header: true,
                    ..CardRow::default()
                },
                CardRow {
                    health: Some(Health::Good),
                    name: "▸ personal".into(),
                    detail: Some("42% used".into()),
                    ..CardRow::default()
                },
                CardRow {
                    health: Some(Health::Bad),
                    name: "work".into(),
                    detail: Some("limit reached".into()),
                    ..CardRow::default()
                },
                CardRow {
                    name: "weekly".into(),
                    detail: Some("resets Sun 03:49".into()),
                    accent: Some((217, 119, 87)),
                    percent: Some(96),
                    ..CardRow::default()
                },
            ],
            summary: "4 items".into(),
            left,
            stale: true,
        }
    }

    #[test]
    fn draw_survives_any_terminal_size_and_layout() {
        // Every layout branch — compact, stacked, wide, three-column,
        // combined — across sizes from degenerate to spacious, all uppercase.
        assert_eq!(SKY.len(), sky::FLYERS.len());
        let nav = Navigation::default();
        for (w, h) in [(1, 1), (40, 8), (65, 30), (66, 18), (90, 24), (150, 45)] {
            for (cards, combined) in [
                (vec![], false),
                (vec![meter_card(false)], false),
                (vec![meter_card(true), meter_card(false)], false),
                (vec![meter_card(false)], true),
                (vec![], true),
            ] {
                let dashboard = combined.then(|| Dashboard::from_cards(&cards));
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                terminal
                    .draw(|f| {
                        draw(f, &state(&cards, dashboard.as_ref(), &nav));
                    })
                    .unwrap();
                assert!(
                    terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .all(|cell| !cell.symbol().chars().any(char::is_lowercase)),
                    "lowercase text in {w}x{h}, combined={combined}"
                );
            }
        }
    }

    #[test]
    fn status_rows_respect_the_card_width() {
        let nav = Navigation::default();
        let bounded = state(&[], None, &nav);
        let mut unbounded = state(&[], None, &nav);
        unbounded.total = None;
        unbounded.remaining = None;
        for box_w in [MAIN_W, 84, 100] {
            let inner = box_w as usize - 2;
            for s in [&bounded, &unbounded] {
                for line in status_lines(s, &[], box_w) {
                    assert!(
                        line.width() <= inner,
                        "status row overflows {inner}: {line:?}"
                    );
                }
            }
            let progress = status_lines(&bounded, &[], box_w)[4].to_string();
            assert!(progress.ends_with("  35%"), "{progress}");
        }
        for cells in [20, 48, 102] {
            for line in session_lines(&bounded, cells) {
                assert!(
                    line.width() <= cells,
                    "session row overflows {cells}: {line:?}"
                );
            }
        }
    }
}
