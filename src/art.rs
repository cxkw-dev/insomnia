// "INSOMNIA" in figlet ANSI Shadow, 62 columns wide.
pub const TITLE: [&str; 6] = [
    "██╗███╗   ██╗███████╗ ██████╗ ███╗   ███╗███╗   ██╗██╗ █████╗ ",
    "██║████╗  ██║██╔════╝██╔═══██╗████╗ ████║████╗  ██║██║██╔══██╗",
    "██║██╔██╗ ██║███████╗██║   ██║██╔████╔██║██╔██╗ ██║██║███████║",
    "██║██║╚██╗██║╚════██║██║   ██║██║╚██╔╝██║██║╚██╗██║██║██╔══██║",
    "██║██║ ╚████║███████║╚██████╔╝██║ ╚═╝ ██║██║ ╚████║██║██║  ██║",
    "╚═╝╚═╝  ╚═══╝╚══════╝ ╚═════╝ ╚═╝     ╚═╝╚═╝  ╚═══╝╚═╝╚═╝  ╚═╝",
];
pub const TITLE_W: u16 = 62;
pub const TITLE_H: u16 = 6;

// --- Pixel sprites: one char = one pixel, two rows per terminal cell. ---
// '.' is transparent; every other char maps through the sprite's palette.

// Saucer with a glass dome, amber running lights, and a dithered tractor
// beam. Rows 0-5 are the ship; the beam rows render only when it flickers on.
pub const UFO_PX: [&str; 10] = [
    "...dddd...",
    "..dhhhhd..",
    ".mmmmmmmm.",
    "mymmyymmym",
    ".mmmmmmmm.",
    "..mmmmmm..",
    "....bb....",
    "...b.bb...",
    "...bb.b...",
    "..b.bb.bb.",
];
pub const UFO_MAP: &[(char, (u8, u8, u8))] = &[
    ('d', (34, 211, 238)),
    ('h', (207, 250, 254)),
    ('m', (148, 163, 184)),
    ('y', (251, 191, 36)),
    ('b', (103, 232, 249)),
];

// Alien raider: an emerald dart flying nose-left with a red cockpit slit and
// twin engine flames ('f' recolors per frame). It fires paired crimson laser
// bolts from the nose — the bolts themselves are drawn by the UI, not here.
pub const RAIDER_PX: [&str; 6] = [
    "....ee",
    "..eehhee",
    "eehhcchheeeeff",
    "eehhcchheeeeff",
    "..eehhee",
    "....ee",
];
pub const RAIDER_MAP_A: &[(char, (u8, u8, u8))] = &[
    ('e', (5, 150, 105)),
    ('h', (110, 231, 183)),
    ('c', (248, 113, 113)),
    ('f', (251, 191, 36)),
];
pub const RAIDER_MAP_B: &[(char, (u8, u8, u8))] = &[
    ('e', (5, 150, 105)),
    ('h', (110, 231, 183)),
    ('c', (248, 113, 113)),
    ('f', (96, 165, 250)),
];

// Pink scout saucer: gold canopy lights over a squat hull. Three of them fly
// in a loose vee; the leader rakes a dashed scanning beam below (UI-drawn).
pub const SCOUT_PX: [&str; 3] = ["..hh", "pphhpp", ".pppp"];
pub const SCOUT_MAP: &[(char, (u8, u8, u8))] = &[('p', (244, 114, 182)), ('h', (253, 224, 71))];

// The docker whale, refitted as a cargo freighter: five crate-colored
// containers on a dark deck strip, a shaded back over a light belly, an eye,
// a pectoral fin, and a cyan ion trail ('i') streaming behind the tail. Two
// shape frames flap the tail flukes; frame A also blows a spout of bubbles
// ('o'). Rows can end early — everything past the last char is transparent.
pub const WHALE_PX_A: [&str; 11] = [
    "........yyyy.eeee",
    "........yyyy.eeee",
    "......cccc.vvvv.aaaa.......o",
    "......cccc.vvvv.aaaa......o",
    "DD....DDDDDDDDDDDDDDDDDDDD",
    ".DD..BBBBBBBBBBBBBBBBBBBBBBB",
    "ii.DDBBBBBBBBBBBBBBBBBBBBBWBB",
    "...DBBBBBBBBBBBBBBBBBBBBBBBBB",
    "....LLLLLLLLLLLLLLLLLLLLLLLL",
    "......LLLLLLLLLLLLLDDLLLL",
    "...................DD",
];
pub const WHALE_PX_B: [&str; 11] = [
    "........yyyy.eeee",
    "........yyyy.eeee",
    "......cccc.vvvv.aaaa",
    "......cccc.vvvv.aaaa",
    "......DDDDDDDDDDDDDDDDDDDD",
    "...DDBBBBBBBBBBBBBBBBBBBBBBB",
    "ii.DDBBBBBBBBBBBBBBBBBBBBBWBB",
    ".DDDBBBBBBBBBBBBBBBBBBBBBBBBB",
    "DD..LLLLLLLLLLLLLLLLLLLLLLLL",
    "......LLLLLLLLLLLLLDDLLLL",
    "...................DD",
];
pub const WHALE_MAP_A: &[(char, (u8, u8, u8))] = &[
    ('D', (30, 64, 175)),
    ('B', (37, 112, 244)),
    ('L', (125, 178, 252)),
    ('W', (240, 249, 255)),
    ('c', (56, 189, 248)),
    ('y', (251, 191, 36)),
    ('e', (52, 211, 153)),
    ('v', (192, 132, 252)),
    ('a', (251, 113, 133)),
    ('i', (103, 232, 249)),
    ('o', (186, 230, 253)),
];
pub const WHALE_MAP_B: &[(char, (u8, u8, u8))] = &[
    ('D', (30, 64, 175)),
    ('B', (37, 112, 244)),
    ('L', (125, 178, 252)),
    ('W', (240, 249, 255)),
    ('c', (56, 189, 248)),
    ('y', (251, 191, 36)),
    ('e', (52, 211, 153)),
    ('v', (192, 132, 252)),
    ('a', (251, 113, 133)),
    ('i', (56, 189, 248)),
    ('o', (125, 211, 252)),
];

// Side-view ship: red fin stripe, cyan cockpit, exhaust flame ('f' recolors
// per frame for the flicker).
pub const ROCKET_PX: [&str; 4] = [
    "...Fmmmmmm..",
    "ff.Fmmccmmnn",
    ".ffFmmccmmnn",
    "...Fmmmmmm..",
];
pub const ROCKET_MAP_A: &[(char, (u8, u8, u8))] = &[
    ('m', (226, 232, 240)),
    ('c', (103, 232, 249)),
    ('F', (220, 80, 80)),
    ('n', (239, 68, 68)),
    ('f', (251, 146, 60)),
];
pub const ROCKET_MAP_B: &[(char, (u8, u8, u8))] = &[
    ('m', (226, 232, 240)),
    ('c', (103, 232, 249)),
    ('F', (220, 80, 80)),
    ('n', (239, 68, 68)),
    ('f', (248, 113, 113)),
];
