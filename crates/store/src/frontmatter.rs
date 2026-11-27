//! Frontmatter — the `---` fences, the YAML key names and the house rendering.
//!
//! The only place a frontmatter key is spelled. Parsing is `serde_yaml_ng`; rendering is by
//! hand: the house key order, flow lists, plain scalars quoted only when YAML needs it.
//! Flow: text → `split` → `parse_fields` → (key, value) pairs → `render` → text.

use serde_yaml_ng::Value;

// =============================================================================
// Interface
// =============================================================================

/// The frontmatter keys the store reads or writes.
pub mod key {
    /// The numbered id.
    pub const ID: &str = "id";
    /// A memory's or a task's title.
    pub const TITLE: &str = "title";
    /// A person's name.
    pub const NAME: &str = "name";
    /// A diary day's date.
    pub const DATE: &str = "date";
    /// A tag piece's word.
    pub const TAG: &str = "tag";
    /// A tag piece's definition, one line.
    pub const DEFINITION: &str = "definition";
    /// A tag piece's other spellings.
    pub const ALIASES: &str = "aliases";
    /// A memory's kind.
    pub const KIND: &str = "kind";
    /// A person's role.
    pub const ROLE: &str = "role";
    /// A person's company.
    pub const COMPANY: &str = "company";
    /// A person's contact: mail, phone, address.
    pub const CONTACT: &str = "contact";
    /// A task's priority.
    pub const PRIORITY: &str = "priority";
    /// A task's category.
    pub const CATEGORY: &str = "category";
    /// A task's owner.
    pub const OWNER: &str = "owner";
    /// A task's due date.
    pub const DUE: &str = "due";
    /// A task's next step, one line (kept after the house keys).
    pub const NEXT: &str = "next";
    /// A task's creation date.
    pub const CREATED: &str = "created";
    /// The tags.
    pub const TAGS: &str = "tags";
    /// The people named with `@`.
    pub const PEOPLE: &str = "people";
    /// The marks typed with `!`.
    pub const MARKS: &str = "marks";
    /// Where a capture came from.
    pub const SOURCE: &str = "source";
    /// The capture a piece came from.
    pub const CAPTURE: &str = "capture";
    /// When it was captured.
    pub const CAPTURED: &str = "captured";
    /// The last change, a date.
    pub const UPDATED: &str = "updated";
    /// The status word.
    pub const STATUS: &str = "status";
    /// The links.
    pub const LINKS: &str = "links";
    /// The attached files.
    pub const FILES: &str = "files";
    /// A file entry's name.
    pub const FILE_NAME: &str = "name";
    /// A file entry's path, relative to the root.
    pub const FILE_PATH: &str = "path";
    /// A file entry's kind.
    pub const FILE_KIND: &str = "kind";
    /// When the sort was accepted.
    pub const SORTED: &str = "sorted";
    /// Who proposed the sort.
    pub const SORTED_BY: &str = "sorted_by";
    /// A field the path decides (reported by the check).
    pub const SLICE: &str = "slice";
    /// A field the path decides (reported by the check).
    pub const LAYER: &str = "layer";
    /// A field the path decides (reported by the check).
    pub const CONFIDENTIAL: &str = "confidential";
    /// How far the owner checked a piece: `low`, `checked`, `validated`.
    pub const CONFIDENCE: &str = "confidence";
    /// Lorisse's question on a piece she wrote unsure ("To check").
    pub const TO_CHECK: &str = "to_check";
    /// A tag file made for a tag put on a piece, not yet defined.
    pub const PROVISIONAL: &str = "provisional";
    /// A piece's short label, two or three words, hers: the map's name of it.
    pub const LABEL: &str = "label";
    /// Her proposed answers to her `to_check` question, the likeliest first, each `text (certainty)`.
    pub const CHECK_ANSWERS: &str = "check_answers";
    /// Her question on a task, one line (the plan job's).
    pub const QUESTION: &str = "question";
    /// Its answers as text; derived from `resolutions` when that is set.
    pub const ANSWERS: &str = "answers";
    /// Its answers, each `{text, does}`, `does` one of `answer`, `drop`, `archive`.
    pub const RESOLUTIONS: &str = "resolutions";
    /// The résumé a retired piece sits behind.
    pub const RETIRED_BY: &str = "retired_by";
    /// Why a memory was retired, one line, as the call gave it.
    pub const RETIRED_WHY: &str = "retired_why";
    /// A stored conversation's thread: `<screen>/<stem>`, the one piece it updates.
    pub const THREAD: &str = "thread";
    /// The models a stored conversation heard from.
    pub const MODELS: &str = "models";
}

