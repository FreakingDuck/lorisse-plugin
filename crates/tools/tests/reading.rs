//! The root VALIDATION.md's table, one test a check: the program started and asked over stdin,
//! without Claude, on temporary copies of the sample.

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::*;

fn text_of(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The folder as `where` says it.
fn shown(path: &Path) -> String {
    text_of(path).replace('\\', "/")
}

// ---------------------------------------------------------------------------------------------
// C-4.1 the session's folder when it is a memory folder, else the setting (3 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_4_1_the_setting_gives_the_folder_whatever_folder_the_program_runs_in() {
    let copy = Temp::sample("setting");
    let elsewhere = Temp::empty("setting_cwd");
    let setting = text_of(copy.path());
    let got = ask(
        &[(SETTING, &setting)],
        Some(elsewhere.path()),
        &[("where", "{}"), ("search", r#"{"text":"boiler"}"#)],
    );
    assert!(got[0].contains(&shown(copy.path())), "{}", got[0]);
    assert!(got[0].contains("the plugin's setting"), "{}", got[0]);
    assert!(got[0].contains("0.7.0"), "{}", got[0]);
    assert!(got[1].contains("M-0001 [home] · The boiler's contract"), "{}", got[1]);
}

#[test]
fn c_4_1_without_the_setting_the_session_folder_is_taken() {
    let copy = Temp::sample("session");
    let project = text_of(copy.path());
    let by_project = ask(&[(PROJECT, &project)], None, &[("where", "{}")]);
    assert!(by_project[0].contains(&shown(copy.path())), "{}", by_project[0]);
    assert!(by_project[0].contains("the folder this session is opened on"), "{}", by_project[0]);
    let by_cwd = ask(&[], Some(copy.path()), &[("where", "{}")]);
    assert!(by_cwd[0].contains("the folder this session is opened on"), "{}", by_cwd[0]);
    for unset in ["", "  ", "${user_config.memory_folder}"] {
        let got = ask(&[(SETTING, unset), (PROJECT, &project)], None, &[("where", "{}")]);
        assert!(got[0].contains("the folder this session is opened on"), "{unset:?}: {}", got[0]);
    }
}

#[test]
fn c_4_1_the_session_folder_wins_over_the_setting_when_it_is_a_memory_folder() {
    let given = Temp::sample("both_given");
    let session = Temp::sample("both_session");
    session.write("memories/M-0020.md", &memory("M-0020", "Only in the session's folder", "A lighthouse.", ""));
    let setting = text_of(given.path());
    let project = text_of(session.path());
    let got = ask(
        &[(SETTING, &setting), (PROJECT, &project)],
        Some(session.path()),
        &[("where", "{}"), ("search", r#"{"text":"lighthouse"}"#)],
    );
    assert!(got[0].contains(&shown(session.path())), "{}", got[0]);
    assert!(got[0].contains("the folder this session is opened on"), "{}", got[0]);
    assert!(got[1].contains("M-0020"), "{}", got[1]);
}

// ---------------------------------------------------------------------------------------------
// C-4.2 no folder: one sentence, nothing created (2 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_4_2_no_folder_where_says_so_and_every_other_tool_the_same_sentence() {
    let empty = Temp::empty("none");
    let project = text_of(empty.path());
    let got = ask(
        &[(PROJECT, &project)],
        Some(empty.path()),
        &[
            ("where", "{}"),
            ("plop", "{}"),
            ("search", r#"{"text":"boiler"}"#),
            ("read", r#"{"piece":"M-0001"}"#),
            ("links", r#"{"piece":"M-0001"}"#),
            ("read", "{}"),
            ("import", r#"{"text":"[2020-01-01] - x"}"#),
        ],
    );
    assert_eq!(got[0].lines().next(), Some(NO_FOLDER), "{}", got[0]);
    assert!(NO_FOLDER.contains("the tool start makes one ready"), "where and plop offer start");
    for other in &got[1..] {
        assert_eq!(other, NO_FOLDER);
    }
}

#[test]
fn c_4_2_no_folder_nothing_is_created() {
    let empty = Temp::empty("none_created");
    let not_memory = Temp::empty("not_memory");
    not_memory.write("notes.txt", "a folder of something else");
    let before = snapshot(not_memory.path());
    let project = text_of(empty.path());
    let calls = [("where", "{}"), ("search", r#"{"text":"x"}"#), ("read", r#"{"piece":"M-0001"}"#)];
    ask(&[(PROJECT, &project)], Some(empty.path()), &calls);
    assert_eq!(fs::read_dir(empty.path()).unwrap().count(), 0, "nothing made in the session's folder");
    let setting = text_of(not_memory.path());
    let got = ask(&[(SETTING, &setting)], Some(empty.path()), &calls);
    for said in &got {
        let first = said.lines().next().unwrap();
        assert!(first.contains(&shown(not_memory.path())) && first.contains("not a memory folder"), "{said}");
    }
    assert_eq!(snapshot(not_memory.path()), before);
    assert_eq!(fs::read_dir(empty.path()).unwrap().count(), 0);
}

// ---------------------------------------------------------------------------------------------
// C-4.3 a piece changed between two calls (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_4_3_a_piece_changed_between_two_calls_is_seen_at_the_next() {
    let copy = Temp::sample("changed");
    let setting = text_of(copy.path());
    let mut live = Live::start(&[(SETTING, &setting)]);
    let first = live.ask("read", r#"{"piece":"M-0001"}"#);
    assert!(first.contains("yearly service contract"), "{first}");
    let path = copy.path().join("memories/M-0001.md");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replace("yearly service contract", "monthly service contract")).unwrap();
    copy.write("memories/M-0021.md", &memory("M-0021", "A new memory", "Written by another program.", ""));
    let second = live.ask("read", r#"{"piece":"M-0001"}"#);
    assert!(second.contains("monthly service contract"), "{second}");
    let found = live.ask("search", r#"{"text":"another program"}"#);
    assert!(found.contains("M-0021"), "{found}");
}

// ---------------------------------------------------------------------------------------------
// C-4.4 a second repository named in the sample's config (2 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_4_4_the_club_s_piece_is_found_with_its_repository() {
    let copy = Temp::sample("club_found");
    let got = ask_one(copy.path(), "search", r#"{"text":"key"}"#);
    assert!(got.contains("C-0001 [club] · The club's key"), "{got}");
    let boiler = ask_one(copy.path(), "search", r#"{"text":"boiler"}"#);
    assert!(boiler.contains("M-0001 [home] · The boiler's contract"), "{boiler}");
}

#[test]
fn c_4_4_read_and_links_say_the_repository() {
    let copy = Temp::sample("club_said");
    let read = ask_one(copy.path(), "read", r#"{"piece":"C-0001"}"#);
    assert!(read.contains("Repository: club"), "{read}");
    assert!(read.contains("The key of the club's room"), "{read}");
    let links = ask_one(copy.path(), "links", r#"{"piece":"C-0001"}"#);
    assert!(links.starts_with("C-0001 [club] · The club's key"), "{links}");
}

// ---------------------------------------------------------------------------------------------
// C-5.1 the sample before and after every tool (1 check)
// ---------------------------------------------------------------------------------------------

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Every tool, with good and bad arguments.
const EVERY_CALL: &[(&str, &str)] = &[
    ("where", "{}"),
    ("plop", "{}"),
    ("search", r#"{"text":"boiler"}"#),
    ("search", r#"{"text":"surprise"}"#),
    ("search", r#"{"text":""}"#),
    ("search", r#"{"text":42}"#),
    ("read", r#"{"piece":"M-0002"}"#),
    ("read", r#"{"piece":"Sam Example"}"#),
    ("read", r#"{"piece":"home"}"#),
    ("read", r#"{"piece":"M-0006"}"#),
    ("read", r#"{"piece":"M-0005"}"#),
    ("read", r#"{"piece":"T-0001"}"#),
    ("read", r#"{"piece":"M-9999"}"#),
    ("read", "{}"),
    ("links", r#"{"piece":"M-0001"}"#),
    ("links", r#"{"piece":"people/sam-example"}"#),
    ("links", r#"{"piece":"C-0001"}"#),
    ("links", r#"{"piece":["M-0001"]}"#),
];

#[test]
fn c_5_1_nothing_is_written_after_every_tool_called_many_times() {
    let copy = Temp::sample("written");
    git(copy.path(), &["init", "-q"]);
    git(copy.path(), &["-c", "core.autocrlf=false", "add", "-A"]);
    git(
        copy.path(),
        &[
            "-c", "user.name=sample", "-c", "user.email=sample@example.invalid",
            // No background upkeep by git after the commit: its lock file would come and go
            // under the snapshot, and look like a change that is not the program's.
            "-c", "maintenance.auto=false", "-c", "gc.auto=0",
            "-c", "commit.gpgsign=false", "commit", "-q", "-m", "the sample",
        ],
    );
    let before = snapshot(copy.path());
    let setting = text_of(copy.path());
    for _ in 0..3 {
        ask(&[(SETTING, &setting)], None, EVERY_CALL);
        ask(&[], Some(copy.path()), EVERY_CALL);
    }
    let differs = differences(&before, &snapshot(copy.path()));
    assert!(differs.is_empty(), "every file as before, none added: {differs:?}");
    assert_eq!(git(copy.path(), &["status", "--porcelain"]), "", "git status clean");
}

// ---------------------------------------------------------------------------------------------
// C-5.2 no connection, no other program (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_5_2_an_empty_path_changes_no_answer_and_the_source_names_no_network() {
    let copy = Temp::sample("no_path");
    let setting = text_of(copy.path());
    let with_path = ask(&[(SETTING, &setting)], None, EVERY_CALL);
    let without = ask(&[(SETTING, &setting), ("PATH", "")], None, EVERY_CALL);
    assert_eq!(with_path, without);
    let remember = r#"{"title":"A try","text":"Written without git."}"#;
    let writes = ask(&[(SETTING, &setting), ("PATH", "")], None, &[("remember", remember), ("undo", "{}")]);
    assert_eq!(writes[0], "Filed as M-0007 [home]: \"A try\".", "not under git, filed without it");
    assert!(writes[1].starts_with("This memory folder keeps no history"), "{}", writes[1]);
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    for dir in ["src", "../store/src", "../store/src/actions"] {
        for entry in fs::read_dir(manifest.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "rs") {
                sources.push(path);
            }
        }
    }
    for path in &sources {
        let text = fs::read_to_string(path).unwrap();
        for word in ["std::net", "TcpStream", "TcpListener", "UdpSocket", "SocketAddr"] {
            assert!(!text.contains(word), "{} names {word}", path.display());
        }
        let in_tools = path.starts_with(manifest.join("src"));
        assert!(!(in_tools && text.contains("Command")), "{} starts a program", path.display());
    }
    let cargo = fs::read_to_string(manifest.join("Cargo.toml")).unwrap();
    let deps = cargo.split("[dependencies]").nth(1).unwrap();
    let named: Vec<&str> = deps.lines().filter(|l| l.contains('=')).collect();
    assert_eq!(
        named,
        [r#"lorisse-store = { path = "../store" }"#, r#"lorisse-guard = { path = "../guard" }"#]
    );
}

// ---------------------------------------------------------------------------------------------
// C-6.1 the confidential piece, by every tool (4 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_6_1_search_never_finds_nor_counts_the_confidential_piece() {
    let copy = Temp::sample("conf_search");
    for text in ["surprise", "June", "Nobody is to know"] {
        let got = ask_one(copy.path(), "search", &format!(r#"{{"text":"{text}"}}"#));
        assert!(got.starts_with("Nothing in the memories, people and tags holds"), "{got}");
    }
    for i in 0..11 {
        let id = format!("M-{:04}", 30 + i);
        copy.write(&format!("memories/{id}.md"), &memory(&id, &format!("Party plan {i}"), "a party", ""));
    }
    let got = ask_one(copy.path(), "search", r#"{"text":"party"}"#);
    assert!(got.contains("11 found in all"), "{got}");
    assert!(!got.contains("M-0006") && !got.contains("surprise"), "{got}");
}

#[test]
fn c_6_1_read_of_the_confidential_number_is_the_answer_for_a_number_that_does_not_exist() {
    let copy = Temp::sample("conf_read");
    let hidden = ask_one(copy.path(), "read", r#"{"piece":"M-0006"}"#);
    let absent = ask_one(copy.path(), "read", r#"{"piece":"M-0099"}"#);
    assert_eq!(hidden, "No piece has the number M-0006.");
    assert_eq!(absent, "No piece has the number M-0099.");
    assert_eq!(ask_one(copy.path(), "links", r#"{"piece":"M-0006"}"#), hidden);
}

#[test]
fn c_6_1_links_never_name_the_confidential_piece() {
    let copy = Temp::sample("conf_links");
    let path = copy.path().join("memories/M-0002.md");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replace("links: [M-0001]", "links: [M-0001, M-0006]")).unwrap();
    let path = copy.path().join("confidential/memories/M-0006.md");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replace("links: []", "links: [M-0001]")).unwrap();
    let links = ask_one(copy.path(), "links", r#"{"piece":"M-0002"}"#);
    assert!(links.contains("M-0001 [home] · The boiler's contract"), "{links}");
    assert!(!links.contains("M-0006") && !links.contains("leads to no piece"), "{links}");
    let incoming = ask_one(copy.path(), "links", r#"{"piece":"M-0001"}"#);
    assert!(!incoming.contains("M-0006") && !incoming.contains("surprise"), "{incoming}");
    let read = ask_one(copy.path(), "read", r#"{"piece":"M-0002"}"#);
    assert!(!read.contains("M-0006"), "{read}");
}

#[test]
fn c_6_1_a_repository_marked_confidential_does_not_exist_either() {
    let copy = Temp::sample("conf_row");
    let config = fs::read_to_string(copy.path().join("config.yaml")).unwrap();
    let rows = "  - { name: work, folder: work, confidential: true }\n";
    fs::write(copy.path().join("config.yaml"), format!("{config}{rows}")).unwrap();
    copy.write("work/memories/M-0040.md", &memory("M-0040", "The secret plan", "A secret.", "M-0001"));
    let calls = [
        ("search", r#"{"text":"secret"}"#),
        ("read", r#"{"piece":"M-0040"}"#),
        ("links", r#"{"piece":"M-0001"}"#),
        ("where", "{}"),
    ];
    let setting = text_of(copy.path());
    let got = ask(&[(SETTING, &setting)], None, &calls);
    assert!(got[0].starts_with("Nothing in the memories"), "{}", got[0]);
    assert_eq!(got[1], "No piece has the number M-0040.");
    for said in &got {
        assert!(!said.contains("M-0040") || said.starts_with("No piece"), "{said}");
        assert!(!said.contains("work]") && !said.contains("secret plan"), "{said}");
    }
}

// ---------------------------------------------------------------------------------------------
// C-6.3 a confidential repository open to Claude (3 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_6_3_a_repository_open_to_claude_is_found_and_read_with_its_repository() {
    let copy = Temp::sample("open_found");
    let found = ask_one(copy.path(), "search", r#"{"text":"printer"}"#);
    assert_eq!(found.lines().next(), Some(DATA_LINE), "{found}");
    assert!(found.contains("O-0001 [office] · The printer's lease"), "{found}");
    let read = ask_one(copy.path(), "read", r#"{"piece":"o-1"}"#);
    assert_eq!(read.lines().next(), Some(DATA_LINE), "{read}");
    assert!(read.contains("Repository: office") && read.contains("The lease is paid each quarter."), "{read}");
    let links = ask_one(copy.path(), "links", r#"{"piece":"O-0001"}"#);
    assert!(links.starts_with("O-0001 [office] · The printer's lease"), "{links}");
}

#[test]
fn c_6_3_plop_counts_the_open_repository_and_names_it_confidential() {
    let copy = Temp::sample("open_plop");
    let got = ask_one(copy.path(), "plop", "{}");
    let state = got.split("\n\n").find(|p| p.starts_with("The memory folder: ")).unwrap();
    assert!(state.contains("\n- office (confidential, kept apart): The work at the office"), "{state}");
    assert!(state.contains("\n- home: ") && state.contains("\n- club: "), "{state}");
    // The sample's retired memory, M-0004, is left out of the count (C-12.2).
    assert!(state.contains("It holds 5 memories and 1 person."), "{state}");
    assert!(state.contains("- O-0001 [office] · The printer's lease"), "{state}");
    assert!(!got.contains("M-0006") && !got.contains("surprise"), "the confidential part stays hidden: {got}");
}

#[test]
fn c_6_3_a_confidential_repository_without_anthropic_among_its_providers_stays_hidden() {
    let copy = Temp::sample("open_not");
    let config = fs::read_to_string(copy.path().join("config.yaml")).unwrap();
    let gemini = config.replace("providers: [anthropic]", "providers: [gemini]");
    assert_ne!(gemini, config, "the sample names its providers");
    fs::write(copy.path().join("config.yaml"), &gemini).unwrap();
    let calls = [
        ("search", r#"{"text":"printer"}"#),
        ("read", r#"{"piece":"O-0001"}"#),
        ("links", r#"{"piece":"O-0001"}"#),
        ("plop", "{}"),
    ];
    let got = ask(&[(SETTING, &text_of(copy.path()))], None, &calls);
    assert!(got[0].starts_with("Nothing in the memories"), "{}", got[0]);
    assert_eq!(got[1], "No piece has the number O-0001.");
    assert_eq!(got[2], got[1]);
    assert!(got[3].contains("\n- home: ") && got[3].contains("\n- club: ") && !got[3].contains("office"), "{}", got[3]);
    fs::write(copy.path().join("config.yaml"), config.replace("[anthropic]", "[ANTHROPIC]")).unwrap();
    let any_case = ask_one(copy.path(), "search", r#"{"text":"printer"}"#);
    assert!(any_case.contains("O-0001 [office]"), "the provider's name in any case: {any_case}");
}

// ---------------------------------------------------------------------------------------------
// C-6.2 the first line of an answer that carries a text (2 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_6_2_read_says_first_that_the_text_is_data() {
    let copy = Temp::sample("data_read");
    for piece in ["M-0002", "Sam Example", "home", "C-0001"] {
        let got = ask_one(copy.path(), "read", &format!(r#"{{"piece":"{piece}"}}"#));
        assert_eq!(got.lines().next(), Some(DATA_LINE), "{got}");
    }
}

#[test]
fn c_6_2_search_says_first_that_the_lines_are_data() {
    let copy = Temp::sample("data_search");
    let got = ask_one(copy.path(), "search", r#"{"text":"boiler"}"#);
    assert_eq!(got.lines().next(), Some(DATA_LINE), "{got}");
    assert!(got.contains("The boiler's contract") && got.contains("Boiler serviced in March"), "{got}");
}

// ---------------------------------------------------------------------------------------------
// C-7.1 a list past its limit (1 check), C-7.2 a text past its limit (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_7_1_a_list_past_ten_ends_with_the_count_in_all() {
    let copy = Temp::sample("list_cut");
    for i in 0..12 {
        let id = format!("M-{:04}", 50 + i);
        copy.write(&format!("memories/{id}.md"), &memory(&id, &format!("Lantern {i}"), "a lantern", ""));
    }
    let got = ask_one(copy.path(), "search", r#"{"text":"lantern"}"#);
    let lines: Vec<&str> = got.lines().collect();
    assert_eq!(lines.len(), 12, "the data line, ten pieces, the count: {got}");
    assert_eq!(lines[11], "12 found in all; the first 10 are shown.");
}

#[test]
fn c_7_2_a_text_past_6000_characters_ends_with_its_full_length() {
    let copy = Temp::sample("text_cut");
    let body = "é".repeat(7000);
    copy.write("memories/M-0060.md", &memory("M-0060", "A long text", &body, ""));
    let got = ask_one(copy.path(), "read", r#"{"piece":"M-0060"}"#);
    let last = got.lines().last().unwrap();
    // The body is the 7000 letters and the file's last newline.
    assert_eq!(last, "[The text is cut here: 6000 of its 7001 characters are shown.]");
    let shown = got.lines().rev().nth(1).unwrap();
    assert_eq!(shown.chars().count(), 6000);
}
