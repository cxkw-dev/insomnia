//! Dashboard cards: assembling render-ready cards from the collected data,
//! and drawing the rows they share — health rows, group headers, meters —
//! plus the separate-box layout's card frames.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Widget},
};

use crate::collect::{severity, CardRow, Container, ContainerState, Health};
use crate::config::{CardConfig, Side};
use crate::text::{pad, truncate, width};
use crate::theme::{
    scale, Rgb, AMBER, AMBER_RGB, BLUE_RGB, BORDER, EMERALD_RGB, MUTED, RED, SLATE_400, SLATE_500,
    SLATE_600, SLATE_700, TEXT, VIOLET_RGB,
};

/// Widest the name column may grow when a row has a detail column; cards
/// with shorter names shrink the column so details get the leftover room.
const NAME_W: usize = 21;
/// A meter row, left to right: indent, branch rail, label, a gap, then the
/// gauge (at most [`GAUGE_MAX_W`]) and its " nnn%" value.
pub(crate) const METER_INDENT: usize = 4;
const BRANCH_W: usize = 2;
pub(crate) const METER_LABEL_W: usize = 15;
pub(crate) const GAUGE_MAX_W: usize = 24;
pub(crate) const PERCENT_W: usize = 5;
/// The cell where every gauge starts, so values line up across rows.
pub(crate) const GAUGE_COLUMN: usize = METER_INDENT + BRANCH_W + METER_LABEL_W + 1;
/// Cells a meter row spends around its label and gauge.
const METER_CHROME: usize = METER_INDENT + BRANCH_W + 1 + PERCENT_W;

const DEFAULT_GLYPH: &str = "◆";

/// The latest a card's command produced: its last good rows, and why the
/// most recent run failed, if it did. A failed run keeps the old rows on
/// screen, marked stale, rather than blanking the card.
#[derive(Clone, Debug, Default)]
pub struct Feed {
    pub rows: Option<Vec<CardRow>>,
    pub error: Option<String>,
}

