//! The VALIDATION.md tables for the drawers, one test a check: drawers C-4.1 to C-4.3, opening
//! C-6.1 and C-6.2, first_day C-5.1, the two
//! Behaviour rows of `drawer` and `move`, and the person's settings kept line by line. The
//! program is started and asked over stdin, without Claude, on temporary copies of the sample,
//! made git folders when a check reads the history; `start` runs with a home of its own.

mod common;

use std::fs;
use std::path::Path;

use common::*;

fn read_file(dir: &Path, rel: &str) -> String {
    fs::read_to_string(dir.join(rel)).unwrap()
}

fn files_of_head(dir: &Path) -> Vec<String> {
    let out = run_git(dir, &["show", "--no-renames", "--name-only", "--format=", "HEAD"]);
    out.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect()
}

fn subject_of_head(dir: &Path) -> String {
    run_git(dir, &["log", "-1", "--format=%s"]).trim().to_string()
}

fn head(dir: &Path) -> String {
    run_git(dir, &["rev-parse", "HEAD"]).trim().to_string()
}

fn shown(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// The lines of the state `plop` gives that list the repositories.
fn repo_lines(got: &str) -> Vec<String> {
    let state = got.split("\n\n").find(|p| p.starts_with("The memory folder: ")).unwrap();
    let lines: Vec<&str> = state.lines().collect();
    let from = lines.iter().position(|l| *l == "Its repositories, each with what it is for:").expect("the repositories' heading");
    lines[from + 1..].iter().take_while(|l| l.starts_with("- ")).map(|l| l.to_string()).collect()
}

const GARDEN: &str = r#"{"name":"Garden work","about":"The garden design clients, their plans and their budgets."}"#;

const GARDEN_LINE: &str = "  - { name: Garden work, folder: garden-work, prefix: G, about: \"The garden design clients, their plans and their budgets.\" }";

// ---------------------------------------------------------------------------------------------
// drawers C-4.1 a repository made: its folder, its numbering, its line in the settings with the lines
// there left as they were; one commit and its undo; a memory filed in it at once (3 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn drawers_c_4_1_a_drawer_is_made_its_folder_its_numbering_and_its_line_the_lines_there_left() {
    let copy = Temp::git_sample("drawer_made");
    let before = read_file(copy.path(), "config.yaml");
    let commits = log(copy.path()).len();
    let said = ask_one(copy.path(), "drawer", GARDEN);
    assert!(said.starts_with("Made the repository Garden work: its folder garden-work, its memories numbered G-0001 on, in one commit of home ("), "{said}");
    assert!(said.ends_with("File in it with \"repository\": \"Garden work\"."), "{said}");
    assert_eq!(read_file(copy.path(), "garden-work/ids.json"), "{\n  \"G\": 0\n}\n");
    let after = read_file(copy.path(), "config.yaml");
    let mut want: Vec<&str> = before.lines().collect();
    want.push(GARDEN_LINE);
    assert_eq!(after.lines().collect::<Vec<_>>(), want, "one line added after the last row, the rest as it was");
    assert_eq!(log(copy.path()).len(), commits + 1, "one commit");
    assert!(log(copy.path())[0].contains(" lorisse@local "), "{:?}", log(copy.path()));
    assert_eq!(files_of_head(copy.path()), ["config.yaml", "garden-work/ids.json"]);
    assert_eq!(subject_of_head(copy.path()), "drawer(garden-work): the repository Garden work made, numbered G");
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
}

