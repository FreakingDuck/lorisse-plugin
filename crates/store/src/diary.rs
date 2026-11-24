//! Diary — a note for a day, appended as a timed line to that day's diary or conversation piece.
//!
//! The owner's own write, like a capture: no inbox piece, no diff; the day is made on first
//! use, found where it lies otherwise (`confidential/` included, behind the gate).
//! Flow: date + text → `append` → `<day folder>/<date>.md` → `- HH:MM text`.

use std::fmt;
use std::fs;
use std::path::PathBuf;

use chrono::{Local, NaiveDate};
use serde_yaml_ng::Value;

use crate::error::StoreError;
use crate::frontmatter::key;
use crate::ids::PieceId;
use crate::inbox::DATE_FORMAT;
use crate::index::{io_error, write_atomic};
use crate::layer::{self, Layer};
use crate::piece::Piece;
use crate::store::Store;

// =============================================================================
// Interface
// =============================================================================

/// Who speaks in a conversation line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speaker {
    /// The owner.
    You,
    /// The agent.
    Lorisse,
}

impl Speaker {
    /// The word a conversation line carries: `you` or `lorisse`.
    pub fn word(self) -> &'static str {
        word_of(self)
    }

    /// The speaker a line's word names.
    pub fn from_word(word: &str) -> Option<Speaker> {
        [Speaker::You, Speaker::Lorisse]
            .into_iter()
            .find(|s| s.word() == word)
    }
}

// =============================================================================
// Implementation
// =============================================================================

/// The format of a note's time.
const LINE_TIME: &str = "%H:%M";

fn word_of(speaker: Speaker) -> &'static str {
    match speaker {
        Speaker::You => "you",
        Speaker::Lorisse => "lorisse",
    }
}

impl fmt::Display for Speaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.word())
    }
}

/// Append `text` to the `layer` day `date`, made when absent; the day's id.
pub(crate) fn append(
    store: &Store,
    layer: Layer,
    date: NaiveDate,
    text: &str,
) -> Result<PieceId, StoreError> {
    if text.trim().is_empty() {
        return Err(StoreError::Invalid(format!("a {layer} note needs a text")));
    }
    let folder = store.config.folder_of(layer);
    let folder =
        folder.ok_or_else(|| StoreError::Config(format!("no folder holds the {layer}")))?;
    let id = PieceId::Named {
        folder: folder.to_string(),
        stem: date.to_string(),
    };
    if store.index.hidden.contains(&id) {
        return Err(StoreError::Confidential(id.to_string()));
    }
    let mut piece = day_piece(store, layer, &id, folder, date)?;
    let now = Local::now();
    let line = timed_line(&now.format(LINE_TIME).to_string(), text);
    piece.set_body(appended(piece.body(), &line));
    piece.set(
        key::UPDATED,
        Value::from(now.format(DATE_FORMAT).to_string()),
    );
    write_atomic(&store.root.join(piece.path()), &piece.render())?;
    Ok(id)
}

/// The text of a conversation line: `<who>: <text>`, leading blank lines dropped.
pub(crate) fn spoken(who: Speaker, text: &str) -> String {
    format!("{who}: {}", text.trim_start())
}

/// The day's piece as it is, or a new one when no file holds it.
fn day_piece(
    store: &Store,
    layer: Layer,
    id: &PieceId,
    folder: &str,
    date: NaiveDate,
) -> Result<Piece, StoreError> {
    if let Some(entry) = store.index.get(id) {
        let path = store.root.join(&entry.path);
        let text = fs::read_to_string(&path).map_err(|e| io_error(&path, e))?;
        return Piece::parse(&entry.path, &text);
    }
    let rel = PathBuf::from(format!("{folder}/{date}.md"));
    if store.root.join(&rel).exists() {
        return Err(StoreError::Exists(rel));
    }
    let mut piece = Piece::empty(rel);
    piece.set(key::DATE, Value::from(date.to_string()));
    if let Some(kind) = layer::default_kind(layer) {
        piece.set(key::KIND, Value::from(kind));
    }
    Ok(piece)
}

/// `- HH:MM first line`, the other non-blank lines indented under it.
fn timed_line(time: &str, text: &str) -> String {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let mut out = format!("- {time} {}", lines.next().unwrap_or(""));
    for line in lines {
        out.push_str(&format!("\n  {line}"));
    }
    out
}

/// The body with the line at its end, a blank line before it after prose.
fn appended(body: &str, line: &str) -> String {
    let nl = if body.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out = body.trim_end_matches(['\r', '\n']).to_string();
    if !out.is_empty() {
        let last = out.lines().last().unwrap_or("");
        let prose = !last.starts_with("- ") && !last.starts_with("  ");
        out.push_str(nl);
        if prose {
            out.push_str(nl);
        }
    }
    out.push_str(&line.replace('\n', nl));
    out.push_str(nl);
    out
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_is_one_timed_line_its_other_lines_indented_under_it() {
        assert_eq!(
            timed_line("09:05", " Q: x \n\n Lorisse: y"),
            "- 09:05 Q: x\n  Lorisse: y"
        );
        assert_eq!(appended("", "- 09:05 a"), "- 09:05 a\n");
        assert_eq!(appended("Prose.\n\n", "- 09:05 a"), "Prose.\n\n- 09:05 a\n");
        assert_eq!(
            appended("- 08:00 b\n  c\n", "- 09:05 a"),
            "- 08:00 b\n  c\n- 09:05 a\n"
        );
        assert_eq!(
            appended("x\r\n", "- 09:05 a\n  b"),
            "x\r\n\r\n- 09:05 a\r\n  b\r\n"
        );
    }

    #[test]
    fn a_spoken_line_names_its_speaker_and_the_word_reads_back() {
        assert_eq!(
            timed_line("10:00", &spoken(Speaker::Lorisse, "\n\nOui.\nDeux.")),
            "- 10:00 lorisse: Oui.\n  Deux."
        );
        assert_eq!(Speaker::from_word("you"), Some(Speaker::You));
        assert_eq!(Speaker::from_word("keeper"), None);
    }
}
