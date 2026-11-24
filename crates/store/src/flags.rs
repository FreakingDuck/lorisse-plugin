//! Flags — `#tag`, `@person`, `!mark` read off a capture's text, the text left as typed.
//!
//! A flag starts a word; its name runs over letters, digits and `-_./`, trailing `.-/` cut.
//! Flow: text → `Flags::parse` → `merged` with the caller's → `tags`, `people`, `marks`.

use serde_yaml_ng::Value;

use crate::frontmatter::key;
use crate::piece::Piece;

// =============================================================================
// Interface
// =============================================================================

/// The flags of a capture, in the order first seen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Flags {
    /// `#tag`.
    pub tags: Vec<String>,
    /// `@person`.
    pub people: Vec<String>,
    /// `!mark`.
    pub marks: Vec<String>,
}

impl Flags {
    /// The flags typed in `text`, each once.
    pub fn parse(text: &str) -> Flags {
        parse_flags(text)
    }
}

// =============================================================================
// Implementation
// =============================================================================

impl Flags {
    /// These flags, then `other`'s not yet there.
    pub(crate) fn merged(&self, other: &Flags) -> Flags {
        let mut out = self.clone();
        for (to, from) in out.lists_mut().into_iter().zip(other.lists()) {
            from.iter().for_each(|f| push_once(to, f));
        }
        out
    }

    /// Set `tags` always, `people` and `marks` when not empty.
    pub(crate) fn apply(&self, piece: &mut Piece) {
        piece.set(key::TAGS, text_list(&self.tags));
        for (name, list) in [(key::PEOPLE, &self.people), (key::MARKS, &self.marks)] {
            if !list.is_empty() {
                piece.set(name, text_list(list));
            }
        }
    }

    fn lists(&self) -> [&Vec<String>; 3] {
        [&self.tags, &self.people, &self.marks]
    }

    fn lists_mut(&mut self) -> [&mut Vec<String>; 3] {
        [&mut self.tags, &mut self.people, &mut self.marks]
    }
}

fn text_list(items: &[String]) -> Value {
    Value::Sequence(items.iter().map(|t| Value::from(t.as_str())).collect())
}

fn push_once(list: &mut Vec<String>, item: &str) {
    if !list.iter().any(|i| i == item) {
        list.push(item.to_string());
    }
}

fn parse_flags(text: &str) -> Flags {
    let mut flags = Flags::default();
    let chars: Vec<char> = text.chars().collect();
    for (at, c) in chars.iter().enumerate() {
        let starts_word = at == 0 || chars[at - 1].is_whitespace();
        let list = match c {
            '#' => &mut flags.tags,
            '@' => &mut flags.people,
            '!' => &mut flags.marks,
            _ => continue,
        };
        if starts_word {
            let name = name_at(&chars[at + 1..]);
            if name.chars().any(char::is_alphanumeric) {
                push_once(list, &name);
            }
        }
    }
    flags
}

/// The flag name at the start of `rest`.
fn name_at(rest: &[char]) -> String {
    let name: String = rest
        .iter()
        .take_while(|c| c.is_alphanumeric() || "-_./".contains(**c))
        .collect();
    name.trim_end_matches(['.', '-', '/']).to_string()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flag_starts_a_word_and_loses_its_trailing_punctuation() {
        let flags = Flags::parse("#bench, @Noa. !urgent! #bench x#no a@b.c # ## !- #été/2");
        assert_eq!(flags.tags, ["bench", "été/2"]);
        assert_eq!(flags.people, ["Noa"]);
        assert_eq!(flags.marks, ["urgent"]);
        let more = Flags {
            tags: vec!["lab".into(), "bench".into()],
            ..Flags::default()
        };
        assert_eq!(flags.merged(&more).tags, ["bench", "été/2", "lab"]);
    }
}
