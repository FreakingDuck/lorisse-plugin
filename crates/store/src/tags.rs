//! Tags — the tag pieces (`tags/<slug>.md`: tag, definition, aliases) and the tags the pieces carry.
//!
//! A tag is its word; its file is named by the slug of that word, an alias counts as the word.
//! Flow: the index + the tag files → `list` → `TagInfo`; a word + a definition → `define` → one file.

use std::path::PathBuf;

use serde::Serialize;
use serde_yaml_ng::Value;

use crate::actions::slug;
use crate::error::StoreError;
use crate::frontmatter::key;
use crate::ids::PieceId;
use crate::index::write_atomic;
use crate::layer::{Layer, TAGS};
use crate::links::Ref;
use crate::piece::Piece;
use crate::store::Store;

// =============================================================================
// Interface
// =============================================================================

/// One tag: its word, its file's slug, its definition, how many pieces carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagInfo {
    /// The file stem: the slug of the word.
    pub slug: String,
    /// The word, as the tag file or the first piece spells it.
    pub name: String,
    /// The definition, when a tag file holds one.
    pub definition: Option<String>,
    /// The other spellings the tag file names.
    pub aliases: Vec<String>,
    /// The visible pieces carrying the word or an alias, the tag pieces left out.
    pub count: usize,
    /// The tag piece, when there is one.
    pub id: Option<PieceId>,
    /// Its file was made for a tag Lorisse put on a piece, and waits for a definition.
    pub provisional: bool,
}

/// The file stem of a tag word: lowercase ASCII, accents folded, dashes between words.
pub fn slug_of(word: &str) -> String {
    slug(word)
}

// =============================================================================
// Implementation
// =============================================================================

/// Every tag of a file or of a piece, the most carried first, then by slug.
pub(crate) fn list(store: &Store) -> Vec<TagInfo> {
    let mut out: Vec<TagInfo> = Vec::new();
    for entry in store.index.entries.iter().filter(|e| e.layer == Layer::Tag) {
        let piece = store.read(&Ref::from(entry.id.clone())).ok();
        let text = |k: &str| {
            piece
                .as_ref()
                .and_then(|p| p.get(k)?.as_str().map(String::from))
        };
        let stem = match &entry.id {
            PieceId::Named { stem, .. } => stem.clone(),
            other => slug(&other.to_string()),
        };
        out.push(TagInfo {
            slug: stem,
            name: entry.title.clone(),
            definition: text(key::DEFINITION).filter(|d| !d.trim().is_empty()),
            provisional: piece
                .as_ref()
                .is_some_and(|p| p.get(key::PROVISIONAL) == Some(&Value::Bool(true))),
            aliases: piece.map(|p| p.list(key::ALIASES)).unwrap_or_default(),
            count: 0,
            id: Some(entry.id.clone()),
        });
    }
    let carriers = store.index.entries.iter().filter(|e| e.layer != Layer::Tag);
    for entry in carriers {
        let mut seen: Vec<usize> = Vec::new();
        for tag in &entry.tags {
            let at = find(&out, tag).unwrap_or_else(|| {
                out.push(carried(tag));
                out.len() - 1
            });
            if !seen.contains(&at) {
                seen.push(at);
                out[at].count += 1;
            }
        }
    }
    out.sort_by(|a, b| b.count.cmp(&a.count).then(a.slug.cmp(&b.slug)));
    out
}

/// A tag no file defines, as a piece carries it.
fn carried(word: &str) -> TagInfo {
    TagInfo {
        slug: slug(word),
        name: word.trim().to_string(),
        definition: None,
        aliases: Vec::new(),
        count: 0,
        id: None,
        provisional: false,
    }
}

/// Whether a tag file defines `word`: by its word, an alias or its slug, any case.
pub(crate) fn has_file(tags: &[TagInfo], word: &str) -> bool {
    find(tags, word).is_some_and(|at| tags[at].id.is_some())
}

/// The tag `word` names: its word or an alias, any case, else its slug.
fn find(tags: &[TagInfo], word: &str) -> Option<usize> {
    let low = word.trim().to_lowercase();
    let named = |t: &TagInfo| {
        t.name.to_lowercase() == low || t.aliases.iter().any(|a| a.to_lowercase() == low)
    };
    let stem = slug(word);
    let by_word = tags.iter().position(named);
    by_word.or_else(|| tags.iter().position(|t| !stem.is_empty() && t.slug == stem))
}

/// Write `definition` into the tag file of `word`, made when absent; its id.
pub(crate) fn define(store: &Store, word: &str, definition: &str) -> Result<PieceId, StoreError> {
    let (word, definition) = (word.trim(), definition.trim());
    let stem = slug(word);
    if stem.is_empty() || definition.is_empty() {
        return Err(StoreError::Invalid(
            "a tag definition needs a tag and a text".into(),
        ));
    }
    if definition.contains('\n') {
        return Err(StoreError::Invalid("a tag definition is one line".into()));
    }
    let found = list(store).into_iter().find(|t| {
        t.id.is_some() && (t.slug == stem || t.name.to_lowercase() == word.to_lowercase())
    });
    let (rel, mut piece) = match found.and_then(|t| t.id) {
        Some(id) => {
            let piece = store.read(&Ref::from(id))?;
            (piece.path().to_path_buf(), piece)
        }
        None => new_tag(store, word, &stem)?,
    };
    piece.set(key::DEFINITION, Value::from(definition));
    piece.remove(key::PROVISIONAL);
    write_atomic(&store.root.join(&rel), &piece.render())?;
    Ok(PieceId::Named {
        folder: rel_folder(&rel),
        stem,
    })
}

/// A new tag file's path and piece; one already there refuses it.
fn new_tag(store: &Store, word: &str, stem: &str) -> Result<(PathBuf, Piece), StoreError> {
    let folder = store.config.folder_of(Layer::Tag).unwrap_or(TAGS);
    let rel = PathBuf::from(format!("{folder}/{stem}.md"));
    if store.root.join(&rel).exists() {
        return Err(StoreError::Exists(rel));
    }
    let mut piece = Piece::empty(rel.clone());
    piece.set(key::TAG, Value::from(word));
    Ok((rel, piece))
}

/// The folder a tag file sits in, for its id.
fn rel_folder(rel: &std::path::Path) -> String {
    let parent = rel.parent().and_then(|p| p.file_name());
    parent.map_or(TAGS.into(), |f| f.to_string_lossy().into_owned())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_finds_its_tag_by_word_alias_or_slug() {
        let mut bois = carried("Bois");
        bois.aliases = vec!["chauffage".into()];
        let tags = [bois, carried("résumé")];
        assert_eq!(find(&tags, "bois"), Some(0));
        assert_eq!(find(&tags, "Chauffage"), Some(0));
        assert_eq!(find(&tags, "resume"), Some(1));
        assert_eq!(find(&tags, "maison"), None);
    }
}
