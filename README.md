# ☾ insomnia

Keep your Mac awake, beautifully. insomnia is a `caffeinate` replacement that
holds IOKit power assertions directly — no sudo and no helper `caffeinate`
process — wrapped in a night-sky TUI. The kernel lets go the instant insomnia
exits. While it runs, a twinkling starfield surrounds Night Watch, a live awake
timer and countdown, and a small dashboard you can teach new tricks.

**macOS only.** insomnia talks to IOKit directly — that's the whole point,
and it doesn't pretend to run anywhere else.

```text
·                              ✦
                                              ☾  I N S O M N I A

  ╭────────────────────────────────────────────────────────────────────────────────────────────────────────╮
  │                                                                                                        │
  │   NIGHT WATCH                                                                                ● AWAKE   │
  │                                                                                                        │
  │   01:17:43 REMAINING                                                                00:42:17 ELAPSED   │
  │   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━◆────────────────────────────────────────────────────────────  35%   │
  │                                                                                                        │
  │   DISPLAY + SYSTEM HELD                                                       RELEASES AUTOMATICALLY   │
  │   AC POWER · BATTERY 100%                                                                              │
  │                                                                                                        │
  │   ──────────────────────────────────────────────────────────────────────────────────────────────────   │
  │                                                                                                        │
  │   AI USAGE                                                                          4 NEED ATTENTION   │
  │                                                                                                        │
  │   CLAUDE                                                                                               │
  │     WORK ACCOUNT        ENTERPRISE · $25 LEFT · ACTIVE                                                 │
  │       ╰ MONTHLY SPEND   ████████████████████▉░░░  87%                                                  │
  │                         $175 / $200                                                                    │
  │     PERSONAL ACCOUNT    42% USED                                                                       │
  │       ╰ 5H WINDOW       ██▏░░░░░░░░░░░░░░░░░░░░░   9%                                                  │
  │                         RESET IN 2H · TODAY 2:30 PM                                                    │
  │       ╰ WEEKLY          ██████████▏░░░░░░░░░░░░░  42%                                                  │
  │                         RESET IN 3D · SUN 3:49 AM                                                      │
  │       ╰ FABLE WEEKLY    ████████████████████████ 100%                                                  │
  │                         RESET IN 2D · SAT 11:59 AM                                                     │
  │                                                                                                        │
  │   CODEX                                                                                                │
  │   › PERSONAL ACCOUNT    PRO · 96% USED · ACTIVE                                                        │
  │       ╰ WEEKLY          ███████████████████████░  96%                                                  │
  │                         RESET IN 4D · MON 9:00 AM                                                      │
  │     WORK ACCOUNT        BUSINESS · 95% USED · STALE 1D                                                 │
  │       ╰ MONTHLY CREDITS ██████████████████████▊░  95%  STALE                                           │
  │                         BILLING IN 1D · FRI 7:00 PM                                                    │
  │                                                                                                        │
  │   COPILOT                                                                                              │
  │     EXAMPLE TEAM        BIZ · UNLIMITED                                                                │
  │                                                                                                        │
  │   1–25 OF 33 · PGUP/PGDN SCROLL                                                                        │
  │                                                                                                        │
  ╰────────────────────────────────────────────────────────────────────────────────────────────────────────╯

                           ↑↓ NAVIGATE   PGUP/PGDN SCROLL   M MOTION:ON   Q RELEASE
                            .
```

That's one frame, rendered headlessly in monochrome — the real thing is in
color, with a sparse, slowly moving sky around a steady dashboard.

The AI usage section in this preview uses synthetic data from a custom card;
the card is not bundled, and insomnia does not read provider credentials. Docker is
the only built-in dashboard integration. Everything else uses the documented
custom-card protocol below.

## Install

