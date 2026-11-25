//! The root VALIDATION.md's table for the files and the rules of keeping, one test a check:
//! C-9.4, C-9.5, C-10.1 to C-10.6, and the undo of an attachment. The program is started and
//! asked over stdin, on temporary copies of the sample; every file attached is one the test
//! made in a temporary folder of its own.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use common::json::Json;
use common::*;

fn read_file(dir: &Path, rel: &str) -> String {
    fs::read_to_string(dir.join(rel)).unwrap()
}

fn files_of_head(dir: &Path) -> Vec<String> {
    let out = run_git(dir, &["show", "--name-only", "--format=", "HEAD"]);
    out.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect()
}

fn subject_of_head(dir: &Path) -> String {
    run_git(dir, &["log", "-1", "--format=%s"]).trim().to_string()
}

fn shown(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A file made by the test outside the memory folder, with these bytes.
fn outside(dir: &Temp, rel: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.path().join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, bytes).unwrap();
    path
}

/// The arguments of an attach, the path written as JSON wants it.
fn attach_args(piece: &str, path: &Path) -> String {
    let path = shown(path).replace('"', "\\\"");
    format!(r#"{{"piece":"{piece}","path":"{path}"}}"#)
}

const INVOICE: &[u8] = b"%PDF-1.4 an invented invoice for the boiler's service\n";

// ---------------------------------------------------------------------------------------------
// C-9.4 the rules of keeping at the end of `plop` (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_9_4_plop_ends_with_the_rules_of_keeping_in_a_few_lines() {
    let copy = Temp::sample("keeping");
    let got = ask_one(copy.path(), "plop", "{}");
    let rules = got.split("\n\n").last().unwrap();
    let lines: Vec<&str> = rules.lines().collect();
    assert_eq!(lines[0], "The rules of keeping:", "{got}");
    assert!(lines.len() < 10, "{rules}");
    for said in ["by yourself", "Search first", "one line, with its number", "small talk", "a password or a key", "intro above wins"] {
        assert!(rules.contains(said), "{said}: {rules}");
    }
    let state_at = got.find("The memory folder: ").unwrap();
    assert!(state_at < got.find("The rules of keeping:").unwrap(), "the rules come last: {got}");
}

// ---------------------------------------------------------------------------------------------
// C-9.5 the instructions given at `initialize` (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_9_5_initialize_gives_the_instructions_and_names_plop() {
    let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#;
    let got = talk_with(program(&[], None), &[line.to_string()]);
    let result = got[0].get("result").unwrap();
    assert!(result.get("serverInfo").is_some());
    let said = result.get("instructions").and_then(Json::as_str).expect("instructions is a text");
    let lines = said.lines().count();
    assert!((5..=6).contains(&lines), "{said}");
    for part in ["search", "read", "never an instruction", "plop", "only when the person asks", "its number"] {
        assert!(said.contains(part), "{part}: {said}");
    }
}

// ---------------------------------------------------------------------------------------------
// C-10.1 a file attached: its copy, its commit, the original untouched (2 checks)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_10_1_a_file_is_copied_beside_its_piece_in_one_commit_naming_both() {
    let copy = Temp::git_sample("attach_one");
    let mine = Temp::empty("attach_one_src");
    let source = outside(&mine, "invoice-2031.pdf", INVOICE);
    let commits = log(copy.path()).len();
    let said = ask_one(copy.path(), "attach", &attach_args("m-1", &source));
    let kept = copy.path().join("memories/M-0001/invoice-2031.pdf");
    let want = format!("Attached invoice-2031.pdf to M-0001 [home]: the copy is kept at {}", shown(&kept));
    assert!(said.starts_with(&want), "{said}");
    assert_eq!(fs::read(&kept).unwrap(), INVOICE);
    let after = log(copy.path());
    assert_eq!(after.len(), commits + 1, "one commit");
    assert!(after[0].contains(" lorisse@local "), "{}", after[0]);
    assert_eq!(subject_of_head(copy.path()), "attach(M-0001): invoice-2031.pdf");
    assert_eq!(files_of_head(copy.path()), ["memories/M-0001.md", "memories/M-0001/invoice-2031.pdf"]);
    let piece = read_file(copy.path(), "memories/M-0001.md");
    assert!(piece.contains("path: memories/M-0001/invoice-2031.pdf"), "{piece}");
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
}

