//! The answer, C-5.1 to C-5.4: the three answers, nothing unnamed, the rules file, nothing written.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::*;
use serde_json::json;

// C-5.1, output: pass is the hook's allow, which skips the prompt.
#[test]
fn c5_1_pass_is_allow() {
    let folder = Folder::with_rules("pass", README_RULES);
    let answer = ask("mcp__claude_ai_Dropbox__fetch", Some(folder.path()));
    assert_eq!(decision(&answer).as_deref(), Some("allow"));
    assert!(reason(&answer).contains("pass"), "{}", reason(&answer));
}

// C-5.1, output: ask is the hook's ask, which makes the app ask.
#[test]
fn c5_1_ask_is_ask() {
    let folder = Folder::with_rules("ask", README_RULES);
    let answer = ask("mcp__claude_ai_Dropbox__create_folder", Some(folder.path()));
    assert_eq!(decision(&answer).as_deref(), Some("ask"));
    assert!(reason(&answer).contains("ask"), "{}", reason(&answer));
}

// C-5.1, output: never is the hook's deny, and the reason names the list, the service, the tool.
#[test]
fn c5_1_never_is_deny_and_names_the_rule() {
    let folder = Folder::with_rules("never", README_RULES);
    let answer = ask("mcp__claude_ai_Dropbox__delete", Some(folder.path()));
    assert_eq!(decision(&answer).as_deref(), Some("deny"));
    let said = reason(&answer);
    for word in ["never", "dropbox", "delete", "rules.yaml"] {
        assert!(said.contains(word), "{word} in {said}");
    }
}

// C-5.2, output: nothing passes that pass does not name.
#[test]
fn c5_2_nothing_passes_unnamed() {
    let folder = Folder::with_rules(
        "unnamed",
        "pass:\n  mail: [send_message]\n  dropbox: [fetch]\nask:\n  gmail: [reply]\nnever:\n  gmail: [forward]\n",
    );
    let here = Some(folder.path());
    for call in [
        "mcp__claude_ai_Gmail__send_message",
        "mcp__claude_ai_Gmail__reply",
        "mcp__claude_ai_Gmail__forward",
        "mcp__claude_ai_Dropbox__fetch_more",
        "mcp__claude_ai_Dropboxer__fetch",
        "Bash",
    ] {
        assert_ne!(decision(&ask(call, here)).as_deref(), Some("allow"), "{call}");
    }
    assert_eq!(decision(&ask("mcp__claude_ai_Dropbox__fetch", here)).as_deref(), Some("allow"));
    // Of Lorisse's own tools, what the first set lets pass (C-4.5): unnamed here, it passes by it.
    let own = ask("mcp__plugin_lorisse_lorisse__where", here);
    assert_eq!(decision(&own).as_deref(), Some("allow"));
    assert!(reason(&own).contains("first set"), "{}", reason(&own));
}

// C-5.2, output: a rules file that cannot be read makes every call of a service ask, with why.
#[test]
fn c5_2_a_file_that_cannot_be_read_makes_every_call_ask() {
    let broken = Folder::with_rules("broken", "pass:\n  lorisse: [where\nnever: {{{\n");
    let unknown = Folder::with_rules("unknown", "pass:\n  lorisse: [where]\nnevr:\n  dropbox: [delete]\n");
    let folder = Folder::new("a-folder");
    std::fs::create_dir(folder.path().join("rules.yaml")).unwrap();
    for here in [broken.path(), unknown.path(), folder.path()] {
        for call in [lorisse("where"), "mcp__claude_ai_Dropbox__delete".into(), "mcp__x__y".into()] {
            let answer = ask(&call, Some(here));
            assert_eq!(decision(&answer).as_deref(), Some("ask"), "{call}");
            assert!(reason(&answer).contains("cannot be read"), "{}", reason(&answer));
        }
        assert_eq!(ask("Bash", Some(here)), None);
    }
}

