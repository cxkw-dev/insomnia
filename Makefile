# insomnia — dev shortcuts.
#
# Two things live in this repo:
#   src/    the terminal app (`insomnia`) and the `insomnia-snapshot` helper
#   macos/  the Swift menu bar app, which bundles that helper
#
# Neither the `insomnia` on your PATH nor the app in /Applications tracks
# the working tree. After changing code, `make install` (terminal) or
# `make install-app` (menu bar) rebuilds and replaces the installed copy.

.PHONY: install run check test preview macos install-app uninstall-app

# --- terminal app -----------------------------------------------------------

install:
	cargo install --path . --quiet

run:
	cargo run

check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test --quiet

test:
	cargo test

preview:
	cargo run --quiet --example preview 110 46 7

# --- menu bar app -----------------------------------------------------------

macos:
	sh macos/build.sh

install-app:
	sh macos/install.sh

uninstall-app:
	sh macos/uninstall.sh
