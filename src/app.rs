//! The main window: pattern, sample text, matches panel and replace preview.

use std::time::Duration;

use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Context, Entity, FocusHandle, Focusable,
    FontWeight, KeyBinding, MouseButton, ScrollHandle, SharedString, Subscription, Task, Window,
    actions, div, prelude::*, px,
};

use crate::codegen;
use crate::editor::{Editor, EditorEvent, Highlight};
use crate::engine::{EngineKind, Flags, MATCH_LIMIT, Matches, Regex};
use crate::library::{self, Library, PatternData};
use crate::theme::{MONO_FONT, Theme};

actions!(
    matcha,
    [
        FocusPattern,
        ToggleReplace,
        FocusNext,
        FocusPrevious,
        SavePattern,
        SavePatternAs,
        NewPattern,
        ToggleLibrary,
        CopyAsCode,
    ]
);

const KEY_CONTEXT: &str = "Matcha";

/// Matches listed in the panel; the rest are still highlighted in the sample.
const LISTED_MATCHES: usize = 500;
const PREVIEW_CHARS: usize = 60;
const NOTICE_DURATION: Duration = Duration::from_secs(3);

/// Shows a shortcut in the platform's notation, e.g. `shortcut("⌘S", "Ctrl+S")`.
fn shortcut(mac: &'static str, other: &'static str) -> &'static str {
    if cfg!(target_os = "macos") {
        mac
    } else {
        other
    }
}

pub fn bind_keys(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("secondary-l", FocusPattern, ctx),
        KeyBinding::new("secondary-r", ToggleReplace, ctx),
        KeyBinding::new("tab", FocusNext, ctx),
        KeyBinding::new("shift-tab", FocusPrevious, ctx),
        KeyBinding::new("secondary-s", SavePattern, ctx),
        KeyBinding::new("secondary-shift-s", SavePatternAs, ctx),
        KeyBinding::new("secondary-n", NewPattern, ctx),
        KeyBinding::new("secondary-o", ToggleLibrary, ctx),
        KeyBinding::new("secondary-shift-c", CopyAsCode, ctx),
    ]);
}

enum Outcome {
    /// No pattern entered yet.
    Empty,
    Error(String),
    Found(Matches),
}

#[derive(Clone, Copy)]
enum FlagKind {
    CaseInsensitive,
    MultiLine,
    DotAll,
    Extended,
    Unicode,
}

impl FlagKind {
    const ALL: [FlagKind; 5] = [
        FlagKind::CaseInsensitive,
        FlagKind::MultiLine,
        FlagKind::DotAll,
        FlagKind::Extended,
        FlagKind::Unicode,
    ];

    fn letter(self) -> &'static str {
        match self {
            FlagKind::CaseInsensitive => "i",
            FlagKind::MultiLine => "m",
            FlagKind::DotAll => "s",
            FlagKind::Extended => "x",
            FlagKind::Unicode => "u",
        }
    }

    fn label(self) -> &'static str {
        match self {
            FlagKind::CaseInsensitive => "ignore case",
            FlagKind::MultiLine => "multi-line",
            FlagKind::DotAll => "dot all",
            FlagKind::Extended => "extended",
            FlagKind::Unicode => "unicode",
        }
    }

    fn get(self, flags: &mut Flags) -> &mut bool {
        match self {
            FlagKind::CaseInsensitive => &mut flags.case_insensitive,
            FlagKind::MultiLine => &mut flags.multi_line,
            FlagKind::DotAll => &mut flags.dot_matches_new_line,
            FlagKind::Extended => &mut flags.ignore_whitespace,
            FlagKind::Unicode => &mut flags.unicode,
        }
    }
}