impl Feed {
    pub fn update(&mut self, result: Result<Vec<CardRow>, String>) {
        match result {
            Ok(rows) => {
                self.rows = Some(rows);
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
}

/// A dashboard card ready to render.
pub struct SideCard {
    pub title: String,
    pub glyph: String,
    pub accent: Rgb,
    pub rows: Vec<CardRow>,
    /// One-line stand-in shown inside the status card when this card
    /// doesn't fit the terminal (separate-box layout).
    pub summary: String,
    /// Floats left of the status card when the terminal fits three columns
    /// (separate-box layout); otherwise the card flows with the right stack.
    pub left: bool,
    /// The last refresh failed; these rows are the previous good ones.
    pub stale: bool,
}

/// Assemble the render-ready card list: custom cards in config order, docker
/// last. Cards that never produced rows (source down, empty output) don't
/// exist.
pub fn build_cards(
    configs: &[CardConfig],
    docker: &Option<Vec<Container>>,
    feeds: &[Feed],
) -> Vec<SideCard> {
    let mut out = Vec::new();
    for (cfg, feed) in configs.iter().zip(feeds) {
        let Some(rows) = feed.rows.as_ref().filter(|rows| !rows.is_empty()) else {
            continue;
        };
        out.push(SideCard {
            title: cfg.title.clone(),
            glyph: cfg.glyph.clone().unwrap_or_else(|| DEFAULT_GLYPH.into()),
            accent: cfg.accent.map_or(VIOLET_RGB, |a| a.0),
            rows: space_headers(rows),
            summary: format!("{} items", rows.len()),
            left: cfg.side == Side::Left,
            stale: feed.error.is_some(),
        });
    }
    if let Some(containers) = docker.as_ref().filter(|c| !c.is_empty()) {
        out.push(SideCard {
            title: "docker".into(),
            glyph: "≋".into(),
            accent: BLUE_RGB,
            rows: containers
                .iter()
                .map(|c| CardRow {
                    health: Some(c.health),
                    container_state: Some(c.state),
                    name: c.name.clone(),
                    detail: Some(c.status.clone()),
                    ..CardRow::default()
                })
                .collect(),
            summary: format!("{} containers", containers.len()),
            left: false,
            stale: false,
        });
    }
    out
}

/// A blank line above every group header but the first, so a card built
/// from several sources — the ai usage card's one group per service —
/// reads as separate blocks instead of one dense list. The spacing lands in
/// the row list itself rather than at render time, because every height
/// budget here is counted in rows: a line conjured during drawing would be
/// one the layout never reserved, and the card would quietly lose its last
/// row to make room.
fn space_headers(rows: &[CardRow]) -> Vec<CardRow> {
    let mut out: Vec<CardRow> = Vec::with_capacity(rows.len() + 4);
    for r in rows {
        if r.header && !out.is_empty() && !out.last().is_some_and(CardRow::is_blank) {
            out.push(CardRow::default());
        }
        out.push(r.clone());
    }
    out
}

/// The color a value takes when its row or meter needs attention.
pub(crate) fn severity_color(sev: Option<Health>, otherwise: Color) -> Color {
    match sev {
        Some(Health::Bad) => RED,
        Some(Health::Warn) => AMBER,
        _ => otherwise,
    }
}

/// Display copies of rows: uppercased once, before anything is measured.
/// Source rows keep their case; selection identity reads those.
fn display_rows<'a>(rows: impl Iterator<Item = &'a CardRow>) -> Vec<CardRow> {
    rows.map(|row| CardRow {
        name: row.name.to_uppercase(),
        detail: row.detail.as_ref().map(|d| d.to_uppercase()),
        ..row.clone()
    })
    .collect()
}

/// The shared column widths for a run of rows: health-row names (when they
/// have a detail beside them) and meter labels.
struct Columns {
    name: usize,
    label: usize,
}

impl Columns {
    fn measure(rows: &[CardRow]) -> Self {
        let widest = |keep: fn(&CardRow) -> bool, cap: usize| {
            rows.iter()
                .filter(|r| keep(r))
                .map(|r| width(&r.name))
                .max()
                .unwrap_or(0)
                .min(cap)
        };
        Self {
            name: widest(|r| r.detail.is_some() && r.percent.is_none(), NAME_W),
            label: widest(|r| r.percent.is_some(), METER_LABEL_W),
        }
    }
}

/// The width [`render_side_card`] needs to show every row of `card` whole:
/// borders and indent, the health dot, the shared name column, and the
/// longest detail — or the title line, whichever is wider.
///
/// Both column widths are the shared ones the renderer will use, not each
/// row's own: a meter with a short label still sits behind the widest
/// label's gauge, so measuring it against its own label would ask for a
/// card too narrow to hold that row's detail.
pub(crate) fn natural_width(card: &SideCard) -> u16 {
    let rows = display_rows(card.rows.iter());
    let cols = Columns::measure(&rows);
    let body = rows
        .iter()
        .map(|r| {
            let detail = r.detail.as_deref().map_or(0, width);
            if r.percent.is_some() {
                return METER_CHROME + cols.label + GAUGE_MAX_W + detail + 2;
            }
            let dot = if r.health.is_some() { 2 } else { 0 };
            match &r.detail {
                Some(_) => dot + cols.name + 1 + detail,
                None => dot + width(&r.name),
            }
        })
        .max()
        .unwrap_or(0);
    let title = width(&card.glyph) + width(&card.title) + 8;
    (body + 4).max(title) as u16
}

/// One separate-layout card: an accent-tinted box titled with a glyph and
/// count, listing rows with optional health dots and detail columns.
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
                card.title.to_uppercase(),
                card.rows.iter().filter(|r| !r.is_blank()).count()
            ),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
    ];
    if bad > 0 {
        title.push(Span::styled(
            format!(" · {bad} BAD"),
            Style::default().fg(RED),
        ));
    }
    if card.stale {
        title.push(Span::styled(" · STALE", Style::default().fg(AMBER)));
    }
    title.push(Span::raw(" "));
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
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
        GAUGE_MAX_W,
    ));
    Paragraph::new(lines).render(inner, buf);
}

/// The body lines of a card: health dots, aligned name/detail columns,
/// accented headers, meters, and a trailing "+N MORE" when rows are cut.
/// `inner_w` is the width of the box the lines will live in, minus borders.
pub(crate) fn card_row_lines(
    rows: &[CardRow],
    shown: usize,
    inner_w: usize,
    tick: u64,
    gauge_max: usize,
) -> Vec<Line<'static>> {
    let hidden = rows.len().saturating_sub(shown);
    let rows = display_rows(rows.iter().take(shown));
    let cols = Columns::measure(&rows);
    let mut lines = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        lines.push(if r.header {
            header_line(r, inner_w)
        } else if let Some(percent) = r.percent {
            let continues = rows.get(i + 1).is_some_and(|next| next.percent.is_some());
            meter_line(r, percent, continues, cols.label, inner_w, gauge_max)
        } else {
            entry_line(r, cols.name, inner_w, tick)
        });
    }
    if hidden > 0 {
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(METER_INDENT)),
            Span::styled(format!("+{hidden} MORE"), Style::default().fg(SLATE_600)),
        ]));
    }
    lines
}

/// A group header: the title in bold.
fn header_line(r: &CardRow, inner_w: usize) -> Line<'static> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            truncate(&r.name, inner_w.saturating_sub(4)),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
    ])
}

