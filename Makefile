# Entry point for development and packaging. Run `make` for the list of targets.

APP_ID     := dev.kakilangit.matcha
TARGET_DIR := $(or $(CARGO_TARGET_DIR),target)
PREFIX     ?= $(HOME)/.local
DESTDIR    ?=
ICON_SIZES := 16 32 48 64 128 256 512
VERSION    := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
UNAME      := $(shell uname -s)
SUDO       := $(if $(filter 0,$(shell id -u)),,sudo)
APPS_DIR   ?= /Applications

# `make install` picks the native package for the platform: the .app on macOS,
# the .deb on Debian/Ubuntu, and a plain file tree under PREFIX elsewhere.
# Setting PREFIX or DESTDIR asks for that file tree on Linux; INSTALL_METHOD
# (app, deb, or local) overrides the choice.
ifeq ($(UNAME),Darwin)
INSTALL_METHOD ?= app
else ifneq ($(filter-out undefined file,$(origin PREFIX) $(origin DESTDIR)),)
INSTALL_METHOD ?= local
else ifneq ($(shell command -v dpkg 2>/dev/null),)
INSTALL_METHOD ?= deb
else
INSTALL_METHOD ?= local
endif

# Benchmarks, profiling, and debugging; see docs/performance-and-debugging.md.
# BENCH filters benchmarks by name, e.g. `make bench BENCH=find_all/email`.
BENCH        ?=
BASELINE     ?= main
PROFILE_SECS ?= 10
LOG          ?= matcha=debug
# e.g. SAMPLY_FLAGS="--save-only -o profile.json.gz" to save instead of opening a browser.
SAMPLY_FLAGS ?=
PROFILE_DIR  := $(TARGET_DIR)/profiles
BENCH_NAME   := $(or $(subst /,-,$(BENCH)),all)
# dtrace, which flamegraph uses on macOS, needs root; perf on Linux does not.
FLAMEGRAPH   := cargo flamegraph --profile profiling $(if $(filter Darwin,$(UNAME)),--root)
DEBUGGER     ?= $(if $(filter Darwin,$(UNAME)),rust-lldb,rust-gdb)

# Packaging options, passed through to the scripts, e.g. `make dmg UNIVERSAL=1`.
export UNIVERSAL MACOS_SIGN_IDENTITY APPIMAGETOOL APPIMAGETOOL_VERSION

.DEFAULT_GOAL := help
.PHONY: help fmt fmt-check check lint test ci fetch run build release icons \
	bench bench-save bench-compare profiling-build profile profile-bench flamegraph \
	flamegraph-bench run-debug debugger tools \
	install uninstall install-app install-deb install-local uninstall-app \
	uninstall-deb uninstall-local app dmg appimage deb deps-linux release-check release-notes \
	release-publish clean

help: ## Show this help
	@awk 'BEGIN {FS = ":.*## "} /^[a-z-]+:.*## / {printf "  %-16s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

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

fetch: ## Download dependencies; fails if Cargo.lock is out of date
	cargo fetch --locked

run: ## Run a debug build
	cargo run

build: ## Build a debug binary
	cargo build

release: ## Build an optimized binary
	cargo build --release

bench: ## Run the benchmarks (BENCH=find_all/email runs a subset)
	cargo bench --bench search -- $(BENCH)

bench-save: ## Run the benchmarks and save them as BASELINE (default main)
	cargo bench --bench search -- --save-baseline $(BASELINE) $(BENCH)

bench-compare: ## Run the benchmarks and compare them with BASELINE
	cargo bench --bench search -- --baseline $(BASELINE) $(BENCH)

profiling-build: ## Build an optimized binary with symbols, for profilers
	cargo build --profile profiling

profile: profiling-build ## Profile the app with samply; use it, quit, and the profile opens
	samply record $(SAMPLY_FLAGS) $(TARGET_DIR)/profiling/matcha

profile-bench: ## Profile benchmarks with samply for PROFILE_SECS each (BENCH=filter)
	samply record $(SAMPLY_FLAGS) cargo bench --profile profiling --bench search -- \
		--profile-time $(PROFILE_SECS) $(BENCH)

flamegraph: ## Flamegraph of the app: use it, quit, then open PROFILE_DIR/app.svg
	mkdir -p $(PROFILE_DIR)
	$(FLAMEGRAPH) --bin matcha -o $(PROFILE_DIR)/app.svg

