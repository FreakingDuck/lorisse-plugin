//! Action — what an agent or the owner proposes; `propose` turns it into a `Diff`.
//!
//! Proposing reads files and writes nothing. An `Action` is YAML (`action: sort`), so a
//! proposal can be a file in the inbox. Every rewritten piece gets `updated: today`.
//! Flow: `Action` → `propose` (load, apply, render) → `Diff` → the human → `accept`.

use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, FixedOffset, Local};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use crate::attach;
use crate::config::status;
use crate::diff::{Change, Diff};
use crate::error::StoreError;
use crate::frontmatter::key;
use crate::ids::PieceId;
use crate::inbox::{self, DATE_FORMAT, Proposal, TIME_FORMAT, next_at};
use crate::index::{Entry, io_error};
use crate::layer::{self, Layer};
use crate::links::{self, Ref};
use crate::notes;
use crate::piece::{Piece, parse_date};
use crate::resolution;
use crate::slice::{self, Place};
use crate::store::{Access, Store};

// =============================================================================
// Interface
// =============================================================================

/// Frontmatter fields to set, in order: key → YAML value.
pub type Fields = serde_yaml_ng::Mapping;

/// One proposed change to the folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase", deny_unknown_fields)]
pub enum Action {
    /// Sort a memory: into its place, or into another layer's new piece.
    Sort {
        /// The memory sorted.
        id: PieceId,
        /// The layer it goes to.
        layer: Layer,
        /// Under `confidential/`.
        #[serde(default)]
        confidential: bool,
        /// The slice folder, if any.
        #[serde(default)]
        slice: Option<String>,
        /// The fields to set (title, kind, tags, links, priority, date, name …).
        #[serde(default)]
        set: Fields,
        /// Who proposed it.
        #[serde(default)]
        sorted_by: Option<String>,
    },
    /// Set and remove fields, replace the body.
    Edit {
        /// The piece edited.
        id: PieceId,
        /// The fields to set.
        #[serde(default)]
        set: Fields,
        /// The keys to remove.
        #[serde(default)]
        unset: Vec<String>,
        /// The new body, if it changes.
        #[serde(default)]
        body: Option<String>,
    },
    /// Append links not yet present.
    Link {
        /// The piece linking.
        from: PieceId,
        /// The targets.
        to: Vec<Ref>,
    },
    /// Merge two memories, tasks or people into the one kept; retire the other, relink to the kept.
    Dedupe {
        /// The older id, kept (a person: the one named here).
        keep: PieceId,
        /// The newer id, retired.
        drop: PieceId,
    },
    /// Close a task (done, its closing line) when it is not, and move it to the archive.
    Archive {
        /// The task.
        id: PieceId,
    },
    /// Retire a memory; behind a résumé, `retired_by` names it.
    Retire {
        /// The memory.
        id: PieceId,
        /// The résumé it sits behind, when one sums it up.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        behind: Option<PieceId>,
    },
    /// Move a filed piece to another layer, or into or out of `confidential/`.
    Move {
        /// The piece moved.
        id: PieceId,
        /// The layer it goes to; `None` keeps its own.
        #[serde(default)]
        layer: Option<Layer>,
        /// Under `confidential/` or not; `None` keeps where it is.
        #[serde(default)]
        confidential: Option<bool>,
        /// The new layer's fields to set in the same move.
        #[serde(default)]
        set: Fields,
    },
    /// Rename a tag on every piece carrying it, its tag file renamed or merged into the new one's.
    #[serde(rename = "rename_tag")]
    RenameTag {
        /// The tag renamed.
        from: String,
        /// Its new word.
        to: String,
        /// Why, one line.
        #[serde(default)]
        why: Option<String>,
    },
}

impl Action {
    /// The piece the action is about; a tag rename: the new tag's piece, `tags/<slug of to>`.
    pub fn target(&self) -> PieceId {
        target_of(self)
    }
}

// =============================================================================
// Implementation
// =============================================================================

mod reshape;

