//! Direct — a job's action written at once, one commit by Lorisse each, and its undo.
//!
//! A proposal route writes the proposal file as before. A direct route is the accept's own write,
//! with `confidence: low` on every piece made or edited (a relinked one keeps its own), `to_check` on a flagged one and a provisional tag file
//! for a new tag; the files' pending state is committed first, by the folder's own identity.
//! Each repository the write touches gets its own commit; undo reverts in the one holding it.
//! Flow: `Action` + `Context` → `route::decide` → `Diff` (marked) → write → commits → `Applied`.

use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::Serialize;
use serde_yaml_ng::Value;

use crate::actions::{Action, slug};
use crate::check::Finding;
use crate::config::status;
use crate::diff::{self, Change, Diff};
use crate::error::StoreError;
use crate::frontmatter::key;
use crate::ids::{self, PieceId};
use crate::index::{Scan, write_atomic};
use crate::layer::{Layer, TAGS};
use crate::links::Ref;
use crate::piece::Piece;
use crate::route::{self, Certainty, Context, Kind, Route, Routing};
use crate::slice::Place;
use crate::store::Store;
use crate::tags;

use crate::git::{self, LORISSE_EMAIL, Who};
use crate::repos::{self, Home};

// =============================================================================
// Interface
// =============================================================================

/// What applying a job's action did.
#[derive(Debug, Clone, PartialEq)]
pub enum Applied {
    /// Written at once (direct, or direct and flagged).
    Direct(Box<DirectWrite>),
    /// Left as a proposal file, as before.
    Proposed {
        /// The proposal file, relative to the root.
        path: PathBuf,
        /// Why it is a proposal.
        routing: Routing,
    },
}

/// A direct write: what was written and its commit.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectWrite {
    /// The route and its reason.
    pub routing: Routing,
    /// The pieces written, a provisional tag file among them.
    pub ids: Vec<PieceId>,
    /// The diff written, marks included.
    pub diff: Diff,
    /// The commit in the repository holding the action's piece, or why there is none.
    pub commit: CommitState,
    /// The commits in the other repositories the write touched, one each.
    pub also: Vec<String>,
}

/// Whether a direct write was committed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitState {
    /// Committed: the commit's full id.
    Made(String),
    /// Written, not committed: why (no repository, git failed).
    NotMade(String),
}

/// Where "since you last looked" starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Since {
    /// Every commit of Lorisse.
    All,
    /// After this commit.
    Commit(String),
    /// From this day on, midnight.
    Date(NaiveDate),
}

/// One commit of Lorisse, read back from its message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LorisseCommit {
    /// The full commit id.
    pub id: String,
    /// When it was made.
    pub date: DateTime<FixedOffset>,
    /// The action's word (`sort`, `note` …).
    pub action: String,
    /// The pieces it wrote.
    pub ids: Vec<String>,
    /// Her one line of why.
    pub why: String,
    /// The job.
    pub job: String,
    /// `sure` or `unsure`.
    pub certainty: String,
    /// The capture it came from, `-` when none.
    pub source: String,
    /// The model.
    pub model: String,
    /// The turn's cost, in USD, as written; empty when another commit of the turn carries it.
    pub cost: String,
    /// The commit of the same turn that carries the cost (`cost: in <id>`), when not this one.
    pub cost_in: Option<String>,
    /// Whether a later commit reverts it.
    pub undone: bool,
}

/// A piece Lorisse flagged: her question and her proposed answers, the likeliest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flagged {
    /// The piece.
    pub id: PieceId,
    /// Her question.
    pub question: String,
    /// Her proposed answers, each `text (certainty)`; empty when she gave none.
    pub answers: Vec<String>,
}

/// What the owner's Fine wrote: the piece and his commit.
#[derive(Debug, Clone, PartialEq)]
pub struct Fined {
    /// The piece, now `confidence: validated`.
    pub id: PieceId,
    /// His commit, or why there is none.
    pub commit: CommitState,
}

// =============================================================================
// Implementation
// =============================================================================

/// The only confidence a job writes.
const LOW: &str = "low";

/// The confidence only the owner's own act writes.
const VALIDATED: &str = "validated";

/// The layers whose pieces carry `confidence`.
const RATED: [Layer; 3] = [Layer::Memory, Layer::Task, Layer::Person];

