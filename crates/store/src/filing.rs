//! Filing — the plugin's writing calls: a memory filed, a note, a link, a change, a retire, a
//! journal line, each one commit by Lorisse in the repository holding the piece, and the undo of
//! one of her changes.
//!
//! Each call checks first (git, the folder's identity, the folder's rules), then writes, then
//! commits; a commit that fails puts every file back as it was. The undo of a filed memory keeps
//! its number in `ids.json`, in the same commit, so the number is never given again.
//! Flow: `Store::open_write` → `remember` / `note` / `link` / `attach` / `change` / `retire` /
//! `journal` → files → one commit; `undo`.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate};
use serde_yaml_ng::Value;

use crate::attach::{self, Attachment};
use crate::check;
use crate::config::status;
use crate::diary;
use crate::direct::{self, Since};
use crate::drawers;
use crate::error::StoreError;
use crate::frontmatter::key;
use crate::git::{self, LORISSE_EMAIL, Who};
use crate::ids::{self, PieceId};
use crate::index::{self, Entry, IDS_FILE, io_error, write_atomic};
use crate::inbox::{DATE_FORMAT, TIME_FORMAT};
use crate::layer::{self, CONFIDENTIAL, Layer};
use crate::links::{self, Ref};
use crate::piece::Piece;
use crate::repos::{self, Home, ROOT_FOLDER};
use crate::route::Kind;
use crate::store::Store;

// =============================================================================
// Interface
// =============================================================================

/// What stops a write before anything is touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    /// No `git` program can be started on this machine.
    NoGit,
    /// The memory folder holds no git repository of its own.
    NotUnderGit,
    /// The repository named has no name or no address set for its commits.
    NoIdentity(String),
}

/// A memory to file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewMemory {
    /// Its title, one line.
    pub title: String,
    /// Its text.
    pub text: String,
    /// Its kind; the folder's first kind when `None`.
    pub kind: Option<String>,
    /// Its tags.
    pub tags: Vec<String>,
    /// The pieces it links to: numbers, or a person's or a tag's name.
    pub links: Vec<String>,
    /// The repository it goes to, by its row's name; the folder's own when `None`.
    pub repo: Option<String>,
}

/// What a writing call did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// The piece written, or the piece a link already there was asked on.
    pub id: PieceId,
    /// The repository holding it, by its row's name.
    pub repo: String,
    /// Whether a file was written; `false` for a link already there.
    pub changed: bool,
    /// Lorisse's commit, its full id; `None` when nothing changed or no git holds the piece.
    pub commit: Option<String>,
}

/// One dated line to import as a memory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dated {
    /// Its title, one line.
    pub title: String,
    /// Its text, the person's words as given.
    pub text: String,
    /// Its category, kept as its tag.
    pub tag: String,
    /// Its date, `YYYY-MM-DD`; `None` when it is not known.
    pub date: Option<String>,
}

/// What an import did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    /// The memories filed, in the order given.
    pub ids: Vec<PieceId>,
    /// The repository holding them, by its row's name.
    pub repo: String,
    /// Lorisse's one commit of the whole import; `None` when no git holds the folder.
    pub commit: Option<String>,
}

/// What an attach did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    /// The piece the file goes with.
    pub id: PieceId,
    /// The repository holding it, by its row's name.
    pub repo: String,
    /// The name the copy was given: the file's own, or a numbered one.
    pub name: String,
    /// The copy, relative to the memory folder, forward slashes.
    pub path: String,
    /// Lorisse's commit, its full id; `None` when no git holds the piece.
    pub commit: Option<String>,
}

/// What a change replaces in a memory or a person; a field left `None` stays as it is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Revision {
    /// The new text, whole.
    pub text: Option<String>,
    /// The new title; a person's name.
    pub title: Option<String>,
    /// The new kind, one of the folder's.
    pub kind: Option<String>,
    /// The new tags, all of them; an empty list leaves none.
    pub tags: Option<Vec<String>>,
}

impl Revision {
    /// Whether the revision names nothing to change.
    pub fn is_empty(&self) -> bool {
        self.text.is_none() && self.title.is_none() && self.kind.is_none() && self.tags.is_none()
    }
}

/// What a change did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revised {
    /// The piece changed.
    pub id: PieceId,
    /// The repository holding it, by its row's name.
    pub repo: String,
    /// The fields that changed (`text`, `title`, `name`, `kind`, `tags`); empty when the piece
    /// already said what was given, nothing then written.
    pub fields: Vec<String>,
    /// Lorisse's commit, its full id; `None` when nothing changed or no git holds the piece.
    pub commit: Option<String>,
}

/// What a journal line did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Journaled {
    /// The day's page, `journal/<YYYY-MM-DD>`.
    pub id: PieceId,
    /// The day, `YYYY-MM-DD`.
    pub day: String,
    /// The repository holding the page, by its row's name.
    pub repo: String,
    /// The page, relative to the memory folder, forward slashes.
    pub path: String,
    /// Whether the page was made by this line.
    pub made: bool,
    /// The time the line carries, `HH:MM`.
    pub time: String,
    /// Lorisse's commit, its full id; `None` when no git holds the page.
    pub commit: Option<String>,
}

/// What an undo took back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undone {
    /// The commit taken back, its full id.
    pub commit: String,
    /// Its subject.
    pub subject: String,
    /// The pieces it named.
    pub ids: Vec<String>,
    /// The new commit that takes it back, its full id.
    pub revert: String,
}

impl fmt::Display for Blocked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Blocked::NoGit => f.write_str("git cannot be started on this machine"),
            Blocked::NotUnderGit => f.write_str("the memory folder is not under git"),
            Blocked::NoIdentity(repo) => write!(f, "git has no name or address for the commits of {repo}"),
        }
    }
}

