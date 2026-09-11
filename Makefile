.PHONY: run test check bundle clean

# Build the bundle and launch it (kills any running copy first).
TARGET_DIR := $(shell cargo metadata --no-deps --format-version 1 | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')

run: bundle
	@pkill -x daybar 2>/dev/null || true
	open "$(TARGET_DIR)/Daybar.app"

test:
	cargo test

check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings

bundle:
	./scripts/bundle.sh

clean:
	cargo clean
