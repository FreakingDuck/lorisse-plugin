//! Store — the open data folder: its root, its config, its index and its access.
//!
//! One write path: `capture` creates a file; every other write is `accept(diff)`, or `apply`,
//! which routes a job's action to the same write plus one commit, or to a proposal file.
//! Flow: `open` → `scan` → `Index` → `list` / `find` / `links` → `propose` → `accept`.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use crate::actions::{self, Action};
use crate::attach::Attachment;
use crate::check::{self, Report};
use crate::config::{StoreConfig, status};
use crate::diary::{self, Speaker};
use crate::diff::{self, Diff};
use crate::direct::{self, Applied, Fined, Flagged, LorisseCommit, Since};
use crate::error::StoreError;
use crate::flags::Flags;
use crate::git;
use crate::ids::PieceId;
use crate::inbox::{self, Proposal, Source};
use crate::index::{self, Entry, Index, io_error};
use crate::kept::{self, Conversation, Kept};
use crate::layer::Layer;
use crate::links::{self, Links, Ref};
use crate::piece::Piece;
use crate::repos::{self, RepoState};
use crate::route::{self, Context, Routing};
use crate::search::{self, Hit};
use crate::tags::{self, TagInfo};

// =============================================================================
// Interface
// =============================================================================

/// Who is reading: the owner, or a model that must not see confidential or raw pieces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Sees every piece.
    Full,
    /// Never handed a piece of a confidential repository (its row's flag) or in the inbox.
    Free,
}

/// Which entries `list` returns; every field left `None` matches all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// The slice folder.
    pub slice: Option<String>,
    /// The layer.
    pub layer: Option<Layer>,
    /// The status word.
    pub status: Option<String>,
    /// A tag the entry carries.
    pub tag: Option<String>,
    /// Updated or captured on or after this day.
    pub since: Option<NaiveDate>,
    /// In a confidential repository or not.
    pub confidential: Option<bool>,
}

/// The open data folder.
#[derive(Debug)]
pub struct Store {
    /// The data folder.
    pub(crate) root: PathBuf,
    /// Its `config.yaml`.
    pub(crate) config: StoreConfig,
    /// Who reads.
    pub(crate) access: Access,
    /// The pieces this access sees.
    pub(crate) index: Index,
}

impl Store {
    /// Open a data folder: read the config, write a listed repository's folder missing from the
    /// root's `.gitignore` (one commit of that line), index every piece, write `index.json`.
    ///
    /// # Errors
    /// [`StoreError::Config`] for a bad `config.yaml`; [`StoreError::Io`] for the folder.
    pub fn open(root: &Path, access: Access) -> Result<Store, StoreError> {
        open_store(root, access)
    }

    /// The data folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The configuration in use.
    pub fn config(&self) -> &StoreConfig {
        &self.config
    }

    /// The access the store was opened with.
    pub fn access(&self) -> Access {
        self.access
    }

    /// Capture a text as a raw memory in the inbox, its typed flags read; at any access.
    ///
    /// # Errors
    /// [`StoreError::Invalid`] for a source not in the config; [`StoreError::Io`].
    pub fn capture(&mut self, text: &str, source: Source) -> Result<Piece, StoreError> {
        self.capture_with(text, source, &Flags::default(), &[])
    }

    /// Capture with flags beside the typed ones and files copied to `inbox/<id>/`.
    ///
    /// # Errors
    /// As [`Store::capture`]; [`StoreError::Io`] for a missing file, [`StoreError::Invalid`]
    /// for a name given twice; nothing is written on any of them.
    pub fn capture_with(
        &mut self,
        text: &str,
        source: Source,
        flags: &Flags,
        files: &[PathBuf],
    ) -> Result<Piece, StoreError> {
        capture_given(self, None, text, &source, flags, files)
    }

