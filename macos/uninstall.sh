#!/bin/sh
# Remove the installed menu bar app and its login item. install.sh may have
# put the app in /Applications or ~/Applications, so remove whichever exist.
set -eu

if pgrep -x Insomnia >/dev/null 2>&1; then
  osascript -e 'tell application "Insomnia" to quit' >/dev/null 2>&1 || pkill -x Insomnia || true
  sleep 1
fi
osascript -e 'tell application "System Events" to delete login item "Insomnia"' >/dev/null 2>&1 || true

removed=0
for installed in /Applications/Insomnia.app "$HOME/Applications/Insomnia.app"; do
  if [ -e "$installed" ]; then
    rm -rf "$installed"
    printf 'Removed %s\n' "$installed"
    removed=1
  fi
done
[ "$removed" = 1 ] || printf 'Insomnia.app is not installed\n'
