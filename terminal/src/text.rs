//! Display-width text helpers. Every measurement here is in terminal cells
//! (`Line::width`), never `chars().count()`, so wide characters (CJK, some
//! symbols) truncate, pad, and wrap without shifting the columns after them.
//! None of these change case; callers uppercase before measuring.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

pub fn width(s: &str) -> usize {
    Line::from(s).width()
}

/// Clip to `max` cells, ending in `…` when anything was cut.
pub fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        String::new()
    } else if width(s) <= max {
        s.to_string()
    } else {
        let mut out = String::new();
        let mut used = 0;
        for ch in s.chars() {
            let w = width(ch.encode_utf8(&mut [0; 4]));
            if used + w > max - 1 {
                break;
            }
            out.push(ch);
            used += w;
        }
        out.push('…');
        out
    }
}

/// Clip or space-fill to exactly `cells` wide.
pub fn pad(s: &str, cells: usize) -> String {
    let clipped = truncate(s, cells);
    let fill = cells.saturating_sub(width(&clipped));
    format!("{clipped}{}", " ".repeat(fill))
}

/// `left` flush left and `right` flush right across `cells`; the right side
/// gets at most half the room and both clip rather than overlap.
pub fn pair(
    left: &str,
    right: &str,
    cells: usize,
    left_color: Color,
    right_color: Color,
) -> Line<'static> {
    let right = truncate(right, width(right).min(cells / 2));
    let left_w = cells.saturating_sub(width(&right) + usize::from(!right.is_empty()) * 2);
    let left = truncate(left, left_w);
    let gap = cells.saturating_sub(width(&left) + width(&right));
    Line::from(vec![
        Span::styled(left, Style::default().fg(left_color)),
        Span::raw(" ".repeat(gap)),
        Span::styled(right, Style::default().fg(right_color)),
    ])
}

/// Word-wrap into `cells`-wide lines with a two-cell hanging indent, so the
/// layout can budget every line up front and page through all of them.
pub fn wrap(text: &str, cells: usize, color: Color) -> Vec<Line<'static>> {
    if cells == 0 {
        return Vec::new();
    }
    let indent = if cells > 4 { 2 } else { 0 };
    let body_w = cells - indent;
    let mut chunks = Vec::new();
    let mut line = String::new();
    for ch in text.chars() {
        if ch == '\n' {
            chunks.push(std::mem::take(&mut line));
            continue;
        }
        let mut next = line.clone();
        next.push(ch);
        if width(&next) > body_w {
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
        if width(ch.encode_utf8(&mut [0; 4])) <= body_w {
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

    #[test]
    fn wide_characters_truncate_and_pad_by_cells() {
        assert_eq!(width("測試"), 4);
        assert_eq!(truncate("測試測試", 5), "測試…");
        assert_eq!(width(&pad("測試", 6)), 6);
        assert_eq!(pad("ABCDEFG", 4), "ABC…");
        assert_eq!(truncate("ABC", 0), "");
    }

    #[test]
    fn wrap_never_exceeds_the_width() {
        let text = "SOME UNBROKEN_非常長的說明_012345678901234567890123456789";
        for cells in 0..40 {
            for line in wrap(text, cells, Color::Reset) {
                assert!(line.width() <= cells, "{cells}: {line:?}");
            }
        }
    }
}