/// A utilization meter: `╰ label ███▍░░░ 42%  detail`, joined to the row
/// above by a branch rail, the rail forking when meters stack. The value
/// turns amber and then red as it nears the limit; the gauge keeps the
/// accent.
fn meter_line(
    r: &CardRow,
    percent: u8,
    continues: bool,
    label_w: usize,
    inner_w: usize,
    gauge_max: usize,
) -> Line<'static> {
    let accent = r.accent.unwrap_or(VIOLET_RGB);
    let bar_color = scale(accent, 1.0);
    let value_color = severity_color(severity(Some(percent), None), bar_color);
    let detail = r.detail.as_deref().unwrap_or_default();
    let branch = if continues { "├ " } else { "╰ " };
    // Reset text must never change the scale of an individual gauge.
    let available = inner_w.saturating_sub(METER_CHROME + label_w);
    let gauge_w = available.min(gauge_max);
    let detail_w = width(detail).min(available.saturating_sub(gauge_w + 2));
    let (filled, partial, empty) = gauge_parts(percent, gauge_w);
    let mut spans = vec![
        Span::raw(" ".repeat(METER_INDENT)),
        Span::styled(branch, Style::default().fg(scale(accent, 0.5))),
        Span::styled(
            format!("{} ", pad(&r.name, label_w)),
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

/// Single-cell indicators breathe gently without changing shape or layout.
pub(crate) fn container_badge(
    state: ContainerState,
    health: Option<Health>,
    tick: u64,
) -> (&'static str, Color) {
    if health == Some(Health::Bad) {
        return ("×", RED);
    }
    // About five seconds per cycle at the normal frame interval.
    let brightness = 0.62 + 0.10 * ((tick % 42) as f32 / 42.0 * std::f32::consts::TAU).sin();
    match state {
        ContainerState::Running => (
            "●",
            if health == Some(Health::Warn) {
                scale(AMBER_RGB, brightness)
            } else {
                scale(EMERALD_RGB, brightness)
            },
        ),
        ContainerState::Paused => ("Ⅱ", scale(AMBER_RGB, 0.65)),
        ContainerState::Stopped => ("○", SLATE_500),
        ContainerState::Restarting => ("◌", scale(AMBER_RGB, brightness)),
        ContainerState::Unknown => ("?", SLATE_400),
    }
}

/// A container's status without its leading lifecycle word (the badge
/// shows that), uppercased, with health checks as ✓ / ×.
pub(crate) fn container_detail(row: &CardRow) -> String {
    let detail = row.detail.as_deref().unwrap_or_default();
    let rest = detail
        .split_once(' ')
        .map_or("", |(_, rest)| rest)
        .trim_start_matches('·')
        .trim();
    rest.to_uppercase()
        .replace("(HEALTHY)", "✓")
        .replace("(UNHEALTHY)", "×")
        .replace("(PAUSED)", "")
}

fn health_color(h: Health) -> Color {
    match h {
        Health::Bad => RED,
        Health::Warn => AMBER,
        Health::Good => MUTED,
        Health::Off => SLATE_600,
    }
}

/// An ordinary row: an optional health dot, then either aligned name/detail
/// columns (a `▸` name marks the active account and renders bright) or a
/// single run of text in the row's accent.
fn entry_line(r: &CardRow, name_w: usize, inner_w: usize, tick: u64) -> Line<'static> {
    if let Some(state) = r.container_state {
        let (badge, color) = container_badge(state, r.health, tick);
        let name_w = name_w.min(inner_w.saturating_sub(6));
        let prefix = format!("  {}  ", pad(&r.name, name_w));
        let detail = container_detail(r);
        return Line::from(vec![
            Span::styled(prefix, Style::default().fg(SLATE_400)),
            Span::styled(
                truncate(badge, inner_w.saturating_sub(name_w + 4)),
                Style::default().fg(color),
            ),
            Span::styled(
                truncate(&format!("  {detail}"), inner_w.saturating_sub(name_w + 5)),
                Style::default().fg(MUTED),
            ),
        ]);
    }
    let mut spans = vec![Span::raw("  ")];
    let mut used = 2usize;
    if let Some(h) = r.health {
        spans.push(Span::styled("● ", Style::default().fg(health_color(h))));
        used += 2;
    }
    match &r.detail {
        Some(detail) => {
            let active = r.name.trim_start().starts_with('▸');
            let style = if active {
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(SLATE_400)
            };
            spans.push(Span::styled(format!("{} ", pad(&r.name, name_w)), style));
            let dw = inner_w.saturating_sub(used + name_w + 1);
            spans.push(Span::styled(
                truncate(detail, dw),
                Style::default().fg(SLATE_500),
            ));
        }
        None => {
            let w = inner_w.saturating_sub(used);
            let fg = r.accent.map_or(SLATE_400, |c| scale(c, 1.0));
            spans.push(Span::styled(truncate(&r.name, w), Style::default().fg(fg)));
        }
    }
    Line::from(spans)
}

/// A compact gauge with eighth-cell precision. Full blocks carry the accent,
/// a partial block preserves small percentages, and shade cells show the
/// unused portion without letting a meter dominate the whole row.
pub(crate) fn gauge_parts(percent: u8, width: usize) -> (usize, Option<char>, usize) {
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

    fn row(name: &str, header: bool) -> CardRow {
        CardRow {
            health: (!header && !name.is_empty()).then_some(Health::Good),
            name: name.into(),
            header,
            ..CardRow::default()
        }
    }

    #[test]
    fn headers_get_breathing_room_but_never_a_leading_blank() {
        let rows = vec![
            row("✻ claude", true),
            row("personal", false),
            row("◎ openai codex", true),
            row("personal", false),
        ];
        let spaced = space_headers(&rows);
        let shape: Vec<&str> = spaced
            .iter()
            .map(|r| if r.is_blank() { "_" } else { "x" })
            .collect();
        assert_eq!(shape, ["x", "x", "_", "x", "x"]);
    }

    #[test]
    fn existing_blanks_are_left_alone() {
        // A command that prints its own blank line before a header doesn't
        // get a second one stacked on top.
        let rows = vec![row("web", false), row("", false), row("≋ docker", true)];
        assert_eq!(space_headers(&rows).len(), rows.len());
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
            accent: VIOLET_RGB,
            rows: vec![
                CardRow {
                    health: Some(Health::Bad),
                    name: "work.account".into(),
                    detail: Some("business · 0 credits left · limit reached · stale 2d".into()),
                    ..CardRow::default()
                },
                meter("5h window", "resets Mon 00:00 (in 4h)"),
                meter("monthly credits", "resets Mon 19:00 (in 21d)"),
            ],
            summary: "3 items".into(),
            left: false,
            stale: false,
        };
        let inner = natural_width(&card) as usize - 2;
        for line in card_row_lines(&card.rows, card.rows.len(), inner, 7, GAUGE_MAX_W) {
            let rendered = text(&line);
            assert!(
                !rendered.contains('…'),
                "clipped at natural width: {rendered}"
            );
            assert!(width(&rendered) <= inner, "overflows: {rendered}");
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
        let line = text(&card_row_lines(&rows, 1, 80, 7, GAUGE_MAX_W)[0]);
        assert!(line.ends_with("RESETS MON 19:00 (IN 21D)"), "{line}");
    }

    #[test]
    fn every_row_kind_renders_at_any_width() {
        // Sections, meters with and without details, health rows, plain and
        // blank rows — none may panic however narrow the card gets.
        let rows = vec![
            row("✻ claude", true),
            CardRow {
                health: Some(Health::Good),
                name: "▸ personal".into(),
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
            let lines = card_row_lines(&rows, rows.len(), w, 7, GAUGE_MAX_W);
            assert_eq!(lines.len(), rows.len());
        }
        let cut = card_row_lines(&rows, 2, 40, 7, GAUGE_MAX_W);
        assert_eq!(text(&cut[2]).trim(), "+4 MORE");
    }

    #[test]
    fn wide_names_keep_the_detail_column_aligned() {
        let rows = vec![
            CardRow {
                health: Some(Health::Good),
                name: "測試".into(),
                detail: Some("one".into()),
                ..CardRow::default()
            },
            CardRow {
                health: Some(Health::Good),
                name: "abcd".into(),
                detail: Some("two".into()),
                ..CardRow::default()
            },
        ];
        let lines: Vec<String> = card_row_lines(&rows, 2, 40, 7, GAUGE_MAX_W)
            .iter()
            .map(text)
            .collect();
        let column = |l: &str, d: &str| width(&l[..l.find(d).unwrap()]);
        assert_eq!(column(&lines[0], "ONE"), column(&lines[1], "TWO"));
    }

    #[test]
    fn failed_refreshes_keep_the_last_rows_and_mark_them_stale() {
        let cfg = CardConfig {
            title: "t".into(),
            command: "true".into(),
            interval: 10,
            glyph: None,
            accent: None,
            side: Side::Right,
        };
        let mut feed = Feed::default();
        feed.update(Err("exit 1".into()));
        assert!(build_cards(
            std::slice::from_ref(&cfg),
            &None,
            std::slice::from_ref(&feed)
        )
        .is_empty());
        feed.update(Ok(vec![row("web", false)]));
        feed.update(Err("timed out after 45s".into()));
        let cards = build_cards(
            std::slice::from_ref(&cfg),
            &None,
            std::slice::from_ref(&feed),
        );
        assert_eq!(cards.len(), 1);
        assert!(cards[0].stale);
        assert_eq!(cards[0].rows[0].name, "web");
    }
}
