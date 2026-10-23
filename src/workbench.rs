//! What the main window does, without the UI: search, replace, explain, copy as
//! code, and saving to the library. The window owns the text fields and passes
//! their contents in as [`Texts`].

use std::ops::Range;

use crate::codegen;
use crate::engine::{EngineKind, Flags, Matches, Regex};
use crate::explain::{self, ExplainError, Part};
use crate::library::{Library, PatternData};

pub enum Outcome {
    /// No pattern entered yet.
    Empty,
    Error(String),
    Found(Matches),
}

/// The current contents of the pattern, sample and replacement fields.
#[derive(Clone, Copy)]
pub struct Texts<'a> {
    pub pattern: &'a str,
    pub sample: &'a str,
    pub replacement: &'a str,
}

/// The inputs to one search, owned so it can run on another thread.
pub struct Request {
    pattern: String,
    sample: String,
    replacement: String,
    flags: Flags,
    engine: EngineKind,
    show_replace: bool,
}

/// Everything one search produces; see [`Workbench::apply_analysis`].
pub struct Analysis {
    outcome: Outcome,
    result: String,
    replace_error: Option<String>,
    explanation: Result<Vec<Part>, ExplainError>,
}

/// Runs the search, replacement and explanation. Pure, so it can run on any thread.
pub fn analyze(request: &Request) -> Analysis {
    let regex = (!request.pattern.is_empty())
        .then(|| Regex::new(&request.pattern, request.flags, request.engine));
    let outcome = match &regex {
        None => Outcome::Empty,
        Some(Err(err)) => Outcome::Error(err.clone()),
        Some(Ok(regex)) => match regex.find_all(&request.sample) {
            Ok(matches) => Outcome::Found(matches),
            Err(err) => Outcome::Error(err),
        },
    };

    let mut replace_error = None;
    let result = match &regex {
        Some(Ok(regex)) if request.show_replace => regex
            .replace_all(&request.sample, &request.replacement)
            .unwrap_or_else(|err| {
                replace_error = Some(err);
                String::new()
            }),
        _ => request.sample.clone(),
    };

    Analysis {
        outcome,
        result,
        replace_error,
        explanation: explain::explain(&request.pattern, request.flags),
    }
}

