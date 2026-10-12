# Matcha

A fast, native desktop app for building, testing, and debugging regular expressions — written in Rust and built on [GPUI](https://www.gpui.rs/).

Matcha is meant as a modern, open-source, cross-platform replacement for [RegExRX](https://www.mactechnologies.com/index.php?page=downloads#regexrx). Write a pattern, paste some text, and see matches, capture groups, and replacements update as you type.

> **Status:** early development. The features below are the planned scope; nothing is released yet.

## Features

- **Live matching.** Matches are highlighted in the sample text as you edit the pattern or the text.
- **Capture group inspector.** Shows every match with its numbered and named groups, plus their byte and character offsets.
- **Replace preview.** Write a replacement template (`$1`, `${name}`, `$$`) and see the result next to the original.
- **Flags.** Toggle case-insensitive (`i`), multi-line (`m`), dot-matches-newline (`s`), extended/verbose (`x`), and Unicode (`u`).
- **Two engines:**
  - [`regex`](https://crates.io/crates/regex), the default: linear-time and safe on untrusted input.
  - [`fancy-regex`](https://crates.io/crates/fancy-regex), for look-around and backreferences.
- **Clear errors.** Invalid patterns show the engine's error message with the failing position marked.
- **Pattern library.** Save, name, and reopen patterns together with their sample text and replacement.
- **Code export.** Copy the pattern as a ready-to-use snippet (Rust first, other languages later).
- **Native and GPU-accelerated.** GPUI renders the UI, so it stays responsive on large inputs.

## Why

RegExRX is a well-loved tool, but it is built for Intel (x86) Macs. Apple is moving macOS entirely to Apple silicon (ARM) and phasing out x86 support, so RegExRX won't run on upcoming macOS versions. It is also macOS-only and closed source. Matcha keeps the same workflow (pattern, text, matches, replacement in one window) and adds:

- native builds for Apple silicon, with no Rosetta needed
- Rust's regex engines, which behave predictably
- macOS and Linux support (Windows when GPUI supports it)
- an open codebase that is easy to extend

## Building from source

### Prerequisites

- Rust stable (latest), installed via [rustup](https://rustup.rs/)
- Platform dependencies for GPUI:
  - **macOS:** Xcode and the Command Line Tools (`xcode-select --install`)
  - **Linux:** Vulkan drivers and the usual X11/Wayland development packages, for example on Debian/Ubuntu:
    ```sh
    sudo apt install libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
      libvulkan-dev libfontconfig-dev libssl-dev
    ```

### Build and run

```sh
git clone https://github.com/kakilangit/matcha.git
cd matcha
cargo run --release
```

### Tests

```sh
cargo test
```

## Usage

1. Type a regular expression in the **Pattern** field.
2. Paste or type text in the **Sample** pane. Matches are highlighted immediately.
3. Open the **Matches** panel to inspect each match and its capture groups.
4. Optionally enter a **Replacement** template to preview the substituted output.
5. Save the pattern to your library, or copy it as code.

### Keyboard shortcuts (planned)

| Action              | macOS   | Linux    |
| ------------------- | ------- | -------- |
| Focus pattern       | `⌘L`    | `Ctrl+L` |
| Toggle replace pane | `⌘R`    | `Ctrl+R` |
| Save pattern        | `⌘S`    | `Ctrl+S` |
| Open library        | `⌘O`    | `Ctrl+O` |
| Copy as code        | `⇧⌘C`   | `Ctrl+Shift+C` |

## Roadmap

- [ ] Core window layout: pattern, sample, matches, replacement
- [ ] Live matching with the `regex` crate
- [ ] Capture group inspector
- [ ] Replace preview
- [ ] Flag toggles
- [ ] `fancy-regex` engine option
- [ ] Pattern library (save/load)
- [ ] Export to code snippets
- [ ] Pattern explanation (break a regex into readable parts)
- [ ] Light and dark themes
- [ ] Packaged releases (`.dmg`, AppImage/Flatpak)

## Contributing

Issues and pull requests are welcome. For anything larger than a small fix, please open an issue first to discuss the approach.

Before submitting:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

## Acknowledgements

- [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui), the UI framework from the [Zed](https://zed.dev) team
- [`regex`](https://github.com/rust-lang/regex) and [`fancy-regex`](https://github.com/fancy-regex/fancy-regex)
- [RegExRX](https://www.mactechnologies.com/index.php?page=downloads#regexrx), the inspiration for this project

## License

[MIT](LICENSE) © 2026 Firman Maulana