// =============================================================================
// Implementation
// =============================================================================

/// The house order of the known keys.
const HOUSE_ORDER: [&str; 28] = [
    key::ID,
    key::TITLE,
    key::NAME,
    key::DATE,
    key::TAG,
    key::DEFINITION,
    key::ALIASES,
    key::KIND,
    key::ROLE,
    key::COMPANY,
    key::CONTACT,
    key::PRIORITY,
    key::CATEGORY,
    key::OWNER,
    key::DUE,
    key::CREATED,
    key::TAGS,
    key::PEOPLE,
    key::MARKS,
    key::SOURCE,
    key::CAPTURE,
    key::CAPTURED,
    key::UPDATED,
    key::STATUS,
    key::LINKS,
    key::FILES,
    key::SORTED,
    key::SORTED_BY,
];

/// The house order of a task, `status` second as the task files have it.
const TASK_ORDER: [&str; 28] = [
    key::ID,
    key::TITLE,
    key::STATUS,
    key::NAME,
    key::DATE,
    key::TAG,
    key::DEFINITION,
    key::ALIASES,
    key::KIND,
    key::ROLE,
    key::COMPANY,
    key::CONTACT,
    key::PRIORITY,
    key::CATEGORY,
    key::OWNER,
    key::DUE,
    key::CREATED,
    key::TAGS,
    key::PEOPLE,
    key::MARKS,
    key::SOURCE,
    key::CAPTURE,
    key::CAPTURED,
    key::UPDATED,
    key::LINKS,
    key::FILES,
    key::SORTED,
    key::SORTED_BY,
];

/// The opening and closing fence.
const FENCE: &str = "---";

/// Keys whose value is a `YYYY-MM-DD` date or null.
pub(crate) const DATE_KEYS: [&str; 4] = [key::UPDATED, key::CREATED, key::DUE, key::DATE];

/// Keys whose value is an RFC 3339 date-time or null.
pub(crate) const TIME_KEYS: [&str; 2] = [key::CAPTURED, key::SORTED];

/// Keys whose value is a list of scalars.
pub(crate) const LIST_KEYS: [&str; 5] =
    [key::TAGS, key::PEOPLE, key::MARKS, key::LINKS, key::ALIASES];

/// Keys whose value is text.
pub(crate) const TEXT_KEYS: [&str; 3] = [key::TITLE, key::NAME, key::TAG];

/// Keys a file must not carry: the path decides them.
pub(crate) const PATH_KEYS: [&str; 3] = [key::SLICE, key::LAYER, key::CONFIDENTIAL];

/// A file cut at its fences.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Split<'a> {
    /// The YAML between the fences.
    pub(crate) yaml: &'a str,
    /// Everything after the closing fence's line.
    pub(crate) body: &'a str,
    /// Whether the opening fence ends with `\r\n`.
    pub(crate) crlf: bool,
}

/// Whether `key` is one of the house keys.
pub(crate) fn is_known(key: &str) -> bool {
    HOUSE_ORDER.contains(&key)
}

/// The rank of a known key; a task writes `status` right after its title.
pub(crate) fn rank(key: &str, task: bool) -> usize {
    let order: &[&str] = if task { &TASK_ORDER } else { &HOUSE_ORDER };
    order.iter().position(|k| *k == key).unwrap_or(order.len())
}

/// Cut a file at its `---` fences.
pub(crate) fn split(text: &str) -> Result<Split<'_>, String> {
    let (rest, crlf) = if let Some(rest) = text.strip_prefix("---\r\n") {
        (rest, true)
    } else if let Some(rest) = text.strip_prefix("---\n") {
        (rest, false)
    } else {
        return Err("no opening --- fence on the first line".into());
    };
    let mut start = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\n', '\r']) == FENCE {
            let body = &rest[start + line.len()..];
            return Ok(Split {
                yaml: &rest[..start],
                body,
                crlf,
            });
        }
        start += line.len();
    }
    Err("no closing --- fence".into())
}

