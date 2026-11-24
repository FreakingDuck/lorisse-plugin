//! PieceId — a numbered id (`M-0001`) or a named one (`people/robin`).
//!
//! Numbered ids compare by number, so `M-10000` follows `M-9999`; the width only pads.
//! Flow: a file stem or an `id:` field → `PieceId::parse` → the index's order and `next`.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

// =============================================================================
// Interface
// =============================================================================

/// The minimum number of digits a numbered id is written with.
pub const MIN_WIDTH: usize = 4;

/// The id of a piece: numbered pieces first, by prefix and number, then named ones.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PieceId {
    /// A layer prefix and a number: `M-0001`.
    Numbered {
        /// The layer's prefix, `M` or `T`.
        prefix: String,
        /// The number, compared as a number.
        n: u64,
    },
    /// A folder and a file stem: `people/robin`, `journal/2026-10-14`.
    Named {
        /// The layer folder the file sits in.
        folder: String,
        /// The file name without `.md`.
        stem: String,
    },
}

impl PieceId {
    /// Read `M-0001`, `M-01`, `M-10000` or `people/robin`; `None` for anything else.
    pub fn parse(text: &str) -> Option<PieceId> {
        parse_id(text.trim())
    }

    /// The id written with at least `width` digits (named ids are unchanged).
    pub fn padded(&self, width: usize) -> String {
        padded_text(self, width)
    }

    /// The prefix of a numbered id.
    pub fn prefix(&self) -> Option<&str> {
        match self {
            PieceId::Numbered { prefix, .. } => Some(prefix),
            PieceId::Named { .. } => None,
        }
    }
}

// =============================================================================
// Implementation
// =============================================================================

/// The numbered id of a file stem, if it is one.
pub(crate) fn numbered(text: &str) -> Option<PieceId> {
    let (prefix, digits) = text.split_once('-')?;
    let letters = !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_uppercase());
    if !letters || digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n = digits.parse().ok()?;
    Some(PieceId::Numbered {
        prefix: prefix.to_string(),
        n,
    })
}

/// The name of a file at `rel`: its folder (an `archive` segment skipped) and its stem.
pub(crate) fn named_from_path(rel: &str, archive: &str) -> Option<PieceId> {
    let mut segments: Vec<&str> = rel.split('/').collect();
    let file = segments.pop()?;
    let stem = file.strip_suffix(".md").unwrap_or(file);
    let folder = segments.pop()?;
    let folder = if folder == archive {
        segments.pop().unwrap_or(folder)
    } else {
        folder
    };
    Some(PieceId::Named {
        folder: folder.to_string(),
        stem: stem.to_string(),
    })
}

/// The id a file name alone gives: its number, else its folder and stem.
pub(crate) fn from_path(rel: &str, archive: &str) -> Option<PieceId> {
    let file = rel.rsplit('/').next()?;
    let stem = file.strip_suffix(".md").unwrap_or(file);
    numbered(stem).or_else(|| named_from_path(rel, archive))
}

fn parse_id(text: &str) -> Option<PieceId> {
    if let Some((folder, stem)) = text.rsplit_once('/') {
        if folder.is_empty() || stem.is_empty() {
            return None;
        }
        return Some(PieceId::Named {
            folder: folder.to_string(),
            stem: stem.to_string(),
        });
    }
    numbered(text)
}

fn padded_text(id: &PieceId, width: usize) -> String {
    match id {
        PieceId::Numbered { prefix, n } => format!("{prefix}-{n:0width$}"),
        PieceId::Named { folder, stem } => format!("{folder}/{stem}"),
    }
}

impl fmt::Display for PieceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&padded_text(self, MIN_WIDTH))
    }
}

impl Serialize for PieceId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PieceId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        PieceId::parse(&text).ok_or_else(|| serde::de::Error::custom(format!("not an id: {text}")))
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_compare_as_numbers_and_the_width_only_pads() {
        let small = PieceId::parse("M-9999").unwrap();
        let big = PieceId::parse("M-10000").unwrap();
        assert!(small < big);
        assert_eq!(big.to_string(), "M-10000");
        assert_eq!(PieceId::parse("M-01").unwrap().to_string(), "M-0001");
        assert_eq!(PieceId::parse("T-7").unwrap().padded(6), "T-000007");
    }

    #[test]
    fn a_name_or_a_date_is_not_a_number() {
        assert_eq!(numbered("2026-10-14"), None);
        assert_eq!(numbered("robin"), None);
        assert_eq!(PieceId::parse("m-1"), None);
        assert_eq!(PieceId::parse("issue#12"), None);
        let named = PieceId::parse("people/robin").unwrap();
        assert_eq!(named.to_string(), "people/robin");
        assert!(PieceId::parse("M-1").unwrap() < named);
    }

    #[test]
    fn a_path_names_its_piece_past_the_archive_folder() {
        let id = named_from_path("work/people/archive/noa.md", "archive").unwrap();
        assert_eq!(id.to_string(), "people/noa");
        assert_eq!(
            from_path("archive/T-0002.md", "archive")
                .unwrap()
                .to_string(),
            "T-0002"
        );
    }
}
