//! Half-block pixel rendering: `▀`/`▄` with independent fg (top) and bg
//! (bottom) colors turn every terminal cell into two vertically stacked
//! pixels, so the sky's trails get double vertical resolution and
//! near-square pixels.

use ratatui::{buffer::Buffer, layout::Rect};

use crate::theme::{rgb, Rgb};

/// Set a single pixel at (cell column, pixel row), merging with any half-block
/// already in the cell so overlapping trails compose correctly (a comet
/// crossing a shooting star occludes only its own pixels).
pub fn set_px(x: i32, y_px: i32, color: Rgb, area: Rect, buf: &mut Buffer) {
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
    let c = rgb(color);
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