    /// Capture as [`Store::capture_with`], for the repository named `repo`: in the root's inbox,
    /// numbered as that repository numbers its pieces.
    ///
    /// # Errors
    /// As [`Store::capture_with`]; [`StoreError::Invalid`] for a name no row has.
    pub fn capture_in(
        &mut self,
        repo: &str,
        text: &str,
        source: Source,
        flags: &Flags,
        files: &[PathBuf],
    ) -> Result<Piece, StoreError> {
        capture_given(self, Some(repo), text, &source, flags, files)
    }

    /// The files of the piece a link names, paths relative to the root.
    ///
    /// # Errors
    /// As [`Store::read`]: behind the gate the files are refused with the piece.
    pub fn files(&self, id: &Ref) -> Result<Vec<Attachment>, StoreError> {
        Ok(read_piece(self, id)?.files())
    }

    /// Read the piece a link names.
    ///
    /// # Errors
    /// [`StoreError::Confidential`] (the id only) behind the gate, [`StoreError::Unknown`].
    pub fn read(&self, id: &Ref) -> Result<Piece, StoreError> {
        read_piece(self, id)
    }

    /// The visible entries matching `filter`, in id order.
    pub fn list(&self, filter: &Filter) -> Vec<&Entry> {
        self.index
            .entries
            .iter()
            .filter(|e| matches(filter, e))
            .collect()
    }

    /// The visible captures waiting in the inbox: its raw pieces only.
    pub fn inbox(&self) -> Vec<&Entry> {
        inbox_entries(self, true)
    }

    /// The visible inbox pieces no longer raw: retired where they lie.
    pub fn retired_in_inbox(&self) -> Vec<&Entry> {
        inbox_entries(self, false)
    }

    /// Append a timed note to the diary day `date`, made when absent; the day's id.
    ///
    /// # Errors
    /// [`StoreError::Invalid`] for a blank text, [`StoreError::Confidential`] for a day behind
    /// the gate, [`StoreError::Exists`] for a file the index does not hold; [`StoreError::Io`].
    pub fn diary_append(&mut self, date: NaiveDate, text: &str) -> Result<PieceId, StoreError> {
        let id = diary::append(self, Layer::Diary, date, text)?;
        self.rebuild_index()?;
        Ok(id)
    }

    /// Append `who: text` as a timed line to the conversation day `date`, made when absent; the
    /// day's id.
    ///
    /// # Errors
    /// As [`Store::diary_append`], for the conversation day.
    pub fn conversation_append(
        &mut self,
        date: NaiveDate,
        who: Speaker,
        text: &str,
    ) -> Result<PieceId, StoreError> {
        append_spoken(self, date, who, text)
    }

    /// Store a conversation of the app as one memory of kind `conversation`: made on its first store, found by its
    /// `thread:` and updated after (its links and models joined); nothing written when it says the same; no commit.
    ///
    /// # Errors
    /// [`StoreError::Invalid`] for a blank thread or body; as [`Store::accept`].
    pub fn keep_conversation(&mut self, conversation: &Conversation) -> Result<Kept, StoreError> {
        kept::keep(self, conversation)
    }

    /// Every visible stored conversation (a memory of kind `conversation` with its `thread:`), in id order.
    pub fn conversations(&self) -> Vec<Piece> {
        kept::conversations(self)
    }

    /// Entries whose title, then whose tags, hold `text` (case-insensitive).
    pub fn find(&self, text: &str) -> Vec<&Entry> {
        search::find(&self.index, text)
    }

    /// Title hits, tag hits, then body hits, each in id order.
    ///
    /// # Errors
    /// [`StoreError::Io`] when a file cannot be read.
    pub fn search(&self, text: &str) -> Result<Vec<Hit>, StoreError> {
        search::search(&self.root, &self.index, text)
    }

    /// What the piece links to and which visible pieces link to it.
    pub fn links(&self, id: &Ref) -> Links {
        links::links_of(&self.index, id, &self.config.archive)
    }

    /// Every visible link naming no piece: the source id and the link.
    pub fn dangling(&self) -> Vec<(PieceId, Ref)> {
        links::dangling_links(&self.index, &self.config.archive)
    }