/// The frontmatter's keys and values in file order.
pub(crate) fn parse_fields(yaml: &str) -> Result<Vec<(String, Value)>, ParseError> {
    let value: Value =
        serde_yaml_ng::from_str(yaml).map_err(|e| ParseError::Yaml(e.to_string()))?;
    let mapping = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Mapping(mapping) => mapping,
        _ => {
            return Err(ParseError::Format(
                "the frontmatter is not a mapping".into(),
            ));
        }
    };
    mapping
        .into_iter()
        .map(|(k, v)| match k {
            Value::String(k) => Ok((k, v)),
            other => Err(ParseError::Format(format!(
                "a key that is not text: {other:?}"
            ))),
        })
        .collect()
}

/// Why a frontmatter did not parse.
#[derive(Debug)]
pub(crate) enum ParseError {
    /// Not YAML.
    Yaml(String),
    /// YAML, but not a mapping of text keys.
    Format(String),
}

/// The file text: fences, the fields in the given order, the body as is.
pub(crate) fn render<'a>(
    fields: impl Iterator<Item = (&'a str, &'a Value)>,
    body: &str,
    crlf: bool,
) -> String {
    let nl = if crlf { "\r\n" } else { "\n" };
    let mut out = format!("{FENCE}{nl}");
    for (key, value) in fields {
        out.push_str(&render_entry(key, value, nl));
    }
    out.push_str(FENCE);
    out.push_str(nl);
    out.push_str(body);
    out
}

/// One `key: value` entry with its line ending(s).
fn render_entry(key: &str, value: &Value, nl: &str) -> String {
    match inline(value) {
        Some(text) => format!("{key}: {text}{nl}"),
        None => block(key, value, nl),
    }
}

/// A value on the key's line, or `None` when it needs a block.
fn inline(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(scalar(s, false)),
        Value::Sequence(items) => {
            let parts: Option<Vec<String>> = items.iter().map(flow_item).collect();
            Some(format!("[{}]", parts?.join(", ")))
        }
        Value::Mapping(m) if m.is_empty() => Some("{}".into()),
        Value::Mapping(_) | Value::Tagged(_) => None,
        other => Some(yaml_text(other)),
    }
}

/// One element of a flow list, or `None` when it is not a scalar.
fn flow_item(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(scalar(s, true)),
        Value::Sequence(_) | Value::Mapping(_) | Value::Tagged(_) => None,
        other => Some(yaml_text(other)),
    }
}

/// A value written by `serde_yaml_ng`, indented under its key.
fn block(key: &str, value: &Value, nl: &str) -> String {
    let mut out = format!("{key}:{nl}");
    for line in yaml_text(value).lines() {
        out.push_str("  ");
        out.push_str(line);
        out.push_str(nl);
    }
    out
}

/// `serde_yaml_ng`'s text of a value, without its final newline.
fn yaml_text(value: &Value) -> String {
    let text = serde_yaml_ng::to_string(value).unwrap_or_default();
    text.trim_end_matches('\n').to_string()
}

/// A string plain when YAML reads it back as the same string, else double-quoted.
pub(crate) fn scalar(s: &str, flow: bool) -> String {
    if reads_back_plain(s, flow) {
        s.to_string()
    } else {
        quoted(s)
    }
}

/// Whether `s` written plain parses back to the same string.
fn reads_back_plain(s: &str, flow: bool) -> bool {
    if s.is_empty() || s.chars().any(|c| c.is_control()) || s.trim() != s {
        return false;
    }
    if s.starts_with([
        '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`',
    ]) {
        return false;
    }
    if s.contains(": ") || s.contains(" #") || s.ends_with(':') {
        return false;
    }
    if flow && s.contains([',', '[', ']', '{', '}']) {
        return false;
    }
    let doc = if flow {
        format!("k: [{s}]")
    } else {
        format!("k: {s}")
    };
    let Ok(Value::Mapping(map)) = serde_yaml_ng::from_str::<Value>(&doc) else {
        return false;
    };
    let read = match map.get("k") {
        Some(Value::Sequence(items)) if flow && items.len() == 1 => &items[0],
        Some(value) if !flow => value,
        _ => return false,
    };
    read.as_str() == Some(s)
}

