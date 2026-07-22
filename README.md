# ☾ insomnia

Keep your Mac awake, beautifully. insomnia is a `caffeinate` replacement that
holds IOKit power assertions directly — no sudo, no child processes, and the
kernel lets go the instant it exits — wrapped in a night-sky TUI: a twinkling
starfield, pixel-art ships crossing on their own orbits, a live status card
with an awake timer and countdown, and a small dashboard you can teach new
tricks.

**macOS only.** insomnia talks to IOKit directly — that's the whole point,
and it doesn't pretend to run anywhere else.

```text
·                    ˚  ✦                               ▀▄                                             ▄▀
                 ·      ⋆                                 ▀▄                                         ▄▀
         ˚           · .     ▄▄▀▀▄▄                         ▀▄˚  ⋆              ·        ·     ✦   ▄▀
       ✧            ▄▄▀▀▄▄    ▀▀▀▀ ⋆  ▄▄▀▀▄▄                  ▀▄                                 ▄▀
        .            ▀▀▀██╗███╗   ██╗███████╗.██████╗ ███╗  ✧███╗███╗   ██╗██╗ █████╗        ˚▄▄▀        ˚
       ✦        ✧    ˚  ██║████╗▄ ██║██╔════╝██╔═══██╗████╗ ████║████╗  ██║██║██╔══██╗        ▀    .
              ▄         ██║██╔██╗ ██║███████╗██║   ██║██╔████╔██║██╔██╗ ██║██║███████║           .      .˚
               ▀▄       ██║██║╚██╗██║╚════██║██║   ██║██║╚██╔╝██║██║╚██╗██║██║██╔══██║        ·      ✧
   ˚             ▀▄    .██║██║ ╚████║███████║╚██████╔╝██║ ╚═╝ ██║██║ ╚████║██║██║  ██║ ·
      .            ▀▄   ╚═╝╚═╝ ▀╚═══╝╚══════╝ ╚═════╝ ╚═╝     ╚═╝╚═╝  ╚═══╝╚═╝╚═╝  ╚═╝           .    │
                     ▀▄        ▀▀                            .               ⋆                ·      ─✦─
           ╭──────────────────── ☾ ─────────────────────╮  ╭──────────── ⌖ todos · 2 ─────────────╮  ✧│     ˚
           │                                            │  │                                      │
 · ⋆       │   ● display  ● system                      │  │  #42 ship the login flow             │
           │                                            │  │  #57 tidy the release notes          │
           │   awake    00:42:17                        │  │                                      │    ·
      ·    │   until    01:17:43                        │  ╰──────────────────────────────────────╯
         ˚ │   ▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▱▱▱▱▱▱▱▱▱▱▱           │                   ˚                ⋆             ·
           │   power    ac power · 100%                 │  ╭──────────── ≋ docker · 5 ────────────╮
        ✦  │                                            │  │                                      │    .
        ⋆  ╰────────────────────────────────────────────╯  │  ● worker                restarting  │
                                            ✦              │  ● web                   up 3 hours  │
                                                           │  ● postgres              up 3 hours  │         .
            ⋆                          ˚                   │  ● redis                 up 3 hours  │     ✧
                             .                             │  ● builder               up 2 hours  │          ˚
         .                                                 │                                      │         ˚
      ˚        ˚  ˚                               ✦✧       ╰──────────────────────────────────────╯
                            ˚      ✧                         · ·    ˚                                    ·
                 ·     ⋆                      ˚q — let it sleep                                .
                                                             ·                                        ·
  ⋆                                        ·   ⋆           ·
  ·                   ✦       ·                   ˚                                                          .
     .      ·                        ˚           ·                                             ˚
✦  · ✦          ˚                                                .                                 .
```

That's one frame, rendered headlessly in monochrome — the real thing is in
color, and everything up there moves.

## Install

Requires [Rust](https://rustup.rs) and a Mac.

```sh
cargo install --path .
insomnia
```

Quit with `q`, `esc`, or `ctrl-c` — the assertions release instantly and your
Mac goes back to sleeping on its own schedule.

## Usage

```sh
insomnia            # display + system stay awake (same as -d -i)
insomnia -d         # just the display
insomnia -t 2h      # two hours, then let go
insomnia -di -t 90m # flags combine; -t takes the last spot in a group
insomnia -q -t 1h   # no TUI, just hold quietly in the foreground
```

| flag | effect |
|------|--------|
| `-d` | keep the display awake |
| `-i` | keep the system awake while idle |
| `-s` | prevent sleep entirely, even lid-closed (AC power only) |
| `-m` | keep disks from idle-sleeping |
| `-t <time>`, `--for <time>` | stay awake for a duration, then release |
| `-q`, `--quiet` | skip the art, hold quietly |
| `--config <path>` | config file (default `~/.config/insomnia/config.toml`) |
| `-h`, `--help` | help |
| `-V`, `--version` | version |

With no flags, insomnia runs as `-d -i`: display and system stay awake.
Durations read the way you'd say them: a bare number is seconds (matching
`caffeinate -t`), or use units — `300`, `90s`, `45m`, `2h`, `1h30m`.

## The dashboard

The status card is always there: the awake timer, a countdown bar when you
set `-t`, and your power source with battery percentage. Around it, dashboard
cards come and go as their sources do.

**Docker is built in.** When the daemon is up, a `≋ docker` card lists every
container with a pulsing health-colored dot, unhealthy ones sorted to the top.
When docker isn't running, the card simply isn't there. To turn it off for
good:

```toml
[docker]
enabled = false
```