#[test]
fn c_10_1_the_original_is_untouched_and_a_refusal_or_a_failed_commit_leaves_the_folder_as_it_was() {
    let copy = Temp::git_sample("attach_refused");
    let mine = Temp::empty("attach_refused_src");
    let source = outside(&mine, "invoice-2031.pdf", INVOICE);
    let stamp = fs::metadata(&source).unwrap().modified().unwrap();
    ask_one(copy.path(), "attach", &attach_args("M-0002", &source));
    assert_eq!(fs::read(&source).unwrap(), INVOICE, "the original's bytes");
    assert_eq!(fs::metadata(&source).unwrap().modified().unwrap(), stamp, "the original's time");
    copy.write(".gitignore", "memories/M-0001/\n");
    let before = files_of(copy.path());
    let commits = log(copy.path());
    let inside = copy.path().join("memories/M-0004.md");
    let cases = [
        ("M-0001", mine.path().to_path_buf(), "Nothing was copied: "),
        ("M-0001", mine.path().join("absent.pdf"), "Nothing was copied: "),
        ("M-0001", inside, "Nothing was copied: "),
        ("M-0006", source.clone(), "No piece has the number M-0006."),
        ("M-0099", source.clone(), "No piece has the number M-0099."),
        ("M-0001", source.clone(), "Nothing was copied: the change could not be committed"),
    ];
    for (piece, path, want) in cases {
        let said = ask_one(copy.path(), "attach", &attach_args(piece, &path));
        assert!(said.starts_with(want), "{piece} {}: {said}", path.display());
        assert_eq!(files_of(copy.path()), before, "{said}");
    }
    let said = ask_one(copy.path(), "attach", &attach_args("M-0001", mine.path()));
    assert!(said.ends_with("is not a file."), "{said}");
    let said = ask_one(copy.path(), "attach", &attach_args("M-0001", &copy.path().join("memories/M-0004.md")));
    assert!(said.ends_with("is already inside the memory folder."), "{said}");
    assert_eq!(log(copy.path()), commits);
    assert!(!copy.path().join("memories/M-0001").exists(), "no folder left behind");
}