pub struct Matcha {
    focus_handle: FocusHandle,
    pattern: Entity<Editor>,
    sample: Entity<Editor>,
    replacement: Entity<Editor>,
    result: Entity<Editor>,
    sample_scroll: ScrollHandle,
    result_scroll: ScrollHandle,
    flags: Flags,
    engine: EngineKind,
    show_replace: bool,
    outcome: Outcome,
    replace_error: Option<String>,
    selected_match: Option<usize>,
    library: Library,
    /// The library entry the workbench was loaded from or last saved to.
    current: Option<u64>,
    show_library: bool,
    /// Name field shown while saving a new library entry.
    name_editor: Entity<Editor>,
    naming: bool,
    /// Entry whose delete button was clicked once and awaits confirmation.
    pending_delete: Option<u64>,
    /// Short-lived message in the status bar, and whether it is an error.
    notice: Option<(String, bool)>,
    _notice_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Matcha {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sample_scroll = ScrollHandle::new();
        let result_scroll = ScrollHandle::new();
        let pattern = cx.new(|cx| Editor::new(cx).placeholder("Regular expression"));
        let sample = cx.new(|cx| {
            Editor::new(cx)
                .placeholder("Text to search")
                .scroll_handle(sample_scroll.clone())
        });
        let replacement =
            cx.new(|cx| Editor::new(cx).placeholder("Replacement, e.g. $1 or ${name}"));
        let result = cx.new(|cx| {
            Editor::new(cx)
                .read_only()
                .scroll_handle(result_scroll.clone())
        });

        let name_editor = cx.new(|cx| Editor::new(cx).single_line().placeholder("Name"));

        let mut subscriptions: Vec<Subscription> = [&pattern, &sample, &replacement]
            .into_iter()
            .map(|editor| {
                cx.subscribe(editor, |this, _, event: &EditorEvent, cx| {
                    if let EditorEvent::Changed = event {
                        this.recompute(cx);
                    }
                })
            })
            .collect();
        subscriptions.push(cx.subscribe_in(
            &name_editor,
            window,
            |this, _, event: &EditorEvent, window, cx| match event {
                EditorEvent::Submit => this.confirm_name(window, cx),
                EditorEvent::Cancel => this.cancel_naming(window, cx),
                EditorEvent::Changed => {}
            },
        ));

        let library = Library::load(library::default_path());
        let show_library = !library.entries().is_empty();

        window.focus(&pattern.focus_handle(cx));

        let mut this = Self {
            focus_handle: cx.focus_handle(),
            pattern,
            sample,
            replacement,
            result,
            sample_scroll,
            result_scroll,
            flags: Flags::default(),
            engine: EngineKind::default(),
            show_replace: true,
            outcome: Outcome::Empty,
            replace_error: None,
            selected_match: None,
            library,
            current: None,
            show_library,
            name_editor,
            naming: false,
            pending_delete: None,
            notice: None,
            _notice_task: None,
            _subscriptions: subscriptions,
        };
        this.recompute(cx);
        this
    }

    fn compile(&self, cx: &App) -> Option<Result<Regex, String>> {
        let pattern = self.pattern.read(cx).text();
        (!pattern.is_empty()).then(|| Regex::new(pattern, self.flags, self.engine))
    }

    /// Re-runs the search and replacement after any input changes.
    fn recompute(&mut self, cx: &mut Context<Self>) {
        let started = std::time::Instant::now();
        let text = self.sample.read(cx).text().to_owned();
        let regex = self.compile(cx);
        self.outcome = match &regex {
            None => Outcome::Empty,
            Some(Err(err)) => Outcome::Error(err.clone()),
            Some(Ok(regex)) => match regex.find_all(&text) {
                Ok(matches) => Outcome::Found(matches),
                Err(err) => Outcome::Error(err),
            },
        };

        self.replace_error = None;
        let replaced = match &regex {
            Some(Ok(regex)) if self.show_replace => {
                let template = self.replacement.read(cx).text();
                regex.replace_all(&text, template).unwrap_or_else(|err| {
                    self.replace_error = Some(err);
                    String::new()
                })
            }
            _ => text,
        };
        self.result
            .update(cx, |editor, cx| editor.set_text(replaced, cx));

        if let (Some(ix), Outcome::Found(found)) = (self.selected_match, &self.outcome)
            && ix >= found.matches.len()
        {
            self.selected_match = None;
        }
        self.update_highlights(cx);
        log::debug!("recompute took {:?}", started.elapsed());
        cx.notify();
    }

