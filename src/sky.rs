//! Everything that lives in the sky: the twinkling starfield, the glint
//! stars, and a registry of transient flyers — comets and shooting stars
//! that occasionally streak through.
//!
//! # Adding your own flyer
//!
//! 1. Write a `fn my_flyer(ctx: &Ctx, buf: &mut Buffer)` that positions
//!    itself from `ctx.tick` and draws with [`pixel::set_px`].
//! 2. Add one `Flyer` entry to [`FLYERS`]. Done — it flies, and users can
//!    ground it in their config with `[sky] my_flyer = false`.
//!
//! The whole sky draws before the title and the cards, so everything here
//! passes behind them.

use ratatui::style::Color;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::pixel;
use crate::theme::{SLATE_400, SLATE_500, SLATE_600, SLATE_700, TEXT, VIOLET_LIGHT};

/// Everything a flyer needs to place itself for the current frame.
pub struct Ctx {
    pub tick: u64,
    pub area: Rect,
}

pub struct Flyer {
    /// The `[sky]` config key that toggles this flyer.
    pub name: &'static str,
    pub draw: fn(&Ctx, &mut Buffer),
}

/// Every flyer in the sky, in draw order. This is the one list to extend.
pub const FLYERS: &[Flyer] = &[
    Flyer {
        name: "comets",
        draw: comets,
    },
    Flyer {
        name: "shooting_stars",
        draw: shooting_stars,
    },
];

pub fn names() -> impl Iterator<Item = &'static str> {
    FLYERS.iter().map(|f| f.name)
}

/// Resolve the `[sky]` toggle table into per-flyer switches, registry order.
/// Unlisted flyers default to on; unknown keys are rejected at config load.
pub fn resolve(toggles: &std::collections::HashMap<String, bool>) -> Vec<bool> {
    FLYERS
        .iter()
        .map(|f| *toggles.get(f.name).unwrap_or(&true))
        .collect()
}

/// Draw the whole sky: the ambient starfield and glint stars, then every
/// enabled flyer from the registry.
pub fn render(ctx: &Ctx, enabled: &[bool], buf: &mut Buffer) {
    starfield(ctx.tick, ctx.area, buf);
    sparkles(ctx.tick, ctx.area, buf);
    for (flyer, on) in FLYERS.iter().zip(enabled) {
        if *on {
            (flyer.draw)(ctx, buf);
        }
    }
}

// Cheap position hash so the starfield is stable across frames without state.
fn mix(x: u32, y: u32) -> u32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

// --- Ambience: always on. ---

// Two depth layers, both stationary: far stars are dim, near stars bright
// and colorful. Only their brightness animates.
fn starfield(tick: u64, area: Rect, buf: &mut Buffer) {
    star_pass(area, buf, tick, 41, &['·', '.', '˚'], false);
    star_pass(area, buf, tick, 53, &['·', '˚', '✦', '⋆', '✧'], true);
}

fn star_pass(area: Rect, buf: &mut Buffer, tick: u64, density: u32, chars: &[char], near: bool) {
    const CYCLE: u64 = 24;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let h = mix(x as u32, y as u32);
            if !h.is_multiple_of(density) {
                continue;
            }
            let ch = chars[(h / density) as usize % chars.len()];
            let phase = ((h >> 7) as u64) % CYCLE;
            let t = (tick / 2 + phase) % CYCLE;
            let level = CYCLE / 2 - (CYCLE as i64 / 2 - t as i64).unsigned_abs() % (CYCLE / 2 + 1);
            let color = if near {
                near_star_color(level as u32, h)
            } else {
                far_star_color(level as u32)
            };
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch);
                cell.set_fg(color);
            }
        }
    }
}