/// Unit separator and record separator of the log formats.
const FIELD: char = '\u{1f}';
const RECORD: char = '\u{1e}';

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// One line: newlines folded to spaces, trimmed.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Route a job's action, then write it at once or leave its proposal file.
pub(crate) fn apply(
    store: &mut Store,
    action: Action,
    context: &Context,
) -> Result<Applied, StoreError> {
    refuse_confidence(&action)?;
    let (routing, diff) = route::decide(store, &action, context)?;
    if routing.route == Route::Proposal {
        let path = write_proposal(store, &action, context)?;
        return Ok(Applied::Proposed { path, routing });
    }
    let written = write_direct(store, diff, routing, context)?;
    Ok(Applied::Direct(Box::new(written)))
}

/// A job may set `confidence` to `low` only: `checked` and `validated` are the owner's.
fn refuse_confidence(action: &Action) -> Result<(), StoreError> {
    let set = match action {
        Action::Sort { set, .. } | Action::Edit { set, .. } | Action::Move { set, .. } => set,
        _ => return Ok(()),
    };
    match set.get(key::CONFIDENCE) {
        None => Ok(()),
        Some(v) if v.as_str().map(str::trim) == Some(LOW) => Ok(()),
        Some(v) => {
            let said = v
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| format!("{v:?}"));
            Err(StoreError::Invalid(format!(
                "confidence: {said}: Lorisse writes low only; checked and validated are the owner's word"
            )))
        }
    }
}

/// The proposal file of the capture (else of the piece): `# why:`, `# by:`, `# certainty:`, the YAML.
fn write_proposal(
    store: &Store,
    action: &Action,
    context: &Context,
) -> Result<PathBuf, StoreError> {
    let capture = context.capture.clone().unwrap_or_else(|| action.target());
    let rel = store.proposal_path(&capture);
    let yaml = serde_yaml_ng::to_string(action)
        .map_err(|e| StoreError::Invalid(format!("the action as YAML: {e}")))?;
    let by = [&context.job, &context.provider, &context.model];
    let by = by.iter().map(|s| s.trim()).filter(|s| !s.is_empty());
    let by = by.collect::<Vec<_>>().join(" ");
    let mut text = format!("# why: {}\n", one_line(&context.why));
    if !by.is_empty() {
        text.push_str(&format!("# by: {by}\n"));
    }
    text.push_str(&format!(
        "# certainty: {}\n{yaml}",
        certainty_word(context.certainty)
    ));
    write_atomic(&store.root.join(&rel), &text)?;
    Ok(rel)
}

fn certainty_word(certainty: Certainty) -> &'static str {
    match certainty {
        Certainty::Sure => "sure",
        Certainty::Unsure => "unsure",
    }
}

/// The marked diff written as an accept writes it, then one commit by Lorisse.
fn write_direct(
    store: &mut Store,
    mut diff: Diff,
    routing: Routing,
    context: &Context,
) -> Result<DirectWrite, StoreError> {
    mark(store, &mut diff, &routing, context)?;
    provisional_tags(store, &mut diff)?;
    let groups = groups_of(store, &diff);
    if !diff.changes.is_empty() {
        commit_base(&groups, &diff, routing.kind)?;
    }
    let ids = diff::write(store, &diff)?;
    store.rebuild_index()?;
    let (commit, also) = if groups.is_empty() {
        let why = "not a git repository: written, not committed";
        (CommitState::NotMade(why.into()), Vec::new())
    } else if diff.changes.is_empty() {
        (CommitState::NotMade("nothing changed".into()), Vec::new())
    } else {
        commit_groups(store, &groups, routing.kind, &ids, context)
    };
    Ok(DirectWrite {
        routing,
        ids,
        diff,
        commit,
        also,
    })
}

/// The diff's paths by repository, the one holding the action's piece first.
fn groups_of(store: &Store, diff: &Diff) -> Vec<(Home, Vec<String>)> {
    let mut groups = repos::split(store, &touched(diff));
    let target = store.index.get(&diff.action.target());
    let first = target.and_then(|e| repos::home_of(store, &e.path_text()));
    if let Some(first) = first {
        groups.sort_by_key(|(home, _)| home.dir != first.dir);
    }
    groups
}