// =============================================================================
// Implementation
// =============================================================================

/// The body line of every commit Lorisse makes through the plugin.
pub(crate) const BODY: &str = "job: plugin";

/// One line: newlines folded to spaces, trimmed.
pub(crate) fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A text for a commit's subject: one line, cut past 60 characters.
fn short(text: &str) -> String {
    let line = one_line(text);
    match line.char_indices().nth(60) {
        Some((at, _)) => format!("{}...", &line[..at]),
        None => line,
    }
}

/// What stops a write in the folder: under git, no git on the machine or no identity. A folder
/// not under git is written all the same, without a history.
pub(crate) fn blocked(store: &Store) -> Option<Blocked> {
    let home = repos::root_home(store)?;
    if !git::available() {
        return Some(Blocked::NoGit);
    }
    unsigned(&home)
}

/// `NoIdentity` when the repository's own commits have no name or address.
fn unsigned(home: &Home) -> Option<Blocked> {
    let named = matches!(home.owner(), Who::Named { .. });
    (!named && !git::has_identity(&home.dir)).then(|| Blocked::NoIdentity(home.name.clone()))
}

/// The repository a write at `rel` commits in, once nothing blocks it; `None` when no git
/// holds it, the write then made without a commit.
pub(crate) fn ready(store: &Store, rel: &str) -> Result<Option<Home>, StoreError> {
    if let Some(said) = blocked(store) {
        return Err(StoreError::Invalid(said.to_string()));
    }
    let Some(home) = repos::home_for_write(store, rel) else {
        return Ok(None);
    };
    if !git::available() {
        return Err(StoreError::Invalid(Blocked::NoGit.to_string()));
    }
    match unsigned(&home) {
        Some(said) => Err(StoreError::Invalid(said.to_string())),
        None => Ok(Some(home)),
    }
}

/// The root row's name.
pub(crate) fn root_name(store: &Store) -> String {
    store.repos().first().map(|r| r.name.clone()).unwrap_or_default()
}

/// The repository a new memory goes to: its folder (`None` the root) and its row's name.
fn repo_named(store: &Store, name: Option<&str>) -> Result<(Option<String>, String), StoreError> {
    let root = root_name(store);
    let Some(name) = name.map(str::trim).filter(|n| !n.is_empty()) else {
        return Ok((None, root));
    };
    if name.eq_ignore_ascii_case(CONFIDENTIAL) {
        return Err(StoreError::Confidential(name.to_string()));
    }
    let row = store.config.repos.iter().find(|r| r.name.trim() == name);
    let Some(row) = row else {
        if name == root {
            return Ok((None, root));
        }
        let seen = store.repos().into_iter().filter(|r| (!r.confidential || r.open) && r.present);
        let names: Vec<String> = seen.map(|r| r.name).collect();
        return Err(StoreError::Invalid(format!(
            "no repository is named {name}: the repositories are {}",
            names.join(", ")
        )));
    };
    if row.confidential {
        return Err(StoreError::Confidential(name.to_string()));
    }
    if row.folder == ROOT_FOLDER {
        return Ok((None, row.name.clone()));
    }
    if !store.root.join(&row.folder).is_dir() {
        return Err(StoreError::Invalid(format!(
            "the repository {name} is not on this machine"
        )));
    }
    Ok((Some(row.folder.clone()), row.name.clone()))
}

/// The visible entry a number or a name leads to.
pub(crate) fn visible<'a>(store: &'a Store, text: &str) -> Result<&'a Entry, StoreError> {
    let link = Ref::new(text);
    if links::is_hidden(&store.index, &link) {
        return Err(StoreError::Unknown(text.trim().to_string()));
    }
    links::resolve(&store.index, &link, &store.config.archive)
        .ok_or_else(|| StoreError::Unknown(text.trim().to_string()))
}

/// The entry of `id`, visible, of one of `layers`.
fn entry_of<'a>(store: &'a Store, id: &PieceId, layers: &[Layer], what: &str) -> Result<&'a Entry, StoreError> {
    let entry = visible(store, &id.to_string())?;
    if !layers.contains(&entry.layer) {
        return Err(StoreError::Invalid(format!(
            "{} is a {}: {what}",
            entry.id, entry.layer
        )));
    }
    Ok(entry)
}

/// The repository name a piece is said with.
fn repo_of(store: &Store, entry: &Entry) -> String {
    entry.repo.clone().unwrap_or_else(|| root_name(store))
}

/// Refuse a piece whose file has changes the person has not committed; none without git.
pub(crate) fn refuse_dirty(home: Option<&Home>, rel: &str) -> Result<(), StoreError> {
    let Some(home) = home else { return Ok(()) };
    match git::dirty(&home.dir, &[home.inner(rel)])?.is_empty() {
        true => Ok(()),
        false => Err(StoreError::Invalid(format!(
            "{rel} has changes not committed: commit them or drop them first"
        ))),
    }
}

