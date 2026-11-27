//! The writing call, shared by the twelve writing tools: remember, note, link, attach, import,
//! change, retire, journal, drawer, move, rule, undo; each tool is in the folder of its subject.
//! Each opens the memory folder for writing at its call, without the confidential right; under
//! git it checks that git can commit there and makes at most one commit by Lorisse, through the
//! store; not under git it writes the files alone. A refusal is one sentence and writes nothing.

use std::path::Path;

use lorisse_store::{Blocked, Layer, NewMemory, PieceId, Revision, StoreError, Written};

use crate::json::Json;
use crate::reading::folder;
use crate::shared::{self, Answer, Found, Memory};
use crate::{drawers, files, first_day, keeping, opening, writing};

/// What one writing call asks, its arguments read.
enum Asked {
    Remember(NewMemory),
    Note(String, String),
    Link(String, String),
    Attach(String, String),
    Import(String),
    Change(String, Revision),
    Retire(String, String, Option<String>),
    Journal(String, Option<String>, Option<String>),
    Drawer(String, String, bool),
    Move(Vec<String>, String),
    Rule(String, Vec<String>, String),
    Undo(Option<String>),
}

/// One of the writing tools, with the call's arguments.
pub fn call(tool: &str, args: Option<&Json>) -> Answer {
    let root = match shared::root() {
        Ok(root) => root,
        Err(answer) => return answer,
    };
    let Some(asked) = read_args(tool, args) else {
        return Answer::error(expected(tool).to_string());
    };
    if let Asked::Change(_, revision) = &asked {
        if revision.is_empty() {
            return Answer::error(keeping::NOTHING_NAMED.to_string());
        }
    }
    let mut memory = match Memory::writing(&root) {
        Ok(memory) => memory,
        Err(said) => return Answer::error(said),
    };
    if let Some(blocked) = memory.store.write_blocked() {
        return Answer::error(blocked_sentence(&blocked, &root));
    }
    match asked {
        Asked::Remember(new) => writing::remember(&mut memory, &new),
        Asked::Note(piece, text) => writing::note(&mut memory, &piece, &text),
        Asked::Link(from, to) => writing::link(&mut memory, &from, &to),
        Asked::Attach(piece, path) => files::attach(&mut memory, &root, &piece, &path),
        Asked::Import(text) => first_day::import(&mut memory, &text),
        Asked::Change(piece, revision) => keeping::change(&mut memory, &piece, &revision),
        Asked::Retire(piece, reason, by) => keeping::retire(&mut memory, &piece, &reason, by.as_deref()),
        Asked::Journal(text, day, repo) => keeping::journal(&mut memory, &text, day.as_deref(), repo.as_deref()),
        Asked::Drawer(name, about, apart) => drawers::drawer(&mut memory, &name, &about, apart),
        Asked::Move(pieces, repo) => drawers::move_to(&mut memory, &pieces, &repo),
        Asked::Rule(service, tools, list) => opening::rule(&mut memory, &root, &service, &tools, &list),
        Asked::Undo(_) if !memory.store.is_git() => Answer::ok(writing::NO_UNDO.to_string()),
        Asked::Undo(change) => writing::undo(&mut memory, change.as_deref()),
    }
}

