#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
app="$repo_root/dist/Insomnia.app"
contents="$app/Contents"
arch=$(uname -m)

cd "$repo_root"
cargo build --release --bin insomnia-snapshot
mkdir -p "$contents/MacOS" "$contents/Resources/Fonts" "$contents/Resources/Icons"
swiftc -O -swift-version 6 -parse-as-library -target "${arch}-apple-macosx13.0" \
  "$repo_root/macos/Sources/InsomniaApp.swift" -o "$contents/MacOS/Insomnia"
cp "$repo_root/target/release/insomnia-snapshot" "$contents/MacOS/insomnia-snapshot"
cp "$repo_root/macos/Fonts/Geist.ttf" "$contents/Resources/Fonts/Geist.ttf"
cp "$repo_root/macos/Fonts/GeistMono.ttf" "$contents/Resources/Fonts/GeistMono.ttf"
cp "$repo_root/macos/Fonts/OFL.txt" "$contents/Resources/Fonts/OFL.txt"
cp "$repo_root/macos/Icons/codex.png" "$contents/Resources/Icons/codex.png"
cp "$repo_root/macos/Icons/copilot.svg" "$contents/Resources/Icons/copilot.svg"
cp "$repo_root/macos/Info.plist" "$contents/Info.plist"
codesign --force --sign - "$contents/MacOS/insomnia-snapshot"
codesign --force --sign - "$app"
printf 'Built %s\n' "$app"
