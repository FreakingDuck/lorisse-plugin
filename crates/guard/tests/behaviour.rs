//! The contract's Behaviour table: a rule nobody has, and any input whatever its shape.

mod common;

use common::*;

// A rule naming a tool or a service nobody has is ignored; the other rules hold.
#[test]
fn a_rule_nobody_has_is_ignored() {
    let folder = Folder::with_rules(
        "nobody",
        "pass:\n  nobody: [nothing]\n  dropbox: [fetch, no_such_tool]\nnever:\n  ghost: [delete]\n",
    );
    let here = Some(folder.path());
    assert_eq!(decision(&ask("mcp__claude_ai_Dropbox__fetch", here)).as_deref(), Some("allow"));
    assert_eq!(ask("mcp__claude_ai_Dropbox__delete", here), None);
}

// Any input, whatever its shape, gets an answer or nothing, and never a panic.
#[test]
fn any_input_gets_an_answer_or_nothing() {
    let folder = Folder::with_rules("shapes", README_RULES);
    let inputs = [
        "", "   ", "null", "42", "[]", "{}", "\"text\"", "{not json", "\u{0}\u{1}",
        r#"{"tool_name": 7}"#,
        r#"{"tool_name": ""}"#,
        r#"{"tool_name": "__"}"#,
        r#"{"tool_name": "mcp__"}"#,
        r#"{"tool_name": "____x"}"#,
        r#"{"tool_name": "mcp__é__ü"}"#,
        r#"{"tool_name": "Write"}"#,
        r#"{"tool_name": "Write", "tool_input": "a string"}"#,
        r#"{"tool_name": "Write", "tool_input": {"file_path": 3}}"#,
        r#"{"tool_name": "Edit", "tool_input": {"file_path": ""}}"#,
        r#"{"tool_name": "mcp__claude_ai_Dropbox__delete", "hook_event_name": "PostToolUse"}"#,
        r#"{"tool_name": "mcp__claude_ai_Dropbox__delete", "tool_input": null}"#,
    ];
    for input in inputs {
        for here in [None, Some(folder.path())] {
            if let Some(text) = lorisse_guard::answer(input, here) {
                read(&text);
            }
        }
    }
    let denied = lorisse_guard::answer(r#"{"tool_name": "mcp__claude_ai_Dropbox__delete"}"#, Some(folder.path()));
    assert!(denied.is_some());
}
