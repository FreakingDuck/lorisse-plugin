//! Diff — what the human reviews and accepts: one `Change` per file, before and after.
//!
//! Accepting checks every change first (the file still holds `before`, a create finds no
//! file, the gate, the lists) and writes nothing on a refusal; then each `after` goes through
//! a temporary file renamed over the old, and only then are old files removed; the proposal
//! file the action came from goes last.
//! Flow: `Diff` → `lines` (review) → `accept`.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};

use crate::actions::{self, Action};
use crate::attach;
use crate::check;
use crate::error::StoreError;
use crate::ids::{self, PieceId};
use crate::inbox;
use crate::index::{io_error, write_atomic};
use crate::piece::Piece;
use crate::slice::Place;
use crate::store::{Access, Store};

// =============================================================================
// Interface
// =============================================================================

/// A proposed action with the files it changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Diff {
    /// What was proposed.
    pub action: Action,
    /// When the diff was made.
    pub made_at: DateTime<FixedOffset>,
    /// One change per file.
    pub changes: Vec<Change>,
    /// The frontmatter keys a move to another layer drops.
    pub dropped: Vec<String>,
}

/// One file of a diff; a move is one change with `from` set; a file move has no texts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The file written, relative to the root.
    pub path: PathBuf,
    /// A move's old path, removed after the write.
    pub from: Option<PathBuf>,
    /// The text the diff was made on; `None` = a create or a file move.
    pub before: Option<String>,
    /// The text written; `None` = a delete or a file move.
    pub after: Option<String>,
}

impl Change {
    /// Whether it moves an attached file as is: `from` set, no texts.
    pub fn is_file_move(&self) -> bool {
        self.from.is_some() && self.before.is_none() && self.after.is_none()
    }
}

impl Diff {
    /// The review: `dropped: <keys>` when a move drops any, then per change a header and a line
    /// diff with `-`, `+` and ` ` prefixes.
    pub fn lines(&self) -> Vec<String> {
        let dropped = (!self.dropped.is_empty()).then(|| dropped_line(&self.dropped));
        dropped
            .into_iter()
            .chain(self.changes.iter().flat_map(change_lines))
            .collect()
    }
}

// =============================================================================
// Implementation
// =============================================================================

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn dropped_line(keys: &[String]) -> String {
    format!("dropped: {}", keys.join(", "))
}

fn change_lines(change: &Change) -> Vec<String> {
    let path = slashed(&change.path);
    let header = match (&change.from, &change.before, &change.after) {
        (Some(from), _, _) => format!("move {} → {path}", slashed(from)),
        (None, None, _) => format!("create {path}"),
        (None, Some(_), None) => format!("delete {path}"),
        (None, Some(_), Some(_)) => format!("edit {path}"),
    };
    let before = change.before.as_deref().unwrap_or("");
    let after = change.after.as_deref().unwrap_or("");
    let mut out = vec![header];
    out.extend(line_diff(before, after));
    out
}

/// The lines of a text, line endings dropped.
fn split_lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

/// A line diff over the longest common subsequence.
pub(crate) fn line_diff(before: &str, after: &str) -> Vec<String> {
    let (a, b) = (split_lines(before), split_lines(after));
    let mut lcs = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            out.push(format!(" {}", a[i]));
            (i, j) = (i + 1, j + 1);
        } else if j == b.len() || (i < a.len() && lcs[i + 1][j] >= lcs[i][j + 1]) {
            out.push(format!("-{}", a[i]));
            i += 1;
        } else {
            out.push(format!("+{}", b[j]));
            j += 1;
        }
    }
    out
}

/// Write the diff, then remove the proposal file it came from; the ids written.
pub(crate) fn accept(store: &Store, diff: &Diff) -> Result<Vec<PieceId>, StoreError> {
    let source = source_file(store, &diff.action);
    let written = write(store, diff)?;
    if let Some(path) = source {
        inbox::remove_file(&store.root, &path)?;
    }
    Ok(written)
}

/// Delete the proposal file the diff came from, if any.
pub(crate) fn reject(store: &Store, diff: &Diff) -> Result<(), StoreError> {
    match source_file(store, &diff.action) {
        Some(path) => inbox::remove_file(&store.root, &path),
        None => Ok(()),
    }
}

/// The pending proposal file an action came from; none for a free store.
fn source_file(store: &Store, action: &Action) -> Option<PathBuf> {
    if store.access == Access::Free {
        return None;
    }
    let pending = inbox::pending(&store.root, &store.config);
    inbox::source_of(&pending, action).map(|p| p.path.clone())
}

