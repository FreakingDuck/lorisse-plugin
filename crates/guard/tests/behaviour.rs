//! The contract's Behaviour table: `rule` with a list none of the three, `rule` on a file that
//! cannot be read, no memory folder, a rule nobody has, and any input whatever its shape.

mod common;

use common::*;
use lorisse_guard::write_rule;

// `rule` with a list that is none of the three: refused in one sentence; nothing written.
#[test]
fn rule_with_a_list_none_of_the_three_is_refused_and_nothing_written() {
    let folder = Folder::with_rules("no_list", README_RULES);
    let empty = Folder::new("no_list_empty");
    for list in ["always", "", "pass ask"] {
        let said = write_rule(folder.path(), "dropbox", &["fetch".to_string()], list).unwrap_err();
        assert!(said.contains("pass, ask or never") && said.ends_with("nothing was written."), "{said}");
        assert_eq!(said.matches(". ").count(), 0, "one sentence: {said}");
        assert!(write_rule(empty.path(), "dropbox", &["fetch".to_string()], list).is_err());
    }
    assert_eq!(std::fs::read_to_string(folder.path().join("rules.yaml")).unwrap(), README_RULES);
    assert!(std::fs::read_dir(empty.path()).unwrap().next().is_none(), "no file made");
}

// `rule` when the rules file cannot be read: refused with the reason; the file left as it is.
#[test]
fn rule_on_a_file_that_cannot_be_read_is_refused_with_the_reason() {
    for text in ["pass:\n  lorisse: [where\n", "pass:\n  lorisse: [where]\nnevr:\n  gmail: [reply]\n"] {
        let folder = Folder::with_rules("unreadable", text);
        let said = write_rule(folder.path(), "gmail", &["reply".to_string()], "never").unwrap_err();
        assert!(said.starts_with("Your rules file, rules.yaml, cannot be read ("), "{said}");
        assert!(said.ends_with("it is left as it is, and nothing was written."), "{said}");
        assert_eq!(std::fs::read_to_string(folder.path().join("rules.yaml")).unwrap(), text);
    }
    let folder = Folder::with_rules("one_line", "pass: {dropbox: [fetch]}\n");
    let said = write_rule(folder.path(), "dropbox", &["move".to_string()], "ask").unwrap_err();
    assert!(said.contains("is left as it is: its list pass is written on one line"), "{said}");
}

// No memory folder: the first set holds, and a call of `rule` still asks.
#[test]
fn without_a_memory_folder_the_first_set_holds() {
    assert_eq!(decision(&ask(&lorisse("where"), None)).as_deref(), Some("allow"));
    assert!(reason(&ask(&lorisse("where"), None)).contains("first set"));
    assert_eq!(decision(&ask(&lorisse("rule"), None)).as_deref(), Some("ask"));
    assert_eq!(ask("mcp__claude_ai_Dropbox__fetch", None), None);
}

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