fn target_of(action: &Action) -> PieceId {
    match action {
        Action::Sort { id, .. }
        | Action::Edit { id, .. }
        | Action::Archive { id }
        | Action::Retire { id, .. }
        | Action::Move { id, .. } => id.clone(),
        Action::Link { from, .. } => from.clone(),
        Action::Dedupe { keep, .. } => keep.clone(),
        Action::RenameTag { to, .. } => PieceId::Named {
            folder: layer::TAGS.into(),
            stem: slug(to),
        },
    }
}

/// A piece read for an action: its entry, parsed piece and exact text.
struct Loaded {
    entry: Entry,
    piece: Piece,
    text: String,
}

/// The clock of one proposal.
struct Now {
    time: DateTime<FixedOffset>,
}

impl Now {
    fn time_text(&self) -> Value {
        Value::from(self.time.format(TIME_FORMAT).to_string())
    }

    fn date_text(&self) -> Value {
        Value::from(self.time.format(DATE_FORMAT).to_string())
    }
}

/// The inbox's pending proposal files around one action.
#[derive(Default)]
struct Files {
    /// Every pending file, by capture then part.
    pending: Vec<Proposal>,
    /// The index of the file the action came from.
    source: Option<usize>,
}

impl Files {
    /// The pending files a full store reads; none for a free one.
    fn of(store: &Store, action: &Action) -> Files {
        if store.access == Access::Free {
            return Files::default();
        }
        let pending = inbox::pending(&store.root, &store.config);
        let source = inbox::source_of(&pending, action).map(|s| s.path.clone());
        let source = source.and_then(|path| pending.iter().position(|p| p.path == path));
        Files { pending, source }
    }

    fn source(&self) -> Option<&Proposal> {
        self.source.map(|i| &self.pending[i])
    }

    /// The pending files of capture `id` other than the source.
    fn others(&self, id: &PieceId) -> usize {
        let source = self.source().map(|s| &s.path);
        let other = |p: &&Proposal| &p.id == id && Some(&p.path) != source;
        self.pending.iter().filter(other).count()
    }
}

/// The diff of an action; nothing is written.
pub(crate) fn propose(store: &Store, action: Action) -> Result<Diff, StoreError> {
    let now = Now {
        time: Local::now().fixed_offset(),
    };
    let files = Files::of(store, &action);
    let mut dropped = Vec::new();
    let mut changes = match &action {
        Action::Sort { .. } => sort(store, &action, &files, &now)?,
        Action::Edit {
            id,
            set,
            unset,
            body,
        } => edit(store, id, set, unset, body, &now)?,
        Action::Link { from, to } => link(store, from, to, &now)?,
        Action::Dedupe { keep, drop } => dedupe(store, keep, drop, &now)?,
        Action::Archive { id } => archive(store, id, &now)?,
        Action::Retire { id, behind } => retire(store, id, behind.as_ref(), &now)?,
        Action::Move {
            id,
            layer,
            confidential,
            set,
        } => {
            let to = reshape::Target {
                layer: *layer,
                confidential: *confidential,
                set,
            };
            let (changes, keys) = reshape::move_piece(store, id, &to, &now)?;
            dropped = keys;
            changes
        }
        Action::RenameTag { from, to, .. } => reshape::rename_tag(store, from, to, &now)?,
    };
    follow_capture(store, &action, &files, &mut changes, &now)?;
    gate(store, &changes)?;
    Ok(Diff {
        action,
        made_at: now.time,
        changes,
        dropped,
    })
}

/// Refuse, in a free store, a change under `confidential/` or the inbox.
pub(crate) fn gate(store: &Store, changes: &[Change]) -> Result<(), StoreError> {
    if store.access == Access::Free {
        for change in changes {
            for path in std::iter::once(&change.path).chain(&change.from) {
                let rel = path.to_string_lossy().replace('\\', "/");
                if slice::is_gated(&rel, &store.config) {
                    let id = crate::ids::from_path(&rel, &store.config.archive);
                    let name = id.map(|i| i.to_string()).unwrap_or(rel);
                    return Err(StoreError::Confidential(name));
                }
            }
        }
    }
    Ok(())
}

