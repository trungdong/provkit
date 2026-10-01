#![allow(dead_code)] // Items shared across test crates may be unused in individual crates.

use std::fs;
use std::path::{Path, PathBuf};

use provkit::{Attributes, DateTime, Document, Formal, FormalAttr as F, Kind, QName, Record};

/// Number of PROV-JSON documents in the corpus.
pub const JSON_CORPUS_SIZE: usize = 398;

/// A name in `http://example.org/`.
pub fn ex(local: &str) -> QName {
    QName::new("http://example.org/", local).unwrap()
}

/// A fixed zoned `dateTime`.
pub fn dt() -> DateTime {
    DateTime::parse("2011-11-16T16:05:33Z").unwrap()
}

/// An element record with `id` and `attrs`.
pub fn element(kind: Kind, id: QName, attrs: Attributes) -> Record {
    Record::from_parts(kind, Some(id), vec![], attrs).unwrap()
}

/// A relation record from its formals.
pub fn relation(kind: Kind, id: Option<QName>, formals: Vec<(F, Formal)>) -> Record {
    Record::from_parts(kind, id, formals, Attributes::new()).unwrap()
}

/// A document in `http://example.org/` under `ex`, holding `records`.
pub fn document(records: Vec<Record>) -> Document {
    let mut d = Document::new();
    d.namespaces.declare("ex", "http://example.org/").unwrap();
    d.records = records;
    d
}

/// Path of a file in `tests/data`.
pub fn data_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// The tab-separated rows of a file, skipping blank lines.
pub fn tsv(path: &Path) -> Vec<Vec<String>> {
    fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.split('\t').map(str::to_owned).collect())
        .collect()
}

/// Path of a corpus subdirectory (`json`, `provn/spec/prov-n`, ...).
pub fn corpus_dir(sub: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(sub)
}

/// Every file with `ext` in `dir`, sorted by name, as (file name, contents).
/// `PROVKIT_ONLY=<substring>` narrows the set for debugging one case.
pub fn files(dir: &Path, ext: &str) -> Vec<(String, String)> {
    let found = files_unchecked(dir, ext);
    require_a_match(!found.is_empty());
    found
}

/// As [`files`], without failing when the filter leaves nothing, for a test
/// that reads several directories and checks the union with
/// [`require_a_match`].
pub fn files_unchecked(dir: &Path, ext: &str) -> Vec<(String, String)> {
    let only = std::env::var("PROVKIT_ONLY").ok();
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == ext))
        .collect();
    entries.sort();
    entries
        .into_iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let text = fs::read_to_string(&p).unwrap();
            (name, text)
        })
        .filter(|(name, _)| only.as_ref().is_none_or(|o| name.contains(o.as_str())))
        .collect()
}

/// Whether `PROVKIT_ONLY` leaves `name` in the run.
pub fn selected(name: &str) -> bool {
    std::env::var("PROVKIT_ONLY")
        .ok()
        .is_none_or(|only| name.contains(only.as_str()))
}

/// Fails a run whose `PROVKIT_ONLY` filter left no file in the test.
pub fn require_a_match(any_selected: bool) {
    if let Ok(only) = std::env::var("PROVKIT_ONLY") {
        assert!(any_selected, "no file matches PROVKIT_ONLY={only}");
    }
}

/// Collects per-file failures so one run reports them all.
#[derive(Default)]
pub struct Report {
    pub failures: Vec<String>,
    pub checked: usize,
}

impl Report {
    pub fn fail(&mut self, name: &str, why: impl std::fmt::Display) {
        self.failures.push(format!("{name}: {why}"));
    }

    /// Read `text` as PROV-JSON, recording a failure under `name` on error.
    #[cfg(feature = "json")]
    pub fn read_json(&mut self, name: &str, text: &str) -> Option<Document> {
        match provkit::json::read(text) {
            Ok(document) => Some(document),
            Err(e) => {
                self.fail(name, format!("read: {e}"));
                None
            }
        }
    }

    pub fn finish(self, expected: usize) {
        // Skip count assertion when PROVKIT_ONLY filter is set, for debugging.
        let only = std::env::var("PROVKIT_ONLY").ok();
        if only.is_none() {
            assert_eq!(self.checked, expected, "unexpected number of cases checked");
        }
        assert!(
            self.failures.is_empty(),
            "{} of {} cases failed:\n{}",
            self.failures.len(),
            self.checked,
            self.failures.join("\n")
        );
    }
}