/// One commit by Lorisse per repository; the first carries the cost, the others name it.
fn commit_groups(
    store: &Store,
    groups: &[(Home, Vec<String>)],
    kind: Kind,
    ids: &[PieceId],
    context: &Context,
) -> (CommitState, Vec<String>) {
    let mut first: Option<CommitState> = None;
    let mut also = Vec::new();
    for (home, paths) in groups {
        let made = match &first {
            Some(CommitState::Made(id)) => Some(id.chars().take(7).collect::<String>()),
            _ if context.first_of_turn => None,
            _ => cost_home(store),
        };
        let (subject, body) = message(kind, ids, context, made.as_deref());
        let state = match git::commit(&home.dir, paths, &[&subject, &body], Who::Lorisse) {
            Ok(Some(id)) => CommitState::Made(id),
            Ok(None) => CommitState::NotMade("nothing to commit".into()),
            Err(e) => CommitState::NotMade(e.to_string()),
        };
        match (&first, state) {
            (None, state) => first = Some(state),
            (Some(_), CommitState::Made(id)) => also.push(id),
            (Some(_), CommitState::NotMade(_)) => {}
        }
    }
    let none = || CommitState::NotMade("nothing to commit".into());
    (first.unwrap_or_else(none), also)
}

/// Lorisse's marks, not a piece's content: her question on a task and its answers, the day put off, the day asked
/// again, her label for the map.
const MARKS: [&str; 6] = [
    key::QUESTION,
    key::ANSWERS,
    key::RESOLUTIONS,
    "asked",
    "snooze_until",
    key::LABEL,
];

/// A piece whose change touches its `links`, `updated` and her marks only: a relink or her question, its confidence kept.
fn only_relinked(change: &Change, after: &Piece) -> Result<bool, StoreError> {
    let Some(before) = &change.before else {
        return Ok(false);
    };
    let mut was = Piece::parse(change.from.as_ref().unwrap_or(&change.path), before)?;
    let mut now = after.clone();
    for name in [key::LINKS, key::UPDATED].into_iter().chain(MARKS) {
        was.remove(name);
        now.remove(name);
    }
    Ok(was.render() == now.render())
}

/// `confidence: low` on every rated piece made or edited, a relinked one kept; `to_check` on the flagged action's own pieces.
fn mark(
    store: &Store,
    diff: &mut Diff,
    routing: &Routing,
    context: &Context,
) -> Result<(), StoreError> {
    let flagged = routing.route == Route::DirectFlagged;
    let asked = context.question.as_deref().map(one_line);
    let question = asked
        .filter(|q| !q.is_empty())
        .unwrap_or_else(|| one_line(&context.why));
    let question = if question.is_empty() {
        "to check".to_string()
    } else {
        question
    };
    let answers: Vec<Value> = context
        .answers
        .iter()
        .map(|a| one_line(a))
        .filter(|a| !a.is_empty())
        .map(Value::from)
        .collect();
    let target = diff.action.target();
    let archive = store.config.archive.clone();
    for change in &mut diff.changes {
        let Some(after) = &change.after else {
            continue;
        };
        let rel = slashed(&change.path);
        if !Place::of(&rel, &store.config).is_some_and(|p| RATED.contains(&p.layer)) {
            continue;
        }
        let mut piece = Piece::parse(&change.path, after)?;
        if !only_relinked(change, &piece)? {
            piece.set(key::CONFIDENCE, Value::from(LOW));
        }
        let id = piece.id().or_else(|| ids::from_path(&rel, &archive));
        let own = id.as_ref() == Some(&target) || change.before.is_none();
        if flagged && own && piece.status() != Some(status::RETIRED) {
            piece.set(key::TO_CHECK, Value::from(question.as_str()));
            if !answers.is_empty() {
                piece.set(key::CHECK_ANSWERS, Value::Sequence(answers.clone()));
            }
        }
        change.after = Some(piece.render());
    }
    Ok(())
}

/// A tag added to a piece with no tag file gets `tags/<slug>.md`, provisional, in the same diff.
fn provisional_tags(store: &Store, diff: &mut Diff) -> Result<(), StoreError> {
    let known = tags::list(store);
    let folder = store
        .config
        .folder_of(Layer::Tag)
        .unwrap_or(TAGS)
        .to_string();
    let mut made: Vec<Change> = Vec::new();
    for change in &diff.changes {
        for word in added_tags(store, change)? {
            let stem = slug(&word);
            let rel = PathBuf::from(format!("{folder}/{stem}.md"));
            let taken = diff.changes.iter().chain(&made).any(|c| c.path == rel);
            if stem.is_empty() || taken || tags::has_file(&known, &word) {
                continue;
            }
            let mut piece = Piece::empty(rel.clone());
            piece.set(key::TAG, Value::from(word.trim()));
            piece.set(key::DEFINITION, Value::from(""));
            piece.set(key::PROVISIONAL, Value::Bool(true));
            made.push(Change {
                path: rel,
                from: None,
                before: None,
                after: Some(piece.render()),
            });
        }
    }
    diff.changes.extend(made);
    Ok(())
}

