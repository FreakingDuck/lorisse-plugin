//! The drawers: `drawer` makes a new repository in the memory folder, `move` puts memories in
//! another one; each one commit in the folder's own history, which undo takes back.

use lorisse_store::Layer;

use crate::shared::write::{piece_of, refused, short};
use crate::shared::{Answer, Memory};

/// The root repository's name, the history a drawer's commit is made in.
fn root_repo(memory: &Memory) -> String {
    memory.store.repos().first().map(|r| r.name.clone()).unwrap_or_default()
}

pub(crate) fn drawer(memory: &mut Memory, name: &str, about: &str, apart: bool) -> Answer {
    if let Some(there) = memory.store.repo_named(name) {
        return Answer::error(format!(
            "The repository {there} is there already, under that name: nothing was written; file in it, or choose another name."
        ));
    }
    let made = match memory.store.drawer(name, about, apart) {
        Ok(made) => made,
        Err(e) => return refused(e),
    };
    let kept = match made.apart {
        true => ", kept apart: confidential and open to Claude, for what must never mix with the rest",
        false => "",
    };
    let mut said = format!(
        "Made the repository {}{kept}: its folder {}, its memories numbered {}-0001 on",
        made.name, made.folder, made.prefix
    );
    match &made.commit {
        Some(commit) => said.push_str(&format!(
            ", in one commit of {} ({}); undo takes it back while nothing is filed in it.",
            root_repo(memory),
            short(commit)
        )),
        None => said.push('.'),
    }
    said.push_str(&format!(" File in it with \"repository\": \"{}\".", made.name));
    Answer::ok(said)
}

/// Numbers in words: `M-0002`, `M-0002 and M-0003`, `M-0001, M-0002 and M-0003`.
fn listed(labels: &[String]) -> String {
    match labels.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

pub(crate) fn move_to(memory: &mut Memory, pieces: &[String], repo: &str) -> Answer {
    let mut ids = Vec::new();
    for piece in pieces {
        match piece_of(memory, piece, &[Layer::Memory], "only a memory is moved") {
            Ok(id) => ids.push(id),
            Err(answer) => return answer,
        }
    }
    let moved = match memory.store.move_to(&ids, repo) {
        Ok(moved) => moved,
        Err(e) => return refused(e),
    };
    let already: Vec<String> = moved.already.iter().map(|id| id.to_string()).collect();
    if moved.ids.is_empty() {
        let verb = if already.len() == 1 { "is" } else { "are" };
        return Answer::ok(format!("{} {verb} in {} already: nothing was written.", listed(&already), moved.repo));
    }
    let names: Vec<String> = moved.ids.iter().map(|id| id.to_string()).collect();
    let (each, it) = match names.len() {
        1 => ("it keeps", "puts it back"),
        _ => ("each keeps", "puts them back"),
    };
    let mut said = format!(
        "Moved {} to {}: {each} its number, its text, its links and its files",
        listed(&names),
        moved.repo
    );
    match &moved.commit {
        Some(commit) => said.push_str(&format!(
            ", in one commit of {} ({}); undo {it}.",
            root_repo(memory),
            short(commit)
        )),
        None => said.push('.'),
    }
    if !already.is_empty() {
        let verb = if already.len() == 1 { "was" } else { "were" };
        said.push_str(&format!(" {} {verb} there already.", listed(&already)));
    }
    Answer::ok(said)
}