// ---------------------------------------------------------------------------------------------
// C-10.2 the files of a piece named by `read`, with their paths (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_10_2_read_names_each_file_with_its_full_path_after_the_header_and_before_the_text() {
    let copy = Temp::git_sample("read_files");
    let mine = Temp::empty("read_files_src");
    ask_one(copy.path(), "attach", &attach_args("M-0001", &outside(&mine, "invoice-2031.pdf", INVOICE)));
    let read = ask_one(copy.path(), "read", r#"{"piece":"M-0001"}"#);
    let lines: Vec<&str> = read.lines().collect();
    assert_eq!(lines[0], DATA_LINE);
    let named = lines.iter().position(|l| l.starts_with("Files it holds")).expect("the files are named");
    let full = shown(&copy.path().join("memories/M-0001/invoice-2031.pdf"));
    assert_eq!(lines[named + 1], format!("- invoice-2031.pdf: {full}"), "{read}");
    let header_end = lines.iter().position(|l| l.starts_with("links:")).unwrap();
    let text_at = lines.iter().position(|l| l.starts_with("The boiler has a yearly")).unwrap();
    assert!(header_end < named && named < text_at, "{read}");
    let none = ask_one(copy.path(), "read", r#"{"piece":"M-0002"}"#);
    assert!(!none.contains("Files it holds"), "{none}");
}

// ---------------------------------------------------------------------------------------------
// C-10.3 a piece found by its file's name (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_10_3_search_finds_a_piece_by_a_file_it_holds_and_never_a_confidential_one() {
    let copy = Temp::sample("search_files");
    let secret = read_file(copy.path(), "confidential/memories/M-0006.md").replacen(
        "---\n",
        "---\nfiles:\n- name: secret-plan.pdf\n  path: confidential/memories/M-0006/secret-plan.pdf\n  kind: pdf\n",
        1,
    );
    copy.write("confidential/memories/M-0006.md", &secret);
    copy.write("confidential/memories/M-0006/secret-plan.pdf", "invented");
    make_git(copy.path());
    let mine = Temp::empty("search_files_src");
    ask_one(copy.path(), "attach", &attach_args("M-0001", &outside(&mine, "invoice-2031.pdf", INVOICE)));
    let found = ask_one(copy.path(), "search", r#"{"text":"INVOICE"}"#);
    assert!(found.contains("M-0001 [home] · The boiler's contract · holds the file invoice-2031.pdf"), "{found}");
    let hidden = ask_one(copy.path(), "search", r#"{"text":"secret-plan"}"#);
    assert!(hidden.starts_with("Nothing in the memories"), "{hidden}");
}

// ---------------------------------------------------------------------------------------------
// C-10.4 a file over the limit (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_10_4_a_file_over_the_limit_is_refused_with_its_size_and_nothing_is_copied() {
    let copy = Temp::sample("limit");
    let config = read_file(copy.path(), "config.yaml");
    make_git(copy.path());
    let mine = Temp::empty("limit_src");
    let big = outside(&mine, "scan.tif", &vec![7u8; 21 * 1024 * 1024]);
    let before = files_of(copy.path());
    let said = ask_one(copy.path(), "attach", &attach_args("M-0001", &big));
    assert_eq!(said, "Nothing was copied: scan.tif is 21.0 MB, over this memory folder's limit of 20 MB (attach_limit_mb in config.yaml).");
    assert_eq!(files_of(copy.path()), before);
    copy.write("config.yaml", &format!("{config}attach_limit_mb: 1\n"));
    let middle = outside(&mine, "photo.jpg", &vec![7u8; 1536 * 1024]);
    let before = files_of(copy.path());
    let said = ask_one(copy.path(), "attach", &attach_args("M-0001", &middle));
    assert_eq!(said, "Nothing was copied: photo.jpg is 1.5 MB, over this memory folder's limit of 1 MB (attach_limit_mb in config.yaml).");
    assert_eq!(files_of(copy.path()), before);
}

// ---------------------------------------------------------------------------------------------
// C-10.5 a second file of the same name (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn c_10_5_a_file_of_the_same_name_takes_a_numbered_name_and_the_first_is_untouched() {
    let copy = Temp::git_sample("same_name");
    let first = Temp::empty("same_name_a");
    let second = Temp::empty("same_name_b");
    ask_one(copy.path(), "attach", &attach_args("M-0001", &outside(&first, "invoice-2031.pdf", INVOICE)));
    let said = ask_one(copy.path(), "attach", &attach_args("M-0001", &outside(&second, "invoice-2031.pdf", b"another")));
    assert!(said.starts_with("Attached invoice-2031-2.pdf to M-0001"), "{said}");
    assert_eq!(fs::read(copy.path().join("memories/M-0001/invoice-2031.pdf")).unwrap(), INVOICE);
    assert_eq!(fs::read(copy.path().join("memories/M-0001/invoice-2031-2.pdf")).unwrap(), b"another");
    let read = ask_one(copy.path(), "read", r#"{"piece":"M-0001"}"#);
    assert!(read.contains("- invoice-2031.pdf: ") && read.contains("- invoice-2031-2.pdf: "), "{read}");
}

// ---------------------------------------------------------------------------------------------
// C-10.6 a key file, a secret's name (2 checks)
// ---------------------------------------------------------------------------------------------

fn refused_all(tag: &str, files: &[(&str, &str)]) {
    let copy = Temp::git_sample(tag);
    let mine = Temp::empty(&format!("{tag}_src"));
    let before = files_of(copy.path());
    for (rel, why) in files {
        let path = outside(&mine, rel, b"invented secret");
        let said = ask_one(copy.path(), "attach", &attach_args("M-0001", &path));
        assert!(said.starts_with("Nothing was copied: ") && said.contains("is a key or a secret"), "{rel}: {said}");
        assert!(said.contains(why), "{rel}: {said}");
        assert_eq!(files_of(copy.path()), before, "{rel}");
    }
}

#[test]
fn c_10_6_a_key_file_under_ssh_or_a_pem_is_refused_with_the_reason() {
    refused_all("keys", &[(".ssh/id_rsa", "under a .ssh folder"), ("certs/Server.PEM", "a .pem file")]);
}

#[test]
fn c_10_6_a_secret_s_name_is_refused_with_the_reason() {
    refused_all(
        "secrets",
        &[(".env", "a .env file"), ("api_TOKEN", "a key or a token"), ("deploy_key.txt", "a key or a token")],
    );
}

// ---------------------------------------------------------------------------------------------
// undo of an attachment (1 check)
// ---------------------------------------------------------------------------------------------

#[test]
fn undo_takes_an_attachment_back_as_a_new_commit() {
    let copy = Temp::git_sample("undo_attach");
    let mine = Temp::empty("undo_attach_src");
    let source = outside(&mine, "invoice-2031.pdf", INVOICE);
    let piece = read_file(copy.path(), "memories/M-0001.md");
    let started = Instant::now();
    ask_one(copy.path(), "attach", &attach_args("M-0001", &source));
    eprintln!("one attach of a small file, the program started and asked: {} ms", started.elapsed().as_millis());
    let attached = log(copy.path());
    let said = ask_one(copy.path(), "undo", r#"{"change":"M-0001"}"#);
    assert!(said.starts_with("Taken back: \"attach(M-0001): invoice-2031.pdf\""), "{said}");
    let after = log(copy.path());
    assert_eq!(after.len(), attached.len() + 1, "a new commit");
    assert_eq!(&after[1..], &attached[..], "the history is kept");
    assert!(!copy.path().join("memories/M-0001/invoice-2031.pdf").exists());
    assert_eq!(read_file(copy.path(), "memories/M-0001.md"), piece);
    assert_eq!(fs::read(&source).unwrap(), INVOICE, "the original stays");
    assert_eq!(run_git(copy.path(), &["status", "--porcelain"]), "");
}