// C-5.3, output: a file tool writing the rules file asks, whatever the rules say.
#[test]
fn c5_3_a_file_tool_on_the_rules_file_asks() {
    let folder = Folder::with_rules("self", "pass:\n  lorisse: [where]\n");
    let here = folder.path();
    let rules = here.join("rules.yaml").display().to_string();
    let slashed = rules.replace('\\', "/");
    let dotted = here.join("sub").join("..").join("RULES.yaml").display().to_string();
    let calls = [
        ("Write", json!({ "file_path": rules, "content": "pass:\n  dropbox: [delete]\n" })),
        ("Edit", json!({ "file_path": slashed, "old_string": "a", "new_string": "b" })),
        ("MultiEdit", json!({ "file_path": dotted, "edits": [] })),
        ("NotebookEdit", json!({ "notebook_path": rules, "new_source": "" })),
    ];
    for (tool, input) in calls {
        let text = lorisse_guard::answer(&received(tool, input), Some(here)).expect(tool);
        assert_eq!(decision(&Some(read(&text))).as_deref(), Some("ask"), "{tool}");
    }
    let relative = json!({ "session_id": "s", "hook_event_name": "PreToolUse", "cwd": here.display().to_string(),
        "tool_name": "Edit", "tool_input": { "file_path": "rules.yaml" } });
    assert!(lorisse_guard::answer(&relative.to_string(), Some(here)).is_some());
    let other = here.join("notes.md").display().to_string();
    assert_eq!(lorisse_guard::answer(&received("Write", json!({ "file_path": other })), Some(here)), None);
    assert_eq!(lorisse_guard::answer(&received("Write", json!({ "file_path": rules })), None), None);
}

/// Every file of a folder and its bytes.
fn snapshot(path: &Path, into: &mut BTreeMap<String, Vec<u8>>) {
    for entry in std::fs::read_dir(path).unwrap().flatten() {
        let p = entry.path();
        match p.is_dir() {
            true => {
                into.insert(p.display().to_string(), Vec::new());
                snapshot(&p, into);
            }
            false => {
                into.insert(p.display().to_string(), std::fs::read(&p).unwrap());
            }
        }
    }
}

// C-5.4, writes: the folder is as it was after every kind of call; the crate needs no network crate.
#[test]
fn c5_4_nothing_written_no_connection() {
    let folder = Folder::with_rules("writes", README_RULES);
    std::fs::write(folder.path().join("config.yaml"), "name: test\n").unwrap();
    let mut before = BTreeMap::new();
    snapshot(folder.path(), &mut before);
    let rules = folder.path().join("rules.yaml").display().to_string();
    for (tool, input) in [
        (lorisse("where"), json!({})),
        ("mcp__claude_ai_Dropbox__delete".to_string(), json!({})),
        ("mcp__claude_ai_Dropbox__move".to_string(), json!({})),
        ("Write".to_string(), json!({ "file_path": rules, "content": "" })),
        ("Bash".to_string(), json!({ "command": "echo" })),
    ] {
        let _ = lorisse_guard::answer(&received(&tool, input), Some(folder.path()));
    }
    let mut after = BTreeMap::new();
    snapshot(folder.path(), &mut after);
    assert_eq!(before, after);
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    let deps: Vec<&str> = manifest
        .split("[dependencies]")
        .nth(1)
        .unwrap_or("")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('['))
        .filter_map(|l| l.split('=').next().map(str::trim))
        .collect();
    assert_eq!(deps, ["serde_json", "serde_yaml_ng"]);
    let sources = ["src/lib.rs", "src/call.rs", "src/rules.rs"];
    for source in sources {
        let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(source)).unwrap();
        for word in ["std::net", "std::process", "fs::write", "File::create", "OpenOptions", "remove_file", "create_dir"] {
            assert!(!text.contains(word), "{word} in {source}");
        }
    }
}
