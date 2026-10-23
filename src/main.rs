mod about;
mod app;
mod codegen;
mod editor;
mod engine;
mod explain;
mod library;
mod platform;
mod theme;

use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, actions, prelude::*, px, size,
};

use crate::about::AboutView;
use crate::app::Matcha;
use crate::theme::Theme;

actions!(matcha, [Quit, About]);

fn main() {
    env_logger::init();
    Application::new().run(|cx: &mut App| {
        cx.set_global(Theme::for_appearance(cx.window_appearance()));
        editor::bind_keys(cx);
        app::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("secondary-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &About, cx| AboutView::open(cx));
        cx.observe_keystrokes(|event, _, _| {
            log::debug!("keystroke: {}", event.keystroke.unparse());
        })
        .detach();
        cx.set_menus(vec![Menu {
            name: "Matcha".into(),
            items: vec![
                MenuItem::action("About Matcha", About),
                MenuItem::separator(),
                MenuItem::action("Quit Matcha", Quit),
            ],
        }]);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
        let window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Matcha".into()),
                    ..Default::default()
                }),
                window_decorations: platform::WINDOW_DECORATIONS,
                app_id: Some("dev.kakilangit.matcha".into()),
                window_min_size: Some(size(px(720.), px(480.))),
                ..Default::default()
            },
            |window, cx| {
                window
                    .observe_window_appearance(|window, cx| {
                        cx.set_global(Theme::for_appearance(window.appearance()));
                        window.refresh();
                    })
                    .detach();
                cx.new(|cx| Matcha::new(window, cx))
            },
        );
        match window {
            Ok(window) => {
                window.update(cx, |_, _, cx| cx.activate(true)).ok();
            }
            Err(err) => {
                log::error!("could not open the Matcha window: {err:#}");
                cx.quit();
            }
        }
    });
}