/// The tags a rated piece carries after the change and not before, any case.
fn added_tags(store: &Store, change: &Change) -> Result<Vec<String>, StoreError> {
    let Some(after) = &change.after else {
        return Ok(Vec::new());
    };
    let rel = slashed(&change.path);
    if !Place::of(&rel, &store.config).is_some_and(|p| RATED.contains(&p.layer)) {
        return Ok(Vec::new());
    }
    let now = Piece::parse(&change.path, after)?.tags();
    let was = match &change.before {
        Some(before) => Piece::parse(change.from.as_ref().unwrap_or(&change.path), before)?.tags(),
        None => Vec::new(),
    };
    let had = |t: &String| was.iter().any(|w| w.to_lowercase() == t.to_lowercase());
    Ok(now.into_iter().filter(|t| !had(t)).collect())
}

/// Every path a diff writes or removes, forward slashes, once each.
fn touched(diff: &Diff) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for change in &diff.changes {
        for path in std::iter::once(&change.path).chain(&change.from) {
            let rel = slashed(path);
            if !out.contains(&rel) {
                out.push(rel);
            }
        }
    }
    out
}

/// The touched files' pending state committed first in each repository, by its own identity, so
/// an undo restores it.
fn commit_base(groups: &[(Home, Vec<String>)], diff: &Diff, kind: Kind) -> Result<(), StoreError> {
    let target = diff.action.target();
    let subject = format!("files as they were, before Lorisse's {kind}({target})");
    for (home, paths) in groups {
        let dirty = git::dirty(&home.dir, paths)?;
        if !dirty.is_empty() {
            git::commit(&home.dir, &dirty, &[&subject], home.owner())?;
        }
    }
    Ok(())
}

