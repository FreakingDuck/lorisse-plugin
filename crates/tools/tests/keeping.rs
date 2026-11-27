//! The VALIDATION.md tables for keeping the memory true and the journal, one test a check:
//! keeping C-4.1 to C-4.3, opening C-5.1 and keeping C-5.1 to C-5.3. The program is started and asked over stdin, without
//! Claude, on temporary copies of the sample, made git folders when a check reads the history.

mod common;

use std::fs;
use std::path::Path;

use common::*;

fn read_file(dir: &Path, rel: &str) -> String {
    fs::read_to_string(dir.join(rel)).unwrap()
}

/// The files the newest commit of `dir` touched.
fn files_of_head(dir: &Path) -> Vec<String> {
    let out = run_git(dir, &["show", "--name-only", "--format=", "HEAD"]);
    out.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect()
}

/// The subject of the newest commit.
fn subject_of_head(dir: &Path) -> String {
    run_git(dir, &["log", "-1", "--format=%s"]).trim().to_string()
}

/// The day of the newest commit, as `YYYY-MM-DD`: the date the test reads, never a fixed one.
fn day_of_head(dir: &Path) -> String {
    run_git(dir, &["log", "-1", "--format=%ad", "--date=short"]).trim().to_string()
}

/// A file's text after its header.
fn body(text: &str) -> String {
    text.splitn(3, "---\n").nth(2).unwrap_or_default().to_string()
}

/// A file's header lines, `updated:` left out.
fn header_but_updated(text: &str) -> Vec<String> {
    let head = text.splitn(3, "---\n").nth(1).unwrap_or_default();
    head.lines().filter(|l| !l.starts_with("updated: ")).map(String::from).collect()
}

/// Whether `line` is a journal entry `- HH:MM <text>`.
fn timed(line: &str, text: &str) -> bool {
    let Some(rest) = line.strip_prefix("- ") else { return false };
    let time: Vec<char> = rest.chars().take(5).collect();
    let digits = [0, 1, 3, 4].iter().all(|i| time.get(*i).is_some_and(|c| c.is_ascii_digit()));
    digits && time.get(2) == Some(&':') && rest.chars().skip(5).collect::<String>() == format!(" {text}")
}

/// The state's line of the counts, as `plop` gives it.
fn counts(folder: &Path) -> String {
    let got = ask_one(folder, "plop", "{}");
    got.lines().find(|l| l.starts_with("It holds ")).unwrap_or_default().to_string()
}

// ---------------------------------------------------------------------------------------------
// keeping C-4.1 a memory's text changed and read back, a title and tags changed with the text left; a
// person's text; one commit, and the text before back after an undo (3 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn keeping_c_4_1_a_memory_s_text_is_changed_and_read_back_the_rest_as_it_was() {
    let copy = Temp::git_sample("change_text");
    let before = read_file(copy.path(), "memories/M-0002.md");
    let args = r#"{"piece":"m-2","text":"The boiler was serviced in April, not in March."}"#;
    let said = ask_one(copy.path(), "change", args);
    assert!(said.starts_with("Changed M-0002 [home]: its text, the rest as it was, in one commit of home"), "{said}");
    let after = read_file(copy.path(), "memories/M-0002.md");
    assert_eq!(header_but_updated(&after), header_but_updated(&before), "only the text: {after}");
    assert_eq!(body(&after), "The boiler was serviced in April, not in March.\n");
    let read = ask_one(copy.path(), "read", r#"{"piece":"M-0002"}"#);
    assert!(read.contains("serviced in April, not in March") && !read.contains("Nothing to repair"), "{read}");
    assert!(read.contains("title: Boiler serviced in March"), "{read}");
}

