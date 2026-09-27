#!/bin/sh
# Remove the installed menu bar app and its login item.
set -eu

# Prefer /Applications; fall back to ~/Applications when it is not writable.
if [ -w /Applications ] || [ -d /Applications/Insomnia.app ]; then
  installed="/Applications/Insomnia.app"
else
  mkdir -p "$HOME/Applications"
  installed="$HOME/Applications/Insomnia.app"
fi

if pgrep -x Insomnia >/dev/null 2>&1; then
  osascript -e 'tell application "Insomnia" to quit' >/dev/null 2>&1 || pkill -x Insomnia || true
fi
osascript -e 'tell application "System Events" to delete login item "Insomnia"' >/dev/null 2>&1 || true
rm -rf "$installed"
printf 'Removed %s\n' "$installed"
