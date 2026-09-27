//! A focused presentation of the existing card protocol. Account meters stay
//! attached to their source row; all details stay visible, and selection follows source identity.

use std::collections::HashSet;

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::cards::{self, SideCard};
use crate::collect::{CardRow, Health};
use crate::theme::{AMBER, MUTED, RED, SELECTION, TEXT, VIOLET_LIGHT};

// One shared value column for accounts, meters, reset details, and containers.
const VALUE_COLUMN: usize = 22;
const ACCOUNT_WIDTH: usize = VALUE_COLUMN - 4;
const METER_LABEL_WIDTH: usize = VALUE_COLUMN - 7;

fn gauge_width(width: usize) -> usize {
    // Keep percentage and stale status space reserved for every meter.
    width.saturating_sub(VALUE_COLUMN + 12).min(24)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EntryKey {
    section: String,
    provider: String,
    name: String,
    occurrence: usize,
}

pub struct Entry {
    pub key: EntryKey,
    provider: String,
    accent: Color,
    row: CardRow,
    details: Vec<CardRow>,
}

impl Entry {
    fn peak(&self) -> Option<&CardRow> {
        self.details
            .iter()
            .filter(|r| r.percent.is_some())
            .max_by_key(|r| r.percent)
    }

    fn attention(&self) -> bool {
        matches!(self.row.health, Some(Health::Bad | Health::Warn))
            || self.peak().is_some_and(|r| r.percent >= Some(75))
    }

    fn stale(&self) -> bool {
        self.row.detail.as_deref().is_some_and(|detail| {
            detail
                .to_lowercase()
                .split_whitespace()
                .any(|w| w == "stale" || w == "cached")
        })
    }
}

struct Section {
    title: String,
    entries: Vec<Entry>,
}

pub struct Dashboard {
    sections: Vec<Section>,
}

impl Dashboard {
    pub fn from_cards(cards: &[SideCard], merged: bool) -> Self {
        let mut sections: Vec<Section> = Vec::new();
        for card in cards {
            let mut section = Section {
                title: card.title.clone(),
                entries: Vec::new(),
            };
            let mut provider = String::new();
            let mut accent = Color::Rgb(card.accent.0, card.accent.1, card.accent.2);
            let mut attach_meter = false;
            for row in &card.rows {
                if merged
                    && row.accent.is_some()
                    && !row.section
                    && row.health.is_none()
                    && row.percent.is_none()
                    && !row.name.trim().is_empty()
                {
                    if !section.entries.is_empty() {
                        sections.push(section);
                    }
                    section = Section {
                        title: clean_label(&row.name),
                        entries: Vec::new(),
                    };
                    provider.clear();
                    accent = rgb(row.accent);
                    attach_meter = false;
                } else if row.section {
                    provider = clean_label(&row.name);
                    accent = rgb(row.accent);
                    attach_meter = false;
                } else if row.percent.is_some() && attach_meter {
                    if let Some(entry) = section.entries.last_mut() {
                        entry.details.push(row.clone());
                    }
                } else if !row.name.trim().is_empty() || row.detail.is_some() {
                    let occurrence = section
                        .entries
                        .iter()
                        .filter(|e| {
                            e.key.provider == provider && e.key.name == identity_name(&row.name)
                        })
                        .count();
                    section.entries.push(Entry {
                        key: EntryKey {
                            section: format!(
                                "{}:{}",
                                section.title,
                                sections.iter().filter(|s| s.title == section.title).count()
                            ),
                            provider: provider.clone(),
                            name: identity_name(&row.name).into(),
                            occurrence,
                        },
                        provider: provider.clone(),
                        accent,
                        row: row.clone(),
                        details: Vec::new(),
                    });
                    attach_meter = row.health.is_some();
                } else {
                    attach_meter = false;
                }
            }
            if !section.entries.is_empty() {
                sections.push(section);
            }
        }
        Self { sections }
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.sections.iter().flat_map(|s| &s.entries)
    }

    pub fn render(&self, nav: &Navigation, width: usize) -> Rendered {
        self.render_animated(nav, width, 0)
    }

    pub fn render_animated(&self, nav: &Navigation, width: usize, tick: u64) -> Rendered {
        let mut result = Rendered::default();
        for section in &self.sections {
            if !result.lines.is_empty() {
                result.lines.push(Line::default());
            }
            let attention = section.entries.iter().filter(|e| e.attention()).count();
            let note = if attention > 0 {
                format!(
                    "{attention} {} attention",
                    if attention == 1 { "needs" } else { "need" }
                )
            } else {
                String::new()
            };
            result.lines.push(pair(
                &section.title.to_uppercase(),
                &note,
                width,
                TEXT,
                AMBER,
            ));
            result.lines.push(Line::default());
            let name_width = ACCOUNT_WIDTH.min(width.saturating_sub(4) / 2);
            let mut provider = "";
            for entry in &section.entries {
                if entry.provider != provider {
                    if !provider.is_empty() {
                        result.lines.push(Line::default());
                    }
                    provider = &entry.provider;
                    if !provider.is_empty() {
                        result.lines.push(Line::from(Span::styled(
                            cards::truncate(&provider.to_uppercase(), width),
                            Style::default().fg(entry.accent),
                        )));
                    }
                }
                let start = result.lines.len();
                let selected = nav.selected.as_ref() == Some(&entry.key);
                let marker = if selected { "›" } else { " " };
                if let Some(state) = entry.row.container_state {
                    let (badge, color) = cards::container_badge(state, entry.row.health, tick);
                    let name = identity_name(&entry.row.name).to_uppercase();
                    let detail = cards::container_detail(&entry.row);
                    let line = Line::from(vec![
                        Span::styled(
                            format!("{marker} {name:width$}  ", width = name_width),
                            Style::default().fg(entry.accent),
                        ),
                        Span::styled(badge, Style::default().fg(color)),
                        Span::styled(format!("  {detail}"), Style::default().fg(MUTED)),
                    ]);
                    if line.width() <= width {
                        result.lines.push(line);
                    } else {
                        result
                            .lines
                            .extend(wrapped(&line.to_string(), width, color));
                    }
                } else if entry.row.percent.is_some() {
                    result
                        .lines
                        .extend(detail_lines(&entry.row, entry.accent, false, width));
                } else {
                    let name = identity_name(&entry.row.name).to_uppercase();
                    let detail = entry
                        .row
                        .detail
                        .as_deref()
                        .unwrap_or_default()
                        .to_uppercase();
                    // Allow a long account to use one of the two gap cells,
                    // keeping the value column fixed without repeating its name.
                    let name_cells = Line::from(name.as_str()).width();
                    let account_width = name_width.max(name_cells.min(name_width + 1));
                    let gap = name_width + 2 - account_width;
                    let detail_width = width.saturating_sub(name_width + 4);
                    if width < 28 || name_cells > account_width {
                        result.lines.extend(wrapped(
                            &format!("{marker} {name}"),
                            width,
                            entry.accent,
                        ));
                        result.lines.extend(wrapped(
                            &detail,
                            width,
                            value_color(None, entry.row.health),
                        ));
                    } else {
                        let line = Line::from(vec![
                            Span::styled(
                                format!("{marker} "),
                                Style::default().fg(value_color(None, entry.row.health)),
                            ),
                            Span::styled(
                                pad(&name, account_width),
                                Style::default().fg(entry.accent),
                            ),
                            Span::raw(" ".repeat(gap)),
                            Span::styled(
                                cards::truncate(&detail, detail_width),
                                Style::default().fg(value_color(None, entry.row.health)),
                            ),
                        ]);
                        result.lines.push(line);
                        if Line::from(detail.as_str()).width() > detail_width {
                            result.lines.extend(wrapped(&detail, width, MUTED));
                        }
                    }
                }
                if selected {
                    if let Some(line) = result.lines.get_mut(start) {
                        *line = line.clone().style(Style::default().bg(SELECTION));
                    }
                }
                for detail in &entry.details {
                    result
                        .lines
                        .extend(detail_lines(detail, entry.accent, entry.stale(), width));
                }
                if selected {
                    result.selected = Some(start..result.lines.len());
                }
            }
        }
        result
    }
}

fn rgb(value: Option<(u8, u8, u8)>) -> Color {
    value.map_or(VIOLET_LIGHT, |(r, g, b)| Color::Rgb(r, g, b))
}

fn detail_lines(row: &CardRow, accent: Color, stale: bool, width: usize) -> Vec<Line<'static>> {
    let mut meter = row.clone();
    meter.detail = None;
    if stale {
        meter.detail = Some("STALE".into());
    }
    if meter.accent.is_none() {
        if let Color::Rgb(r, g, b) = accent {
            meter.accent = Some((r, g, b));
        }
    }
    let mut lines = if width < 55 {
        wrapped(
            &format!(
                "{}  {}%{}",
                row.name,
                row.percent.unwrap_or(0),
                if stale { " · STALE" } else { "" }
            ),
            width,
            accent,
        )
    } else {
        meter.name = pad(&row.name.to_uppercase(), METER_LABEL_WIDTH);
        let mut lines = cards::card_row_lines_sized(
            std::slice::from_ref(&meter),
            1,
            width,
            0,
            gauge_width(width),
        );
        let rendered: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        if !rendered.contains(&row.name.to_uppercase()) {
            lines.extend(wrapped(&row.name, width, accent));
        }
        lines
    };
    if let Some(detail) = &row.detail {
        let indent = if width >= 55 { VALUE_COLUMN - 2 } else { 0 };
        for mut line in wrapped(
            &display_reset_detail(detail),
            width.saturating_sub(indent),
            MUTED,
        ) {
            line.spans.insert(0, Span::raw(" ".repeat(indent)));
            lines.push(line);
        }
    }
    lines
}

