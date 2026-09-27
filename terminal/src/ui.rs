//! The TUI: terminal setup, the render loop, and the layout — a status card
//! plus the dashboard cards built by [`crate::cards`], floating over the sky
//! rendered by [`crate::sky`].

use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Widget},
    Frame, Terminal,
};

use crate::art;
use crate::cards::{self, SideCard};
use crate::collect::{self, CardData, Container, Health, Update};
use crate::config::{Config, FileConfig};
use crate::dashboard::{Dashboard, Navigation};
use crate::power::{Kind, PowerStatus};
use crate::sky;
use crate::theme::{
    scale, AMBER, BACKGROUND, BORDER, EMERALD, GOLD, MUTED, RED, SLATE_400, SLATE_600, SLATE_700,
    TEXT, VIOLET_LIGHT,
};

const MAIN_W: u16 = 64;
const MAIN_MAX_W: u16 = 84;
const SIDE_W: u16 = 40;
/// Side cards grow past [`SIDE_W`] to fit their longest row, up to this.
const SIDE_MAX_W: u16 = 64;
const GAP: u16 = 2;

/// Everything the renderer needs for one frame.
pub struct ViewState {
    pub tick: u64,
    pub elapsed: Duration,
    pub total: Option<Duration>,
    pub remaining: Option<Duration>,
    pub kinds: Vec<Kind>,
    pub power: PowerStatus,
    /// Dashboard cards with data, in display order (custom cards, then docker).
    pub cards: Vec<SideCard>,
    /// `[dashboard] combined` — render the (single, pre-folded) card's rows
    /// inside the status box itself, so the whole dashboard is one box.
    pub merged: bool,
    /// Per-flyer sky toggles, aligned with [`sky::FLYERS`].
    pub sky: Vec<bool>,
    pub motion: bool,
    pub navigation: Navigation,
}

pub fn run(cfg: &Config, kinds: &[Kind], fc: &FileConfig) -> io::Result<Duration> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        orig_hook(info);
    }));

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let start = Instant::now();
    let deadline = cfg.timeout.map(|t| start + t);
    let updates = collect::spawn(fc.docker.enabled, fc.cards.clone());
    let sky_on = sky::resolve(&fc.sky);
    let mut motion = *fc.sky.get("motion").unwrap_or(&true);
    let mut navigation = Navigation::default();
    let mut power = PowerStatus::poll();
    let mut docker: Option<Vec<Container>> = None;
    let mut card_data: Vec<Option<CardData>> = vec![None; fc.cards.len()];
    let mut tick: u64 = 0;

    let result = loop {
        let now = Instant::now();
        if let Some(d) = deadline {
            if now >= d {
                break Ok(());
            }
        }
        while let Ok(update) = updates.try_recv() {
            match update {
                Update::Power(p) => power = p,
                Update::Docker(d) => docker = d,
                Update::Card(i, d) => card_data[i] = d,
            }
        }
        let cards = cards::build_cards(&fc.cards, &docker, &card_data, &fc.dashboard);
        let dashboard = Dashboard::from_cards(&cards, fc.dashboard.combined);
        navigation.sync(&dashboard);
        let state = ViewState {
            tick,
            elapsed: start.elapsed(),
            total: cfg.timeout,
            remaining: deadline.map(|d| d.saturating_duration_since(now)),
            kinds: kinds.to_vec(),
            power: power.clone(),
            cards,
            merged: fc.dashboard.combined,
            sky: sky_on.clone(),
            motion,
            navigation: navigation.clone(),
        };
        let mut viewport = None;
        if let Err(e) = terminal.draw(|f| {
            if state.merged {
                viewport = draw_focused(f, &state);
            } else {
                draw(f, &state);
            }
        }) {
            break Err(e);
        }
        let ready = match event::poll(Duration::from_millis(if motion { 120 } else { 250 })) {
            Ok(ready) => ready,
            Err(error) => break Err(error),
        };
        if ready {
            let event = match event::read() {
                Ok(event) => event,
                Err(error) => break Err(error),
            };
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    let quit = matches!(
                        key.code,
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc
                    ) || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL));
                    if quit {
                        break Ok(());
                    }
                    if matches!(key.code, KeyCode::Char('m') | KeyCode::Char('M')) {
                        motion = !motion;
                    }
                    if fc.dashboard.combined {
                        match key.code {
                            KeyCode::Down | KeyCode::Char('j') => {
                                navigation.move_selection(&dashboard, 1)
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                navigation.move_selection(&dashboard, -1)
                            }
                            KeyCode::PageDown => {
                                if let Some(viewport) = &viewport {
                                    navigation.scroll =
                                        viewport.page(viewport.height.max(1) as isize);
                                }
                            }
                            KeyCode::PageUp => {
                                if let Some(viewport) = &viewport {
                                    navigation.scroll =
                                        viewport.page(-(viewport.height.max(1) as isize));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        tick += 1;
    };

    restore_terminal()?;
    result.map(|_| start.elapsed())
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, cursor::Show)
}

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
        let budget = (avail.saturating_sub(gap) as usize).saturating_sub(4);
        if skip.contains(&i) || budget < 2 {
            fold.push(i);
            continue;
        }
        let shown = if n <= budget { n } else { budget - 1 };
        let h = (shown + usize::from(n > shown) + 4) as u16;
        avail = avail.saturating_sub(h + gap);
        placed.push((i, h, shown));
    }
    placed
}

