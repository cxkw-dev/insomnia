#!/bin/sh
# Install (or reinstall) the menu bar app: build dist/Insomnia.app, replace
# the copy in /Applications, register it as a login item, and relaunch it.
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
built="$repo_root/dist/Insomnia.app"
# Prefer /Applications; fall back to ~/Applications when it is not writable.
if [ -w /Applications ] || [ -d /Applications/Insomnia.app ]; then
  installed="/Applications/Insomnia.app"
else
  mkdir -p "$HOME/Applications"
  installed="$HOME/Applications/Insomnia.app"
fi

sh "$repo_root/macos/build.sh"

# Stop the running copy so the bundle can be swapped cleanly.
if pgrep -x Insomnia >/dev/null 2>&1; then
  osascript -e 'tell application "Insomnia" to quit' >/dev/null 2>&1 || pkill -x Insomnia || true
  sleep 1
fi

rm -rf "$installed"
ditto "$built" "$installed"

# Register as a login item (idempotent).
osascript >/dev/null <<APPLESCRIPT
tell application "System Events"
  if not (exists login item "Insomnia") then
    make login item at end with properties {path:"$installed", hidden:false}
  end if
end tell
APPLESCRIPT

open "$installed"
printf 'Installed %s and set it to start at login\n' "$installed"
