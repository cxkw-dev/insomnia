#!/bin/sh
# Install the starter config and the ai usage card into ~/.config/insomnia.
# Never overwrites: files you already have are kept as they are.
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
dest="${XDG_CONFIG_HOME:-$HOME/.config}/insomnia"
mkdir -p "$dest/ai-usage"
chmod 700 "$dest/ai-usage"

place() { # place <source> <target> <mode>
  if [ -e "$2" ]; then
    if cmp -s "$1" "$2"; then
      printf 'up to date  %s\n' "$2"
    else
      printf 'kept yours  %s (compare with %s)\n' "$2" "$1"
    fi
  else
    cp "$1" "$2" && chmod "$3" "$2"
    printf 'created     %s\n' "$2"
  fi
}

place "$repo_root/cards/config.example.toml" "$dest/config.toml" 600
place "$repo_root/cards/ai-usage.sh" "$dest/ai-usage.sh" 700
place "$repo_root/cards/accounts.conf.example" "$dest/ai-usage/accounts.conf" 600

for bin in jq curl gh; do
  command -v "$bin" >/dev/null 2>&1 || printf 'note: %s not found (brew install %s)\n' "$bin" "$bin"
done