fn load(store: &Store, id: &PieceId) -> Result<Loaded, StoreError> {
    if store.index.hidden.contains(id) {
        return Err(StoreError::Confidential(id.to_string()));
    }
    let entry = store
        .index
        .get(id)
        .ok_or_else(|| StoreError::Unknown(id.to_string()))?;
    let path = store.root.join(&entry.path);
    let text = fs::read_to_string(&path).map_err(|e| io_error(&path, e))?;
    let piece = Piece::parse(&entry.path, &text)?;
    Ok(Loaded {
        entry: entry.clone(),
        piece,
        text,
    })
}

/// The file name of a loaded piece.
fn file_name(loaded: &Loaded) -> String {
    let path = loaded.entry.path_text();
    path.rsplit('/').next().unwrap_or_default().to_string()
}

fn apply(piece: &mut Piece, set: &Fields) -> Result<(), StoreError> {
    for (k, v) in set {
        let key = k
            .as_str()
            .ok_or_else(|| StoreError::Invalid(format!("a field name that is not text: {k:?}")))?;
        piece.set(key, v.clone());
    }
    if set.contains_key(key::RESOLUTIONS) {
        resolution::derive(piece).map_err(StoreError::Invalid)?;
    }
    Ok(())
}

/// The text a diff writes for a piece: its links bare.
fn written(piece: &Piece) -> String {
    let mut piece = piece.clone();
    if let Some(Value::Sequence(items)) = piece.get(key::LINKS) {
        let bare = |v: &Value| match v {
            Value::String(s) => Value::from(Ref::new(s).bare().as_str()),
            other => other.clone(),
        };
        let items: Vec<Value> = items.iter().map(bare).collect();
        piece.set(key::LINKS, Value::Sequence(items));
    }
    piece.render()
}

fn edit_change(loaded: &Loaded, piece: &Piece) -> Change {
    Change {
        path: loaded.entry.path.clone(),
        from: None,
        before: Some(loaded.text.clone()),
        after: Some(written(piece)),
    }
}

fn create_change(piece: &Piece) -> Change {
    Change {
        path: piece.path().to_path_buf(),
        from: None,
        before: None,
        after: Some(written(piece)),
    }
}

fn move_change(loaded: &Loaded, to: PathBuf, piece: &Piece) -> Change {
    if to == loaded.entry.path {
        return edit_change(loaded, piece);
    }
    Change {
        path: to,
        from: Some(loaded.entry.path.clone()),
        before: Some(loaded.text.clone()),
        after: Some(written(piece)),
    }
}

/// A move, then the moves of the piece's files to beside `to`.
fn move_with_files(
    loaded: &Loaded,
    to: PathBuf,
    mut piece: Piece,
) -> Result<Vec<Change>, StoreError> {
    let files = attach::relocate(&mut piece, &loaded.entry.path, &to)?;
    let mut out = vec![move_change(loaded, to, &piece)];
    out.extend(files);
    Ok(out)
}

/// `<folder of place>/<file>`, or a config error when the layer has no folder.
fn path_in(store: &Store, place: &Place, file: &str) -> Result<PathBuf, StoreError> {
    let folder = place
        .folder(&store.config)
        .ok_or_else(|| StoreError::Config(format!("no folder holds the layer {}", place.layer)))?;
    Ok(PathBuf::from(format!("{folder}/{file}")))
}

fn memory_only(loaded: &Loaded, what: &str) -> Result<(), StoreError> {
    match loaded.entry.layer {
        Layer::Memory => Ok(()),
        other => Err(StoreError::Invalid(format!(
            "{}: {what} takes a memory, not a {other}",
            loaded.entry.id
        ))),
    }
}

