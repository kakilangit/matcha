//! Saved patterns, stored as JSON in the user's data directory.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::{EngineKind, Flags};
use crate::platform;

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

#[derive(Default, Serialize, Deserialize)]
struct LibraryFile {
    version: u32,
    entries: Vec<Entry>,
}

const FILE_VERSION: u32 = 1;

pub struct Library {
    path: Option<PathBuf>,
    entries: Vec<Entry>,
    /// Set when the file exists but could not be read. Saving is then refused so a
    /// damaged or newer file is never overwritten.
    load_error: Option<String>,
}

impl Library {
    /// Loads the library from `path`. A missing file is an empty library.
    pub fn load(path: Option<PathBuf>) -> Self {
        let mut library = Self {
            path,
            entries: Vec::new(),
            load_error: None,
        };
        let Some(path) = library.path.as_deref() else {
            library.load_error = Some("No data directory found; the library is disabled.".into());
            return library;
        };
        match read_entries(path) {
            Ok(entries) => library.entries = entries,
            Err(err) => {
                library.load_error = Some(format!("Could not read {}: {err}", path.display()));
            }
        }
        library
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn get(&self, id: u64) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn load_error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }

    /// Adds a new entry at the top of the list and saves. Returns its id.
    pub fn add(&mut self, name: String, data: PatternData) -> Result<u64, String> {
        let id = self.entries.iter().map(|entry| entry.id).max().unwrap_or(0) + 1;
        let mut entries = self.entries.clone();
        entries.insert(0, Entry { id, name, data });
        self.commit(entries).map(|()| id)
    }

    pub fn update(&mut self, id: u64, data: PatternData) -> Result<(), String> {
        let mut entries = self.entries.clone();
        let entry = entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or("This pattern is no longer in the library.")?;
        entry.data = data;
        self.commit(entries)
    }

    pub fn remove(&mut self, id: u64) -> Result<(), String> {
        let mut entries = self.entries.clone();
        entries.retain(|entry| entry.id != id);
        self.commit(entries)
    }

    /// Saves `entries` and only then makes them current, so a failed save leaves
    /// the library as it is on disk.
    fn commit(&mut self, entries: Vec<Entry>) -> Result<(), String> {
        if let Some(err) = &self.load_error {
            return Err(err.clone());
        }
        let path = self.path.as_deref().ok_or("No data directory found.")?;
        write_entries(path, &entries)
            .map_err(|err| format!("Could not save {}: {err}", path.display()))?;
        self.entries = entries;
        Ok(())
    }
}

fn read_entries(path: &Path) -> io::Result<Vec<Entry>> {
    let json = match fs::read_to_string(path) {
        Ok(json) => json,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let file: LibraryFile = serde_json::from_str(&json).map_err(io::Error::other)?;
    if file.version > FILE_VERSION {
        return Err(io::Error::other(format!(
            "it was written by a newer version of Matcha (format {})",
            file.version
        )));
    }
    Ok(file.entries)
}

/// Writes to a temporary file first, so a crash never leaves a half-written library.
fn write_entries(path: &Path, entries: &[Entry]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let file = LibraryFile {
        version: FILE_VERSION,
        entries: entries.to_vec(),
    };
    let json = serde_json::to_string_pretty(&file).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)
}

/// `MATCHA_LIBRARY` if set, otherwise the platform data directory.
pub fn default_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MATCHA_LIBRARY") {
        return Some(PathBuf::from(path));
    }
    let dir = platform::data_dir()?;
    Some(dir.join("library.json"))
}

#[cfg(test)]
mod tests {
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
    fn missing_file_is_an_empty_library() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::load(Some(dir.path().join("library.json")));
        assert_eq!(library.entries(), []);
        assert!(library.load_error().is_none());
    }

    #[test]
    fn saves_and_reloads_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/library.json");

        let mut library = Library::load(Some(path.clone()));
        let first = library.add("Emails".into(), data("a")).unwrap();
        let second = library.add("Dates".into(), data("b")).unwrap();
        assert_ne!(first, second);
        library.update(first, data("a2")).unwrap();

        let reloaded = Library::load(Some(path.clone()));
        let names: Vec<&str> = reloaded.entries().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Dates", "Emails"]);
        assert_eq!(reloaded.get(first).unwrap().data, data("a2"));

        let mut reloaded = reloaded;
        reloaded.remove(second).unwrap();
        assert_eq!(Library::load(Some(path)).entries().len(), 1);
    }

    #[test]
    fn unreadable_file_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        fs::write(&path, "not json").unwrap();

        let mut library = Library::load(Some(path.clone()));
        assert!(library.load_error().is_some());
        assert!(library.add("New".into(), data("x")).is_err());
        assert_eq!(library.entries(), []);
        assert_eq!(fs::read_to_string(&path).unwrap(), "not json");
    }

    #[test]
    fn failed_save_keeps_entries_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        let mut library = Library::load(Some(sub.join("library.json")));
        let id = library.add("Kept".into(), data("a")).unwrap();
        let before = library.entries().to_vec();

        // A file where the directory should be makes every save fail.
        fs::remove_dir_all(&sub).unwrap();
        fs::write(&sub, "").unwrap();

        assert!(library.add("New".into(), data("b")).is_err());
        assert!(library.update(id, data("changed")).is_err());
        assert!(library.remove(id).is_err());
        assert_eq!(library.entries(), before);
    }

    #[test]
    fn rejects_files_from_newer_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        fs::write(&path, r#"{"version": 99, "entries": []}"#).unwrap();
        assert!(Library::load(Some(path)).load_error().is_some());
    }

    #[test]
    fn missing_fields_take_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        fs::write(
            &path,
            r#"{"version": 1, "entries": [{"id": 3, "name": "Old", "pattern": "x"}]}"#,
        )
        .unwrap();
        let library = Library::load(Some(path));
        let entry = library.get(3).unwrap();
        assert_eq!(entry.data.pattern, "x");
        assert_eq!(entry.data.flags, Flags::default());
        assert_eq!(entry.data.engine, EngineKind::Standard);
    }
}
