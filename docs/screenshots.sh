#!/bin/sh
# Regenerate the README images from fixture data — never your real accounts.
#   docs/terminal.png  the TUI frame from `examples/preview.rs`, in color
#   docs/menubar.png   the menu bar popover, from a DESIGN_PREVIEW build
# Needs Google Chrome (headless, to draw the terminal cells) and the Command
# Line Tools (swiftc, for the preview build and docs/frame.swift).
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
docs="$repo_root/docs"
chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
[ -x "$chrome" ] || { echo "screenshots: needs Google Chrome" >&2; exit 1; }
work=$(mktemp -d "${TMPDIR:-/tmp}/insomnia-shots.XXXXXX")
app_pid=""
# Also stop the preview app if the script dies while it is still up.
trap 'if [ -n "$app_pid" ]; then kill "$app_pid" 2>/dev/null || true; fi; rm -rf "$work"' EXIT

swiftc -O "$docs/frame.swift" -o "$work/frame"

# shoot <html> <png> <width> <height> — 2x
shoot() {
  "$chrome" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
    --window-size="$3,$4" --screenshot="$2" "file://$1" >/dev/null 2>&1
}

# --- terminal -----------------------------------------------------------------

cols=110 rows=46 cell_w=8 cell_h=17 bar=34 inset=18
cargo run --quiet --manifest-path "$repo_root/terminal/Cargo.toml" \
  --example preview "$cols" "$rows" 7 --html \
  | sed -e 's/^.*<pre /<pre /' -e 's#</pre></html>#</pre>#' > "$work/cells.html"
win_w=$((cols * cell_w + inset * 2))
win_h=$((rows * cell_h + inset * 2 + bar))
cat > "$work/terminal.html" <<HTML
<!doctype html><meta charset="utf-8"><style>
html,body{margin:0;background:#11121d}
.win{width:${win_w}px;height:${win_h}px;overflow:hidden;background:#11121d}
.bar{height:${bar}px;display:flex;align-items:center;gap:8px;padding:0 14px;background:#191a27}
.bar i{width:12px;height:12px;border-radius:50%;display:block}
.bar i:nth-child(1){background:#ff5f57}.bar i:nth-child(2){background:#febc2e}.bar i:nth-child(3){background:#28c840}
.bar b{flex:1;text-align:center;margin-right:52px;font:500 12px -apple-system,sans-serif;color:#8a8fa3}
pre{margin:${inset}px;font:13.5px/${cell_h}px Menlo,monospace;white-space:pre}
span{display:inline-block;width:${cell_w}px;height:${cell_h}px;vertical-align:top}
</style><div class="win"><div class="bar"><i></i><i></i><i></i><b>insomnia -t 2h</b></div>
$(cat "$work/cells.html")
</div>
HTML
shoot "$work/terminal.html" "$work/terminal.png" "$win_w" "$win_h"
"$work/frame" "$work/terminal.png" "$docs/terminal.png" 24
printf 'wrote %s\n' "$docs/terminal.png"

# --- menu bar popover ---------------------------------------------------------

DESIGN_PREVIEW=1 sh "$repo_root/macos/build.sh" >/dev/null
mkdir -p "$work/xdg/insomnia"
cat > "$work/xdg/insomnia/config.toml" <<TOML
[accounts]
business = ["work account", "example-team"]

[[card]]
title = "ai usage"
command = 'cat "$docs/fixtures/ai-usage.txt"'

[[card]]
title = "services"
command = 'cat "$docs/fixtures/services.txt"'
TOML
ready="$work/window"
XDG_CONFIG_HOME="$work/xdg" INSOMNIA_PREVIEW_READY="$ready" \
  "$repo_root/dist/Insomnia Preview.app/Contents/MacOS/Insomnia" >/dev/null 2>&1 &
app_pid=$!
i=0
until [ -s "$ready" ]; do
  i=$((i + 1)); [ "$i" -lt 60 ] || { echo "screenshots: preview never loaded" >&2; exit 1; }
  sleep 0.5
done
screencapture -x -o -l"$(cat "$ready")" "$work/popover.png"
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""
"$work/frame" "$work/popover.png" "$docs/menubar.png" 28
printf 'wrote %s\n' "$docs/menubar.png"