flamegraph-bench: ## Flamegraph of benchmarks, PROFILE_SECS each (BENCH=filter)
	mkdir -p $(PROFILE_DIR)
	$(FLAMEGRAPH) --bench search -o $(PROFILE_DIR)/bench-$(BENCH_NAME).svg -- \
		--bench --profile-time $(PROFILE_SECS) $(BENCH)

run-debug: ## Run a debug build with logs and backtraces (LOG=matcha=trace for more)
	RUST_LOG=$(LOG) RUST_BACKTRACE=1 cargo run

debugger: build ## Start the debug build under rust-lldb (macOS) or rust-gdb (Linux)
	$(DEBUGGER) $(TARGET_DIR)/debug/matcha

tools: ## Install the profiling tools: cargo-flamegraph and samply
	cargo install --locked flamegraph samply

icons: ## Regenerate assets/icons from assets/icon.svg
	./scripts/build-icons.sh

install: install-$(INSTALL_METHOD) ## Install for this platform: .deb on Debian/Ubuntu, .app on macOS, else PREFIX

uninstall: uninstall-$(INSTALL_METHOD) ## Remove what `make install` installed

# apt downloads as the unprivileged _apt user, which cannot read a checkout under
# a private home directory, so install from a world-readable copy.
install-deb: deb ## Install the .deb with apt (Debian/Ubuntu)
	tmp=$$(mktemp -d) && trap 'rm -rf "$$tmp"' EXIT && chmod 755 "$$tmp" && \
		deb=matcha_$(VERSION)-1_$$(dpkg --print-architecture).deb && \
		cp "dist/$$deb" "$$tmp/$$deb" && chmod 644 "$$tmp/$$deb" && \
		$(SUDO) apt-get install -y "$$tmp/$$deb"

uninstall-deb: ## Remove the matcha .deb package
	$(SUDO) apt-get remove -y matcha

install-app: app ## Copy dist/Matcha.app to APPS_DIR (default /Applications) (macOS)
	rm -rf "$(APPS_DIR)/Matcha.app"
	cp -R dist/Matcha.app "$(APPS_DIR)/Matcha.app"

uninstall-app: ## Remove Matcha.app from APPS_DIR (macOS)
	rm -rf "$(APPS_DIR)/Matcha.app"

install-local: release ## Install binary, icon, and launcher entry under PREFIX/DESTDIR (Linux)
	install -Dm755 $(TARGET_DIR)/release/matcha $(DESTDIR)$(PREFIX)/bin/matcha
	install -d $(DESTDIR)$(PREFIX)/share/applications
	sed 's|^Exec=.*|Exec=$(PREFIX)/bin/matcha|' assets/$(APP_ID).desktop \
		> $(DESTDIR)$(PREFIX)/share/applications/$(APP_ID).desktop
	for size in $(ICON_SIZES); do \
		install -Dm644 assets/icons/matcha-$$size.png \
			$(DESTDIR)$(PREFIX)/share/icons/hicolor/$${size}x$${size}/apps/$(APP_ID).png; \
	done

uninstall-local: ## Remove what `make install-local` installed
	rm -f $(DESTDIR)$(PREFIX)/bin/matcha $(DESTDIR)$(PREFIX)/share/applications/$(APP_ID).desktop
	for size in $(ICON_SIZES); do \
		rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/$${size}x$${size}/apps/$(APP_ID).png; \
	done

app: ## Build dist/Matcha.app (macOS)
	SKIP_DMG=1 ./scripts/package-macos.sh

dmg: ## Build dist/Matcha.app and a .dmg (macOS; UNIVERSAL=1 adds Intel)
	./scripts/package-macos.sh

appimage: ## Build a .AppImage in dist/ (Linux)
	./scripts/package-appimage.sh

deb: ## Build a .deb in dist/ (Linux)
	./scripts/package-deb.sh

deps-linux: ## Install the build dependencies (Debian/Ubuntu)
	./scripts/install-deps-linux.sh

release-check: ## Check TAG (e.g. v0.1.0): matches Cargo.toml and CHANGELOG.md, on main
	./scripts/release-check.sh $(TAG)

release-notes: ## Print the CHANGELOG.md section for VERSION (e.g. 0.1.0)
	@./scripts/changelog-notes.sh $(VERSION)

release-publish: ## Create the GitHub release for TAG from dist/ (needs gh)
	./scripts/release-publish.sh $(TAG)

clean: ## Remove build output and dist/
	cargo clean
	rm -rf dist
