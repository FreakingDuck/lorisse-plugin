//! Attachment — a file that goes with a piece, kept in the folder beside it (`<stem>/<name>`).
//!
//! A capture copies the files in; a move of the piece moves them as file-move changes.
//! Flow: paths → `copy_in` → `files:` entries; a move → `relocate` → `Change`s.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_yaml_ng::{Mapping, Value};

use crate::diff::Change;
use crate::error::StoreError;
use crate::frontmatter::key;
use crate::index::io_error;
use crate::piece::Piece;

// =============================================================================
// Interface
// =============================================================================

/// One `files:` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// The file name.
    pub name: String,
    /// The file, relative to the root, forward slashes.
    pub path: String,
    /// `image`, `audio`, `video`, `pdf`, `text` or `file`, from the extension.
    pub kind: String,
}

// =============================================================================
// Implementation
// =============================================================================

/// Extensions per kind; any other is `file`.
const KINDS: [(&str, &[&str]); 5] = [
    (
        "image",
        &["jpg", "jpeg", "png", "gif", "webp", "heic", "bmp", "svg"],
    ),
    (
        "audio",
        &["mp3", "m4a", "wav", "ogg", "flac", "aac", "opus"],
    ),
    ("video", &["mp4", "mov", "webm", "mkv", "avi"]),
    ("pdf", &["pdf"]),
    ("text", &["txt", "md", "csv", "json", "yaml", "yml", "log"]),
];

/// The kind of a file name.
pub(crate) fn kind_of(name: &str) -> &'static str {
    let ext = Path::new(name).extension().map(|e| e.to_string_lossy());
    let ext = ext.map(|e| e.to_lowercase()).unwrap_or_default();
    let found = KINDS.iter().find(|(_, exts)| exts.contains(&ext.as_str()));
    found.map(|(kind, _)| *kind).unwrap_or("file")
}

/// The folder of a piece's files: its path without `.md`.
pub(crate) fn sidecar(md: &Path) -> PathBuf {
    md.with_extension("")
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// `<dir>/<name>`, forward slashes.
fn under(dir: &Path, name: &str) -> PathBuf {
    PathBuf::from(format!("{}/{name}", slashed(dir)))
}

/// Whether `name` is one plain file name.
fn plain_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\'])
}

/// Whether a `files:` value is null or a list of mappings with a text `name`.
pub(crate) fn is_file_list(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Sequence(items) => items.iter().all(|i| {
            let name = i.as_mapping().and_then(|m| m.get(key::FILE_NAME));
            name.and_then(Value::as_str).is_some()
        }),
        _ => false,
    }
}

/// The entries of a `files:` value; an entry with no name is skipped.
pub(crate) fn of_value(value: Option<&Value>) -> Vec<Attachment> {
    let Some(Value::Sequence(items)) = value else {
        return Vec::new();
    };
    let text = |m: &Mapping, k: &str| m.get(k).and_then(Value::as_str).map(str::to_string);
    let entry = |m: &Mapping| {
        Some(Attachment {
            name: text(m, key::FILE_NAME)?,
            path: text(m, key::FILE_PATH).unwrap_or_default(),
            kind: text(m, key::FILE_KIND).unwrap_or_default(),
        })
    };
    items
        .iter()
        .filter_map(|i| entry(i.as_mapping()?))
        .collect()
}

/// The `files:` value of a list.
pub(crate) fn to_value(files: &[Attachment]) -> Value {
    let entry = |a: &Attachment| {
        let mut m = Mapping::new();
        m.insert(key::FILE_NAME.into(), a.name.as_str().into());
        m.insert(key::FILE_PATH.into(), a.path.as_str().into());
        m.insert(key::FILE_KIND.into(), a.kind.as_str().into());
        Value::Mapping(m)
    };
    Value::Sequence(files.iter().map(entry).collect())
}

