//! The program's guard mode, asked as the Claude app asks it: one call on stdin, one answer or
//! nothing on stdout, always a good ending. The rules themselves are the guard crate's tests.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn folder(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lorisse_guard_mode_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn guard(received: &str, cwd: &Path) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lorisse"))
        .arg("guard")
        .current_dir(cwd)
        .env_remove("LORISSE_MEMORY_FOLDER")
        .env_remove("CLAUDE_PLUGIN_OPTION_MEMORY_FOLDER")
        .env_remove("CLAUDE_PROJECT_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(received.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "the guard always ends well");
    String::from_utf8(out.stdout).unwrap()
}

fn call(tool: &str) -> String {
    format!(r#"{{"hook_event_name":"PreToolUse","tool_name":"{tool}","tool_input":{{}}}}"#)
}

#[test]
fn without_a_memory_folder_the_first_set_lets_lorisse_pass() {
    let nowhere = folder("nowhere");
    let got = guard(&call("mcp__plugin_lorisse_lorisse__where"), &nowhere);
    assert!(got.contains(r#""permissionDecision":"allow""#), "{got}");
    let other = guard(&call("mcp__claude_ai_Dropbox__delete"), &nowhere);
    assert!(other.trim().is_empty(), "{other}");
    let _ = fs::remove_dir_all(&nowhere);
}

#[test]
fn the_rules_of_the_folder_the_session_is_opened_on_are_obeyed() {
    let memory = folder("rules");
    fs::write(memory.join("config.yaml"), "kinds: [fact]\n").unwrap();
    fs::write(memory.join("rules.yaml"), "pass:\n  lorisse: [where]\nnever:\n  lorisse: [search]\n").unwrap();
    let refused = guard(&call("mcp__plugin_lorisse_lorisse__search"), &memory);
    assert!(refused.contains(r#""permissionDecision":"deny""#), "{refused}");
    assert!(refused.contains("search"), "{refused}");
    let passed = guard(&call("mcp__plugin_lorisse_lorisse__where"), &memory);
    assert!(passed.contains(r#""permissionDecision":"allow""#), "{passed}");
    let _ = fs::remove_dir_all(&memory);
}

#[test]
fn anything_else_on_stdin_gets_nothing_and_a_good_ending() {
    let nowhere = folder("garbage");
    for received in ["", "not json", "{}", r#"{"hook_event_name":"PreToolUse"}"#] {
        let got = guard(received, &nowhere);
        assert!(got.trim().is_empty(), "{received:?}: {got}");
    }
    let _ = fs::remove_dir_all(&nowhere);
}
