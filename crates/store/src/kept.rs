//! Kept — a conversation of the app stored as one memory of kind `conversation`, made once, then updated in place.
//!
//! The piece is found again by its `thread:`; a store that changes nothing writes nothing. No commit of its own.
//! Flow: `Conversation` → `keep` (found by thread: an edit's change; else a new `M-` piece) → `diff::write` → `Kept`.

use std::fs;
use std::path::PathBuf;

use chrono::{Local, NaiveDate};
use serde::Serialize;
use serde_yaml_ng::Value;

use crate::actions::{Action, Fields};
use crate::config::status;
use crate::diff::{self, Change, Diff};
use crate::error::StoreError;
use crate::frontmatter::key;
use crate::ids::PieceId;
use crate::inbox::{self, DATE_FORMAT};
use crate::index::io_error;
use crate::layer::Layer;
use crate::links::Ref;
use crate::piece::{Piece, parse_date};
use crate::slice::Place;
use crate::store::{Filter, Store};

// =============================================================================
// Interface
// =============================================================================

/// The `kind:` of a stored conversation; always allowed, whatever `config.yaml`'s kinds.
pub const CONVERSATION: &str = "conversation";

/// One conversation of the app, as the store keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    /// Its thread, `<screen>/<stem>` (`chat/2026-11-08-2`): the one piece it updates.
    pub thread: String,
    /// Its title: his first words.
    pub title: String,
    /// The day it started.
    pub date: NaiveDate,
    /// The models it heard from, in order.
    pub models: Vec<String>,
    /// The pieces it read or made.
    pub links: Vec<Ref>,
    /// The turns, as the body.
    pub body: String,
}

/// What a store did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Kept {
    /// The memory's id.
    pub id: PieceId,
    /// Its file, relative to the root, forward slashes.
    pub path: String,
    /// Made by this store (else found by its thread).
    pub made: bool,
    /// Something was written.
    pub changed: bool,
}

/// The thread a piece keeps, when it is a stored conversation.
pub fn thread_of(piece: &Piece) -> Option<&str> {
    let kind = piece.kind() == Some(CONVERSATION);
    kind.then(|| piece.get(key::THREAD).and_then(Value::as_str))
        .flatten()
}

// =============================================================================
// Implementation
// =============================================================================

/// Every visible stored conversation, in id order.
pub(crate) fn conversations(store: &Store) -> Vec<Piece> {
    let filter = Filter {
        layer: Some(Layer::Memory),
        ..Filter::default()
    };
    let read = |id: &PieceId| store.read(&Ref::from(id.clone())).ok();
    let pieces = store.list(&filter).into_iter().filter_map(|e| read(&e.id));
    pieces.filter(|p| thread_of(p).is_some()).collect()
}

/// The memory of `c.thread` made or updated; nothing written when it already says the same.
pub(crate) fn keep(store: &mut Store, c: &Conversation) -> Result<Kept, StoreError> {
    if c.thread.trim().is_empty() || c.body.trim().is_empty() {
        return Err(StoreError::Invalid(
            "a conversation needs a thread and turns".into(),
        ));
    }
    let found = conversations(store)
        .into_iter()
        .find(|p| thread_of(p) == Some(c.thread.as_str()));
    let kept = match found {
        Some(old) => update(store, old, c)?,
        None => make(store, c)?,
    };
    if kept.changed {
        store.rebuild_index()?;
    }
    Ok(kept)
}

fn today() -> Value {
    Value::from(Local::now().format(DATE_FORMAT).to_string())
}

fn texts<'a>(items: impl Iterator<Item = &'a str>) -> Value {
    Value::Sequence(items.map(Value::from).collect())
}

/// `first`, then each of `then` not yet there.
fn joined<T: PartialEq + Clone>(first: &[T], then: &[T]) -> Vec<T> {
    let mut out = first.to_vec();
    out.extend(then.iter().filter(|t| !first.contains(t)).cloned());
    out
}