/// The sources' names, checked: each a readable file, no name twice, the folder free.
pub(crate) fn check_sources(
    root: &Path,
    dir: &Path,
    sources: &[PathBuf],
) -> Result<Vec<String>, StoreError> {
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for source in sources {
        let meta = fs::metadata(source).map_err(|e| io_error(source, e))?;
        let name = source.file_name().map(|n| n.to_string_lossy().into_owned());
        let name = name.filter(|n| meta.is_file() && plain_name(n));
        let name =
            name.ok_or_else(|| StoreError::Invalid(format!("{}: not a file", source.display())))?;
        if !seen.insert(name.clone()) {
            return Err(StoreError::Invalid(format!("{name}: given twice")));
        }
        names.push(name);
    }
    if !sources.is_empty() && root.join(dir).exists() {
        return Err(StoreError::Exists(dir.to_path_buf()));
    }
    Ok(names)
}

/// Copy the sources into `dir`; on a failure the copies made here are removed.
pub(crate) fn copy_in(
    root: &Path,
    dir: &Path,
    sources: &[PathBuf],
    names: &[String],
) -> Result<Vec<Attachment>, StoreError> {
    let mut out = Vec::new();
    for (source, name) in sources.iter().zip(names) {
        let rel = under(dir, name);
        if let Err(e) = copy_file(source, &root.join(&rel)) {
            remove_copies(root, dir);
            return Err(e);
        }
        out.push(Attachment {
            name: name.clone(),
            path: slashed(&rel),
            kind: kind_of(name).to_string(),
        });
    }
    Ok(out)
}

/// Remove a folder this capture created, with what it holds.
pub(crate) fn remove_copies(root: &Path, dir: &Path) {
    let _ = fs::remove_dir_all(root.join(dir));
}

/// Copy `from` to `to` through a temporary file; parent folders created.
pub(crate) fn copy_file(from: &Path, to: &Path) -> Result<(), StoreError> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
    }
    let mut tmp = to.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::copy(from, &tmp).map_err(|e| io_error(from, e))?;
    fs::rename(&tmp, to).map_err(|e| io_error(to, e))
}

/// Point the piece's files beside `to` and give the file moves from beside `from`.
pub(crate) fn relocate(
    piece: &mut Piece,
    from: &Path,
    to: &Path,
) -> Result<Vec<Change>, StoreError> {
    let files = piece.files();
    if files.is_empty() || from == to {
        return Ok(Vec::new());
    }
    let (old, new) = (sidecar(from), sidecar(to));
    let mut moved = Vec::new();
    let mut changes = Vec::new();
    for file in files {
        if !plain_name(&file.name) {
            return Err(StoreError::Invalid(format!(
                "files: {} is not a file name",
                file.name
            )));
        }
        let target = under(&new, &file.name);
        changes.push(Change {
            path: target.clone(),
            from: Some(under(&old, &file.name)),
            before: None,
            after: None,
        });
        moved.push(Attachment {
            path: slashed(&target),
            ..file
        });
    }
    piece.set(key::FILES, to_value(&moved));
    Ok(changes)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_comes_from_the_extension_in_any_case() {
        assert_eq!(kind_of("probe.JPG"), "image");
        assert_eq!(kind_of("memo.m4a"), "audio");
        assert_eq!(kind_of("a.tar.gz"), "file");
        assert_eq!(kind_of("README"), "file");
        assert_eq!(
            sidecar(Path::new("inbox/M-0001.md")),
            Path::new("inbox/M-0001")
        );
        assert!(!plain_name("..") && !plain_name("a/b") && plain_name("a.b"));
    }

    #[test]
    fn a_file_list_reads_back_from_its_value() {
        let files = vec![Attachment {
            name: "p q.jpg".into(),
            path: "inbox/M-1/p q.jpg".into(),
            kind: "image".into(),
        }];
        let value = to_value(&files);
        assert!(is_file_list(&value));
        assert_eq!(of_value(Some(&value)), files);
        assert!(!is_file_list(&Value::from("x")));
    }
}
