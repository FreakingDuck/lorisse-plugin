//! Index — one `Entry` per piece, rebuilt from the files at every open; `index.json`.
//!
//! The walk reads only what the path rule names; a free store reads only file names under
//! the gate. Problems found on the way go to the check, never into the index.
//! Flow: the folder → `scan` → `Scan` (entries, hidden ids, numbers, problems) → `index.json`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::check::Finding;
use crate::config::StoreConfig;
use crate::error::StoreError;
use crate::ids::{self, PieceId};
use crate::layer::Layer;
use crate::links::Ref;
use crate::piece::Piece;
use crate::repos;
use crate::slice::{self, MD, Place};
use crate::store::Access;

// =============================================================================
// Interface
// =============================================================================

/// The derived index file at the root.
pub const INDEX_FILE: &str = "index.json";

/// The file at the root, and in a row's folder for its own prefix, keeping the largest number ever given, so a freed id is never given again.
pub const IDS_FILE: &str = "ids.json";

/// One index row: enough to list a piece without reading its file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The `id:` field, else the folder and the stem.
    pub id: PieceId,
    /// The file, relative to the root, forward slashes.
    pub path: PathBuf,
    /// The name of the repository row holding it; `None` at the root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// The slice folder, if any.
    pub slice: Option<String>,
    /// The layer, from the folder.
    pub layer: Layer,
    /// In a confidential repository (its row's flag, or `confidential/`).
    pub confidential: bool,
    /// The model providers its repository row allows; empty at the root or for a row without a list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
    /// Under an archive folder.
    pub archived: bool,
    /// The title, name or date.
    pub title: String,
    /// The tags.
    pub tags: Vec<String>,
    /// The status word.
    pub status: Option<String>,
    /// The links, as bare ids (a path's `.md` and folders dropped).
    pub links: Vec<Ref>,
    /// When it was captured.
    pub captured: Option<DateTime<FixedOffset>>,
    /// The last change.
    pub updated: Option<NaiveDate>,
}

// =============================================================================
// Implementation
// =============================================================================

/// The proposal file suffix in the inbox.
pub(crate) const SORT_SUFFIX: &str = ".sort.yaml";

/// The suffix a skipped proposal file is renamed to.
pub(crate) const SKIPPED_SUFFIX: &str = ".skipped.yaml";

/// The pieces a store can see, and what it knows of the rest.
#[derive(Debug, Default)]
pub(crate) struct Index {
    /// The visible pieces, in id order.
    pub(crate) entries: Vec<Entry>,
    /// The ids behind the gate, from their file names.
    pub(crate) hidden: BTreeSet<PieceId>,
    /// The largest number per prefix, over every file name and `id:` read in its numbering.
    pub(crate) max: BTreeMap<String, u64>,
}

/// One walk of the folder: the index and what the check reports.
#[derive(Debug, Default)]
pub(crate) struct Scan {
    /// The index.
    pub(crate) index: Index,
    /// Unreadable files and ids used twice.
    pub(crate) problems: Vec<Finding>,
    /// Each indexed piece with its entry and its text.
    pub(crate) pieces: Vec<(Entry, Piece, String)>,
    /// The `inbox/*.sort.yaml` files.
    pub(crate) proposals: Vec<String>,
    /// The `archive/*.md` files with no layer prefix.
    pub(crate) unplaced: Vec<String>,
}

impl Entry {
    /// The path as text with forward slashes.
    pub(crate) fn path_text(&self) -> String {
        self.path.to_string_lossy().replace('\\', "/")
    }

    fn of(id: PieceId, rel: &str, place: Place, piece: &Piece, config: &StoreConfig) -> Entry {
        let repo = place.repo.as_deref().and_then(|f| repos::row_at(config, f));
        let providers = repo.as_ref().map(|r| r.providers.clone());
        Entry {
            id,
            path: PathBuf::from(rel),
            repo: repo.map(|r| r.name),
            slice: place.slice,
            layer: place.layer,
            confidential: place.confidential,
            providers: providers.unwrap_or_default(),
            archived: place.archived,
            title: piece.title().to_string(),
            tags: piece.tags(),
            status: piece.status().map(str::to_string),
            links: piece.links().iter().map(Ref::bare).collect(),
            captured: piece.captured(),
            updated: piece.updated(),
        }
    }
}

impl Index {
    /// The entry with this id.
    pub(crate) fn get(&self, id: &PieceId) -> Option<&Entry> {
        self.entries.iter().find(|e| &e.id == id)
    }

    /// Count `id`'s number at `rel` for `next`; an id of another prefix inside a numbered row counts nowhere.
    fn count(&mut self, id: &PieceId, rel: &str, config: &StoreConfig) {
        let PieceId::Numbered { prefix, n } = id else {
            return;
        };
        let numbering = repos::numbering_of(config, rel, Some(prefix));
        if numbering.is_some_and(|own| own != *prefix) {
            return;
        }
        let max = self.max.entry(prefix.clone()).or_default();
        *max = (*max).max(*n);
    }
}

