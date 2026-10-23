//! Keeps the library in a JSON file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use matcha::library::{Entry, PatternStore};

#[derive(Default, Serialize, Deserialize)]
struct LibraryFile {
    version: u32,
    entries: Vec<Entry>,
}

const FILE_VERSION: u32 = 1;

pub struct JsonFileStore {
    path: PathBuf,
}

impl JsonFileStore {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl PatternStore for JsonFileStore {
    /// A missing file is an empty library.
    fn read(&self) -> Result<Vec<Entry>, String> {
        read_entries(&self.path)
            .map_err(|err| format!("Could not read {}: {err}", self.path.display()))
    }

    fn write(&self, entries: &[Entry]) -> Result<(), String> {
        write_entries(&self.path, entries)
            .map_err(|err| format!("Could not save {}: {err}", self.path.display()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use matcha::engine::{EngineKind, Flags};
    use matcha::library::PatternData;

    fn entry(id: u64, pattern: &str) -> Entry {
        Entry {
            id,
            name: format!("Entry {id}"),
            data: PatternData {
                pattern: pattern.into(),
                engine: EngineKind::Fancy,
                ..PatternData::default()
            },
        }
    }

    #[test]
    fn missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonFileStore::new(dir.path().join("library.json"));
        assert_eq!(store.read().unwrap(), []);
    }

    #[test]
    fn writes_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonFileStore::new(dir.path().join("nested/library.json"));
        let entries = vec![entry(2, "b"), entry(1, "a")];
        store.write(&entries).unwrap();
        assert_eq!(store.read().unwrap(), entries);
    }

    #[test]
    fn invalid_json_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        fs::write(&path, "not json").unwrap();
        assert!(JsonFileStore::new(path).read().is_err());
    }

    #[test]
    fn rejects_files_from_newer_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        fs::write(&path, r#"{"version": 99, "entries": []}"#).unwrap();
        let err = JsonFileStore::new(path).read().unwrap_err();
        assert!(err.contains("newer version"), "{err}");
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
        let entries = JsonFileStore::new(path).read().unwrap();
        assert_eq!(entries[0].data.pattern, "x");
        assert_eq!(entries[0].data.flags, Flags::default());
        assert_eq!(entries[0].data.engine, EngineKind::Standard);
    }

    #[test]
    fn write_error_names_the_file() {
        let dir = tempfile::tempdir().unwrap();
        // A file where the directory should be makes the write fail.
        fs::write(dir.path().join("sub"), "").unwrap();
        let path = dir.path().join("sub/library.json");
        let err = JsonFileStore::new(path.clone()).write(&[]).unwrap_err();
        assert!(err.contains(&path.display().to_string()), "{err}");
    }
}
