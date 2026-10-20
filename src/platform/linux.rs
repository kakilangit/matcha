//! Linux specifics: client-drawn window frame, Ctrl shortcuts, a monospace font.

use std::path::PathBuf;

use gpui::{
    AnyElement, App, CursorStyle, Decorations, FontWeight, Hsla, KeyBinding, MouseButton,
    ResizeEdge, Window, WindowDecorations, div, prelude::*, px,
};

use crate::editor::{
    DeleteWordLeft, DocumentEnd, DocumentStart, Redo, SelectWordLeft, SelectWordRight, WordLeft,
    WordRight,
};
use crate::theme::Theme;

pub const MONO_FONT: &str = "DejaVu Sans Mono";

/// Compositors such as GNOME draw no frame, so Matcha draws its own title bar.
pub const WINDOW_DECORATIONS: Option<WindowDecorations> = Some(WindowDecorations::Client);

/// Shows a shortcut in the platform's notation, e.g. `shortcut("⌘S", "Ctrl+S")`.
pub fn shortcut(_mac: &'static str, other: &'static str) -> &'static str {
    other
}

pub fn bind_navigation_keys(cx: &mut App, ctx: Option<&'static str>) {
    cx.bind_keys([
        KeyBinding::new("ctrl-left", WordLeft, ctx),
        KeyBinding::new("ctrl-right", WordRight, ctx),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, ctx),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("ctrl-home", DocumentStart, ctx),
        KeyBinding::new("ctrl-end", DocumentEnd, ctx),
        KeyBinding::new("ctrl-y", Redo, ctx),
    ]);
}

/// `$XDG_DATA_HOME/matcha`, or `~/.local/share/matcha`.
pub fn data_dir() -> Option<PathBuf> {
    match std::env::var_os("XDG_DATA_HOME") {
        Some(data) if !data.is_empty() => Some(PathBuf::from(data).join("matcha")),
        _ => {
            let home = std::env::var_os("HOME").map(PathBuf::from)?;
            Some(home.join(".local/share/matcha"))
        }
    }
}

/// Wraps the window content with a title bar and resize handles, unless the
/// compositor draws its own frame.
pub fn frame(content: impl IntoElement, window: &Window, theme: &Theme) -> AnyElement {
    if !matches!(window.window_decorations(), Decorations::Client { .. }) {
        return content.into_any_element();
    }
    div()
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.background)
        .child(title_bar(window, theme))
        .child(div().flex_1().min_h_0().child(content))
        .children(resize_handles())
        .into_any_element()
}

/// Title bar for windows without a compositor-drawn frame: drag area plus window buttons.
fn title_bar(window: &Window, theme: &Theme) -> impl IntoElement + use<> {
    let button = |id: &'static str, label: &'static str, hover: Hsla| {
        div()
            .id(id)
            .w(px(40.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(theme.text_muted)
            .cursor_pointer()
            .hover(|el| el.bg(hover).text_color(theme.text))
            .child(label)
    };
    let controls = window.window_controls();
    div()
        .id("title-bar")
        .flex()
        .items_center()
        .justify_between()
        .h(px(36.))
        .flex_none()
        .bg(theme.surface)
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .id("title-drag")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .pl_4()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .on_mouse_down(MouseButton::Left, |event, window, _| {
                    if event.click_count == 2 {
                        window.zoom_window();
                    } else {
                        window.start_window_move();
                    }
                })
                .child("Matcha"),
        )
        .child(
            div()
                .flex()
                .h_full()
                .when(controls.minimize, |el| {
                    el.child(
                        button("win-min", "–", theme.border).on_click(|_, window, _| {
                            window.minimize_window();
                        }),
                    )
                })
                .when(controls.maximize, |el| {
                    el.child(
                        button("win-max", "▢", theme.border).on_click(|_, window, _| {
                            window.zoom_window();
                        }),
                    )
                })
                .child(button("win-close", "✕", theme.error).on_click(|_, _, cx| cx.quit())),
        )
}

/// Invisible strips along the window edges and corners that start a compositor resize.
fn resize_handles() -> Vec<AnyElement> {
    const EDGE: f32 = 5.;
    const CORNER: f32 = 10.;
    let handle = |id: &'static str, edge: ResizeEdge, cursor: CursorStyle| {
        div().id(id).absolute().cursor(cursor).on_mouse_down(
            MouseButton::Left,
            move |_, window, _| {
                window.start_window_resize(edge);
            },
        )
    };
    vec![
        handle("rs-top", ResizeEdge::Top, CursorStyle::ResizeUpDown)
            .top_0()
            .left(px(CORNER))
            .right(px(CORNER))
            .h(px(EDGE))
            .into_any_element(),
        handle("rs-bottom", ResizeEdge::Bottom, CursorStyle::ResizeUpDown)
            .bottom_0()
            .left(px(CORNER))
            .right(px(CORNER))
            .h(px(EDGE))
            .into_any_element(),
        handle("rs-left", ResizeEdge::Left, CursorStyle::ResizeLeftRight)
            .left_0()
            .top(px(CORNER))
            .bottom(px(CORNER))
            .w(px(EDGE))
            .into_any_element(),
        handle("rs-right", ResizeEdge::Right, CursorStyle::ResizeLeftRight)
            .right_0()
            .top(px(CORNER))
            .bottom(px(CORNER))
            .w(px(EDGE))
            .into_any_element(),
        handle(
            "rs-tl",
            ResizeEdge::TopLeft,
            CursorStyle::ResizeUpLeftDownRight,
        )
        .top_0()
        .left_0()
        .size(px(CORNER))
        .into_any_element(),
        handle(
            "rs-tr",
            ResizeEdge::TopRight,
            CursorStyle::ResizeUpRightDownLeft,
        )
        .top_0()
        .right_0()
        .size(px(CORNER))
        .into_any_element(),
        handle(
            "rs-bl",
            ResizeEdge::BottomLeft,
            CursorStyle::ResizeUpRightDownLeft,
        )
        .bottom_0()
        .left_0()
        .size(px(CORNER))
        .into_any_element(),
        handle(
            "rs-br",
            ResizeEdge::BottomRight,
            CursorStyle::ResizeUpLeftDownRight,
        )
        .bottom_0()
        .right_0()
        .size(px(CORNER))
        .into_any_element(),
    ]
}
