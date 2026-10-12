//! Colors and fonts, chosen from the window's light or dark appearance.
#![expect(
    clippy::unreadable_literal,
    reason = "colors are written as hex RRGGBB(AA) codes, like in CSS"
)]

use gpui::{Global, Hsla, WindowAppearance, rgb, rgba};

#[cfg(target_os = "macos")]
pub const MONO_FONT: &str = "Menlo";
#[cfg(not(target_os = "macos"))]
pub const MONO_FONT: &str = "DejaVu Sans Mono";

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub background: Hsla,
    pub surface: Hsla,
    pub border: Hsla,
    pub border_focused: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub accent: Hsla,
    pub accent_text: Hsla,
    pub error: Hsla,
    pub cursor: Hsla,
    pub selection: Hsla,
    /// Match highlights; consecutive matches alternate so adjacent ones stay distinguishable.
    pub matches: [Hsla; 2],
    /// Marker drawn for zero-width matches.
    pub empty_match: Hsla,
    /// Background of the match currently selected in the matches panel.
    pub match_selected: Hsla,
    pub group: Hsla,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            background: rgb(0xf5f5f2).into(),
            surface: rgb(0xffffff).into(),
            border: rgb(0xd9d9d3).into(),
            border_focused: rgb(0x5b8c3a).into(),
            text: rgb(0x1f2420).into(),
            text_muted: rgb(0x6f756f).into(),
            accent: rgb(0x5b8c3a).into(),
            accent_text: rgb(0xffffff).into(),
            error: rgb(0xc0392b).into(),
            cursor: rgb(0x2f6f1f).into(),
            selection: rgba(0x5b8c3a40).into(),
            matches: [rgba(0xa8d67f99).into(), rgba(0x7fc2d699).into()],
            empty_match: rgb(0xd08b16).into(),
            match_selected: rgba(0xf2c94ccc).into(),
            group: rgb(0x2f6f1f).into(),
        }
    }

    pub fn dark() -> Self {
        Self {
            background: rgb(0x1b1e1b).into(),
            surface: rgb(0x232723).into(),
            border: rgb(0x383d38).into(),
            border_focused: rgb(0x8cc265).into(),
            text: rgb(0xe3e6e1).into(),
            text_muted: rgb(0x8e958d).into(),
            accent: rgb(0x8cc265).into(),
            accent_text: rgb(0x14170f).into(),
            error: rgb(0xff7b6b).into(),
            cursor: rgb(0xb5e08f).into(),
            selection: rgba(0x8cc26550).into(),
            matches: [rgba(0x5d8a3c99).into(), rgba(0x3c708a99).into()],
            empty_match: rgb(0xf0a830).into(),
            match_selected: rgba(0xb8902acc).into(),
            group: rgb(0xb5e08f).into(),
        }
    }

    /// Picks the theme for the system appearance, unless `MATCHA_THEME` is set to
    /// `light` or `dark` (useful on Linux desktops that don't report an appearance).
    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        match std::env::var("MATCHA_THEME").as_deref() {
            Ok("light") => return Self::light(),
            Ok("dark") => return Self::dark(),
            _ => {}
        }
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::light(),
        }
    }
}

impl Global for Theme {}
