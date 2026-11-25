//! Filing — the plugin's writing calls: a memory filed, a note, a link, each one commit by
//! Lorisse in the repository holding the piece, and the undo of one of her changes.
//!
//! Each call checks first (git, the folder's identity, the folder's rules), then writes, then
//! commits; a commit that fails puts every file back as it was. The undo of a filed memory keeps
//! its number in `ids.json`, in the same commit, so the number is never given again.
//! Flow: `Store::open_write` → `remember` / `note` / `link` / `attach` → files → one commit; `undo`.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use serde_yaml_ng::Value;

use crate::attach::{self, Attachment};
use crate::check;
use crate::config::status;
use crate::direct::{self, Since};
use crate::error::StoreError;
use crate::frontmatter::key;
use crate::git::{self, LORISSE_EMAIL, Who};
use crate::ids::{self, PieceId};
use crate::index::{self, Entry, IDS_FILE, io_error, write_atomic};
use crate::inbox::{DATE_FORMAT, TIME_FORMAT};
use crate::layer::{CONFIDENTIAL, Layer};
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
    /// Lorisse's commit, its full id; `None` when nothing changed.
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
    /// Lorisse's commit, its full id.
    pub commit: String,
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
const BODY: &str = "job: plugin";

/// One line: newlines folded to spaces, trimmed.
fn one_line(text: &str) -> String {
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

/// What stops a write in the folder: git, the root's repository, its identity.
pub(crate) fn blocked(store: &Store) -> Option<Blocked> {
    if !git::available() {
        return Some(Blocked::NoGit);
    }
    match repos::root_home(store) {
        None => Some(Blocked::NotUnderGit),
        Some(home) => unsigned(&home),
    }
}

/// `NoIdentity` when the repository's own commits have no name or address.
fn unsigned(home: &Home) -> Option<Blocked> {
    let named = matches!(home.owner(), Who::Named { .. });
    (!named && !git::has_identity(&home.dir)).then(|| Blocked::NoIdentity(home.name.clone()))
}

/// The repository a write at `rel` commits in, once nothing blocks it.
fn ready(store: &Store, rel: &str) -> Result<Home, StoreError> {
    if let Some(said) = blocked(store) {
        return Err(StoreError::Invalid(said.to_string()));
    }
    let home = repos::home_for_write(store, rel)
        .ok_or_else(|| StoreError::Invalid(Blocked::NotUnderGit.to_string()))?;
    match unsigned(&home) {
        Some(said) => Err(StoreError::Invalid(said.to_string())),
        None => Ok(home),
    }
}

/// The root row's name.
fn root_name(store: &Store) -> String {
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
        let seen = store.repos().into_iter().filter(|r| !r.confidential && r.present);
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
fn visible<'a>(store: &'a Store, text: &str) -> Result<&'a Entry, StoreError> {
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

/// Refuse a piece whose file has changes the person has not committed.
fn refuse_dirty(home: &Home, rel: &str) -> Result<(), StoreError> {
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
    let id = store.next_id("M", row).padded(store.config.id_width);
    let memories = store.config.folder_of(Layer::Memory).unwrap_or("memories");
    let rel = match &folder {
        Some(f) => format!("{f}/{memories}/{id}.md"),
        None => format!("{memories}/{id}.md"),
    };
    if store.root.join(&rel).exists() {
        return Err(StoreError::Exists(PathBuf::from(&rel)));
    }
    let now = Local::now().fixed_offset();
    let mut piece = Piece::empty(PathBuf::from(&rel));
    piece.set(key::ID, Value::from(id.as_str()));
    piece.set(key::TITLE, Value::from(title.as_str()));
    piece.set(key::KIND, Value::from(kind));
    let tags = memory.tags.iter().map(|t| one_line(t)).filter(|t| !t.is_empty());
    piece.set(key::TAGS, Value::Sequence(tags.map(Value::from).collect()));
    if store.config.sources.iter().any(|s| s == "chat") {
        piece.set(key::SOURCE, Value::from("chat"));
    }
    piece.set(key::CAPTURED, Value::from(now.format(TIME_FORMAT).to_string()));
    piece.set(key::UPDATED, Value::from(now.format(DATE_FORMAT).to_string()));
    piece.set(key::STATUS, Value::from(status::SORTED));
    piece.set(key::LINKS, Value::Sequence(linked));
    piece.set_body(format!("{text}\n"));
    let outside = check::outside_lists(Layer::Memory, &piece, &store.config);
    if !outside.is_empty() {
        return Err(StoreError::Invalid(outside.join("; ")));
    }
    let home = ready(store, &rel)?;
    let subject = format!("{}({id}): {}", Kind::Remember, short(&title));
    let commit = write_and_commit(store, &home, &[(rel, Some(piece.render()))], &subject)?;
    let id = PieceId::parse(&id).ok_or_else(|| StoreError::Invalid(format!("{id}: not an id")))?;
    Ok(Written {
        id,
        repo,
        commit: Some(commit),
    })
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
    refuse_dirty(&home, &rel)?;
    let subject = format!("{}({}): {}", Kind::Note, entry.id, short(&line));
    let commit = write_and_commit(store, &home, &[(rel, Some(after))], &subject)?;
    Ok(Written {
        repo: repo_of(store, &entry),
        id: entry.id,
        commit: Some(commit),
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
    refuse_dirty(&home, &rel)?;
    let subject = format!("{}({}): to {}", Kind::Link, entry.id, target.id);
    let commit = write_and_commit(store, &home, &[(rel, Some(piece.render()))], &subject)?;
    Ok(Written {
        commit: Some(commit),
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
    refuse_dirty(&home, &rel)?;
    let subject = format!("{}({}): {}", Kind::Attach, entry.id, short(&name));
    let commit = copy_and_commit(store, &home, source, &file_rel, &rel, &piece.render(), &subject)?;
    Ok(Attached {
        repo: repo_of(store, &entry),
        id: entry.id,
        name,
        path: file_rel,
        commit,
    })
}

/// Copy `source` to `file_rel`, write the piece, then one commit by Lorisse of the two; on any
/// failure both are put back, the copy removed with a folder made for it. The commit's full id.
fn copy_and_commit(
    store: &Store,
    home: &Home,
    source: &Path,
    file_rel: &str,
    piece_rel: &str,
    text: &str,
    subject: &str,
) -> Result<String, StoreError> {
    let file = store.root.join(file_rel);
    let dir = file.parent().map(Path::to_path_buf);
    let made_dir = dir.as_ref().is_some_and(|d| !d.exists());
    let saved = save([store.root.join(piece_rel), file.clone()]);
    let inner = vec![home.inner(file_rel), home.inner(piece_rel)];
    let written = attach::copy_file(source, &file)
        .and_then(|()| write_atomic(&store.root.join(piece_rel), text));
    let made = written.and_then(|()| git::commit(&home.dir, &inner, &[subject, BODY], Who::Lorisse));
    let error = match made {
        Ok(Some(id)) => return Ok(id),
        Ok(None) => StoreError::Git("nothing to commit".into()),
        Err(e) => e,
    };
    unstage(home, &inner);
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
type Saved = Vec<(PathBuf, Option<Vec<u8>>)>;

fn save(paths: impl IntoIterator<Item = PathBuf>) -> Saved {
    paths.into_iter().map(|p| { let bytes = fs::read(&p).ok(); (p, bytes) }).collect()
}

/// Every file put back as it was saved; a failure is silent, the next one is still tried.
fn restore(saved: &Saved) {
    for (path, bytes) in saved {
        let _ = match bytes {
            Some(bytes) => fs::write(path, bytes),
            None if path.exists() => fs::remove_file(path),
            None => Ok(()),
        };
    }
}

/// The paths unstaged again after a commit that failed; a failure is silent.
fn unstage(home: &Home, inner: &[String]) {
    let mut args = vec!["reset", "-q", "--"];
    args.extend(inner.iter().map(String::as_str));
    let _ = git::run(&home.dir, &args, Who::Folder);
}

fn failed(e: StoreError) -> StoreError {
    StoreError::Git(format!("the change could not be committed, the folder is as it was: {e}"))
}

/// Write each change (a text, or `None` for a removal), then one commit by Lorisse of exactly
/// those paths; on any failure every file is put back and unstaged. The commit's full id.
fn write_and_commit(
    store: &Store,
    home: &Home,
    changes: &[(String, Option<String>)],
    subject: &str,
) -> Result<String, StoreError> {
    let saved = save(changes.iter().map(|(rel, _)| store.root.join(rel)));
    let inner: Vec<String> = changes.iter().map(|(rel, _)| home.inner(rel)).collect();
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
    let made = outcome.and_then(|()| git::commit(&home.dir, &inner, &[subject, BODY], Who::Lorisse));
    match made {
        Ok(Some(id)) => Ok(id),
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
    let commits = direct::commits(store, &Since::All)?;
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
    let pieces: Vec<String> = files.iter().filter(|f| !here.contains(f)).cloned().collect();
    direct::refuse_later(&home.dir, &full, &pieces)?;
    if let Some(file) = git::dirty(&home.dir, &here)?.first() {
        return Err(StoreError::Invalid(format!(
            "{file} has changes not committed: commit them or drop them first"
        )));
    }
    let subject = git::run(&home.dir, &["log", "-1", "--format=%s", &full], Who::Folder)?;
    let subject = subject.trim().to_string();
    let added = git::run(
        &home.dir,
        &["diff-tree", "--root", "--no-commit-id", "--name-only", "-r", "--diff-filter=A", &full],
        Who::Folder,
    )?;
    let numbered = added
        .lines()
        .filter_map(|f| ids::from_path(&home.outer(f.trim()), &store.config.archive))
        .any(|id| matches!(id, PieceId::Numbered { .. }));
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