/// What [`Workbench::apply_analysis`] produced for the window to show.
pub struct Recomputed {
    /// The replace preview, or the sample unchanged when there is nothing to replace.
    pub result: String,
    pub explanation_changed: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Save {
    Saved(String),
    /// Not saved to the library yet; ask for a name and call [`Workbench::save_new`].
    NeedsName,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Delete {
    /// First click: the entry is marked and a second click deletes it.
    NeedsConfirmation,
    Deleted(String),
}

pub struct Workbench {
    flags: Flags,
    engine: EngineKind,
    show_replace: bool,
    outcome: Outcome,
    replace_error: Option<String>,
    explanation: Result<Vec<Part>, ExplainError>,
    selected_match: Option<usize>,
    library: Library,
    /// The library entry the workbench was loaded from or last saved to.
    current: Option<u64>,
    /// Entry whose delete button was clicked once and awaits confirmation.
    pending_delete: Option<u64>,
}

impl Workbench {
    pub fn new(library: Library) -> Self {
        Self {
            flags: Flags::default(),
            engine: EngineKind::default(),
            show_replace: true,
            outcome: Outcome::Empty,
            replace_error: None,
            explanation: Ok(Vec::new()),
            selected_match: None,
            library,
            current: None,
            pending_delete: None,
        }
    }

    pub fn flags(&self) -> Flags {
        self.flags
    }

    pub fn engine(&self) -> EngineKind {
        self.engine
    }

    pub fn show_replace(&self) -> bool {
        self.show_replace
    }

    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }

    pub fn replace_error(&self) -> Option<&str> {
        self.replace_error.as_deref()
    }

    pub fn explanation(&self) -> &Result<Vec<Part>, ExplainError> {
        &self.explanation
    }

    pub fn selected_match(&self) -> Option<usize> {
        self.selected_match
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    pub fn current(&self) -> Option<u64> {
        self.current
    }

    pub fn pending_delete(&self) -> Option<u64> {
        self.pending_delete
    }

    // The setters below change settings only; call `recompute` afterwards.

    pub fn set_flags(&mut self, flags: Flags) {
        self.flags = flags;
    }

    /// Returns whether the engine changed.
    pub fn set_engine(&mut self, engine: EngineKind) -> bool {
        let changed = self.engine != engine;
        self.engine = engine;
        changed
    }

    pub fn set_show_replace(&mut self, show: bool) {
        self.show_replace = show;
    }

    /// Snapshots the inputs so [`analyze`] can run on another thread.
    pub fn request(&self, texts: Texts) -> Request {
        Request {
            pattern: texts.pattern.to_owned(),
            sample: texts.sample.to_owned(),
            replacement: texts.replacement.to_owned(),
            flags: self.flags,
            engine: self.engine,
            show_replace: self.show_replace,
        }
    }

    /// Takes in the result of [`analyze`].
    pub fn apply_analysis(&mut self, analysis: Analysis) -> Recomputed {
        self.outcome = analysis.outcome;
        self.replace_error = analysis.replace_error;
        let explanation_changed = analysis.explanation != self.explanation;
        self.explanation = analysis.explanation;

        if let (Some(ix), Outcome::Found(found)) = (self.selected_match, &self.outcome)
            && ix >= found.matches.len()
        {
            self.selected_match = None;
        }
        Recomputed {
            result: analysis.result,
            explanation_changed,
        }
    }

    /// Analyzes `texts` on this thread, for tests.
    #[cfg(test)]
    pub fn recompute(&mut self, texts: Texts) -> Recomputed {
        let analysis = analyze(&self.request(texts));
        self.apply_analysis(analysis)
    }

    /// Selects match `ix` and returns its range in the sample, if it exists.
    pub fn select_match(&mut self, ix: usize) -> Option<Range<usize>> {
        let Outcome::Found(found) = &self.outcome else {
            return None;
        };
        let range = found.matches.get(ix)?.range.clone();
        self.selected_match = Some(ix);
        Some(range)
    }

    /// Rust code for the pattern, or a message saying why there is none.
    pub fn rust_code(&self, texts: Texts) -> Result<String, String> {
        if texts.pattern.is_empty() {
            return Err("Enter a pattern to copy it as code.".into());
        }
        if Regex::new(texts.pattern, self.flags, self.engine).is_err() {
            return Err("Fix the pattern before copying it as code.".into());
        }
        let replacement = if self.show_replace {
            texts.replacement
        } else {
            ""
        };
        Ok(codegen::rust_snippet(
            texts.pattern,
            self.flags,
            self.engine,
            replacement,
        ))
    }

    pub fn data(&self, texts: Texts) -> PatternData {
        PatternData {
            pattern: texts.pattern.to_owned(),
            sample: texts.sample.to_owned(),
            replacement: texts.replacement.to_owned(),
            flags: self.flags,
            engine: self.engine,
        }
    }

    /// Whether the workbench differs from `data`, without copying the texts.
    pub fn is_modified(&self, data: &PatternData, texts: Texts) -> bool {
        data.pattern != texts.pattern
            || data.sample != texts.sample
            || data.replacement != texts.replacement
            || data.flags != self.flags
            || data.engine != self.engine
    }

    /// Saves over the current entry, or asks for a name when there is none.
    pub fn save(&mut self, texts: Texts) -> Result<Save, String> {
        let Some(id) = self.current.filter(|&id| self.library.get(id).is_some()) else {
            return Ok(Save::NeedsName);
        };
        self.library.update(id, self.data(texts))?;
        let name = self
            .library
            .get(id)
            .map(|entry| entry.name.clone())
            .unwrap_or_default();
        Ok(Save::Saved(name))
    }

    /// Checks that a new entry can be saved, before asking for its name.
    pub fn start_saving_new(&mut self) -> Result<(), String> {
        if self.library.load_error().is_some() {
            return Err("Saving is disabled because the library file can't be read.".into());
        }
        self.pending_delete = None;
        Ok(())
    }

    /// The suggested name for Save As: the current entry's name plus " copy".
    pub fn save_as_name(&self) -> String {
        self.current
            .and_then(|id| self.library.get(id))
            .map(|entry| format!("{} copy", entry.name))
            .unwrap_or_default()
    }

    /// Saves a new entry named `name` and makes it the current one.
    pub fn save_new(&mut self, name: String, texts: Texts) -> Result<(), String> {
        let id = self.library.add(name, self.data(texts))?;
        self.current = Some(id);
        Ok(())
    }

    /// Makes entry `id` current and returns its contents for the text fields.
    pub fn open(&mut self, id: u64) -> Option<PatternData> {
        let data = self.library.get(id)?.data.clone();
        self.pending_delete = None;
        self.current = Some(id);
        self.apply(&data);
        Some(data)
    }

    /// Starts an unsaved pattern and returns the empty contents for the text fields.
    pub fn new_pattern(&mut self) -> PatternData {
        let data = PatternData::default();
        self.current = None;
        self.apply(&data);
        data
    }

    fn apply(&mut self, data: &PatternData) {
        self.flags = data.flags;
        self.engine = data.engine;
        self.selected_match = None;
    }

    /// Deletes entry `id` on the second call in a row for it.
    pub fn delete(&mut self, id: u64) -> Result<Delete, String> {
        if self.pending_delete != Some(id) {
            self.pending_delete = Some(id);
            return Ok(Delete::NeedsConfirmation);
        }
        self.pending_delete = None;
        let name = self
            .library
            .get(id)
            .map(|entry| entry.name.clone())
            .unwrap_or_default();
        self.library.remove(id)?;
        if self.current == Some(id) {
            self.current = None;
        }
        Ok(Delete::Deleted(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::testing::FakeStore;

    fn texts<'a>(pattern: &'a str, sample: &'a str, replacement: &'a str) -> Texts<'a> {
        Texts {
            pattern,
            sample,
            replacement,
        }
    }

    /// A workbench with no store, as when there is no data directory.
    fn bench() -> Workbench {
        Workbench::new(Library::load(None))
    }

    fn match_count(bench: &Workbench) -> Option<usize> {
        match bench.outcome() {
            Outcome::Found(found) => Some(found.matches.len()),
            _ => None,
        }
    }

    #[test]
    fn searches_can_move_between_threads() {
        fn assert_send<T: Send>() {}
        assert_send::<Request>();
        assert_send::<Analysis>();
    }

    #[test]
    fn empty_pattern_shows_the_sample() {
        let mut bench = bench();
        let out = bench.recompute(texts("", "abc", "x"));
        assert!(matches!(bench.outcome(), Outcome::Empty));
        assert_eq!(out.result, "abc");
    }

    #[test]
    fn finds_and_replaces() {
        let mut bench = bench();
        let out = bench.recompute(texts(r"(\d)", "a1b2", "<$1>"));
        assert_eq!(match_count(&bench), Some(2));
        assert_eq!(out.result, "a<1>b<2>");
        assert!(out.explanation_changed);
        assert!(
            !bench
                .recompute(texts(r"(\d)", "a1", ""))
                .explanation_changed
        );
    }

    #[test]
    fn hidden_replace_leaves_the_sample() {
        let mut bench = bench();
        bench.set_show_replace(false);
        let out = bench.recompute(texts("a", "aa", "b"));
        assert_eq!(out.result, "aa");
        let code = bench.rust_code(texts("a", "", "b")).unwrap();
        assert!(!code.contains("replace_all"), "{code}");
    }

    #[test]
    fn invalid_pattern_is_an_error_outcome() {
        let mut bench = bench();
        let out = bench.recompute(texts("(", "abc", ""));
        assert!(matches!(bench.outcome(), Outcome::Error(_)));
        assert_eq!(out.result, "abc");
    }

    #[test]
    fn flags_and_engine_apply_on_recompute() {
        let mut bench = bench();
        bench.set_flags(Flags {
            case_insensitive: true,
            ..Flags::default()
        });
        bench.recompute(texts("a", "aA", ""));
        assert_eq!(match_count(&bench), Some(2));

        assert!(bench.set_engine(EngineKind::Fancy));
        assert!(!bench.set_engine(EngineKind::Fancy));
        bench.recompute(texts(r"a(?=b)", "ab", ""));
        assert_eq!(match_count(&bench), Some(1));
    }

    #[test]
    fn selection_is_cleared_when_the_match_goes_away() {
        let mut bench = bench();
        bench.recompute(texts("a", "aaa", ""));
        assert_eq!(bench.select_match(2), Some(2..3));
        assert_eq!(bench.select_match(5), None);
        assert_eq!(bench.selected_match(), Some(2));
        bench.recompute(texts("a", "a", ""));
        assert_eq!(bench.selected_match(), None);
    }

    #[test]
    fn rust_code_needs_a_valid_pattern() {
        let bench = bench();
        assert!(bench.rust_code(texts("", "", "")).is_err());
        assert!(bench.rust_code(texts("(", "", "")).is_err());
        assert!(bench.rust_code(texts("a", "", "")).is_ok());
    }

    #[test]
    fn saving_without_a_store_is_refused() {
        let mut bench = bench();
        assert_eq!(bench.save(texts("a", "", "")), Ok(Save::NeedsName));
        assert!(bench.start_saving_new().is_err());
        assert!(bench.save_new("A".into(), texts("a", "", "")).is_err());
        assert_eq!(bench.current(), None);
    }

    #[test]
    fn saves_new_then_overwrites_current() {
        let store = FakeStore::default();
        let mut bench = Workbench::new(store.library());
        assert_eq!(bench.save(texts("a", "", "")), Ok(Save::NeedsName));
        bench.start_saving_new().unwrap();
        bench.save_new("Mine".into(), texts("a", "", "")).unwrap();
        let id = bench.current().unwrap();

        assert_eq!(
            bench.save(texts("b", "", "")),
            Ok(Save::Saved("Mine".into()))
        );
        assert_eq!(store.stored().len(), 1);
        assert_eq!(store.stored()[0].data.pattern, "b");
        assert_eq!(bench.save_as_name(), "Mine copy");
        assert!(!bench.is_modified(&store.stored()[0].data, texts("b", "", "")));
        assert!(bench.is_modified(&store.stored()[0].data, texts("c", "", "")));
        assert_eq!(
            bench.library().get(id).map(|e| e.name.as_str()),
            Some("Mine")
        );
    }

    #[test]
    fn opening_an_entry_restores_its_settings() {
        let store = FakeStore::default();
        let mut bench = Workbench::new(store.library());
        bench.set_engine(EngineKind::Fancy);
        bench
            .save_new("Fancy".into(), texts("a", "s", "r"))
            .unwrap();
        let id = bench.current().unwrap();

        bench.new_pattern();
        assert_eq!(bench.current(), None);
        assert_eq!(bench.engine(), EngineKind::Standard);

        let data = bench.open(id).unwrap();
        assert_eq!((data.pattern.as_str(), data.sample.as_str()), ("a", "s"));
        assert_eq!(bench.engine(), EngineKind::Fancy);
        assert_eq!(bench.current(), Some(id));
        assert_eq!(bench.open(id + 1), None);
    }

    #[test]
    fn delete_needs_a_second_click() {
        let store = FakeStore::default();
        let mut bench = Workbench::new(store.library());
        bench.save_new("Gone".into(), texts("a", "", "")).unwrap();
        let id = bench.current().unwrap();

        assert_eq!(bench.delete(id), Ok(Delete::NeedsConfirmation));
        assert_eq!(bench.pending_delete(), Some(id));
        assert_eq!(bench.delete(id), Ok(Delete::Deleted("Gone".into())));
        assert_eq!(bench.current(), None);
        assert_eq!(store.stored(), []);
    }

    #[test]
    fn failed_save_keeps_the_current_entry() {
        let store = FakeStore::default();
        let mut bench = Workbench::new(store.library());
        bench.save_new("Kept".into(), texts("a", "", "")).unwrap();
        let id = bench.current();

        store.0.borrow_mut().fail_writes = true;
        assert!(bench.save_new("New".into(), texts("b", "", "")).is_err());
        assert_eq!(bench.current(), id);
        assert!(bench.save(texts("b", "", "")).is_err());
    }
}
