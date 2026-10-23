//! Startup: the one place that picks the adapters and opens the main window.

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, actions, prelude::*, px, size,
};

use matcha::about::AboutView;
use matcha::app::{self, Matcha};
use matcha::editor;
use matcha::json_store::JsonFileStore;
use matcha::library::{Library, PatternStore};
use matcha::platform;
use matcha::theme::{self, Theme};
use matcha::update::{GitHubReleases, ReleaseSource};

actions!(matcha, [Quit, About, CheckForUpdates]);

/// Starts the app: wires the adapters into the windows and runs until quit.
pub fn run() {
    env_logger::init();
    log::info!(
        "Matcha {} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_COMMIT")
    );
    Application::new().run(|cx: &mut App| {
        cx.set_global(Theme::for_appearance(cx.window_appearance()));
        editor::bind_keys(cx);
        app::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("secondary-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        let releases: Arc<dyn ReleaseSource> =
            Arc::new(GitHubReleases::new(env!("CARGO_PKG_VERSION")));
        let about_releases = releases.clone();
        cx.on_action(move |_: &About, cx| AboutView::open(false, &about_releases, cx));
        cx.on_action(move |_: &CheckForUpdates, cx| AboutView::open(true, &releases, cx));
        cx.observe_keystrokes(|event, _, _| {
            log::debug!("keystroke: {}", event.keystroke.unparse());
        })
        .detach();
        cx.set_menus(vec![Menu {
            name: "Matcha".into(),
            items: vec![
                MenuItem::action("About Matcha", About),
                MenuItem::action("Check for Updates…", CheckForUpdates),
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
                theme::follow_appearance(window);
                cx.new(|cx| Matcha::new(Library::load(library_store()), window, cx))
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

/// The library file: `MATCHA_LIBRARY` if set, otherwise in the platform data directory.
fn library_store() -> Option<Box<dyn PatternStore>> {
    let path = match std::env::var_os("MATCHA_LIBRARY") {
        Some(path) => PathBuf::from(path),
        None => platform::data_dir()?.join("library.json"),
    };
    Some(Box::new(JsonFileStore::new(path)))
}