// Custom cards supply reset text, sometimes in 24-hour format. Convert only
// standalone clock tokens; preserve dates, durations, and already formatted times.
fn display_reset_detail(detail: &str) -> String {
    let words: Vec<_> = detail.split_whitespace().collect();
    words
        .iter()
        .enumerate()
        .map(|(i, word)| {
            let next = words.get(i + 1).copied().unwrap_or_default();
            if !next.eq_ignore_ascii_case("AM") && !next.eq_ignore_ascii_case("PM") {
                if let Some((hour, minute)) = word.split_once(':') {
                    if hour.len() <= 2 && minute.len() == 2 {
                        if let (Ok(hour), Ok(minute)) = (hour.parse::<u8>(), minute.parse::<u8>()) {
                            if hour < 24 && minute < 60 {
                                return format!(
                                    "{}:{minute:02} {}",
                                    if hour % 12 == 0 { 12 } else { hour % 12 },
                                    if hour < 12 { "AM" } else { "PM" }
                                );
                            }
                        }
                    }
                }
            }
            word.to_uppercase()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Default)]
pub struct Navigation {
    pub selected: Option<EntryKey>,
    pub scroll: isize,
}

impl Navigation {
    pub fn sync(&mut self, dashboard: &Dashboard) {
        let keys: HashSet<_> = dashboard.entries().map(|e| e.key.clone()).collect();
        if self.selected.as_ref().is_none_or(|key| !keys.contains(key)) {
            self.selected = dashboard
                .entries()
                .find(|e| e.attention())
                .or_else(|| dashboard.entries().next())
                .map(|e| e.key.clone());
            self.scroll = 0;
        }
    }

    pub fn move_selection(&mut self, dashboard: &Dashboard, step: isize) {
        self.sync(dashboard);
        let keys: Vec<_> = dashboard.entries().map(|e| e.key.clone()).collect();
        if keys.is_empty() {
            return;
        }
        let index = keys
            .iter()
            .position(|k| Some(k) == self.selected.as_ref())
            .unwrap_or(0);
        let next = (index as isize + step).rem_euclid(keys.len() as isize) as usize;
        self.selected = Some(keys[next].clone());
        self.scroll = 0;
    }
}

#[derive(Default)]
pub struct Rendered {
    pub lines: Vec<Line<'static>>,
    selected: Option<std::ops::Range<usize>>,
}

impl Rendered {
    pub fn start(&self, height: usize, scroll: isize) -> usize {
        let max = self.lines.len().saturating_sub(height);
        let base = self.selected.as_ref().map_or(0, |range| {
            if range.end <= height {
                0
            } else if range.len() <= height {
                range.end.saturating_sub(height)
            } else {
                range.start
            }
        });
        base.saturating_add_signed(scroll).min(max)
    }
}

fn clean_label(text: &str) -> String {
    text.trim()
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim()
        .to_lowercase()
}

fn identity_name(text: &str) -> &str {
    text.trim().trim_start_matches('▸').trim()
}

fn value_color(percent: Option<u8>, health: Option<Health>) -> Color {
    if percent >= Some(90) || health == Some(Health::Bad) {
        RED
    } else if percent >= Some(75) || health == Some(Health::Warn) {
        AMBER
    } else {
        MUTED
    }
}

fn pad(text: &str, width: usize) -> String {
    let clipped = cards::truncate(text, width);
    format!(
        "{}{}",
        clipped,
        " ".repeat(width.saturating_sub(Line::from(clipped.as_str()).width()))
    )
}

pub fn pair(
    left: &str,
    right: &str,
    width: usize,
    left_color: Color,
    right_color: Color,
) -> Line<'static> {
    let left = left.to_uppercase();
    let right = right.to_uppercase();
    let right_w = Line::from(right.as_str()).width().min(width / 2);
    let right = cards::truncate(&right, right_w);
    let left_w = width.saturating_sub(right_w + usize::from(!right.is_empty()) * 2);
    let left = cards::truncate(&left, left_w);
    let gap = width
        .saturating_sub(Line::from(left.as_str()).width() + Line::from(right.as_str()).width());
    Line::from(vec![
        Span::styled(left, Style::default().fg(left_color)),
        Span::raw(" ".repeat(gap)),
        Span::styled(right, Style::default().fg(right_color)),
    ])
}

