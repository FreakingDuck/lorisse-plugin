//! Reading: the memory folder found (`folder`), and the four questions on it: `where`,
//! `search`, `read`, `links`; each on the folder opened for reading at its call, memories,
//! people, tags and the journal's pages only.

pub(crate) mod folder;

use lorisse_store::{Entry, Ref, StoreError, Value};

use crate::shared::{as_number, cut, day_of, no_piece, readable, retired, root, Answer, Found, Memory};
use crate::shared::{DATA_LINE, LIMIT, LINE_LIMIT, TEXT_LIMIT, VERSION};
use folder::Folder;

/// The sentence for a piece this version does not read.
const ONLY: &str = "This version of Lorisse reads memories, people and tags only";

// =============================================================================
// The tools
// =============================================================================

/// `where`: the folder, how it was found, the version.
pub fn where_is() -> Answer {
    let version = format!("Lorisse's version: {VERSION}.");
    match folder::find() {
        Folder::At(path, found) => Answer::ok(format!(
            "Lorisse is here.\nThe memory folder: {}\nFound by: {}.\n{version}",
            folder::shown(&path),
            found.words()
        )),
        Folder::NotMemory(path) => Answer::ok(format!("{}\n{version}", folder::not_memory(&path))),
        Folder::None => Answer::ok(format!("{}\n{version}", folder::NO_FOLDER)),
    }
}

/// `search`, `read` or `links`, with its arguments' text (`None` when missing or not a text).
pub fn call(tool: &str, arg: Option<&str>) -> Answer {
    let root = match root() {
        Ok(root) => root,
        Err(answer) => return answer,
    };
    let arg = arg.map(str::trim).filter(|a| !a.is_empty());
    let Some(arg) = arg else {
        return Answer::error(expected(tool).to_string());
    };
    let arg = match tool {
        "search" => arg.to_string(),
        _ => as_number(arg),
    };
    let arg = arg.as_str();
    let memory = match Memory::open(&root) {
        Ok(memory) => memory,
        Err(said) => return Answer::error(said),
    };
    match tool {
        "search" => memory.search(arg),
        "read" => match memory.day_named(arg) {
            Some(day) => memory.day(&day),
            None => memory.read(arg),
        },
        _ => memory.links(arg),
    }
}

/// What a tool expects, said when its argument is missing or of the wrong kind.
pub fn expected(tool: &str) -> &'static str {
    match tool {
        "search" => "search needs \"text\": the words to look for, as a text.",
        "read" => {
            "read needs \"piece\": a number such as M-0002, a person's name, a tag's word or a day of the journal as YYYY-MM-DD, as a text."
        }
        _ => "links needs \"piece\": a number such as M-0002, or a person's or a tag's name, as a text.",
    }
}

// =============================================================================
// The questions, on the memory of one call
// =============================================================================

impl Memory {
    /// The answer for a reference that leads to no piece this version reads, or the entry.
    fn readable_entry(&self, text: &str) -> Result<Entry, Answer> {
        match self.locate(text) {
            Found::Piece(entry) if readable(entry.layer) => Ok(entry),
            Found::Piece(entry) => Err(Answer::ok(format!(
                "{ONLY}: {} is a {}.",
                entry.id, entry.layer
            ))),
            Found::Inbox(id) => Err(Answer::ok(format!(
                "{ONLY}: {id} is a capture still in the inbox."
            ))),
            Found::Broken(path, why) => Err(Answer::ok(format!(
                "The file {} cannot be read, its header is broken: {why}.",
                path.to_string_lossy().replace('\\', "/")
            ))),
            Found::Nothing => Err(Answer::ok(no_piece(text))),
        }
    }

    fn search(&self, text: &str) -> Answer {
        let hits = match self.store.search(text) {
            Ok(hits) => hits,
            Err(e) => return Answer::error(format!("The memory folder cannot be read: {e}.")),
        };
        let mut found: Vec<(Entry, Option<String>, Option<String>)> = Vec::new();
        for hit in hits {
            let Some(entry) = self.entry(&hit.id) else {
                continue;
            };
            if readable(entry.layer) || day_of(&entry).is_some() {
                found.push((entry, hit.line, hit.file));
            }
        }
        // The pieces that hold first, then the journal's days, then what is retired.
        found.sort_by_key(|(e, _, _)| match (retired(e), day_of(e).is_some()) {
            (true, _) => 2,
            (false, true) => 1,
            (false, false) => 0,
        });
        let total = found.len();
        let broken = self.broken();
        let mut lines = Vec::new();
        if total == 0 {
            lines.push(format!(
                "Nothing in the memories, people and tags holds \"{text}\"."
            ));
        } else {
            lines.push(DATA_LINE.to_string());
        }
        for (entry, line, file) in found.iter().take(LIMIT) {
            let mut said = match day_of(entry) {
                Some(day) => self.page_name(entry, &day),
                None => format!("{} · {}", self.label(entry), entry.title),
            };
            if retired(entry) {
                said.push_str(" (retired)");
            }
            match line {
                Some(line) => {
                    let (line, was_cut) = cut(line.trim(), LINE_LIMIT);
                    let more = if was_cut { "..." } else { "" };
                    said.push_str(&format!(" · {line}{more}"));
                }
                None if file.is_some() => {
                    let name = file.as_deref().unwrap_or_default();
                    said.push_str(&format!(" · holds the file {name}"));
                }
                None if day_of(entry).is_some() => said.push_str(" · in its day"),
                None if entry.title.to_lowercase().contains(&text.to_lowercase()) => {
                    said.push_str(" · in its title");
                }
                None => said.push_str(&format!(" · in its tags: {}", entry.tags.join(", "))),
            }
            lines.push(said);
        }
        if total > LIMIT {
            lines.push(format!("{total} found in all; the first {LIMIT} are shown."));
        }
        if !broken.is_empty() {
            let names: Vec<String> = broken
                .iter()
                .map(|(p, _)| p.to_string_lossy().replace('\\', "/"))
                .collect();
            lines.push(format!(
                "Not searched, their header cannot be read: {}.",
                names.join(", ")
            ));
        }
        Answer::ok(lines.join("\n"))
    }

