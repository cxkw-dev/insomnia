//! Everything that lives in the sky: the twinkling starfield, the glint
//! stars, and a registry of pixel-art flyers that cross the screen.
//!
//! # Adding your own flyer
//!
//! 1. Draw a sprite in `art.rs` — rows of chars, one char per pixel, mapped
//!    to colors by a palette slice (`.` and space are transparent).
//! 2. Write a `fn my_ship(ctx: &Ctx, buf: &mut Buffer)` here that positions
//!    it from `ctx.tick` and calls `pixel::draw_px_art`.
//! 3. Add one `Flyer` entry to [`FLYERS`]. Done — it flies, and users can
//!    toggle it in their config with `[sky] my_ship = false`.
//!
//! Flyers on `Layer::Behind` render behind the title and cards; `Layer::Front`
//! renders in front of everything. Position off `ctx`: `card_band` is a cell
//! row at card height, `low_lane` a cell row just below the hint line.

use ratatui::{buffer::Buffer, layout::Rect};

use crate::art;
use crate::pixel;
use crate::theme::{SLATE_400, SLATE_500, SLATE_600, SLATE_700, TEXT, VIOLET_LIGHT};
use ratatui::style::Color;

/// Everything a flyer needs to place itself for the current frame.
pub struct Ctx {
    pub tick: u64,
    pub area: Rect,
    /// A cell row at card height — flyers here pass behind the cards.
    pub card_band: i32,
    /// A cell row just below the hint line — open sky on most layouts.
    pub low_lane: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Drawn before the title and cards: the flyer passes behind them.
    Behind,
    /// Drawn after everything: the flyer passes in front, up close.
    Front,
}

pub struct Flyer {
    /// The `[sky]` config key that toggles this flyer.
    pub name: &'static str,
    pub layer: Layer,
    pub draw: fn(&Ctx, &mut Buffer),
}