#[test]
fn drawers_c_4_1_undo_takes_a_drawer_back_and_refuses_once_something_is_filed_in_it() {
    let copy = Temp::git_sample("drawer_undo");
    let before = files_of(copy.path());
    ask_one(copy.path(), "drawer", GARDEN);
    let back = ask_one(copy.path(), "undo", "{}");
    assert!(back.starts_with("Taken back: \"drawer(garden-work): the repository Garden work made, numbered G\""), "{back}");
    assert_eq!(files_of(copy.path()), before, "the line and the folder are gone");
    assert!(!copy.path().join("garden-work").exists());
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
    ask_one(copy.path(), "drawer", GARDEN);
    let made = head(copy.path());
    let filed = ask_one(copy.path(), "remember", r#"{"title":"The house on the hill","text":"A first client.","repository":"Garden work"}"#);
    assert!(filed.starts_with("Filed as G-0001 [Garden work]"), "{filed}");
    let held = files_of(copy.path());
    let refused = ask_one(copy.path(), "undo", &format!(r#"{{"change":"{made}"}}"#));
    assert!(refused.starts_with("Nothing was written: something was filed in the repository Garden work since it was made"), "{refused}");
    assert_eq!(refused.matches(". ").count(), 0, "one sentence: {refused}");
    assert_eq!(files_of(copy.path()), held, "nothing written");
}

#[test]
fn drawers_c_4_1_a_memory_is_filed_in_a_new_drawer_at_the_next_call_and_without_git_it_is_made_all_the_same() {
    let copy = Temp::git_sample("drawer_at_once");
    let setting = copy.path().to_string_lossy().into_owned();
    let mut live = Live::start(&[(SETTING, &setting)]);
    live.ask("drawer", r#"{"name":"Clients","about":"Each client of the garden design work."}"#);
    let filed = live.ask("remember", r#"{"title":"The house on the hill","text":"Her budget is 4000.","repository":"Clients"}"#);
    assert!(copy.path().join("clients/memories").is_dir(), "filed in its folder");
    let config = read_file(copy.path(), "config.yaml");
    assert!(config.contains("name: Clients, folder: clients, prefix: CL,"), "C is the club's, so two letters: {config}");
    assert!(filed.starts_with("Filed as CL-0001 [Clients]"), "{filed}");
    let plain = Temp::sample("drawer_no_git");
    let said = ask_one(plain.path(), "drawer", GARDEN);
    assert!(said.starts_with("Made the repository Garden work: its folder garden-work, its memories numbered G-0001 on. File in it"), "{said}");
    assert!(plain.path().join("garden-work/ids.json").is_file());
    assert!(read_file(plain.path(), "config.yaml").contains(GARDEN_LINE));
    let filed = ask_one(plain.path(), "remember", r#"{"title":"A plan","text":"Roses on the left.","repository":"Garden work"}"#);
    assert!(filed.starts_with("Filed as G-0001 [Garden work]"), "{filed}");
}

// ---------------------------------------------------------------------------------------------
// opening C-6.1 each repository said with what it is for by `plop` (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn opening_c_6_1_plop_says_each_repository_one_a_line_with_what_it_is_for() {
    let copy = Temp::git_sample("drawer_plop");
    let config = read_file(copy.path(), "config.yaml");
    let bare = config.replace(", about: \"The sports club: its keys, its meetings, its members.\"", "");
    assert_ne!(bare, config, "the sample says what the club is for");
    fs::write(copy.path().join("config.yaml"), bare).unwrap();
    run_git(copy.path(), &["-c", "maintenance.auto=false", "-c", "gc.auto=0", "commit", "-q", "-am", "the club says nothing"]);
    ask_one(copy.path(), "drawer", GARDEN);
    let got = ask_one(copy.path(), "plop", "{}");
    assert_eq!(
        repo_lines(&got),
        [
            "- home: The house, the garden and the people who work on them.",
            "- club",
            "- office (confidential, kept apart): The work at the office: its equipment and its leases.",
            "- Garden work: The garden design clients, their plans and their budgets.",
        ],
        "{got}"
    );
}

// ---------------------------------------------------------------------------------------------
// drawers C-4.2 a memory moved, several at once: the number, the text, the links and the files kept, a
// link to it still followed; one commit and its undo (3 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn drawers_c_4_2_a_memory_moves_with_its_number_its_text_its_header_and_its_files() {
    let copy = Temp::git_sample("move_one");
    let mine = Temp::empty("move_one_src");
    let source = mine.path().join("invoice-2031.pdf");
    fs::write(&source, b"%PDF-1.4 an invented invoice\n").unwrap();
    ask_one(copy.path(), "attach", &format!(r#"{{"piece":"M-0001","path":"{}"}}"#, shown(&source)));
    let before = read_file(copy.path(), "memories/M-0001.md");
    let said = ask_one(copy.path(), "move", r#"{"pieces":["m-1"],"repository":"Club"}"#);
    assert!(said.starts_with("Moved M-0001 to club: it keeps its number, its text, its links and its files, in one commit of home ("), "{said}");
    assert!(said.ends_with("undo puts it back."), "{said}");
    assert!(!copy.path().join("memories/M-0001.md").exists() && !copy.path().join("memories/M-0001").exists());
    let after = read_file(copy.path(), "club/memories/M-0001.md");
    assert_eq!(after, before.replace("path: memories/M-0001/", "path: club/memories/M-0001/"), "only the file's path changes");
    assert_eq!(fs::read(copy.path().join("club/memories/M-0001/invoice-2031.pdf")).unwrap(), b"%PDF-1.4 an invented invoice\n");
    let read = ask_one(copy.path(), "read", r#"{"piece":"M-0001"}"#);
    assert!(read.contains("M-0001 [club] · The boiler's contract") && read.contains("Repository: club"), "{read}");
    let last = copy.path().file_name().unwrap().to_string_lossy().into_owned();
    let file = read.lines().find(|l| l.starts_with("- invoice-2031.pdf: ")).expect("the file is named");
    assert!(file.ends_with(&format!("/{last}/club/memories/M-0001/invoice-2031.pdf")), "{read}");
    let found = ask_one(copy.path(), "search", r#"{"text":"yearly service"}"#);
    assert!(found.contains("M-0001 [club] · The boiler's contract"), "{found}");
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
}

#[test]
fn drawers_c_4_2_several_move_in_one_commit_and_a_link_to_one_still_leads_to_it() {
    let copy = Temp::git_sample("move_several");
    let commits = log(copy.path()).len();
    let said = ask_one(copy.path(), "move", r#"{"pieces":["M-0001","M-0003"],"repository":"club"}"#);
    assert!(said.starts_with("Moved M-0001 and M-0003 to club: each keeps its number"), "{said}");
    assert_eq!(log(copy.path()).len(), commits + 1, "one commit");
    assert!(log(copy.path())[0].contains(" lorisse@local "), "{:?}", log(copy.path()));
    assert_eq!(subject_of_head(copy.path()), "move(M-0001, M-0003): to club");
    assert_eq!(
        files_of_head(copy.path()),
        ["club/memories/M-0001.md", "club/memories/M-0003.md", "memories/M-0001.md", "memories/M-0003.md"]
    );
    let links = ask_one(copy.path(), "links", r#"{"piece":"M-0002"}"#);
    assert!(links.contains("- M-0001 [club] · The boiler's contract"), "the link still leads to it: {links}");
    let back = ask_one(copy.path(), "links", r#"{"piece":"M-0001"}"#);
    assert!(back.contains("Linked from:\n- M-0002 [home]"), "{back}");
}

#[test]
fn drawers_c_4_2_undo_puts_them_back_and_a_moved_number_is_never_given_again() {
    let copy = Temp::git_sample("move_undo");
    let before = files_of(copy.path());
    ask_one(copy.path(), "move", r#"{"pieces":"M-0002, M-0003","repository":"club"}"#);
    let back = ask_one(copy.path(), "undo", "{}");
    assert!(back.starts_with("Taken back: \"move(M-0002, M-0003): to club\""), "{back}");
    assert_eq!(files_of(copy.path()), before, "every file back as it was");
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
    let filed = ask_one(copy.path(), "remember", r#"{"title":"The gutter","text":"Cleaned in October."}"#);
    assert!(filed.starts_with("Filed as M-0007 [home]"), "{filed}");
    let moved = ask_one(copy.path(), "move", r#"{"pieces":["M-0007"],"repository":"club"}"#);
    assert!(moved.starts_with("Moved M-0007 to club"), "{moved}");
    assert!(files_of_head(copy.path()).contains(&"ids.json".to_string()), "the root's counter keeps it in the same commit");
    let next = ask_one(copy.path(), "remember", r#"{"title":"The fence","text":"Painted in May."}"#);
    assert!(next.starts_with("Filed as M-0008 [home]"), "M-0007 is never given again: {next}");
}

// ---------------------------------------------------------------------------------------------
// drawers C-4.3 a repository made confidential and open to Claude, read and written; a move out of a
// confidential one refused (2 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn drawers_c_4_3_a_drawer_kept_apart_is_confidential_open_to_claude_read_and_written() {
    let copy = Temp::git_sample("drawer_apart");
    let said = ask_one(copy.path(), "drawer", r#"{"name":"Health","about":"The doctors and the treatments.","apart":true}"#);
    assert!(said.starts_with("Made the repository Health, kept apart: confidential and open to Claude"), "{said}");
    let config = read_file(copy.path(), "config.yaml");
    assert!(config.ends_with("  - { name: Health, folder: health, confidential: true, providers: [anthropic], prefix: H, about: \"The doctors and the treatments.\" }\n"), "{config}");
    let filed = ask_one(copy.path(), "remember", r#"{"title":"The dentist","text":"Every six months.","repository":"Health"}"#);
    assert!(filed.starts_with("Filed as H-0001 [Health]"), "{filed}");
    let read = ask_one(copy.path(), "read", r#"{"piece":"H-0001"}"#);
    assert!(read.contains("Repository: Health") && read.contains("Every six months."), "{read}");
    let plop = ask_one(copy.path(), "plop", "{}");
    assert!(repo_lines(&plop).contains(&"- Health (confidential, kept apart): The doctors and the treatments.".to_string()), "{plop}");
}

#[test]
fn drawers_c_4_3_a_piece_goes_into_a_confidential_repository_and_never_out_of_one() {
    let copy = Temp::git_sample("move_apart");
    let into = ask_one(copy.path(), "move", r#"{"pieces":["M-0003"],"repository":"office"}"#);
    assert!(into.starts_with("Moved M-0003 to office"), "{into}");
    let before = files_of(copy.path());
    let commits = log(copy.path());
    for args in [r#"{"pieces":["O-0001"],"repository":"home"}"#, r#"{"pieces":["M-0003"],"repository":"club"}"#] {
        let said = ask_one(copy.path(), "move", args);
        assert!(said.starts_with("Nothing was written: ") && said.contains("never takes a piece out of a confidential repository"), "{said}");
        assert_eq!(said.matches(". ").count(), 0, "one sentence: {said}");
        assert_eq!(files_of(copy.path()), before, "{said}");
    }
    let hidden = ask_one(copy.path(), "move", r#"{"pieces":["M-0006"],"repository":"home"}"#);
    assert_eq!(hidden, "No piece has the number M-0006.");
    assert_eq!(log(copy.path()), commits);
}

// ---------------------------------------------------------------------------------------------
// opening C-6.2 the three rules among those `plop` gives (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn opening_c_6_2_the_rules_of_plop_say_when_a_drawer_is_made_a_piece_moved_and_never_to_ask() {
    let copy = Temp::sample("drawer_rules");
    let got = ask_one(copy.path(), "plop", "{}");
    let rules = got.split("\n\n").last().unwrap();
    assert!(rules.starts_with("The rules of keeping:"), "{got}");
    for said in [
        "- A subject that keeps coming and has no repository of its own gets one (drawer), said in one line.",
        "- A piece found in the wrong repository is moved (move).",
        "- Never ask the person where a thing goes: choose the repository yourself.",
    ] {
        assert!(rules.lines().any(|l| l == said), "{said}: {rules}");
    }
}

// ---------------------------------------------------------------------------------------------
// first_day C-5.1 what `start` tells Claude to ask and to do (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn first_day_c_5_1_start_tells_claude_to_let_the_person_talk_and_to_make_the_drawers_by_itself() {
    let home = Temp::empty("drawer_start_home");
    let global = Temp::empty("drawer_start_git");
    global.write("gitconfig", "");
    let cwd = Temp::empty("drawer_start_cwd");
    let home_text = home.path().to_string_lossy().into_owned();
    let global_text = global.path().join("gitconfig").to_string_lossy().into_owned();
    let env = [(HOME, home_text.as_str()), ("GIT_CONFIG_GLOBAL", global_text.as_str()), ("GIT_CONFIG_NOSYSTEM", "1")];
    let said = ask(&env, Some(cwd.path()), &[("start", r#"{"name":"Sam Example"}"#)]).remove(0);
    let next = said.split("\n\n").find(|p| p.starts_with("What to do next")).expect("what to do next");
    for part in [
        "open questions, a few at a time, never a form",
        "who they are, what fills their days, what they would hate to lose, what must never mix",
        "make the repositories by yourself with drawer",
        "kept apart (apart: true) when they say something must never mix",
        "File what they said as memories with remember, each in the repository it belongs to",
        "say in three lines at most what you made",
    ] {
        assert!(next.contains(part), "{part}: {next}");
    }
    assert!(said.find("What to do next").unwrap() < said.find("```").unwrap(), "before the block: {said}");
}

// ---------------------------------------------------------------------------------------------
// The two Behaviour rows of the drawers, and the settings kept line by line
// ---------------------------------------------------------------------------------------------

#[test]
fn behaviour_a_drawer_with_a_name_already_taken_is_one_sentence_naming_it_and_nothing_written() {
    let copy = Temp::git_sample("drawer_taken");
    let before = files_of(copy.path());
    let commits = log(copy.path());
    for (name, there) in [("CLUB", "club"), ("Home", "home"), ("office", "office")] {
        let said = ask_one(copy.path(), "drawer", &format!(r#"{{"name":"{name}","about":"Anything."}}"#));
        assert_eq!(said, format!("The repository {there} is there already, under that name: nothing was written; file in it, or choose another name."));
    }
    let apart = ask_one(copy.path(), "drawer", r#"{"name":"Confidential","about":"Anything."}"#);
    assert!(apart.starts_with("Nothing was written: confidential is the name of the folder's confidential part"), "{apart}");
    assert_eq!(files_of(copy.path()), before);
    assert_eq!(log(copy.path()), commits);
}

#[test]
fn behaviour_a_move_to_the_repository_a_piece_is_in_is_one_sentence_and_nothing_written() {
    let copy = Temp::git_sample("move_there");
    let before = files_of(copy.path());
    let commits = log(copy.path());
    let said = ask_one(copy.path(), "move", r#"{"pieces":["M-0002"],"repository":"HOME"}"#);
    assert_eq!(said, "M-0002 is in home already: nothing was written.");
    let both = ask_one(copy.path(), "move", r#"{"pieces":["C-0001","M-0002"],"repository":"home"}"#);
    assert!(both.starts_with("Moved C-0001 to home") && both.ends_with("M-0002 was there already."), "{both}");
    let back = ask_one(copy.path(), "undo", "{}");
    assert!(back.starts_with("Taken back: \"move(C-0001): to home\""), "{back}");
    assert_eq!(files_of(copy.path()), before);
    for (args, want) in [
        (r#"{"pieces":["M-0099"],"repository":"club"}"#, "No piece has the number M-0099."),
        (r#"{"pieces":["Sam Example"],"repository":"club"}"#, "people/sam-example is a person: only a memory is moved; nothing was written."),
        (r#"{"pieces":["M-0002"],"repository":"garage"}"#, "Nothing was written: no repository is named garage: the repositories are home, club, office."),
    ] {
        assert_eq!(ask_one(copy.path(), "move", args), want);
    }
    assert_eq!(files_of(copy.path()), before);
    assert_eq!(log(copy.path()).len(), commits.len() + 2, "the move and its undo, nothing else");
}

#[test]
fn a_settings_file_with_comments_and_an_odd_layout_keeps_every_old_line() {
    let copy = Temp::git_sample("drawer_layout");
    let odd = "# My settings, my way.\r\n\r\nkinds:   [fact, decision, preference, idea, need]   # what I keep\r\n# below, the folders\r\nrepos:\r\n    # the root first\r\n    -   name: home\r\n        folder: .\r\n\r\n    - name: club     # the club\r\n      folder: club\r\n      prefix: C\r\n\r\n# the end of the repositories\r\nsources: [phone, chat, agent, console, app]\r\nperson_sections: [What I know, How I handle]\r\n";
    fs::write(copy.path().join("config.yaml"), odd).unwrap();
    run_git(copy.path(), &["-c", "maintenance.auto=false", "-c", "gc.auto=0", "commit", "-q", "-am", "my own layout"]);
    let said = ask_one(copy.path(), "drawer", GARDEN);
    assert!(said.starts_with("Made the repository Garden work"), "{said}");
    let after = read_file(copy.path(), "config.yaml");
    let old: Vec<&str> = odd.split_inclusive('\n').collect();
    let new: Vec<&str> = after.split_inclusive('\n').collect();
    assert_eq!(new.len(), old.len() + 4, "four lines added, one key a line: {after}");
    let added = &new[12..16];
    assert_eq!(&new[..12], &old[..12], "every line before, byte for byte");
    assert_eq!(&new[16..], &old[12..], "every line after, byte for byte");
    assert_eq!(
        added,
        [
            "    - name: Garden work\r\n",
            "      folder: garden-work\r\n",
            "      prefix: G\r\n",
            "      about: \"The garden design clients, their plans and their budgets.\"\r\n",
        ]
    );
    let filed = ask_one(copy.path(), "remember", r#"{"title":"A plan","text":"Roses on the left.","repository":"Garden work"}"#);
    assert!(filed.starts_with("Filed as G-0001 [Garden work]"), "{filed}");
    let bare = Temp::git_sample("drawer_no_repos");
    let config = read_file(bare.path(), "config.yaml");
    let cut = config.find("# Three repositories").unwrap();
    let mine = config[..cut].to_string();
    fs::write(bare.path().join("config.yaml"), &mine).unwrap();
    fs::remove_dir_all(bare.path().join("office")).unwrap();
    fs::remove_dir_all(bare.path().join("club")).unwrap();
    run_git(bare.path(), &["-c", "maintenance.auto=false", "-c", "gc.auto=0", "commit", "-q", "-am", "the root alone"]);
    let root_name = bare.path().file_name().unwrap().to_string_lossy().into_owned();
    ask_one(bare.path(), "drawer", GARDEN);
    let after = read_file(bare.path(), "config.yaml");
    assert!(after.starts_with(&mine), "{after}");
    let tail = &after[mine.len()..];
    assert_eq!(tail, format!("repos:\n  - {{ name: {root_name}, folder: . }}\n{GARDEN_LINE}\n"));
    let plop = ask_one(bare.path(), "plop", "{}");
    assert!(repo_lines(&plop)[0] == format!("- {root_name}"), "the root keeps the name it had: {plop}");
}