pub(crate) fn remember(store: &mut Store, memory: &NewMemory) -> Result<Written, StoreError> {
    let title = one_line(&memory.title);
    let text = memory.text.trim();
    if title.is_empty() || text.is_empty() {
        return Err(StoreError::Invalid("a memory needs a title and a text".into()));
    }
    let (folder, repo) = repo_named(store, memory.repo.as_deref())?;
    let config = &store.config;
    let kind = match memory.kind.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        Some(kind) if config.kinds.iter().any(|k| k == kind) => kind.to_string(),
        Some(kind) => {
            return Err(StoreError::Invalid(format!(
                "the kind {kind} is not one this memory folder knows ({})",
                config.kinds.join(", ")
            )));
        }
        None => config.kinds.first().cloned().unwrap_or_else(|| "fact".into()),
    };
    let mut linked: Vec<Value> = Vec::new();
    for link in memory.links.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        let to = visible(store, link)?.id.to_string();
        if !linked.contains(&Value::from(to.as_str())) {
            linked.push(Value::from(to));
        }
    }
    let row = folder.as_ref().map(|_| repo.as_str());
    let id = store.next_id("M", row);
    let tags: Vec<String> = memory.tags.iter().map(|t| one_line(t)).filter(|t| !t.is_empty()).collect();
    let fields = Fields { title: &title, text, kind: &kind, tags: &tags, links: linked, date: None, source: "chat" };
    let (rel, piece) = new_piece(store, folder.as_deref(), &id, fields)?;
    let home = ready(store, &rel)?;
    let apart = home.as_ref().and_then(|h| counter_apart(store, h, &id));
    if let Some((file, other)) = &apart {
        refuse_dirty(Some(other), file)?;
    }
    let id_text = id.padded(store.config.id_width);
    let subject = format!("{}({id_text}): {}", Kind::Remember, short(&title));
    let commit = write_and_commit(store, home.as_ref(), &[(rel, Some(piece.render()))], &subject)?;
    if let Some(apart) = &apart {
        keep_counter(store, apart, &id);
    }
    let id = PieceId::parse(&id_text).ok_or_else(|| StoreError::Invalid(format!("{id_text}: not an id")))?;
    Ok(Written {
        id,
        repo,
        changed: true,
        commit,
    })
}

/// What a new memory's header and text are made of.
struct Fields<'a> {
    title: &'a str,
    text: &'a str,
    kind: &'a str,
    tags: &'a [String],
    links: Vec<Value>,
    date: Option<&'a str>,
    /// The source written when the folder's settings list it.
    source: &'a str,
}

/// A new memory's path, relative to the root, and its piece, checked against the folder's lists.
fn new_piece(store: &Store, folder: Option<&str>, id: &PieceId, fields: Fields) -> Result<(String, Piece), StoreError> {
    let id = id.padded(store.config.id_width);
    let memories = store.config.folder_of(Layer::Memory).unwrap_or("memories");
    let rel = match folder {
        Some(f) => format!("{f}/{memories}/{id}.md"),
        None => format!("{memories}/{id}.md"),
    };
    if store.root.join(&rel).exists() {
        return Err(StoreError::Exists(PathBuf::from(&rel)));
    }
    let now = Local::now().fixed_offset();
    let mut piece = Piece::empty(PathBuf::from(&rel));
    piece.set(key::ID, Value::from(id.as_str()));
    piece.set(key::TITLE, Value::from(fields.title));
    if let Some(date) = fields.date {
        piece.set(key::DATE, Value::from(date));
    }
    piece.set(key::KIND, Value::from(fields.kind));
    piece.set(key::TAGS, Value::Sequence(fields.tags.iter().map(|t| Value::from(t.as_str())).collect()));
    let sources = &store.config.sources;
    let source = [fields.source, "chat"].into_iter().find(|s| sources.iter().any(|w| w == s));
    if let Some(source) = source {
        piece.set(key::SOURCE, Value::from(source));
    }
    piece.set(key::CAPTURED, Value::from(now.format(TIME_FORMAT).to_string()));
    piece.set(key::UPDATED, Value::from(now.format(DATE_FORMAT).to_string()));
    piece.set(key::STATUS, Value::from(status::SORTED));
    piece.set(key::LINKS, Value::Sequence(fields.links));
    piece.set_body(format!("{}\n", fields.text));
    let outside = check::outside_lists(Layer::Memory, &piece, &store.config);
    if !outside.is_empty() {
        return Err(StoreError::Invalid(outside.join("; ")));
    }
    Ok((rel, piece))
}

pub(crate) fn import(store: &mut Store, lines: &[Dated]) -> Result<Imported, StoreError> {
    if lines.is_empty() {
        return Err(StoreError::Invalid("there is no line to import".into()));
    }
    let repo = root_name(store);
    let kind = store.config.kinds.first().cloned().unwrap_or_else(|| "fact".into());
    let first = store.next_id("M", None);
    let PieceId::Numbered { prefix, n } = first else {
        return Err(StoreError::Invalid("a memory's number cannot be made".into()));
    };
    let mut changes = Vec::new();
    let mut ids = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let title = one_line(&line.title);
        let text = line.text.trim();
        if title.is_empty() || text.is_empty() {
            return Err(StoreError::Invalid("a memory needs a title and a text".into()));
        }
        let date = line.date.as_deref().map(str::trim);
        if let Some(day) = date {
            chrono::NaiveDate::parse_from_str(day, DATE_FORMAT)
                .map_err(|_| StoreError::Invalid(format!("{day} is not a date")))?;
        }
        let tags: Vec<String> = Some(one_line(&line.tag)).filter(|t| !t.is_empty()).into_iter().collect();
        let id = PieceId::Numbered { prefix: prefix.clone(), n: n + i as u64 };
        let fields = Fields { title: &title, text, kind: &kind, tags: &tags, links: Vec::new(), date, source: "import" };
        let (rel, piece) = new_piece(store, None, &id, fields)?;
        changes.push((rel, Some(piece.render())));
        ids.push(id);
    }
    let home = ready(store, &changes[0].0)?;
    let named: Vec<String> = ids.iter().map(|id| id.padded(store.config.id_width)).collect();
    let subject = format!("{}({}): imported, {} memories", Kind::Remember, named.join(", "), ids.len());
    let commit = write_and_commit(store, home.as_ref(), &changes, &subject)?;
    let ids = named.iter().filter_map(|i| PieceId::parse(i)).collect();
    Ok(Imported { ids, repo, commit })
}