pub fn draw(f: &mut Frame, s: &ViewState) {
    let area = f.area();

    f.buffer_mut()
        .set_style(area, Style::default().bg(BACKGROUND).fg(TEXT));
    if s.merged {
        draw_focused(f, s);
        return;
    }

    if area.width < 66 || area.height < 18 {
        draw_compact(f, s);
        return;
    }

    // Merged mode: the dashboard is one pre-folded card whose rows render
    // inside the status box itself — there are no side cards at all.
    let merged_card = if s.merged { s.cards.first() } else { None };
    // The side cards sail beside the status card on a wide terminal and dock
    // beneath it on a narrow one. Cards are placed in order, each getting as
    // many rows as the height allows; a card with no headroom left shrinks
    // to a summary row inside the status card.
    let has_side = !s.cards.is_empty() && !s.merged;
    let wide = has_side && area.width >= MAIN_W + GAP + SIDE_W + 2;
    // Cards marked `left` get their own column when the terminal holds three;
    // otherwise they flow into the right-hand stack with everything else.
    // When *every* card is `left` there is no third column — the single
    // column simply swaps to the other side of the status card. In merged
    // mode nothing is placed at all; the rows live in the status box.
    let card_idx: Vec<usize> = if s.merged {
        Vec::new()
    } else {
        (0..s.cards.len()).collect()
    };
    let left_idx: Vec<usize> = card_idx
        .iter()
        .copied()
        .filter(|&i| s.cards[i].left)
        .collect();
    let wide3 = wide
        && !left_idx.is_empty()
        && left_idx.len() < card_idx.len()
        && area.width >= MAIN_W + 2 * (GAP + SIDE_W) + 2;
    let right_idx: Vec<usize> = card_idx
        .iter()
        .copied()
        .filter(|&i| !(wide3 && s.cards[i].left))
        .collect();
    // A combined dashboard gets the extra room a large terminal offers,
    // while standalone status cards preserve space for their side columns.
    // MAIN_MAX_W is the box's comfortable size, not its ceiling: a card whose
    // widest row needs more grows past it rather than clipping the row, as
    // far as the terminal allows.
    let bw = match merged_card {
        Some(card) => {
            let max = MAIN_MAX_W.max(cards::natural_width(card));
            area.width
                .saturating_sub(4)
                .clamp(MAIN_W, max)
                .min(area.width.saturating_sub(2))
        }
        None => MAIN_W.min(area.width.saturating_sub(2)),
    };
    let main_base = status_lines(s, &[], bw).len() as u16 + 3;

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
            .saturating_sub(art::TITLE_H + 1 + 2 + 2)
            .saturating_sub(if wide { 0 } else { main_base + growth + 1 });
        let mut refold: Vec<usize> = Vec::new();
        placed_left = if wide3 {
            place_column(&s.cards, &left_idx, avail, &folded, &mut refold)
        } else {
            Vec::new()
        };
        placed = place_column(&s.cards, &right_idx, avail, &folded, &mut refold);
        if refold == folded {
            break;
        }
        folded = refold;
    }

    let mut lines = status_lines(s, &folded, bw);
    if let Some(card) = merged_card {
        // As many rows as the height allows, then a "+N MORE" line.
        let budget = area.height.saturating_sub(art::TITLE_H + 1 + 2 + 2 + 3) as usize;
        let base = lines.len() + 1;
        let n = card.rows.len();
        let shown = if base + n <= budget {
            n
        } else {
            budget.saturating_sub(base + 1)
        };
        lines.push(Line::default());
        lines.extend(cards::card_row_lines(
            &card.rows,
            shown,
            bw.saturating_sub(2) as usize,
            s.tick,
        ));
    }
    let main_h = lines.len() as u16 + 3;
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
    let title_rect = Rect {
        x: area.x + area.width.saturating_sub(art::TITLE_W) / 2,
        y: y0,
        width: art::TITLE_W.min(area.width),
        height: art::TITLE_H,
    };
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
    let (main_x, left_x, side_x, side_w, side_y0) = if wide3 && side_h > 0 {
        let gx = area.x + area.width.saturating_sub(MAIN_W + 2 * (GAP + sw)) / 2;
        let mx = gx + sw + GAP;
        (mx, gx, mx + MAIN_W + GAP, sw, cards_y)
    } else if wide && side_h > 0 {
        let gx = area.x + area.width.saturating_sub(MAIN_W + GAP + sw) / 2;
        if !placed.is_empty() && placed.iter().all(|&(i, _, _)| s.cards[i].left) {
            (gx + sw + GAP, gx, gx, sw, cards_y)
        } else {
            (gx, gx, gx + MAIN_W + GAP, sw, cards_y)
        }
    } else {
        let x = area.x + (area.width - bw) / 2;
        let sx = area.x + area.width.saturating_sub(sw) / 2;
        (x, x, sx, sw, cards_y + main_h + 1)
    };
    // Everything below is clamped to the screen: a pathological config (say,
    // a dozen folded cards on an 18-row terminal) clips instead of panicking.
    let brect = Rect {
        x: main_x,
        y: cards_y,
        width: bw,
        height: main_h,
    }
    .intersection(area);
    let mut side_rects: Vec<(usize, Rect, usize)> = Vec::new();
    for (col_x, col) in [(left_x, &placed_left), (side_x, &placed)] {
        let mut sy = side_y0;
        for &(i, h, shown) in col {
            side_rects.push((
                i,
                Rect {
                    x: col_x,
                    y: sy,
                    width: side_w,
                    height: h,
                }
                .intersection(area),
                shown,
            ));
            sy += h + 1;
        }
    }
    let hint_y = side_rects
        .iter()
        .map(|(_, r, _)| r.bottom())
        .max()
        .map_or(brect.bottom(), |b| b.max(brect.bottom()))
        + 1;

    let ctx = sky::Ctx { tick: s.tick, area };
    let buf = f.buffer_mut();
    sky::render_motion(&ctx, &s.sky, s.motion, buf);

    Title { tick: s.tick }.render(title_rect, buf);

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
        cards::render_side_card(
            &s.cards[*i],
            *shown,
            *rect,
            if s.motion { s.tick } else { 0 },
            buf,
        );
    }

    if hint_y < area.bottom() {
        let hint = Rect {
            x: area.x,
            y: hint_y,
            width: area.width,
            height: 1,
        };
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
        .render(hint, buf);
    }
}

