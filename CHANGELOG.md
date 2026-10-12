# Changelog

All notable changes to Matcha are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-10-12

### Added

- Live match highlighting as you type, including zero-width matches.
- Matches panel with numbered and named capture groups and their offsets; click a match to select it.
- Replace preview with `$1`, `${name}`, and `$$`.
- Flags: ignore case, multi-line, dot all, extended, and Unicode.
- Two engines: `regex` and `fancy-regex` (look-around and backreferences).
- Error messages that point at the problem in the pattern.
- Pattern library: save, update, and delete patterns with their sample text, replacement, flags, and engine.
- Copy as Rust: a ready-to-paste snippet for the selected engine and flags.
- Explain panel that breaks a pattern into readable parts.
- Light and dark themes that follow the system.
- macOS `.dmg` and Linux AppImage packages.

[Unreleased]: https://github.com/kakilangit/matcha/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/kakilangit/matcha/releases/tag/v0.1.0