// Near stars carry color like the poster: violet, amber, and sky-blue mixed
// in with the white ones, muted at low twinkle and vivid at the peak.
fn near_star_color(level: u32, h: u32) -> Color {
    let tint = match h % 23 {
        0..=2 => Some((167, 139, 250)),
        3..=5 => Some((252, 211, 77)),
        6..=8 => Some((125, 211, 252)),
        _ => None,
    };
    match level {
        0..=3 => SLATE_600,
        4..=6 => SLATE_500,
        7..=10 => match tint {
            Some(rgb) => crate::theme::scale(rgb, 0.6),
            None => SLATE_400,
        },
        _ => match tint {
            Some(rgb) => Color::Rgb(rgb.0, rgb.1, rgb.2),
            None => TEXT,
        },
    }
}

fn far_star_color(level: u32) -> Color {
    match level {
        0..=5 => SLATE_700,
        6..=9 => SLATE_600,
        _ => SLATE_500,
    }
}

// Rare big glint stars: a bright center that grows cross-shaped rays at the
// peak of its twinkle.
fn sparkles(tick: u64, area: Rect, buf: &mut Buffer) {
    const CYCLE: u64 = 32;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let h = mix(x as u32 + 131, y as u32 + 977);
            if !h.is_multiple_of(599) {
                continue;
            }
            let phase = ((h >> 7) as u64) % CYCLE;
            let t = (tick / 3 + phase) % CYCLE;
            let level = CYCLE / 2 - (CYCLE as i64 / 2 - t as i64).unsigned_abs() % (CYCLE / 2 + 1);
            let center = match (h >> 11) % 3 {
                0 => VIOLET_LIGHT,
                1 => Color::Rgb(252, 211, 77),
                _ => TEXT,
            };
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char('✦');
                cell.set_fg(if level >= 8 { center } else { SLATE_500 });
            }
            if level >= 12 {
                let arms: [(i32, i32, char); 4] =
                    [(-1, 0, '─'), (1, 0, '─'), (0, -1, '│'), (0, 1, '│')];
                for (dx, dy, ch) in arms {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx >= area.left() as i32
                        && nx < area.right() as i32
                        && ny >= area.top() as i32
                        && ny < area.bottom() as i32
                    {
                        if let Some(cell) = buf.cell_mut((nx as u16, ny as u16)) {
                            cell.set_char(ch);
                            cell.set_fg(SLATE_600);
                        }
                    }
                }
            }
        }
    }
}

// --- Flyers: each one toggleable from config. ---

// Pixel comets: a bright two-pixel head with a smooth tail fading into the
// dark. Three of them ride offset cycles in different directions — shallow
// down-right, down-left, and a steep golden one.
fn comets(ctx: &Ctx, buf: &mut Buffer) {
    let (tick, area) = (ctx.tick, ctx.area);
    const LIFE: u64 = 26;
    const COOL: [(u8, u8, u8); 12] = [
        (240, 249, 255),
        (226, 232, 240),
        (186, 230, 253),
        (125, 211, 252),
        (125, 211, 252),
        (96, 165, 250),
        (96, 165, 250),
        (71, 110, 180),
        (71, 110, 180),
        (51, 65, 85),
        (51, 65, 85),
        (30, 41, 59),
    ];
    const WARM: [(u8, u8, u8); 12] = [
        (255, 251, 235),
        (254, 243, 199),
        (253, 230, 138),
        (252, 211, 77),
        (251, 191, 36),
        (245, 158, 11),
        (217, 119, 6),
        (180, 83, 9),
        (146, 64, 14),
        (120, 53, 15),
        (69, 26, 3),
        (41, 37, 36),
    ];
    // (cycle, seed, dx, dy, warm): the sign of dx picks the travel direction.
    const COMETS: [(u64, u32, i32, i32, bool); 3] = [
        (180, 0x9e37, 2, 1, false),
        (250, 0x51ed, -2, 1, false),
        (320, 0x3c6f, 1, 2, true),
    ];
    if area.width < 40 || area.height < 12 {
        return;
    }
    for (cycle, seed, dx, dy, warm) in COMETS {
        let t = tick % cycle;
        if t >= LIFE {
            continue;
        }
        let h = mix((tick / cycle) as u32, seed);
        let half_w = (area.width as u32 / 2).max(1);
        let sx = if dx > 0 {
            area.x as i32 + (h % half_w) as i32
        } else {
            area.right() as i32 - (h % half_w) as i32
        };
        let sy_px = area.y as i32 * 2 + ((h >> 9) % (area.height as u32 / 2).max(1)) as i32;
        let head = (sx + t as i32 * dx, sy_px + t as i32 * dy);
        let (tx, ty) = (dx.signum(), dy.signum());
        let tail = if warm { &WARM } else { &COOL };
        pixel::set_px(head.0, head.1 - 1, (240, 249, 255), area, buf);
        for (k, rgb) in tail.iter().enumerate() {
            pixel::set_px(
                head.0 - k as i32 * tx,
                head.1 - k as i32 * ty,
                *rgb,
                area,
                buf,
            );
        }
    }
}