    /// The diff an action would write; nothing is written.
    ///
    /// # Errors
    /// [`StoreError::Unknown`], [`StoreError::Confidential`], [`StoreError::Invalid`].
    pub fn propose(&self, action: Action) -> Result<Diff, StoreError> {
        actions::propose(self, action)
    }

    /// Write a diff exactly, or nothing when a file changed; the ids written.
    ///
    /// # Errors
    /// [`StoreError::Changed`], [`StoreError::Exists`], [`StoreError::Invalid`],
    /// [`StoreError::Confidential`]; nothing is written on any of them.
    pub fn accept(&mut self, diff: Diff) -> Result<Vec<PieceId>, StoreError> {
        let written = diff::accept(self, &diff)?;
        self.rebuild_index()?;
        Ok(written)
    }

    /// Drop a diff: the proposal file it came from is deleted, nothing else.
    ///
    /// # Errors
    /// [`StoreError::Io`] when the proposal file cannot be deleted.
    pub fn reject(&self, diff: Diff) -> Result<(), StoreError> {
        diff::reject(self, &diff)
    }

    /// Skip a proposal file: kept as `<name>.skipped.yaml`; the ids written (a capture retired
    /// where it lies when this was its last file and an earlier one was accepted).
    ///
    /// # Errors
    /// [`StoreError::Confidential`] for a free store, [`StoreError::Exists`], [`StoreError::Io`].
    pub fn skip(&mut self, proposal: &Proposal) -> Result<Vec<PieceId>, StoreError> {
        skip_file(self, proposal)
    }

    /// The first free proposal file name of a capture: `<id>.sort.yaml`, `<id>.2.sort.yaml` …
    pub fn proposal_path(&self, id: &PieceId) -> PathBuf {
        inbox::next_path(&self.root, &self.config, id)
    }

    /// Rebuild the index from the files, write `index.json`, keep the largest numbers in `ids.json`;
    /// the number of entries.
    ///
    /// # Errors
    /// [`StoreError::Io`] when the folder, `index.json` or `ids.json` cannot be read or written.
    pub fn rebuild_index(&mut self) -> Result<usize, StoreError> {
        rebuild(self)
    }

    /// Walk the folder again and report what is off, Lorisse's commits included in a git folder.
    ///
    /// # Errors
    /// [`StoreError::Io`] when the folder cannot be walked.
    pub fn check(&self) -> Result<Report, StoreError> {
        check_folder(self)
    }

    /// The proposal files in the inbox (`<id>.sort.yaml`, `<id>.2.sort.yaml` …); none for free.
    ///
    /// # Errors
    /// [`StoreError::Yaml`] naming the first proposal that is not an action; see
    /// [`Store::proposal_files`] for a list that carries each file's error.
    pub fn proposals(&self) -> Result<Vec<(PieceId, Action)>, StoreError> {
        inbox::strict(proposal_list(self)?)
    }

    /// Every proposal file by capture then part, each parsed on its own, a broken one with its
    /// error; none for free.
    ///
    /// # Errors
    /// [`StoreError::Io`] when the folder cannot be walked.
    pub fn proposal_files(&self) -> Result<Vec<Proposal>, StoreError> {
        proposal_list(self)
    }

    /// The next id of a piece of `prefix`'s layer landing in the repository named `repo` (`None`
    /// the root): a row with its own prefix numbers with it, else the root's numbering; the
    /// largest number ever seen in that numbering (its folders, its `ids.json`) plus one.
    pub fn next_id(&self, prefix: &str, repo: Option<&str>) -> PieceId {
        let own = repo.and_then(|name| repos::prefix_named(&self.config, name));
        inbox::next(&self.index, own.unwrap_or(prefix))
    }

    /// Every tag a file defines or a visible piece carries, the most carried first.
    pub fn tags(&self) -> Vec<TagInfo> {
        tags::list(self)
    }

    /// The owner's definition of `tag`, written into `tags/<slug>.md`, made when absent; its id.
    ///
    /// # Errors
    /// [`StoreError::Invalid`] for a blank tag or text, or a text of two lines; [`StoreError::Io`].
    pub fn tag_define(&mut self, tag: &str, definition: &str) -> Result<PieceId, StoreError> {
        let id = tags::define(self, tag, definition)?;
        self.rebuild_index()?;
        Ok(id)
    }