/// An I/O error naming the path.
pub(crate) fn io_error(path: &Path, e: std::io::Error) -> StoreError {
    StoreError::Io(format!("{}: {e}", path.display()))
}

/// Walk the folder and build the index.
pub(crate) fn scan(root: &Path, config: &StoreConfig, access: Access) -> Result<Scan, StoreError> {
    let mut files = Vec::new();
    walk(root, "", 0, &mut files)?;
    files.sort();
    let mut scan = Scan::default();
    let mut found: Vec<(Entry, Piece, String)> = Vec::new();
    for rel in files {
        if proposal_stem(&rel, config).is_some() {
            scan.proposals.push(rel);
            continue;
        }
        let Some(place) = Place::of(&rel, config) else {
            if is_archive_file(&rel, config) {
                scan.unplaced.push(rel);
            }
            continue;
        };
        if let Some(id) = ids::from_path(&rel, &config.archive) {
            scan.index.count(&id, &rel, config);
        }
        if access == Access::Free && slice::is_gated(&rel, config) {
            scan.index
                .hidden
                .extend(ids::from_path(&rel, &config.archive));
            continue;
        }
        match read_piece(root, &rel) {
            Ok((piece, text)) => {
                let id = piece
                    .id()
                    .or_else(|| ids::named_from_path(&rel, &config.archive));
                let Some(id) = id else { continue };
                scan.index.count(&id, &rel, config);
                found.push((Entry::of(id, &rel, place, &piece, config), piece, text));
            }
            Err(reason) => scan.problems.push(Finding::at(&rel, None, reason)),
        }
    }
    keep_unique(&mut scan, found, config);
    Ok(scan)
}

/// Write the entries to `index.json`.
pub(crate) fn write_index(root: &Path, entries: &[Entry]) -> Result<(), StoreError> {
    let path = root.join(INDEX_FILE);
    let text = serde_json::to_string_pretty(entries)
        .map_err(|e| StoreError::Io(format!("{}: {e}", path.display())))?;
    write_atomic(&path, &(text + "\n"))
}

/// Raise `max` to the numbers each `ids.json` kept, then keep the result there: the root's for
/// every prefix but the rows', each numbered row's in its folder for its own (a row absent here
/// left alone); an unreadable file counts as empty.
pub(crate) fn keep_high(
    root: &Path,
    config: &StoreConfig,
    max: &mut BTreeMap<String, u64>,
) -> Result<(), StoreError> {
    let rows = repos::numbered_rows(config);
    let theirs = |p: &str| rows.iter().any(|(_, own)| *own == p);
    keep_in(&root.join(IDS_FILE), max, &|p| !theirs(p))?;
    for (folder, own) in &rows {
        let dir = root.join(folder);
        if dir.is_dir() {
            keep_in(&dir.join(IDS_FILE), max, &|p| p == *own)?;
        }
    }
    Ok(())
}

/// One `ids.json`: the prefixes `mine` names raised both ways, written when it changed.
fn keep_in(
    path: &Path,
    max: &mut BTreeMap<String, u64>,
    mine: &dyn Fn(&str) -> bool,
) -> Result<(), StoreError> {
    let kept: BTreeMap<String, u64> = fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    for (prefix, n) in kept.iter().filter(|(p, _)| mine(p)) {
        let high = max.entry(prefix.clone()).or_default();
        *high = (*high).max(*n);
    }
    let file: BTreeMap<String, u64> = max
        .iter()
        .filter(|(p, _)| mine(p))
        .map(|(p, n)| (p.clone(), *n))
        .collect();
    if file == kept {
        return Ok(());
    }
    let text = serde_json::to_string_pretty(&file)
        .map_err(|e| StoreError::Io(format!("{}: {e}", path.display())))?;
    write_atomic(path, &(text + "\n"))
}

/// Write `text` through a temporary file renamed over `path`; parent folders created.
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, text).map_err(|e| io_error(&tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| io_error(path, e))
}

/// The id of an inbox proposal file, `inbox/<id>.sort.yaml`.
pub(crate) fn proposal_stem<'a>(rel: &'a str, config: &StoreConfig) -> Option<&'a str> {
    let (dir, file) = rel.split_once('/')?;
    if dir != config.inbox {
        return None;
    }
    let stem = file.strip_suffix(SORT_SUFFIX)?;
    match stem.split_once('/') {
        None => Some(stem),
        Some((folder, name)) => named_stem(folder, name).then_some(stem),
    }
}

/// Whether `<folder>/<name>` is a named id's proposal stem (`tags/bois`): one folder deep, not a number.
fn named_stem(folder: &str, name: &str) -> bool {
    let plain = |s: &str| !s.is_empty() && !s.contains('/') && !s.starts_with('.');
    plain(folder) && plain(name) && crate::ids::numbered(folder).is_none()
}

