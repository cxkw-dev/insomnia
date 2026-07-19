//! The TUI: terminal setup, the render loop, and the card layout — a status
//! card plus any dashboard cards (docker and custom command cards), floating
//! over the sky rendered by [`crate::sky`].

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
use crate::collect::{self, CardData, CardRow, Container, Health, Update};
use crate::config::{accent_rgb, CardConfig, Config, FileConfig};
use crate::power::{Kind, PowerStatus};
use crate::sky;
use crate::theme::{
    gradient, gradient_rgb, pulse, scale, tri, AMBER, EMERALD, GOLD, HEALTH_BAD, HEALTH_GOOD,
    HEALTH_WARN, RED, SLATE_400, SLATE_500, SLATE_600, SLATE_700, TEXT, VIOLET_LIGHT,
};

const MAIN_W: u16 = 46;
const SIDE_W: u16 = 40;
const GAP: u16 = 2;
/// Column width for row names when a row has a detail column.
const NAME_W: usize = 21;
const DEFAULT_GLYPH: &str = "◆";
const DEFAULT_ACCENT: (u8, u8, u8) = (167, 139, 250);
const DOCKER_ACCENT: (u8, u8, u8) = (59, 130, 246);

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
    /// Per-flyer sky toggles, aligned with [`sky::FLYERS`].
    pub sky: Vec<bool>,
}