/// What a writing tool expects, said when an argument is missing or of the wrong kind.
pub fn expected(tool: &str) -> &'static str {
    match tool {
        "remember" => {
            "remember needs \"title\" and \"text\", each as a text; \"kind\" and \"repository\", if given, as a text, \"tags\" and \"links\" as a list of texts."
        }
        "note" => {
            "note needs \"piece\": a number such as M-0002 or a person's name, and \"text\": the line to add, each as a text."
        }
        "link" => {
            "link needs \"from\" and \"to\": each a number such as M-0002, or a person's or a tag's name, as a text."
        }
        "attach" => {
            "attach needs \"piece\": a number such as M-0002 or a person's name, and \"path\": the full path of a file on this machine, each as a text."
        }
        "import" => {
            "import needs \"text\": the lines another assistant gave, \"[YYYY-MM-DD] - text\" under their headings, as a text."
        }
        "change" => {
            "change needs \"piece\": a number such as M-0002 or a person's name, as a text; then any of \"text\", \"title\", \"kind\" as a text, and \"tags\" as a list of texts."
        }
        "retire" => {
            "retire needs \"piece\": a memory's number such as M-0002, and \"reason\": why it no longer holds, each as a text; \"replaced_by\", if given, the number of the memory that takes its place, as a text."
        }
        "journal" => {
            "journal needs \"text\": what happened or was decided, as a text; \"day\", if given, as YYYY-MM-DD, and \"repository\", if given, as a text."
        }
        "drawer" => {
            "drawer needs \"name\": the repository's name, and \"about\": one sentence on what it is for, each as a text; \"apart\", if given, as true or false."
        }
        "move" => {
            "move needs \"pieces\": the numbers of the memories to move, such as M-0002, as a list of texts, and \"repository\": the repository to put them in, by name, as a text."
        }
        "rule" => {
            "rule needs \"service\": the service the tools belong to, as a text, \"tools\": each tool as the service names it, as a list of texts, and \"list\": pass, ask or never, as a text."
        }
        _ => "undo takes \"change\", if given: the commit, or the number of the piece, whose change to take back, as a text.",
    }
}

/// The one sentence for what stops every write here.
fn blocked_sentence(blocked: &Blocked, root: &Path) -> String {
    match blocked {
        Blocked::NoGit => "Lorisse cannot write because git is not found on this machine: install git, then ask again; nothing was written.".to_string(),
        Blocked::NotUnderGit => format!(
            "Lorisse cannot write because the memory folder {} is not under git: run \"git init\" in it and make a first commit, then ask again; nothing was written.",
            folder::shown(&root.to_path_buf())
        ),
        Blocked::NoIdentity(repo) => format!(
            "Lorisse cannot write because git has no name or no address set for the commits of {repo}: set user.name and user.email with git config, then ask again; nothing was written."
        ),
    }
}

/// A text argument: `Ok(None)` when absent or null, `Err` when of another kind.
fn text_arg<'a>(args: Option<&'a Json>, key: &str) -> Result<Option<&'a str>, ()> {
    match args.and_then(|a| a.get(key)) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::Str(s)) => Ok(Some(s.trim()).filter(|s| !s.is_empty())),
        Some(_) => Err(()),
    }
}

/// A text argument that must be there.
fn needed(args: Option<&Json>, key: &str) -> Option<String> {
    text_arg(args, key).ok().flatten().map(str::to_string)
}

/// A list of texts: an array of texts, or one text split at its commas; empty when absent.
fn list_arg(args: Option<&Json>, key: &str) -> Result<Vec<String>, ()> {
    let words = |s: &str| s.split(',').map(str::trim).filter(|w| !w.is_empty()).map(String::from).collect();
    match args.and_then(|a| a.get(key)) {
        None | Some(Json::Null) => Ok(Vec::new()),
        Some(Json::Str(s)) => Ok(words(s)),
        Some(Json::Array(items)) => items
            .iter()
            .map(|i| i.as_str().map(|s| s.trim().to_string()).ok_or(()))
            .filter(|w| w.as_ref().map_or(true, |w| !w.is_empty()))
            .collect(),
        Some(_) => Err(()),
    }
}

/// A yes or no: `true` or `false`, as such or as a text; `false` when absent.
fn flag_arg(args: Option<&Json>, key: &str) -> Result<bool, ()> {
    match args.and_then(|a| a.get(key)) {
        None | Some(Json::Null) => Ok(false),
        Some(Json::Bool(b)) => Ok(*b),
        Some(Json::Str(s)) if s.trim().eq_ignore_ascii_case("true") => Ok(true),
        Some(Json::Str(s)) if s.trim().eq_ignore_ascii_case("false") || s.trim().is_empty() => Ok(false),
        Some(_) => Err(()),
    }
}

