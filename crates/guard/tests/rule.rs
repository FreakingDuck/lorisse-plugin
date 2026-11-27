//! The rule asked once, C-6.1 and C-6.2: a rule written in the person's file as text, and a call
//! of `rule` that always asks. The commit and its undo are the program's tests (`tools`).

mod common;

use common::*;
use lorisse_guard::{write_rule, List};

fn tools(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
}

fn rules_of(folder: &Folder) -> String {
    std::fs::read_to_string(folder.path().join("rules.yaml")).unwrap()
}

// C-6.1, writes: a rule written: its service, its tools, its list; the guard reads it at once.
#[test]
fn c6_1_a_rule_is_written_under_its_service_and_its_list() {
    let folder = Folder::with_rules("written", "pass:\n  lorisse: [where, search]\nask: {}\n");
    let done = write_rule(folder.path(), "dropbox", &tools(&["fetch", "list_folder"]), "pass").unwrap();
    assert!(done.changed && !done.made);
    assert_eq!(done.list, List::Pass);
    assert_eq!(done.said, "pass fetch, list_folder");
    assert_eq!(rules_of(&folder), "pass:\n  lorisse: [where, search]\n  dropbox: [fetch, list_folder]\nask: {}\n");
    assert_eq!(decision(&ask("mcp__claude_ai_Dropbox__fetch", Some(folder.path()))).as_deref(), Some("allow"));
    let asked = write_rule(folder.path(), "Dropbox", &tools(&["move", "copy"]), "ASK").unwrap();
    assert_eq!(asked.said, "pass fetch, list_folder; ask move, copy");
    assert!(rules_of(&folder).ends_with("ask:\n  Dropbox: [move, copy]\n"), "{}", rules_of(&folder));
    assert_eq!(decision(&ask("mcp__claude_ai_Dropbox__copy", Some(folder.path()))).as_deref(), Some("ask"));
    let again = write_rule(folder.path(), "dropbox", &tools(&["FETCH"]), "pass").unwrap();
    assert!(!again.changed, "a rule already there changes nothing");
}

// C-6.1, writes: a tool named under another list moves to the new one, and only that tool.
#[test]
fn c6_1_a_tool_named_under_another_list_moves_to_the_new_one() {
    let folder = Folder::with_rules(
        "moved",
        "pass:\n  dropbox: [fetch, move]\nask:\n  dropbox:\n    - copy\n    - delete\nnever:\n  gmail: [send_message]\n",
    );
    let done = write_rule(folder.path(), "dropbox", &tools(&["move", "delete"]), "never").unwrap();
    assert_eq!(done.said, "pass fetch; ask copy; never move, delete");
    assert_eq!(
        rules_of(&folder),
        "pass:\n  dropbox: [fetch]\nask:\n  dropbox:\n    - copy\nnever:\n  gmail: [send_message]\n  dropbox: [move, delete]\n"
    );
    let here = Some(folder.path());
    assert_eq!(decision(&ask("mcp__claude_ai_Dropbox__move", here)).as_deref(), Some("deny"));
    assert_eq!(decision(&ask("mcp__claude_ai_Dropbox__fetch", here)).as_deref(), Some("allow"));
    assert_eq!(decision(&ask("mcp__claude_ai_Gmail__send_message", here)).as_deref(), Some("deny"));
}

// C-6.1, writes: comments, an odd layout and CRLF endings: every old line is still there after.
#[test]
fn c6_1_every_other_line_of_the_person_s_file_stays_as_it_was() {
    let mine = "# My rules, by hand.\r\n\r\npass:   # what runs alone\r\n    lorisse:   [ where, plop ]\r\n    # the files\r\n    dropbox: [\"fetch\", 'search']  # reads only\r\n\r\nnever:\r\n    gmail:\r\n    - send_message\r\n    - forward   # never\r\n# the end";
    let folder = Folder::with_rules("odd", mine);
    write_rule(folder.path(), "google calendar", &tools(&["list_events"]), "ask").unwrap();
    write_rule(folder.path(), "gmail", &tools(&["reply"]), "never").unwrap();
    let after = rules_of(&folder);
    let mut new = after.split_inclusive('\n').map(str::to_string).collect::<Vec<_>>();
    for old in mine.split_inclusive('\n') {
        let old = match old.ends_with('\n') {
            true => old.to_string(),
            false => format!("{old}\r\n"),
        };
        let at = new.iter().position(|l| *l == old).unwrap_or_else(|| panic!("{old:?} kept in {after:?}"));
        new.remove(at);
    }
    assert_eq!(new, ["    - reply\r\n", "ask:\r\n", "    google calendar: [list_events]\r\n"]);
    assert!(!after.replace("\r\n", "").contains('\n'), "every line ends as the file's do: {after:?}");
    let here = Some(folder.path());
    assert_eq!(decision(&ask("mcp__claude_ai_Gmail__reply", here)).as_deref(), Some("deny"));
    assert_eq!(decision(&ask("mcp__claude_ai_Google_Calendar__list_events", here)).as_deref(), Some("ask"));
}

// C-6.2, output: a call of `rule` asks, with the reason, with no file, with the first set copied.
#[test]
fn c6_2_a_call_of_rule_always_asks_with_its_reason() {
    let none = Folder::new("rule_none");
    let copied = Folder::with_rules("rule_first", lorisse_guard::FIRST_RULES);
    for here in [None, Some(none.path()), Some(copied.path())] {
        let answer = ask(&lorisse("rule"), here);
        assert_eq!(decision(&answer).as_deref(), Some("ask"));
        let said = reason(&answer);
        assert!(said.contains("You are about to change your own rules (rules.yaml)"), "{said}");
    }
    let listed = lorisse_guard::FIRST_RULES.lines().find(|l| l.trim_start().starts_with("lorisse:")).unwrap();
    assert!(!listed.split(['[', ',', ']']).any(|t| t.trim() == "rule"), "rule is not in the first set: {listed}");
}

// C-6.2, output: a file that names `rule` under pass, or a file that cannot be read, still asks.
#[test]
fn c6_2_a_file_naming_rule_under_pass_does_not_let_it_through() {
    let pass = Folder::with_rules("rule_pass", "pass:\n  lorisse: [where, rule]\n");
    let broken = Folder::with_rules("rule_broken", "pass: [\n");
    for here in [pass.path(), broken.path()] {
        let answer = ask(&lorisse("rule"), Some(here));
        assert_eq!(decision(&answer).as_deref(), Some("ask"));
        assert!(reason(&answer).contains("your own rules"), "{}", reason(&answer));
    }
    assert_eq!(decision(&ask(&lorisse("where"), Some(pass.path()))).as_deref(), Some("allow"));
}
