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
use crate::power::{Kind, PowerStatus};
use crate::sky;
use crate::theme::{
    gradient, gradient_rgb, pulse, scale, tri, AMBER, EMERALD, GOLD, RED, SLATE_400, SLATE_500,
    SLATE_600, SLATE_700, TEXT, VIOLET_LIGHT,
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
            cards: cards::build_cards(&fc.cards, &docker, &card_data, &fc.dashboard),
            merged: fc.dashboard.combined,
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
    let bw = match merged_card {
        Some(_) => area
            .width
            .saturating_sub(4)
            .clamp(MAIN_W, MAIN_MAX_W)
            .min(area.width.saturating_sub(2)),
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
        // As many rows as the height allows, then a "+N more" line.
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
    sky::render(&ctx, &s.sky, buf);

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
        cards::render_side_card(&s.cards[*i], *shown, *rect, s.tick, buf);
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
}

/// The single-line layout for terminals too small for the full art: the sky
/// still twinkles behind a centered wordmark and status line.
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

    let ctx = sky::Ctx { tick: s.tick, area };
    let buf = f.buffer_mut();
    sky::render(&ctx, &s.sky, buf);
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
            let width = display_width.saturating_sub(16) as usize;
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
        extra.push(row(&cards::truncate(&card.title, 9), spans));
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
                    // Clear any half-block background a trail passing behind
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collect::CardRow;
    use ratatui::{backend::TestBackend, Terminal};

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
            }
        }
    }
}