pub(crate) fn note(store: &mut Store, id: &PieceId, text: &str) -> Result<Written, StoreError> {
    let line = one_line(text);
    if line.is_empty() {
        return Err(StoreError::Invalid("a note needs a text".into()));
    }
    let what = "a note goes on a memory or a person";
    let entry = entry_of(store, id, &[Layer::Memory, Layer::Person], what)?.clone();
    let rel = entry.path_text();
    let path = store.root.join(&rel);
    let before = fs::read_to_string(&path).map_err(|e| io_error(&path, e))?;
    let nl = if before.contains("\r\n") { "\r\n" } else { "\n" };
    let mut after = before.clone();
    if !after.is_empty() && !after.ends_with('\n') {
        after.push_str(nl);
    }
    let today = Local::now().format(DATE_FORMAT);
    after.push_str(&format!("- {today}: {line}{nl}"));
    let home = ready(store, &rel)?;
    refuse_dirty(home.as_ref(), &rel)?;
    let subject = format!("{}({}): {}", Kind::Note, entry.id, short(&line));
    let commit = write_and_commit(store, home.as_ref(), &[(rel, Some(after))], &subject)?;
    Ok(Written {
        repo: repo_of(store, &entry),
        id: entry.id,
        changed: true,
        commit,
    })
}

pub(crate) fn link(store: &mut Store, from: &PieceId, to: &PieceId) -> Result<Written, StoreError> {
    let layers = [Layer::Memory, Layer::Person, Layer::Tag];
    let what = "a link goes from a memory, a person or a tag";
    let entry = entry_of(store, from, &layers, what)?.clone();
    let target = visible(store, &to.to_string())?.clone();
    if target.id == entry.id {
        return Err(StoreError::Invalid(format!("{} cannot link to itself", entry.id)));
    }
    let archive = &store.config.archive;
    let unchanged = Written {
        id: entry.id.clone(),
        repo: repo_of(store, &entry),
        changed: false,
        commit: None,
    };
    if entry.links.iter().any(|l| links::names(l, &target, archive)) {
        return Ok(unchanged);
    }
    let rel = entry.path_text();
    let path = store.root.join(&rel);
    let before = fs::read_to_string(&path).map_err(|e| io_error(&path, e))?;
    let mut piece = Piece::parse(&entry.path, &before)?;
    let mut list: Vec<Value> = piece.links().iter().map(|l| Value::from(l.as_str())).collect();
    list.push(Value::from(target.id.to_string()));
    piece.set(key::LINKS, Value::Sequence(list));
    let home = ready(store, &rel)?;
    refuse_dirty(home.as_ref(), &rel)?;
    let subject = format!("{}({}): to {}", Kind::Link, entry.id, target.id);
    let commit = write_and_commit(store, home.as_ref(), &[(rel, Some(piece.render()))], &subject)?;
    Ok(Written {
        changed: true,
        commit,
        ..unchanged
    })
}

pub(crate) fn attach(store: &mut Store, id: &PieceId, source: &Path) -> Result<Attached, StoreError> {
    let given = attach::check_given(&store.root, source, store.config.attach_limit_mb)?;
    let what = "a file goes with a memory or a person";
    let entry = entry_of(store, id, &[Layer::Memory, Layer::Person], what)?.clone();
    let rel = entry.path_text();
    let path = store.root.join(&rel);
    let before = fs::read_to_string(&path).map_err(|e| io_error(&path, e))?;
    let mut piece = Piece::parse(&entry.path, &before)?;
    let mut files = piece.files();
    let md = Path::new(&rel);
    let name = attach::free_name(&store.root, &attach::sidecar(md), &given, &files);
    let file_rel = attach::beside(md, &name);
    files.push(Attachment {
        name: name.clone(),
        path: file_rel.clone(),
        kind: attach::kind_of(&name).to_string(),
    });
    piece.set(key::FILES, attach::to_value(&files));
    let home = ready(store, &rel)?;
    refuse_dirty(home.as_ref(), &rel)?;
    let subject = format!("{}({}): {}", Kind::Attach, entry.id, short(&name));
    let commit = copy_and_commit(store, home.as_ref(), source, &file_rel, &rel, &piece.render(), &subject)?;
    Ok(Attached {
        repo: repo_of(store, &entry),
        id: entry.id,
        name,
        path: file_rel,
        commit,
    })
}

/// A text as a piece's body: its lines as given, trimmed at both ends, one newline at the end.
fn body_of(text: &str, nl: &str) -> String {
    let lines: Vec<&str> = text.trim().lines().map(str::trim_end).collect();
    format!("{}{nl}", lines.join(nl))
}