fn sort(
    store: &Store,
    action: &Action,
    files: &Files,
    now: &Now,
) -> Result<Vec<Change>, StoreError> {
    let Action::Sort {
        id,
        layer,
        confidential,
        slice,
        set,
        sorted_by,
    } = action
    else {
        return Ok(Vec::new());
    };
    let loaded = load(store, id)?;
    memory_only(&loaded, "a sort")?;
    let stamp = |piece: &mut Piece, status: &str| {
        piece.set(key::STATUS, Value::from(status));
        piece.set(key::UPDATED, now.date_text());
        piece.set(key::SORTED, now.time_text());
        if let Some(by) = sorted_by {
            piece.set(key::SORTED_BY, Value::from(by.as_str()));
        }
    };
    let place = Place::new(Layer::Memory, *confidential, slice.clone());
    if *layer == Layer::Memory {
        let mut piece = loaded.piece.clone();
        apply(&mut piece, set)?;
        stamp(&mut piece, status::SORTED);
        let to = path_in(store, &place, &file_name(&loaded))?;
        return move_with_files(&loaded, to, piece);
    }
    let (mut created, link) = new_piece(store, *layer, *confidential, slice, set, &loaded, now)?;
    let sorted = loaded.piece.status() == Some(status::SORTED);
    if files.others(id) > 0 || sorted {
        return Ok(made_and_linked(&loaded, created, &link, now));
    }
    let at = created.path().to_path_buf();
    let files = attach::relocate(&mut created, &loaded.entry.path, &at)?;
    let mut retired = loaded.piece.clone();
    retired.remove(key::FILES);
    stamp(&mut retired, status::RETIRED);
    let links = merged(&[Ref::new(&link)], &retired.links(), &[]);
    retired.set(key::LINKS, ref_list(&links));
    let home = Place::new(Layer::Memory, *confidential, None);
    let to = path_in(store, &home, &file_name(&loaded))?;
    let mut out = vec![create_change(&created), move_change(&loaded, to, &retired)];
    out.extend(files);
    Ok(out)
}

/// A piece made from a capture that stays where it is: it keeps its files and links the piece.
fn made_and_linked(capture: &Loaded, mut created: Piece, link: &str, now: &Now) -> Vec<Change> {
    created.remove(key::FILES);
    let mut kept = capture.piece.clone();
    let links = merged(&kept.links(), &[Ref::new(link)], &[]);
    kept.set(key::LINKS, ref_list(&links));
    kept.set(key::UPDATED, now.date_text());
    vec![create_change(&created), edit_change(capture, &kept)]
}

/// A capture's file accepted for another piece: the raw capture links that piece, and is
/// retired where it lies when no other file of it is left.
fn follow_capture(
    store: &Store,
    action: &Action,
    files: &Files,
    changes: &mut Vec<Change>,
    now: &Now,
) -> Result<(), StoreError> {
    let Some(capture) = files.source().map(|s| s.id.clone()) else {
        return Ok(());
    };
    if action.target() == capture || changes.is_empty() {
        return Ok(());
    }
    let Some(loaded) = raw_in_inbox(store, &capture)? else {
        return Ok(());
    };
    let path = &loaded.entry.path;
    if changes
        .iter()
        .any(|c| &c.path == path || c.from.as_ref() == Some(path))
    {
        return Ok(());
    }
    let mut piece = loaded.piece.clone();
    let target = Ref::from(action.target());
    piece.set(
        key::LINKS,
        ref_list(&merged(&piece.links(), &[target], &[&capture])),
    );
    if files.others(&capture) == 0 {
        piece.set(key::STATUS, Value::from(status::RETIRED));
    }
    piece.set(key::UPDATED, now.date_text());
    changes.push(edit_change(&loaded, &piece));
    Ok(())
}

/// The capture `id` when it is raw in the inbox, read; else `None`.
fn raw_in_inbox(store: &Store, id: &PieceId) -> Result<Option<Loaded>, StoreError> {
    let Some(entry) = store.index.get(id) else {
        return Ok(None);
    };
    let inbox = format!("{}/", store.config.inbox);
    let raw = entry.status.as_deref().is_none_or(|s| s == status::RAW);
    if !raw || !entry.path_text().starts_with(&inbox) {
        return Ok(None);
    }
    load(store, id).map(Some)
}