/// The fields of `c` onto `piece`: title, models and links joined to what it had, the body.
fn fill(piece: &mut Piece, c: &Conversation) {
    piece.set(key::TITLE, Value::from(c.title.trim()));
    let models = piece.get(key::MODELS).and_then(Value::as_sequence);
    let had: Vec<String> = models
        .map(|s| s.iter().filter_map(Value::as_str).map(str::to_string))
        .into_iter()
        .flatten()
        .collect();
    let models = joined(&had, &c.models);
    piece.set(key::MODELS, texts(models.iter().map(String::as_str)));
    let links: Vec<Ref> = c.links.iter().map(Ref::bare).collect();
    let links = joined(&piece.links(), &links);
    piece.set(key::LINKS, texts(links.iter().map(Ref::as_str)));
    piece.set_body(format!("{}\n", c.body.trim_end()));
}

fn make(store: &Store, c: &Conversation) -> Result<Kept, StoreError> {
    let id = inbox::next_at(&store.index, &store.config, "M", None);
    let name = id.padded(store.config.id_width);
    let place = Place::new(Layer::Memory, false, None);
    let folder = place
        .folder(&store.config)
        .ok_or_else(|| StoreError::Config("no folder holds the layer memory".into()))?;
    let path = PathBuf::from(format!("{folder}/{name}.md"));
    let mut piece = Piece::empty(path.clone());
    piece.set(key::ID, Value::from(name.as_str()));
    piece.set(
        key::DATE,
        Value::from(c.date.format(DATE_FORMAT).to_string()),
    );
    piece.set(key::KIND, Value::from(CONVERSATION));
    piece.set(key::THREAD, Value::from(c.thread.trim()));
    piece.set(key::CREATED, today());
    piece.set(key::UPDATED, today());
    piece.set(key::STATUS, Value::from(status::SORTED));
    fill(&mut piece, c);
    let change = Change {
        path: path.clone(),
        from: None,
        before: None,
        after: Some(piece.render()),
    };
    written(store, &id, change)?;
    Ok(Kept {
        id,
        path: slashed(&path),
        made: true,
        changed: true,
    })
}

fn update(store: &Store, old: Piece, c: &Conversation) -> Result<Kept, StoreError> {
    let path = old.path().to_path_buf();
    let id = old
        .id()
        .ok_or_else(|| StoreError::Invalid(format!("{}: no id", slashed(&path))))?;
    let mut piece = old.clone();
    fill(&mut piece, c);
    let first = old
        .get(key::DATE)
        .and_then(Value::as_str)
        .and_then(parse_date);
    let date = first.map_or(c.date, |d| d.min(c.date));
    piece.set(key::DATE, Value::from(date.format(DATE_FORMAT).to_string()));
    if piece.render() == old.render() {
        return Ok(Kept {
            id,
            path: slashed(&path),
            made: false,
            changed: false,
        });
    }
    piece.set(key::UPDATED, today());
    let full = store.root.join(&path);
    let before = fs::read_to_string(&full).map_err(|e| io_error(&full, e))?;
    let change = Change {
        path: path.clone(),
        from: None,
        before: Some(before),
        after: Some(piece.render()),
    };
    written(store, &id, change)?;
    Ok(Kept {
        id,
        path: slashed(&path),
        made: false,
        changed: true,
    })
}

/// One change written through the diff's checks (the gate, the base, the lists).
fn written(store: &Store, id: &PieceId, change: Change) -> Result<(), StoreError> {
    let diff = Diff {
        action: Action::Edit {
            id: id.clone(),
            set: Fields::new(),
            unset: Vec::new(),
            body: None,
        },
        made_at: Local::now().fixed_offset(),
        changes: vec![change],
        dropped: Vec::new(),
    };
    diff::write(store, &diff).map(|_| ())
}