Requires [Rust](https://rustup.rs) and a Mac.

```sh
cargo install --path .
insomnia
```

Already installed from this checkout? Rebuild the copy on your `PATH`, quit the
running app with `q`, and launch it again:

```sh
make install
insomnia
```

Quit with `q`, `esc`, or `ctrl-c` — the assertions release instantly and your
Mac goes back to sleeping on its own schedule.

### Menu bar app

The native macOS app lives in [`macos/`](macos/). It keeps the CLI intact and
reads the same `~/.config/insomnia/config.toml` cards through a bundled Rust
helper. The popover groups AI accounts into **Business AI** and **Personal**,
with `cxkw.dev` accounts under Personal. Each provider's mark and quota meters
use its brand color, with reset details below. It refreshes every 30 seconds
and has a manual refresh button.

Build it with the macOS Command Line Tools and open the resulting app:

```sh
make macos
open dist/Insomnia.app
```

Click the moon in the menu bar to see usage and use the switch beside the
Insomnia wordmark to keep the display awake. The switch starts off when the
app opens. While on, the app holds a macOS display sleep assertion; turning it
off or quitting releases the assertion. The crescent turns amber and glows
while awake. Closing the popover leaves the app running in the menu bar.
The app does not install itself
or start at login.

The popover takes its high-contrast typography, black canvas, thin dividers,
and restrained glow from [Vercel's homepage](https://vercel.com/home). It uses
Geist type, thin quota meters, and compact provider marks.
The marks and their sources are documented in [`macos/Icons/`](macos/Icons/).

The app uses your existing card commands, including the local `ai usage` card
if you have configured one. If no cards are configured, add a `[[card]]` block
as described below. Credentials stay with the local commands; the app reads
their rendered rows.

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

The default layout is one quiet frame. `night watch` stays anchored above the
dashboard, with remaining time first, elapsed time beside it, and power details
underneath. Without a duration, it shows elapsed time and `until you release`.
The wordmark, labels, and displayed card text use uppercase; commands and source
data are unchanged.

Every provider has its own color, taken from the card's existing header and
meter accents. Accounts, all quota windows, reset times, and container statuses
are displayed immediately. Reset and billing details appear beneath each usage
meter, with uppercase days and 12-hour AM/PM times. Percentages use amber and red for high usage;
provider labels and gauges keep their identifying colors. Source-marked stale
data keeps its bar and carries an explicit stale label. A 100% value fills
the entire gauge. Account values, usage bars, reset details, and container
indicators share one aligned column. Usage bars stay compact at up to 24 cells,
with equal space reserved for percentages and stale status.

Docker containers use subtle single-character indicators: a softly breathing
muted green `●` for running, amber `Ⅱ` for paused, a dim `○` for stopped,
a softly breathing amber `◌` for restarting, and a red `×` for errors. Uptime,
health-check results, and exit details remain
visible. The animation represents the last polled state, not CPU activity.

| key | action |
|-----|--------|
| `↑` / `↓`, `k` / `j` | select a row |
| `page up` / `page down` | scroll the dashboard |
| `m` | toggle sky and container animation; data and timers keep updating |
| `q`, `esc`, `ctrl-c` | release and quit |

Selection follows the same source row across refreshes. Narrow terminals wrap
quota details; short terminals scroll the dashboard while keeping the session
above it. A truly tiny terminal uses compact mode.

**Docker is built in.** Every container stays visible, including stopped ones.
Rows show the state (running, stopped, paused, or restarting), uptime or exit
details, and health-check results when available. Cleanly stopped containers
are quiet; failures and unhealthy containers need attention. When the daemon
is unavailable, the section is absent. To turn it off:

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
| `glyph` | no | `◆` | title glyph in the optional separate-card layout |
| `accent` | no | `violet` | separate-card accent: `violet` `pink` `sky` `blue` `emerald` `gold` `red` `slate` `coral` `teal` |
| `side` | no | `right` | `left` places separate cards left of the session when space permits |

The row protocol: **each line of stdout is one source row.** Plain text remains
readable as uppercase text. `ok|name|detail`, `warn|name|detail`,
`bad|name|detail`, and `off|name|detail` describe healthy, warning, failed, and
dormant items. The detail is optional. `hdr|accent|text` starts a group, such
as a provider or environment. `bar|accent|label|percent|detail` supplies a
0–100 utilization meter; consecutive meters belong to the preceding health
row. Its optional detail is useful for reset times.

The focused layout shows every meter and detail immediately. Standalone meters
and plain-text cards also work. Provider labels and gauges use their configured
accents; amber and red values mark attention. A command contributes at most 32
source rows and its card only appears when the command succeeds with output.

Card commands are trusted local code and run through `sh -c`. Keep tokens and
secrets out of the TOML file; prefer authenticated CLIs, Keychain-backed tools,
or environment variables that are already available to the command.

The focused layout is the default (`combined = true`). The previous separate
cards remain available with `combined = false`: they sit beside the session
on wide terminals, stack below it on narrow ones, and fold when space runs out.
Row navigation and scrolling belong to the focused layout.

```toml
[dashboard]
combined = false # optional separate-card layout
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
enabled = true    # the default; false retires the docker card

[sky]
comets = false    # a calmer night — shooting stars only

[[card]]
title = "pull requests"
glyph = "⇄"
accent = "sky"
interval = 120
command = 'gh search prs --state=open --review-requested=@me --json title --jq ".[].title"'
```

No config file is needed at all — everything above is optional.

## The sky

Sparse stars and occasional flyers stay around the edges of the dashboard.
Status indicators and the wordmark remain steady. Press `m` to freeze the sky,
or start with motion off in your config:

```toml
[sky]
motion = false
```

The individual flyer switches still apply when motion is on:

| flyer | what it does |
|-------|--------------|
| `comets` | three of them — two ice-blue, one golden — long smooth tails on long offset orbits |
| `shooting_stars` | brief silver streaks that dart across and are gone; each pass starts somewhere new |

Ground either from the `[sky]` table:

```toml
[sky]
shooting_stars = false
```

Unlisted flyers stay on, and a misspelled name gets an error that lists what
the sky holds.

## Add your own flyer

The sky is a registry, and it's built to be extended — the full guide lives
in a doc comment at the top of [`src/sky.rs`](src/sky.rs). Two steps:

1. **Write a draw function** in `sky.rs` that positions itself from
   `ctx.tick` and draws with `pixel::set_px` — one call per pixel, half-block
   resolution, out-of-bounds pixels clipped for free.
2. **Add one `Flyer` entry** to the `FLYERS` list.

That's it — it flies, and the `[sky]` config toggle for it appears
automatically. Pull requests are very welcome. The sky has room.

## How it works

insomnia calls the same IOKit API `caffeinate` uses —
`IOPMAssertionCreateWithName` — to hold assertions like
`PreventUserIdleDisplaySleep` and `PreventUserIdleSystemSleep` directly from
the process. No sudo and no separate keep-awake process. The kernel ties
assertions to their process, so they release the moment insomnia exits,
however it exits — even SIGKILL can't leave your display pinned awake. See for
yourself while it's running:

```sh
pmset -g assertions | grep insomnia
```

Everything slow — `pmset`, `docker`, your card commands — runs on background
threads, one per source, streaming updates to the renderer. A hanging command
only ever delays its own card; the animation never stutters.

## Development

```sh
make run       # the real thing, from the working tree
make check     # fmt + clippy + tests
make preview   # one headless frame (cargo run --example preview <w> <h> <tick>)
make install   # rebuild and replace the installed binary

# inspect the actual terminal cells in color using fixture data
cargo run --quiet --example preview 110 46 7 --html > preview.html
```

The `insomnia` on your PATH is the installed copy — code changes don't reach
it until `make install` (or `cargo install --path .`) rebuilds it.

## License

[MIT](LICENSE)