/// Every flyer in the sky, in draw order. This is the one list to extend.
pub const FLYERS: &[Flyer] = &[
    Flyer {
        name: "mothership",
        layer: Layer::Behind,
        draw: mothership,
    },
    Flyer {
        name: "comets",
        layer: Layer::Behind,
        draw: comets,
    },
    Flyer {
        name: "ufo",
        layer: Layer::Behind,
        draw: ufo,
    },
    Flyer {
        name: "scouts",
        layer: Layer::Behind,
        draw: scouts,
    },
    Flyer {
        name: "raider",
        layer: Layer::Behind,
        draw: raider,
    },
    Flyer {
        name: "whale",
        layer: Layer::Behind,
        draw: whale,
    },
    Flyer {
        name: "rocket",
        layer: Layer::Front,
        draw: rocket,
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

/// Draw the ambience and every enabled behind-layer flyer.
pub fn render_behind(ctx: &Ctx, enabled: &[bool], buf: &mut Buffer) {
    starfield(ctx.tick, ctx.area, buf);
    sparkles(ctx.tick, ctx.area, buf);
    for (flyer, on) in FLYERS.iter().zip(enabled) {
        if *on && flyer.layer == Layer::Behind {
            (flyer.draw)(ctx, buf);
        }
    }
}

/// Draw every enabled front-layer flyer (over the cards and hint).
pub fn render_front(ctx: &Ctx, enabled: &[bool], buf: &mut Buffer) {
    for (flyer, on) in FLYERS.iter().zip(enabled) {
        if *on && flyer.layer == Layer::Front {
            (flyer.draw)(ctx, buf);
        }
    }
}

/// Draw a single flyer by name if it's enabled — used by the compact view.
pub fn draw_one(name: &str, ctx: &Ctx, enabled: &[bool], buf: &mut Buffer) {
    for (flyer, on) in FLYERS.iter().zip(enabled) {
        if *on && flyer.name == name {
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

// The saucer drifts right-to-left near the top, passing behind the title,
// marquee lights rotating, its tractor beam flickering on and off.
fn ufo(ctx: &Ctx, buf: &mut Buffer) {
    let (tick, area) = (ctx.tick, ctx.area);
    if area.width < 30 {
        return;
    }
    let period = area.width as u64 + 50;
    let pos = period - (tick / 4) % period;
    let x = area.x as i32 + pos as i32 - 25;
    let y = area.y as i32 + 1 + ((tick / 16) % 2) as i32;
    let rows: &[&str] = if (tick / 24) % 3 == 0 {
        &art::UFO_PX[..6]
    } else {
        &art::UFO_PX
    };
    let map = if (tick / 8) % 2 == 0 {
        art::UFO_MAP_A
    } else {
        art::UFO_MAP_B
    };
    pixel::draw_px_art(rows, map, x, y, 0.7, area, buf);
}

// A vee of pink scout saucers sweeps left-to-right along the top; every few
// passes the leader rakes a dashed scanning beam over whatever lies below.
fn scouts(ctx: &Ctx, buf: &mut Buffer) {
    let (tick, area) = (ctx.tick, ctx.area);
    if area.width < 40 {
        return;
    }
    let period = area.width as u64 + 90;
    let x = area.x as i32 + ((tick / 3 + 57) % period) as i32 - 30;
    let y = area.y as i32 + 2;
    for (dx, dy, phase) in [(0, 0, 0u64), (-11, 1, 8), (11, 1, 16)] {
        let bob = ((tick / 12 + phase) % 2) as i32;
        pixel::draw_px_art(
            &art::SCOUT_PX,
            art::SCOUT_MAP,
            x + dx,
            y + dy + bob,
            0.8,
            area,
            buf,
        );
    }
    // The scanning beam: dashed pink, the gaps flowing downward as it sweeps.
    if (tick / 48) % 4 == 0 {
        let bx = x + 3;
        let py0 = y * 2 + 8;
        for k in 0..12i32 {
            if (k + (tick / 2) as i32) % 3 != 0 {
                let fade = 1.0 - k as f32 * 0.07;
                pixel::set_px(bx, py0 + k, pixel::dim((244, 114, 182), fade), area, buf);
                pixel::set_px(
                    bx + 1,
                    py0 + k,
                    pixel::dim((244, 114, 182), fade * 0.8),
                    area,
                    buf,
                );
            }
        }
    }
}

// Alien raider: crosses right-to-left along the low lane, loosing twin
// crimson laser bolts that streak ahead of it every few seconds. Skipped
// entirely when the layout leaves it no sky to fly in.
fn raider(ctx: &Ctx, buf: &mut Buffer) {
    let (tick, area, y_cell) = (ctx.tick, ctx.area, ctx.low_lane);
    if area.width < 50 || y_cell + 4 > area.bottom() as i32 {
        return;
    }
    let period = area.width as u64 + 80;
    let pos = period - (tick / 3) % period;
    let x = area.x as i32 + pos as i32 - 40;
    let y = y_cell + ((tick / 14) % 2) as i32;
    let (rows, map) = if (tick / 2) % 2 == 0 {
        (&art::RAIDER_PX_A, art::RAIDER_MAP_A)
    } else {
        (&art::RAIDER_PX_B, art::RAIDER_MAP_B)
    };
    pixel::draw_px_art(rows, map, x, y, 0.95, area, buf);

    // Bolts spawn at the nose and outrun the ship at 3px a tick, white-hot at
    // the head and cooling to ember through the tail. Fire only while the
    // nose is actually on screen — no shots from beyond the void.
    const FIRE_CYCLE: u64 = 96;
    const BOLT_LIFE: u64 = 26;
    let ft = tick % FIRE_CYCLE;
    if ft < BOLT_LIFE && x < area.right() as i32 {
        let bx = x - 2 - ft as i32 * 3;
        let py = y * 2 + 2;
        const BOLT: [(u8, u8, u8); 5] = [
            (254, 242, 242),
            (252, 165, 165),
            (248, 113, 113),
            (220, 38, 38),
            (127, 29, 29),
        ];
        for (k, rgb) in BOLT.iter().enumerate() {
            pixel::set_px(bx + k as i32, py, *rgb, area, buf);
            pixel::set_px(bx + k as i32, py + 1, *rgb, area, buf);
        }
    }
}

// The docker whale freighter drifts slowly to the right at card height,
// ferrying its containers behind the dashboards and out the other side.
fn whale(ctx: &Ctx, buf: &mut Buffer) {
    let (tick, area, y_cell) = (ctx.tick, ctx.area, ctx.card_band);
    if area.width < 40 {
        return;
    }
    let period = area.width as u64 + 70;
    let x = area.x as i32 + ((tick / 5) % period) as i32 - 30;
    let y = y_cell + ((tick / 18) % 2) as i32;
    // Slow tail flap (shape), fast ion-trail shimmer (palette).
    let rows: &[&str] = if (tick / 24) % 2 == 0 {
        &art::WHALE_PX_A
    } else {
        &art::WHALE_PX_B
    };
    let map = if (tick / 4) % 2 == 0 {
        art::WHALE_MAP_A
    } else {
        art::WHALE_MAP_B
    };
    pixel::draw_px_art(rows, map, x, y, 0.9, area, buf);
}

// Drawn last: the rocket flies in front of everything, close and fast.
fn rocket(ctx: &Ctx, buf: &mut Buffer) {
    let (tick, area) = (ctx.tick, ctx.area);
    let y = area.bottom().saturating_sub(3);
    if area.width < 30 || y >= area.bottom() {
        return;
    }
    let period = area.width as u64 + 60;
    let x0 = area.x as i32 + ((tick / 2) % period) as i32 - 30;
    let map = if tick % 2 == 0 {
        art::ROCKET_MAP_A
    } else {
        art::ROCKET_MAP_B
    };
    pixel::draw_px_art(&art::ROCKET_PX, map, x0, y as i32, 1.0, area, buf);
}

// The mothership: right-to-left along the very top, so slow it barely moves,
// and on so long a cycle that most passes of the sky never see it. Running
// lights ripple green down the spine while it's here.
fn mothership(ctx: &Ctx, buf: &mut Buffer) {
    let (tick, area) = (ctx.tick, ctx.area);
    if area.width < 60 {
        return;
    }
    let period = area.width as u64 * 3 + 200;
    let pos = period - (tick / 7) % period;
    let x = area.x as i32 + pos as i32 - 30;
    if x > area.right() as i32 {
        return;
    }
    let y = area.y as i32;
    let map = if (tick / 10) % 2 == 0 {
        art::MOTHERSHIP_MAP_A
    } else {
        art::MOTHERSHIP_MAP_B
    };
    pixel::draw_px_art(&art::MOTHERSHIP_PX, map, x, y, 0.55, area, buf);
}
