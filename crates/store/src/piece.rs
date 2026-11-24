//! Piece — one `.md` file: its frontmatter fields, its unknown keys, its body.
//!
//! A file in the house style renders back byte for byte; an unknown key is kept, after the
//! known ones, in its order. Flow: text → `Piece::parse` → accessors / `set` → `render`.

use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, NaiveDate};
use serde_yaml_ng::Value;

use crate::attach::{self, Attachment};
use crate::error::StoreError;
use crate::frontmatter::{self, DATE_KEYS, LIST_KEYS, ParseError, TEXT_KEYS, TIME_KEYS, key};
use crate::ids::PieceId;
use crate::layer::Layer;
use crate::links::Ref;

// =============================================================================
// Interface
// =============================================================================

/// One piece: frontmatter and body.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// The file, relative to the root.
    path: PathBuf,
    /// The known keys, in the house order.
    head: Vec<(String, Value)>,
    /// The unknown keys, in file order.
    extra: Vec<(String, Value)>,
    /// Everything after the closing fence.
    body: String,
    /// Whether the frontmatter lines end with `\r\n`.
    crlf: bool,
}

impl Piece {
    /// Parse a file's text; `path` (relative to the root) names it in errors.
    ///
    /// # Errors
    /// [`StoreError::Yaml`] when the frontmatter is not YAML, [`StoreError::Format`] naming
    /// the fence or the key that is wrong.
    pub fn parse(path: &Path, text: &str) -> Result<Piece, StoreError> {
        parse_piece(path, text)
    }

    /// The file's text in the house style.
    pub fn render(&self) -> String {
        let fields = self.head.iter().chain(&self.extra);
        frontmatter::render(fields.map(|(k, v)| (k.as_str(), v)), &self.body, self.crlf)
    }

    /// The file, relative to the root.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The `id:` field, if any.
    pub fn id(&self) -> Option<PieceId> {
        self.text(key::ID).and_then(PieceId::parse)
    }

    /// The title, else the name, else the date, else the tag, else `""`.
    pub fn title(&self) -> &str {
        let keys = [key::TITLE, key::NAME, key::DATE, key::TAG];
        keys.iter().find_map(|k| self.text(k)).unwrap_or("")
    }

    /// The `tags:` list.
    pub fn tags(&self) -> Vec<String> {
        self.list(key::TAGS)
    }

    /// The `links:` list.
    pub fn links(&self) -> Vec<Ref> {
        self.list(key::LINKS).iter().map(|l| Ref::new(l)).collect()
    }

    /// The `status:` word.
    pub fn status(&self) -> Option<&str> {
        self.text(key::STATUS)
    }

    /// The `kind:` word.
    pub fn kind(&self) -> Option<&str> {
        self.text(key::KIND)
    }

    /// The `captured:` date-time.
    pub fn captured(&self) -> Option<DateTime<FixedOffset>> {
        self.text(key::CAPTURED)
            .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
    }

    /// The `updated:` date.
    pub fn updated(&self) -> Option<NaiveDate> {
        self.text(key::UPDATED).and_then(parse_date)
    }

    /// The `files:` list.
    pub fn files(&self) -> Vec<Attachment> {
        attach::of_value(self.get(key::FILES))
    }

    /// The body, byte for byte.
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Replace the body.
    pub fn set_body(&mut self, body: String) {
        self.body = body;
    }

    /// The value of a key, known or not.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.head
            .iter()
            .chain(&self.extra)
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Set a key: a known one in its house slot, an unknown one in place or at the end.
    pub fn set(&mut self, key: &str, value: Value) {
        set_field(self, key, value);
    }

    /// Remove a key; its old value.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        remove_field(self, key)
    }

    /// Every key, known ones first.
    pub fn keys(&self) -> Vec<&str> {
        self.head
            .iter()
            .chain(&self.extra)
            .map(|(k, _)| k.as_str())
            .collect()
    }
}

// =============================================================================
// Implementation
// =============================================================================

impl Piece {
    /// An empty piece at `path`, LF line endings.
    pub(crate) fn empty(path: PathBuf) -> Piece {
        Piece {
            path,
            head: Vec::new(),
            extra: Vec::new(),
            body: String::new(),
            crlf: false,
        }
    }

    /// The same piece at another path.
    pub(crate) fn moved(mut self, path: PathBuf) -> Piece {
        self.path = path;
        self
    }