    fn update_highlights(&mut self, cx: &mut Context<Self>) {
        let theme = *cx.global::<Theme>();
        let mut highlights = Vec::new();
        let mut markers = Vec::new();
        if let Outcome::Found(found) = &self.outcome {
            for (ix, m) in found.matches.iter().enumerate() {
                if m.range.is_empty() {
                    markers.push(m.range.start);
                } else {
                    let color = if self.selected_match == Some(ix) {
                        theme.match_selected
                    } else {
                        theme.matches[ix % 2]
                    };
                    highlights.push(Highlight {
                        range: m.range.clone(),
                        color,
                    });
                }
            }
        }
        self.sample.update(cx, |editor, cx| {
            editor.set_highlights(highlights, markers, cx)
        });
    }

    fn select_match(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Outcome::Found(found) = &self.outcome else {
            return;
        };
        let Some(range) = found.matches.get(ix).map(|m| m.range.clone()) else {
            return;
        };
        self.selected_match = Some(ix);
        self.sample
            .update(cx, |editor, cx| editor.select_range(range, cx));
        self.update_highlights(cx);
        cx.notify();
    }

    fn toggle_flag(&mut self, flag: FlagKind, cx: &mut Context<Self>) {
        let value = flag.get(&mut self.flags);
        *value = !*value;
        self.recompute(cx);
    }

    fn set_engine(&mut self, engine: EngineKind, cx: &mut Context<Self>) {
        if self.engine != engine {
            self.engine = engine;
            self.recompute(cx);
        }
    }

    fn focus_pattern(&mut self, _: &FocusPattern, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.pattern.focus_handle(cx));
    }

    fn toggle_replace(&mut self, _: &ToggleReplace, window: &mut Window, cx: &mut Context<Self>) {
        self.show_replace = !self.show_replace;
        if !self.show_replace && self.replacement.focus_handle(cx).is_focused(window) {
            window.focus(&self.pattern.focus_handle(cx));
        }
        self.recompute(cx);
    }

    fn copy_as_code(&mut self, _: &CopyAsCode, _: &mut Window, cx: &mut Context<Self>) {
        let pattern = self.pattern.read(cx).text();
        if pattern.is_empty() {
            self.show_notice("Enter a pattern to copy it as code.".into(), true, cx);
            return;
        }
        if Regex::new(pattern, self.flags, self.engine).is_err() {
            self.show_notice(
                "Fix the pattern before copying it as code.".into(),
                true,
                cx,
            );
            return;
        }
        let replacement = if self.show_replace {
            self.replacement.read(cx).text()
        } else {
            ""
        };
        let code = codegen::rust_snippet(pattern, self.flags, self.engine, replacement);
        cx.write_to_clipboard(ClipboardItem::new_string(code));
        let notice = format!("Copied Rust code for {}", self.engine.label());
        self.show_notice(notice, false, cx);
    }

    fn focus_order(&self) -> Vec<&Entity<Editor>> {
        let mut order = vec![&self.pattern, &self.sample];
        if self.show_replace {
            order.push(&self.replacement);
        }
        order
    }

    fn cycle_focus(&self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        let order = self.focus_order();
        let current = order
            .iter()
            .position(|editor| editor.focus_handle(cx).is_focused(window));
        let len = order.len() as isize;
        let next = match current {
            Some(ix) => (ix as isize + step).rem_euclid(len) as usize,
            None => 0,
        };
        window.focus(&order[next].focus_handle(cx));
    }

    fn focus_next(&mut self, _: &FocusNext, window: &mut Window, cx: &mut Context<Self>) {
        self.cycle_focus(1, window, cx);
    }

    fn focus_previous(&mut self, _: &FocusPrevious, window: &mut Window, cx: &mut Context<Self>) {
        self.cycle_focus(-1, window, cx);
    }
}

/// Library actions.
impl Matcha {
    fn current_data(&self, cx: &App) -> PatternData {
        PatternData {
            pattern: self.pattern.read(cx).text().to_owned(),
            sample: self.sample.read(cx).text().to_owned(),
            replacement: self.replacement.read(cx).text().to_owned(),
            flags: self.flags,
            engine: self.engine,
        }
    }