    /// The route a job's action would take with this context; nothing is written.
    ///
    /// # Errors
    /// As [`Store::propose`]; [`StoreError::Invalid`] for a kind the action does not fit.
    pub fn route(&self, action: &Action, context: &Context) -> Result<Routing, StoreError> {
        route::decide(self, action, context).map(|(routing, _)| routing)
    }

    /// Apply a job's action: written at once and committed by Lorisse, or left as a proposal file.
    ///
    /// # Errors
    /// [`StoreError::Invalid`] for a `confidence` above `low`; as [`Store::route`] and
    /// [`Store::accept`]; [`StoreError::Git`] when the files' pending state cannot be committed
    /// first (nothing written then).
    pub fn apply(&mut self, action: Action, context: &Context) -> Result<Applied, StoreError> {
        direct::apply(self, action, context)
    }

    /// Undo one of Lorisse's commits by `git revert`; the revert's commit id.
    ///
    /// # Errors
    /// [`StoreError::Invalid`] with a plain reason: no repository, not hers, undone already, a
    /// file changed since; [`StoreError::Git`] when the revert fails.
    pub fn undo(&mut self, commit: &str) -> Result<String, StoreError> {
        direct::undo(self, commit)
    }

    /// Lorisse's commits since a commit or a day, newest first; none outside a git folder.
    ///
    /// # Errors
    /// [`StoreError::Invalid`] for an unknown commit; [`StoreError::Git`].
    pub fn lorisse_commits(&self, since: &Since) -> Result<Vec<LorisseCommit>, StoreError> {
        direct::commits(self, since)
    }

    /// The pieces Lorisse wrote unsure, with her question ("To check").
    ///
    /// # Errors
    /// As [`Store::read`].
    pub fn to_check(&self) -> Result<Vec<(PieceId, String)>, StoreError> {
        direct::to_check(self)
    }

    /// The pieces Lorisse wrote unsure, each with her question and her proposed answers.
    ///
    /// # Errors
    /// As [`Store::read`].
    pub fn flagged(&self) -> Result<Vec<Flagged>, StoreError> {
        direct::flagged(self)
    }

    /// The owner's Fine on a live memory, task or person: `confidence: validated`, `to_check` gone, one commit
    /// by the folder's own identity (never Lorisse's); a job's action cannot write `validated` ([`Store::apply`]).
    ///
    /// # Errors
    /// [`StoreError::Unknown`], [`StoreError::Invalid`] for another layer or an archived piece, as [`Store::accept`].
    pub fn fine(&mut self, id: &PieceId) -> Result<Fined, StoreError> {
        direct::fine(self, id)
    }

    /// Whether the data folder is a git repository of its own (a `.git` at its root).
    pub fn is_git(&self) -> bool {
        git::is_repo(&self.root)
    }

    /// The repositories: the root, each `repos:` row, then `confidential/` when no row lists it
    /// and it is there; each present or absent on this machine, a repository or not.
    pub fn repos(&self) -> Vec<RepoState> {
        repos::states(self)
    }
}

// =============================================================================
// Implementation
// =============================================================================

fn rebuild(store: &mut Store) -> Result<usize, StoreError> {
    store.index = index::scan(&store.root, &store.config, store.access)?.index;
    index::keep_high(&store.root, &store.config, &mut store.index.max)?;
    index::write_index(&store.root, &store.index.entries)?;
    Ok(store.index.entries.len())
}

fn check_folder(store: &Store) -> Result<Report, StoreError> {
    let scan = index::scan(&store.root, &store.config, store.access)?;
    let commits = direct::findings(store, &scan);
    let mut report = check::report(scan, &store.config);
    report.findings.extend(commits);
    Ok(report)
}

