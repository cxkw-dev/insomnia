<h1 align="center">☾ insomnia</h1>

<p align="center">
  Keep your Mac awake — from the terminal or the menu bar — with a dashboard
  for the things you keep an eye on.
</p>

This repo contains **two separate apps**. Use either one, or both:

<table>
  <tr>
    <th width="50%">Terminal app — <code>insomnia</code></th>
    <th width="50%">Menu bar app — <code>Insomnia.app</code></th>
  </tr>
  <tr>
    <td><img src="docs/terminal.png" alt="The insomnia terminal app: a session timer above an AI usage dashboard in a starfield"></td>
    <td align="center"><img src="docs/menubar.png" alt="The Insomnia menu bar popover: AI accounts grouped into Enterprise and Personal with quota meters" width="300"></td>
  </tr>
  <tr>
    <td>A full-screen terminal UI. Run it when you want the Mac awake; quit
    it and the Mac sleeps normally again. A timed or open-ended session sits
    above a scrollable dashboard, with a night sky around it.</td>
    <td>A moon in the menu bar that's always there. Click it for a popover
    with your dashboard cards and a switch that keeps the display awake.</td>
  </tr>
</table>

|  | Terminal app | Menu bar app |
|--|--------------|--------------|
| **Start it** | run `insomnia` | starts at login; click ☾ |
| **Keeps awake** | display, system, lid-closed, disks — you pick; for a set time or until you quit | display only, while the switch is on |
| **Shows** | your cards, Docker containers, the session timer | your cards, with AI accounts grouped into Enterprise and Personal |
| **Built from** | [`terminal/`](terminal/) (Rust) | [`macos/`](macos/) (Swift + a Rust helper from `terminal/`) |
| **Install** | `make install` | `make install-app` |

