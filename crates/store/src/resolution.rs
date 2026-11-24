//! Resolution — her answers to her question on a task, each with what a tap on it does.
//!
//! `resolutions:` holds `{text, does}`; `answers:` is derived from it as the texts alone, so a
//! reader of `answers` keeps working. A piece without `resolutions` reads its `answers` as plain ones.
//! Flow: `set.resolutions` → `checked` → `answers` derived → the file; the file → `of_piece`.

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use crate::frontmatter::key;
use crate::piece::{Piece, scalar_text};

// =============================================================================
// Interface
// =============================================================================

/// What a tap on an answer does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Does {
    /// An answer: her turn on the task with his words.
    Answer,
    /// The task dropped: archived, his answer its closing line.
    Drop,
    /// The task finished: archived.
    Archive,
}

/// One answer to her question on a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    /// The answer, as the chip shows it.
    pub text: String,
    /// What a tap on it does.
    pub does: Does,
}

/// The `does` words, in the order the spec names them.
pub const DOES: [&str; 3] = ["answer", "drop", "archive"];

impl Does {
    /// Its word: `answer`, `drop`, `archive`.
    pub fn word(self) -> &'static str {
        word_of(self)
    }

    /// The `does` a word names; `None` for another word.
    pub fn from_word(word: &str) -> Option<Does> {
        named(word)
    }
}

impl Piece {
    /// Her answers on the task: `resolutions` when set, else each of `answers` as a plain answer.
    pub fn resolutions(&self) -> Vec<Resolution> {
        of_piece(self)
    }
}

// =============================================================================
// Implementation
// =============================================================================

fn word_of(does: Does) -> &'static str {
    match does {
        Does::Answer => DOES[0],
        Does::Drop => DOES[1],
        Does::Archive => DOES[2],
    }
}

fn named(word: &str) -> Option<Does> {
    match word.trim() {
        "answer" => Some(Does::Answer),
        "drop" => Some(Does::Drop),
        "archive" => Some(Does::Archive),
        _ => None,
    }
}

fn of_piece(piece: &Piece) -> Vec<Resolution> {
    if let Some(value) = piece.get(key::RESOLUTIONS)
        && let Ok(all) = checked(value)
    {
        return all;
    }
    let plain = |text: String| Resolution {
        text,
        does: Does::Answer,
    };
    piece.list(key::ANSWERS).into_iter().map(plain).collect()
}

/// The resolutions a value holds, or why it is not a list of `{text, does}` with a known `does`.
pub(crate) fn checked(value: &Value) -> Result<Vec<Resolution>, String> {
    let items = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Sequence(items) => items,
        _ => return Err("resolutions: not a list".into()),
    };
    items.iter().map(one).collect()
}

fn one(item: &Value) -> Result<Resolution, String> {
    let map = item
        .as_mapping()
        .ok_or("resolutions: each one {text, does}")?;
    if let Some(k) = map
        .keys()
        .find(|k| !matches!(k.as_str(), Some("text" | "does")))
    {
        return Err(format!(
            "resolutions: unknown field {k:?} beside text and does"
        ));
    }
    let text = map.get("text").and_then(scalar_text).unwrap_or_default();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return Err("resolutions: an answer without text".into());
    }
    let word = map.get("does").and_then(Value::as_str).unwrap_or("answer");
    let does = named(word).ok_or_else(|| {
        format!(
            "resolutions: does `{word}` is not one of {}",
            DOES.join(", ")
        )
    })?;
    Ok(Resolution { text, does })
}

/// `set.resolutions` checked, then `answers` set to their texts on the piece.
pub(crate) fn derive(piece: &mut Piece) -> Result<(), String> {
    let Some(value) = piece.get(key::RESOLUTIONS).cloned() else {
        return Ok(());
    };
    let all = checked(&value)?;
    let texts = all.iter().map(|r| Value::from(r.text.as_str())).collect();
    piece.set(key::ANSWERS, Value::Sequence(texts));
    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn task(extra: &str) -> Piece {
        let text = format!("---\nid: T-0001\ntitle: A task\nstatus: todo\n{extra}---\n");
        Piece::parse(Path::new("tasks/T-0001.md"), &text).unwrap()
    }

    #[test]
    fn resolutions_win_over_answers_and_answers_alone_read_as_plain_answers() {
        let both = task(
            "answers: [Oui, Plus tard]\nresolutions: [{text: 'Oui, 14:00', does: answer}, {text: Abandonner, does: drop}]\n",
        );
        let got = both.resolutions();
        assert_eq!(got.len(), 2);
        assert_eq!(
            (got[0].text.as_str(), got[1].does),
            ("Oui, 14:00", Does::Drop)
        );
        let old = task("answers: [Reçus, Plus tard]\n");
        let got: Vec<(String, Does)> = old
            .resolutions()
            .into_iter()
            .map(|r| (r.text, r.does))
            .collect();
        assert_eq!(
            got,
            [
                ("Reçus".to_string(), Does::Answer),
                ("Plus tard".to_string(), Does::Answer)
            ]
        );
        assert!(task("").resolutions().is_empty());
    }

    #[test]
    fn derive_writes_answers_from_the_texts_and_refuses_an_unknown_does() {
        let mut piece =
            task("resolutions: [{text: 'Yes, 14:00'}, {text: Abandonner, does: drop}]\n");
        derive(&mut piece).unwrap();
        assert_eq!(piece.list(key::ANSWERS), ["Yes, 14:00", "Abandonner"]);
        let bad = task("resolutions: [{text: x, does: delete}]\n");
        let err = checked(bad.get(key::RESOLUTIONS).unwrap()).unwrap_err();
        assert!(
            err.contains("delete") && err.contains("answer, drop, archive"),
            "{err}"
        );
        let blank = task("resolutions: [{text: ' ', does: drop}]\n");
        assert!(checked(blank.get(key::RESOLUTIONS).unwrap()).is_err());
        assert_eq!(Does::from_word("archive").map(Does::word), Some("archive"));
    }
}
