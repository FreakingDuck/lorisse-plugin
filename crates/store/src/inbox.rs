//! Inbox — a capture lands here as a raw memory; its proposals wait here as `<id>.sort.yaml`,
//! `<id>.2.sort.yaml` …, one action each; a skipped one is kept as `.skipped.yaml`.
//!
//! The capture's text is the body byte for byte; only a final newline is added when missing.
//! Flow: text + `Source` + flags + files → `capture` → `inbox/M-<next>.md` (+ `inbox/M-<next>/`);
//! `*.sort.yaml` → `pending`, one `Proposal` per file, by capture then part.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use serde_yaml_ng::Value;

use crate::actions::Action;
use crate::attach;
use crate::config::{StoreConfig, list, status};
use crate::error::StoreError;
use crate::flags::Flags;
use crate::frontmatter::key;
use crate::ids::PieceId;
use crate::index::{
    Index, SKIPPED_SUFFIX, SORT_SUFFIX, io_error, proposal_part, proposal_stem, write_atomic,
};
use crate::piece::Piece;
use crate::repos;

// =============================================================================
// Interface
// =============================================================================

/// Where a capture came from: one of the configuration's `sources`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Source(String);

/// One proposal file of the inbox, read on its own.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    /// The piece it is about, from the file name.
    pub id: PieceId,
    /// The file, relative to the root.
    pub path: PathBuf,
    /// 1 for `<id>.sort.yaml`, n for `<id>.<n>.sort.yaml`.
    pub part: u32,
    /// The action, when the file parses.
    pub action: Option<Action>,
    /// The parser's message, when it does not.
    pub error: Option<String>,
}

impl Source {
    /// A source from its name (`phone`, `chat`, `agent`, `console`, `app`).
    pub fn new(name: &str) -> Source {
        Source(name.trim().to_string())
    }

    /// The source's name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// =============================================================================
// Implementation
// =============================================================================

/// The longest title a capture's first line gives, in characters.
const TITLE_CHARS: usize = 80;

/// The format of a date-time field.
pub(crate) const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%:z";

/// The format of a date field.
pub(crate) const DATE_FORMAT: &str = "%Y-%m-%d";

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The next numbered id of `prefix`.
pub(crate) fn next(index: &Index, prefix: &str) -> PieceId {
    let n = index.max.get(prefix).copied().unwrap_or(0) + 1;
    PieceId::Numbered {
        prefix: prefix.to_string(),
        n,
    }
}

/// The next id of a piece of `layer_prefix`'s layer landing in the repository at `folder`
/// (`None` the root): a numbered row's prefix, else the layer's.
pub(crate) fn next_at(
    index: &Index,
    config: &StoreConfig,
    layer_prefix: &str,
    folder: Option<&str>,
) -> PieceId {
    let own = folder.and_then(|f| repos::prefix_at(config, f));
    next(index, own.unwrap_or(layer_prefix))
}

/// What a capture is given.
pub(crate) struct Given<'a> {
    /// The text, kept as typed.
    pub(crate) text: &'a str,
    /// Where it came from.
    pub(crate) source: &'a Source,
    /// Flags beside those typed in the text.
    pub(crate) flags: &'a Flags,
    /// Files to copy beside it.
    pub(crate) files: &'a [PathBuf],
}

/// Write the capture as `inbox/<id>.md`, its files under `inbox/<id>/`, and return it.
pub(crate) fn capture(
    root: &Path,
    config: &StoreConfig,
    id: &PieceId,
    given: &Given,
) -> Result<Piece, StoreError> {
    let source = given.source;
    if !config
        .list(list::SOURCES)
        .iter()
        .any(|s| s == source.as_str())
    {
        return Err(StoreError::Invalid(format!(
            "source: {source} is not in {}",
            list::SOURCES
        )));
    }
    let id_text = id.padded(config.id_width);
    let rel = PathBuf::from(format!("{}/{id_text}.md", config.inbox));
    if root.join(&rel).exists() {
        return Err(StoreError::Exists(rel));
    }
    let dir = attach::sidecar(&rel);
    let names = attach::check_sources(root, &dir, given.files)?;
    let mut piece = capture_piece(rel, &id_text, given.text, source);
    Flags::parse(given.text)
        .merged(given.flags)
        .apply(&mut piece);
    let files = attach::copy_in(root, &dir, given.files, &names)?;
    if !files.is_empty() {
        piece.set(key::FILES, attach::to_value(&files));
    }
    let written = write_atomic(&root.join(piece.path()), &piece.render());
    if written.is_err() && !files.is_empty() {
        attach::remove_copies(root, &dir);
    }
    written.map(|()| piece)
}

fn capture_piece(rel: PathBuf, id_text: &str, text: &str, source: &Source) -> Piece {
    let now = Local::now().fixed_offset();
    let mut piece = Piece::empty(rel);
    piece.set(key::ID, Value::from(id_text));
    piece.set(key::TITLE, Value::from(first_line(text)));
    piece.set(key::TAGS, Value::Sequence(Vec::new()));
    piece.set(key::SOURCE, Value::from(source.as_str()));
    piece.set(
        key::CAPTURED,
        Value::from(now.format(TIME_FORMAT).to_string()),
    );
    piece.set(
        key::UPDATED,
        Value::from(now.format(DATE_FORMAT).to_string()),
    );
    piece.set(key::STATUS, Value::from(status::RAW));
    piece.set(key::LINKS, Value::Sequence(Vec::new()));
    let mut body = text.to_string();
    if !body.ends_with('\n') {
        body.push('\n');
    }
    piece.set_body(body);
    piece
}

