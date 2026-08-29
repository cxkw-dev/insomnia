# ☾ insomnia

Keep your Mac awake, beautifully. insomnia is a `caffeinate` replacement that
holds IOKit power assertions directly — no sudo and no helper `caffeinate`
process — wrapped in a night-sky TUI. The kernel lets go the instant insomnia
exits. While it runs, a twinkling starfield surrounds Night Watch, a live awake
timer and countdown, and a small dashboard you can teach new tricks.

**macOS only.** insomnia talks to IOKit directly — that's the whole point,
and it doesn't pretend to run anywhere else.

```text
·                    ˚  ✦                                                                               ˚
                 ·      ⋆                             ▀▄                                                ▄▀
         ˚           · .      ✦                         ▀▄    ˚  ⋆              ·        ·     ✦      ▄▀
       ✧                           ⋆                      ▀▄                                        ▄▀
        .                            ✧  ✦    .☾  I N S O M N I A                             ˚    ▄▀     ˚
       ✦      ▀▄✧    ˚        .                              ✦▀▄                                ▄▀ .
             ╭ ● awake ─────────────────────────────────────────────────────────────────────────╮.      .˚
             │                                                                                  │    ✧
   ˚         │   NIGHT WATCH                                              ● display  ● system   │
      .      │                                                                                  │.    │
             │   00:42:17 awake                                            01:17:43 remaining   │    ─✦─
             │   ━━━━━━━━━━━━━━━━━━━━━━━━━━◆───────────────────────────────────────────────  35%│    ✧│     ˚
             │   ↯ ac power · 100%                                         auto release armed   │˚
 · ⋆         │                                                                                  │
             │  ✦ ai usage                                                                      │
             │  ─ ✻ claude ──────────────────────────────────────────────────────────────────── │      ·
      ·      │  ● ▸ andy.nguyen      enterprise · $25.65 left · active                          │
         ˚   │    ╰ monthly spend   ████████████▏░  87%  $174.35 / $200                         │          ·
             │  ●   cxkw.dev         42% used                                                   │
        ✦    │    ├ 5h window       █▎░░░░░░░░░░░░   9%  resets today 14:30                     │      .
        ⋆    │    ├ weekly          █████▉░░░░░░░░  42%  resets Sun 03:49                       │
             │    ╰ Fable weekly    ██████████████ 100%  resets Sat 11:59                       │
             │                                                                                  │           .
            ⋆│  ─ ◎ openai codex ────────────────────────────────────────────────────────────── │       ✧
             │  ● ▸ cxkw.dev         pro · 96% used · active                                    │            ˚
         .   │    ╰ weekly          █████████████▌  96%  resets Mon 09:00                       │           ˚
      ˚      │  ●   andy.nguyen      business · 95% used · stale 1d                             │
             │    ╰ monthly credits █████████████▎  95%  resets Fri 19:00                       │        ·
             │                                                                                  │
             │  ─ ⧉ copilot ─────────────────────────────────────────────────────────────────── │     ·
  ⋆          │  ● ▸ andy-nguyen-cxkw biz · unlimited                                            │
  ·          │                                                                                  │            .
     .      ·│  ≋ docker                                                                        │
✦  · ✦       │  ● worker             restarting                                                 │  .
 .   .       │  ● web                up 3 hours                                                 │ ˚˚
             │  ● postgres           up 3 hours                                                 │
             │  ● redis              up 3 hours                                                 │       ˚
            ⋆│  ● builder            up 2 hours                                                 │      ·
         ✧   │                                                                                  │      .     ˚
             ╰──────────────────────────────────────────────────────────────────────────────────╯✧
      ·   ·                           ✦                                                  ✦˚  ✦   ˚         ˚
˚   ·         ˚                ˚˚           q release & let it sleep                       ⋆              .
            .   . ·             ·              ✦             .
                         ˚  ˚     ⋆             ·         ✦
                            ˚           ·    ✦                   ✧ ·
                ˚·       ·                ˚ ˚                 ✦                                ⋆  ✦
```

That's one frame, rendered headlessly in monochrome — the real thing is in
color, and everything up there moves.

The AI usage section in this preview demonstrates a local custom card; it is
not bundled and insomnia does not read provider credentials itself. Docker is
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

Night Watch is always there. It shows which power assertions are active, how
long insomnia has held them, and the current power source and battery level.
Give insomnia a duration with `-t` and it adds the remaining time, a progress
timeline, and an `auto release armed` state. Without a duration, the timeline
runs indefinitely until you release it with `q`.

Around Night Watch, dashboard cards come and go as their sources do.

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
with that accent and a horizontal divider, preceded by a blank line so each
group reads as its own block. This is useful for giving each AI provider or
environment a distinct visual lane inside one card.
`bar|accent|label|percent|detail` renders a colored utilization meter from
0–100 as a compact, high-resolution gauge; consecutive meters are joined to
the account above them by a small branch rail. The detail is optional and is
useful for reset times. A card shows at most 32 rows, and only appears at all
when its command succeeds and prints something, so a quiet source costs you
nothing.

Card commands are trusted local code and run through `sh -c`. Keep tokens and
secrets out of the TOML file; prefer authenticated CLIs, Keychain-backed tools,
or environment variables that are already available to the command.

Layout takes care of itself: cards sail beside the status card on a wide
terminal, dock beneath it on a narrow one, and any card that doesn't fit the
height folds into a one-line summary inside the status card. A truly tiny
terminal gets a single-line compact mode, starfield included.

Prefer one box to many? A `[dashboard]` table with `combined = true` folds
every card — custom and docker alike — into the status box itself, each as
a section under a header in that card's glyph and accent, so the whole app
is a single box that expands responsively on wider terminals:

```toml
[dashboard]
combined = true
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

Beyond the always-on starfield and glint stars, two kinds of flyers keep
their own schedules, each occasional enough that a pass always feels like a
small event:

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
```

The `insomnia` on your PATH is the installed copy — code changes don't reach
it until `make install` (or `cargo install --path .`) rebuilds it.

## License

[MIT](LICENSE)