    fn read(&self, text: &str) -> Answer {
        let entry = match self.readable_entry(text) {
            Ok(entry) => entry,
            Err(answer) => return answer,
        };
        let mut piece = match self.store.read(&Ref::from(entry.id.clone())) {
            Ok(piece) => piece,
            Err(_) => return Answer::ok(no_piece(&entry.id.to_string())),
        };
        let links = piece.links();
        let hidden = |l: &Ref| {
            matches!(self.store.read(l), Err(StoreError::Confidential(_)))
                && !self.store.in_inbox(l)
        };
        if links.iter().any(hidden) {
            let kept = links.iter().filter(|l| !hidden(l));
            let kept = kept.map(|l| Value::String(l.to_string())).collect();
            piece.set("links", Value::Sequence(kept));
        }
        let rendered = piece.render();
        let header = rendered.strip_suffix(piece.body()).unwrap_or(&rendered);
        let mut lines = vec![DATA_LINE.to_string()];
        let mut title = format!("{} · {}", self.label(&entry), entry.title);
        if retired(&entry) {
            title.push_str(" (retired)");
        }
        lines.push(title);
        if retired(&entry) {
            lines.extend(self.retired_lines(&piece));
        }
        if self.several {
            let repo = entry.repo.as_deref().unwrap_or(&self.root_name);
            lines.push(format!("Repository: {repo}"));
        }
        lines.push(header.trim_end().to_string());
        let files = piece.files();
        if !files.is_empty() {
            lines.push("Files it holds, each at its full path (open one with your own reader):".to_string());
            for file in &files {
                let at = folder::shown(&self.store.root().join(&file.path));
                lines.push(format!("- {}: {at}", file.name));
            }
        }
        let body = piece.body();
        let (shown, was_cut) = cut(body, TEXT_LIMIT);
        lines.push(shown.trim_end().to_string());
        if was_cut {
            let length = body.chars().count();
            lines.push(format!(
                "[The text is cut here: {TEXT_LIMIT} of its {length} characters are shown.]"
            ));
        }
        Answer::ok(lines.join("\n"))
    }

    fn links(&self, text: &str) -> Answer {
        let entry = match self.readable_entry(text) {
            Ok(entry) => entry,
            Err(answer) => return answer,
        };
        let links = self.store.links(&Ref::from(entry.id.clone()));
        let mut lines = vec![format!("{} · {}", self.label(&entry), entry.title)];
        let mut out = Vec::new();
        for link in &links.out {
            match self.leads_to(link) {
                Some(said) => out.push(format!("- {said}")),
                None => continue,
            }
        }
        match out.is_empty() {
            true => lines.push("It links to nothing.".to_string()),
            false => {
                lines.push("It links to:".to_string());
                lines.extend(out);
            }
        }
        let incoming: Vec<String> = links
            .incoming
            .iter()
            .filter_map(|id| self.entry(id))
            .map(|e| format!("- {} · {}", self.label(&e), e.title))
            .collect();
        match incoming.is_empty() {
            true => lines.push("Nothing links to it.".to_string()),
            false => {
                lines.push("Linked from:".to_string());
                lines.extend(incoming);
            }
        }
        Answer::ok(lines.join("\n"))
    }

    /// One link as said, or `None` for a confidential piece, which is never named.
    pub(crate) fn leads_to(&self, link: &Ref) -> Option<String> {
        match self.store.read(link) {
            Ok(piece) => match self.entry_at(piece.path()) {
                Some(e) => Some(format!("{} · {}", self.label(&e), e.title)),
                None => Some(format!("{link}: this link leads to no piece.")),
            },
            Err(StoreError::Confidential(_)) if self.store.in_inbox(link) => {
                let id = link.id().map(|i| i.to_string()).unwrap_or(link.to_string());
                Some(format!("{id} · a capture still in the inbox"))
            }
            Err(StoreError::Confidential(_)) => None,
            Err(_) => {
                let broken = link.id().and_then(|id| self.broken_named(&id));
                match broken {
                    Some((path, _)) => Some(format!(
                        "{link}: its file {} cannot be read.",
                        path.to_string_lossy().replace('\\', "/")
                    )),
                    None => Some(format!("{link}: this link leads to no piece.")),
                }
            }
        }
    }
}