#[test]
fn keeping_c_4_1_a_title_and_tags_are_changed_and_the_text_is_left() {
    let copy = Temp::git_sample("change_title");
    let before = read_file(copy.path(), "memories/M-0001.md");
    let args = r#"{"piece":"M-0001","title":"The boiler's service contract","tags":["home","admin"]}"#;
    let said = ask_one(copy.path(), "change", args);
    assert!(said.starts_with("Changed M-0001 [home]: its title and its tags, the rest as it was"), "{said}");
    let after = read_file(copy.path(), "memories/M-0001.md");
    assert!(after.contains("title: The boiler's service contract\n") && after.contains("tags: [home, admin]\n"), "{after}");
    assert!(after.contains("kind: fact\n") && after.contains("links: []\n"), "{after}");
    assert_eq!(body(&after), body(&before), "the text is left");
    let found = ask_one(copy.path(), "search", r#"{"text":"service contract"}"#);
    assert!(found.contains("M-0001 [home] · The boiler's service contract"), "{found}");
}

#[test]
fn keeping_c_4_1_a_person_s_text_is_one_commit_and_undo_brings_the_text_before_back() {
    let copy = Temp::git_sample("change_person");
    let before = read_file(copy.path(), "people/sam-example.md");
    let commits = log(copy.path()).len();
    let args = r###"{"piece":"Sam Example","text":"## What I know\nA plumber; no longer does boilers.\n## How I handle\nCall after ten."}"###;
    let said = ask_one(copy.path(), "change", args);
    assert!(said.starts_with("Changed people/sam-example [home]: its text"), "{said}");
    assert_eq!(log(copy.path()).len(), commits + 1, "one commit");
    assert!(log(copy.path())[0].contains(" lorisse@local "), "{:?}", log(copy.path()));
    assert_eq!(files_of_head(copy.path()), ["people/sam-example.md"]);
    assert_eq!(subject_of_head(copy.path()), "edit(people/sam-example): text changed");
    assert!(read_file(copy.path(), "people/sam-example.md").contains("no longer does boilers"));
    assert!(read_file(copy.path(), "people/sam-example.md").contains("name: Sam Example\n"));
    let back = ask_one(copy.path(), "undo", "{}");
    assert!(back.starts_with("Taken back: \"edit(people/sam-example): text changed\""), "{back}");
    assert_eq!(read_file(copy.path(), "people/sam-example.md"), before, "the text before is back");
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
}

// ---------------------------------------------------------------------------------------------
// keeping C-4.2 a memory retired with its reason: still read, said as retired; last in a search; left
// out of the count (3 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn keeping_c_4_2_a_retired_memory_is_still_read_said_as_retired_with_its_reason() {
    let copy = Temp::git_sample("retire_read");
    let args = r#"{"piece":"M-0001","reason":"The contract ended in May."}"#;
    let said = ask_one(copy.path(), "retire", args);
    assert!(said.starts_with("Retired M-0001 [home], \"The boiler's contract\": The contract ended in May."), "{said}");
    assert!(said.contains("Nothing is deleted") && said.contains("in one commit of home"), "{said}");
    assert_eq!(files_of_head(copy.path()), ["memories/M-0001.md"]);
    assert_eq!(subject_of_head(copy.path()), "retire(M-0001): The contract ended in May.");
    let file = read_file(copy.path(), "memories/M-0001.md");
    assert!(file.contains("status: retired\n") && file.contains("retired_why: The contract ended in May.\n"), "{file}");
    let read = ask_one(copy.path(), "read", r#"{"piece":"M-0001"}"#);
    assert_eq!(read.lines().next(), Some(DATA_LINE), "{read}");
    assert!(read.contains("M-0001 [home] · The boiler's contract (retired)"), "{read}");
    assert!(read.contains("Retired, it no longer holds: The contract ended in May."), "{read}");
    assert!(read.contains("The boiler has a yearly service contract."), "the text is kept: {read}");
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
}

#[test]
fn keeping_c_4_2_a_retired_memory_comes_last_in_a_search_said_as_retired() {
    let copy = Temp::git_sample("retire_search");
    ask_one(copy.path(), "retire", r#"{"piece":"M-0001","reason":"The contract ended."}"#);
    let found = ask_one(copy.path(), "search", r#"{"text":"boiler"}"#);
    let listed: Vec<&str> = found.lines().filter(|l| l.contains(" · ")).collect();
    let last = listed.last().unwrap();
    assert!(last.starts_with("M-0001 [home] · The boiler's contract (retired)"), "{found}");
    assert!(listed.iter().any(|l| l.starts_with("M-0002 [home]")), "{found}");
    assert_eq!(listed.iter().filter(|l| l.contains("(retired)")).count(), 1, "{found}");
}

#[test]
fn keeping_c_4_2_the_count_plop_gives_leaves_the_retired_out() {
    let copy = Temp::git_sample("retire_count");
    assert_eq!(counts(copy.path()), "It holds 5 memories and 1 person.", "the sample's M-0004 is retired");
    ask_one(copy.path(), "retire", r#"{"piece":"M-0003","reason":"The garden stays a lawn."}"#);
    assert_eq!(counts(copy.path()), "It holds 4 memories and 1 person.");
    assert!(copy.path().join("memories/M-0003.md").is_file(), "nothing is deleted");
}

// ---------------------------------------------------------------------------------------------
// keeping C-4.3 a memory retired for another: the two linked, the other named (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn keeping_c_4_3_a_memory_retired_for_another_is_linked_to_it_and_names_it() {
    let copy = Temp::git_sample("retire_for");
    let filed = ask_one(copy.path(), "remember", r#"{"title":"The new boiler contract","text":"Signed with another firm."}"#);
    assert!(filed.starts_with("Filed as M-0007 [home]"), "{filed}");
    let args = r#"{"piece":"M-0001","reason":"A new contract replaced it.","replaced_by":"M-0007"}"#;
    let said = ask_one(copy.path(), "retire", args);
    assert!(said.contains("M-0007 [home] takes its place, and the two are linked"), "{said}");
    assert_eq!(files_of_head(copy.path()), ["memories/M-0001.md"], "one commit");
    let file = read_file(copy.path(), "memories/M-0001.md");
    assert!(file.contains("links: [M-0007]\n") && file.contains("retired_by: M-0007\n"), "{file}");
    let read = ask_one(copy.path(), "read", r#"{"piece":"M-0001"}"#);
    assert!(read.contains("Its place is taken by M-0007 [home] · The new boiler contract"), "{read}");
    let links = ask_one(copy.path(), "links", r#"{"piece":"M-0007"}"#);
    assert!(links.contains("M-0001 [home] · The boiler's contract"), "{links}");
}

// ---------------------------------------------------------------------------------------------
// opening C-5.1 the two rules among those `plop` gives (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn opening_c_5_1_the_rules_of_plop_say_a_fact_is_changed_and_what_no_longer_holds_is_retired() {
    let copy = Temp::sample("keeping_rules");
    let got = ask_one(copy.path(), "plop", "{}");
    let rules = got.split("\n\n").last().unwrap();
    assert!(rules.starts_with("The rules of keeping:"), "{got}");
    for said in [
        "- A fact that changed is changed (change), not filed a second time.",
        "- What no longer holds is retired (retire), never left beside its contrary.",
    ] {
        assert!(rules.lines().any(|l| l == said), "{said}: {rules}");
    }
}

// ---------------------------------------------------------------------------------------------
// keeping C-5.1 an entry added to today's page and to a named day, what was there left; one commit and
// its undo; in a named repository (3 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn keeping_c_5_1_an_entry_goes_at_the_end_of_today_s_page_and_of_a_named_day_what_was_there_left() {
    let copy = Temp::git_sample("journal_pages");
    let said = ask_one(copy.path(), "journal", r#"{"text":"The plumber came this morning."}"#);
    assert!(said.starts_with("Written in the journal of ") && said.contains("the page made for it"), "{said}");
    let today = day_of_head(copy.path());
    let page = read_file(copy.path(), &format!("journal/{today}.md"));
    assert!(page.contains(&format!("date: {today}\n")) && page.contains("kind: diary\n"), "{page}");
    assert!(timed(body(&page).trim_end(), "The plumber came this morning."), "{page}");
    let before = read_file(copy.path(), "journal/2019-05-14.md");
    let said = ask_one(copy.path(), "journal", r#"{"text":"Decided to keep the old ladder.","day":"2019-05-14"}"#);
    assert!(said.starts_with("Written in the journal of 2019-05-14 [home] at "), "{said}");
    assert!(!said.contains("page made"), "{said}");
    let after = body(&read_file(copy.path(), "journal/2019-05-14.md"));
    assert!(after.starts_with(&body(&before)), "what was there is left: {after}");
    assert!(timed(after.lines().last().unwrap(), "Decided to keep the old ladder."), "{after}");
    assert_eq!(after.lines().count(), 3, "{after}");
}

#[test]
fn keeping_c_5_1_an_entry_is_one_commit_and_undo_takes_it_back() {
    let copy = Temp::git_sample("journal_undo");
    let commits = log(copy.path()).len();
    let said = ask_one(copy.path(), "journal", r#"{"text":"The roof was checked.","day":"2031-03-03"}"#);
    assert!(said.contains("in one commit of home") && said.ends_with("undo takes it back."), "{said}");
    assert_eq!(log(copy.path()).len(), commits + 1);
    assert!(log(copy.path())[0].contains(" lorisse@local "), "{:?}", log(copy.path()));
    assert_eq!(files_of_head(copy.path()), ["journal/2031-03-03.md"]);
    assert_eq!(subject_of_head(copy.path()), "note(journal/2031-03-03): The roof was checked.");
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
    let back = ask_one(copy.path(), "undo", "{}");
    assert!(back.starts_with("Taken back: \"note(journal/2031-03-03): The roof was checked.\""), "{back}");
    assert!(!copy.path().join("journal/2031-03-03.md").exists());
    assert_eq!(log(copy.path()).len(), commits + 2, "the history keeps both");
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
}

#[test]
fn keeping_c_5_1_an_entry_goes_to_the_repository_the_call_names() {
    let copy = Temp::git_sample("journal_club");
    let args = r#"{"text":"The club's lock was oiled.","day":"2031-03-04","repository":"club"}"#;
    let said = ask_one(copy.path(), "journal", args);
    assert!(said.starts_with("Written in the journal of 2031-03-04 [club] at "), "{said}");
    assert_eq!(files_of_head(copy.path()), ["club/journal/2031-03-04.md"]);
    let read = ask_one(copy.path(), "read", r#"{"piece":"2031-03-04"}"#);
    assert!(read.contains("The journal of 2031-03-04 [club]") && read.contains("The club's lock was oiled."), "{read}");
    let home = ask_one(copy.path(), "journal", r#"{"text":"The hedge was cut.","day":"2031-03-05"}"#);
    assert!(home.starts_with("Written in the journal of 2031-03-05 [home]"), "{home}");
    assert_eq!(files_of_head(copy.path()), ["journal/2031-03-05.md"]);
}

// ---------------------------------------------------------------------------------------------
// keeping C-5.2 a day's page read; an entry found by a search, said with its day (2 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn keeping_c_5_2_read_given_a_day_gives_its_page_and_a_confidential_page_stays_unseen() {
    let copy = Temp::sample("journal_read");
    copy.write("confidential/journal/2019-06-01.md", "---\ndate: 2019-06-01\nkind: diary\n---\n- 10:00 A surprise is planned.\n");
    for asked in ["2019-05-14", "journal/2019-05-14"] {
        let read = ask_one(copy.path(), "read", &format!(r#"{{"piece":"{asked}"}}"#));
        let lines: Vec<&str> = read.lines().collect();
        assert_eq!(lines[0], DATA_LINE, "{read}");
        assert_eq!(lines[1], "The journal of 2019-05-14 [home]", "{read}");
        assert_eq!(lines[2], "- 08:40 The window cleaner came and did the front of the house.", "{read}");
        assert_eq!(lines[3], "- 17:15 Decided to paint the garden shed green in the summer.", "{read}");
    }
    let hidden = ask_one(copy.path(), "read", r#"{"piece":"2019-06-01"}"#);
    assert_eq!(hidden, "The journal has no page for 2019-06-01.");
}

#[test]
fn keeping_c_5_2_search_finds_in_the_journal_each_match_said_with_its_day() {
    let copy = Temp::sample("journal_search");
    copy.write("confidential/journal/2019-06-01.md", "---\ndate: 2019-06-01\nkind: diary\n---\n- 10:00 The shed surprise.\n");
    let found = ask_one(copy.path(), "search", r#"{"text":"window cleaner"}"#);
    assert_eq!(found.lines().next(), Some(DATA_LINE), "{found}");
    assert!(found.contains("The journal of 2019-05-14 [home] · - 08:40 The window cleaner came"), "{found}");
    let shed = ask_one(copy.path(), "search", r#"{"text":"shed"}"#);
    assert!(shed.contains("The journal of 2019-05-14 [home] · - 17:15 Decided to paint the garden shed"), "{shed}");
    assert!(!shed.contains("2019-06-01") && !shed.contains("surprise"), "the confidential page stays unseen: {shed}");
}

// ---------------------------------------------------------------------------------------------
// keeping C-5.3 the last day written, said by `plop` (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn keeping_c_5_3_plop_says_after_the_state_the_last_day_the_journal_was_written() {
    let copy = Temp::git_sample("journal_plop");
    let got = ask_one(copy.path(), "plop", "{}");
    let said = "The journal was last written on 2019-05-14.";
    assert!(got.contains(&format!("\n{said}\n")), "{got}");
    assert!(got.find("It holds ").unwrap() < got.find(said).unwrap(), "after the state: {got}");
    assert!(got.find(said).unwrap() < got.find("The rules of keeping:").unwrap(), "{got}");
    ask_one(copy.path(), "journal", r#"{"text":"The gate was painted.","day":"2031-04-01"}"#);
    let again = ask_one(copy.path(), "plop", "{}");
    assert!(again.contains("\nThe journal was last written on 2031-04-01.\n"), "{again}");
    let empty = Temp::sample("journal_none");
    fs::remove_dir_all(empty.path().join("journal")).unwrap();
    let none = ask_one(empty.path(), "plop", "{}");
    assert!(none.contains("\nThe journal has no page yet.\n"), "{none}");
}