fn read_args(tool: &str, args: Option<&Json>) -> Option<Asked> {
    match tool {
        "remember" => Some(Asked::Remember(NewMemory {
            title: needed(args, "title")?,
            text: needed(args, "text")?,
            kind: text_arg(args, "kind").ok()?.map(str::to_string),
            tags: list_arg(args, "tags").ok()?,
            links: list_arg(args, "links").ok()?,
            repo: text_arg(args, "repository").ok()?.map(str::to_string),
        })),
        "note" => Some(Asked::Note(needed(args, "piece")?, needed(args, "text")?)),
        "link" => Some(Asked::Link(needed(args, "from")?, needed(args, "to")?)),
        "attach" => Some(Asked::Attach(needed(args, "piece")?, needed(args, "path")?)),
        "import" => Some(Asked::Import(needed(args, "text")?)),
        "change" => {
            let tags = match args.and_then(|a| a.get("tags")) {
                None | Some(Json::Null) => None,
                Some(_) => Some(list_arg(args, "tags").ok()?),
            };
            let text = match args.and_then(|a| a.get("text")) {
                None | Some(Json::Null) => None,
                Some(Json::Str(s)) => Some(s.to_string()),
                Some(_) => return None,
            };
            let revision = Revision {
                text,
                title: text_arg(args, "title").ok()?.map(str::to_string),
                kind: text_arg(args, "kind").ok()?.map(str::to_string),
                tags,
            };
            Some(Asked::Change(needed(args, "piece")?, revision))
        }
        "retire" => Some(Asked::Retire(
            needed(args, "piece")?,
            needed(args, "reason")?,
            text_arg(args, "replaced_by").ok()?.map(str::to_string),
        )),
        "journal" => Some(Asked::Journal(
            needed(args, "text")?,
            text_arg(args, "day").ok()?.map(str::to_string),
            text_arg(args, "repository").ok()?.map(str::to_string),
        )),
        "drawer" => Some(Asked::Drawer(needed(args, "name")?, needed(args, "about")?, flag_arg(args, "apart").ok()?)),
        "move" => {
            let mut pieces = list_arg(args, "pieces").ok()?;
            if pieces.is_empty() {
                pieces = list_arg(args, "piece").ok()?;
            }
            if pieces.is_empty() {
                return None;
            }
            Some(Asked::Move(pieces, needed(args, "repository")?))
        }
        "rule" => {
            let tools = list_arg(args, "tools").ok()?;
            if tools.is_empty() {
                return None;
            }
            Some(Asked::Rule(needed(args, "service")?, tools, text_arg(args, "list").ok()?.unwrap_or("").to_string()))
        }
        _ => Some(Asked::Undo(text_arg(args, "change").ok()?.map(str::to_string))),
    }
}

/// The one sentence for a write the store refused; nothing was written.
pub(crate) fn refused(error: StoreError) -> Answer {
    let said = match error {
        StoreError::Confidential(_) => {
            "Lorisse writes nothing in a confidential part of the memory folder: nothing was written.".to_string()
        }
        StoreError::Unknown(name) => {
            let none = shared::no_piece(&shared::as_number(&name));
            format!("{}, so nothing can link to it: nothing was written.", none.trim_end_matches('.'))
        }
        StoreError::Invalid(reason) | StoreError::Git(reason) => {
            format!("Nothing was written: {}.", reason.trim_end_matches('.'))
        }
        other => format!("Nothing was written: {other}."),
    };
    Answer::error(said)
}

pub(crate) fn short(commit: &str) -> String {
    commit.chars().take(7).collect()
}

/// `, in one commit of <repo> (<id>)` under git; nothing without it.
pub(crate) fn in_commit(written: &Written) -> String {
    match &written.commit {
        Some(commit) => format!(", in one commit of {} ({})", written.repo, short(commit)),
        None => String::new(),
    }
}

/// The entry a number or a name leads to among `layers`, or the answer that refuses it.
pub(crate) fn piece_of(memory: &Memory, text: &str, layers: &[Layer], what: &str) -> Result<PieceId, Answer> {
    let text = shared::as_number(text);
    match memory.locate(&text) {
        Found::Piece(entry) if layers.contains(&entry.layer) => Ok(entry.id),
        Found::Piece(entry) => Err(Answer::error(format!(
            "{} is a {}: {what}; nothing was written.",
            entry.id, entry.layer
        ))),
        Found::Inbox(id) => Err(Answer::error(format!(
            "{id} is a capture still in the inbox: {what}; nothing was written."
        ))),
        Found::Broken(path, why) => Err(Answer::error(format!(
            "The file {} cannot be read, its header is broken ({why}): nothing was written.",
            path.to_string_lossy().replace('\\', "/")
        ))),
        Found::Nothing => Err(Answer::error(shared::no_piece(&text))),
    }
}