    /// Whether the workbench differs from the saved entry, without copying the texts.
    fn is_modified(&self, data: &PatternData, cx: &App) -> bool {
        data.pattern != self.pattern.read(cx).text()
            || data.sample != self.sample.read(cx).text()
            || data.replacement != self.replacement.read(cx).text()
            || data.flags != self.flags
            || data.engine != self.engine
    }

    fn show_notice(&mut self, text: String, is_error: bool, cx: &mut Context<Self>) {
        self.notice = Some((text, is_error));
        self._notice_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(NOTICE_DURATION).await;
            this.update(cx, |this, cx| {
                this.notice = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Fills the workbench from `data` and re-runs the search.
    fn apply(&mut self, data: &PatternData, cx: &mut Context<Self>) {
        self.pattern
            .update(cx, |editor, cx| editor.set_text(data.pattern.clone(), cx));
        self.sample
            .update(cx, |editor, cx| editor.set_text(data.sample.clone(), cx));
        self.replacement.update(cx, |editor, cx| {
            editor.set_text(data.replacement.clone(), cx)
        });
        self.flags = data.flags;
        self.engine = data.engine;
        self.selected_match = None;
        self.recompute(cx);
    }

    fn save_pattern(&mut self, _: &SavePattern, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.current.filter(|&id| self.library.get(id).is_some()) else {
            self.start_naming(String::new(), window, cx);
            return;
        };
        let data = self.current_data(cx);
        match self.library.update(id, data) {
            Ok(()) => {
                let name = self
                    .library
                    .get(id)
                    .map(|e| e.name.clone())
                    .unwrap_or_default();
                self.show_notice(format!("Saved “{name}”"), false, cx);
            }
            Err(err) => self.show_notice(err, true, cx),
        }
    }

    fn save_pattern_as(&mut self, _: &SavePatternAs, window: &mut Window, cx: &mut Context<Self>) {
        let name = self
            .current
            .and_then(|id| self.library.get(id))
            .map(|entry| format!("{} copy", entry.name))
            .unwrap_or_default();
        self.start_naming(name, window, cx);
    }

    fn start_naming(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.library.load_error().is_some() {
            self.show_library = true;
            self.show_notice(
                "Saving is disabled because the library file can't be read.".into(),
                true,
                cx,
            );
            return;
        }
        self.show_library = true;
        self.naming = true;
        self.pending_delete = None;
        self.name_editor.update(cx, |editor, cx| {
            editor.set_text(name, cx);
            editor.select_all_text(cx);
        });
        window.focus(&self.name_editor.focus_handle(cx));
        cx.notify();
    }

    fn confirm_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name_editor.read(cx).text().trim().to_owned();
        if name.is_empty() {
            return;
        }
        let data = self.current_data(cx);
        match self.library.add(name.clone(), data) {
            Ok(id) => {
                self.current = Some(id);
                self.naming = false;
                window.focus(&self.pattern.focus_handle(cx));
                self.show_notice(format!("Saved “{name}”"), false, cx);
            }
            Err(err) => self.show_notice(err, true, cx),
        }
    }

    fn cancel_naming(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.naming = false;
        window.focus(&self.pattern.focus_handle(cx));
        cx.notify();
    }

    fn load_entry(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(data) = self.library.get(id).map(|entry| entry.data.clone()) else {
            return;
        };
        self.naming = false;
        self.pending_delete = None;
        self.current = Some(id);
        self.apply(&data, cx);
        window.focus(&self.pattern.focus_handle(cx));
    }

    fn delete_entry(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.pending_delete != Some(id) {
            self.pending_delete = Some(id);
            cx.notify();
            return;
        }
        self.pending_delete = None;
        let name = self
            .library
            .get(id)
            .map(|e| e.name.clone())
            .unwrap_or_default();
        match self.library.remove(id) {
            Ok(()) => {
                if self.current == Some(id) {
                    self.current = None;
                }
                self.show_notice(format!("Deleted “{name}”"), false, cx);
            }
            Err(err) => self.show_notice(err, true, cx),
        }
    }

    fn new_pattern(&mut self, _: &NewPattern, window: &mut Window, cx: &mut Context<Self>) {
        self.current = None;
        self.naming = false;
        self.apply(&PatternData::default(), cx);
        window.focus(&self.pattern.focus_handle(cx));
    }

    fn toggle_library(&mut self, _: &ToggleLibrary, window: &mut Window, cx: &mut Context<Self>) {
        self.show_library = !self.show_library;
        if !self.show_library && self.naming {
            self.cancel_naming(window, cx);
        }
        cx.notify();
    }
}

impl Focusable for Matcha {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn section_label(text: impl Into<SharedString>, theme: &Theme) -> gpui::Div {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_muted)
        .child(text.into())
}

/// A bordered text box around an editor, highlighted while the editor has focus.
/// Clicking the box outside the text (its padding or the empty space below)
/// focuses the editor and moves the cursor to the end.
fn editor_box(editor: &Entity<Editor>, window: &Window, cx: &App, theme: &Theme) -> gpui::Div {
    let focused = editor.focus_handle(cx).is_focused(window);
    let editor_handle = editor.clone();
    div()
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            window.focus(&editor_handle.focus_handle(cx));
            // Keep the focusable window root from taking focus back.
            window.prevent_default();
            editor_handle.update(cx, |editor, cx| editor.move_to_end(cx));
        })
        .bg(theme.surface)
        .border_1()
        .border_color(if focused {
            theme.border_focused
        } else {
            theme.border
        })
        .rounded_md()
        .px_3()
        .py_2()
        .font_family(MONO_FONT)
        .text_sm()
        .child(editor.clone())
}

