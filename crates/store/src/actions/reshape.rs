//! Reshape — a filed piece moved to another layer or across `confidential/`; a tag renamed everywhere.
//!
//! A move to another layer makes a new piece (the next id of its layer, or a person's slug), drops
//! the old layer's own keys and retires the old piece linking the new; every link to it turns.
//! Flow: `Move` → `move_piece` → changes + the keys dropped; `RenameTag` → `rename_tag` → changes.

use serde_yaml_ng::Value;

use super::{
    Fields, Loaded, Now, apply, create_change, edit_change, file_name, load, merged,
    move_with_files, path_in, person_body, ref_list, relinked, slug, text_list, to_archive,
};
use crate::attach;
use crate::config::status;
use crate::diff::Change;
use crate::error::StoreError;
use crate::frontmatter::key;
use crate::ids::PieceId;
use crate::inbox::next_at;
use crate::index::Entry;
use crate::layer::Layer;
use crate::links::Ref;
use crate::piece::Piece;
use crate::slice::Place;
use crate::store::Store;

// =============================================================================
// Implementation
// =============================================================================

/// Where a move sends a piece: a layer, a side of `confidential/`, the new layer's fields.
pub(super) struct Target<'a> {
    pub(super) layer: Option<Layer>,
    pub(super) confidential: Option<bool>,
    pub(super) set: &'a Fields,
}

/// A memory's own keys, dropped when it becomes a task or a person.
const MEMORY_OWN: [&str; 1] = [key::KIND];
/// A task's own keys, dropped when it becomes a memory.
const TASK_OWN: [&str; 6] = [
    key::PRIORITY,
    key::CATEGORY,
    key::OWNER,
    key::DUE,
    key::NEXT,
    key::CREATED,
];
/// The keys a new piece is given anew, never carried from the old one.
const RESET: [&str; 4] = [key::ID, key::STATUS, key::SORTED, key::SORTED_BY];

/// The changes of a move and the keys it drops.
pub(super) fn move_piece(
    store: &Store,
    id: &PieceId,
    to: &Target,
    now: &Now,
) -> Result<(Vec<Change>, Vec<String>), StoreError> {
    let loaded = load(store, id)?;
    let entry = &loaded.entry;
    if entry
        .path_text()
        .starts_with(&format!("{}/", store.config.inbox))
    {
        return Err(StoreError::Invalid(format!(
            "{id}: a raw capture is sorted, not moved"
        )));
    }
    let layer = to.layer.unwrap_or(entry.layer);
    let confidential = to.confidential.unwrap_or(entry.confidential);
    if layer == entry.layer {
        let changes = in_place(store, &loaded, confidential, to.set, now)?;
        return Ok((changes, Vec::new()));
    }
    to_layer(store, &loaded, layer, confidential, to.set, now)
}

/// Same layer: the piece and its files into or out of a confidential repository, its id kept, the fields set.
fn in_place(
    store: &Store,
    loaded: &Loaded,
    confidential: bool,
    set: &Fields,
    now: &Now,
) -> Result<Vec<Change>, StoreError> {
    let entry = &loaded.entry;
    if confidential == entry.confidential && set.is_empty() {
        return Err(StoreError::Invalid(format!(
            "{}: nothing to move",
            entry.id
        )));
    }
    let mut piece = loaded.piece.clone();
    apply(&mut piece, set)?;
    piece.set(key::UPDATED, now.date_text());
    let place = Place {
        archived: entry.archived,
        ..Place::beside(entry, entry.layer, confidential, &store.config)
    };
    let to = path_in(store, &place, &file_name(loaded))?;
    move_with_files(loaded, to, piece)
}

/// The keys of `from` a piece loses on its way to `to`; a pair the store does not move refused.
fn own_keys(entry: &Entry, to: Layer) -> Result<&'static [&'static str], StoreError> {
    match (entry.layer, to) {
        (Layer::Memory, Layer::Task | Layer::Person) => Ok(&MEMORY_OWN),
        (Layer::Task, Layer::Memory) => Ok(&TASK_OWN),
        (from, to) => Err(StoreError::Invalid(format!(
            "{}: a move takes a memory to a task or a person, or a task to a memory; not a {from} to a {to}",
            entry.id
        ))),
    }
}

