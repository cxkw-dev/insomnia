//! Dashboard cards: assembling render-ready cards from the collected data
//! and drawing them — accent-tinted boxes of health rows, section headers,
//! and utilization meters.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Widget},
};

use crate::collect::{CardData, CardRow, Container, Health};
use crate::config::{accent_rgb, CardConfig, DashboardConfig};
use crate::theme::{
    pulse, scale, HEALTH_BAD, HEALTH_GOOD, HEALTH_WARN, RED, SLATE_400, SLATE_500, SLATE_600,
    SLATE_700, TEXT,
};

/// Widest the name column may grow when a row has a detail column; cards
/// with shorter names shrink the column so details get the leftover room.
const NAME_W: usize = 21;
/// Widest a meter row's label column may grow.
const METER_LABEL_W: usize = 15;
/// A meter's gauge never grows past the max, and detail text may not
/// squeeze it below the min.
const GAUGE_MAX_W: usize = 14;
const GAUGE_MIN_W: usize = 9;
/// Cells a meter row spends around its label and gauge: the indent, the
/// branch rail, the label gap, and the " nnn%" value.
const METER_CHROME: usize = 4 + 2 + 1 + 5;

const DEFAULT_GLYPH: &str = "◆";
const DEFAULT_ACCENT: (u8, u8, u8) = (167, 139, 250);
const DOCKER_ACCENT: (u8, u8, u8) = (59, 130, 246);

/// A dashboard card ready to render.
pub struct SideCard {
    pub title: String,
    pub glyph: String,
    pub accent: (u8, u8, u8),
    pub rows: Vec<CardRow>,
    /// One-line stand-in shown inside the status card when this card
    /// doesn't fit the terminal.
    pub summary: String,
    /// Floats left of the status card when the terminal fits three columns;
    /// otherwise the card flows with the right-hand stack.
    pub left: bool,
}

/// Assemble the render-ready card list: custom cards in config order, docker
/// last. Cards with no data (source down) or no rows simply don't exist.
/// With `[dashboard] combined = true` the survivors fold into one card, each
/// as a section under its own glyph-and-accent header; the UI then renders
/// that card's rows inside the status box, so everything is one box.
pub fn build_cards(
    configs: &[CardConfig],
    docker: &Option<Vec<Container>>,
    data: &[Option<CardData>],
    dash: &DashboardConfig,
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
            rows: space_sections(&d.rows),
            summary: format!("{} items", d.rows.len()),
            left: cfg.side.as_deref() == Some("left"),
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
                        ..CardRow::default()
                    })
                    .collect(),
                summary: format!("{} running", containers.len()),
                left: false,
            });
        }
    }
    if dash.combined && !out.is_empty() {
        let mut rows = Vec::new();
        for (k, c) in out.iter().enumerate() {
            if k > 0 {
                rows.push(CardRow::default());
            }
            rows.push(CardRow {
                name: format!("{} {}", c.glyph, c.title),
                accent: Some(c.accent),
                ..CardRow::default()
            });
            rows.extend(c.rows.iter().cloned());
        }
        let summary = format!("{} sections", out.len());
        out = vec![SideCard {
            title: dash.title.clone().unwrap_or_else(|| "dashboard".into()),
            glyph: dash.glyph.clone().unwrap_or_else(|| DEFAULT_GLYPH.into()),
            accent: dash
                .accent
                .as_deref()
                .and_then(accent_rgb)
                .unwrap_or(DEFAULT_ACCENT),
            rows,
            summary,
            left: dash.side.as_deref() == Some("left"),
        }];
    }
    out
}

/// A blank line above every section header but the first, so a card built
/// from several sources — the ai usage card's one section per service —
/// reads as separate blocks instead of one dense list. The spacing lands in
/// the row list itself rather than at render time, because every height
/// budget here is counted in rows: a line conjured during drawing would be
/// one the layout never reserved, and the card would quietly lose its last
/// row to make room.
fn space_sections(rows: &[CardRow]) -> Vec<CardRow> {
    let mut out: Vec<CardRow> = Vec::with_capacity(rows.len() + 4);
    for r in rows {
        if r.section && !out.is_empty() && !out.last().is_some_and(is_blank) {
            out.push(CardRow::default());
        }
        out.push(r.clone());
    }
    out
}

/// A spacer row: no dot, no text, no meter — just vertical room.
fn is_blank(r: &CardRow) -> bool {
    r.health.is_none() && r.percent.is_none() && !r.section && r.name.trim().is_empty()
}

