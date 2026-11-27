//! What the guard's tests share: a temporary memory folder, a call, the answer read.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{json, Value};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A new empty folder under the system's temporary folder, removed when dropped.
pub struct Folder(pub PathBuf);

impl Folder {
    pub fn new(name: &str) -> Folder {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("lorisse-guard-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("the test's folder");
        Folder(path)
    }

    /// A folder holding this `rules.yaml`.
    pub fn with_rules(name: &str, rules: &str) -> Folder {
        let folder = Folder::new(name);
        folder.write_rules(rules);
        folder
    }

    pub fn write_rules(&self, rules: &str) {
        std::fs::write(self.0.join("rules.yaml"), rules).expect("the rules file");
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The text a `PreToolUse` hook receives for a call of this tool with this input.
pub fn received(tool: &str, input: Value) -> String {
    json!({
        "session_id": "abc123",
        "transcript_path": "/home/someone/.claude/projects/x/transcript.jsonl",
        "cwd": "/home/someone/project",
        "permission_mode": "default",
        "hook_event_name": "PreToolUse",
        "tool_name": tool,
        "tool_input": input,
        "tool_use_id": "toolu_01ABC"
    })
    .to_string()
}

/// The answer to a call of this tool with no input, in this folder.
pub fn ask(tool: &str, folder: Option<&Path>) -> Option<Value> {
    lorisse_guard::answer(&received(tool, json!({})), folder).map(|text| read(&text))
}

/// The printed text, read back, and checked to be the hook's shape.
pub fn read(text: &str) -> Value {
    let value: Value = serde_json::from_str(text).expect("the answer is JSON");
    let out = &value["hookSpecificOutput"];
    assert_eq!(out["hookEventName"], "PreToolUse", "{text}");
    assert!(out["permissionDecisionReason"].is_string(), "{text}");
    value
}

/// The decision of an answer: allow, ask or deny.
pub fn decision(answer: &Option<Value>) -> Option<String> {
    answer
        .as_ref()
        .map(|v| v["hookSpecificOutput"]["permissionDecision"].as_str().unwrap_or("").to_string())
}

/// The reason of an answer.
pub fn reason(answer: &Option<Value>) -> String {
    answer
        .as_ref()
        .and_then(|v| v["hookSpecificOutput"]["permissionDecisionReason"].as_str())
        .unwrap_or("")
        .to_string()
}

/// The README's example.
pub const README_RULES: &str = "\
pass:
  lorisse: [where, plop, search, read, links, remember, note, link, undo]
  dropbox: [search, list_folder, fetch, file_preview, get_file_metadata]
ask:
  dropbox: [move, copy, create_folder, create_file]
never:
  dropbox: [delete, create_shared_link]
  gmail:   [send_message, reply, forward]
";

/// Lorisse's fifteen tools.
pub const LORISSE_TOOLS: [&str; 15] = [
    "where", "plop", "search", "read", "links", "remember", "note", "link", "attach", "undo",
    "start", "import", "change", "retire", "journal",
];

/// A tool of this plugin, by its full name.
pub fn lorisse(tool: &str) -> String {
    format!("mcp__plugin_lorisse_lorisse__{tool}")
}