/// Another layer: a new piece, the old one retired linking it, every link to the old turned.
fn to_layer(
    store: &Store,
    loaded: &Loaded,
    layer: Layer,
    confidential: bool,
    set: &Fields,
    now: &Now,
) -> Result<(Vec<Change>, Vec<String>), StoreError> {
    let entry = &loaded.entry;
    let own = own_keys(entry, layer)?;
    if entry.archived {
        return Err(StoreError::Invalid(format!(
            "{}: archived, never moved to another layer",
            entry.id
        )));
    }
    let mut piece = loaded.piece.clone();
    let dropped: Vec<String> = own
        .iter()
        .filter(|k| piece.remove(k).is_some())
        .map(|k| k.to_string())
        .collect();
    for k in RESET {
        piece.remove(k);
    }
    let place = Place::beside(entry, layer, confidential, &store.config);
    let (stem, link) = named(store, &mut piece, &place, set)?;
    match layer {
        Layer::Task => {
            let first = store.config.task_statuses.first().map(String::as_str);
            piece.set(key::STATUS, Value::from(first.unwrap_or("todo")));
            piece.set(key::CREATED, now.date_text());
        }
        Layer::Memory => piece.set(key::STATUS, Value::from(status::SORTED)),
        _ => {}
    }
    piece.set(key::UPDATED, now.date_text());
    apply(&mut piece, set)?;
    let at = path_in(store, &place, &format!("{stem}.md"))?;
    let mut created = piece.moved(at.clone());
    let files = attach::relocate(&mut created, &entry.path, &at)?;
    let new_ref = Ref::new(&link);
    let mut retired = loaded.piece.clone();
    retired.remove(key::FILES);
    retired.set(key::STATUS, Value::from(status::RETIRED));
    let links = merged(std::slice::from_ref(&new_ref), &retired.links(), &[]);
    retired.set(key::LINKS, ref_list(&links));
    retired.set(key::UPDATED, now.date_text());
    let mut out = vec![create_change(&created)];
    match entry.layer {
        Layer::Task => out.extend(to_archive(store, loaded, retired)?),
        _ => out.push(edit_change(loaded, &retired)),
    }
    out.extend(files);
    out.extend(relinked(store, loaded, loaded, &new_ref, now)?);
    Ok((out, dropped))
}

/// The new piece's id (the next of its layer in the repository it lands in) or a person's name;
/// its file stem and link text.
fn named(
    store: &Store,
    piece: &mut Piece,
    place: &Place,
    set: &Fields,
) -> Result<(String, String), StoreError> {
    let (layer, config) = (place.layer, &store.config);
    if let Some(prefix) = layer.prefix() {
        let home = place.home(config);
        let id = next_at(&store.index, config, prefix, home).padded(config.id_width);
        piece.set(key::ID, Value::from(id.as_str()));
        return Ok((id.clone(), id));
    }
    let given = set.get(key::NAME).and_then(Value::as_str);
    let name = given.unwrap_or(piece.title()).trim().to_string();
    let stem = slug(&name);
    if stem.is_empty() {
        return Err(StoreError::Invalid(format!("a {layer} needs set.name")));
    }
    piece.remove(key::TITLE);
    piece.set(key::NAME, Value::from(name.as_str()));
    let body = person_body(&store.config.person_sections, piece.body());
    piece.set_body(body);
    let folder = store.config.folder_of(layer).unwrap_or_default();
    Ok((stem.clone(), format!("{folder}/{stem}")))
}

