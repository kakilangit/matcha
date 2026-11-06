<p align="center">
  <img src="assets/icons/matcha-256.png" alt="Matcha icon" width="128">
</p>

<h1 align="center">Matcha</h1>

<p align="center">
  <strong>The regex workbench that keeps up with you.</strong><br>
  Write a pattern and watch the matches, groups, and replacements update as you type.
</p>

<p align="center">
  <a href="https://github.com/kakilangit/matcha/releases/latest"><img src="https://img.shields.io/github/v/release/kakilangit/matcha?label=download" alt="Latest release"></a>
  <img src="https://img.shields.io/badge/macOS%20%26%20Linux-native-black" alt="Native on macOS and Linux">
  <img src="https://img.shields.io/badge/price-free-brightgreen" alt="Free">
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/kakilangit/static/refs/heads/main/matcha/matcha_screenshot.png" alt="Matcha showing live matches, named capture groups, and a replace preview" width="800">
</p>

---

Regular expressions are easy to get wrong and hard to read back. Matcha is a fast desktop app for
getting them right. Paste in some sample text, write a pattern, and see exactly what it matches,
what each group captures, and what a replacement produces, right as you type. No browser tab, no
waiting, and your text stays on your machine.

## Why Matcha

- **Instant.** Every keystroke updates the results, even on large samples, and typing never lags.
- **Understandable.** Explain breaks any pattern into plain-language parts. Hover a part to see
  what it matches.
- **Ready to use.** Copy a working pattern straight into your code as a Rust snippet.
- **Native.** A real desktop app for macOS and Linux, not a web page in a window.
- **Free.**

## What it does

### See every match

- Matches are highlighted live, including zero-width matches
- Numbered and named capture groups, with their offsets; click a match to select it
- Clear error messages that point at the exact spot that's wrong

### Try replacements safely

- Preview the result before you use it, with `$1`, `${name}`, and `$$`

### Pick the right engine

- **Standard:** fast on any input, the right choice for most patterns
- **Advanced:** adds look-around and backreferences when you need them
- Type a pattern that needs Advanced, and Matcha offers to switch
- Flags: ignore case (`i`), multi-line (`m`), dot matches newline (`s`), verbose (`x`), and Unicode (`u`)

### Keep what works

- The **Pattern library** saves each pattern with its sample text, replacement, flags, and engine
- **Copy as Rust** gives you a ready-to-paste snippet for the engine and flags you picked
- Light and dark themes follow your system

## Install

### macOS

With [Homebrew](https://brew.sh):

```sh
brew install --cask kakilangit/tap/matcha-app
```

Or download `Matcha-<version>-universal.dmg` from
[Releases](https://github.com/kakilangit/matcha/releases/latest) and drag Matcha to Applications.
Runs on Apple silicon and Intel Macs.

> [!NOTE]
> Matcha isn't notarized yet, so macOS blocks it at first. Homebrew takes care of that for you.
> If you installed from the `.dmg`, run:
>
> ```sh
> xattr -cr /Applications/Matcha.app
> ```

### Linux

On Debian or Ubuntu, install the `.deb` (`amd64` or `arm64`) from
[Releases](https://github.com/kakilangit/matcha/releases/latest). It adds Matcha to your app
launcher:

```sh
sudo apt install ./matcha_<version>-1_amd64.deb
```

On any other distribution, use the AppImage (x86_64 or aarch64):

```sh
chmod +x Matcha-<version>-x86_64.AppImage
./Matcha-<version>-x86_64.AppImage
```

Both need glibc 2.35 or newer (Ubuntu 22.04 or later, for example).

## Feedback

Matcha is young and improving fast. Bug reports and ideas are welcome in
[Issues](https://github.com/kakilangit/matcha/issues).

## License

Free to use. See [LICENSE](LICENSE).