/// Shows text on one line, making newlines and tabs visible and truncating long values.
fn preview(text: &str) -> String {
    let mut out = String::new();
    for (ix, ch) in text.chars().enumerate() {
        if ix == PREVIEW_CHARS {
            out.push('…');
            break;
        }
        match ch {
            '\n' => out.push('⏎'),
            '\t' => out.push('⇥'),
            '\r' => out.push('␍'),
            _ => out.push(ch),
        }
    }
    out
}

impl Matcha {
    fn render_header(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let engine_button = |engine: EngineKind, cx: &mut Context<Self>| {
            let active = self.engine == engine;
            div()
                .id(SharedString::from(engine.label()))
                .px_3()
                .py_1()
                .rounded_md()
                .text_sm()
                .cursor_pointer()
                .when(active, |el| {
                    el.bg(theme.accent).text_color(theme.accent_text)
                })
                .when(!active, |el| el.hover(|el| el.bg(theme.border)))
                .child(engine.label())
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.set_engine(engine, cx)),
                )
        };
        let library_button = div()
            .id("library-toggle")
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_1()
            .rounded_lg()
            .border_1()
            .text_sm()
            .cursor_pointer()
            .when(self.show_library, |el| {
                el.bg(theme.surface)
                    .border_color(theme.accent)
                    .text_color(theme.accent)
            })
            .when(!self.show_library, |el| {
                el.border_color(theme.border)
                    .hover(|el| el.bg(theme.surface))
            })
            .child("Library")
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child(shortcut("⌘O", "Ctrl+O")),
            )
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.toggle_library(&ToggleLibrary, window, cx)
            }));
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.accent)
                    .child("Matcha"),
            )
            .child(
                div().flex().gap_2().child(library_button).child(
                    div()
                        .flex()
                        .gap_1()
                        .p_1()
                        .rounded_lg()
                        .bg(theme.surface)
                        .border_1()
                        .border_color(theme.border)
                        .child(engine_button(EngineKind::Standard, cx))
                        .child(engine_button(EngineKind::Fancy, cx)),
                ),
            )
    }

    fn render_library(
        &self,
        window: &Window,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px_2()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .text_color(theme.text_muted)
                .hover(|el| el.bg(theme.border).text_color(theme.text))
                .child(label)
        };
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(section_label("LIBRARY", theme))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(button("library-new", "New").on_click(cx.listener(
                        |this, _: &ClickEvent, window, cx| {
                            this.new_pattern(&NewPattern, window, cx)
                        },
                    )))
                    .child(button("library-save", "Save").on_click(cx.listener(
                        |this, _: &ClickEvent, window, cx| {
                            this.save_pattern(&SavePattern, window, cx)
                        },
                    ))),
            );

        let naming = self.naming.then(|| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .p_1()
                .child(editor_box(&self.name_editor, window, cx, theme))
                .child(
                    div()
                        .px_1()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child("Enter to save · Esc to cancel"),
                )
        });

        let entries = self.library.entries();
        let empty = (entries.is_empty() && !self.naming && self.library.load_error().is_none())
            .then(|| {
                div()
                    .p_2()
                    .text_sm()
                    .text_color(theme.text_muted)
                    .child(format!(
                        "No saved patterns yet. Press {} to save the current one.",
                        shortcut("⌘S", "Ctrl+S")
                    ))
            });

        let rows = entries.iter().map(|entry| {
            let id = entry.id;
            let selected = self.current == Some(id);
            let modified = selected && self.is_modified(&entry.data, cx);
            let confirming = self.pending_delete == Some(id);
            let pattern = if entry.data.pattern.is_empty() {
                "(empty pattern)".to_owned()
            } else {
                preview(&entry.data.pattern)
            };
            div()
                .id(("entry", id))
                .group("entry")
                .flex()
                .items_center()
                .gap_1()
                .px_2()
                .py_1p5()
                .rounded_md()
                .cursor_pointer()
                .when(selected, |el| el.bg(theme.background))
                .when(!selected, |el| el.hover(|el| el.bg(theme.background)))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(div().min_w_0().truncate().child(entry.name.clone()))
                                .when(modified, |el| {
                                    el.child(div().text_color(theme.accent).child("•"))
                                }),
                        )
                        .child(
                            div()
                                .font_family(MONO_FONT)
                                .text_xs()
                                .text_color(theme.text_muted)
                                .truncate()
                                .child(pattern),
                        ),
                )
                .child(
                    div()
                        .id(("delete", id))
                        .flex_none()
                        .px_1()
                        .rounded_sm()
                        .text_xs()
                        .when(confirming, |el| {
                            el.bg(theme.error).text_color(theme.surface).child("Delete")
                        })
                        .when(!confirming, |el| {
                            el.text_color(theme.text_muted)
                                .invisible()
                                .group_hover("entry", |el| el.visible())
                                .hover(|el| el.text_color(theme.error))
                                .child("✕")
                        })
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.delete_entry(id, cx);
                        })),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.load_entry(id, window, cx)
                }))
        });

        div()
            .flex()
            .flex_col()
            .gap_1()
            .w(px(240.))
            .flex_none()
            .min_h_0()
            .child(header)
            .child(
                div()
                    .id("library")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_md()
                    .p_1()
                    .children(naming)
                    .children(empty)
                    .children(rows),
            )
            .children(self.library.load_error().map(|err| {
                div()
                    .text_xs()
                    .text_color(theme.error)
                    .child(err.to_owned())
            }))
    }

    fn render_flags(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut flags = self.flags;
        div().flex().gap_1().children(FlagKind::ALL.map(|flag| {
            let on = *flag.get(&mut flags);
            div()
                .id(SharedString::from(flag.letter()))
                .flex()
                .gap_1()
                .px_2()
                .py_0p5()
                .rounded_md()
                .border_1()
                .text_xs()
                .cursor_pointer()
                .when(on, |el| {
                    el.bg(theme.accent)
                        .border_color(theme.accent)
                        .text_color(theme.accent_text)
                })
                .when(!on, |el| {
                    el.border_color(theme.border)
                        .text_color(theme.text_muted)
                        .hover(|el| el.bg(theme.border))
                })
                .child(
                    div()
                        .font_family(MONO_FONT)
                        .font_weight(FontWeight::BOLD)
                        .child(flag.letter()),
                )
                .child(flag.label())
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_flag(flag, cx)),
                )
        }))
    }

    fn render_matches(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let text = self.sample.read(cx).text().to_owned();
        let body: AnyElement = match &self.outcome {
            Outcome::Empty => div()
                .text_sm()
                .text_color(theme.text_muted)
                .child("Enter a pattern to see matches.")
                .into_any_element(),
            Outcome::Error(_) => div()
                .text_sm()
                .text_color(theme.text_muted)
                .child("Fix the pattern to see matches.")
                .into_any_element(),
            Outcome::Found(found) if found.matches.is_empty() => div()
                .text_sm()
                .text_color(theme.text_muted)
                .child("No matches.")
                .into_any_element(),
            Outcome::Found(found) => {
                let mut chars = CharCounter::new(&text);
                let rows = found
                    .matches
                    .iter()
                    .take(LISTED_MATCHES)
                    .enumerate()
                    .map(|(ix, m)| {
                        let selected = self.selected_match == Some(ix);
                        let byte_span = format!("{}–{}", m.range.start, m.range.end);
                        let char_span =
                            (chars.count_to(m.range.start), chars.count_to(m.range.end));
                        let span = if char_span == (m.range.start, m.range.end) {
                            byte_span
                        } else {
                            format!("{byte_span} (chars {}–{})", char_span.0, char_span.1)
                        };
                        let match_text = if m.range.is_empty() {
                            "(empty)".to_string()
                        } else {
                            preview(&text[m.range.clone()])
                        };
                        div()
                            .id(("match", ix))
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .px_2()
                            .py_1p5()
                            .rounded_md()
                            .cursor_pointer()
                            .when(selected, |el| el.bg(theme.match_selected))
                            .when(!selected, |el| el.hover(|el| el.bg(theme.background)))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .items_baseline()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_muted)
                                            .child(format!("#{}", ix + 1)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .font_family(MONO_FONT)
                                            .text_sm()
                                            .child(match_text),
                                    )
                                    .child(
                                        div()
                                            .flex_none()
                                            .text_xs()
                                            .text_color(theme.text_muted)
                                            .child(span),
                                    ),
                            )
                            .children(m.groups.iter().map(|group| {
                                let label = match &group.name {
                                    Some(name) => format!("{} {name}", group.index),
                                    None => group.index.to_string(),
                                };
                                let value = match &group.range {
                                    Some(range) => preview(&text[range.clone()]),
                                    None => "(none)".to_string(),
                                };
                                div()
                                    .flex()
                                    .gap_2()
                                    .pl_4()
                                    .text_xs()
                                    .child(
                                        div()
                                            .min_w(px(64.))
                                            .text_color(theme.group)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(label),
                                    )
                                    .child(
                                        div()
                                            .font_family(MONO_FONT)
                                            .when(group.range.is_none(), |el| {
                                                el.text_color(theme.text_muted)
                                            })
                                            .child(value),
                                    )
                            }))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.select_match(ix, cx)
                            }))
                    });
                let hidden = found.matches.len().saturating_sub(LISTED_MATCHES);
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .children(rows)
                    .when(hidden > 0, |el| {
                        el.child(
                            div()
                                .px_2()
                                .py_1()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child(format!("{hidden} more matches not listed")),
                        )
                    })
                    .into_any_element()
            }
        };

        div()
            .id("matches")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .p_1()
            .child(body)
    }

    fn status_text(&self) -> (String, bool) {
        match &self.outcome {
            Outcome::Empty => ("No pattern".into(), false),
            Outcome::Error(_) => ("Invalid pattern".into(), true),
            Outcome::Found(found) => {
                let count = found.matches.len();
                let mut status = match count {
                    1 => "1 match".to_string(),
                    _ => format!("{count} matches"),
                };
                if found.truncated {
                    status = format!("Showing the first {MATCH_LIMIT} matches");
                }
                if let Some(group_count) = found.matches.first().map(|m| m.groups.len())
                    && group_count > 0
                {
                    status.push_str(&format!(
                        " · {group_count} group{}",
                        if group_count == 1 { "" } else { "s" }
                    ));
                }
                (status, false)
            }
        }
    }
}