pub(crate) fn change(store: &mut Store, id: &PieceId, revision: &Revision) -> Result<Revised, StoreError> {
    if revision.is_empty() {
        return Err(StoreError::Invalid("a change names a text, a title, a kind or tags".into()));
    }
    let what = "a change goes on a memory or a person";
    let entry = entry_of(store, id, &[Layer::Memory, Layer::Person], what)?.clone();
    let rel = entry.path_text();
    let path = store.root.join(&rel);
    let before = fs::read_to_string(&path).map_err(|e| io_error(&path, e))?;
    let nl = if before.contains("\r\n") { "\r\n" } else { "\n" };
    let mut piece = Piece::parse(&entry.path, &before)?;
    let mut fields: Vec<String> = Vec::new();
    if let Some(text) = &revision.text {
        if text.trim().is_empty() {
            return Err(StoreError::Invalid("a text is never made empty: retire the memory instead".into()));
        }
        let body = body_of(text, nl);
        if piece.body() != body {
            piece.set_body(body);
            fields.push("text".into());
        }
    }
    if let Some(title) = &revision.title {
        let title = one_line(title);
        if title.is_empty() {
            return Err(StoreError::Invalid("a title is never made empty".into()));
        }
        let (name, word) = match entry.layer {
            Layer::Person => (key::NAME, "name"),
            _ => (key::TITLE, "title"),
        };
        if piece.text(name) != Some(title.as_str()) {
            piece.set(name, Value::from(title));
            fields.push(word.into());
        }
    }
    if let Some(kind) = &revision.kind {
        let kind = kind.trim();
        let kinds = &store.config.kinds;
        if !kinds.iter().any(|k| k == kind) {
            return Err(StoreError::Invalid(format!(
                "the kind {kind} is not one this memory folder knows ({})",
                kinds.join(", ")
            )));
        }
        if piece.kind() != Some(kind) {
            piece.set(key::KIND, Value::from(kind));
            fields.push("kind".into());
        }
    }
    if let Some(tags) = &revision.tags {
        let tags: Vec<String> = tags.iter().map(|t| one_line(t)).filter(|t| !t.is_empty()).collect();
        if piece.tags() != tags || piece.get(key::TAGS).is_none() {
            piece.set(key::TAGS, Value::Sequence(tags.iter().map(|t| Value::from(t.as_str())).collect()));
            fields.push("tags".into());
        }
    }
    let unchanged = Revised { id: entry.id.clone(), repo: repo_of(store, &entry), fields: Vec::new(), commit: None };
    if fields.is_empty() {
        return Ok(unchanged);
    }
    piece.set(key::UPDATED, Value::from(Local::now().format(DATE_FORMAT).to_string()));
    let outside = check::outside_lists(entry.layer, &piece, &store.config);
    if !outside.is_empty() {
        return Err(StoreError::Invalid(outside.join("; ")));
    }
    let home = ready(store, &rel)?;
    refuse_dirty(home.as_ref(), &rel)?;
    let subject = format!("{}({}): {} changed", Kind::Edit, entry.id, fields.join(", "));
    let commit = write_and_commit(store, home.as_ref(), &[(rel, Some(piece.render()))], &subject)?;
    Ok(Revised { fields, commit, ..unchanged })
}

pub(crate) fn retire(store: &mut Store, id: &PieceId, reason: &str, by: Option<&PieceId>) -> Result<Written, StoreError> {
    let reason = one_line(reason);
    if reason.is_empty() {
        return Err(StoreError::Invalid("a retire needs its reason".into()));
    }
    let entry = entry_of(store, id, &[Layer::Memory], "only a memory is retired")?.clone();
    let unchanged = Written { id: entry.id.clone(), repo: repo_of(store, &entry), changed: false, commit: None };
    if entry.status.as_deref() == Some(status::RETIRED) {
        return Ok(unchanged);
    }
    let taker = match by {
        Some(by) => {
            let taker = visible(store, &by.to_string())?.clone();
            if taker.id == entry.id {
                return Err(StoreError::Invalid(format!("{} cannot take its own place", entry.id)));
            }
            if taker.layer != Layer::Memory || taker.status.as_deref() == Some(status::RETIRED) {
                return Err(StoreError::Invalid(format!(
                    "{} cannot take the place of {}: a memory that holds takes the place of one that no longer does",
                    taker.id, entry.id
                )));
            }
            Some(taker)
        }
        None => None,
    };
    let rel = entry.path_text();
    let path = store.root.join(&rel);
    let before = fs::read_to_string(&path).map_err(|e| io_error(&path, e))?;
    let mut piece = Piece::parse(&entry.path, &before)?;
    piece.set(key::STATUS, Value::from(status::RETIRED));
    piece.set(key::UPDATED, Value::from(Local::now().format(DATE_FORMAT).to_string()));
    piece.set(key::RETIRED_WHY, Value::from(reason.as_str()));
    let mut subject = format!("{}({}): {}", Kind::Retire, entry.id, short(&reason));
    if let Some(taker) = &taker {
        let archive = &store.config.archive;
        let mut list: Vec<Value> = piece.links().iter().map(|l| Value::from(l.as_str())).collect();
        if !entry.links.iter().any(|l| links::names(l, taker, archive)) {
            list.push(Value::from(taker.id.to_string()));
        }
        piece.set(key::LINKS, Value::Sequence(list));
        piece.set(key::RETIRED_BY, Value::from(taker.id.to_string()));
        subject = format!("{subject}; {} takes its place", taker.id);
    }
    let outside = check::outside_lists(Layer::Memory, &piece, &store.config);
    if !outside.is_empty() {
        return Err(StoreError::Invalid(outside.join("; ")));
    }
    let home = ready(store, &rel)?;
    refuse_dirty(home.as_ref(), &rel)?;
    let commit = write_and_commit(store, home.as_ref(), &[(rel, Some(piece.render()))], &subject)?;
    Ok(Written { changed: true, commit, ..unchanged })
}