/// A dashboard card ready to render.
pub struct SideCard {
    pub title: String,
    pub glyph: String,
    pub accent: (u8, u8, u8),
    pub rows: Vec<CardRow>,
    /// One-line stand-in shown inside the status card when this card
    /// doesn't fit the terminal.
    pub summary: String,
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
        let state = ViewState {
            tick,
            elapsed: start.elapsed(),
            total: cfg.timeout,
            remaining: deadline.map(|d| d.saturating_duration_since(now)),
            kinds: kinds.to_vec(),
            power: power.clone(),
            cards: build_cards(&fc.cards, &docker, &card_data),
            sky: sky_on.clone(),
        };
        if let Err(e) = terminal.draw(|f| draw(f, &state)) {
            break Err(e);
        }
        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let quit = matches!(
                        key.code,
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc
                    ) || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL));
                    if quit {
                        break Ok(());
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

/// Assemble the render-ready card list: custom cards in config order, docker
/// last. Cards with no data (source down) or no rows simply don't exist.
fn build_cards(
    configs: &[CardConfig],
    docker: &Option<Vec<Container>>,
    data: &[Option<CardData>],
) -> Vec<SideCard> {
    let mut out = Vec::new();
    for (cfg, latest) in configs.iter().zip(data) {
        let Some(d) = latest else { continue };
        if d.rows.is_empty() {
            continue;
        }
        out.push(SideCard {
            title: cfg.title.clone(),
            glyph: cfg.glyph.clone().unwrap_or_else(|| DEFAULT_GLYPH.into()),
            accent: cfg
                .accent
                .as_deref()
                .and_then(accent_rgb)
                .unwrap_or(DEFAULT_ACCENT),
            rows: d.rows.clone(),
            summary: format!("{} items", d.rows.len()),
        });
    }
    if let Some(containers) = docker {
        if !containers.is_empty() {
            out.push(SideCard {
                title: "docker".into(),
                glyph: "≋".into(),
                accent: DOCKER_ACCENT,
                rows: containers
                    .iter()
                    .map(|c| CardRow {
                        health: Some(c.health),
                        name: c.name.clone(),
                        detail: Some(c.status.clone()),
                    })
                    .collect(),
                summary: format!("{} running", containers.len()),
            });
        }
    }
    out
}

pub fn draw(f: &mut Frame, s: &ViewState) {
    let area = f.area();

    if area.width < 66 || area.height < 18 {
        draw_compact(f, s);
        return;
    }

    // The side cards sail beside the status card on a wide terminal and dock
    // beneath it on a narrow one. Cards are placed in order, each getting as
    // many rows as the height allows; a card with no headroom left shrinks
    // to a summary row inside the status card.
    let has_side = !s.cards.is_empty();
    let wide = has_side && area.width >= MAIN_W + GAP + SIDE_W + 2;
    let main_base = status_lines(s, &[]).len() as u16 + 3;

    // (card index, card height, rows shown) for every card that fits.
    // Folding a card adds a summary row to the status card, which in the
    // stacked layout shrinks the very budget being divided — so placement
    // reruns until the folded set stops changing.
    let mut placed: Vec<(usize, u16, usize)> = Vec::new();
    let mut folded: Vec<usize> = Vec::new();
    loop {
        let growth = if folded.is_empty() {
            0
        } else {
            folded.len() as u16 + 1
        };
        let mut avail = area
            .height
            .saturating_sub(art::TITLE_H + 1 + 2 + 2)
            .saturating_sub(if wide { 0 } else { main_base + growth + 1 });
        placed.clear();
        let mut refold: Vec<usize> = Vec::new();
        for (i, card) in s.cards.iter().enumerate() {
            let n = card.rows.len();
            let gap = u16::from(!placed.is_empty());
            let budget = (avail.saturating_sub(gap) as usize).saturating_sub(4);
            if folded.contains(&i) || budget < 2 {
                refold.push(i);
                continue;
            }
            let shown = if n <= budget { n } else { budget - 1 };
            let h = (shown + usize::from(n > shown) + 4) as u16;
            avail = avail.saturating_sub(h + gap);
            placed.push((i, h, shown));
        }
        if refold == folded {
            break;
        }
        folded = refold;
    }

    let lines = status_lines(s, &folded);
    let main_h = lines.len() as u16 + 3;
    let side_h: u16 =
        placed.iter().map(|(_, h, _)| h).sum::<u16>() + placed.len().saturating_sub(1) as u16;
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
    let bw = MAIN_W.min(area.width.saturating_sub(2));
    let (main_x, side_x, side_w, side_y0) = if wide && side_h > 0 {
        let gx = area.x + area.width.saturating_sub(MAIN_W + GAP + SIDE_W) / 2;
        (gx, gx + MAIN_W + GAP, SIDE_W, cards_y)
    } else {
        let x = area.x + (area.width - bw) / 2;
        (x, x, bw, cards_y + main_h + 1)
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
    let mut sy = side_y0;
    for &(i, h, shown) in &placed {
        side_rects.push((
            i,
            Rect {
                x: side_x,
                y: sy,
                width: side_w,
                height: h,
            }
            .intersection(area),
            shown,
        ));
        sy += h + 1;
    }
    let hint_y = side_rects
        .last()
        .map_or(brect.bottom(), |(_, r, _)| r.bottom().max(brect.bottom()))
        + 1;

    let ctx = sky::Ctx {
        tick: s.tick,
        area,
        card_band: (cards_y + 2) as i32,
        low_lane: (hint_y + 1) as i32,
    };
    let buf = f.buffer_mut();
    sky::render_behind(&ctx, &s.sky, buf);

    Title { tick: s.tick }.render(title_rect, buf);

    Clear.render(brect, buf);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(scale((139, 92, 246), 0.8)))
        .title(" ☾ ")
        .title_style(Style::default().fg(GOLD))
        .title_alignment(Alignment::Center);
    let inner = block.inner(brect);
    block.render(brect, buf);
    Paragraph::new(lines).render(inner, buf);

    for (i, rect, shown) in &side_rects {
        render_side_card(&s.cards[*i], *shown, *rect, s.tick, buf);
    }

    if hint_y < area.bottom() {
        let hint = Rect {
            x: area.x,
            y: hint_y,
            width: area.width,
            height: 1,
        };
        Paragraph::new(Line::from(Span::styled(
            "q — let it sleep",
            Style::default()
                .fg(SLATE_600)
                .add_modifier(Modifier::ITALIC),
        )))
        .alignment(Alignment::Center)
        .render(hint, buf);
    }

    sky::render_front(&ctx, &s.sky, buf);
}

fn draw_compact(f: &mut Frame, s: &ViewState) {
    let area = f.area();
    let mid = area.y + area.height / 2;
    let name = "☾  I N S O M N I A";
    let n = name.chars().count();
    let spans: Vec<Span> = name
        .chars()
        .enumerate()
        .map(|(i, c)| {
            Span::styled(
                c.to_string(),
                Style::default()
                    .fg(gradient(i as f32 / (n - 1) as f32))
                    .add_modifier(Modifier::BOLD),
            )
        })
        .collect();
    let mut status = format!("awake {}", fmt_hms(s.elapsed));
    if let Some(r) = s.remaining {
        status.push_str(&format!(" · {} left", fmt_hms(r)));
    }
    status.push_str(" · q to sleep");

    let ctx = sky::Ctx {
        tick: s.tick,
        area,
        card_band: area.y as i32 + 1,
        low_lane: area.y as i32 + 1,
    };
    let buf = f.buffer_mut();
    sky::draw_one("ufo", &ctx, &s.sky, buf);
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
    sky::draw_one("rocket", &ctx, &s.sky, buf);
}

/// The status card body. `folded` lists cards that didn't fit as cards and
/// appear here as one-line summaries instead.
fn status_lines(s: &ViewState, folded: &[usize]) -> Vec<Line<'static>> {
    let mut lines = vec![Line::default(), badges(s), Line::default()];
    lines.push(row(
        "awake",
        vec![Span::styled(
            fmt_hms(s.elapsed),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )],
    ));
    match s.remaining {
        Some(r) => {
            lines.push(row(
                "until",
                vec![Span::styled(
                    fmt_hms(r),
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                )],
            ));
            let total = s.total.unwrap_or(r).as_secs_f32().max(1.0);
            let frac = (r.as_secs_f32() / total).clamp(0.0, 1.0);
            let width = 30usize;
            let filled = (frac * width as f32).round() as usize;
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled("▰".repeat(filled), Style::default().fg(VIOLET_LIGHT)),
                Span::styled("▱".repeat(width - filled), Style::default().fg(SLATE_700)),
            ]));
        }
        None => lines.push(row(
            "until",
            vec![Span::styled(
                "forever",
                Style::default()
                    .fg(SLATE_400)
                    .add_modifier(Modifier::ITALIC),
            )],
        )),
    }
    lines.push(row("power", power_spans(&s.power)));

    let mut extra = Vec::new();
    for &i in folded {
        let card = &s.cards[i];
        let bad = card
            .rows
            .iter()
            .filter(|r| r.health == Some(Health::Bad))
            .count();
        let mut spans = vec![Span::styled(
            card.summary.clone(),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )];
        if bad > 0 {
            spans.push(Span::styled(
                format!(" · {bad} bad"),
                Style::default().fg(RED),
            ));
        }
        extra.push(row(&truncate(&card.title, 9), spans));
    }
    if !extra.is_empty() {
        lines.push(Line::default());
        lines.extend(extra);
    }
    lines
}

