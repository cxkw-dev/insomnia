# insomnia — dev shortcuts.
#
# Two apps live in this repo:
#   terminal/  the Rust TUI (`insomnia`) and the `insomnia-snapshot` helper
#   macos/     the Swift menu bar app, which bundles that helper
#   cards/     the optional ai usage card and a starter config (`make setup`)
#
# Neither the `insomnia` on your PATH nor the app in ~/Applications tracks
# the working tree. After changing code, `make install` (terminal) or
# `make install-app` (menu bar) rebuilds and replaces the installed copy.

CARGO := cd terminal && cargo

.PHONY: install setup run check test preview screenshots macos install-app uninstall-app

# --- terminal app -----------------------------------------------------------

install:
	cargo install --path terminal --quiet

# Starter config + ai usage card in ~/.config/insomnia; never overwrites.
setup:
	sh cards/setup.sh

run:
	$(CARGO) run

SWIFTC := swiftc -typecheck -swift-version 6 -parse-as-library \
	-target $(shell uname -m)-apple-macosx13.0 macos/Sources/*.swift

check:
	$(CARGO) fmt --check
	$(CARGO) clippy --all-targets -- -D warnings
	$(CARGO) test --quiet
	$(SWIFTC)
	$(SWIFTC) -D DESIGN_PREVIEW
	bash -n cards/ai-usage.sh
	for f in cards/setup.sh macos/*.sh docs/screenshots.sh; do sh -n "$$f" || exit 1; done

test:
	$(CARGO) test

preview:
	$(CARGO) run --quiet --example preview 110 46 7

# README images from fixture data (docs/fixtures); needs Google Chrome.
screenshots:
	sh docs/screenshots.sh

# --- menu bar app -----------------------------------------------------------

macos:
	sh macos/build.sh

install-app:
	sh macos/install.sh

uninstall-app:
	sh macos/uninstall.sh