**Custom cards** are `[[card]]` blocks in `~/.config/insomnia/config.toml`
(or `$XDG_CONFIG_HOME/insomnia/config.toml`, or wherever `--config` points).
Each one runs a shell command on a timer and turns its stdout into rows:

| field | required | default | meaning |
|-------|----------|---------|---------|
| `title` | yes | — | the card's name |
| `command` | yes | — | run via `sh -c` every `interval` seconds |
| `interval` | no | `10` | seconds between runs (minimum 2) |
| `glyph` | no | `◆` | a single-width character beside the title |
| `accent` | no | `violet` | `violet` `pink` `sky` `blue` `emerald` `gold` `red` `slate` `coral` `teal` |
| `side` | no | `right` | `left` floats the card to the left of the status card when the terminal fits three columns |

The row protocol: **each line of stdout is one row.** Plain text renders
as-is. Lines shaped `ok|name|detail`, `warn|name|detail`, or `bad|name|detail`
get a pulsing health dot and aligned columns — the detail is optional, and
`off|name|detail` gives a steady slate dot for rows that are present but
dormant. A line shaped `hdr|accent|text` becomes a section header tinted
with that accent, for cards that group rows from several sources. A card
shows at most 32 rows, and only appears at all when its command succeeds and
prints something, so a quiet source costs you nothing.

Layout takes care of itself: cards sail beside the status card on a wide
terminal, dock beneath it on a narrow one, and any card that doesn't fit the
height folds into a one-line summary inside the status card. A truly tiny
terminal gets a single-line compact mode, ufo included.

Prefer one box to many? A `[dashboard]` table with `combined = true` folds
every card — custom and docker alike — into a single box, each as a section
under a header in that card's glyph and accent. `title`, `glyph`, `accent`,
and `side` then describe the combined box itself:

```toml
[dashboard]
combined = true
side = "left"
```

Some cards worth stealing:

```toml
# Today's todos, from a local app's API.
[[card]]
title = "todos"
glyph = "⌖"
accent = "violet"
command = 'curl -sf localhost:3000/api/todos | jq -r ".[].title"'

# Pull requests waiting on you.
[[card]]
title = "pull requests"
glyph = "⇄"
accent = "sky"
interval = 120
command = 'gh search prs --state=open --review-requested=@me --json title --jq ".[].title"'

# A health check, using the row protocol.
[[card]]
title = "services"
glyph = "♥"
accent = "emerald"
interval = 30
command = '''
for url in localhost:3000 localhost:8080; do
  if curl -sf -o /dev/null "$url"; then echo "ok|$url"; else echo "bad|$url|no answer"; fi
done
'''
```

And a complete config, all three sections together:

```toml
# ~/.config/insomnia/config.toml

[docker]
enabled = true    # the default; false grounds the whale's cargo card

[sky]
raider = false    # a calmer night — no laser fire

[[card]]
title = "pull requests"
glyph = "⇄"
accent = "sky"
interval = 120
command = 'gh search prs --state=open --review-requested=@me --json title --jq ".[].title"'
```

No config file is needed at all — everything above is optional.

## The sky

Beyond the always-on starfield and glint stars, six flyers keep their own
schedules:

| flyer | what it does |
|-------|--------------|
| `mothership` | a vast violet carrier, high and slow on so long an orbit that most nights never see it — green running lights ripple down its spine when you do |
| `comets` | three of them — two ice-blue, one golden — streaking on long offset orbits |
| `ufo` | drifts behind the title, tractor beam flickering on and off |
| `scouts` | a vee of pink saucers; every few passes the leader rakes a scanning beam below |
| `raider` | strafes the low sky, loosing twin crimson laser bolts ahead of itself |
| `whale` | the docker whale freighter, slowly ferrying containers behind the cards |
| `rocket` | the only one up close — flies in front of everything, low and fast |

Ground any of them from the `[sky]` table:

```toml
[sky]
raider = false
scouts = false
```

Unlisted flyers stay on, and a misspelled name gets an error that lists what
the sky holds.

## Add your own ship

The sky is a registry, and it's built to be extended — the full guide lives
in a doc comment at the top of [`src/sky.rs`](src/sky.rs). Three steps:

1. **Draw a sprite** in `art.rs` — rows of characters, one per pixel, mapped
   to colors by a palette slice (`.` and space are transparent).
2. **Write a draw function** in `sky.rs` that positions it from `ctx.tick`
   and calls `pixel::draw_px_art`.
3. **Add one `Flyer` entry** to the `FLYERS` list.

That's it — it flies, and the `[sky]` config toggle for it appears
automatically. Pull requests are very welcome. The sky has room.

## How it works

insomnia calls the same IOKit API `caffeinate` uses —
`IOPMAssertionCreateWithName` — to hold assertions like
`PreventUserIdleDisplaySleep` and `PreventUserIdleSystemSleep` directly from
the process. No sudo, no child processes. The kernel ties assertions to their
process, so they release the moment insomnia exits, however it exits — even
SIGKILL can't leave your display pinned awake. See for yourself while it's
running:

```sh
pmset -g assertions | grep insomnia
```

Everything slow — `pmset`, `docker`, your card commands — runs on background
threads, one per source, streaming updates to the renderer. A hanging command
only ever delays its own card; the animation never stutters.

## Development

```sh
cargo run                                # the real thing
cargo test                               # config parsing, durations, row protocol
cargo run --example preview 110 34 7     # one headless frame: <width> <height> <tick>
```

## License

[MIT](LICENSE)
