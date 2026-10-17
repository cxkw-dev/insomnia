#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
app="$repo_root/dist/Insomnia.app"
# DESIGN_PREVIEW=1 builds a windowed copy for screenshots (docs/screenshots.sh).
swift_flags=""
if [ "${DESIGN_PREVIEW:-0}" = 1 ]; then
  app="$repo_root/dist/Insomnia Preview.app"
  swift_flags="-D DESIGN_PREVIEW"
fi
contents="$app/Contents"
arch=$(uname -m)

cargo build --release --manifest-path "$repo_root/terminal/Cargo.toml" --bin insomnia-snapshot
# Start from an empty bundle so resources dropped from the repo don't linger.
rm -rf "$app"
mkdir -p "$contents/MacOS" "$contents/Resources/Fonts" "$contents/Resources/Icons"
# shellcheck disable=SC2086 # swift_flags is deliberately word-split
swiftc -O -swift-version 6 -parse-as-library -target "${arch}-apple-macosx13.0" $swift_flags \
  "$repo_root"/macos/Sources/*.swift -o "$contents/MacOS/Insomnia"
cp "$repo_root/terminal/target/release/insomnia-snapshot" "$contents/MacOS/insomnia-snapshot"
cp "$repo_root/macos/Fonts/Geist.ttf" "$contents/Resources/Fonts/Geist.ttf"
cp "$repo_root/macos/Fonts/GeistMono.ttf" "$contents/Resources/Fonts/GeistMono.ttf"
cp "$repo_root/macos/Fonts/OFL.txt" "$contents/Resources/Fonts/OFL.txt"
cp "$repo_root/macos/Icons/codex.png" "$contents/Resources/Icons/codex.png"
cp "$repo_root/macos/Icons/copilot.svg" "$contents/Resources/Icons/copilot.svg"
cp "$repo_root/macos/Info.plist" "$contents/Info.plist"
codesign --force --sign - "$contents/MacOS/insomnia-snapshot"
codesign --force --sign - "$app"
printf 'Built %s\n' "$app"