fn row(label: &str, value: Vec<Span<'static>>) -> Line<'static> {
    let mut spans = vec![
        Span::raw("   "),
        Span::styled(format!("{label:<9}"), Style::default().fg(SLATE_500)),
    ];
    spans.extend(value);
    Line::from(spans)
}

fn badges(s: &ViewState) -> Line<'static> {
    let mut spans = vec![Span::raw("   ")];
    for (i, kind) in s.kinds.iter().enumerate() {
        let rgb = match kind {
            Kind::Display => (56, 189, 248),
            Kind::Idle => (52, 211, 153),
            Kind::System => (251, 191, 36),
            Kind::Disk => (192, 132, 252),
        };
        spans.push(Span::styled(
            "●",
            Style::default().fg(pulse(rgb, s.tick + i as u64 * 8)),
        ));
        spans.push(Span::styled(
            format!(" {}", kind.label()),
            Style::default().fg(SLATE_400),
        ));
        spans.push(Span::raw("  "));
    }
    Line::from(spans)
}

/// One dashboard card: an accent-tinted box titled with a glyph and count,
/// listing rows with optional health dots and detail columns.
fn render_side_card(card: &SideCard, shown: usize, rect: Rect, tick: u64, buf: &mut Buffer) {
    Clear.render(rect, buf);
    let bad = card
        .rows
        .iter()
        .filter(|r| r.health == Some(Health::Bad))
        .count();
    let mut title = vec![
        Span::styled(
            format!(" {} ", card.glyph),
            Style::default().fg(scale(card.accent, 1.0)),
        ),
        Span::styled(
            format!("{} · {}", card.title, card.rows.len()),
            Style::default().fg(SLATE_400),
        ),
    ];
    if bad > 0 {
        title.push(Span::styled(
            format!(" · {bad} bad"),
            Style::default().fg(RED),
        ));
    }
    title.push(Span::raw(" "));
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(scale(card.accent, 0.6)))
        .title(Line::from(title))
        .title_alignment(Alignment::Center);
    let inner = block.inner(rect);
    block.render(rect, buf);

    let mut lines = vec![Line::default()];
    for (i, r) in card.rows.iter().take(shown).enumerate() {
        let mut spans = vec![Span::raw("  ")];
        let mut used = 2usize;
        if let Some(h) = r.health {
            let rgb = match h {
                Health::Bad => HEALTH_BAD,
                Health::Warn => HEALTH_WARN,
                Health::Good => HEALTH_GOOD,
            };
            spans.push(Span::styled(
                "● ",
                Style::default().fg(pulse(rgb, tick + i as u64 * 8)),
            ));
            used += 2;
        }
        match &r.detail {
            Some(detail) => {
                spans.push(Span::styled(
                    format!("{:<NAME_W$} ", truncate(&r.name, NAME_W)),
                    Style::default().fg(SLATE_400),
                ));
                let dw = (inner.width as usize).saturating_sub(used + NAME_W + 1);
                spans.push(Span::styled(
                    truncate(detail, dw),
                    Style::default().fg(SLATE_500),
                ));
            }
            None => {
                let w = (inner.width as usize).saturating_sub(used);
                spans.push(Span::styled(
                    truncate(&r.name, w),
                    Style::default().fg(SLATE_400),
                ));
            }
        }
        lines.push(Line::from(spans));
    }
    if card.rows.len() > shown {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                format!("+{} more", card.rows.len() - shown),
                Style::default().fg(SLATE_600),
            ),
        ]));
    }
    Paragraph::new(lines).render(inner, buf);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn power_spans(p: &PowerStatus) -> Vec<Span<'static>> {
    if p.on_ac {
        let label = match p.percent {
            Some(pc) => format!("ac power · {pc}%"),
            None => "ac power".into(),
        };
        vec![Span::styled(label, Style::default().fg(EMERALD))]
    } else {
        let (label, color) = match p.percent {
            Some(pc) if pc < 20 => (format!("battery · {pc}%"), RED),
            Some(pc) => (format!("battery · {pc}%"), AMBER),
            None => ("battery".into(), AMBER),
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
        let shift = self.tick as f32 * 0.006;
        for (dy, line) in art::TITLE.iter().enumerate() {
            let y = area.y + dy as u16;
            for (dx, ch) in line.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let x = x0 + dx as u16;
                let t = tri(dx as f32 / (art::TITLE_W - 1) as f32 + shift);
                let color = if ch == '█' {
                    gradient(t)
                } else {
                    scale(gradient_rgb(t), 0.45)
                };
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_fg(color);
                    // Clear any half-block background a sprite passing behind
                    // the title left in this cell.
                    cell.set_bg(ratatui::style::Color::Reset);
                }
            }
        }
    }
}

pub fn fmt_hms(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}
