//! Half-block pixel rendering: `▀` with independent fg (top) and bg (bottom)
//! colors turns every terminal cell into two vertically stacked pixels, so
//! sprites get double vertical resolution and near-square pixels.

use ratatui::{buffer::Buffer, layout::Rect, style::Color};

pub type Rgb = (u8, u8, u8);

pub fn dim(rgb: Rgb, f: f32) -> Rgb {
    (
        (rgb.0 as f32 * f) as u8,
        (rgb.1 as f32 * f) as u8,
        (rgb.2 as f32 * f) as u8,
    )
}

fn color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.0, rgb.1, rgb.2)
}

fn lookup(map: &[(char, Rgb)], ch: char) -> Option<Rgb> {
    if ch == '.' || ch == ' ' {
        return None;
    }
    map.iter().find(|(c, _)| *c == ch).map(|(_, rgb)| *rgb)
}

/// Draw a pixel-art sprite anchored at a cell position; `.` and space are
/// transparent. Each pixel goes through `set_px`, so overlapping sprites
/// compose correctly (a ship crossing a planet occludes only its own pixels).
pub fn draw_px_art(
    rows: &[&str],
    map: &[(char, Rgb)],
    cell_x: i32,
    cell_y: i32,
    depth: f32,
    area: Rect,
    buf: &mut Buffer,
) {
    for (dy, row) in rows.iter().enumerate() {
        for (dx, ch) in row.chars().enumerate() {
            if let Some(rgb) = lookup(map, ch) {
                set_px(
                    cell_x + dx as i32,
                    cell_y * 2 + dy as i32,
                    dim(rgb, depth),
                    area,
                    buf,
                );
            }
        }
    }
}

/// Set a single pixel at (cell column, pixel row), merging with any half-block
/// already in the cell so freehand drawing (comet, nebula) composes correctly.
pub fn set_px(x: i32, y_px: i32, rgb: Rgb, area: Rect, buf: &mut Buffer) {
    let cy = y_px.div_euclid(2);
    if x < area.left() as i32
        || x >= area.right() as i32
        || cy < area.top() as i32
        || cy >= area.bottom() as i32
    {
        return;
    }
    let Some(cell) = buf.cell_mut((x as u16, cy as u16)) else {
        return;
    };
    let c = color(rgb);
    let top = y_px.rem_euclid(2) == 0;
    let sym = cell.symbol();
    if top {
        if sym == "▄" {
            let old = cell.style().fg;
            cell.set_char('▀');
            cell.set_fg(c);
            if let Some(o) = old {
                cell.set_bg(o);
            }
        } else {
            cell.set_char('▀');
            cell.set_fg(c);
        }
    } else if sym == "▀" {
        cell.set_bg(c);
    } else if sym == "▄" {
        cell.set_fg(c);
    } else {
        cell.set_char('▄');
        cell.set_fg(c);
    }
}