/// The capture and the part of a proposal stem: `M-0007` → 1, `M-0007.2` → 2, `tags/bois.2` → 2.
pub(crate) fn proposal_part(stem: &str) -> Option<(PieceId, u32)> {
    let numbered_part = stem.rsplit_once('.').and_then(|(head, part)| {
        let digits = !part.is_empty() && part.chars().all(|c| c.is_ascii_digit());
        let part: u32 = part.parse().ok().filter(|p| digits && *p >= 2)?;
        Some((PieceId::parse(head)?, part))
    });
    numbered_part.or_else(|| PieceId::parse(stem).map(|id| (id, 1)))
}

/// Whether `rel` is a `.md` file directly in the archive folder of the root or of a repository.
fn is_archive_file(rel: &str, config: &StoreConfig) -> bool {
    let rest = match rel.split_once('/') {
        Some((first, rest)) if repos::row_at(config, first).is_some() => rest,
        _ => rel,
    };
    match rest.split_once('/') {
        Some((dir, file)) => dir == config.archive && !file.contains('/') && file.ends_with(MD),
        None => false,
    }
}

/// The relative paths of every file at most five segments deep, dot folders skipped; a link
/// at the root to a folder (a repository's junction) is walked as that folder.
fn walk(dir: &Path, rel: &str, depth: usize, out: &mut Vec<String>) -> Result<(), StoreError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(io_error(dir, e)),
    };
    for entry in entries {
        let entry = entry.map_err(|e| io_error(dir, e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let child = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        let kind = entry.file_type().map_err(|e| io_error(&entry.path(), e))?;
        let linked = depth == 0 && kind.is_symlink() && entry.path().is_dir();
        if (kind.is_dir() || linked) && depth < 4 {
            walk(&entry.path(), &child, depth + 1, out)?;
        } else if kind.is_file() {
            out.push(child);
        }
    }
    Ok(())
}

/// A file's piece and text, or the reason it cannot be read.
fn read_piece(root: &Path, rel: &str) -> Result<(Piece, String), String> {
    let text = fs::read_to_string(root.join(rel)).map_err(|e| format!("cannot read: {e}"))?;
    match Piece::parse(Path::new(rel), &text) {
        Ok(piece) => Ok((piece, text)),
        Err(StoreError::Yaml { reason, .. }) => Err(format!("not YAML: {reason}")),
        Err(StoreError::Format { reason, .. }) => Err(reason),
        Err(other) => Err(other.to_string()),
    }
}

/// Index the pieces whose id is used once in its numbering; report the others, every path named.
fn keep_unique(scan: &mut Scan, found: Vec<(Entry, Piece, String)>, config: &StoreConfig) {
    let key = |e: &Entry| {
        let numbering = repos::numbering_of(config, &e.path_text(), e.id.prefix());
        (numbering, e.id.clone())
    };
    let mut paths: BTreeMap<(Option<String>, PieceId), Vec<String>> = BTreeMap::new();
    for (entry, _, _) in &found {
        paths.entry(key(entry)).or_default().push(entry.path_text());
    }
    for (entry, piece, text) in found {
        let same = &paths[&key(&entry)];
        if same.len() > 1 {
            let others: Vec<&str> = same
                .iter()
                .filter(|p| **p != entry.path_text())
                .map(String::as_str)
                .collect();
            let text = format!("id {} used twice, also in {}", entry.id, others.join(", "));
            scan.problems.push(Finding::at(
                &entry.path_text(),
                Some(entry.id.clone()),
                text,
            ));
            continue;
        }
        scan.index.entries.push(entry.clone());
        scan.pieces.push((entry, piece, text));
    }
    scan.index
        .entries
        .sort_by(|a, b| a.id.cmp(&b.id).then(a.path.cmp(&b.path)));
    scan.pieces.sort_by(|a, b| a.0.id.cmp(&b.0.id));
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proposal_file_is_named_by_its_id() {
        let config = StoreConfig::default();
        assert_eq!(
            proposal_stem("inbox/M-0007.sort.yaml", &config),
            Some("M-0007")
        );
        assert_eq!(proposal_stem("memories/M-0007.sort.yaml", &config), None);
        assert_eq!(proposal_stem("inbox/M-0007/x.sort.yaml", &config), None);
        let m7 = PieceId::parse("M-0007").unwrap();
        assert_eq!(proposal_part("M-0007"), Some((m7.clone(), 1)));
        assert_eq!(proposal_part("M-0007.12"), Some((m7, 12)));
        for not in ["M-0007.1", "M-0007.x", "M-0007.", "nope.2"] {
            assert_eq!(proposal_part(not), None, "{not}");
        }
        assert!(is_archive_file("confidential/archive/notes.md", &config));
        assert!(!is_archive_file("archive/x/notes.md", &config));
    }
}