pub(crate) fn journal(store: &mut Store, day: Option<&str>, text: &str, repo: Option<&str>) -> Result<Journaled, StoreError> {
    if text.trim().is_empty() {
        return Err(StoreError::Invalid("a journal line needs a text".into()));
    }
    let now = Local::now();
    let date = match day.map(str::trim).filter(|d| !d.is_empty()) {
        Some(day) => NaiveDate::parse_from_str(day, DATE_FORMAT)
            .ok()
            .filter(|d| d.format(DATE_FORMAT).to_string() == day)
            .ok_or_else(|| StoreError::Invalid(format!("{day} is not a day written YYYY-MM-DD")))?,
        None => now.date_naive(),
    };
    let (folder, repo) = repo_named(store, repo)?;
    let days = store
        .config
        .folder_of(Layer::Diary)
        .ok_or_else(|| StoreError::Config("no folder holds the journal".into()))?
        .to_string();
    let rel = match &folder {
        Some(f) => format!("{f}/{days}/{date}.md"),
        None => format!("{days}/{date}.md"),
    };
    let id = PieceId::Named { folder: days, stem: date.to_string() };
    let held = store.index.entries.iter().find(|e| e.path_text() == rel).cloned();
    let (mut piece, made) = match held {
        Some(entry) => {
            let path = store.root.join(&rel);
            let text = fs::read_to_string(&path).map_err(|e| io_error(&path, e))?;
            (Piece::parse(&entry.path, &text)?, false)
        }
        None if store.root.join(&rel).exists() => {
            return Err(StoreError::Invalid(format!("{rel} is there but its header cannot be read")));
        }
        None => {
            let mut piece = Piece::empty(PathBuf::from(&rel));
            piece.set(key::DATE, Value::from(date.to_string()));
            if let Some(kind) = layer::default_kind(Layer::Diary) {
                piece.set(key::KIND, Value::from(kind));
            }
            (piece, true)
        }
    };
    let time = now.format(diary::LINE_TIME).to_string();
    piece.set_body(diary::appended(piece.body(), &diary::timed_line(&time, text)));
    piece.set(key::UPDATED, Value::from(now.format(DATE_FORMAT).to_string()));
    let home = ready(store, &rel)?;
    if !made {
        refuse_dirty(home.as_ref(), &rel)?;
    }
    let subject = format!("{}({id}): {}", Kind::Note, short(text));
    let commit = write_and_commit(store, home.as_ref(), &[(rel.clone(), Some(piece.render()))], &subject)?;
    Ok(Journaled { id, day: date.to_string(), repo, path: rel, made, time, commit })
}

/// A settings file at the root other than `config.yaml` (the person's rules), its new text
/// whole, as one commit by Lorisse; written alone when no git holds it.
pub(crate) fn setting(store: &mut Store, file: &str, text: &str, subject: &str) -> Result<Option<String>, StoreError> {
    let plain = file.ends_with(".yaml") && !file.contains(['/', '\\']) && !file.starts_with('.');
    if !plain || file == crate::config::CONFIG_FILE {
        return Err(StoreError::Invalid(format!("{file} is not a settings file Lorisse writes")));
    }
    let home = ready(store, file)?;
    refuse_dirty(home.as_ref(), file)?;
    write_and_commit(store, home.as_ref(), &[(file.to_string(), Some(text.to_string()))], &one_line(subject))
}

/// Copy `source` to `file_rel`, write the piece, then one commit by Lorisse of the two when git
/// holds them; on any failure both are put back, the copy removed with a folder made for it.
/// The commit's full id, `None` without git.
fn copy_and_commit(
    store: &Store,
    home: Option<&Home>,
    source: &Path,
    file_rel: &str,
    piece_rel: &str,
    text: &str,
    subject: &str,
) -> Result<Option<String>, StoreError> {
    let file = store.root.join(file_rel);
    let dir = file.parent().map(Path::to_path_buf);
    let made_dir = dir.as_ref().is_some_and(|d| !d.exists());
    let saved = save([store.root.join(piece_rel), file.clone()]);
    let inner = match home {
        Some(home) => vec![home.inner(file_rel), home.inner(piece_rel)],
        None => Vec::new(),
    };
    let written = attach::copy_file(source, &file)
        .and_then(|()| write_atomic(&store.root.join(piece_rel), text));
    let made = written.and_then(|()| match home {
        Some(home) => git::commit(&home.dir, &inner, &[subject, BODY], Who::Lorisse).map(|c| c.map(Some)),
        None => Ok(Some(None)),
    });
    let error = match made {
        Ok(Some(id)) => return Ok(id),
        Ok(None) => StoreError::Git("nothing to commit".into()),
        Err(e) => e,
    };
    if let Some(home) = home {
        unstage(home, &inner);
    }
    restore(&saved);
    let mut tmp = file.into_os_string();
    tmp.push(".tmp");
    let _ = fs::remove_file(PathBuf::from(tmp));
    if let (true, Some(dir)) = (made_dir, dir) {
        let _ = fs::remove_dir(dir);
    }
    Err(failed(error))
}

/// The files as they were before a write: each path and its bytes, `None` when absent.
pub(crate) type Saved = Vec<(PathBuf, Option<Vec<u8>>)>;

pub(crate) fn save(paths: impl IntoIterator<Item = PathBuf>) -> Saved {
    paths.into_iter().map(|p| { let bytes = fs::read(&p).ok(); (p, bytes) }).collect()
}

/// Every file put back as it was saved; a failure is silent, the next one is still tried.
pub(crate) fn restore(saved: &Saved) {
    for (path, bytes) in saved {
        let _ = match bytes {
            Some(bytes) => fs::write(path, bytes),
            None if path.exists() => fs::remove_file(path),
            None => Ok(()),
        };
    }
}

/// The paths unstaged again after a commit that failed; a failure is silent.
pub(crate) fn unstage(home: &Home, inner: &[String]) {
    let mut args = vec!["reset", "-q", "--"];
    args.extend(inner.iter().map(String::as_str));
    let _ = git::run(&home.dir, &args, Who::Folder);
}

pub(crate) fn failed(e: StoreError) -> StoreError {
    StoreError::Git(format!("the change could not be committed, the folder is as it was: {e}"))
}

