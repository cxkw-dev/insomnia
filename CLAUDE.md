# CLAUDE.md

Guidance for Claude Code when working in this repository.

## What this is

insomnia is a macOS-only Rust TUI and native menu bar app that keep the Mac
awake. Both hold IOKit power assertions directly. The TUI has a night-sky
animation with a status card and a configurable dashboard. `lib.rs` has a
`compile_error!` for non-macOS targets, so everything (including tests) must
run on a Mac.

## Commands

```sh
make run       # run from the working tree
make check     # fmt, clippy -D warnings, tests, Swift typecheck, script syntax — run before committing
make test      # cargo test
make preview   # one headless frame (cargo run --example preview <w> <h> <tick>)
make screenshots   # regenerate docs/terminal.png + docs/menubar.png from fixtures
make install   # rebuild and replace the installed binary
make macos     # build dist/Insomnia.app with the bundled snapshot helper
make install-app   # build, copy to /Applications (or ~/Applications), login item, relaunch
make uninstall-app # remove the installed app and its login item
make setup     # starter config + ai usage card into ~/.config/insomnia (updates the script, keeps config)
cd terminal && cargo test durations   # a single test, by substring
```

The Rust crate lives in `terminal/`, so bare `cargo` commands need either
`cd terminal` or `--manifest-path terminal/Cargo.toml`. The Makefile targets
handle this from the repo root.

**The install gotcha:** the `insomnia` on the user's PATH is the installed
copy (`~/.cargo/bin/insomnia`), and the menu bar app the user runs is the copy
in `/Applications/Insomnia.app` (or `~/Applications` when the system folder is
not writable). Code changes do nothing to either until
`make install` / `make install-app`. When the user says a change "isn't
showing up", this is almost always why. `dist/` is only build output; nothing
runs from it.

**Seeing a change:** `make preview` renders one frame headlessly to stdout —
use it to eyeball layout work without a live terminal. The README's images
come from `make screenshots`: `docs/terminal.png` is the preview example's
`--html` export, and `docs/menubar.png` is a `DESIGN_PREVIEW` build of the app
fed `docs/fixtures/`. If a change alters either look, regenerate them.

## Layout

One repo, two apps that share `~/.config/insomnia/config.toml`:

- `terminal/` — the Rust crate: `Cargo.toml`, `src/`, and `examples/`. Builds
  the `insomnia` TUI and the `insomnia-snapshot` helper binary.
- `macos/` — the Swift menu bar app plus `build.sh`, `install.sh`,
  `uninstall.sh`, fonts, and icons. It bundles the snapshot helper from
  `terminal/target/release/`.
- `cards/` — the optional `ai-usage.sh` card, `config.example.toml`, an
  `accounts.conf` example, and `setup.sh` (`make setup`: always installs the
  script, never overwrites config or accounts). A test parses
  `config.example.toml`, so keep it valid.
- `docs/` — README images, `screenshots.sh`, `frame.swift` (rounded corners
  and shadow), and the fixture rows the menu bar shot uses.
- `dist/` — gitignored output of `make macos`.

The root holds only the Makefile, README, LICENSE, and this file. Module
paths below (`main.rs`, `ui.rs`, …) are relative to `terminal/src/`.

## Architecture

One render thread, one thread per data source, one channel between them.

- `main.rs` — flags, assertion creation, quiet mode; hands off to `ui::run`.
- `duration.rs` — parsing (`1h30m`) and formatting (`01:17:43`, `1h 30m`).
- `text.rs` — display-width truncate / pad / pair / wrap. Measure in cells,
  never `chars().count()`, and uppercase before measuring.
- `power.rs` — IOKit assertions (created before the TUI, dropped on exit)
  and battery status via `pmset`.
- `config.rs` — CLI flags, durations, and the TOML config
  (`~/.config/insomnia/config.toml`): docker toggle, `[sky]` flyer toggles,
  `[[card]]` blocks (typed `Accent` / `Side`), `[dashboard] combined`, and
  `[accounts]` (menu bar grouping; the snapshot passes it through, the TUI
  ignores it).
- `collect.rs` — background threads for docker and card commands, streaming
  `Update`s over mpsc; card commands run in their own process group and are
  killed whole on timeout. Also the row protocol parser (`parse_row`) and
  the shared severity cutoffs (`WARN_PCT`, `BAD_PCT`, `severity`).
- `cards.rs` — turns collected data into render-ready `SideCard`s
  (`build_cards`; a failed refresh keeps the last `Feed` rows, marked stale)
  and draws rows: health rows, group headers, meters, and the meter column
  constants the dashboard aligns to.
- `dashboard.rs` — the combined layout: each card a section, meters attached
  to their account, stable selection identities (card, header, name).
