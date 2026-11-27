//! The VALIDATION.md tables, one test a check: tools C-4.2, writing C-4.1 to C-4.9
//! and opening C-4.1 to C-4.3; the files are `files.rs`. The program is started and asked over stdin, without Claude, on
//! temporary copies of the sample made git folders, a name and an address set in each copy only.

mod common;

use std::fs;
use std::path::Path;

use common::json::Json;
use common::*;

fn read_file(dir: &Path, rel: &str) -> String {
    fs::read_to_string(dir.join(rel)).unwrap()
}

/// The files the newest commit of `dir` touched.
fn files_of_head(dir: &Path) -> Vec<String> {
    let out = run_git(dir, &["show", "--name-only", "--format=", "HEAD"]);
    out.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect()
}

/// The day of the newest commit, as `YYYY-MM-DD`: the date the test reads, never a fixed one.
fn day_of_head(dir: &Path) -> String {
    run_git(dir, &["log", "-1", "--format=%ad", "--date=short"]).trim().to_string()
}

/// The subject of the newest commit.
fn subject_of_head(dir: &Path) -> String {
    run_git(dir, &["log", "-1", "--format=%s"]).trim().to_string()
}

const GUTTER: &str = r#"{"title":"The gutter was cleaned","text":"The gutter was cleaned in October.","kind":"fact","tags":["home"],"links":["M-0001"]}"#;

// ---------------------------------------------------------------------------------------------
// tools C-4.2 each tool's declaration: reads only, or writes (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn tools_c_4_2_the_reading_tools_are_declared_read_only_and_the_writing_tools_as_writing() {
    let line = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#.to_string();
    let got = talk_with(program(&[], None), &[line]);
    let Some(Json::Array(tools)) = got[0].get("result").and_then(|r| r.get("tools")) else {
        panic!("tools is an array");
    };
    let hint = |tool: &Json, key: &str| tool.get("annotations").and_then(|a| a.get(key)).cloned();
    for tool in tools {
        let name = tool.get("name").and_then(Json::as_str).unwrap();
        let reads = ["where", "plop", "search", "read", "links"].contains(&name);
        assert_eq!(hint(tool, "readOnlyHint"), Some(Json::Bool(reads)), "{name}");
        assert_eq!(hint(tool, "openWorldHint"), Some(Json::Bool(false)), "{name}");
        if !reads {
            assert_eq!(hint(tool, "destructiveHint"), Some(Json::Bool(false)), "{name}");
            let once = ["link", "start", "change", "retire", "drawer", "move", "rule"].contains(&name);
            assert_eq!(hint(tool, "idempotentHint"), Some(Json::Bool(once)), "{name}");
        }
    }
    assert_eq!(tools.len(), 18);
}

