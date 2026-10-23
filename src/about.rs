//! The About window: icon, version and commit, with Copy and Close buttons.

use std::sync::Arc;

use gpui::{
    App, Bounds, ClipboardItem, Context, FontWeight, Image, ImageFormat, MouseButton, Render,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, img, point, prelude::*, px, size,
};

use crate::platform::{self, MONO_FONT};
use crate::theme::Theme;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const COMMIT: &str = env!("GIT_COMMIT");
const ICON: &[u8] = include_bytes!("../assets/icons/matcha-256.png");

pub struct AboutView {
    icon: Arc<Image>,
}

impl AboutView {
    /// Opens the About window, or brings the open one to the front.
    pub fn open(cx: &mut App) {
        let existing = cx
            .windows()
            .into_iter()
            .find_map(|window| window.downcast::<AboutView>());
        if let Some(window) = existing {
            window
                .update(cx, |_, window, _| window.activate_window())
                .ok();
            return;
        }

        let bounds = Bounds::centered(None, size(px(300.), px(340.)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: None,
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(12.), px(12.))),
                }),
                is_resizable: false,
                is_minimizable: false,
                window_decorations: platform::WINDOW_DECORATIONS,
                app_id: Some("dev.kakilangit.matcha".into()),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| AboutView {
                    icon: Arc::new(Image::from_bytes(ImageFormat::Png, ICON.to_vec())),
                })
            },
        );
        if let Err(err) = opened {
            log::error!("could not open the About window: {err:#}");
        }
    }
}

impl Render for AboutView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.global::<Theme>();
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px_4()
                .py_1()
                .rounded_md()
                .text_sm()
                .cursor_pointer()
                .bg(theme.border)
                .hover(|el| el.bg(theme.text_muted).text_color(theme.background))
                .child(label)
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .bg(theme.background)
            .text_color(theme.text)
            .child(
                // Linux has no title bar here, so the content doubles as the drag area.
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        window.start_window_move();
                    })
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Matcha"),
                    )
                    .child(img(self.icon.clone()).size(px(96.)).my_2())
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child(format!("Version {VERSION}")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_family(MONO_FONT)
                            .text_color(theme.text_muted)
                            .child(COMMIT),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .pb_6()
                    .child(button("about-copy", "Copy").on_click(|_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(COMMIT.to_owned()));
                    }))
                    .child(
                        button("about-close", "Close")
                            .on_click(|_, window, _| window.remove_window()),
                    ),
            )
    }
}