/// Same word, any case.
fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// The tag piece of `word`: its word, an alias or its file's stem.
fn tag_file<'a>(store: &'a Store, word: &str) -> Option<&'a Entry> {
    let stem = slug(word);
    let named = |e: &&Entry| matches!(&e.id, PieceId::Named { stem: s, .. } if *s == stem);
    let tags = store.index.entries.iter().filter(|e| e.layer == Layer::Tag);
    tags.clone()
        .find(|e| same(&e.title, word))
        .or_else(|| tags.clone().find(named))
}

/// Every piece carrying `from` with `to` instead (duplicates folded), and the tag files renamed or merged.
pub(super) fn rename_tag(
    store: &Store,
    from: &str,
    to: &str,
    now: &Now,
) -> Result<Vec<Change>, StoreError> {
    let (from, to) = (from.trim(), to.trim());
    if from.is_empty() || slug(to).is_empty() {
        return Err(StoreError::Invalid("a tag rename needs from and to".into()));
    }
    if from == to {
        return Err(StoreError::Invalid(format!("{from}: the same tag")));
    }
    let mut out = Vec::new();
    let carriers = store.index.entries.iter().filter(|e| e.layer != Layer::Tag);
    for entry in carriers.filter(|e| e.tags.iter().any(|t| same(t, from))) {
        let loaded = load(store, &entry.id)?;
        let mut tags: Vec<String> = Vec::new();
        for tag in loaded.piece.tags() {
            let tag = if same(&tag, from) {
                to.to_string()
            } else {
                tag
            };
            if !tags.iter().any(|t| same(t, &tag)) {
                tags.push(tag);
            }
        }
        let mut piece = loaded.piece.clone();
        piece.set(key::TAGS, text_list(&tags));
        piece.set(key::UPDATED, now.date_text());
        out.push(edit_change(&loaded, &piece));
    }
    out.extend(tag_files(store, from, to)?);
    if out.is_empty() {
        return Err(StoreError::Invalid(format!(
            "{from}: no piece carries the tag"
        )));
    }
    Ok(out)
}

/// `from`'s tag file renamed to `to`'s, or merged into it when `to` has one: aliases and definition kept.
fn tag_files(store: &Store, from: &str, to: &str) -> Result<Vec<Change>, StoreError> {
    let Some(old) = tag_file(store, from) else {
        return Ok(Vec::new());
    };
    let old = load(store, &old.id)?;
    let old_aliases = old.piece.list(key::ALIASES);
    let with = |kept: &[String], more: &[String]| {
        let mut out: Vec<String> = Vec::new();
        for alias in kept.iter().chain(more) {
            if !same(alias, to) && !out.iter().any(|a| same(a, alias)) {
                out.push(alias.clone());
            }
        }
        out
    };
    let mut more = vec![from.to_string()];
    more.extend(old_aliases);
    match tag_file(store, to).filter(|e| e.id != old.entry.id) {
        Some(new) => {
            let new = load(store, &new.id)?;
            let mut piece = new.piece.clone();
            let aliases = with(&new.piece.list(key::ALIASES), &more);
            piece.set(key::ALIASES, text_list(&aliases));
            let defined = piece.get(key::DEFINITION).is_some_and(|d| !d.is_null());
            if let (false, Some(definition)) = (defined, old.piece.get(key::DEFINITION)) {
                piece.set(key::DEFINITION, definition.clone());
            }
            let gone = Change {
                path: old.entry.path.clone(),
                from: None,
                before: Some(old.text.clone()),
                after: None,
            };
            Ok(vec![edit_change(&new, &piece), gone])
        }
        None => {
            let mut piece = old.piece.clone();
            piece.set(key::TAG, Value::from(to));
            piece.set(key::ALIASES, text_list(&with(&[], &more)));
            let conf = old.entry.confidential;
            let place = Place::beside(&old.entry, Layer::Tag, conf, &store.config);
            let at = path_in(store, &place, &format!("{}.md", slug(to)))?;
            move_with_files(&old, at, piece)
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_is_the_same_whatever_its_case_and_its_spaces() {
        assert!(same(" Bois", "bois"));
        assert!(!same("bois", "bois-de-chauffage"));
        assert_eq!(crate::layer::TAGS, "tags");
    }
}