/// Write each change (a text, or `None` for a removal), then one commit by Lorisse of exactly
/// those paths when git holds them; on any failure every file is put back and unstaged. The
/// commit's full id, `None` without git.
pub(crate) fn write_and_commit(
    store: &Store,
    home: Option<&Home>,
    changes: &[(String, Option<String>)],
    subject: &str,
) -> Result<Option<String>, StoreError> {
    let saved = save(changes.iter().map(|(rel, _)| store.root.join(rel)));
    let mut outcome = Ok(());
    for (rel, after) in changes {
        let path = store.root.join(rel);
        outcome = match after {
            Some(text) => write_atomic(&path, text),
            None => fs::remove_file(&path).map_err(|e| io_error(&path, e)),
        };
        if outcome.is_err() {
            break;
        }
    }
    let Some(home) = home else {
        if let Err(e) = outcome {
            restore(&saved);
            return Err(StoreError::Io(format!("the change could not be written, the folder is as it was: {e}")));
        }
        return Ok(None);
    };
    let inner: Vec<String> = changes.iter().map(|(rel, _)| home.inner(rel)).collect();
    let made = outcome.and_then(|()| git::commit(&home.dir, &inner, &[subject, BODY], Who::Lorisse));
    match made {
        Ok(Some(id)) => Ok(Some(id)),
        Ok(None) => {
            restore(&saved);
            Err(failed(StoreError::Git("nothing to commit".into())))
        }
        Err(e) => {
            unstage(home, &inner);
            restore(&saved);
            Err(failed(e))
        }
    }
}

/// The body line of the commit that keeps the folder's counter: never taken back by an undo.
const COUNTER_BODY: &str = "job: counter";

/// The `job:` word of that commit, as [`direct::commits`] reads it.
const COUNTER_JOB: &str = "counter";

/// The counter file of `id`'s numbering, from the root, and its repository, when that is
/// another history than `home`'s and git keeps the file there; `None` otherwise.
fn counter_apart(store: &Store, home: &Home, id: &PieceId) -> Option<(String, Home)> {
    let prefix = id.prefix()?;
    let rows = repos::numbered_rows(&store.config);
    let file = match rows.iter().find(|(_, own)| *own == prefix) {
        Some((folder, _)) => format!("{folder}/{IDS_FILE}"),
        None => IDS_FILE.to_string(),
    };
    let other = repos::home_for_write(store, &file)?;
    let kept = other.dir != home.dir && !git::is_ignored(&other.dir, &other.inner(&file));
    kept.then_some((file, other))
}

/// The counter raised to `id` in its own history, one commit signed as Lorisse's; on a failure
/// the file is put back as it was, the number still carried by the piece's own file.
fn keep_counter(store: &Store, (file, other): &(String, Home), id: &PieceId) {
    let PieceId::Numbered { prefix, n } = id else { return };
    let path = store.root.join(file);
    let saved = save([path.clone()]);
    if !matches!(index::raise_in(&path, prefix, *n), Ok(true)) {
        restore(&saved);
        return;
    }
    let id_text = id.padded(store.config.id_width);
    let subject = format!("{}({id_text}): the folder's counter, {id_text} given", Kind::Remember);
    let inner = vec![other.inner(file)];
    if !matches!(git::commit(&other.dir, &inner, &[&subject, COUNTER_BODY], Who::Lorisse), Ok(Some(_))) {
        unstage(other, &inner);
        restore(&saved);
    }
}

/// A counter the undo raised in another history than its own, committed there as Lorisse's;
/// on a failure it stays written, so the number is still never given again.
fn keep_counter_after_undo(other: &Home, rel: &str, given: &[String]) {
    let inner = vec![other.inner(rel)];
    if !git::dirty(&other.dir, &inner).is_ok_and(|d| !d.is_empty()) {
        return;
    }
    let subject = format!("{}({}): the folder's counter kept after an undo", Kind::Remember, given.join(", "));
    if git::commit(&other.dir, &inner, &[&subject, COUNTER_BODY], Who::Lorisse).is_err() {
        unstage(other, &inner);
    }
}

/// Whether a repository, or a file in it, is confidential: never written by the plugin.
fn confidential_in(store: &Store, home: &Home, files: &[String]) -> bool {
    let row = store.config.repos.iter().any(|r| r.name == home.name && r.confidential);
    let alias = home.name == CONFIDENTIAL && !store.config.repos.iter().any(|r| r.name == CONFIDENTIAL);
    row || alias || files.iter().any(|f| repos::is_confidential(&home.outer(f), &store.config))
}

/// The repository holding a commit.
fn home_of_commit(store: &Store, id: &str) -> Option<(Home, String)> {
    let homes = repos::homes(store);
    homes.into_iter().find_map(|h| git::resolve(&h.dir, id).ok().map(|full| (h, full)))
}

/// The commit an undo takes back: the one named, else Lorisse's last still standing.
fn chosen(store: &Store, change: Option<&str>) -> Result<Option<(Home, String)>, StoreError> {
    let mut commits = direct::commits(store, &Since::All)?;
    let counters: Vec<String> = commits.iter().filter(|c| c.job == COUNTER_JOB).map(|c| c.id.clone()).collect();
    commits.retain(|c| c.job != COUNTER_JOB);
    let standing = |c: &&direct::LorisseCommit| !c.undone;
    let Some(text) = change.map(str::trim).filter(|c| !c.is_empty()) else {
        for commit in commits.iter().filter(standing) {
            let Some((home, full)) = home_of_commit(store, &commit.id) else { continue };
            if !confidential_in(store, &home, &git::files_of(&home.dir, &full)?) {
                return Ok(Some((home, full)));
            }
        }
        return Ok(None);
    };
    if let Some(piece) = PieceId::parse(text) {
        let on = |c: &&direct::LorisseCommit| c.ids.iter().any(|i| PieceId::parse(i).as_ref() == Some(&piece));
        return match commits.iter().filter(on).find(standing) {
            Some(commit) => Ok(home_of_commit(store, &commit.id)),
            None if commits.iter().any(|c| on(&c)) => Err(StoreError::Invalid(format!(
                "Lorisse's changes on {piece} are taken back already"
            ))),
            None => Err(StoreError::Invalid(format!("Lorisse has made no change on {piece}"))),
        };
    }
    let (home, full) = home_of_commit(store, text)
        .ok_or_else(|| StoreError::Invalid(format!("no change is named {text} in this memory folder")))?;
    let short: String = full.chars().take(7).collect();
    if counters.contains(&full) {
        return Err(StoreError::Invalid(format!(
            "{short} keeps the folder's counter: a number is given once, so it is never taken back"
        )));
    }
    if git::author_mail(&home.dir, &full)? != LORISSE_EMAIL {
        return Err(StoreError::Invalid(format!(
            "{short} is not a change of Lorisse's: undo takes back hers only"
        )));
    }
    if commits.iter().any(|c| c.id == full && c.undone) {
        return Err(StoreError::Invalid(format!("{short} is taken back already")));
    }
    Ok(Some((home, full)))
}

