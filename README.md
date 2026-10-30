<p align="center">
  <img src="icon.png" alt="Matcha icon" width="128">
</p>

# Matcha

A native regex workbench for macOS and Linux, built with Rust and [GPUI](https://www.gpui.rs/). Matches, groups, and replacements update as you type.

[![Latest release](https://img.shields.io/github/v/release/kakilangit/matcha)](https://github.com/kakilangit/matcha/releases/latest)

> **Status:** early development.

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
  brew install --cask kakilangit/tap/matcha-app
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
  Or, on Debian/Ubuntu, install the `.deb` (`amd64` or `arm64`), which adds a launcher entry and icon:
  ```sh
  sudo apt install ./matcha_<version>-1_amd64.deb
  ```

## Feedback

Report bugs and ideas in [Issues](https://github.com/kakilangit/matcha/issues).

## License

See [LICENSE](LICENSE).
