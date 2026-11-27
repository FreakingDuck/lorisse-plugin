//! Keeping the memory true, and the journal: `change` corrects a memory or a person, `retire`
//! puts a memory aside with its reason, `journal` writes a day's page; and what `read`,
//! `search` and `plop` say of a retired memory and of the journal's pages.

use std::fs;

use lorisse_store::{key, Entry, Layer, Piece, Ref, Revision, Value};

use crate::first_day;
use crate::shared::write::{piece_of, refused, short};
use crate::shared::{cut, day_of, Answer, Memory, DATA_LINE, TEXT_LIMIT};

/// What `change` says when the call names nothing to change.
pub const NOTHING_NAMED: &str = "change needs at least one of \"text\", \"title\", \"kind\" or \"tags\", the parts to replace: nothing was written.";

/// The parts of a piece a change names, in words: `its text and its tags`.
fn parts(fields: &[String]) -> String {
    let said: Vec<String> = fields.iter().map(|f| format!("its {f}")).collect();
    match said.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

pub(crate) fn change(memory: &mut Memory, piece: &str, revision: &Revision) -> Answer {
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

pub(crate) fn retire(memory: &mut Memory, piece: &str, reason: &str, by: Option<&str>) -> Answer {
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

pub(crate) fn journal(memory: &mut Memory, text: &str, day: Option<&str>, repo: Option<&str>) -> Answer {
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

impl Memory {
    /// What `read` says of a retired memory: why, and the memory that takes its place.
    pub(crate) fn retired_lines(&self, piece: &Piece) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(why) = piece.get(key::RETIRED_WHY).and_then(Value::as_str) {
            lines.push(format!("Retired, it no longer holds: {why}"));
        }
        let taker = piece.get(key::RETIRED_BY).and_then(Value::as_str).map(|t| Ref::new(t));
        if let Some(said) = taker.and_then(|t| self.leads_to(&t)) {
            lines.push(format!("Its place is taken by {said}"));
        }
        lines
    }

    /// The journal's pages Claude sees, each with its day.
    fn pages(&self) -> Vec<(&Entry, String)> {
        let entries = self.entries().into_iter();
        entries.filter_map(|e| day_of(e).map(|d| (e, d))).collect()
    }

    /// The last day the journal was written, by its pages' days.
    pub(crate) fn last_day(&self) -> Option<String> {
        self.pages().into_iter().map(|(_, d)| d).max()
    }

    /// The day a `read` names: `YYYY-MM-DD`, or the page's own name `journal/YYYY-MM-DD`.
    pub(crate) fn day_named(&self, text: &str) -> Option<String> {
        let folder = self.store.config().folder_of(Layer::Diary).unwrap_or("journal");
        let day = text.strip_prefix(&format!("{folder}/")).unwrap_or(text);
        first_day::is_date(day).then(|| day.to_string())
    }

    /// `read` of a day: its pages, each said with its repository when several are seen.
    pub(crate) fn day(&self, day: &str) -> Answer {
        let pages: Vec<&Entry> = self.pages().into_iter().filter(|(_, d)| d == day).map(|(e, _)| e).collect();
        if pages.is_empty() {
            return Answer::ok(format!("The journal has no page for {day}."));
        }
        let mut lines = vec![DATA_LINE.to_string()];
        for entry in pages {
            let Ok(text) = fs::read_to_string(self.store.root().join(&entry.path)) else {
                continue;
            };
            let Ok(piece) = Piece::parse(&entry.path, &text) else {
                continue;
            };
            lines.push(self.page_name(entry, day));
            let body = piece.body();
            let (shown, was_cut) = cut(body, TEXT_LIMIT);
            lines.push(shown.trim_end().to_string());
            if was_cut {
                let length = body.chars().count();
                lines.push(format!("[The text is cut here: {TEXT_LIMIT} of its {length} characters are shown.]"));
            }
        }
        Answer::ok(lines.join("\n"))
    }

    /// A journal page as every answer says it: `the journal of <day>`, with its repository.
    pub(crate) fn page_name(&self, entry: &Entry, day: &str) -> String {
        match self.several {
            true => format!("The journal of {day} [{}]", entry.repo.as_deref().unwrap_or(&self.root_name)),
            false => format!("The journal of {day}"),
        }
    }
}
