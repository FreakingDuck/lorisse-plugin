//! The nine writing tools: remember, note, link, attach, import, change, retire, journal, undo.
//! Each opens the memory folder for writing at its call, without the confidential right; under
//! git it checks that git can commit there and makes at most one commit by Lorisse, through the
//! store; not under git it writes the files alone. A refusal is one sentence and writes nothing.

use std::path::Path;

use lorisse_store::{Blocked, Layer, NewMemory, PieceId, Revision, StoreError, Written};

use crate::first_day;
use crate::folder;
use crate::json::Json;
use crate::tools::{self, Answer, Found, Memory};

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
    Undo(Option<String>),
}

/// What `change` says when the call names nothing to change.
pub const NOTHING_NAMED: &str = "change needs at least one of \"text\", \"title\", \"kind\" or \"tags\", the parts to replace: nothing was written.";

/// What `undo` says in a folder not under git.
pub const NO_UNDO: &str = "This memory folder keeps no history, it is not under git, so there is no change to go back through: to have one, install git and run \"git init\" in the folder, then make a first commit; nothing was done.";

/// `remember`, `note`, `link`, `attach`, `import` or `undo`, with the call's arguments.
pub fn call(tool: &str, args: Option<&Json>) -> Answer {
    let root = match tools::root() {
        Ok(root) => root,
        Err(answer) => return answer,
    };
    let Some(asked) = read_args(tool, args) else {
        return Answer::error(expected(tool).to_string());
    };
    if let Asked::Change(_, revision) = &asked {
        if revision.is_empty() {
            return Answer::error(NOTHING_NAMED.to_string());
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
        Asked::Remember(new) => remember(&mut memory, &new),
        Asked::Note(piece, text) => note(&mut memory, &piece, &text),
        Asked::Link(from, to) => link(&mut memory, &from, &to),
        Asked::Attach(piece, path) => attach(&mut memory, &root, &piece, &path),
        Asked::Import(text) => first_day::import(&mut memory, &text),
        Asked::Change(piece, revision) => change(&mut memory, &piece, &revision),
        Asked::Retire(piece, reason, by) => retire(&mut memory, &piece, &reason, by.as_deref()),
        Asked::Journal(text, day, repo) => journal(&mut memory, &text, day.as_deref(), repo.as_deref()),
        Asked::Undo(_) if !memory.store.is_git() => Answer::ok(NO_UNDO.to_string()),
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
    let title = new.title.trim();
    match &written.commit {
        Some(commit) => Answer::ok(format!(
            "Filed as {label}: \"{title}\", in one commit of {} ({}); undo takes it back.",
            written.repo,
            short(commit)
        )),
        None => Answer::ok(format!("Filed as {label}: \"{title}\".")),
    }
}

/// `, in one commit of <repo> (<id>)` under git; nothing without it.
fn in_commit(written: &Written) -> String {
    match &written.commit {
        Some(commit) => format!(", in one commit of {} ({})", written.repo, short(commit)),
        None => String::new(),
    }
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
            "Noted on {}: one dated line added at the end of its text{}.",
            memory.label_of(&written.id, &written.repo),
            in_commit(&written)
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
        Ok(written) if !written.changed && memory.store.is_git() => Answer::ok(format!(
            "{} already links to {to_label}: nothing changed, and no commit was made.",
            memory.label_of(&written.id, &written.repo)
        )),
        Ok(written) if !written.changed => Answer::ok(format!(
            "{} already links to {to_label}: nothing changed.",
            memory.label_of(&written.id, &written.repo)
        )),
        Ok(written) => Answer::ok(format!(
            "Linked: {} now links to {to_label}{}.",
            memory.label_of(&written.id, &written.repo),
            in_commit(&written)
        )),
        Err(e) => refused(e),
    }
}

fn attach(memory: &mut Memory, root: &Path, piece: &str, path: &str) -> Answer {
    let what = "a file goes with a memory or a person";
    let id = match piece_of(memory, piece, &[Layer::Memory, Layer::Person], what) {
        Ok(id) => id,
        Err(answer) => return answer,
    };
    match memory.store.attach(&id, Path::new(path)) {
        Ok(attached) => {
            let label = memory.label_of(&attached.id, &attached.repo);
            let kept = folder::shown(&root.join(&attached.path));
            Answer::ok(match &attached.commit {
                Some(commit) => format!(
                    "Attached {} to {label}: the copy is kept at {kept}, in one commit of {} ({}); the original stays where it was, and undo takes it back.",
                    attached.name,
                    attached.repo,
                    short(commit)
                ),
                None => format!(
                    "Attached {} to {label}: the copy is kept at {kept}; the original stays where it was.",
                    attached.name
                ),
            })
        }
        Err(StoreError::Invalid(reason)) | Err(StoreError::Git(reason)) => Answer::error(format!(
            "Nothing was copied: {}.",
            reason.trim_end_matches('.')
        )),
        Err(e) => refused(e),
    }
}

/// The parts of a piece a change names, in words: `its text and its tags`.
fn parts(fields: &[String]) -> String {
    let said: Vec<String> = fields.iter().map(|f| format!("its {f}")).collect();
    match said.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

fn change(memory: &mut Memory, piece: &str, revision: &Revision) -> Answer {
    let what = "a change goes on a memory or a person";
    let id = match piece_of(memory, piece, &[Layer::Memory, Layer::Person], what) {
        Ok(id) => id,
        Err(answer) => return answer,
    };
    match memory.store.change(&id, revision) {
        Ok(done) if done.fields.is_empty() => Answer::ok(format!(
            "{} already says that: nothing changed, and nothing was written.",
            memory.label_of(&done.id, &done.repo)
        )),
        Ok(done) => {
            let label = memory.label_of(&done.id, &done.repo);
            let changed = parts(&done.fields);
            Answer::ok(match &done.commit {
                Some(commit) => format!(
                    "Changed {label}: {changed}, the rest as it was, in one commit of {} ({}); undo brings back what it said before.",
                    done.repo,
                    short(commit)
                ),
                None => format!("Changed {label}: {changed}, the rest as it was."),
            })
        }
        Err(e) => refused(e),
    }
}

fn retire(memory: &mut Memory, piece: &str, reason: &str, by: Option<&str>) -> Answer {
    let id = match piece_of(memory, piece, &[Layer::Memory], "only a memory is retired") {
        Ok(id) => id,
        Err(answer) => return answer,
    };
    let taker = match by {
        Some(by) => match piece_of(memory, by, &[Layer::Memory], "only a memory takes the place of another") {
            Ok(taker) => Some(taker),
            Err(answer) => return answer,
        },
        None => None,
    };
    let title = memory.entry(&id).map(|e| e.title).unwrap_or_default();
    match memory.store.retire(&id, reason, taker.as_ref()) {
        Ok(written) if !written.changed => Answer::ok(format!(
            "{} is retired already: nothing was written.",
            memory.label_of(&written.id, &written.repo)
        )),
        Ok(written) => {
            let label = memory.label_of(&written.id, &written.repo);
            let mut said = format!("Retired {label}, \"{title}\": {}", reason.trim().trim_end_matches('.'));
            if let Some(taker) = &taker {
                let taker = match memory.entry(taker) {
                    Some(entry) => memory.label(&entry),
                    None => taker.to_string(),
                };
                said.push_str(&format!("; {taker} takes its place, and the two are linked"));
            }
            said.push_str(". Nothing is deleted: read still gives it, said as retired");
            match &written.commit {
                Some(commit) => said.push_str(&format!(
                    "; in one commit of {} ({}), and undo takes it back.",
                    written.repo,
                    short(commit)
                )),
                None => said.push('.'),
            }
            Answer::ok(said)
        }
        Err(e) => refused(e),
    }
}

fn journal(memory: &mut Memory, text: &str, day: Option<&str>, repo: Option<&str>) -> Answer {
    if first_day::looks_secret(text) {
        return Answer::error(
            "Nothing was written: the text has a secret's shape (a password, a key or a card number), and Lorisse never keeps one.".to_string(),
        );
    }
    if let Some(day) = day.filter(|d| !first_day::is_date(d)) {
        return Answer::error(format!("Nothing was written: {day} is not a day; journal takes \"day\" as YYYY-MM-DD."));
    }
    match memory.store.journal(day, text, repo) {
        Ok(done) => {
            let page = match memory.several {
                true => format!("the journal of {} [{}]", done.day, done.repo),
                false => format!("the journal of {}", done.day),
            };
            let made = if done.made { ", the page made for it" } else { "" };
            Answer::ok(match &done.commit {
                Some(commit) => format!(
                    "Written in {page} at {}, one line at the end of the page{made}, in one commit of {} ({}); undo takes it back.",
                    done.time,
                    done.repo,
                    short(commit)
                ),
                None => format!("Written in {page} at {}, one line at the end of the page{made}.", done.time),
            })
        }
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
