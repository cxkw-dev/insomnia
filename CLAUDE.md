# CLAUDE.md

Guidance for Claude Code when working in this repository.

## What this is

insomnia is a macOS-only Rust TUI that keeps the Mac awake — a `caffeinate`
replacement holding IOKit power assertions directly — wrapped in a night-sky
animation with a status card and a configurable dashboard. `lib.rs` has a
`compile_error!` for non-macOS targets, so everything (including tests) must
run on a Mac.

## Commands

```sh
make run       # run from the working tree
make check     # fmt --check + clippy -D warnings + tests — run before committing
make test      # cargo test
make preview   # one headless frame (cargo run --example preview <w> <h> <tick>)
make install   # rebuild and replace the installed binary
cargo test durations   # a single test, by substring
```

**The install gotcha:** the `insomnia` on the user's PATH is the installed
copy (`~/.cargo/bin/insomnia`). Code changes do nothing to it until
`make install`. When the user says a change "isn't showing up", this is
almost always why.

**Seeing a change:** `make preview` renders one frame headlessly to stdout —
use it to eyeball layout work without a live terminal. The README's hero
frame is that exact output (`cargo run --example preview 110 46 7`); if a
change alters the frame's look, regenerate the README block from it.

## Architecture

One render thread, one thread per data source, one channel between them.

- `main.rs` — flags, assertion creation, quiet mode; hands off to `ui::run`.
- `power.rs` — IOKit assertions (created before the TUI, dropped on exit)
  and battery status via `pmset`.
- `config.rs` — CLI flags, durations, and the TOML config
  (`~/.config/insomnia/config.toml`): docker toggle, `[sky]` flyer toggles,
  `[[card]]` blocks, `[dashboard] combined`.
- `collect.rs` — background threads for docker and card commands, streaming
  `Update`s over mpsc; also the row protocol parser (`parse_row`).
- `cards.rs` — turns collected data into render-ready `SideCard`s
  (`build_cards`) and draws them: health rows, section headers, meters.
- `ui.rs` — the event loop, layout (wide / stacked / three-column / merged /
  compact), and the status card.
- `sky.rs` — starfield and glint ambience plus the `FLYERS` registry
  (comets, shooting stars).
- `art.rs` — the title art. `pixel.rs` — half-block pixel primitive.
  `theme.rs` — the shared palette.

## Invariants worth knowing

- **Height budgets are counted in rows.** Card spacing must live in the row
  lists themselves (see `space_sections`) — a line conjured at render time is
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
  `hdr|accent|text` for section headers; `bar|accent|label|percent|detail`
  for meters; anything else renders as plain text. Malformed protocol lines
  fall back to plain text rather than erroring. Cards cap at 32 rows.

## Conventions

- Commit messages are lowercase, `area: summary` style (`sky: …`, `ui: …`,
  `cards: …`), with no attribution trailers — match `git log`.
- Degenerate terminal sizes must clip, not panic: keep the smoke tests in
  `ui.rs`, `cards.rs`, and `sky.rs` passing, and extend them when adding
  render paths.
- Slow work never goes on the render thread — new data sources get their own
  thread in `collect.rs` and report over the existing channel.