- `bin/insomnia-snapshot.rs` — JSON snapshot of the configured cards for the
  menu bar app, using the same collector and row protocol as the TUI.
- `ui.rs` — the event loop and both layouts: combined (session over the
  scrollable dashboard) and separate boxes (wide / stacked / three-column),
  each falling back to compact. `draw` dispatches on `ViewState.dashboard`.
- `sky.rs` — starfield and glint ambience plus the `FLYERS` registry
  (comets, shooting stars).
- `art.rs` — the title art. `pixel.rs` — half-block pixel primitive.
  `theme.rs` — the shared palette.
- `macos/Sources/` — the SwiftUI menu bar app: `Snapshot.swift` (decoding
  and the async helper runner), `Accounts.swift` (turns snapshot rows into
  provider accounts once per refresh), `AccountVisibility.swift` (reads and
  writes `hidden` flags in `accounts.conf` for the popover's account
  editor), `DashboardModel.swift`,
  `DashboardView.swift`, `Appearance.swift`, and `InsomniaApp.swift`.
  `macos/build.sh` compiles every file and bundles the Rust helper, fonts,
  and icons; `dist/` is ignored.

## Invariants worth knowing

- **Nothing personal in the repo.** Account names, emails, and credentials
  live only in the user's `~/.config/insomnia/` (config `[accounts]`,
  `ai-usage/accounts.conf`, captured snapshots). Fixtures use generic names
  like `personal` / `work account`; the app never hardcodes an identity.
  The installed `~/.config/insomnia/ai-usage.sh` is a copy — repo changes to
  `cards/ai-usage.sh` don't reach it until `make setup`.
- **Never rotate a login without cause.** `ai-usage.sh` refreshes an OAuth
  token only when it has expired or the API answered 401, under a
  `with_lock` lock, re-reading credentials after acquiring it. Refresh tokens
  are single-use and shared with the providers' own CLIs.
- **Account visibility lives in `accounts.conf`.** `<service> <name> hidden`
  stops the script fetching and emitting that account; the menu bar editor
  only rewrites that file. Account names in rows are never truncated by the
  script, since they double as these identifiers.
- **The default is focused and uppercase.** Keep one frame with the session
  anchored above a scrollable dashboard. `combined = false` retains separate
  cards. Source commands and data are not rewritten; only displayed text is
  uppercased. `m` toggles sky motion, and `[sky] motion = false` starts still.
- **Selection follows source identity.** Attach consecutive meters to their
  account and display every quota and reset time immediately. Provider labels
  and gauges keep their source accents; there are no disclosure controls.
  Source-marked stale values keep their gauge with an explicit stale label.

- **Height budgets are counted in rows.** Card spacing must live in the row
  lists themselves (see `space_headers`) — a line conjured at render time is
  one the layout never reserved, and the card silently loses its last row.
- **Folding reruns placement.** A card that doesn't fit folds into a summary
  row inside the status card, which shrinks the very budget being divided —
  the placement loop in `ui::draw` iterates until the folded set is stable.
- **Half-block pixels.** `pixel::set_px` addresses (cell column, pixel row):
  two vertical pixels per cell via `▀`/`▄` fg/bg, merging with whatever is
  already in the cell. Out-of-bounds pixels are clipped, never panic.
- **The sky is a registry.** A flyer is a draw function plus one entry in
  `sky::FLYERS`; its `[sky]` config toggle appears automatically, and config
  loading rejects unknown toggle names against `sky::names()`. No ships —
  the sky deliberately holds only comets and shooting stars.
- **Animation is hash-driven, not stateful.** Everything positions itself
  from `tick` and a position hash (`sky::mix`), so any frame is
  reproducible headlessly — that's what the preview example and the
  render-smoke tests rely on.
- **The row protocol** (card command stdout, one row per line):
  `ok|name|detail`, `warn|…`, `bad|…`, `off|…` for health rows;
  `hdr|accent|text` for group headers; `bar|accent|label|percent|detail`
  for meters; anything else renders as plain text. Malformed protocol lines
  fall back to plain text rather than erroring. Cards cap at 64 rows plus a
  `+N more rows` line. The menu bar treats a header as an AI provider only
  when its last word is `claude`, `codex`, or `copilot`.

## Conventions

- Commit messages are lowercase, `area: summary` style (`sky: …`, `ui: …`,
  `cards: …`), with no attribution trailers — match `git log`.
- Degenerate terminal sizes must clip, not panic: keep the smoke tests in
  `ui.rs`, `cards.rs`, and `sky.rs` passing, and extend them when adding
  render paths.
- Slow work never goes on the render thread — new data sources get their own
  thread in `collect.rs` and report over the existing channel.
