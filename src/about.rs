//! The About window: icon, version and commit, update status, and Copy and Close buttons.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, ClipboardItem, Context, FontWeight, Image, ImageFormat, MouseButton,
    Render, Subscription, Task, TitlebarOptions, Window, WindowBounds, WindowOptions, div, img,
    point, prelude::*, px, size,
};

use crate::platform::{self, MONO_FONT};
use crate::theme::{self, Theme};
use crate::update::{self, ReleaseSource};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const COMMIT: &str = env!("GIT_COMMIT");
const ICON: &[u8] = include_bytes!("../assets/icons/matcha-256.png");

enum UpdateStatus {
    NotChecked,
    Checking,
    UpToDate,
    Available(String),
    Failed,
}

pub struct AboutView {
    icon: Arc<Image>,
    releases: Arc<dyn ReleaseSource>,
    status: UpdateStatus,
    /// The running update check; replacing it cancels the previous one.
    check: Task<()>,
    _theme: Subscription,
}

impl AboutView {
    /// Opens the About window, or brings the open one to the front, and
    /// optionally starts an update check against `releases`.
    pub fn open(check: bool, releases: &Arc<dyn ReleaseSource>, cx: &mut App) {
        let existing = cx
            .windows()
            .into_iter()
            .find_map(|window| window.downcast::<AboutView>());
        if let Some(window) = existing {
            window
                .update(cx, |view, window, cx| {
                    window.activate_window();
                    if check {
                        view.check_for_updates(cx);
                    }
                })
                .ok();
            return;
        }

        let bounds = Bounds::centered(None, size(px(300.), px(360.)), cx);
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
            |window, cx| {
                theme::follow_appearance(window);
                cx.new(|cx| {
                    let mut view = AboutView {
                        icon: Arc::new(Image::from_bytes(ImageFormat::Png, ICON.to_vec())),
                        releases: releases.clone(),
                        status: UpdateStatus::NotChecked,
                        check: Task::ready(()),
                        _theme: cx.observe_global::<Theme>(|_, cx| cx.notify()),
                    };
                    if check {
                        view.check_for_updates(cx);
                    }
                    view
                })
            },
        );
        if let Err(err) = opened {
            log::error!("could not open the About window: {err:#}");
        }
    }

    fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        self.status = UpdateStatus::Checking;
        let releases = self.releases.clone();
        self.check = cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { update::newer_version(releases.as_ref(), VERSION) })
                .await;
            this.update(cx, |this, cx| {
                this.status = match result {
                    Ok(Some(version)) => UpdateStatus::Available(version),
                    Ok(None) => UpdateStatus::UpToDate,
                    Err(err) => {
                        log::warn!("update check failed: {err}");
                        UpdateStatus::Failed
                    }
                };
                cx.notify();
            })
            .ok();
        });
        cx.notify();
    }

    fn render_status(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let link = |id: &'static str, label: String| {
            div()
                .id(id)
                .text_sm()
                .cursor_pointer()
                .text_color(theme.accent)
                .hover(gpui::Styled::underline)
                .child(label)
        };
        let note = |label: &'static str| div().text_sm().text_color(theme.text_muted).child(label);
        match &self.status {
            UpdateStatus::NotChecked => link("about-check", "Check for updates".into())
                .on_click(cx.listener(|this, _, _, cx| this.check_for_updates(cx)))
                .into_any_element(),
            UpdateStatus::Checking => note("Checking for updates…").into_any_element(),
            UpdateStatus::UpToDate => note("Matcha is up to date").into_any_element(),
            UpdateStatus::Available(version) => {
                link("about-download", format!("Version {version} is available"))
                    .on_click(|_, _, cx| cx.open_url(update::LATEST_RELEASE_PAGE))
                    .into_any_element()
            }
            UpdateStatus::Failed => link("about-retry", "Couldn't check for updates. Retry".into())
                .text_color(theme.error)
                .on_click(cx.listener(|this, _, _, cx| this.check_for_updates(cx)))
                .into_any_element(),
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
                    )
                    .child(div().mt_2().child(self.render_status(&theme, cx))),
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