/// A YAML double-quoted string.
fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn back(text: &str) -> Value {
        let map: serde_yaml_ng::Mapping = serde_yaml_ng::from_str(text).unwrap();
        map.get("k").unwrap().clone()
    }

    #[test]
    fn a_string_is_quoted_only_when_yaml_needs_it() {
        let plain_cases = [
            "Daily brief",
            "M-0001",
            "2026-10-13",
            "2026-10-13T22:30:00+02:00",
            "issue#12",
            "Robin « x » — y",
            "back\\slash \"q\"",
        ];
        for plain in plain_cases {
            assert_eq!(scalar(plain, false), plain);
        }
        assert_eq!(scalar("people/robin", true), "people/robin");
        let cases = [
            "",
            " lead",
            "trail ",
            "a: b",
            "a #b",
            "ends:",
            "null",
            "true",
            "12",
            "1.5",
            "~",
            "[x]",
            "{x}",
            "- x",
            "*x",
            "&x",
            "!x",
            "|",
            ">",
            "'x",
            "\"x",
            "%x",
            "@x",
            "`x",
            "two\nlines",
            "tab\there",
            "#x",
        ];
        for s in cases {
            let text = scalar(s, false);
            assert!(text.starts_with('"'), "{s:?} → {text}");
            assert_eq!(
                back(&format!("k: {text}")),
                Value::from(s),
                "{s:?} → {text}"
            );
        }
        for s in ["a, b", "x]", "y{"] {
            let item = scalar(s, true);
            assert!(item.starts_with('"'), "{s:?}");
            let read = back(&format!("k: [{item}]"));
            assert_eq!(read, Value::Sequence(vec![Value::from(s)]));
        }
    }

    #[test]
    fn lists_are_flow_and_null_stays_null() {
        let tags = Value::Sequence(vec![Value::from("a"), Value::from("b c")]);
        assert_eq!(render_entry("tags", &tags, "\n"), "tags: [a, b c]\n");
        assert_eq!(
            render_entry("links", &Value::Sequence(vec![]), "\n"),
            "links: []\n"
        );
        assert_eq!(render_entry("due", &Value::Null, "\n"), "due: null\n");
        assert_eq!(render_entry("n", &Value::from(3), "\n"), "n: 3\n");
    }

    #[test]
    fn a_nested_mapping_is_written_as_an_indented_block() {
        let fields = parse_fields("extra:\n  a: 1\n  b: [x]\n").unwrap();
        let text = render(fields.iter().map(|(k, v)| (k.as_str(), v)), "", false);
        assert_eq!(parse_fields(split(&text).unwrap().yaml).unwrap(), fields);
    }

    #[test]
    fn the_fences_split_the_file_and_keep_the_body_bytes() {
        let s = split("---\nid: M-1\n---\nbody\n---\nmore\r\n").unwrap();
        assert_eq!(s.yaml, "id: M-1\n");
        assert_eq!(s.body, "body\n---\nmore\r\n");
        assert!(!s.crlf);
        let crlf = split("---\r\nid: M-1\r\n---\r\nbody").unwrap();
        assert_eq!(crlf.body, "body");
        assert!(crlf.crlf);
        assert!(split("id: M-1\n").unwrap_err().contains("opening"));
        assert!(split("---\nid: M-1\n").unwrap_err().contains("closing"));
    }

    #[test]
    fn a_task_writes_its_status_after_its_title() {
        let mut keys = vec![key::LINKS, key::UPDATED, key::STATUS, key::TITLE, key::ID];
        keys.sort_by_key(|k| rank(k, true));
        assert_eq!(
            keys,
            [key::ID, key::TITLE, key::STATUS, key::UPDATED, key::LINKS]
        );
        keys.sort_by_key(|k| rank(k, false));
        assert_eq!(
            keys,
            [key::ID, key::TITLE, key::UPDATED, key::STATUS, key::LINKS]
        );
    }
}