/// The diff retiring a raw capture where it lies once its last file is skipped, when an
/// earlier file of it was accepted (its links are not empty).
pub(crate) fn retire_skipped(store: &Store, id: &PieceId) -> Result<Option<Diff>, StoreError> {
    let Some(loaded) = raw_in_inbox(store, id)? else {
        return Ok(None);
    };
    let pending = inbox::pending(&store.root, &store.config);
    if pending.iter().any(|p| &p.id == id) || loaded.piece.links().is_empty() {
        return Ok(None);
    }
    let now = Now {
        time: Local::now().fixed_offset(),
    };
    let changes = retire(store, id, None, &now)?;
    Ok(Some(Diff {
        action: Action::Retire {
            id: id.clone(),
            behind: None,
        },
        made_at: now.time,
        changes,
        dropped: Vec::new(),
    }))
}

/// The new piece of another layer a capture becomes, and the link text naming it.
fn new_piece(
    store: &Store,
    layer: Layer,
    confidential: bool,
    slice: &Option<String>,
    set: &Fields,
    capture: &Loaded,
    now: &Now,
) -> Result<(Piece, String), StoreError> {
    let config = &store.config;
    let capture_id = capture.piece.text(key::ID).map(str::to_string);
    let capture_id = capture_id.unwrap_or_else(|| capture.entry.id.to_string());
    let place = Place::new(layer, confidential, slice.clone());
    if layer == Layer::Tag {
        return tag_piece(store, &place, set, &capture_id, now);
    }
    let field = |name: &str| set.get(name).and_then(Value::as_str).map(str::to_string);
    let (stem, link, mut piece) = match layer.prefix() {
        Some(prefix) => {
            let home = place.home(config);
            let id = next_at(&store.index, config, prefix, home).padded(config.id_width);
            let mut piece = Piece::empty(PathBuf::new());
            piece.set(key::ID, Value::from(id.as_str()));
            piece.set(key::TITLE, Value::from(capture.piece.title()));
            let first = config
                .task_statuses
                .first()
                .map(String::as_str)
                .unwrap_or("todo");
            piece.set(key::STATUS, Value::from(first));
            if !config.owner.is_empty() {
                piece.set(key::OWNER, Value::from(config.owner.as_str()));
            }
            piece.set(key::CREATED, now.date_text());
            (id.clone(), id, piece)
        }
        None => named_piece(store, layer, &field, now)?,
    };
    piece.set(key::CAPTURE, Value::from(capture_id));
    piece.set(key::UPDATED, now.date_text());
    piece.set(key::LINKS, Value::Sequence(Vec::new()));
    carry(&capture.piece, &mut piece);
    apply(&mut piece, set)?;
    let body = capture.piece.body();
    let body = match layer {
        Layer::Person => person_body(&config.person_sections, body),
        _ => body.to_string(),
    };
    piece.set_body(body);
    let path = path_in(store, &place, &format!("{stem}.md"))?;
    Ok((piece.moved(path), link))
}

/// A tag piece made from a capture: `tag`, its `set` (definition, aliases), the capture named, no body.
fn tag_piece(
    store: &Store,
    place: &Place,
    set: &Fields,
    capture_id: &str,
    now: &Now,
) -> Result<(Piece, String), StoreError> {
    let word = set.get(key::TAG).and_then(Value::as_str).map(str::trim);
    let word = word.filter(|w| !slug(w).is_empty());
    let word = word.ok_or_else(|| StoreError::Invalid("a tag needs set.tag".into()))?;
    let stem = slug(word);
    let mut piece = Piece::empty(PathBuf::new());
    apply(&mut piece, set)?;
    piece.set(key::TAG, Value::from(word));
    piece.set(key::CAPTURE, Value::from(capture_id));
    piece.set(key::UPDATED, now.date_text());
    let folder = store.config.folder_of(Layer::Tag).unwrap_or(layer::TAGS);
    let link = format!("{folder}/{stem}");
    let path = path_in(store, place, &format!("{stem}.md"))?;
    Ok((piece.moved(path), link))
}

/// A person's body: the capture's words under the first heading, the second one empty.
fn person_body(sections: &[String; 2], words: &str) -> String {
    let nl = if words.contains("\r\n") { "\r\n" } else { "\n" };
    let words = words.trim_end_matches(['\r', '\n']);
    let [know, handle] = sections;
    format!("## {know}{nl}{nl}{words}{nl}{nl}## {handle}{nl}")
}

