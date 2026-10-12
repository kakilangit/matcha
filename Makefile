# Entry point for development and packaging. Run `make` for the list of targets.

APP_ID     := dev.kakilangit.matcha
TARGET_DIR := $(or $(CARGO_TARGET_DIR),target)
PREFIX     ?= $(HOME)/.local
DESTDIR    ?=
ICON_SIZES := 16 32 48 64 128 256 512

.DEFAULT_GOAL := help
.PHONY: help fmt fmt-check check lint test ci run build release icons \
	install uninstall app dmg appimage clean

help: ## Show this help
	@awk 'BEGIN {FS = ":.*## "} /^[a-z-]+:.*## / {printf "  %-12s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

fmt: ## Format the code
	cargo fmt --all

fmt-check: ## Fail if the code is not formatted
	cargo fmt --all -- --check

check: ## Type-check all targets
	cargo check --all-targets

lint: ## Run clippy (pedantic; no unwrap, expect, or panic outside tests)
	cargo clippy --all-targets -- -D warnings

test: ## Run the tests
	cargo test

ci: fmt-check check lint test ## Everything CI runs

run: ## Run a debug build
	cargo run

build: ## Build a debug binary
	cargo build

release: ## Build an optimized binary
	cargo build --release

icons: ## Regenerate assets/icons from assets/icon.svg
	./scripts/build-icons.sh

install: release ## Install on Linux with icon and launcher entry (PREFIX, DESTDIR)
	install -Dm755 $(TARGET_DIR)/release/matcha $(DESTDIR)$(PREFIX)/bin/matcha
	install -Dm644 assets/$(APP_ID).desktop $(DESTDIR)$(PREFIX)/share/applications/$(APP_ID).desktop
	for size in $(ICON_SIZES); do \
		install -Dm644 assets/icons/matcha-$$size.png \
			$(DESTDIR)$(PREFIX)/share/icons/hicolor/$${size}x$${size}/apps/$(APP_ID).png; \
	done

uninstall: ## Remove what `make install` installed
	rm -f $(DESTDIR)$(PREFIX)/bin/matcha $(DESTDIR)$(PREFIX)/share/applications/$(APP_ID).desktop
	for size in $(ICON_SIZES); do \
		rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/$${size}x$${size}/apps/$(APP_ID).png; \
	done

app: ## Build dist/Matcha.app (macOS)
	SKIP_DMG=1 ./scripts/package-macos.sh

dmg: ## Build dist/Matcha.app and a .dmg (macOS; UNIVERSAL=1 for Intel too)
	./scripts/package-macos.sh

appimage: ## Build a .AppImage in dist/ (Linux)
	./scripts/package-appimage.sh

clean: ## Remove build output and dist/
	cargo clean
	rm -rf dist
