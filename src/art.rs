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

// Saucer: cyan glass dome with a highlight, a steel disc shaded dark at the
// rim, and a marquee of running lights around the waist — the A/B palettes
// swap amber and cyan so the lights appear to rotate. Rows 0-5 are the ship;
// the dithered tractor-beam cone renders only when it flickers on.
pub const UFO_PX: [&str; 11] = [
    ".....dhhd.....",
    "....dhhhhd....",
    "..ssmmmmmmss..",
    "smyLmyLmyLmyms",
    ".ssmmmmmmmmss.",
    "...ssssssss...",
    "......gg......",
    ".....g.gg.....",
    ".....gg.g.....",
    "....g.gg.gg...",
    "...gg..g..g...",
];
pub const UFO_MAP_A: &[(char, (u8, u8, u8))] = &[
    ('d', (34, 211, 238)),
    ('h', (207, 250, 254)),
    ('m', (148, 163, 184)),
    ('s', (71, 85, 105)),
    ('y', (251, 191, 36)),
    ('L', (22, 78, 99)),
    ('g', (103, 232, 249)),
];
pub const UFO_MAP_B: &[(char, (u8, u8, u8))] = &[
    ('d', (34, 211, 238)),
    ('h', (207, 250, 254)),
    ('m', (148, 163, 184)),
    ('s', (71, 85, 105)),
    ('y', (100, 76, 14)),
    ('L', (103, 232, 249)),
    ('g', (103, 232, 249)),
];

// Alien raider: a swept-wing emerald interceptor flying nose-left — glowing
// cannon tip, red cockpit slit, dark hull with a bright leading edge, slate
// engine pods trailing flame. Frame B stretches the flame a pixel and the
// maps recolor it, so the exhaust both flickers and breathes. It fires
// paired crimson laser bolts from the nose — those are drawn by the UI.
pub const RAIDER_PX_A: [&str; 6] = [
    ".........eee.......",
    "....eehEEEEEee.....",
    "wchhEEEEEEEEEEppff.",
    "wchhEEEEEEEEEEppff.",
    "....eehEEEEEee.....",
    ".........eee.......",
];
pub const RAIDER_PX_B: [&str; 6] = [
    ".........eee.......",
    "....eehEEEEEee.....",
    "wchhEEEEEEEEEEppfff",
    "wchhEEEEEEEEEEppfff",
    "....eehEEEEEee.....",
    ".........eee.......",
];
pub const RAIDER_MAP_A: &[(char, (u8, u8, u8))] = &[
    ('E', (4, 120, 87)),
    ('e', (52, 211, 153)),
    ('h', (167, 243, 208)),
    ('c', (248, 113, 113)),
    ('w', (254, 202, 202)),
    ('p', (51, 65, 85)),
    ('f', (251, 191, 36)),
];
pub const RAIDER_MAP_B: &[(char, (u8, u8, u8))] = &[
    ('E', (4, 120, 87)),
    ('e', (52, 211, 153)),
    ('h', (167, 243, 208)),
    ('c', (248, 113, 113)),
    ('w', (254, 202, 202)),
    ('p', (51, 65, 85)),
    ('f', (96, 165, 250)),
];

// Pink scout saucer: a gold canopy over a two-tone hull — bright pink on
// top, deep magenta below — with a pair of amber landing lights underneath.
// Three fly in a loose vee; the leader rakes a scanning beam (UI-drawn).
pub const SCOUT_PX: [&str; 4] = ["...hh...", ".pphhpp.", "pPPPPPPp", "..y..y.."];
pub const SCOUT_MAP: &[(char, (u8, u8, u8))] = &[
    ('p', (244, 114, 182)),
    ('P', (219, 39, 119)),
    ('h', (253, 224, 71)),
    ('y', (251, 191, 36)),
];

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

// The hero rocket, nose-right: white hull shaded slate along the spine, a
// framed cyan porthole, red tail fins, and a tapering red nose cone. The
// exhaust has a pale-gold core ('x') inside an outer flame ('f') that the
// two maps flip between orange and red for the flicker.
pub const ROCKET_PX: [&str; 6] = [
    "......smmmmmms.....",
    "ff..FFmmWWmmmmn....",
    "fffxFFmWccWmmmmnnn.",
    ".ffxFFmWccWmmmmnnn.",
    "f...FFmmWWmmmmn....",
    "......smmmmmms.....",
];
pub const ROCKET_MAP_A: &[(char, (u8, u8, u8))] = &[
    ('m', (226, 232, 240)),
    ('s', (148, 163, 184)),
    ('W', (248, 250, 252)),
    ('c', (103, 232, 249)),
    ('F', (220, 80, 80)),
    ('n', (239, 68, 68)),
    ('f', (251, 146, 60)),
    ('x', (254, 240, 138)),
];
pub const ROCKET_MAP_B: &[(char, (u8, u8, u8))] = &[
    ('m', (226, 232, 240)),
    ('s', (148, 163, 184)),
    ('W', (248, 250, 252)),
    ('c', (103, 232, 249)),
    ('F', (220, 80, 80)),
    ('n', (239, 68, 68)),
    ('f', (248, 113, 113)),
    ('x', (254, 240, 138)),
];

// The mothership: a vast violet crescent-carrier that crosses only rarely,
// high and slow. A deep hull under a lighter rim, a keel in shadow with
// hanging spires, and a spine of green portholes whose A/B palettes swap
// bright and dim so the lights ripple down the hull.
pub const MOTHERSHIP_PX: [&str; 7] = [
    "..........vvvvvvvv..........",
    ".....vvvvVVVVVVVVvvvv.......",
    "..vvVVVVVVVVVVVVVVVVVvv.....",
    ".vVgGVVgGVVgGVVgGVVgGVVgGVv.",
    "..vVVVVVVVVVVVVVVVVVVVVVv...",
    "....ssssssssssssssssssss....",
    "......s...s....s...s...s....",
];
pub const MOTHERSHIP_MAP_A: &[(char, (u8, u8, u8))] = &[
    ('v', (139, 92, 246)),
    ('V', (91, 33, 182)),
    ('s', (51, 65, 85)),
    ('g', (74, 222, 128)),
    ('G', (22, 101, 52)),
];
pub const MOTHERSHIP_MAP_B: &[(char, (u8, u8, u8))] = &[
    ('v', (139, 92, 246)),
    ('V', (91, 33, 182)),
    ('s', (51, 65, 85)),
    ('g', (22, 101, 52)),
    ('G', (74, 222, 128)),
];