/// The capture's flags and files, when not empty, onto the piece it becomes.
fn carry(capture: &Piece, piece: &mut Piece) {
    for name in [key::TAGS, key::PEOPLE, key::MARKS, key::FILES] {
        let value = capture
            .get(name)
            .filter(|v| v.as_sequence().is_some_and(|s| !s.is_empty()));
        if let Some(value) = value {
            piece.set(name, value.clone());
        }
    }
}

/// A diary or conversation day, or a person: its stem, its link text and its first fields.
fn named_piece(
    store: &Store,
    layer: Layer,
    field: &dyn Fn(&str) -> Option<String>,
    now: &Now,
) -> Result<(String, String, Piece), StoreError> {
    let (name_key, stem) = if layer::is_day(layer) {
        let date = field(key::DATE).filter(|d| parse_date(d).is_some());
        let date = date.ok_or_else(|| StoreError::Invalid(format!("a {layer} needs a date")))?;
        (key::DATE, date)
    } else {
        let name = field(key::NAME).filter(|n| !slug(n).is_empty());
        let name = name.ok_or_else(|| StoreError::Invalid(format!("a {layer} needs a name")))?;
        (key::NAME, slug(&name))
    };
    let mut piece = Piece::empty(PathBuf::new());
    piece.set(name_key, Value::from(field(name_key).unwrap_or_default()));
    if let Some(kind) = layer::default_kind(layer) {
        piece.set(key::KIND, Value::from(kind));
    }
    piece.set(key::CAPTURED, now.time_text());
    piece.set(key::STATUS, Value::from(status::RAW));
    let folder = store.config.folder_of(layer).unwrap_or_default();
    Ok((stem.clone(), format!("{folder}/{stem}"), piece))
}

/// A file stem from a name: lowercase ASCII, accents folded, dashes between words.
pub(crate) fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        let folded = match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => "a",
            'ç' => "c",
            'è' | 'é' | 'ê' | 'ë' => "e",
            'ì' | 'í' | 'î' | 'ï' => "i",
            'ñ' => "n",
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => "o",
            'ù' | 'ú' | 'û' | 'ü' => "u",
            'ÿ' | 'ý' => "y",
            'œ' => "oe",
            'æ' => "ae",
            c if c.is_ascii_alphanumeric() => {
                out.push(c);
                continue;
            }
            _ => "-",
        };
        if folded != "-" || !out.ends_with('-') {
            out.push_str(folded);
        }
    }
    out.trim_matches('-').to_string()
}

/// `first` then the refs of `then` not yet there, none of `except`.
fn merged(first: &[Ref], then: &[Ref], except: &[&PieceId]) -> Vec<Ref> {
    let mut out: Vec<Ref> = Vec::new();
    for link in first.iter().chain(then) {
        let same = |r: &Ref| r == link || (r.id().is_some() && r.id() == link.id());
        let excluded = link.id().is_some_and(|id| except.contains(&&id));
        if !excluded && !out.iter().any(same) {
            out.push(link.clone());
        }
    }
    out
}

fn ref_list(links: &[Ref]) -> Value {
    Value::Sequence(links.iter().map(|l| Value::from(l.as_str())).collect())
}

fn text_list(items: &[String]) -> Value {
    Value::Sequence(items.iter().map(|t| Value::from(t.as_str())).collect())
}

fn edit(
    store: &Store,
    id: &PieceId,
    set: &Fields,
    unset: &[String],
    body: &Option<String>,
    now: &Now,
) -> Result<Vec<Change>, StoreError> {
    let loaded = load(store, id)?;
    let mut piece = loaded.piece.clone();
    apply(&mut piece, set)?;
    for name in unset {
        piece.remove(name);
    }
    if let Some(body) = body {
        piece.set_body(body.clone());
    }
    piece.set(key::UPDATED, now.date_text());
    Ok(vec![edit_change(&loaded, &piece)])
}

