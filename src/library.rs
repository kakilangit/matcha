//! Saved patterns. Where they are kept is up to the [`PatternStore`] passed in.

use serde::{Deserialize, Serialize};

use matcha::engine::{EngineKind, Flags};

/// Everything needed to restore a pattern into the workbench.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PatternData {
    pub pattern: String,
    pub sample: String,
    pub replacement: String,
    pub flags: Flags,
    pub engine: EngineKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    pub name: String,
    #[serde(flatten)]
    pub data: PatternData,
}

/// Where the library is kept. Errors are messages ready to show to the user.
pub trait PatternStore {
    /// Reads every entry; nothing saved yet is an empty list, not an error.
    ///
    /// # Errors
    ///
    /// A message for the user when the stored library can't be read or is from a
    /// newer version.
    fn read(&self) -> Result<Vec<Entry>, String>;
    /// Replaces everything stored with `entries`.
    ///
    /// # Errors
    ///
    /// A message for the user when the library can't be written.
    fn write(&self, entries: &[Entry]) -> Result<(), String>;
}

pub struct Library {
    store: Option<Box<dyn PatternStore>>,
    entries: Vec<Entry>,
    /// Set when the store could not be read. Saving is then refused so a damaged
    /// or newer library is never overwritten.
    load_error: Option<String>,
}

impl Library {
    /// Loads the library from `store`; `None` means there is nowhere to keep it.
    pub fn load(store: Option<Box<dyn PatternStore>>) -> Self {
        let (entries, load_error) = match store.as_deref().map(PatternStore::read) {
            None => (
                Vec::new(),
                Some("No data directory found; the library is disabled.".into()),
            ),
            Some(Ok(entries)) => (entries, None),
            Some(Err(err)) => (Vec::new(), Some(err)),
        };
        Self {
            store,
            entries,
            load_error,
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    #[must_use]
    pub fn get(&self, id: u64) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    #[must_use]
    pub fn load_error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }

    /// Adds a new entry at the top of the list and saves. Returns its id.
    ///
    /// # Errors
    ///
    /// When the library is disabled or the store fails; the entries are unchanged.
    pub fn add(&mut self, name: String, data: PatternData) -> Result<u64, String> {
        let id = self.entries.iter().map(|entry| entry.id).max().unwrap_or(0) + 1;
        let mut entries = self.entries.clone();
        entries.insert(0, Entry { id, name, data });
        self.commit(entries).map(|()| id)
    }

    /// Replaces entry `id`'s contents and saves.
    ///
    /// # Errors
    ///
    /// When the entry is gone, the library is disabled, or the store fails; the
    /// entries are unchanged.
    pub fn update(&mut self, id: u64, data: PatternData) -> Result<(), String> {
        let mut entries = self.entries.clone();
        let entry = entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or("This pattern is no longer in the library.")?;
        entry.data = data;
        self.commit(entries)
    }

    /// Removes entry `id` and saves.
    ///
    /// # Errors
    ///
    /// When the library is disabled or the store fails; the entries are unchanged.
    pub fn remove(&mut self, id: u64) -> Result<(), String> {
        let mut entries = self.entries.clone();
        entries.retain(|entry| entry.id != id);
        self.commit(entries)
    }

    /// Saves `entries` and only then makes them current, so a failed save leaves
    /// the library as it is in the store.
    fn commit(&mut self, entries: Vec<Entry>) -> Result<(), String> {
        if let Some(err) = &self.load_error {
            return Err(err.clone());
        }
        let store = self.store.as_deref().ok_or("No data directory found.")?;
        store.write(&entries)?;
        self.entries = entries;
        Ok(())
    }
}

#[cfg(test)]
pub mod testing {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{Entry, Library, PatternStore};

    /// An in-memory store; clones share the same contents.
    #[derive(Clone, Default)]
    pub struct FakeStore(pub Rc<RefCell<FakeState>>);

    #[derive(Default)]
    pub struct FakeState {
        pub entries: Vec<Entry>,
        pub read_error: Option<String>,
        pub fail_writes: bool,
    }

    impl FakeStore {
        #[must_use]
        pub fn library(&self) -> Library {
            Library::load(Some(Box::new(self.clone())))
        }

        #[must_use]
        pub fn stored(&self) -> Vec<Entry> {
            self.0.borrow().entries.clone()
        }
    }

    impl PatternStore for FakeStore {
        fn read(&self) -> Result<Vec<Entry>, String> {
            let state = self.0.borrow();
            match &state.read_error {
                Some(err) => Err(err.clone()),
                None => Ok(state.entries.clone()),
            }
        }

        fn write(&self, entries: &[Entry]) -> Result<(), String> {
            let mut state = self.0.borrow_mut();
            if state.fail_writes {
                return Err("disk full".into());
            }
            state.entries = entries.to_vec();
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::FakeStore;
    use super::*;

    fn data(pattern: &str) -> PatternData {
        PatternData {
            pattern: pattern.into(),
            sample: "sample".into(),
            replacement: "$1".into(),
            flags: Flags {
                case_insensitive: true,
                ..Flags::default()
            },
            engine: EngineKind::Fancy,
        }
    }

    #[test]
    fn empty_store_is_an_empty_library() {
        let library = FakeStore::default().library();
        assert_eq!(library.entries(), []);
        assert!(library.load_error().is_none());
    }

    #[test]
    fn saves_and_reloads_entries() {
        let store = FakeStore::default();
        let mut library = store.library();
        let first = library.add("Emails".into(), data("a")).unwrap();
        let second = library.add("Dates".into(), data("b")).unwrap();
        assert_ne!(first, second);
        library.update(first, data("a2")).unwrap();

        let mut reloaded = store.library();
        let names: Vec<&str> = reloaded.entries().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Dates", "Emails"]);
        assert_eq!(reloaded.get(first).unwrap().data, data("a2"));

        reloaded.remove(second).unwrap();
        assert_eq!(store.library().entries().len(), 1);
    }

    #[test]
    fn unreadable_store_is_never_overwritten() {
        let store = FakeStore::default();
        store.0.borrow_mut().read_error = Some("damaged".into());

        let mut library = store.library();
        assert_eq!(library.load_error(), Some("damaged"));
        store.0.borrow_mut().read_error = None;
        assert!(library.add("New".into(), data("x")).is_err());
        assert_eq!(library.entries(), []);
        assert_eq!(store.stored(), []);
    }

    #[test]
    fn failed_save_keeps_entries_unchanged() {
        let store = FakeStore::default();
        let mut library = store.library();
        let id = library.add("Kept".into(), data("a")).unwrap();
        let before = library.entries().to_vec();

        store.0.borrow_mut().fail_writes = true;
        assert!(library.add("New".into(), data("b")).is_err());
        assert!(library.update(id, data("changed")).is_err());
        assert!(library.remove(id).is_err());
        assert_eq!(library.entries(), before);
        assert_eq!(store.stored(), before);
    }

    #[test]
    fn no_store_disables_the_library() {
        let mut library = Library::load(None);
        assert!(library.load_error().is_some());
        assert!(library.add("New".into(), data("x")).is_err());
        assert_eq!(library.entries(), []);
    }
}
