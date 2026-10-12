mod app;
mod editor;
mod engine;
mod theme;

use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, actions, prelude::*, px, size,
};

use crate::app::Matcha;
use crate::theme::Theme;

actions!(matcha, [Quit]);

fn main() {
    env_logger::init();
    Application::new().run(|cx: &mut App| {
        cx.set_global(Theme::for_appearance(cx.window_appearance()));
        editor::bind_keys(cx);
        app::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("secondary-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.observe_keystrokes(|event, _, _| {
            log::debug!("keystroke: {}", event.keystroke.unparse())
        })
        .detach();
        cx.set_menus(vec![Menu {
            name: "Matcha".into(),
            items: vec![MenuItem::action("Quit Matcha", Quit)],
        }]);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Matcha".into()),
                        ..Default::default()
                    }),
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
            )
            .expect("failed to open the Matcha window");
        window
            .update(cx, |_, _, cx| cx.activate(true))
            .expect("window was just opened");
    });
}
