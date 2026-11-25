//! The contract's Behaviour table and the README's "The tools", beyond the validation's table:
//! the answers on the sample, the arguments, a broken header, and the program staying up.

mod common;

use common::*;

#[test]
fn search_puts_a_title_first_leaves_tasks_out_and_says_a_retired_memory_after_the_others() {
    let copy = Temp::sample("order");
    let got = ask_one(copy.path(), "search", r#"{"text":"boiler"}"#);
    let at = |s: &str| got.find(s).unwrap_or_else(|| panic!("{s} in {got}"));
    assert!(at("M-0001 [") < at("people/sam-example ["), "{got}");
    assert!(!got.contains("T-0001"), "a task is not shown: {got}");
    assert!(!got.contains("M-0005"), "the inbox is not shown: {got}");
    let phone = ask_one(copy.path(), "search", r#"{"text":"phone"}"#);
    let retired = phone.lines().position(|l| l.contains("M-0004")).expect("the retired memory is found");
    assert!(phone.lines().nth(retired).unwrap().contains("(retired)"), "{phone}");
    let after = phone.lines().skip(retired + 1).any(|l| !l.contains("(retired)") && l.contains(" · "));
    assert!(!after, "nothing live after the retired one: {phone}");
}

#[test]
fn read_gives_every_field_of_the_header_then_the_text_and_finds_a_person_and_a_tag_by_name() {
    let copy = Temp::sample("fields");
    let got = ask_one(copy.path(), "read", r#"{"piece":"m-2"}"#);
    for field in ["id: M-0002", "title: Boiler serviced in March", "kind: fact", "tags: [home]",
        "source: phone", "captured: 2020-03-12T18:30:00+01:00", "updated: 2020-03-12",
        "status: sorted", "links: [M-0001]", "Nothing to repair."] {
        assert!(got.contains(field), "{field}: {got}");
    }
    let sam = ask_one(copy.path(), "read", r#"{"piece":"sam example"}"#);
    assert!(sam.contains("role: plumber") && sam.contains("Call in the morning."), "{sam}");
    let tag = ask_one(copy.path(), "read", r#"{"piece":"Home"}"#);
    assert!(tag.contains("definition: the house and what keeps it running"), "{tag}");
}

#[test]
fn links_go_both_ways_and_a_task_or_a_capture_is_number_and_title_only() {
    let copy = Temp::sample("both_ways");
    let two = ask_one(copy.path(), "links", r#"{"piece":"M-0002"}"#);
    assert!(two.contains("It links to:") && two.contains("· The boiler's contract"), "{two}");
    let one = ask_one(copy.path(), "links", r#"{"piece":"M-0001"}"#);
    for from in ["· Boiler serviced in March", "· Sam Example", "T-0001 [", "· Call the plumber"] {
        assert!(one.contains(from), "{from}: {one}");
    }
    let path = copy.path().join("memories/M-0003.md");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("links: []", "links: [M-0005, M-0077]")).unwrap();
    let three = ask_one(copy.path(), "links", r#"{"piece":"M-0003"}"#);
    assert!(three.contains("M-0005 · a capture still in the inbox"), "{three}");
    assert!(three.contains("M-0077: this link leads to no piece."), "{three}");
    assert!(!three.contains("roof"), "no title or text of the capture: {three}");
}

#[test]
fn a_task_or_a_capture_asked_by_number_is_one_sentence() {
    let copy = Temp::sample("only");
    let task = ask_one(copy.path(), "read", r#"{"piece":"T-0001"}"#);
    assert_eq!(task, "This version of Lorisse reads memories, people and tags only: T-0001 is a task.");
    let capture = ask_one(copy.path(), "read", r#"{"piece":"M-0005"}"#);
    assert_eq!(
        capture,
        "This version of Lorisse reads memories, people and tags only: M-0005 is a capture still in the inbox."
    );
    let links = ask_one(copy.path(), "links", r#"{"piece":"T-0001"}"#);
    assert_eq!(links, task);
}

#[test]
fn an_argument_missing_or_of_the_wrong_kind_says_what_is_expected() {
    let copy = Temp::sample("arguments");
    let setting = copy.path().to_string_lossy().into_owned();
    let lines = vec![
        call_line(1, "search", "{}"),
        call_line(2, "search", r#"{"text":7}"#),
        call_line(3, "read", r#"{"piece":null}"#),
        call_line(4, "links", r#"{"id":"M-0001"}"#),
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"read"}}"#.to_string(),
    ];
    let replies = talk_with(program(&[(SETTING, &setting)], None), &lines);
    let said: Vec<(String, bool)> = replies.iter().map(answer_of).collect();
    for (text, is_error) in &said {
        assert!(*is_error, "{text}");
        assert!(text.contains("needs \"") && text.ends_with("as a text."), "{text}");
    }
    assert!(said[0].0.starts_with("search needs \"text\""));
    assert!(said[3].0.starts_with("links needs \"piece\""));
}

#[test]
fn a_piece_whose_header_cannot_be_read_is_named_and_the_rest_is_given() {
    let copy = Temp::sample("broken");
    copy.write("memories/M-0070.md", "---\ntitle: [unclosed boiler\n---\nA boiler note.\n");
    let search = ask_one(copy.path(), "search", r#"{"text":"boiler"}"#);
    assert!(search.contains("The boiler's contract"), "the rest is given: {search}");
    assert!(search.ends_with("Not searched, their header cannot be read: memories/M-0070.md."), "{search}");
    let read = ask_one(copy.path(), "read", r#"{"piece":"M-0070"}"#);
    assert!(read.starts_with("The file memories/M-0070.md cannot be read"), "{read}");
}

const GUTTER: &str = r#"{"title":"The gutter was cleaned","text":"The gutter was cleaned in October."}"#;

#[test]
fn a_writing_tool_in_a_folder_not_under_git_is_refused_and_writes_nothing() {
    let copy = Temp::sample("not_git");
    let before = snapshot(copy.path());
    let setting = copy.path().to_string_lossy().into_owned();
    let calls = [
        ("remember", GUTTER),
        ("note", r#"{"piece":"M-0002","text":"x"}"#),
        ("link", r#"{"from":"M-0003","to":"M-0001"}"#),
        ("undo", "{}"),
    ];
    for said in ask(&[(SETTING, &setting)], None, &calls) {
        assert!(said.starts_with("Lorisse cannot write because the memory folder"), "{said}");
        assert!(said.contains("is not under git: run \"git init\""), "{said}");
    }
    assert_eq!(snapshot(copy.path()), before);
}

#[test]
fn a_writing_tool_without_git_or_without_a_name_for_the_commits_names_what_is_missing() {
    let copy = Temp::git_sample("no_name");
    run_git(copy.path(), &["config", "--unset", "user.name"]);
    run_git(copy.path(), &["config", "--unset", "user.email"]);
    let empty = Temp::empty("no_global");
    empty.write("gitconfig", "");
    let global = empty.path().join("gitconfig").to_string_lossy().into_owned();
    let setting = copy.path().to_string_lossy().into_owned();
    let before = files_of(copy.path());
    let commits = log(copy.path());
    let env = [(SETTING, setting.as_str()), ("GIT_CONFIG_GLOBAL", &global), ("GIT_CONFIG_NOSYSTEM", "1")];
    let said = ask(&env, None, &[("remember", GUTTER), ("undo", "{}")]);
    for said in &said {
        assert!(said.starts_with("Lorisse cannot write because git has no name or no address set for the commits of home"), "{said}");
    }
    let no_git = ask(&[(SETTING, &setting), ("PATH", "")], None, &[("note", r#"{"piece":"M-0002","text":"x"}"#)]);
    assert!(no_git[0].starts_with("Lorisse cannot write because git is not found on this machine"), "{}", no_git[0]);
    assert_eq!(files_of(copy.path()), before);
    assert_eq!(log(copy.path()), commits);
}

#[test]
fn undo_with_no_change_of_lorisse_s_says_so_in_one_sentence() {
    let copy = Temp::git_sample("nothing_to_undo");
    let said = ask_one(copy.path(), "undo", "{}");
    assert_eq!(said, "Lorisse has no change of hers left to take back in this memory folder: nothing was done.");
    ask_one(copy.path(), "remember", GUTTER);
    ask_one(copy.path(), "undo", "{}");
    assert_eq!(ask_one(copy.path(), "undo", "{}"), said, "once taken back, nothing is left");
}

/// A hook that refuses every commit, so a write fails after its files are written.
fn refuse_commits(dir: &std::path::Path) {
    let hook = dir.join(".git/hooks/pre-commit");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn a_change_that_fails_halfway_leaves_the_folder_as_it_was() {
    let copy = Temp::git_sample("halfway");
    ask_one(copy.path(), "remember", GUTTER);
    refuse_commits(copy.path());
    let before = files_of(copy.path());
    let commits = log(copy.path());
    let calls = [
        ("remember", r#"{"title":"The hedge","text":"Cut in May."}"#),
        ("note", r#"{"piece":"M-0002","text":"x"}"#),
        ("link", r#"{"from":"M-0003","to":"M-0001"}"#),
        ("undo", "{}"),
    ];
    let setting = copy.path().to_string_lossy().into_owned();
    for said in ask(&[(SETTING, &setting)], None, &calls) {
        assert!(said.starts_with("Nothing was written: the change could not be committed, the folder is as it was"), "{said}");
    }
    assert_eq!(files_of(copy.path()), before);
    assert_eq!(log(copy.path()), commits);
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
}

#[test]
fn the_program_stays_up_whatever_comes() {
    let copy = Temp::sample("up");
    let setting = copy.path().to_string_lossy().into_owned();
    let lines = vec![
        "not json at all".to_string(),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"nope","arguments":{}}}"#.to_string(),
        call_line(2, "read", r#"{"piece":"../../../etc/passwd"}"#),
        call_line(3, "read", r#"{"piece":"people/../../x"}"#),
        call_line(4, "search", &format!(r#"{{"text":"{}"}}"#, "z".repeat(100_000))),
        call_line(5, "links", r#"{"piece":"M-99999999999999999999999"}"#),
        r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":"x"}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#.to_string(),
    ];
    let replies = talk_with(program(&[(SETTING, &setting)], None), &lines);
    assert_eq!(replies.len(), lines.len(), "one answer a line");
    assert!(replies[7].get("result").is_some(), "still answering at the end");
}