fn slashed(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Access;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Dir {
            let n = std::process::id();
            let root = std::env::temp_dir().join(format!("lorisse-store-kept-{name}-{n}"));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Dir(root)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn talk(thread: &str, body: &str, links: &[&str]) -> Conversation {
        Conversation {
            thread: thread.into(),
            title: "Le jardin cet été".into(),
            date: NaiveDate::from_ymd_opt(2026, 11, 8).unwrap(),
            models: vec!["claude-opus-5-5".into()],
            links: links.iter().map(|l| Ref::new(l)).collect(),
            body: body.into(),
        }
    }

    fn seeded(dir: &Dir) -> Store {
        fs::create_dir_all(dir.0.join("memories")).unwrap();
        let text =
            "---\nid: M-0001\ntitle: Le potager\nkind: fact\nstatus: sorted\n---\nTomates.\n";
        fs::write(dir.0.join("memories/M-0001.md"), text).unwrap();
        Store::open(&dir.0, Access::Full).unwrap()
    }

    #[test]
    fn a_first_store_makes_one_memory_of_kind_conversation_with_its_thread_day_models_links_and_turns()
     {
        let dir = Dir::new("make");
        let mut store = seeded(&dir);
        let c = talk("chat/2026-11-08", "you: le jardin ?\n\nCOUCOU", &["M-0001"]);
        let kept = store.keep_conversation(&c).unwrap();
        assert_eq!(
            (kept.id.to_string().as_str(), kept.made, kept.changed),
            ("M-0002", true, true)
        );
        assert_eq!(kept.path, "memories/M-0002.md");
        let piece = store.read(&Ref::new("M-0002")).unwrap();
        assert_eq!(piece.kind(), Some(CONVERSATION));
        assert_eq!(thread_of(&piece), Some("chat/2026-11-08"));
        assert_eq!(piece.title(), "Le jardin cet été");
        assert_eq!(piece.links(), [Ref::new("M-0001")]);
        assert_eq!(piece.body(), "you: le jardin ?\n\nCOUCOU\n");
        let text = fs::read_to_string(dir.0.join("memories/M-0002.md")).unwrap();
        for line in [
            "date: 2026-11-08",
            "status: sorted",
            "thread: chat/2026-11-08",
            "models: [claude-opus-5-5]",
        ] {
            assert!(text.contains(line), "{line}: {text}");
        }
        assert!(!dir.0.join("inbox").exists(), "never a capture");
        assert!(
            store.check().unwrap().findings.is_empty(),
            "the kind passes any list"
        );
    }

    #[test]
    fn the_same_thread_updates_its_one_piece_joins_its_links_and_models_and_a_store_of_the_same_writes_nothing()
     {
        let dir = Dir::new("update");
        let mut store = seeded(&dir);
        let first = talk("chat/2026-11-08", "you: un", &[]);
        let id = store.keep_conversation(&first).unwrap().id;
        let path = dir.0.join("memories/M-0002.md");
        let same = store.keep_conversation(&first).unwrap();
        assert_eq!(
            (same.id.clone(), same.made, same.changed),
            (id.clone(), false, false)
        );
        let mut more = talk("chat/2026-11-08", "you: un\n\nyou: deux", &["M-0001"]);
        more.models = vec!["claude-fable-5-1".into()];
        let kept = store.keep_conversation(&more).unwrap();
        assert_eq!((kept.id, kept.made, kept.changed), (id, false, true));
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("models: [claude-opus-5-5, claude-fable-5-1]"),
            "{text}"
        );
        assert!(
            text.contains("links: [M-0001]") && text.ends_with("you: deux\n"),
            "{text}"
        );
        assert_eq!(conversations(&store).len(), 1, "one piece per thread");
        let other = store.keep_conversation(&talk("memories/2026-11-08", "you: x", &[]));
        assert_eq!(other.unwrap().id.to_string(), "M-0003");
    }

    #[test]
    fn a_blank_conversation_or_thread_is_refused_and_nothing_is_written() {
        let dir = Dir::new("blank");
        let mut store = seeded(&dir);
        assert!(
            store
                .keep_conversation(&talk("chat/x", " \n", &[]))
                .is_err()
        );
        assert!(store.keep_conversation(&talk(" ", "you: a", &[])).is_err());
        assert_eq!(fs::read_dir(dir.0.join("memories")).unwrap().count(), 1);
    }
}
