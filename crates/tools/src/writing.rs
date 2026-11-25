//! The four writing tools: remember, note, link, undo. Each opens the memory folder for writing
//! at its call, without the confidential right, checks that git can commit there, and makes at
//! most one commit by Lorisse, through the store; a refusal is one sentence and writes nothing.

use std::path::Path;

use lorisse_store::{Blocked, Layer, NewMemory, PieceId, StoreError, Written};

use crate::folder;
use crate::json::Json;
use crate::tools::{self, Answer, Found, Memory};

/// What one writing call asks, its arguments read.
enum Asked {
    Remember(NewMemory),
    Note(String, String),
    Link(String, String),
    Undo(Option<String>),
}

/// `remember`, `note`, `link` or `undo`, with the call's arguments.
pub fn call(tool: &str, args: Option<&Json>) -> Answer {
    let root = match tools::root() {
        Ok(root) => root,
        Err(answer) => return answer,
    };
    let Some(asked) = read_args(tool, args) else {
        return Answer::error(expected(tool).to_string());
    };
    let mut memory = match Memory::writing(&root) {
        Ok(memory) => memory,
        Err(said) => return Answer::error(said),
    };
    if let Some(blocked) = memory.store.write_blocked() {
        return Answer::error(blocked_sentence(&blocked, &root));
    }
    match asked {
        Asked::Remember(new) => remember(&mut memory, &new),
        Asked::Note(piece, text) => note(&mut memory, &piece, &text),
        Asked::Link(from, to) => link(&mut memory, &from, &to),
        Asked::Undo(change) => undo(&mut memory, change.as_deref()),
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
        _ => Some(Asked::Undo(text_arg(args, "change").ok()?.map(str::to_string))),
    }
}

/// The one sentence for a write the store refused; nothing was written.
fn refused(error: StoreError) -> Answer {
    let said = match error {
        StoreError::Confidential(_) => {
            "Lorisse writes nothing in a confidential part of the memory folder: nothing was written.".to_string()
        }
        StoreError::Unknown(name) => {
            let none = tools::no_piece(&tools::as_number(&name));
            format!("{}, so nothing can link to it: nothing was written.", none.trim_end_matches('.'))
        }
        StoreError::Invalid(reason) | StoreError::Git(reason) => {
            format!("Nothing was written: {}.", reason.trim_end_matches('.'))
        }
        other => format!("Nothing was written: {other}."),
    };
    Answer::error(said)
}

fn short(commit: &str) -> String {
    commit.chars().take(7).collect()
}

fn remember(memory: &mut Memory, new: &NewMemory) -> Answer {
    let written = match memory.store.remember(new) {
        Ok(written) => written,
        Err(e) => return refused(e),
    };
    let label = memory.label_of(&written.id, &written.repo);
    Answer::ok(format!(
        "Filed as {label}: \"{}\", in one commit of {} ({}); undo takes it back.",
        new.title.trim(),
        written.repo,
        committed(&written)
    ))
}

fn committed(written: &Written) -> String {
    written.commit.as_deref().map(short).unwrap_or_default()
}

/// The entry a number or a name leads to among `layers`, or the answer that refuses it.
fn piece_of(memory: &Memory, text: &str, layers: &[Layer], what: &str) -> Result<PieceId, Answer> {
    let text = tools::as_number(text);
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
        Found::Nothing => Err(Answer::error(tools::no_piece(&text))),
    }
}

fn note(memory: &mut Memory, piece: &str, text: &str) -> Answer {
    let what = "a note goes on a memory or a person";
    let id = match piece_of(memory, piece, &[Layer::Memory, Layer::Person], what) {
        Ok(id) => id,
        Err(answer) => return answer,
    };
    match memory.store.note(&id, text) {
        Ok(written) => Answer::ok(format!(
            "Noted on {}: one dated line added at the end of its text, in one commit of {} ({}).",
            memory.label_of(&written.id, &written.repo),
            written.repo,
            committed(&written)
        )),
        Err(e) => refused(e),
    }
}

fn link(memory: &mut Memory, from: &str, to: &str) -> Answer {
    let layers = [Layer::Memory, Layer::Person, Layer::Tag];
    let from = match piece_of(memory, from, &layers, "a link goes from a memory, a person or a tag") {
        Ok(id) => id,
        Err(answer) => return answer,
    };
    let to_text = tools::as_number(to);
    let to = match memory.locate(&to_text) {
        Found::Piece(entry) if tools::readable(entry.layer) => entry.id,
        Found::Piece(entry) => {
            return Answer::error(format!(
                "{} is a {}: a link goes to a memory, a person or a tag; nothing was written.",
                entry.id, entry.layer
            ));
        }
        _ => return refused(StoreError::Unknown(to_text)),
    };
    let to_label = match memory.entry(&to) {
        Some(entry) => memory.label(&entry),
        None => to.to_string(),
    };
    match memory.store.link_to(&from, &to) {
        Ok(written) if written.commit.is_none() => Answer::ok(format!(
            "{} already links to {to_label}: nothing changed, and no commit was made.",
            memory.label_of(&written.id, &written.repo)
        )),
        Ok(written) => Answer::ok(format!(
            "Linked: {} now links to {to_label}, in one commit of {} ({}).",
            memory.label_of(&written.id, &written.repo),
            written.repo,
            committed(&written)
        )),
        Err(e) => refused(e),
    }
}

fn undo(memory: &mut Memory, change: Option<&str>) -> Answer {
    let change = change.map(tools::as_number);
    match memory.store.undo_change(change.as_deref()) {
        Ok(Some(undone)) => {
            let mut said = format!(
                "Taken back: \"{}\" ({}), as a new commit {}; the history keeps both.",
                undone.subject,
                short(&undone.commit),
                short(&undone.revert)
            );
            if undone.subject.starts_with("remember(") {
                said.push_str(&format!(" The number {} is not given again.", undone.ids.join(", ")));
            }
            Answer::ok(said)
        }
        Ok(None) => Answer::ok(
            "Lorisse has no change of hers left to take back in this memory folder: nothing was done.".to_string(),
        ),
        Err(e) => refused(e),
    }
}