/// The first non-empty line, trimmed, cut at 80 characters.
fn first_line(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty());
    line.unwrap_or("").chars().take(TITLE_CHARS).collect()
}

/// Every proposal file of the inbox, each parsed on its own, by capture then part.
pub(crate) fn pending(root: &Path, config: &StoreConfig) -> Vec<Proposal> {
    let inbox = root.join(&config.inbox);
    let mut files = proposal_names(&inbox, &config.inbox, config);
    let Ok(entries) = fs::read_dir(&inbox) else {
        return Vec::new();
    };
    for dir in entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
    {
        let name = dir.file_name().to_string_lossy().into_owned();
        let rel = format!("{}/{name}", config.inbox);
        files.extend(proposal_names(&dir.path(), &rel, config));
    }
    proposals(root, config, &files)
}

/// The proposal files directly in `dir` (`rel` from the root): a named id's sit one folder down.
fn proposal_names(dir: &Path, rel: &str, config: &StoreConfig) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('.'))
        .map(|name| format!("{rel}/{name}"))
        .filter(|rel| proposal_stem(rel, config).is_some())
        .collect()
}

/// The pending file an action came from: the one holding it, else a sort's `<id>.sort.yaml`.
pub(crate) fn source_of<'a>(pending: &'a [Proposal], action: &Action) -> Option<&'a Proposal> {
    let same = pending.iter().find(|p| p.action.as_ref() == Some(action));
    same.or_else(|| match action {
        Action::Sort { id, .. } => pending.iter().find(|p| &p.id == id && p.part == 1),
        _ => None,
    })
}

/// The first free proposal file name of a capture, skipped names counted as taken.
pub(crate) fn next_path(root: &Path, config: &StoreConfig, id: &PieceId) -> PathBuf {
    let base = id.padded(config.id_width);
    let stem = |n: u32| match n {
        1 => base.clone(),
        n => format!("{base}.{n}"),
    };
    let taken = |stem: &str| {
        [SORT_SUFFIX, SKIPPED_SUFFIX]
            .iter()
            .any(|s| root.join(&config.inbox).join(format!("{stem}{s}")).exists())
    };
    let free = (1..).map(stem).find(|s| !taken(s)).unwrap_or(base.clone());
    PathBuf::from(format!("{}/{free}{SORT_SUFFIX}", config.inbox))
}

/// Rename a proposal file to `<stem>.skipped.yaml`, never over another file.
pub(crate) fn skip_file(root: &Path, rel: &Path) -> Result<(), StoreError> {
    let text = rel.to_string_lossy().replace('\\', "/");
    let stem = text
        .strip_suffix(SORT_SUFFIX)
        .ok_or_else(|| StoreError::Invalid(format!("{text}: not a proposal file")))?;
    let to = PathBuf::from(format!("{stem}{SKIPPED_SUFFIX}"));
    if root.join(&to).exists() {
        return Err(StoreError::Exists(to));
    }
    let from = root.join(rel);
    fs::rename(&from, root.join(&to)).map_err(|e| io_error(&from, e))
}

/// Delete one proposal file; one already gone is no error.
pub(crate) fn remove_file(root: &Path, rel: &Path) -> Result<(), StoreError> {
    let path = root.join(rel);
    match fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io_error(&path, e)),
        _ => Ok(()),
    }
}

/// The given proposal files, each parsed on its own, by capture then part.
pub(crate) fn proposals(root: &Path, config: &StoreConfig, files: &[String]) -> Vec<Proposal> {
    let mut out = Vec::new();
    for rel in files {
        let Some((id, part)) = proposal_stem(rel, config).and_then(proposal_part) else {
            continue;
        };
        let path = root.join(rel);
        let parsed = fs::read_to_string(&path)
            .map_err(|e| io_error(&path, e).to_string())
            .and_then(|text| serde_yaml_ng::from_str(&text).map_err(|e| e.to_string()));
        let (action, error) = match parsed {
            Ok(action) => (Some(action), None),
            Err(error) => (None, Some(error)),
        };
        let path = PathBuf::from(rel);
        out.push(Proposal {
            id,
            path,
            part,
            action,
            error,
        });
    }
    out.sort_by(|a, b| (&a.id, a.part).cmp(&(&b.id, b.part)));
    out
}

/// Every proposal's id and action, or the first file that does not parse.
pub(crate) fn strict(list: Vec<Proposal>) -> Result<Vec<(PieceId, Action)>, StoreError> {
    let pair = |p: Proposal| match (p.action, p.error) {
        (Some(action), _) => Ok((p.id, action)),
        (None, error) => Err(StoreError::Yaml {
            path: p.path,
            reason: error.unwrap_or_default(),
        }),
    };
    list.into_iter().map(pair).collect()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_is_the_first_non_empty_line_cut_at_eighty_characters() {
        assert_eq!(first_line("\n\n  hello  \nworld"), "hello");
        assert_eq!(first_line("   \n"), "");
        assert_eq!(first_line(&"é".repeat(100)).chars().count(), 80);
    }

    #[test]
    fn a_capture_piece_keeps_its_text_and_adds_one_newline() {
        let source = Source::new("console");
        let piece = capture_piece(
            PathBuf::from("inbox/M-0001.md"),
            "M-0001",
            "a\r\nb",
            &source,
        );
        assert_eq!(piece.body(), "a\r\nb\n");
        let text = piece.render();
        assert!(text.starts_with("---\nid: M-0001\ntitle: a\ntags: []\nsource: console\n"));
        let again = capture_piece(PathBuf::from("inbox/M-0001.md"), "M-0001", "a\n", &source);
        assert_eq!(again.body(), "a\n");
    }
}