/// Every `ids.json` of the folder, relative to the root.
fn ids_files(store: &Store) -> Vec<String> {
    let rows = repos::numbered_rows(&store.config);
    let mut out = vec![IDS_FILE.to_string()];
    out.extend(rows.iter().map(|(folder, _)| format!("{folder}/{IDS_FILE}")));
    out
}

pub(crate) fn undo(store: &mut Store, change: Option<&str>) -> Result<Option<Undone>, StoreError> {
    if let Some(said) = blocked(store) {
        return Err(StoreError::Invalid(said.to_string()));
    }
    let Some((home, full)) = chosen(store, change)? else {
        return Ok(None);
    };
    if let Some(said) = unsigned(&home) {
        return Err(StoreError::Invalid(said.to_string()));
    }
    let files = git::files_of(&home.dir, &full)?;
    if confidential_in(store, &home, &files) {
        return Err(StoreError::Confidential(full.chars().take(7).collect()));
    }
    let ids_all = ids_files(store);
    let here: Vec<String> = ids_all
        .iter()
        .filter(|rel| repos::home_for_write(store, rel).is_some_and(|h| h.dir == home.dir))
        .map(|rel| home.inner(rel))
        .collect();
    let apart: Vec<(String, Home)> = ids_all
        .iter()
        .filter_map(|rel| {
            let other = repos::home_for_write(store, rel)?;
            let kept = other.dir != home.dir && !git::is_ignored(&other.dir, &other.inner(rel));
            kept.then(|| (rel.clone(), other))
        })
        .collect();
    let pieces: Vec<String> = files.iter().filter(|f| !here.contains(f)).cloned().collect();
    direct::refuse_later(&home.dir, &full, &pieces)?;
    if let Some(file) = git::dirty(&home.dir, &here)?.first() {
        return Err(StoreError::Invalid(format!(
            "{file} has changes not committed: commit them or drop them first"
        )));
    }
    for (rel, other) in &apart {
        refuse_dirty(Some(other), rel)?;
    }
    let subject = git::run(&home.dir, &["log", "-1", "--format=%s", &full], Who::Folder)?;
    let subject = subject.trim().to_string();
    let drawer = drawers::folder_of_subject(&subject).map(|f| home.outer(&f));
    if let Some(folder) = &drawer {
        drawers::refuse_filled(store, folder)?;
    }
    let added = git::run(
        &home.dir,
        &["diff-tree", "--root", "--no-commit-id", "--name-only", "-r", "--diff-filter=A", &full],
        Who::Folder,
    )?;
    let given: Vec<String> = added
        .lines()
        .filter_map(|f| ids::from_path(&home.outer(f.trim()), &store.config.archive))
        .filter(|id| matches!(id, PieceId::Numbered { .. }))
        .map(|id| id.padded(store.config.id_width))
        .collect();
    let numbered = !given.is_empty();
    let mut paths: Vec<PathBuf> = files.iter().map(|f| home.dir.join(f)).collect();
    paths.extend(ids_all.iter().map(|rel| store.root.join(rel)));
    let saved = save(paths);
    let back = |e: StoreError, abort: bool| {
        if abort {
            let _ = git::run(&home.dir, &["revert", "--abort"], Who::Folder);
        }
        let mut all = files.clone();
        all.extend(here.iter().cloned());
        unstage(&home, &all);
        restore(&saved);
        failed(e)
    };
    if let Err(e) = git::run(&home.dir, &["revert", "--no-commit", &full], Who::Folder) {
        return Err(back(e, true));
    }
    if numbered {
        let mut max = store.index.max.clone();
        if let Err(e) = index::keep_high(&store.root, &store.config, &mut max) {
            return Err(back(e, true));
        }
        let kept: Vec<&str> = here
            .iter()
            .filter(|p| home.dir.join(p).is_file() && !git::is_ignored(&home.dir, p))
            .map(String::as_str)
            .collect();
        if !kept.is_empty() {
            let mut args = vec!["add", "--"];
            args.extend(kept);
            if let Err(e) = git::run(&home.dir, &args, Who::Folder) {
                return Err(back(e, true));
            }
        }
    }
    let title = format!("Revert \"{subject}\"");
    let why = format!("This reverts commit {full}.");
    let args = ["commit", "--quiet", "-m", &title, "-m", &why];
    if let Err(e) = git::run(&home.dir, &args, home.owner()) {
        return Err(back(e, true));
    }
    let revert = git::head(&home.dir)?;
    if let Some(folder) = &drawer {
        drawers::remove_if_empty(&store.root.join(folder));
    }
    if numbered {
        for (rel, other) in &apart {
            keep_counter_after_undo(other, rel, &given);
        }
    }
    let ids = direct::commits(store, &Since::All)?
        .into_iter()
        .find(|c| c.id == full)
        .map(|c| c.ids)
        .unwrap_or_default();
    Ok(Some(Undone {
        commit: full,
        subject,
        ids,
        revert,
    }))
}