/// The session stays anchored while the dashboard scrolls independently.
struct Viewport {
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

fn draw_focused(f: &mut Frame, s: &ViewState) -> Option<Viewport> {
    let area = f.area();
    f.buffer_mut()
        .set_style(area, Style::default().bg(BACKGROUND).fg(TEXT));
    if area.width < 32 || area.height < 14 {
        draw_compact(f, s);
        return None;
    }
    let width = area.width.saturating_sub(4).min(110);
    let content_w = width.saturating_sub(8) as usize;
    let dashboard = Dashboard::from_cards(&s.cards, true);
    let rows =
        dashboard.render_animated(&s.navigation, content_w, if s.motion { s.tick } else { 0 });
    let mut header = focused_status(s, content_w);
    if !rows.lines.is_empty() {
        header.push(Line::default());
        header.push(Line::from(Span::styled(
            "─".repeat(content_w),
            Style::default().fg(BORDER),
        )));
        header.push(Line::default());
    }
    let max_height = area.height.saturating_sub(6);
    let panel_height = (header.len() + rows.lines.len() + 4).min(max_height as usize) as u16;
    let panel = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + 3,
        width,
        panel_height,
    );
    let body = Rect::new(
        panel.x + 4,
        panel.y + 2,
        width.saturating_sub(8),
        panel.height.saturating_sub(4),
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
            cards::truncate(
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
    sky::render_motion(&sky::Ctx { tick: s.tick, area }, &s.sky, s.motion, buf);
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
    let title = Rect::new(area.x, area.y + 1, area.width, 1);
    Title { tick: 0 }.render(title, buf);
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .render(panel, buf);
    Paragraph::new(lines).render(body, buf);
    let motion = if s.motion { "ON" } else { "OFF" };
    let hint = if content_w >= 62 && dashboard.entries().next().is_some() {
        format!("↑↓ NAVIGATE   PGUP/PGDN SCROLL   M MOTION:{motion}   Q RELEASE")
    } else if dashboard.entries().next().is_some() {
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
        cards::truncate(&hint, hint_area.width as usize),
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

fn focused_status(s: &ViewState, width: usize) -> Vec<Line<'static>> {
    let mut lines = vec![
        crate::dashboard::pair("NIGHT WATCH", "● AWAKE", width, VIOLET_LIGHT, EMERALD),
        Line::default(),
    ];
    let time = match s.remaining {
        Some(remaining) => format!("{} REMAINING", fmt_hms(remaining)),
        None => format!("{} AWAKE", fmt_hms(s.elapsed)),
    };
    if s.remaining.is_some() && width >= 48 {
        lines.push(crate::dashboard::pair(
            &time,
            &format!("{} ELAPSED", fmt_hms(s.elapsed)),
            width,
            TEXT,
            MUTED,
        ));
    } else {
        lines.push(Line::from(Span::styled(
            time,
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )));
    }
    if let (Some(remaining), Some(total)) = (s.remaining, s.total) {
        let fraction =
            (1.0 - remaining.as_secs_f32() / total.as_secs_f32().max(1.0)).clamp(0.0, 1.0);
        let rail = width.saturating_sub(5);
        let filled = (fraction * rail.saturating_sub(1) as f32).round() as usize;
        lines.push(Line::from(vec![
            Span::styled("━".repeat(filled), Style::default().fg(VIOLET_LIGHT)),
            Span::styled("◆", Style::default().fg(VIOLET_LIGHT)),
            Span::styled(
                "─".repeat(rail.saturating_sub(filled + 1)),
                Style::default().fg(BORDER),
            ),
            Span::styled(
                format!(" {:>3}%", (fraction * 100.0).round() as u8),
                Style::default().fg(MUTED),
            ),
        ]));
    }
    lines.push(Line::default());
    let held = format!(
        "{} HELD",
        s.kinds
            .iter()
            .map(|k| k.label().to_uppercase())
            .collect::<Vec<_>>()
            .join(" + ")
    );
    let release = if s.remaining.is_some() {
        "RELEASES AUTOMATICALLY"
    } else {
        "UNTIL YOU RELEASE"
    };
    if Line::from(held.as_str()).width() + release.len() + 2 <= width {
        lines.push(crate::dashboard::pair(&held, release, width, MUTED, MUTED));
    } else {
        lines.push(Line::from(Span::styled(
            cards::truncate(&held, width),
            Style::default().fg(MUTED),
        )));
        lines.push(Line::from(Span::styled(
            cards::truncate(release, width),
            Style::default().fg(MUTED),
        )));
    }
    let power = match (s.power.on_ac, s.power.percent) {
        (true, Some(percent)) => format!("AC POWER · BATTERY {percent}%"),
        (true, None) => "AC POWER".into(),
        (false, Some(percent)) => format!("BATTERY {percent}%"),
        (false, None) => "BATTERY".into(),
    };
    lines.push(Line::from(Span::styled(
        cards::truncate(&power, width),
        Style::default().fg(
            if s.power.percent.is_some_and(|p| p < 20) && !s.power.on_ac {
                AMBER
            } else {
                MUTED
            },
        ),
    )));
    lines
}

/// The single-line layout for terminals too small for the full art: the sky
/// still twinkles behind a centered wordmark and status line.
fn draw_compact(f: &mut Frame, s: &ViewState) {
    let area = f.area();
    let mid = area.y + area.height / 2;
    let spans = vec![Span::styled(
        art::TITLE[0],
        Style::default().fg(VIOLET_LIGHT),
    )];
    let mut status = format!("● AWAKE {}", fmt_hms(s.elapsed));
    if let Some(r) = s.remaining {
        status.push_str(&format!(" · {} LEFT", fmt_hms(r)));
    }
    status.push_str(" · Q RELEASE");

    let ctx = sky::Ctx { tick: s.tick, area };
    let buf = f.buffer_mut();
    sky::render_motion(&ctx, &s.sky, s.motion, buf);
    if mid > area.y {
        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .render(
                Rect {
                    x: area.x,
                    y: mid - 1,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
    }
    if mid + 1 < area.bottom() {
        Paragraph::new(Line::from(Span::styled(
            status,
            Style::default().fg(SLATE_400),
        )))
        .alignment(Alignment::Center)
        .render(
            Rect {
                x: area.x,
                y: mid + 1,
                width: area.width,
                height: 1,
            },
            buf,
        );
    }
}

/// The status card body. `folded` lists cards that didn't fit as cards and
/// appear here as one-line summaries instead.
fn status_lines(s: &ViewState, folded: &[usize], display_width: u16) -> Vec<Line<'static>> {
    let inner_w = display_width.saturating_sub(2) as usize;
    let mut lines = vec![Line::default(), watch_header(s, inner_w), Line::default()];

    match s.remaining {
        Some(r) => {
            lines.push(split_line(
                vec![
                    Span::raw("   "),
                    Span::styled(
                        format!("{} AWAKE", fmt_hms(s.elapsed)),
                        Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                    ),
                ],
                vec![Span::styled(
                    format!("{} REMAINING", fmt_hms(r)),
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                )],
                inner_w,
            ));
            let total = s.total.unwrap_or(r).as_secs_f32().max(1.0);
            let elapsed_frac = (1.0 - r.as_secs_f32() / total).clamp(0.0, 1.0);
            lines.push(timeline(elapsed_frac, inner_w));
        }
        None => {
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    fmt_hms(s.elapsed),
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" AWAKE", Style::default().fg(SLATE_400)),
            ]));
            let rail_w = inner_w.saturating_sub(8);
            lines.push(Line::from(vec![
                Span::raw("   "),
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
    lines.push(split_line(
        {
            let mut spans = vec![Span::raw("   ")];
            spans.extend(power_spans(&s.power));
            spans
        },
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

    let mut extra = Vec::new();
    for &i in folded {
        let card = &s.cards[i];
        let bad = card
            .rows
            .iter()
            .filter(|r| r.health == Some(Health::Bad))
            .count();
        let mut spans = vec![Span::styled(
            card.summary.to_uppercase(),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )];
        if bad > 0 {
            spans.push(Span::styled(
                format!(" · {bad} BAD"),
                Style::default().fg(RED),
            ));
        }
        extra.push(split_line(
            vec![
                Span::raw("   "),
                Span::styled(
                    format!(
                        "{} {}",
                        card.glyph,
                        cards::truncate(&card.title.to_uppercase(), 18)
                    ),
                    Style::default().fg(scale(card.accent, 1.0)),
                ),
            ],
            spans,
            inner_w,
        ));
    }
    if !extra.is_empty() {
        lines.push(Line::default());
        lines.extend(extra);
    }
    lines
}

fn split_line(
    mut left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    width: usize,
) -> Line<'static> {
    const RIGHT_PAD: usize = 3;
    let used = span_width(&left) + span_width(&right) + RIGHT_PAD;
    left.push(Span::raw(" ".repeat(width.saturating_sub(used))));
    left.extend(right);
    left.push(Span::raw(" ".repeat(RIGHT_PAD)));
    Line::from(left)
}

fn span_width(spans: &[Span<'static>]) -> usize {
    spans.iter().map(|span| span.content.chars().count()).sum()
}

fn watch_header(s: &ViewState, width: usize) -> Line<'static> {
    let left = vec![
        Span::raw("   "),
        Span::styled(
            "NIGHT WATCH",
            Style::default()
                .fg(VIOLET_LIGHT)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    split_line(left, mode_spans(s), width)
}

fn mode_spans(s: &ViewState) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (i, kind) in s.kinds.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default().fg(SLATE_700)));
        }
        let rgb = match kind {
            Kind::Display => (56, 189, 248),
            Kind::Idle => (52, 211, 153),
            Kind::System => (251, 191, 36),
            Kind::Disk => (192, 132, 252),
        };
        spans.push(Span::styled("●", Style::default().fg(scale(rgb, 0.75))));
        spans.push(Span::styled(
            format!(" {}", kind.label().to_uppercase()),
            Style::default().fg(SLATE_400),
        ));
    }
    spans
}

fn timeline(frac: f32, width: usize) -> Line<'static> {
    let rail_w = width.saturating_sub(8);
    let marker = if rail_w > 0 {
        (frac * rail_w.saturating_sub(1) as f32).round() as usize
    } else {
        0
    };
    let mut spans = vec![
        Span::raw("   "),
        Span::styled("━".repeat(marker), Style::default().fg(VIOLET_LIGHT)),
        Span::styled("◆", Style::default().fg(GOLD)),
    ];
    spans.push(Span::styled(
        "─".repeat(rail_w.saturating_sub(marker + 1)),
        Style::default().fg(SLATE_700),
    ));
    let percent = (frac * 100.0).round() as u8;
    spans.push(Span::styled(
        format!(" {percent:>3}%"),
        Style::default().fg(SLATE_400),
    ));
    Line::from(spans)
}

fn power_spans(p: &PowerStatus) -> Vec<Span<'static>> {
    if p.on_ac {
        let label = match p.percent {
            Some(pc) => format!("↯ AC POWER · {pc}%"),
            None => "↯ AC POWER".into(),
        };
        vec![Span::styled(label, Style::default().fg(EMERALD))]
    } else {
        let (label, color) = match p.percent {
            Some(pc) if pc < 20 => (format!("◐ BATTERY · {pc}%"), RED),
            Some(pc) => (format!("◐ BATTERY · {pc}%"), AMBER),
            None => ("◐ BATTERY".into(), AMBER),
        };
        vec![Span::styled(label, Style::default().fg(color))]
    }
}

struct Title {
    tick: u64,
}

impl Widget for Title {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let x0 = area.x + area.width.saturating_sub(art::TITLE_W) / 2;
        let _ = self.tick;