// Shooting stars: brief silver streaks that dart across the sky and are gone
// — a blink, not a passage like the comets. Each rides its own prime-length
// cycle with a hashed start position, so they stay rare, land somewhere new
// every pass, and never bunch up.
fn shooting_stars(ctx: &Ctx, buf: &mut Buffer) {
    let (tick, area) = (ctx.tick, ctx.area);
    if area.width < 30 || area.height < 8 {
        return;
    }
    const LIFE: u64 = 11;
    // Head to tail: white-hot cooling through slate into the dark.
    const TRAIL: [(u8, u8, u8); 6] = [
        (248, 250, 252),
        (226, 232, 240),
        (148, 163, 184),
        (100, 116, 139),
        (71, 85, 105),
        (51, 65, 85),
    ];
    // (cycle, seed, dx): dx sets speed and direction; each star also drops
    // one pixel row per tick, for a shallow slanting fall.
    const STARS: [(u64, u32, i32); 3] = [(97, 0x7a21, 3), (149, 0x2fd3, -3), (233, 0xc489, 4)];
    for (cycle, seed, dx) in STARS {
        let t = tick % cycle;
        if t >= LIFE {
            continue;
        }
        let h = mix((tick / cycle) as u32, seed);
        let half_w = (area.width as u32 / 2).max(1);
        let sx = if dx > 0 {
            area.x as i32 + (h % half_w) as i32
        } else {
            area.right() as i32 - (h % half_w) as i32
        };
        // Start in the upper half of the sky (pixel rows), falling from there.
        let sy = area.y as i32 * 2 + ((h >> 9) % area.height as u32) as i32;
        let head = (sx + t as i32 * dx, sy + t as i32);
        for (k, rgb) in TRAIL.iter().enumerate() {
            let k = k as i32;
            pixel::set_px(head.0 - k * dx.signum(), head.1 - k / 3, *rgb, area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn resolve_defaults_on_and_honors_toggles() {
        let mut toggles = HashMap::new();
        assert!(resolve(&toggles).iter().all(|on| *on));
        toggles.insert("comets".into(), false);
        let on = resolve(&toggles);
        assert_eq!(on.len(), FLYERS.len());
        assert!(!on[0]);
        assert!(on[1..].iter().all(|on| *on));
    }

    #[test]
    fn every_size_and_tick_renders_without_panic() {
        // Covers the guards: degenerate areas, just under and over each
        // flyer's minimum, and ticks spanning every flyer's live window.
        let all_on = vec![true; FLYERS.len()];
        for (w, h) in [
            (0, 0),
            (1, 1),
            (29, 7),
            (30, 8),
            (39, 11),
            (40, 12),
            (120, 40),
        ] {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            for tick in (0..400).step_by(3) {
                render(&Ctx { tick, area }, &all_on, &mut buf);
            }
        }
    }
}