/// Check every change, then write them all; the ids written.
pub(crate) fn write(store: &Store, diff: &Diff) -> Result<Vec<PieceId>, StoreError> {
    actions::gate(store, &diff.changes)?;
    for change in &diff.changes {
        check_base(&store.root, change)?;
    }
    let mut written = Vec::new();
    for change in &diff.changes {
        written.extend(check_after(store, change)?);
    }
    for change in &diff.changes {
        write_change(&store.root, change)?;
    }
    for change in &diff.changes {
        remove_old(&store.root, change)?;
    }
    written.sort();
    written.dedup();
    Ok(written)
}

/// The file still holds `before`; a moved file is there; a create or a target finds no file.
fn check_base(root: &Path, change: &Change) -> Result<(), StoreError> {
    let source = change.from.as_ref().unwrap_or(&change.path);
    if change.is_file_move() && !root.join(source).is_file() {
        return Err(StoreError::Changed(source.clone()));
    }
    let target_taken = root.join(&change.path).exists();
    match &change.before {
        None if target_taken => return Err(StoreError::Exists(change.path.clone())),
        None => {}
        Some(before) => {
            let now = fs::read_to_string(root.join(source)).ok();
            if now.as_deref() != Some(before.as_str()) {
                return Err(StoreError::Changed(source.clone()));
            }
        }
    }
    if change.from.as_ref().is_some_and(|f| *f != change.path) && target_taken {
        return Err(StoreError::Exists(change.path.clone()));
    }
    Ok(())
}

/// The id of the piece written, after checking its values against the lists.
fn check_after(store: &Store, change: &Change) -> Result<Option<PieceId>, StoreError> {
    let Some(after) = &change.after else {
        return Ok(None);
    };
    let piece = Piece::parse(&change.path, after)?;
    let rel = slashed(&change.path);
    let id = piece
        .id()
        .or_else(|| ids::from_path(&rel, &store.config.archive));
    if let Some(place) = Place::of(&rel, &store.config) {
        let outside = check::outside_lists(place.layer, &piece, &store.config);
        if !outside.is_empty() {
            let name = id.as_ref().map(|i| i.to_string()).unwrap_or(rel);
            return Err(StoreError::Invalid(format!(
                "{name}: {}",
                outside.join("; ")
            )));
        }
    }
    Ok(id)
}

/// Write the change's text, or copy the moved file; nothing is removed here.
fn write_change(root: &Path, change: &Change) -> Result<(), StoreError> {
    let path = root.join(&change.path);
    match (&change.after, &change.from) {
        (Some(after), _) => write_atomic(&path, after),
        (None, Some(from)) if change.is_file_move() => attach::copy_file(&root.join(from), &path),
        _ => Ok(()),
    }
}

/// Remove a delete's file, a move's old file, and a moved file's folder once empty.
fn remove_old(root: &Path, change: &Change) -> Result<(), StoreError> {
    let old = match (&change.after, &change.from) {
        (None, None) => root.join(&change.path),
        (_, Some(from)) if *from != change.path => root.join(from),
        _ => return Ok(()),
    };
    fs::remove_file(&old).map_err(|e| io_error(&old, e))?;
    if change.is_file_move() {
        let _ = old.parent().map(fs::remove_dir);
    }
    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_diff_keeps_the_common_lines_and_marks_the_rest() {
        let before = "a\nb\nc\nd\n";
        let after = "a\nc\nx\nd\ne\n";
        assert_eq!(
            line_diff(before, after),
            [" a", "-b", " c", "+x", " d", "+e"]
        );
        assert_eq!(line_diff("", "new\n"), ["+new"]);
        assert_eq!(line_diff("old\r\n", ""), ["-old"]);
        assert_eq!(line_diff("same\n", "same\n"), [" same"]);
    }

    #[test]
    fn a_move_and_a_create_name_their_paths() {
        let moved = Change {
            path: PathBuf::from("memories/M-1.md"),
            from: Some(PathBuf::from("inbox/M-1.md")),
            before: Some("x\n".into()),
            after: Some("y\n".into()),
        };
        assert_eq!(
            change_lines(&moved),
            ["move inbox/M-1.md → memories/M-1.md", "-x", "+y"]
        );
        let created = Change {
            from: None,
            before: None,
            ..moved
        };
        assert_eq!(change_lines(&created)[0], "create memories/M-1.md");
    }
}