fn link(store: &Store, from: &PieceId, to: &[Ref], now: &Now) -> Result<Vec<Change>, StoreError> {
    let loaded = load(store, from)?;
    let before = loaded.piece.links();
    let links = merged(&before, to, &[]);
    if links.len() == before.len() {
        return Ok(Vec::new());
    }
    let mut piece = loaded.piece.clone();
    piece.set(key::LINKS, ref_list(&links));
    piece.set(key::UPDATED, now.date_text());
    Ok(vec![edit_change(&loaded, &piece)])
}

/// Two memories, two tasks or two people, the kept one older (people: any two).
fn dedupable(kept: &Loaded, dropped: &Loaded) -> Result<(), StoreError> {
    let (k, d) = (&kept.entry, &dropped.entry);
    if k.layer != d.layer || layer::is_day(k.layer) {
        return Err(StoreError::Invalid(format!(
            "a dedupe takes two memories, two tasks or two people: {} is a {}, {} a {}",
            k.id, k.layer, d.id, d.layer
        )));
    }
    let older = match (&k.id, &d.id) {
        (PieceId::Numbered { prefix: a, n: x }, PieceId::Numbered { prefix: b, n: y }) => {
            a == b && x < y
        }
        (PieceId::Named { .. }, PieceId::Named { .. }) => k.id != d.id,
        _ => false,
    };
    if !older {
        return Err(StoreError::Invalid(format!(
            "a dedupe keeps the older id: {} is not older than {}",
            k.id, d.id
        )));
    }
    Ok(())
}

