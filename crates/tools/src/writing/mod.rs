//! Writing: `remember` files a memory, `note` adds a dated line, `link` links two pieces, and
//! `undo` takes back a change of Lorisse's; each through the writing call of `shared::write`.

use lorisse_store::{Layer, NewMemory, StoreError};

use crate::shared::write::{in_commit, piece_of, refused, short};
use crate::shared::{self, Answer, Found, Memory};

/// What `undo` says in a folder not under git.
pub const NO_UNDO: &str = "This memory folder keeps no history, it is not under git, so there is no change to go back through: to have one, install git and run \"git init\" in the folder, then make a first commit; nothing was done.";

pub(crate) fn remember(memory: &mut Memory, new: &NewMemory) -> Answer {
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

pub(crate) fn note(memory: &mut Memory, piece: &str, text: &str) -> Answer {
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

pub(crate) fn link(memory: &mut Memory, from: &str, to: &str) -> Answer {
    let layers = [Layer::Memory, Layer::Person, Layer::Tag];
    let from = match piece_of(memory, from, &layers, "a link goes from a memory, a person or a tag") {
        Ok(id) => id,
        Err(answer) => return answer,
    };
    let to_text = shared::as_number(to);
    let to = match memory.locate(&to_text) {
        Found::Piece(entry) if shared::readable(entry.layer) => entry.id,
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

pub(crate) fn undo(memory: &mut Memory, change: Option<&str>) -> Answer {
    let change = change.map(shared::as_number);
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
