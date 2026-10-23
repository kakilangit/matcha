//! macOS specifics: native window frame, Command shortcuts, a Menlo monospace font.

use std::path::PathBuf;

use gpui::{AnyElement, App, KeyBinding, Window, WindowDecorations, prelude::*};

use matcha::editor::{
    DeleteWordLeft, DocumentEnd, DocumentStart, End, Home, SelectEnd, SelectHome, SelectWordLeft,
    SelectWordRight, WordLeft, WordRight,
};
use matcha::theme::Theme;

pub const MONO_FONT: &str = "Menlo";

/// The system draws the title bar.
pub const WINDOW_DECORATIONS: Option<WindowDecorations> = None;

/// Shows a shortcut in the platform's notation, e.g. `shortcut("⌘S", "Ctrl+S")`.
pub fn shortcut(mac: &'static str, _other: &'static str) -> &'static str {
    mac
}

pub fn bind_navigation_keys(cx: &mut App, ctx: Option<&'static str>) {
    cx.bind_keys([
        KeyBinding::new("alt-left", WordLeft, ctx),
        KeyBinding::new("alt-right", WordRight, ctx),
        KeyBinding::new("alt-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("alt-shift-right", SelectWordRight, ctx),
        KeyBinding::new("alt-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("cmd-left", Home, ctx),
        KeyBinding::new("cmd-right", End, ctx),
        KeyBinding::new("cmd-shift-left", SelectHome, ctx),
        KeyBinding::new("cmd-shift-right", SelectEnd, ctx),
        KeyBinding::new("cmd-up", DocumentStart, ctx),
        KeyBinding::new("cmd-down", DocumentEnd, ctx),
    ]);
}

/// `~/Library/Application Support/Matcha`.
pub fn data_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    Some(home.join("Library/Application Support/Matcha"))
}

/// Wraps the window content; nothing to add, the system frame is used.
pub fn frame(content: impl IntoElement, _window: &Window, _theme: &Theme) -> AnyElement {
    content.into_any_element()
}