// ---------------------------------------------------------------------------------------------
// writing C-4.1 a memory filed, then read back (2 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn writing_c_4_1_remember_gives_the_number_and_read_gives_the_memory_back_as_filed() {
    let copy = Temp::git_sample("remember_read");
    let said = ask_one(copy.path(), "remember", GUTTER);
    assert!(said.starts_with("Filed as M-0007 [home]:"), "{said}");
    let file = read_file(copy.path(), "memories/M-0007.md");
    let read = ask_one(copy.path(), "read", r#"{"piece":"M-0007"}"#);
    assert_eq!(read.lines().next(), Some(DATA_LINE), "{read}");
    for line in file.lines().filter(|l| *l != "---") {
        assert!(read.lines().any(|r| r == line), "{line} in {read}");
    }
    for field in ["title: The gutter was cleaned", "kind: fact", "tags: [home]", "links: [M-0001]",
        "status: sorted", "The gutter was cleaned in October."] {
        assert!(read.contains(field), "{field}: {read}");
    }
    let found = ask_one(copy.path(), "search", r#"{"text":"gutter"}"#);
    assert!(found.contains("M-0007 [home] · The gutter was cleaned"), "{found}");
}

#[test]
fn writing_c_4_1_a_memory_without_a_kind_takes_the_folder_s_first_and_a_person_is_linked_by_name() {
    let copy = Temp::git_sample("remember_default");
    let args = r#"{"title":"Sam fixed the tap","text":"Sam fixed the kitchen tap.","links":["Sam Example"]}"#;
    let said = ask_one(copy.path(), "remember", args);
    assert!(said.starts_with("Filed as M-0007"), "{said}");
    let file = read_file(copy.path(), "memories/M-0007.md");
    assert!(file.contains("kind: fact\n") && file.contains("tags: []\n"), "{file}");
    assert!(file.contains("links: [people/sam-example]\n"), "{file}");
    let links = ask_one(copy.path(), "links", r#"{"piece":"Sam Example"}"#);
    assert!(links.contains("M-0007 [home] · Sam fixed the tap"), "{links}");
}

// ---------------------------------------------------------------------------------------------
// writing C-4.2 a note on a memory and on a person (2 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn writing_c_4_2_a_note_on_a_memory_is_one_dated_line_at_the_end_and_nothing_else() {
    let copy = Temp::git_sample("note_memory");
    let before = read_file(copy.path(), "memories/M-0002.md");
    let said = ask_one(copy.path(), "note", r#"{"piece":"M-0002","text":"The next service is due in March."}"#);
    assert!(said.starts_with("Noted on M-0002 [home]"), "{said}");
    let after = read_file(copy.path(), "memories/M-0002.md");
    let day = day_of_head(copy.path());
    assert_eq!(after, format!("{before}- {day}: The next service is due in March.\n"));
}

#[test]
fn writing_c_4_2_a_note_on_a_person_by_name_is_one_dated_line_at_the_end_and_nothing_else() {
    let copy = Temp::git_sample("note_person");
    let before = read_file(copy.path(), "people/sam-example.md");
    let said = ask_one(copy.path(), "note", r#"{"piece":"Sam Example","text":"Prefers a text message."}"#);
    assert!(said.starts_with("Noted on people/sam-example"), "{said}");
    let after = read_file(copy.path(), "people/sam-example.md");
    let day = day_of_head(copy.path());
    assert_eq!(after, format!("{before}- {day}: Prefers a text message.\n"));
}

// ---------------------------------------------------------------------------------------------
// writing C-4.3 a link, and the same link twice (2 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn writing_c_4_3_a_link_is_added_and_seen_both_ways() {
    let copy = Temp::git_sample("link_once");
    let said = ask_one(copy.path(), "link", r#"{"from":"M-0003","to":"m-1"}"#);
    assert!(said.starts_with("Linked: M-0003 [home] now links to M-0001 [home]"), "{said}");
    assert!(read_file(copy.path(), "memories/M-0003.md").contains("links: [M-0001]\n"));
    let links = ask_one(copy.path(), "links", r#"{"piece":"M-0001"}"#);
    assert!(links.contains("M-0003 [home] · The garden plan"), "{links}");
}

#[test]
fn writing_c_4_3_the_same_link_twice_changes_nothing_and_says_so() {
    let copy = Temp::git_sample("link_twice");
    let before = files_of(copy.path());
    let commits = log(copy.path());
    let said = ask_one(copy.path(), "link", r#"{"from":"M-0002","to":"M-0001"}"#);
    assert_eq!(said, "M-0002 [home] already links to M-0001 [home]: nothing changed, and no commit was made.");
    assert_eq!(files_of(copy.path()), before);
    assert_eq!(log(copy.path()), commits);
}

// ---------------------------------------------------------------------------------------------
// writing C-4.4 one commit a change, its message, the person's own uncommitted change left; a repository
// with its own history and the folder's counter (4 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn writing_c_4_4_each_change_is_exactly_one_commit_of_lorisse_s_holding_its_piece_alone() {
    let copy = Temp::git_sample("one_commit");
    let calls = [
        ("remember", GUTTER, "memories/M-0007.md"),
        ("note", r#"{"piece":"M-0002","text":"Checked."}"#, "memories/M-0002.md"),
        ("link", r#"{"from":"M-0003","to":"M-0001"}"#, "memories/M-0003.md"),
    ];
    for (tool, args, file) in calls {
        let before = log(copy.path()).len();
        ask_one(copy.path(), tool, args);
        let after = log(copy.path());
        assert_eq!(after.len(), before + 1, "{tool}: one commit");
        assert!(after[0].contains(" lorisse@local "), "{tool}: {}", after[0]);
        assert_eq!(files_of_head(copy.path()), [file], "{tool}");
        assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "", "{tool}: nothing else changed");
    }
}

#[test]
fn writing_c_4_4_the_message_says_what_was_done_and_names_the_piece() {
    let copy = Temp::git_sample("message");
    ask_one(copy.path(), "remember", GUTTER);
    assert_eq!(subject_of_head(copy.path()), "remember(M-0007): The gutter was cleaned");
    ask_one(copy.path(), "note", r#"{"piece":"M-0002","text":"Checked again."}"#);
    assert_eq!(subject_of_head(copy.path()), "note(M-0002): Checked again.");
    ask_one(copy.path(), "link", r#"{"from":"Sam Example","to":"M-0002"}"#);
    assert_eq!(subject_of_head(copy.path()), "link(people/sam-example): to M-0002");
}

#[test]
fn writing_c_4_4_a_change_the_person_has_not_committed_is_left_as_it_was_and_not_in_the_commit() {
    let copy = Temp::git_sample("theirs");
    let mine = format!("{}A line of the person's own.\n", read_file(copy.path(), "memories/M-0004.md"));
    copy.write("memories/M-0004.md", &mine);
    copy.write("notes.txt", "the person's own file\n");
    ask_one(copy.path(), "remember", GUTTER);
    assert_eq!(files_of_head(copy.path()), ["memories/M-0007.md"]);
    assert_eq!(read_file(copy.path(), "memories/M-0004.md"), mine);
    let status = run_git(copy.path(), &["status", "--porcelain"]);
    assert_eq!(status, " M memories/M-0004.md\n?? notes.txt\n");
    let before = files_of(copy.path());
    let said = ask_one(copy.path(), "note", r#"{"piece":"M-0004","text":"x"}"#);
    assert!(said.contains("memories/M-0004.md has changes not committed"), "{said}");
    assert_eq!(files_of(copy.path()), before);
}

#[test]
fn writing_c_4_4_a_repository_with_its_own_history_and_the_folder_s_counter_leaves_nothing_uncommitted() {
    let copy = Temp::sample("counter");
    let config = read_file(copy.path(), "config.yaml");
    copy.write("config.yaml", &format!("{config}  - {{ name: shed, folder: shed }}\n"));
    copy.write("shed/notes.txt", "the shed's own history\n");
    copy.write(".gitignore", "shed/\n");
    let shed = copy.path().join("shed");
    make_git(&shed);
    make_git(copy.path());
    let clean = |dir: &Path, when: &str| assert_eq!(run_git(dir, &["status", "--porcelain"]), "", "{when}");
    let args = r#"{"title":"The shed's roof","text":"Tarred in May.","repository":"shed"}"#;
    let said = ask_one(copy.path(), "remember", args);
    assert!(said.starts_with("Filed as M-0007 [shed]") && said.contains("one commit of shed"), "{said}");
    assert_eq!(files_of_head(&shed), ["memories/M-0007.md"]);
    assert!(log(&shed)[0].contains(" lorisse@local remember(M-0007)"), "{:?}", log(&shed));
    let root = log(copy.path());
    assert_eq!(root.len(), 2, "the counter is one more commit at the root: {root:?}");
    assert!(root[0].contains(" lorisse@local remember(M-0007): the folder's counter"), "{}", root[0]);
    assert_eq!(files_of_head(copy.path()), ["ids.json"]);
    assert!(read_file(copy.path(), "ids.json").contains("\"M\": 7"));
    clean(copy.path(), "after remember, the root");
    clean(&shed, "after remember, the shed");
    let counter: String = root[0].chars().take(7).collect();
    let refused = ask_one(copy.path(), "undo", &format!(r#"{{"change":"{counter}"}}"#));
    assert!(refused.contains("keeps the folder's counter"), "{refused}");
    let said = ask_one(copy.path(), "undo", "{}");
    assert!(said.starts_with("Taken back: \"remember(M-0007): The shed's roof\""), "{said}");
    assert!(!shed.join("memories/M-0007.md").exists());
    assert_eq!(log(copy.path()), root, "the counter is not taken back");
    clean(copy.path(), "after undo, the root");
    clean(&shed, "after undo, the shed");
    let again = ask_one(copy.path(), "remember", r#"{"title":"The hedge","text":"Cut in May."}"#);
    assert!(again.starts_with("Filed as M-0008 [home]"), "a number is given once: {again}");
}

// ---------------------------------------------------------------------------------------------
// writing C-4.5 the last change taken back, a named one, a commit that is not Lorisse's (3 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn writing_c_4_5_undo_takes_back_the_last_change_as_a_new_commit_and_keeps_the_history() {
    let copy = Temp::git_sample("undo_last");
    ask_one(copy.path(), "remember", GUTTER);
    let filed = log(copy.path());
    let said = ask_one(copy.path(), "undo", "{}");
    assert!(said.starts_with("Taken back: \"remember(M-0007): The gutter was cleaned\""), "{said}");
    let after = log(copy.path());
    assert_eq!(after.len(), filed.len() + 1, "a new commit");
    assert_eq!(&after[1..], &filed[..], "the history is kept");
    assert!(after[0].contains(" sample@example.invalid Revert "), "{}", after[0]);
    assert!(!copy.path().join("memories/M-0007.md").exists());
    let found = ask_one(copy.path(), "search", r#"{"text":"gutter"}"#);
    assert!(found.starts_with("Nothing in the memories"), "{found}");
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
}

#[test]
fn writing_c_4_5_undo_takes_back_the_change_named_by_its_commit_or_its_piece() {
    let copy = Temp::git_sample("undo_named");
    let before = read_file(copy.path(), "memories/M-0002.md");
    ask_one(copy.path(), "note", r#"{"piece":"M-0002","text":"Checked."}"#);
    let note = run_git(copy.path(), &["rev-parse", "--short", "HEAD"]).trim().to_string();
    ask_one(copy.path(), "link", r#"{"from":"M-0003","to":"M-0001"}"#);
    let said = ask_one(copy.path(), "undo", &format!(r#"{{"change":"{note}"}}"#));
    assert!(said.starts_with("Taken back: \"note(M-0002): Checked.\""), "{said}");
    assert_eq!(read_file(copy.path(), "memories/M-0002.md"), before);
    assert!(read_file(copy.path(), "memories/M-0003.md").contains("links: [M-0001]"));
    let said = ask_one(copy.path(), "undo", r#"{"change":"m-3"}"#);
    assert!(said.starts_with("Taken back: \"link(M-0003): to M-0001\""), "{said}");
    assert!(read_file(copy.path(), "memories/M-0003.md").contains("links: []"));
}

#[test]
fn writing_c_4_5_a_commit_lorisse_did_not_make_is_never_taken_back() {
    let copy = Temp::git_sample("undo_theirs");
    ask_one(copy.path(), "note", r#"{"piece":"M-0002","text":"Checked."}"#);
    let path = copy.path().join("memories/M-0003.md");
    let theirs = read_file(copy.path(), "memories/M-0003.md").replace("The garden plan", "The garden's plan");
    fs::write(&path, &theirs).unwrap();
    run_git(copy.path(), &["commit", "-q", "-am", "my own change"]);
    let own = run_git(copy.path(), &["rev-parse", "--short", "HEAD"]).trim().to_string();
    let before = files_of(copy.path());
    let said = ask_one(copy.path(), "undo", &format!(r#"{{"change":"{own}"}}"#));
    assert!(said.contains("is not a change of Lorisse's"), "{said}");
    assert_eq!(files_of(copy.path()), before);
    let said = ask_one(copy.path(), "undo", "{}");
    assert!(said.starts_with("Taken back: \"note(M-0002): Checked.\""), "{said}");
    assert_eq!(read_file(copy.path(), "memories/M-0003.md"), theirs, "the person's change stays");
}

// ---------------------------------------------------------------------------------------------
// writing C-4.6 the number after an undo (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn writing_c_4_6_a_number_taken_back_is_never_given_again() {
    let copy = Temp::git_sample("number");
    assert!(ask_one(copy.path(), "remember", GUTTER).starts_with("Filed as M-0007"));
    let said = ask_one(copy.path(), "undo", "{}");
    assert!(said.ends_with("The number M-0007 is not given again."), "{said}");
    let again = ask_one(copy.path(), "remember", r#"{"title":"The gutter, again","text":"Cleaned."}"#);
    assert!(again.starts_with("Filed as M-0008"), "{again}");
    assert_eq!(ask_one(copy.path(), "read", r#"{"piece":"M-0007"}"#), "No piece has the number M-0007.");
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
}

// ---------------------------------------------------------------------------------------------
// writing C-4.7 a write asked in the confidential part, and in a confidential repository not open to
// Claude; a memory filed, noted and taken back in one that is open (3 checks)
// ---------------------------------------------------------------------------------------------

const CONFIDENTIAL_SAID: &str =
    "Lorisse writes nothing in a confidential part of the memory folder: nothing was written.";

#[test]
fn writing_c_4_7_nothing_is_written_in_the_confidential_part() {
    let copy = Temp::git_sample("conf_part");
    let before = files_of(copy.path());
    let commits = log(copy.path());
    let args = r#"{"title":"A secret","text":"Kept quiet.","repository":"confidential"}"#;
    assert_eq!(ask_one(copy.path(), "remember", args), CONFIDENTIAL_SAID);
    let note = ask_one(copy.path(), "note", r#"{"piece":"M-0006","text":"x"}"#);
    assert_eq!(note, "No piece has the number M-0006.");
    let link = ask_one(copy.path(), "link", r#"{"from":"M-0001","to":"M-0006"}"#);
    assert!(link.starts_with("No piece has the number M-0006, so nothing can link to it"), "{link}");
    assert_eq!(files_of(copy.path()), before);
    assert_eq!(log(copy.path()), commits);
}

#[test]
fn writing_c_4_7_nothing_is_written_in_a_repository_marked_confidential() {
    let copy = Temp::sample("conf_row");
    let config = read_file(copy.path(), "config.yaml");
    copy.write("config.yaml", &format!("{config}  - {{ name: work, folder: work, confidential: true }}\n"));
    copy.write("work/memories/M-0040.md", &memory("M-0040", "The secret plan", "A secret.", ""));
    make_git(copy.path());
    let before = files_of(copy.path());
    let commits = log(copy.path());
    let args = r#"{"title":"Another secret","text":"Kept quiet.","repository":"work"}"#;
    assert_eq!(ask_one(copy.path(), "remember", args), CONFIDENTIAL_SAID);
    let note = ask_one(copy.path(), "note", r#"{"piece":"M-0040","text":"x"}"#);
    assert_eq!(note, "No piece has the number M-0040.");
    assert_eq!(files_of(copy.path()), before);
    assert_eq!(log(copy.path()), commits);
}

#[test]
fn writing_c_4_7_a_repository_open_to_claude_is_filed_noted_and_taken_back_as_any_other() {
    let copy = Temp::git_sample("open_write");
    let commits = log(copy.path()).len();
    let args = r#"{"title":"The lease ends","text":"The printer's lease ends in June.","repository":"office"}"#;
    let said = ask_one(copy.path(), "remember", args);
    assert!(said.starts_with("Filed as O-0002 [office]") && said.contains("one commit of office"), "{said}");
    assert_eq!(files_of_head(copy.path()), ["office/memories/O-0002.md"]);
    let noted = ask_one(copy.path(), "note", r#"{"piece":"O-0001","text":"Paid for the spring."}"#);
    assert!(noted.starts_with("Noted on O-0001 [office]"), "{noted}");
    assert!(read_file(copy.path(), "office/memories/O-0001.md").ends_with(": Paid for the spring.\n"));
    let back = ask_one(copy.path(), "undo", "{}");
    assert!(back.starts_with("Taken back: \"note(O-0001): Paid for the spring.\""), "{back}");
    let back = ask_one(copy.path(), "undo", "{}");
    assert!(back.starts_with("Taken back: \"remember(O-0002)") && back.ends_with("The number O-0002 is not given again."), "{back}");
    assert!(!copy.path().join("office/memories/O-0002.md").exists());
    assert_eq!(log(copy.path()).len(), commits + 4, "four commits, the history kept");
    assert!(read_file(copy.path(), "office/ids.json").contains("\"O\": 2"));
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
}

// ---------------------------------------------------------------------------------------------
// Older pieces numbered with the folder's letter in a repository with a prefix (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn an_older_piece_with_the_folder_s_letter_is_read_as_it_is_and_a_new_memory_takes_the_prefix() {
    let copy = Temp::sample("older_letter");
    copy.write("office/memories/M-0020.md", &memory("M-0020", "The old desk", "Bought before the office had its letter.", ""));
    make_git(copy.path());
    let found = ask_one(copy.path(), "search", r#"{"text":"desk"}"#);
    assert!(found.contains("M-0020 [office] · The old desk"), "{found}");
    let read = ask_one(copy.path(), "read", r#"{"piece":"M-0020"}"#);
    assert!(read.contains("Repository: office") && read.contains("id: M-0020"), "{read}");
    let args = r#"{"title":"The new desk","text":"Bought in March.","repository":"office"}"#;
    let said = ask_one(copy.path(), "remember", args);
    assert!(said.starts_with("Filed as O-0002 [office]"), "{said}");
    let home = ask_one(copy.path(), "remember", r#"{"title":"The hedge","text":"Cut in May."}"#);
    assert!(home.starts_with("Filed as M-0007 [home]"), "the office's M-0020 is not the root's: {home}");
}

// ---------------------------------------------------------------------------------------------
// writing C-4.8 a memory filed in the named repository, and without a name (2 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn writing_c_4_8_a_memory_goes_to_the_repository_the_call_names() {
    let copy = Temp::sample("club_named");
    copy.write(".gitignore", "club/\n");
    make_git(&copy.path().join("club"));
    make_git(copy.path());
    let root_commits = log(copy.path());
    let args = r#"{"title":"The club's new lock","text":"The lock was changed.","repository":"club"}"#;
    let said = ask_one(copy.path(), "remember", args);
    assert!(said.starts_with("Filed as C-0002 [club]"), "{said}");
    let club = copy.path().join("club");
    assert!(club.join("memories/C-0002.md").is_file());
    assert_eq!(files_of_head(&club), ["memories/C-0002.md"]);
    assert!(log(&club)[0].contains(" lorisse@local remember(C-0002)"), "{:?}", log(&club));
    assert_eq!(log(copy.path()), root_commits, "nothing committed at the root");
    let tracked = Temp::git_sample("club_tracked");
    let said = ask_one(tracked.path(), "remember", args);
    assert!(said.starts_with("Filed as C-0002 [club]"), "{said}");
    assert_eq!(files_of_head(tracked.path()), ["club/memories/C-0002.md"]);
}

#[test]
fn writing_c_4_8_without_a_name_a_memory_goes_to_the_folder_s_own_repository() {
    let copy = Temp::git_sample("club_unnamed");
    let said = ask_one(copy.path(), "remember", r#"{"title":"The hedge","text":"Cut in May."}"#);
    assert!(said.starts_with("Filed as M-0007 [home]") && said.contains("one commit of home"), "{said}");
    assert_eq!(files_of_head(copy.path()), ["memories/M-0007.md"]);
}

// ---------------------------------------------------------------------------------------------
// writing C-4.9 an unknown kind, a link to no piece (2 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn writing_c_4_9_an_unknown_kind_is_refused_with_its_reason_and_nothing_is_written() {
    let copy = Temp::git_sample("kind");
    let before = files_of(copy.path());
    let commits = log(copy.path());
    let said = ask_one(copy.path(), "remember", r#"{"title":"A rumour","text":"Heard.","kind":"rumour"}"#);
    assert_eq!(
        said,
        "Nothing was written: the kind rumour is not one this memory folder knows (fact, decision, preference, idea, need)."
    );
    assert_eq!(files_of(copy.path()), before);
    assert_eq!(log(copy.path()), commits);
}

#[test]
fn writing_c_4_9_a_link_to_a_piece_that_does_not_exist_is_refused_and_nothing_is_written() {
    let copy = Temp::git_sample("dangling");
    let before = files_of(copy.path());
    let commits = log(copy.path());
    let link = ask_one(copy.path(), "link", r#"{"from":"M-0001","to":"M-0099"}"#);
    let remember = ask_one(copy.path(), "remember", r#"{"title":"x","text":"y","links":["M-0099"]}"#);
    for said in [&link, &remember] {
        assert_eq!(said, "No piece has the number M-0099, so nothing can link to it: nothing was written.");
    }
    assert_eq!(files_of(copy.path()), before);
    assert_eq!(log(copy.path()), commits);
}

// ---------------------------------------------------------------------------------------------
// opening C-4.1 the intro page given whole, changed between two calls, absent (3 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn opening_c_4_1_plop_gives_the_intro_page_whole() {
    let copy = Temp::sample("intro_whole");
    let intro = read_file(copy.path(), "intro.md");
    let got = ask_one(copy.path(), "plop", "{}");
    let want = format!(
        "The page below is the person's own word on how Lorisse works for them (intro.md, at the root of their memory folder): follow it.\n{}\n",
        intro.trim_end()
    );
    assert!(got.starts_with(&want), "{got}");
}

#[test]
fn opening_c_4_1_the_intro_changed_between_two_calls_is_the_new_text() {
    let copy = Temp::sample("intro_changed");
    let setting = copy.path().to_string_lossy().into_owned();
    let mut live = Live::start(&[(SETTING, &setting)]);
    assert!(live.ask("plop", "{}").contains("Answer short, in plain words"));
    copy.write("intro.md", "Answer in one line, always.\n");
    let second = live.ask("plop", "{}");
    assert!(second.contains("\nAnswer in one line, always.\n"), "{second}");
    assert!(!second.contains("Answer short, in plain words"), "{second}");
}

#[test]
fn opening_c_4_1_without_the_page_plop_gives_the_plugin_s_own_and_where_to_write_theirs() {
    let copy = Temp::sample("intro_absent");
    fs::remove_file(copy.path().join("intro.md")).unwrap();
    let got = ask_one(copy.path(), "plop", "{}");
    let first = got.lines().next().unwrap();
    assert_eq!(first, "The person has written no intro.md at the root of their memory folder yet: below is the plugin's own.");
    let shown = copy.path().to_string_lossy().replace('\\', "/");
    assert!(got.contains(&format!("makes the page intro.md at the root of {shown}")), "{got}");
    assert!(!copy.path().join("intro.md").exists(), "plop writes nothing");
}

// ---------------------------------------------------------------------------------------------
// opening C-4.2 the state after the intro (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn opening_c_4_2_plop_gives_the_folder_its_repositories_the_counts_and_the_five_filed_last() {
    let copy = Temp::git_sample("state");
    ask_one(copy.path(), "remember", GUTTER);
    let got = ask_one(copy.path(), "plop", "{}");
    let shown = copy.path().to_string_lossy().replace('\\', "/");
    let state = got.split("\n\n").find(|p| p.starts_with("The memory folder: ")).unwrap();
    let want = [
        format!("The memory folder: {shown}"),
        "Its repositories, each with what it is for:".to_string(),
        "- home: The house, the garden and the people who work on them.".to_string(),
        "- club: The sports club: its keys, its meetings, its members.".to_string(),
        "- office (confidential, kept apart): The work at the office: its equipment and its leases.".to_string(),
        "It holds 6 memories and 1 person.".to_string(),
        DATA_LINE.to_string(),
        "Filed last:".to_string(),
        "- M-0007 [home] · The gutter was cleaned".to_string(),
        "- M-0002 [home] · Boiler serviced in March".to_string(),
        "- C-0001 [club] · The club's key".to_string(),
        "- M-0003 [home] · The garden plan".to_string(),
        "- M-0001 [home] · The boiler's contract".to_string(),
    ];
    assert_eq!(state.lines().collect::<Vec<_>>(), want, "{got}");
    assert!(!got.contains("M-0006") && !got.contains("surprise"), "{got}");
}

// ---------------------------------------------------------------------------------------------
// opening C-4.3 the intro said as the person's word, a memory's text as data (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn opening_c_4_3_the_intro_is_said_as_the_person_s_word_and_the_rest_as_data() {
    let copy = Temp::sample("intro_word");
    let got = ask_one(copy.path(), "plop", "{}");
    let lines: Vec<&str> = got.lines().collect();
    assert!(lines[0].starts_with("The page below is the person's own word"), "{got}");
    let intro_at = lines.iter().position(|l| *l == "# How Lorisse works for me").unwrap();
    let data_at = lines.iter().position(|l| *l == DATA_LINE).expect("the titles are said as data");
    assert!(intro_at < data_at, "the intro first, the folder's data after: {got}");
    assert!(lines[data_at + 1..].iter().any(|l| l.contains("The boiler's contract")), "{got}");
    let read = ask_one(copy.path(), "read", r#"{"piece":"M-0001"}"#);
    assert_eq!(read.lines().next(), Some(DATA_LINE));
}