fn dedupe(
    store: &Store,
    keep: &PieceId,
    drop: &PieceId,
    now: &Now,
) -> Result<Vec<Change>, StoreError> {
    let kept = load(store, keep)?;
    let dropped = load(store, drop)?;
    dedupable(&kept, &dropped)?;
    let archive = &store.config.archive;
    let either =
        |l: &Ref| links::names(l, &kept.entry, archive) || links::names(l, &dropped.entry, archive);
    let mut piece = kept.piece.clone();
    let mut tags = kept.piece.tags();
    for tag in dropped.piece.tags() {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    if !tags.is_empty() || piece.get(key::TAGS).is_some() {
        piece.set(key::TAGS, text_list(&tags));
    }
    let links = merged(&kept.piece.links(), &dropped.piece.links(), &[]);
    let links: Vec<Ref> = links.into_iter().filter(|l| !either(l)).collect();
    piece.set(key::LINKS, ref_list(&links));
    piece.set(key::UPDATED, now.date_text());
    let keep_ref = Ref::from(kept.entry.id.clone());
    let own: Vec<Ref> = dropped
        .piece
        .links()
        .into_iter()
        .filter(|l| !either(l))
        .collect();
    let mut retired = dropped.piece.clone();
    retired.set(key::STATUS, Value::from(status::RETIRED));
    retired.set(
        key::LINKS,
        ref_list(&merged(std::slice::from_ref(&keep_ref), &own, &[])),
    );
    retired.set(key::UPDATED, now.date_text());
    let mut out = vec![edit_change(&kept, &piece)];
    match dropped.entry.layer {
        Layer::Task => out.extend(to_archive(store, &dropped, retired)?),
        _ => out.push(edit_change(&dropped, &retired)),
    }
    out.extend(relinked(store, &kept, &dropped, &keep_ref, now)?);
    Ok(out)
}

/// Every other visible piece linking to `dropped`, its link turned to `keep_ref`.
fn relinked(
    store: &Store,
    kept: &Loaded,
    dropped: &Loaded,
    keep_ref: &Ref,
    now: &Now,
) -> Result<Vec<Change>, StoreError> {
    let archive = &store.config.archive;
    let to_drop = |l: &Ref| links::names(l, &dropped.entry, archive);
    let mut out = Vec::new();
    for entry in &store.index.entries {
        let other = entry.id != kept.entry.id && entry.id != dropped.entry.id;
        if !other || !entry.links.iter().any(to_drop) {
            continue;
        }
        let loaded = load(store, &entry.id)?;
        let turned: Vec<Ref> = loaded
            .piece
            .links()
            .into_iter()
            .map(|l| if to_drop(&l) { keep_ref.clone() } else { l })
            .collect();
        let mut piece = loaded.piece.clone();
        piece.set(key::LINKS, ref_list(&merged(&turned, &[], &[])));
        piece.set(key::UPDATED, now.date_text());
        out.push(edit_change(&loaded, &piece));
    }
    Ok(out)
}

/// The moves of a task and its files into the archive folder of its place.
fn to_archive(store: &Store, loaded: &Loaded, piece: Piece) -> Result<Vec<Change>, StoreError> {
    let entry = &loaded.entry;
    let place = Place {
        archived: true,
        ..Place::beside(entry, Layer::Task, entry.confidential, &store.config)
    };
    let to = path_in(store, &place, &file_name(loaded))?;
    move_with_files(loaded, to, piece)
}

fn archive(store: &Store, id: &PieceId, now: &Now) -> Result<Vec<Change>, StoreError> {
    let loaded = load(store, id)?;
    let entry = &loaded.entry;
    if entry.layer != Layer::Task {
        return Err(StoreError::Invalid(format!(
            "{id}: only a task is archived, not a {}",
            entry.layer
        )));
    }
    if entry.archived {
        return Err(StoreError::Invalid(format!("{id}: already archived")));
    }
    let mut piece = loaded.piece.clone();
    if loaded.piece.status() != Some(status::DONE) {
        piece.set(key::STATUS, Value::from(status::DONE));
        piece.set_body(notes::closed_body(&loaded.piece, now.time.date_naive()));
    }
    piece.set(key::UPDATED, now.date_text());
    to_archive(store, &loaded, piece)
}

fn retire(
    store: &Store,
    id: &PieceId,
    behind: Option<&PieceId>,
    now: &Now,
) -> Result<Vec<Change>, StoreError> {
    let loaded = load(store, id)?;
    memory_only(&loaded, "a retire")?;
    let mut piece = loaded.piece.clone();
    if let Some(resume) = behind {
        if resume == id || store.index.get(resume).is_none() {
            return Err(StoreError::Invalid(format!(
                "{id}: retired behind {resume}, which is not another piece"
            )));
        }
        piece.set(key::RETIRED_BY, Value::from(resume.to_string()));
    }
    piece.set(key::STATUS, Value::from(status::RETIRED));
    piece.set(key::UPDATED, now.date_text());
    Ok(vec![edit_change(&loaded, &piece)])
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_becomes_a_lowercase_ascii_slug() {
        assert_eq!(slug("Robin"), "robin");
        assert_eq!(slug("  Jean-Ève  Dœuf "), "jean-eve-doeuf");
        assert_eq!(slug("«»"), "");
    }

    #[test]
    fn a_hand_written_proposal_reads_as_an_action() {
        let text = "action: sort\nid: M-0007\nlayer: task\nset:\n  title: Bank\n";
        let action: Action = serde_yaml_ng::from_str(text).unwrap();
        let Action::Sort {
            id,
            layer,
            set,
            confidential,
            ..
        } = &action
        else {
            panic!("{action:?}");
        };
        assert_eq!(id.to_string(), "M-0007");
        assert_eq!((*layer, *confidential), (Layer::Task, false));
        assert_eq!(set.get("title"), Some(&Value::from("Bank")));
        let back: Action =
            serde_yaml_ng::from_str(&serde_yaml_ng::to_string(&action).unwrap()).unwrap();
        assert_eq!(back, action);
        let typo = "action: retire\nid: M-1\nwhy: x\n";
        assert!(serde_yaml_ng::from_str::<Action>(typo).is_err());
    }

    #[test]
    fn merged_links_keep_their_order_and_drop_repeats() {
        let refs = |t: &[&str]| t.iter().map(|s| Ref::new(s)).collect::<Vec<_>>();
        let keep = PieceId::parse("M-1").unwrap();
        let out = merged(
            &refs(&["M-0002", "a/b"]),
            &refs(&["M-02", "M-0001", "c/d"]),
            &[&keep],
        );
        assert_eq!(out, refs(&["M-0002", "a/b", "c/d"]));
    }
}
