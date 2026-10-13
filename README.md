<p align="center">
  <img src="assets/icons/matcha-256.png" alt="Matcha icon" width="128">
</p>

# Matcha

A native regex workbench for macOS and Linux, built with Rust and [GPUI](https://www.gpui.rs/). Matches, groups, and replacements update as you type.

> **Status:** early development. Latest release: [v0.1.0](https://github.com/kakilangit/matcha/releases/latest).

<p align="center">
  <img src="https://raw.githubusercontent.com/kakilangit/static/refs/heads/main/matcha/matcha_screenshot.png" alt="Matcha showing live matches, named capture groups, and a replace preview" width="800">
</p>

## Features

- Live match highlighting, including zero-width matches
- Numbered and named capture groups with offsets; click a match to select it
- Replace preview with `$1`, `${name}`, and `$$`
- Flags: `i`, `m`, `s`, `x`, `u`
- Two engines: [`regex`](https://crates.io/crates/regex) (linear time) and [`fancy-regex`](https://crates.io/crates/fancy-regex) (look-around, backreferences)
- Clear error messages that point at the problem
- Pattern library: save patterns with their sample text, replacement, flags, and engine
- Copy as Rust: a ready-to-paste snippet for the selected engine and flags
- Explain: breaks a pattern into readable parts; hover a part to highlight it
- Light and dark themes; override with `MATCHA_THEME=light|dark`

## Install

- **macOS** (Apple silicon and Intel), with [Homebrew](https://brew.sh):
  ```sh
  brew install --cask kakilangit/tap/matcha
  ```
  Or download `Matcha-<version>-universal.dmg` from [Releases](https://github.com/kakilangit/matcha/releases/latest) and drag Matcha to Applications. The app is not notarized yet, so macOS blocks it until you clear the quarantine flag (Homebrew does this for you):
  ```sh
  xattr -cr /Applications/Matcha.app
  ```
- **Linux**, from [Releases](https://github.com/kakilangit/matcha/releases/latest) (x86_64 or aarch64, glibc 2.35+, e.g. Ubuntu 22.04 or newer):
  ```sh
  chmod +x Matcha-<version>-x86_64.AppImage
  ./Matcha-<version>-x86_64.AppImage
  ```
  To build and install from source with a launcher entry instead, see [Build](#build).

## Build

Requires Rust stable ([rustup](https://rustup.rs/)).

- **macOS:** Xcode Command Line Tools (`xcode-select --install`)
- **Linux (Debian/Ubuntu):** `make deps-linux`

```sh
git clone https://github.com/kakilangit/matcha.git
cd matcha
make run
```

Run `make` to list all targets.

> `Cargo.lock` pins `libc` to 0.2.189 because a GPUI dependency fails to build with 0.2.190 on Linux. Don't run `cargo update -p libc` yet.

## Packaging

| Command         | Output                                   | Platform |
| --------------- | ---------------------------------------- | -------- |
| `make dmg`      | `dist/Matcha.app`, `dist/Matcha-<version>-<arch>.dmg` | macOS |
| `make app`      | `dist/Matcha.app` only                   | macOS    |
| `make appimage` | `dist/Matcha-<version>-<arch>.AppImage`  | Linux    |
| `make install`  | binary, icon, and launcher entry under `PREFIX` (default `~/.local`) | Linux |

- **macOS:** `UNIVERSAL=1 make dmg` builds for Apple silicon and Intel. Without `MACOS_SIGN_IDENTITY` the app is signed ad hoc, so other Macs will block it until it is signed with a Developer ID and notarized.
- **Linux:** the AppImage needs `file` and downloads `appimagetool` 1.9.1 on first use. It uses the build machine's glibc, so build on the oldest distribution you want to support.

## Releasing

CI only calls `make` targets, so every step can be run locally the same way. It runs `make ci` on every pull request to `main` and every push to `main`; pushes to `main` also build the packages as workflow artifacts.

To publish a release:

1. Set `version` in `Cargo.toml`, and move the `[Unreleased]` notes in `CHANGELOG.md` under a heading for that version.
2. Merge to `main`, then check, tag, and push from the merged commit:
   ```sh
   git fetch origin main
   make release-check TAG=v0.1.0
   git tag v0.1.0 && git push origin v0.1.0
   ```

The release workflow runs the same `make release-check`, builds the `.dmg` and AppImages, and publishes them with the version's changelog section as the notes (`make release-notes VERSION=0.1.0` shows it).

## Library

Saved patterns live in `~/Library/Application Support/Matcha/library.json` on macOS and `~/.local/share/matcha/library.json` on Linux. Set `MATCHA_LIBRARY` to use another file.

## Shortcuts

| Action           | macOS          | Linux                    |
| ---------------- | -------------- | ------------------------ |
| Focus pattern    | `⌘L`           | `Ctrl+L`                 |
| Toggle replace   | `⌘R`           | `Ctrl+R`                 |
| Save / save as   | `⌘S` / `⇧⌘S`   | `Ctrl+S` / `Ctrl+Shift+S` |
| Library          | `⌘O`           | `Ctrl+O`                 |
| New pattern      | `⌘N`           | `Ctrl+N`                 |
| Copy as Rust     | `⇧⌘C`          | `Ctrl+Shift+C`           |
| Matches / explain | `⌘E`          | `Ctrl+E`                 |
| Next field       | `Tab`          | `Tab`                    |
| Undo / redo      | `⌘Z` / `⇧⌘Z`   | `Ctrl+Z` / `Ctrl+Y`      |
| Quit             | `⌘Q`           | `Ctrl+Q`                 |

## Roadmap

- [x] Live matching, capture groups, replace preview
- [x] Flags and two engines
- [x] Light and dark themes
- [x] Pattern library
- [x] Copy as Rust code
- [x] Pattern explanation
- [x] Packaging: `.dmg` and AppImage
- [x] Published releases built by CI

## Contributing

Issues and pull requests are welcome. Before submitting, run:

```sh
make ci
```

This checks formatting and runs clippy with the pedantic lints, which also reject `unwrap`, `expect`, and `panic!` outside tests.

## License

[MIT](LICENSE) © 2026 Firman Maulana