/// The width [`render_side_card`] needs to show every row of `card` whole:
/// borders and indent, the health dot, the shared name column, and the
/// longest detail — or the title line, whichever is wider.
///
/// Both column widths are the shared ones the renderer will use, not each
/// row's own: a meter with a short label still sits behind the widest
/// label's gauge, so measuring it against its own label would ask for a
/// card too narrow to hold that row's detail.
pub fn natural_width(card: &SideCard) -> u16 {
    let name_w = card
        .rows
        .iter()
        .filter(|r| r.detail.is_some() && r.percent.is_none())
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(0)
        .min(NAME_W);
    let label_w = card
        .rows
        .iter()
        .filter(|r| r.percent.is_some())
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(0)
        .min(METER_LABEL_W);
    let body = card
        .rows
        .iter()
        .map(|r| {
            if r.percent.is_some() {
                let detail = r.detail.as_deref().map_or(0, |d| d.chars().count() + 2);
                return METER_CHROME + label_w + GAUGE_MAX_W + detail;
            }
            let dot = if r.health.is_some() { 2 } else { 0 };
            match &r.detail {
                Some(d) => dot + name_w + 1 + d.chars().count(),
                None => dot + r.name.chars().count(),
            }
        })
        .max()
        .unwrap_or(0);
    let title = card.glyph.chars().count() + card.title.chars().count() + 8;
    (body + 4).max(title) as u16
}

/// One dashboard card: an accent-tinted box titled with a glyph and count,
/// listing rows with optional health dots and detail columns.
pub fn render_side_card(card: &SideCard, shown: usize, rect: Rect, tick: u64, buf: &mut Buffer) {
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
            // Spacer rows are layout, not content — they don't count.
            format!(
                "{} · {}",
                card.title,
                card.rows.iter().filter(|r| !is_blank(r)).count()
            ),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
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
        .title_alignment(Alignment::Left);
    let inner = block.inner(rect);
    block.render(rect, buf);

    let mut lines = vec![Line::default()];
    lines.extend(card_row_lines(
        &card.rows,
        shown,
        inner.width as usize,
        tick,
    ));
    Paragraph::new(lines).render(inner, buf);
}

/// The body lines of a card: health dots, aligned name/detail columns,
/// accented headers, meters, and a trailing "+N more" when rows are cut.
/// `inner_w` is the width of the box the lines will live in, minus borders.
pub fn card_row_lines(
    rows: &[CardRow],
    shown: usize,
    inner_w: usize,
    tick: u64,
) -> Vec<Line<'static>> {
    let name_w = rows
        .iter()
        .take(shown)
        .filter(|r| r.detail.is_some() && r.percent.is_none())
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(NAME_W)
        .min(NAME_W);
    let label_w = rows
        .iter()
        .take(shown)
        .filter(|r| r.percent.is_some())
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(0)
        .min(METER_LABEL_W);
    let mut lines = Vec::new();
    for (i, r) in rows.iter().take(shown).enumerate() {
        lines.push(if r.section {
            section_line(r, inner_w)
        } else if let Some(percent) = r.percent {
            let continues = rows.get(i + 1).is_some_and(|next| next.percent.is_some());
            meter_line(r, percent, continues, label_w, inner_w)
        } else {
            entry_line(r, i, name_w, inner_w, tick)
        });
    }
    if rows.len() > shown {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                format!("+{} more", rows.len() - shown),
                Style::default().fg(SLATE_600),
            ),
        ]));
    }
    lines
}

/// A section header: `─ title ────────` in the section's accent.
fn section_line(r: &CardRow, inner_w: usize) -> Line<'static> {
    let accent = r.accent.unwrap_or(DEFAULT_ACCENT);
    let rule_w = inner_w.saturating_sub(r.name.chars().count() + 6);
    Line::from(vec![
        Span::raw("  "),
        Span::styled("─ ", Style::default().fg(scale(accent, 0.55))),
        Span::styled(
            r.name.clone(),
            Style::default()
                .fg(scale(accent, 1.0))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", "─".repeat(rule_w)),
            Style::default().fg(scale(accent, 0.35)),
        ),
    ])
}