    /// A text field.
    pub(crate) fn text(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// A list field as text; empty when absent or null.
    pub(crate) fn list(&self, key: &str) -> Vec<String> {
        match self.get(key) {
            Some(Value::Sequence(items)) => items.iter().filter_map(scalar_text).collect(),
            _ => Vec::new(),
        }
    }

    /// Whether the piece is a task, by its id's prefix.
    fn is_task(&self) -> bool {
        let prefix = self.id().and_then(|id| id.prefix().map(str::to_string));
        prefix.as_deref().and_then(Layer::from_prefix) == Some(Layer::Task)
    }

    /// Put the known keys back in the house order.
    fn reorder(&mut self) {
        let task = self.is_task();
        self.head.sort_by_key(|(k, _)| frontmatter::rank(k, task));
    }
}

/// The text of a scalar list item.
pub(crate) fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// A `YYYY-MM-DD` date.
pub(crate) fn parse_date(text: &str) -> Option<NaiveDate> {
    if text.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

fn parse_piece(path: &Path, text: &str) -> Result<Piece, StoreError> {
    let format = |reason: String| StoreError::Format {
        path: path.to_path_buf(),
        reason,
    };
    let split = frontmatter::split(text).map_err(format)?;
    let fields = frontmatter::parse_fields(split.yaml).map_err(|e| match e {
        ParseError::Yaml(reason) => StoreError::Yaml {
            path: path.to_path_buf(),
            reason,
        },
        ParseError::Format(reason) => format(reason),
    })?;
    let (head, extra) = fields
        .into_iter()
        .partition(|(k, _)| frontmatter::is_known(k));
    let mut piece = Piece {
        path: path.to_path_buf(),
        head,
        extra,
        body: split.body.to_string(),
        crlf: split.crlf,
    };
    for (key, value) in &piece.head {
        check_value(key, value).map_err(|reason| format(format!("{key}: {reason}")))?;
    }
    piece.reorder();
    Ok(piece)
}

/// Why a known key's value is not of its shape.
fn check_value(key: &str, value: &Value) -> Result<(), String> {
    let text = value.as_str();
    let ok = if key == key::ID {
        text.and_then(PieceId::parse).is_some()
    } else if TEXT_KEYS.contains(&key) {
        text.is_some()
    } else if DATE_KEYS.contains(&key) {
        value.is_null() || text.and_then(parse_date).is_some()
    } else if TIME_KEYS.contains(&key) {
        value.is_null() || text.is_some_and(|t| DateTime::parse_from_rfc3339(t).is_ok())
    } else if LIST_KEYS.contains(&key) {
        list_of_scalars(value)
    } else if key == key::FILES {
        attach::is_file_list(value)
    } else {
        true
    };
    match ok {
        true => Ok(()),
        false => Err(format!("not {}", expected(key))),
    }
}

fn list_of_scalars(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Sequence(items) => items.iter().all(|i| scalar_text(i).is_some()),
        _ => false,
    }
}

/// What a known key holds, for a message.
fn expected(key: &str) -> &'static str {
    if key == key::ID {
        "an id"
    } else if TEXT_KEYS.contains(&key) {
        "text"
    } else if DATE_KEYS.contains(&key) {
        "a YYYY-MM-DD date"
    } else if TIME_KEYS.contains(&key) {
        "an RFC 3339 date-time with offset"
    } else if key == key::FILES {
        "a list of files, each with a name"
    } else {
        "a list"
    }
}

fn set_field(piece: &mut Piece, key: &str, value: Value) {
    let known = frontmatter::is_known(key);
    let fields = if known {
        &mut piece.head
    } else {
        &mut piece.extra
    };
    match fields.iter_mut().find(|(k, _)| k == key) {
        Some((_, v)) => *v = value,
        None => fields.push((key.to_string(), value)),
    }
    if known {
        piece.reorder();
    }
}

fn remove_field(piece: &mut Piece, key: &str) -> Option<Value> {
    for fields in [&mut piece.head, &mut piece.extra] {
        if let Some(at) = fields.iter().position(|(k, _)| k == key) {
            return Some(fields.remove(at).1);
        }
    }
    None
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const TASK: &str = "---\nid: T-0001\ntitle: A task\nstatus: todo\npriority: low\n\
                        due: null\ncreated: 2026-10-14\nupdated: 2026-10-14\nlinks: [M-0001]\n\
                        ---\n## Context\n";

    fn parse(text: &str) -> Result<Piece, StoreError> {
        Piece::parse(Path::new("tasks/T-0001.md"), text)
    }

    #[test]
    fn a_task_in_the_house_style_renders_back_byte_for_byte() {
        let piece = parse(TASK).unwrap();
        assert_eq!(piece.render(), TASK);
        assert_eq!(piece.links(), [Ref::new("M-0001")]);
        let crlf = TASK.replace('\n', "\r\n");
        assert_eq!(parse(&crlf).unwrap().render(), crlf);
    }

    #[test]
    fn a_set_key_lands_in_its_slot_and_an_unknown_one_at_the_end() {
        let mut piece = parse(TASK).unwrap();
        piece.set("mood", Value::from("calm"));
        piece.set(key::OWNER, Value::from("Name"));
        let text = piece.render();
        assert!(
            text.contains("priority: low\nowner: Name\ndue: null\n"),
            "{text}"
        );
        assert!(
            text.contains("links: [M-0001]\nmood: calm\n---\n"),
            "{text}"
        );
        assert_eq!(piece.remove("mood"), Some(Value::from("calm")));
        assert_eq!(piece.keys().last(), Some(&key::LINKS));
    }

    #[test]
    fn a_bad_value_names_its_key() {
        let err = parse(&TASK.replace("2026-10-14\nlinks", "2026-13-40\nlinks")).unwrap_err();
        assert!(
            err.to_string().contains("updated: not a YYYY-MM-DD date"),
            "{err}"
        );
        let err = parse(&TASK.replace("id: T-0001", "id: nope")).unwrap_err();
        assert!(err.to_string().contains("id: not an id"), "{err}");
        let err = parse(&TASK.replace("links: [M-0001]", "links: {a: 1}")).unwrap_err();
        assert!(err.to_string().contains("links"), "{err}");
        let err = parse("---\ntitle: [unclosed\n---\n").unwrap_err();
        assert!(matches!(err, StoreError::Yaml { .. }), "{err}");
    }
}