fn open_store(root: &Path, access: Access) -> Result<Store, StoreError> {
    let mut store = Store {
        root: root.to_path_buf(),
        config: StoreConfig::load(root)?,
        access,
        index: Index::default(),
    };
    repos::ignore_rows(&store);
    store.rebuild_index()?;
    Ok(store)
}

fn append_spoken(
    store: &mut Store,
    date: NaiveDate,
    who: Speaker,
    text: &str,
) -> Result<PieceId, StoreError> {
    if text.trim().is_empty() {
        return Err(StoreError::Invalid(
            "a conversation line needs a text".into(),
        ));
    }
    let line = diary::spoken(who, text);
    let id = diary::append(store, Layer::Conversation, date, &line)?;
    store.rebuild_index()?;
    Ok(id)
}

fn capture_given(
    store: &mut Store,
    repo: Option<&str>,
    text: &str,
    source: &Source,
    flags: &Flags,
    files: &[PathBuf],
) -> Result<Piece, StoreError> {
    let known = |name: &str| {
        store
            .config
            .repos
            .iter()
            .any(|r| r.name.trim() == name.trim())
    };
    if let Some(name) = repo.filter(|name| !known(name)) {
        return Err(StoreError::Invalid(format!("repos: no row named {name}")));
    }
    let given = inbox::Given {
        text,
        source,
        flags,
        files,
    };
    let prefix = Layer::Memory.prefix().unwrap_or("M");
    let id = store.next_id(prefix, repo);
    let piece = inbox::capture(&store.root, &store.config, &id, &given)?;
    store.rebuild_index()?;
    Ok(piece)
}

/// The inbox entries that are raw (no status counts as raw), or those that are not.
fn inbox_entries(store: &Store, raw: bool) -> Vec<&Entry> {
    let prefix = format!("{}/", store.config.inbox);
    let is_raw = |e: &Entry| e.status.as_deref().is_none_or(|s| s == status::RAW);
    let entries = store.index.entries.iter();
    entries
        .filter(|e| e.path_text().starts_with(&prefix) && is_raw(e) == raw)
        .collect()
}

fn skip_file(store: &mut Store, proposal: &Proposal) -> Result<Vec<PieceId>, StoreError> {
    if store.access == Access::Free {
        return Err(StoreError::Confidential(proposal.id.to_string()));
    }
    inbox::skip_file(&store.root, &proposal.path)?;
    let Some(diff) = actions::retire_skipped(store, &proposal.id)? else {
        return Ok(Vec::new());
    };
    let written = diff::write(store, &diff)?;
    store.rebuild_index()?;
    Ok(written)
}

fn matches(filter: &Filter, entry: &Entry) -> bool {
    let since = filter.since.is_none_or(|since| {
        let captured = entry.captured.map(|c| c.date_naive());
        entry
            .updated
            .into_iter()
            .chain(captured)
            .any(|day| day >= since)
    });
    since
        && filter
            .slice
            .as_ref()
            .is_none_or(|s| entry.slice.as_ref() == Some(s))
        && filter.layer.is_none_or(|l| entry.layer == l)
        && filter
            .status
            .as_ref()
            .is_none_or(|s| entry.status.as_ref() == Some(s))
        && filter.tag.as_ref().is_none_or(|t| entry.tags.contains(t))
        && filter.confidential.is_none_or(|c| entry.confidential == c)
}

fn read_piece(store: &Store, id: &Ref) -> Result<Piece, StoreError> {
    if links::is_hidden(&store.index, id) {
        return Err(StoreError::Confidential(id.to_string()));
    }
    let entry = links::resolve(&store.index, id, &store.config.archive)
        .ok_or_else(|| StoreError::Unknown(id.to_string()))?;
    let path = store.root.join(&entry.path);
    let text = fs::read_to_string(&path).map_err(|e| io_error(&path, e))?;
    Piece::parse(&entry.path, &text)
}

fn proposal_list(store: &Store) -> Result<Vec<Proposal>, StoreError> {
    if store.access == Access::Free {
        return Ok(Vec::new());
    }
    Ok(inbox::pending(&store.root, &store.config))
}
