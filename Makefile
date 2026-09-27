# insomnia — dev shortcuts.
#
# Two apps live in this repo:
#   terminal/  the Rust TUI (`insomnia`) and the `insomnia-snapshot` helper
#   macos/     the Swift menu bar app, which bundles that helper
#
# Neither the `insomnia` on your PATH nor the app in ~/Applications tracks
# the working tree. After changing code, `make install` (terminal) or
# `make install-app` (menu bar) rebuilds and replaces the installed copy.

CARGO := cd terminal && cargo

.PHONY: install run check test preview macos install-app uninstall-app

# --- terminal app -----------------------------------------------------------

install:
	cargo install --path terminal --quiet

run:
	$(CARGO) run

check:
	$(CARGO) fmt --check
	$(CARGO) clippy --all-targets -- -D warnings
	$(CARGO) test --quiet

test:
	$(CARGO) test

preview:
	$(CARGO) run --quiet --example preview 110 46 7

# --- menu bar app -----------------------------------------------------------

macos:
	sh macos/build.sh

install-app:
	sh macos/install.sh

uninstall-app:
	sh macos/uninstall.sh
