//! The rules, C-4.1 to C-4.4: where they are read, the first set, the strictest, the unnamed.

mod common;

use common::*;

// C-4.1, input: the folder's rules.yaml is the one read.
#[test]
fn c4_1_the_folder_rules_file_is_read() {
    let folder = Folder::with_rules("read", README_RULES);
    let here = Some(folder.path());
    let listed = ask("mcp__claude_ai_Dropbox__list_folder", here);
    assert_eq!(decision(&listed).as_deref(), Some("allow"));
    let moved = ask("mcp__claude_ai_Dropbox__move", here);
    assert_eq!(decision(&moved).as_deref(), Some("ask"));
    let sent = ask("mcp__claude_ai_Gmail__send_message", here);
    assert_eq!(decision(&sent).as_deref(), Some("deny"));
}

// C-4.1, input: a change of the file holds from the next call on.
#[test]
fn c4_1_a_change_holds_from_the_next_call() {
    let folder = Folder::with_rules("change", README_RULES);
    let here = Some(folder.path());
    let call = "mcp__claude_ai_Dropbox__search";
    assert_eq!(decision(&ask(call, here)).as_deref(), Some("allow"));
    folder.write_rules("never:\n  dropbox: [search]\n");
    assert_eq!(decision(&ask(call, here)).as_deref(), Some("deny"));
    folder.write_rules("pass: {}\n");
    assert_eq!(decision(&ask(call, here)), None);
}

// C-4.2, output: a folder without rules.yaml is ruled by the first set.
#[test]
fn c4_2_a_folder_without_the_file_has_the_first_set() {
    let folder = Folder::new("first");
    let here = Some(folder.path());
    for tool in LORISSE_TOOLS {
        let answer = ask(&lorisse(tool), here);
        assert_eq!(decision(&answer).as_deref(), Some("allow"), "{tool}");
        assert!(reason(&answer).contains("first set"), "{}", reason(&answer));
    }
    assert_eq!(ask(&lorisse("forget"), here), None);
    assert_eq!(ask("mcp__claude_ai_Dropbox__list_folder", here), None);
    assert_eq!(ask("Bash", here), None);
    assert!(std::fs::read_dir(folder.path()).unwrap().next().is_none(), "nothing written");
}

// C-4.2, output: no memory folder at all, the same first set; it is the plugin's own file.
#[test]
fn c4_2_no_folder_has_the_first_set() {
    for tool in LORISSE_TOOLS {
        assert_eq!(decision(&ask(&lorisse(tool), None)).as_deref(), Some("allow"), "{tool}");
    }
    assert_eq!(ask("mcp__claude_ai_Gmail__send_message", None), None);
    let shipped = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/first_rules.yaml")).unwrap();
    assert_eq!(lorisse_guard::FIRST_RULES, shipped);
    let copied = Folder::with_rules("copied", lorisse_guard::FIRST_RULES);
    assert_eq!(decision(&ask(&lorisse("where"), Some(copied.path()))).as_deref(), Some("allow"));
}

// C-4.5, output: a tool of Lorisse's the person's file names nowhere passes by the first set.
#[test]
fn c4_5_a_lorisse_tool_the_file_does_not_name_passes_by_the_first_set() {
    let folder = Folder::with_rules("older", "pass:\n  lorisse: [where, search]\n");
    let here = Some(folder.path());
    for tool in ["journal", "change", "retire"] {
        let answer = ask(&lorisse(tool), here);
        assert_eq!(decision(&answer).as_deref(), Some("allow"), "{tool}");
        assert!(reason(&answer).contains("first set"), "{}", reason(&answer));
    }
    let named = ask(&lorisse("where"), here);
    assert!(reason(&named).contains("rules.yaml"), "{}", reason(&named));
    assert_eq!(ask(&lorisse("forget"), here), None, "a name the first set does not hold either");
}

// C-4.5, output: a tool of Lorisse's the person's file names under ask or never follows the file.
#[test]
fn c4_5_a_lorisse_tool_the_file_names_follows_the_file() {
    let asks = Folder::with_rules("journal_ask", "pass:\n  lorisse: [where, search]\nask:\n  lorisse: [journal]\n");
    let answer = ask(&lorisse("journal"), Some(asks.path()));
    assert_eq!(decision(&answer).as_deref(), Some("ask"));
    assert!(reason(&answer).contains("rules.yaml"), "{}", reason(&answer));
    let never = Folder::with_rules("journal_never", "pass:\n  lorisse: [where, search, journal]\nnever:\n  lorisse: [journal]\n");
    let answer = ask(&lorisse("journal"), Some(never.path()));
    assert_eq!(decision(&answer).as_deref(), Some("deny"), "the strictest inside the file");
    assert!(reason(&answer).contains("rules.yaml"), "{}", reason(&answer));
}

// C-4.5, output: a tool of another service the file does not name is left to the app.
#[test]
fn c4_5_another_service_s_tool_not_named_is_left_to_the_app() {
    let folder = Folder::with_rules("other", "pass:\n  lorisse: [where, search]\n  dropbox: [fetch]\n");
    let here = Some(folder.path());
    assert_eq!(ask("mcp__claude_ai_Dropbox__delete", here), None);
    assert_eq!(ask("mcp__claude_ai_Gmail__send_message", here), None);
    assert_eq!(ask("Bash", here), None);
}

// C-4.3, output: a tool in two or three lists takes the strictest.
#[test]
fn c4_3_a_tool_in_two_lists_takes_the_strictest() {
    let folder = Folder::with_rules(
        "strictest",
        "pass:\n  dropbox: [move, copy, fetch]\nask:\n  dropbox: [move, copy]\nnever:\n  Dropbox: [move]\n",
    );
    let here = Some(folder.path());
    assert_eq!(decision(&ask("mcp__claude_ai_Dropbox__move", here)).as_deref(), Some("deny"));
    assert_eq!(decision(&ask("mcp__claude_ai_Dropbox__copy", here)).as_deref(), Some("ask"));
    assert_eq!(decision(&ask("mcp__claude_ai_Dropbox__fetch", here)).as_deref(), Some("allow"));
}

// C-4.4, output: a call no rule names is left to the app: nothing printed.
#[test]
fn c4_4_a_call_no_rule_names_is_left_to_the_app() {
    let folder = Folder::with_rules("unnamed", README_RULES);
    let here = Some(folder.path());
    for call in [
        "mcp__claude_ai_Dropbox__sign_cancel_signature_request",
        "mcp__claude_ai_Google_Calendar__list_events",
        "mcp__plugin_other_other__where",
        "Bash",
        "Read",
    ] {
        assert_eq!(ask(call, here), None, "{call}");
    }
}