        // Give the compact wordmark quiet negative space when a comet crosses
        // the title while leaving the surrounding sky alive.
        for y in area.y..area.bottom() {
            for x in
                (x0 + art::WORDMARK_X)..(x0 + art::WORDMARK_X + art::WORDMARK_W).min(area.right())
            {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_fg(ratatui::style::Color::Reset);
                    cell.set_bg(BACKGROUND);
                }
            }
        }

        for (dy, line) in art::TITLE.iter().enumerate() {
            let y = area.y + dy as u16;
            for (dx, ch) in line.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let x = x0 + dx as u16;
                let color = VIOLET_LIGHT;
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_fg(color);
                    // Clear any half-block background a trail passing behind
                    // the title left in this cell.
                    cell.set_bg(BACKGROUND);
                }
            }
        }
    }
}

pub fn fmt_hms(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
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

    fn state(cards: Vec<SideCard>, merged: bool) -> ViewState {
        ViewState {
            tick: 7,
            elapsed: Duration::from_secs(2537),
            total: Some(Duration::from_secs(7200)),
            remaining: Some(Duration::from_secs(4663)),
            kinds: vec![Kind::Display, Kind::Idle],
            power: PowerStatus {
                on_ac: true,
                percent: Some(100),
            },
            cards,
            merged,
            sky: vec![true; sky::FLYERS.len()],
            motion: true,
            navigation: Navigation::default(),
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
                    section: true,
                    ..CardRow::default()
                },
                CardRow {
                    health: Some(Health::Good),
                    name: "▸ cxkw.dev".into(),
                    detail: Some("42% used".into()),
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
            summary: "3 items".into(),
            left,
        }
    }

    #[test]
    fn draw_survives_any_terminal_size_and_layout() {
        // Every layout branch — compact, stacked, wide, three-column,
        // merged — across sizes from degenerate to spacious.
        for (w, h) in [(1, 1), (40, 8), (65, 30), (66, 18), (90, 24), (150, 45)] {
            for (cards, merged) in [
                (vec![], false),
                (vec![meter_card(false)], false),
                (vec![meter_card(true), meter_card(false)], false),
                (vec![meter_card(false)], true),
            ] {
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                terminal.draw(|f| draw(f, &state(cards, merged))).unwrap();
                assert!(
                    terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .all(|cell| { !cell.symbol().chars().any(char::is_lowercase) }),
                    "lowercase text in {w}x{h}, merged={merged}"
                );
            }
        }
    }

    #[test]
    fn night_watch_rows_respect_the_card_width() {
        let bounded = state(vec![], false);
        let mut unbounded = state(vec![], false);
        unbounded.total = None;
        unbounded.remaining = None;
        for box_width in [MAIN_W, MAIN_MAX_W, 100] {
            let inner = box_width as usize - 2;
            for s in [&bounded, &unbounded] {
                for line in status_lines(s, &[], box_width) {
                    let width: usize = line
                        .spans
                        .iter()
                        .map(|span| span.content.chars().count())
                        .sum();
                    assert!(
                        width <= inner,
                        "status row is {width} cells in {inner}: {line:?}"
                    );
                }
            }
            let lines = status_lines(&bounded, &[], box_width);
            let progress: String = lines[4]
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            assert!(progress.ends_with("  35%"), "{progress}");
        }
    }
}
