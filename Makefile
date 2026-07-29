# insomnia — dev shortcuts.
#
# The `insomnia` on your PATH is the installed copy, not the working tree:
# after changing code, `make install` rebuilds and replaces it so the next
# launch runs what you just wrote.

.PHONY: install run check test preview

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
