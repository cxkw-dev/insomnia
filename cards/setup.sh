#!/bin/sh
# Install the starter config and the ai usage card into ~/.config/insomnia.
# Your config.toml and accounts.conf are never overwritten; ai-usage.sh is
# code, not config, so it is always replaced with this repo's copy.
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

update() { # update <source> <target> <mode> — always install the latest copy
  if [ -e "$2" ] && cmp -s "$1" "$2"; then
    chmod "$3" "$2"
    printf 'up to date  %s\n' "$2"
  else
    if [ -e "$2" ]; then verb=updated; else verb=created; fi
    # copy then rename, so a card running mid-install never reads half a script
    cp "$1" "$2.tmp.$$" && chmod "$3" "$2.tmp.$$" && mv "$2.tmp.$$" "$2"
    printf '%-11s %s\n' "$verb" "$2"
  fi
}

place "$repo_root/cards/config.example.toml" "$dest/config.toml" 600
update "$repo_root/cards/ai-usage.sh" "$dest/ai-usage.sh" 700
place "$repo_root/cards/accounts.conf.example" "$dest/ai-usage/accounts.conf" 600

for bin in jq curl gh; do
  command -v "$bin" >/dev/null 2>&1 || printf 'note: %s not found (brew install %s)\n' "$bin" "$bin"
done