/// The commit holding the turn's cost: Lorisse's latest over every repository, or the one its
/// own cost line names.
fn cost_home(store: &Store) -> Option<String> {
    let mut latest: Option<(DateTime<FixedOffset>, String)> = None;
    for home in repos::homes(store) {
        let Ok(log) = git::lorisse_log(&home.dir, &["-1", "--format=%H%x1f%aI%x1f%B"]) else {
            continue;
        };
        let mut parts = log.trim().splitn(3, FIELD);
        let (Some(id), Some(date)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Ok(date) = DateTime::parse_from_rfc3339(date.trim()) else {
            continue;
        };
        if latest.as_ref().is_none_or(|(d, _)| date > *d) {
            latest = Some((date, named_cost(id, parts.next().unwrap_or(""))));
        }
    }
    latest.map(|(_, id)| id)
}

/// The commit a body's cost line names (`cost: in <id>`), else this one's short id.
fn named_cost(id: &str, body: &str) -> String {
    let named = body.lines().find_map(|l| {
        let (_, cost) = l.split_once("  cost: ")?;
        cost.trim()
            .strip_prefix("in ")
            .map(|h| h.trim().to_string())
    });
    named.unwrap_or_else(|| id.trim().chars().take(7).collect())
}

/// The commit message: `<action>(<ids>): <why>`, then job, certainty, source, model and cost
/// (the turn's in USD, or `in <commit>` when `home` carries it).
fn message(kind: Kind, ids: &[PieceId], context: &Context, home: Option<&str>) -> (String, String) {
    let ids: Vec<String> = ids.iter().map(PieceId::to_string).collect();
    let why = one_line(&context.why);
    let why = if why.is_empty() {
        kind.word().to_string()
    } else {
        why
    };
    let subject = format!("{kind}({}): {why}", ids.join(", "));
    let or_dash = |s: &str| {
        if s.trim().is_empty() {
            "-".to_string()
        } else {
            one_line(s)
        }
    };
    let source = context.capture.as_ref().map(PieceId::to_string);
    let cost = match home {
        Some(home) => format!("in {home}"),
        None => format!("{:.4} USD", context.cost_usd),
    };
    let body = format!(
        "job: {}\ncertainty: {}\nsource: {}\nmodel: {}  cost: {cost}",
        or_dash(&context.job),
        certainty_word(context.certainty),
        source.unwrap_or_else(|| "-".into()),
        or_dash(&context.model),
    );
    (subject, body)
}

/// A subject `<action>(<ids>): <why>` read back; `None` when it is not one action.
fn parse_subject(subject: &str) -> Option<(Kind, Vec<String>, String)> {
    let (word, rest) = subject.split_once('(')?;
    let kind = Kind::from_word(word)?;
    let (inside, why) = rest.split_once("): ")?;
    let ids: Vec<String> = inside
        .split(", ")
        .map(str::trim)
        .map(String::from)
        .collect();
    if ids.iter().any(|i| PieceId::parse(i).is_none()) {
        return None;
    }
    Some((kind, ids, why.to_string()))
}

/// Revert one of Lorisse's commits; refused when a later commit or a pending change touches its files.
pub(crate) fn undo(store: &mut Store, commit: &str) -> Result<String, StoreError> {
    let homes = repos::homes(store);
    if homes.is_empty() {
        return Err(StoreError::Invalid(
            "undo needs the data folder to be a git repository".into(),
        ));
    }
    let found = homes
        .iter()
        .find_map(|h| git::resolve(&h.dir, commit).ok().map(|full| (h, full)));
    let (home, full) =
        found.ok_or_else(|| StoreError::Invalid(format!("no such commit: {commit}")))?;
    let root = home.dir.clone();
    let short: String = full.chars().take(7).collect();
    if git::author_mail(&root, &full)? != LORISSE_EMAIL {
        return Err(StoreError::Invalid(format!(
            "{short} is not a commit of Lorisse's: undo takes hers only"
        )));
    }
    if git::reverted(&root)?
        .iter()
        .any(|r| full.starts_with(r.as_str()))
    {
        return Err(StoreError::Invalid(format!("{short} is undone already")));
    }
    let files = git::files_of(&root, &full)?;
    refuse_later(&root, &full, &files)?;
    let id = git::revert(&root, &full, home.owner())?;
    store.rebuild_index()?;
    Ok(id)
}

/// Refuse an undo when a file of the commit changed since, committed, pending or staged; a
/// later commit undone and its revert cancel out.
pub(crate) fn refuse_later(root: &Path, full: &str, files: &[String]) -> Result<(), StoreError> {
    let pairs = git::reverts(root)?;
    let cancelled = |id: &String| {
        let paired =
            |(revert, gone): &(String, String)| revert == id || id.starts_with(gone.as_str());
        pairs.iter().any(paired)
    };
    for file in files {
        let later = git::later_on(root, full, file)?;
        if let Some(later) = later.iter().find(|id| !cancelled(id)) {
            let short: String = later.chars().take(7).collect();
            return Err(StoreError::Invalid(format!(
                "{file} changed in a later commit ({short}): undo that one first, or edit it by hand"
            )));
        }
    }
    if let Some(file) = git::dirty(root, files)?.first() {
        return Err(StoreError::Invalid(format!(
            "{file} has changes not committed: commit or drop them first"
        )));
    }
    if !git::staged(root)?.is_empty() {
        return Err(StoreError::Invalid(
            "files are staged in the data folder: commit them first".into(),
        ));
    }
    Ok(())
}

/// Lorisse's commits since a commit or a day over every repository, newest first, each read
/// from its message.
pub(crate) fn commits(store: &Store, since: &Since) -> Result<Vec<LorisseCommit>, StoreError> {
    let homes = repos::homes(store);
    let anchor = match since {
        Since::Commit(id) => Some(anchor_of(&homes, id)?),
        _ => None,
    };
    let mut out = Vec::new();
    for home in &homes {
        let bound = match (since, &anchor) {
            (Since::Date(day), _) => Some(format!("--since={day} 00:00:00")),
            (_, Some((dir, full, _))) if *dir == home.dir => Some(format!("{full}..HEAD")),
            _ => None,
        };
        let mut extra = vec!["--format=%H%x1f%aI%x1f%B%x1e"];
        extra.extend(bound.as_deref());
        let log = git::lorisse_log(&home.dir, &extra)?;
        let undone = git::reverted(&home.dir)?;
        let records = log.split(RECORD).map(|r| r.trim_start_matches('\n'));
        let records = records.filter(|r| !r.trim().is_empty());
        let after = |c: &LorisseCommit| match &anchor {
            Some((dir, _, date)) if *dir != home.dir => c.date > *date,
            _ => true,
        };
        out.extend(
            records
                .filter_map(|r| read_commit(r, &undone))
                .filter(after),
        );
    }
    out.sort_by_key(|c| std::cmp::Reverse(c.date));
    Ok(out)
}

/// The repository holding a commit, its full id and its date.
fn anchor_of(
    homes: &[Home],
    id: &str,
) -> Result<(PathBuf, String, DateTime<FixedOffset>), StoreError> {
    for home in homes {
        if let Ok(full) = git::resolve(&home.dir, id) {
            let date = git::date_of(&home.dir, &full)?;
            let date = DateTime::parse_from_rfc3339(&date)
                .map_err(|e| StoreError::Git(format!("git log: {e}")))?;
            return Ok((home.dir.clone(), full, date));
        }
    }
    Err(StoreError::Invalid(format!("no such commit: {id}")))
}

/// One record `id \x1f date \x1f message`; `None` when its date does not parse.
fn read_commit(record: &str, undone: &[String]) -> Option<LorisseCommit> {
    let mut parts = record.splitn(3, FIELD);
    let id = parts.next()?.trim().to_string();
    let date = DateTime::parse_from_rfc3339(parts.next()?.trim()).ok()?;
    let message = parts.next().unwrap_or("");
    let mut lines = message.lines();
    let subject = lines.next().unwrap_or("").trim();
    let (action, ids, why) = match parse_subject(subject) {
        Some((kind, ids, why)) => (kind.word().to_string(), ids, why),
        None => (String::new(), Vec::new(), subject.to_string()),
    };
    let mut out = LorisseCommit {
        undone: undone.iter().any(|u| id.starts_with(u.as_str())),
        id,
        date,
        action,
        ids,
        why,
        job: String::new(),
        certainty: String::new(),
        source: String::new(),
        model: String::new(),
        cost: String::new(),
        cost_in: None,
    };
    for line in lines {
        read_body_line(&mut out, line.trim());
    }
    Some(out)
}

fn read_body_line(out: &mut LorisseCommit, line: &str) {
    let Some((name, value)) = line.split_once(": ") else {
        return;
    };
    let value = value.trim().to_string();
    match name {
        "job" => out.job = value,
        "certainty" => out.certainty = value,
        "source" => out.source = value,
        "model" => match value.split_once("  cost: ") {
            Some((model, cost)) => {
                out.model = model.trim().to_string();
                match cost.trim().strip_prefix("in ") {
                    Some(home) => out.cost_in = Some(home.trim().to_string()),
                    None => out.cost = cost.trim().to_string(),
                }
            }
            None => out.model = value,
        },
        _ => {}
    }
}

/// The pieces Lorisse flagged, with her question.
pub(crate) fn to_check(store: &Store) -> Result<Vec<(PieceId, String)>, StoreError> {
    let mut out = Vec::new();
    let rated = store
        .index
        .entries
        .iter()
        .filter(|e| RATED.contains(&e.layer));
    for entry in rated.filter(|e| !e.archived) {
        let piece = store.read(&Ref::from(entry.id.clone()))?;
        if let Some(question) = piece.text(key::TO_CHECK).filter(|q| !q.trim().is_empty()) {
            out.push((entry.id.clone(), question.to_string()));
        }
    }
    Ok(out)
}

/// The pieces Lorisse flagged, with her question and her proposed answers.
pub(crate) fn flagged(store: &Store) -> Result<Vec<Flagged>, StoreError> {
    let mut out = Vec::new();
    for (id, question) in to_check(store)? {
        let piece = store.read(&Ref::from(id.clone()))?;
        let answers = piece.list(key::CHECK_ANSWERS);
        let answers = answers.into_iter().map(|a| one_line(&a));
        let answers = answers.filter(|a| !a.is_empty()).collect();
        out.push(Flagged {
            id,
            question,
            answers,
        });
    }
    Ok(out)
}

/// The owner's Fine on a live rated piece: `confidence: validated`, her question gone, one commit in his name.
pub(crate) fn fine(store: &mut Store, id: &PieceId) -> Result<Fined, StoreError> {
    let entry = store
        .index
        .get(id)
        .ok_or_else(|| StoreError::Unknown(id.to_string()))?;
    if !RATED.contains(&entry.layer) || entry.archived {
        return Err(StoreError::Invalid(format!(
            "{id}: Fine takes a live memory, task or person"
        )));
    }
    let mut set = crate::actions::Fields::new();
    set.insert(Value::from(key::CONFIDENCE), Value::from(VALIDATED));
    let unset = [key::TO_CHECK, key::CHECK_ANSWERS]
        .map(String::from)
        .to_vec();
    let action = Action::Edit {
        id: id.clone(),
        set,
        unset,
        body: None,
    };
    let diff = crate::actions::propose(store, action)?;
    let paths = touched(&diff);
    let written = diff::accept(store, &diff)?;
    store.rebuild_index()?;
    let id = written.into_iter().next().unwrap_or_else(|| id.clone());
    let groups = repos::split(store, &paths);
    let commit = match groups.first() {
        None => CommitState::NotMade("not a git repository: written, not committed".into()),
        Some((home, paths)) => {
            let subject = format!("fine({id}): validated by the owner");
            match git::commit(&home.dir, paths, &[&subject], home.owner()) {
                Ok(Some(made)) => CommitState::Made(made),
                Ok(None) => CommitState::NotMade("nothing to commit".into()),
                Err(e) => CommitState::NotMade(e.to_string()),
            }
        }
    };
    Ok(Fined { id, commit })
}

/// One file of a commit's patch: its path, whether it was made, the confidences it wrote.
#[derive(Debug, Default, PartialEq)]
struct Touched {
    path: String,
    created: bool,
    confidence: Vec<String>,
}

/// The check's lines on Lorisse's commits; a git failure is one finding, never an error.
pub(crate) fn findings(store: &Store, scan: &Scan) -> Vec<Finding> {
    let mut out = Vec::new();
    for home in repos::homes(store) {
        out.extend(lorisse_findings(store, scan, &home).unwrap_or_else(|e| {
            vec![Finding {
                path: None,
                id: None,
                text: format!(
                    "the commits of Lorisse could not be read ({}): {e}",
                    home.name
                ),
            }]
        }));
    }
    out
}

fn lorisse_findings(store: &Store, scan: &Scan, home: &Home) -> Result<Vec<Finding>, StoreError> {
    let extra = ["--format=%x1e%H%x1f%s", "--no-renames", "--unified=0", "-p"];
    let log = git::lorisse_log(&home.dir, &extra)?;
    let undone = git::reverted(&home.dir)?;
    let mut out = Vec::new();
    for record in log.split(RECORD).filter(|r| !r.trim().is_empty()) {
        let (head, patch) = record.split_once('\n').unwrap_or((record, ""));
        let Some((sha, subject)) = head.split_once(FIELD) else {
            continue;
        };
        if undone.iter().any(|u| sha.starts_with(u.as_str())) {
            continue;
        }
        let short: String = sha.chars().take(7).collect();
        let mut files = files_of_patch(patch);
        for file in &mut files {
            file.path = home.outer(&file.path);
        }
        out.extend(commit_findings(store, scan, &short, subject, &files));
    }
    Ok(out)
}

/// What is off in one commit of Lorisse.
fn commit_findings(
    store: &Store,
    scan: &Scan,
    short: &str,
    subject: &str,
    files: &[Touched],
) -> Vec<Finding> {
    let archive = &store.config.archive;
    let mut out = Vec::new();
    let at = |path: &str, text: String| Finding::at(path, piece_of(path, archive), text);
    match parse_subject(subject) {
        None => out.push(Finding {
            path: None,
            id: None,
            text: format!("commit {short} of Lorisse is not one action: {subject}"),
        }),
        Some((_, ids, _)) => {
            let ids: Vec<PieceId> = ids.iter().filter_map(|i| PieceId::parse(i)).collect();
            for file in files {
                if piece_of(&file.path, archive).is_some_and(|id| !ids.contains(&id)) {
                    let text = format!(
                        "commit {short} of Lorisse touches a piece its action does not name: more than one action"
                    );
                    out.push(at(&file.path, text));
                }
            }
        }
    }
    for file in files {
        for value in file.confidence.iter().filter(|v| v.as_str() != LOW) {
            let text =
                format!("written by Lorisse (commit {short}) with confidence: {value}, above low");
            out.push(at(&file.path, text));
        }
        if file.created && !has_source(scan, &file.path) {
            let text = format!("written by Lorisse (commit {short}) without its source link");
            out.push(at(&file.path, text));
        }
    }
    out
}

/// The piece a committed path belongs to: its own id, or its folder's for a file beside it.
fn piece_of(rel: &str, archive: &str) -> Option<PieceId> {
    if rel.ends_with(".md") {
        return ids::from_path(rel, archive);
    }
    let (folder, _) = rel.rsplit_once('/')?;
    ids::from_path(&format!("{folder}.md"), archive)
}

/// A rated piece still at `rel` names its capture, or is a capture itself; anything else passes.
fn has_source(scan: &Scan, rel: &str) -> bool {
    let Some((entry, piece, _)) = scan.pieces.iter().find(|(e, _, _)| e.path_text() == rel) else {
        return true;
    };
    if !RATED.contains(&entry.layer) || piece.get(key::CAPTURED).is_some() {
        return true;
    }
    let capture = piece.text(key::CAPTURE).and_then(PieceId::parse);
    capture.is_some_and(|id| scan.index.get(&id).is_some() || scan.index.hidden.contains(&id))
}

/// The files of a `--unified=0 -p` patch.
fn files_of_patch(patch: &str) -> Vec<Touched> {
    let mut out: Vec<Touched> = Vec::new();
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            let path = rest.rsplit_once(" b/").map_or(rest, |(_, b)| b);
            out.push(Touched {
                path: path.to_string(),
                ..Touched::default()
            });
        } else if let Some(last) = out.last_mut() {
            if line.starts_with("new file mode") {
                last.created = true;
            } else if let Some(value) = line.strip_prefix("+confidence:") {
                last.confidence
                    .push(value.trim().trim_matches('"').to_string());
            }
        }
    }
    out
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subject_reads_back_as_one_action_or_none() {
        let (kind, ids, why) = parse_subject("sort(M-0001, T-0002): à faire").unwrap();
        assert_eq!((kind, why.as_str()), (Kind::Sort, "à faire"));
        assert_eq!(ids, ["M-0001", "T-0002"]);
        assert_eq!(
            parse_subject("files as they were, before Lorisse's sort(M-1)"),
            None
        );
        assert_eq!(parse_subject("sort(M-1) and edit(T-2): two"), None);
        assert_eq!(parse_subject("delete(M-1): no such kind"), None);
    }

    #[test]
    fn the_message_carries_the_job_the_certainty_the_source_the_model_and_the_cost() {
        let context = Context {
            why: "un truc\nà acheter".into(),
            job: "keeper".into(),
            model: "claude-fable-5-1".into(),
            capture: PieceId::parse("M-7"),
            cost_usd: 0.01234,
            certainty: Certainty::Sure,
            ..Context::default()
        };
        let ids = [
            PieceId::parse("M-7").unwrap(),
            PieceId::parse("T-3").unwrap(),
        ];
        let (subject, body) = message(Kind::Sort, &ids, &context, None);
        assert_eq!(subject, "sort(M-0007, T-0003): un truc à acheter");
        assert_eq!(
            body,
            "job: keeper\ncertainty: sure\nsource: M-0007\nmodel: claude-fable-5-1  cost: 0.0123 USD"
        );
        let record = format!("abc1234{FIELD}2026-10-22T10:00:00+02:00{FIELD}{subject}\n\n{body}\n");
        let read = read_commit(&record, &["abc1234".into()]).unwrap();
        assert_eq!(
            (read.action.as_str(), read.job.as_str()),
            ("sort", "keeper")
        );
        assert_eq!(
            (read.model.as_str(), read.cost.as_str()),
            ("claude-fable-5-1", "0.0123 USD")
        );
        assert_eq!((read.source.as_str(), read.undone), ("M-0007", true));
        assert_eq!(read.cost_in, None);
        let (_, later) = message(Kind::Link, &ids, &context, Some("abc1234"));
        assert!(
            later.ends_with("model: claude-fable-5-1  cost: in abc1234"),
            "{later}"
        );
        let record =
            format!("def5678{FIELD}2026-10-22T10:00:00+02:00{FIELD}{subject}\n\n{later}\n");
        let read = read_commit(&record, &[]).unwrap();
        assert_eq!(
            (read.cost.as_str(), read.cost_in.as_deref()),
            ("", Some("abc1234"))
        );
    }

    #[test]
    fn a_patch_names_its_files_what_it_made_and_the_confidence_it_wrote() {
        let patch = "diff --git a/tasks/T-0001.md b/tasks/T-0001.md\nnew file mode 100644\n\
                     --- /dev/null\n+++ b/tasks/T-0001.md\n@@ -0,0 +1,3 @@\n+---\n\
                     +confidence: validated\ndiff --git a/inbox/M-0001.md b/inbox/M-0001.md\n\
                     deleted file mode 100644\n";
        let files = files_of_patch(patch);
        assert_eq!(files.len(), 2);
        assert!(files[0].created && !files[1].created);
        assert_eq!(files[0].confidence, ["validated"]);
        assert_eq!(
            piece_of("memories/M-0007/scan.pdf", "archive"),
            PieceId::parse("M-7")
        );
        assert_eq!(
            piece_of("tags/lait.md", "archive"),
            PieceId::parse("tags/lait")
        );
    }
}