/// A utilization meter: `╰ label ███▍░░░ 42%  detail`, joined to the row
/// above by a branch rail, the rail forking when meters stack. The value
/// turns amber past 75 and red past 90; the gauge keeps the accent.
fn meter_line(
    r: &CardRow,
    percent: u8,
    continues: bool,
    label_w: usize,
    inner_w: usize,
) -> Line<'static> {
    let accent = r.accent.unwrap_or(DEFAULT_ACCENT);
    let bar_color = scale(accent, 1.0);
    let value_color = match percent {
        90..=100 => scale(HEALTH_BAD, 1.0),
        75..=89 => scale(HEALTH_WARN, 1.0),
        _ => bar_color,
    };
    let detail = r.detail.as_deref().unwrap_or_default();
    let branch = if continues { "├ " } else { "╰ " };
    // The detail takes whatever the row has left once the gauge keeps its
    // minimum — no fixed ceiling, or a wide terminal would still clip a
    // reset time the card has ample room for.
    let available = inner_w.saturating_sub(METER_CHROME + label_w);
    let detail_w = detail
        .chars()
        .count()
        .min(available.saturating_sub(GAUGE_MIN_W + 2));
    let gauge_w = available
        .saturating_sub(usize::from(detail_w > 0) * 2 + detail_w)
        .min(GAUGE_MAX_W);
    let (filled, partial, empty) = gauge_parts(percent, gauge_w);
    let mut spans = vec![
        Span::raw("    "),
        Span::styled(branch, Style::default().fg(scale(accent, 0.5))),
        Span::styled(
            format!("{:<label_w$} ", truncate(&r.name, label_w)),
            Style::default().fg(SLATE_400),
        ),
        Span::styled("█".repeat(filled), Style::default().fg(bar_color)),
    ];
    if let Some(ch) = partial {
        spans.push(Span::styled(ch.to_string(), Style::default().fg(bar_color)));
    }
    spans.extend([
        Span::styled("░".repeat(empty), Style::default().fg(SLATE_700)),
        Span::styled(
            format!(" {percent:>3}%"),
            Style::default()
                .fg(value_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    if detail_w > 0 {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            truncate(detail, detail_w),
            Style::default().fg(SLATE_500),
        ));
    }
    Line::from(spans)
}

/// An ordinary row: an optional pulsing health dot, then either aligned
/// name/detail columns (a `▸` name marks the active account and renders
/// bright) or a single run of text in the row's accent.
fn entry_line(r: &CardRow, i: usize, name_w: usize, inner_w: usize, tick: u64) -> Line<'static> {
    let mut spans = vec![Span::raw("  ")];
    let mut used = 2usize;
    if let Some(h) = r.health {
        let fg = match h {
            Health::Bad => pulse(HEALTH_BAD, tick + i as u64 * 8),
            Health::Warn => pulse(HEALTH_WARN, tick + i as u64 * 8),
            Health::Good => pulse(HEALTH_GOOD, tick + i as u64 * 8),
            Health::Off => SLATE_600,
        };
        spans.push(Span::styled("● ", Style::default().fg(fg)));
        used += 2;
    }
    match &r.detail {
        Some(detail) => {
            let active = r.name.trim_start().starts_with('▸');
            spans.push(Span::styled(
                format!("{:<name_w$} ", truncate(&r.name, name_w)),
                Style::default()
                    .fg(if active { TEXT } else { SLATE_400 })
                    .add_modifier(if active {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ));
            let dw = inner_w.saturating_sub(used + name_w + 1);
            spans.push(Span::styled(
                truncate(detail, dw),
                Style::default().fg(SLATE_500),
            ));
        }
        None => {
            let w = inner_w.saturating_sub(used);
            let fg = r.accent.map_or(SLATE_400, |rgb| scale(rgb, 1.0));
            spans.push(Span::styled(truncate(&r.name, w), Style::default().fg(fg)));
        }
    }
    Line::from(spans)
}

/// A compact gauge with eighth-cell precision. Full blocks carry the accent,
/// a partial block preserves small percentages, and shade cells show the
/// unused portion without letting a meter dominate the whole row.
fn gauge_parts(percent: u8, width: usize) -> (usize, Option<char>, usize) {
    if width == 0 {
        return (0, None, 0);
    }
    let eighths = (usize::from(percent) * width * 8 + 50) / 100;
    let full = (eighths / 8).min(width);
    let remainder = eighths % 8;
    let partial = if full < width {
        match remainder {
            0 => None,
            1 => Some('▏'),
            2 => Some('▎'),
            3 => Some('▍'),
            4 => Some('▌'),
            5 => Some('▋'),
            6 => Some('▊'),
            _ => Some('▉'),
        }
    } else {
        None
    };
    let empty = width.saturating_sub(full + usize::from(partial.is_some()));
    (full, partial, empty)
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauge_keeps_small_values_visible_and_width_stable() {
        assert_eq!(gauge_parts(0, 10), (0, None, 10));
        assert_eq!(gauge_parts(9, 10), (0, Some('▉'), 9));
        assert_eq!(gauge_parts(50, 10), (5, None, 5));
        assert_eq!(gauge_parts(100, 10), (10, None, 0));
    }

    fn row(name: &str, section: bool) -> CardRow {
        CardRow {
            health: (!section && !name.is_empty()).then_some(Health::Good),
            name: name.into(),
            section,
            ..CardRow::default()
        }
    }

    #[test]
    fn sections_get_breathing_room_but_never_a_leading_blank() {
        let rows = vec![
            row("✻ claude", true),
            row("cxkw.dev", false),
            row("◎ openai codex", true),
            row("cxkw.dev", false),
        ];
        let spaced = space_sections(&rows);
        let shape: Vec<&str> = spaced
            .iter()
            .map(|r| if is_blank(r) { "_" } else { "x" })
            .collect();
        assert_eq!(shape, ["x", "x", "_", "x", "x"]);
    }

    #[test]
    fn existing_blanks_are_left_alone() {
        // The combined dashboard already separates cards with a blank row;
        // a section header right after one must not stack a second.
        let rows = vec![row("web", false), row("", false), row("≋ docker", true)];
        assert_eq!(space_sections(&rows).len(), rows.len());
    }

    fn text(line: &Line<'static>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn natural_width_holds_every_row_whole() {
        // The ai usage card's shape: the longest reset time rides a *short*
        // label, so it sits behind the widest label's gauge — measuring that
        // row against its own label would ask for a card too narrow for it.
        let meter = |name: &str, detail: &str| CardRow {
            name: name.into(),
            detail: Some(detail.into()),
            percent: Some(42),
            ..CardRow::default()
        };
        let card = SideCard {
            title: "ai usage".into(),
            glyph: "✦".into(),
            accent: DEFAULT_ACCENT,
            rows: vec![
                CardRow {
                    health: Some(Health::Bad),
                    name: "andy.nguyen".into(),
                    detail: Some("business · 0 credits left · limit reached · stale 2d".into()),
                    ..CardRow::default()
                },
                meter("5h window", "resets Mon 00:00 (in 4h)"),
                meter("monthly credits", "resets Mon 19:00 (in 21d)"),
            ],
            summary: "3 items".into(),
            left: false,
        };
        let inner = natural_width(&card) as usize - 2;
        for line in card_row_lines(&card.rows, card.rows.len(), inner, 7) {
            let rendered = text(&line);
            assert!(
                !rendered.contains('…'),
                "clipped at natural width: {rendered}"
            );
            assert!(rendered.chars().count() <= inner, "overflows: {rendered}");
        }
    }

    #[test]
    fn a_meter_detail_grows_into_the_room_the_card_has() {
        // No fixed ceiling on the detail column: given the width, the whole
        // reset time shows.
        let rows = vec![CardRow {
            name: "weekly".into(),
            detail: Some("resets Mon 19:00 (in 21d)".into()),
            percent: Some(50),
            ..CardRow::default()
        }];
        let line = text(&card_row_lines(&rows, 1, 80, 7)[0]);
        assert!(line.ends_with("resets Mon 19:00 (in 21d)"), "{line}");
    }

    #[test]
    fn every_row_kind_renders_at_any_width() {
        // Sections, meters with and without details, health rows, plain and
        // blank rows — none may panic however narrow the card gets.
        let rows = vec![
            row("✻ claude", true),
            CardRow {
                health: Some(Health::Good),
                name: "▸ cxkw.dev".into(),
                detail: Some("42% used".into()),
                ..CardRow::default()
            },
            CardRow {
                name: "weekly".into(),
                detail: Some("resets Sun 03:49".into()),
                accent: Some((16, 163, 127)),
                percent: Some(96),
                ..CardRow::default()
            },
            CardRow {
                name: "5h window".into(),
                percent: Some(9),
                ..CardRow::default()
            },
            CardRow::default(),
            row("plain text", false),
        ];
        for w in 0..60 {
            let lines = card_row_lines(&rows, rows.len(), w, 7);
            assert_eq!(lines.len(), rows.len());
        }
    }
}