// Wrap before layout so all details are budgeted and reachable with page keys.
fn wrapped(text: &str, width: usize, color: Color) -> Vec<Line<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let indent = if width > 4 { 2 } else { 0 };
    let body_w = width - indent;
    let mut chunks = Vec::new();
    let mut line = String::new();
    for ch in text.to_uppercase().chars() {
        let mut next = line.clone();
        next.push(ch);
        if ch == '\n' {
            chunks.push(std::mem::take(&mut line));
            continue;
        }
        if Line::from(next.as_str()).width() > body_w {
            if ch == ' ' {
                chunks.push(std::mem::take(&mut line));
                continue;
            }
            if let Some(split) = line.rfind(' ').filter(|&split| split > 0) {
                chunks.push(line[..split].to_owned());
                line = line[split + 1..].to_owned();
            } else {
                chunks.push(std::mem::take(&mut line));
            }
        }
        if Line::from(ch.to_string()).width() <= body_w {
            line.push(ch);
        }
    }
    if !line.is_empty() {
        chunks.push(line);
    }
    chunks
        .into_iter()
        .map(|line| {
            Line::from(Span::styled(
                format!("{}{}", " ".repeat(indent), line.trim_end()),
                Style::default().fg(color),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(rows: Vec<CardRow>) -> SideCard {
        SideCard {
            title: "AI Usage".into(),
            glyph: "◆".into(),
            accent: (1, 2, 3),
            rows,
            summary: String::new(),
            left: false,
        }
    }

    fn account(name: &str, detail: &str) -> CardRow {
        CardRow {
            name: name.into(),
            detail: Some(detail.into()),
            health: Some(Health::Good),
            ..CardRow::default()
        }
    }

    fn meter(name: &str, percent: u8) -> CardRow {
        CardRow {
            name: name.into(),
            percent: Some(percent),
            detail: Some("Resets Mon 09:00".into()),
            ..CardRow::default()
        }
    }

    fn fixture() -> Dashboard {
        Dashboard::from_cards(
            &[card(vec![
                CardRow {
                    name: "✻ Claude".into(),
                    section: true,
                    ..CardRow::default()
                },
                account("▸ Personal", "active"),
                meter("5h window", 9),
                meter("weekly", 42),
                meter("model weekly", 100),
                account("Work", "Stale 1d"),
                meter("monthly", 95),
            ])],
            false,
        )
    }

    fn text(rendered: &Rendered) -> String {
        rendered
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn long_account_names_are_displayed_once_without_moving_the_value_column() {
        let name = "andy-nguyen_kyndryl";
        let dashboard =
            Dashboard::from_cards(&[card(vec![account(name, "biz · unlimited")])], false);
        let rendered = text(&dashboard.render(&Navigation::default(), 82));
        assert_eq!(rendered.matches("ANDY-NGUYEN_KYNDRYL").count(), 1);
        let row = rendered
            .lines()
            .find(|line| line.contains("ANDY-NGUYEN_KYNDRYL"))
            .unwrap();
        assert_eq!(row.find("BIZ"), Some(VALUE_COLUMN));
        assert!(!rendered.contains('…'));
        let name = "a_much_longer_account_name";
        let dashboard = Dashboard::from_cards(&[card(vec![account(name, "active")])], false);
        let rendered = text(&dashboard.render(&Navigation::default(), 82));
        assert_eq!(rendered.matches(&name.to_uppercase()).count(), 1);
        assert!(!rendered.contains('…'));
    }

    #[test]
    fn account_meter_and_reset_values_share_a_column() {
        for width in [55, 65, 82, 102] {
            let rendered = fixture().render(&Navigation::default(), width);
            for line in &rendered.lines {
                let text = line.to_string();
                if let Some(column) = text.chars().position(|ch| "█▏▎▍▌▋▊▉░".contains(ch))
                {
                    assert_eq!(column, VALUE_COLUMN);
                }
                if text.trim_start().starts_with("RESETS") {
                    assert_eq!(
                        text.chars().take_while(|ch| *ch == ' ').count(),
                        VALUE_COLUMN
                    );
                }
            }
        }
    }

    #[test]
    fn reset_details_are_below_the_meter_in_twelve_hour_time() {
        let mut row = meter("5h window", 9);
        row.detail = Some("91% left · reset in 2h · TODAY 14:30 CDT".into());
        for width in [32, 55, 82, 102] {
            let lines = detail_lines(&row, VIOLET_LIGHT, false, width);
            assert!(!lines[0].to_string().contains("RESET"));
            let details = lines
                .iter()
                .skip(1)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            assert!(details.contains("RESET IN 2H"));
            assert!(details
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .contains("2:30 PM CDT"));
            assert!(lines.iter().all(|line| line.width() <= width));
        }
        assert_eq!(
            display_reset_detail("SUN 00:00 · FRI 12:00"),
            "SUN 12:00 AM · FRI 12:00 PM"
        );
        assert_eq!(
            display_reset_detail("TODAY 2:30 PM · 02:14:00"),
            "TODAY 2:30 PM · 02:14:00"
        );
    }

    #[test]
    fn all_quotas_and_stale_state_are_visible_without_interaction() {
        let dashboard = fixture();
        assert_eq!(dashboard.entries().count(), 2);
        let rendered = dashboard.render(&Navigation::default(), 82);
        let text = text(&rendered);
        assert!(text.contains("100%"));
        assert!(text.contains("95%"));
        assert!(text.contains("STALE 1D"));
        assert!(text.contains("9%"));
        assert!(!text.contains("!"));
        let stale_line = rendered
            .lines
            .iter()
            .find(|line| line.spans.iter().any(|span| span.content.contains("95%")))
            .unwrap();
        assert!(stale_line
            .spans
            .iter()
            .any(|span| span.content.contains('█')));
    }

    #[test]
    fn one_hundred_percent_fills_the_entire_bar_even_when_stale() {
        for stale in [false, true] {
            let lines = detail_lines(
                &meter("model weekly", 100),
                Color::Rgb(217, 119, 87),
                stale,
                82,
            );
            let spans = &lines[0].spans;
            assert!(spans
                .iter()
                .any(|span| span.content == "█".repeat(gauge_width(82))));
            assert!(!spans.iter().any(|span| span.content.contains('░')));
            assert!(spans.iter().any(|span| span.content.contains("100%")));
            if stale {
                assert!(spans.iter().any(|span| span.content.contains("STALE")));
            }
        }
    }

    #[test]
    fn quota_bars_and_percentages_align_regardless_of_reset_text() {
        for width in [55, 65, 82, 98, 102] {
            let mut expected = None;
            for percent in [0, 9, 42, 95, 100] {
                for detail in [
                    None,
                    Some("resets today 14:30"),
                    Some("resets mon 19:00 (in 21d)"),
                ] {
                    for stale in [false, true] {
                        let mut row = meter("weekly", percent);
                        row.detail = detail.map(str::to_owned);
                        let lines = detail_lines(&row, Color::Rgb(217, 119, 87), stale, width);
                        let line: String = lines[0]
                            .spans
                            .iter()
                            .map(|span| span.content.as_ref())
                            .collect();
                        let chars: Vec<char> = line.chars().collect();
                        let start = chars
                            .iter()
                            .position(|ch| "█▏▎▍▌▋▊▉░".contains(*ch))
                            .unwrap();
                        let length = chars[start..]
                            .iter()
                            .take_while(|ch| "█▏▎▍▌▋▊▉░".contains(**ch))
                            .count();
                        let percent_column = chars.iter().position(|ch| *ch == '%').unwrap();
                        let dimensions = (start, length, percent_column);
                        assert_eq!(length, gauge_width(width));
                        assert_eq!(*expected.get_or_insert(dimensions), dimensions);
                        assert!(lines.iter().all(|line| line.width() <= width));
                    }
                }
            }
        }
    }

    #[test]
    fn every_quota_is_visible_immediately_with_its_provider_color() {
        let mut dashboard = fixture();
        let coral = Color::Rgb(217, 119, 87);
        let teal = Color::Rgb(16, 163, 127);
        dashboard.sections[0].entries[0].accent = coral;
        dashboard.sections[0].entries[1].accent = teal;
        let rendered = dashboard.render(&Navigation::default(), 82);
        let content = text(&rendered);
        assert!(content.contains("5H WINDOW"));
        assert!(content.contains("MODEL WEEKLY"));
        assert!(content.contains("RESETS MON 9:00 AM"));
        assert!(rendered
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .any(|span| span.content.contains('█') && span.style.fg == Some(coral)));
        assert!(rendered
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .any(|span| span.content.contains("WORK") && span.style.fg == Some(teal)));
    }

    #[test]
    fn selection_follows_identity_through_refresh_and_reordering() {
        let original = Dashboard::from_cards(
            &[card(vec![account("one", "a"), account("two", "b")])],
            false,
        );
        let mut nav = Navigation::default();
        nav.sync(&original);
        nav.move_selection(&original, 1);
        let key = nav.selected.clone();
        let refreshed = Dashboard::from_cards(
            &[card(vec![account("▸ two", "changed"), account("one", "a")])],
            false,
        );
        nav.sync(&refreshed);
        assert_eq!(nav.selected, key);
        assert!(text(&refreshed.render(&nav, 60)).contains("CHANGED"));
        let empty = Dashboard::from_cards(&[], false);
        nav.sync(&empty);
        nav.move_selection(&empty, -1);
        assert!(nav.selected.is_none());
    }

    #[test]
    fn ordinary_custom_content_and_orphan_meters_remain_reachable() {
        let dashboard = Dashboard::from_cards(
            &[card(vec![
                CardRow {
                    name: "Service Checks".into(),
                    section: true,
                    ..CardRow::default()
                },
                CardRow {
                    name: "/Some/Long/Path/With_A_Case_Sensitive_Name".into(),
                    ..CardRow::default()
                },
                meter("standalone", 32),
            ])],
            false,
        );
        assert_eq!(dashboard.entries().count(), 2);
        let mut nav = Navigation::default();
        nav.sync(&dashboard);
        let rendered = text(&dashboard.render(&nav, 30));
        assert!(rendered.contains("SERVICE CHECKS"));
        assert!(rendered.contains("/SOME/LONG/PATH"));
        assert!(rendered.contains("32%"));
    }

    #[test]
    fn narrow_tables_wrap_details_without_lowercase_or_overflow() {
        let mut dashboard = fixture();
        dashboard.sections[0].entries[0].row.name = "▸ 測試 very long account".into();
        dashboard.sections[0].entries[0].row.detail =
            Some("SOME UNBROKEN_非常長的說明_012345678901234567890123456789".into());
        let mut nav = Navigation::default();
        nav.sync(&dashboard);
        for width in 0..110 {
            for line in dashboard.render(&nav, width).lines {
                assert!(line.width() <= width, "width {width}: {line:?}");
                assert!(!line
                    .spans
                    .iter()
                    .any(|span| span.content.chars().any(char::is_lowercase)));
            }
        }
    }

    #[test]
    fn viewport_keeps_selection_visible_and_reaches_every_detail() {
        let dashboard = fixture();
        let mut nav = Navigation::default();
        nav.sync(&dashboard);
        nav.move_selection(&dashboard, 1);
        let rendered = dashboard.render(&nav, 40);
        let selected = rendered.selected.as_ref().unwrap();
        let start = rendered.start(6, 0);
        assert!((start..start + 6).contains(&selected.start));
        assert_eq!(rendered.start(6, isize::MAX), rendered.lines.len() - 6);
        assert_eq!(rendered.start(6, isize::MIN), 0);
    }

    #[test]
    fn container_symbols_animate_and_preserve_diagnostics() {
        use crate::collect::ContainerState as State;
        let mut rows = Vec::new();
        for (name, state, health, detail) in [
            (
                "WEB",
                State::Running,
                Health::Good,
                "running · up 3 hours (healthy)",
            ),
            ("PAUSED", State::Paused, Health::Warn, "paused"),
            (
                "BUILDER",
                State::Stopped,
                Health::Off,
                "stopped · exited (0) 2 hours ago",
            ),
            ("WORKER", State::Restarting, Health::Warn, "restarting"),
            (
                "BROKEN",
                State::Running,
                Health::Bad,
                "running · up 2 hours (unhealthy)",
            ),
        ] {
            rows.push(CardRow {
                name: name.into(),
                container_state: Some(state),
                health: Some(health),
                detail: Some(detail.into()),
                ..CardRow::default()
            });
        }
        let dashboard = Dashboard::from_cards(&[card(rows)], false);
        let first = text(&dashboard.render_animated(&Navigation::default(), 82, 0));
        let next = text(&dashboard.render_animated(&Navigation::default(), 82, 2));
        assert_eq!(first, next); // Motion changes only the indicator's brightness.
        assert_ne!(
            cards::container_badge(State::Running, Some(Health::Good), 0).1,
            cards::container_badge(State::Running, Some(Health::Good), 10).1,
        );
        for expected in [
            "●",
            "Ⅱ",
            "○",
            "◌",
            "×",
            "UP 3 HOURS ✓",
            "EXITED (0) 2 HOURS AGO",
        ] {
            assert!(first.contains(expected), "missing {expected}: {first}");
        }
        assert!(!first.contains("RUNNING"));
        assert!(!first.contains("RESTARTING"));
        for width in 0..110 {
            let rendered = dashboard.render_animated(&Navigation::default(), width, 7);
            assert!(rendered.lines.iter().all(|line| line.width() <= width));
        }
        for state in [State::Paused, State::Stopped] {
            assert_eq!(
                cards::container_badge(state, None, 0),
                cards::container_badge(state, None, 20)
            );
        }
    }
}