**What they share:** one config file, `~/.config/insomnia/config.toml`. Every
`[[card]]` in it — a shell command whose output becomes dashboard rows —
appears in both apps. The optional [AI usage card](#ai-usage-card) is one of
those.

**macOS only.** Both hold IOKit power assertions directly, the same mechanism
as `caffeinate`, with no sudo and no helper process.

## Install

You need a Mac, [Rust](https://rustup.rs), and the Command Line Tools
(`xcode-select --install`, needed for the menu bar app).

```sh
git clone https://github.com/cxkw-dev/insomnia && cd insomnia

make install       # terminal app: puts `insomnia` in ~/.cargo/bin
make install-app   # menu bar app: copies Insomnia.app to /Applications (or ~/Applications), starts at login
make setup         # optional: a starter config and the AI usage card (both apps)
```

Install one app or both. Each install is a **copy**: after you pull changes,
rerun the matching `make` target to get them.

`make setup` writes `~/.config/insomnia/config.toml`, the AI usage script, and
an empty account list. It never overwrites your config or account list; the
script itself is updated to this checkout's version each time. Without it,
both apps run on defaults.

To remove the menu bar app and its login item: `make uninstall-app`.

## Terminal app

```sh
insomnia            # display + system stay awake (same as -d -i)
insomnia -d         # just the display
insomnia -t 2h      # two hours, then let go
insomnia -di -t 90m # flags combine; -t takes the last spot in a group
insomnia -q -t 1h   # no UI, just hold quietly in the foreground
```

| flag | effect |
|------|--------|
| `-d` | keep the display awake |
| `-i` | keep the system awake while idle |
| `-s` | prevent sleep entirely, even lid-closed (AC power only) |
| `-m` | keep disks from idle-sleeping |
| `-t <time>`, `--for <time>` | stay awake for a duration, then release |
| `-q`, `--quiet` | no UI; hold quietly |
| `--config <path>` | use another config file |
| `-h` / `-V` | help / version |

Durations read the way you'd say them: `300` (seconds, like `caffeinate -t`),
`90s`, `45m`, `2h`, `1h30m`.

| key | action |
|-----|--------|
| `↑` `↓` / `k` `j` | select a row |
| `page up` / `page down` | scroll the dashboard |
| `m` | pause or resume the sky's motion |
| `q`, `esc`, `ctrl-c` | release and quit |

The session stays pinned at the top and the dashboard scrolls beneath it;
small terminals wrap and scroll, and a tiny one falls back to a single line.
Prefer separate boxes beside the session? Set `[dashboard] combined = false`.

**Docker** is built into the terminal app: every container, with its state,
uptime, health checks, and exit details. It appears whenever the Docker
daemon answers; `[docker] enabled = false` turns it off.

**The sky** — sparse stars, comets, and the odd shooting star — stays around
the edges. `m` freezes it; `[sky] motion = false` starts it still, and
`comets = false` / `shooting_stars = false` ground either one.

## Menu bar app

Click the moon to open the popover. It shows:

- **AI accounts** from the [AI usage card](#ai-usage-card), grouped into
  **Enterprise** and **Personal**, each with its quota meters and reset
  times. An account near its limit, stale, or logged out is tinted.
- **Every other card** below, as plain rows.

**Accounts** in the footer opens a list of every AI account with a switch.
Turn one off and it disappears from both apps, and the card stops fetching
it; turn it back on any time. See [turning an account off](#turning-an-account-off).

The switch beside the wordmark keeps the **display** awake; the moon turns
amber while it's on. Turn it off or quit the app to release it. (The
terminal app is the one for system, lid-closed, or timed holds.)

The popover refreshes when you open it and every minute in the background.

### Grouping accounts

Accounts are grouped by plan: org, business, enterprise, and team plans go
under Enterprise, everything else under Personal. To place an account
yourself, list any case-insensitive part of its name in your config:

```toml
[accounts]
personal = ["jane", "side-project"]
enterprise = ["acme"]
```

## Cards (both apps)

A card is a shell command on a timer. Each line it prints becomes a row:

```toml
[[card]]
title = "services"
interval = 30
command = '''
for url in localhost:3000 localhost:8080; do
  if curl -sf -o /dev/null "$url"; then echo "ok|$url"; else echo "bad|$url|no answer"; fi
done
'''
```

| field | default | meaning |
|-------|---------|---------|
| `title` | required | the card's name |
| `command` | required | run with `sh -c` every `interval` seconds |
| `interval` | `10` | seconds between runs (minimum 2) |
| `glyph`, `accent`, `side` | — | terminal app's separate-box layout only |

Lines can use a small pipe-separated format; anything else shows as plain
text:

| line | shows as |
|------|----------|
| `ok\|name\|detail` | a healthy item (`warn`, `bad`, `off` for other states; detail optional) |
| `hdr\|accent\|text` | a group header, e.g. a provider or environment |
| `bar\|accent\|label\|percent\|detail` | a 0–100 meter, attached to the row above |

Accents: `violet` `pink` `sky` `blue` `emerald` `gold` `red` `slate` `coral`
`teal`.

A card appears once its command succeeds with output, and shows up to 64 rows
(the rest collapse into a `+N more rows` line). A command still running after
45 seconds in the terminal app (15 in the menu bar) is stopped, along with
anything it started. If a refresh fails, the terminal app keeps the last good
rows, marked stale.

Card commands run as you. Keep tokens out of the TOML; use authenticated CLIs
(`gh`, `docker`), the Keychain, or your environment. More ideas:

```toml
[[card]]
title = "pull requests"
interval = 120
command = 'gh search prs --state=open --review-requested=@me --json title --jq ".[].title"'
```

## AI usage card

[`cards/ai-usage.sh`](cards/ai-usage.sh) is a ready-made card for quota
windows, reset times, spend, and hourly trend. `make setup` installs it and
adds it to your config, so it shows up in both apps.

| service | accounts come from | needs |
|---------|--------------------|-------|
| Claude | Claude Code's login and Claude Desktop's active session | `jq`, `curl` |
| Codex | `~/.codex/auth.json`, shared by the Codex CLI and app | `jq`, `curl` |
| Copilot | every account in `gh auth status` | `jq`, `curl`, [`gh`](https://cli.github.com) |

Services you aren't logged into are skipped. To see what it will show, run
`~/.config/insomnia/ai-usage.sh all`.

### Adding accounts

Whatever you're logged into right now appears on its own, marked `▸`.

- **Copilot** — `gh auth login` each GitHub account; all of them appear.
- **Another Claude or Codex account** — log into it (`/login` in Claude Code,
  or `codex login`), save it under a name, then log back into your usual one:

  ```sh
  ~/.config/insomnia/ai-usage.sh capture claude work
  ~/.config/insomnia/ai-usage.sh capture codex work
  ```

- **Always list an account**, even while it's logged out, in
  `~/.config/insomnia/ai-usage/accounts.conf` (`<service> <name>` per line).
  It shows as `not logged in` until a login or capture with that name exists.

### Turning an account off

Use the switches under **Accounts** in the menu bar popover, or edit
`accounts.conf` yourself: add `hidden` after an account's name and it
disappears from both apps, and the card stops fetching it. Nothing is
deleted — remove the word (or flip the switch back) to bring it back:

```text
claude old-job hidden
```

A captured account keeps refreshing while it's logged in somewhere the card
can see. Once it's logged out everywhere it eventually goes `stale`, and the
row says so rather than showing old numbers as current; capture it again to
revive it.

### Privacy

Everything stays in `~/.config/insomnia/ai-usage/` (mode 700): captured
logins, a short response cache, and trend samples. The script talks only to
each provider's own usage and sign-in endpoints — the same undocumented ones
their apps use, so they can change without notice. A login is refreshed only
when its token has expired or been refused, one run at a time (the two apps
share a lock), and tokens never appear on a command line. Reading Claude
Desktop's session may prompt once for Keychain access to "Claude Safe
Storage".

## Configuration

`~/.config/insomnia/config.toml` (or `$XDG_CONFIG_HOME/insomnia/config.toml`).
Everything is optional. The annotated starting point is
[`cards/config.example.toml`](cards/config.example.toml).

| section | used by | what it does |
|---------|---------|--------------|
| `[[card]]` | both apps | dashboard cards (any number) |
| `[accounts]` | menu bar app | which AI accounts are Enterprise or Personal |
| `[dashboard]` | terminal app | `combined = false` for separate boxes |
| `[docker]` | terminal app | `enabled = false` hides containers |
| `[sky]` | terminal app | `motion`, `comets`, `shooting_stars` |

```toml
[[card]]
title = "ai usage"
interval = 30
command = '"$HOME/.config/insomnia/ai-usage.sh" all'

[accounts]
personal = ["jane"]

[dashboard]
combined = true

[docker]
enabled = true

[sky]
motion = true
```

A misspelled section, setting, or accent is an error that lists the valid
names.

## How it works

Both apps call `IOPMAssertionCreateWithName`, the API `caffeinate` uses. The
kernel ties each assertion to the process holding it, so it's released the
moment that process exits — even on `SIGKILL`. Check while either runs:

```sh
pmset -g assertions | grep -i insomnia
```

The menu bar app doesn't run your card commands itself. It calls
`insomnia-snapshot`, a small helper built from `terminal/` and bundled inside
`Insomnia.app`, which reads the same config and returns the rows as JSON.

## Repository layout

| path | what's there |
|------|--------------|
| [`terminal/`](terminal/) | Rust crate: the `insomnia` terminal app and the `insomnia-snapshot` helper |
| [`macos/`](macos/) | Swift menu bar app and its build / install scripts |
| [`cards/`](cards/) | the AI usage card, a starter config, and `make setup`'s script |
| [`docs/`](docs/) | README images and the script that regenerates them from fake data |

## Development

```sh
make run           # run the terminal app from the working tree
make macos         # build dist/Insomnia.app without installing it
make check         # fmt, clippy, tests, Swift typecheck, script syntax
make preview       # print one terminal frame, headless
make screenshots   # regenerate docs/*.png from fixture data (needs Chrome)
```

To add something to the sky, write a draw function and add it to
`sky::FLYERS`; the guide is at the top of
[`terminal/src/sky.rs`](terminal/src/sky.rs). Pull requests welcome.

## License

[MIT](LICENSE)