/// Converts increasing byte offsets to char offsets in a single pass.
struct CharCounter<'a> {
    text: &'a str,
    byte: usize,
    chars: usize,
}

impl<'a> CharCounter<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            byte: 0,
            chars: 0,
        }
    }

    fn count_to(&mut self, byte: usize) -> usize {
        if self.text.is_ascii() {
            return byte;
        }
        if byte < self.byte {
            self.byte = 0;
            self.chars = 0;
        }
        self.chars += self.text[self.byte..byte].chars().count();
        self.byte = byte;
        self.chars
    }
}

impl Render for Matcha {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.global::<Theme>();
        let (status, status_is_error) = match &self.notice {
            Some((notice, is_error)) => (notice.clone(), *is_error),
            None => self.status_text(),
        };
        let error = match &self.outcome {
            Outcome::Error(err) => Some(err.clone()),
            _ => None,
        };

        let sample_box = editor_box(&self.sample, window, cx, &theme)
            .id("sample")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.sample_scroll);

        let replace_section = self.show_replace.then(|| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .flex_1()
                .min_h_0()
                .child(section_label("REPLACEMENT", &theme))
                .child(editor_box(&self.replacement, window, cx, &theme))
                .child(div().h_1())
                .child(section_label("RESULT", &theme))
                .child(match &self.replace_error {
                    Some(err) => div()
                        .text_sm()
                        .text_color(theme.error)
                        .child(err.clone())
                        .into_any_element(),
                    None => editor_box(&self.result, window, cx, &theme)
                        .id("result")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.result_scroll)
                        .into_any_element(),
                })
        });

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::focus_pattern))
            .on_action(cx.listener(Self::toggle_replace))
            .on_action(cx.listener(Self::focus_next))
            .on_action(cx.listener(Self::focus_previous))
            .on_action(cx.listener(Self::save_pattern))
            .on_action(cx.listener(Self::save_pattern_as))
            .on_action(cx.listener(Self::new_pattern))
            .on_action(cx.listener(Self::toggle_library))
            .on_action(cx.listener(Self::copy_as_code))
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(theme.background)
            .text_color(theme.text)
            .child(self.render_header(&theme, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(section_label("PATTERN", &theme))
                                    .child(
                                        div()
                                            .id("copy-as-code")
                                            .px_2()
                                            .rounded_md()
                                            .text_xs()
                                            .cursor_pointer()
                                            .text_color(theme.text_muted)
                                            .hover(|el| el.bg(theme.border).text_color(theme.text))
                                            .child(format!(
                                                "Copy as Rust  {}",
                                                shortcut("⇧⌘C", "Ctrl+Shift+C")
                                            ))
                                            .on_click(cx.listener(
                                                |this, _: &ClickEvent, window, cx| {
                                                    this.copy_as_code(&CopyAsCode, window, cx)
                                                },
                                            )),
                                    ),
                            )
                            .child(self.render_flags(&theme, cx)),
                    )
                    .child(editor_box(&self.pattern, window, cx, &theme))
                    .children(error.map(|err| {
                        div()
                            .font_family(MONO_FONT)
                            .text_xs()
                            .text_color(theme.error)
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.error)
                            .child(err)
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .flex_1()
                    .min_h_0()
                    .when(self.show_library, |el| {
                        el.child(self.render_library(window, &theme, cx))
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .flex_1()
                                    .min_h_0()
                                    .child(section_label("SAMPLE TEXT", &theme))
                                    .child(sample_box),
                            )
                            .children(replace_section),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .w(px(340.))
                            .min_h_0()
                            .child(section_label("MATCHES", &theme))
                            .child(self.render_matches(&theme, cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .justify_between()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .when(status_is_error, |el| el.text_color(theme.error))
                            .child(status),
                    )
                    .child(div().flex_none().child(format!(
                        "{} · {} save · {} library · {} replace",
                        self.engine.label(),
                        shortcut("⌘S", "Ctrl+S"),
                        shortcut("⌘O", "Ctrl+O"),
                        shortcut("⌘R", "Ctrl+R"),
                    ))),
            )
    }
}
