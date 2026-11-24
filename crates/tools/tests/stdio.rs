//! Starts the built program and talks to it over stdin and stdout, as a client would.

#[path = "../src/json.rs"]
#[allow(dead_code)]
mod json;

use json::Json;
use std::io::Write;
use std::process::{Command, Stdio};

/// Sends the lines, closes stdin, and returns every line the program wrote on stdout.
fn talk(lines: &[&str], env: &[(&str, Option<&str>)]) -> Vec<Json> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lorisse"));
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in env {
        match v {
            Some(v) => cmd.env(k, v),
            None => cmd.env_remove(k),
        };
    }
    let mut child = cmd.spawn().expect("the program starts");
    {
        let stdin = child.stdin.as_mut().unwrap();
        for l in lines {
            writeln!(stdin, "{}", l).unwrap();
        }
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "the program ends cleanly when stdin closes");
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| json::parse(l).expect("every stdout line is JSON"))
        .collect()
}

const INIT: &str = r#"{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}"#;

fn with_id(base: &str, id: &str) -> String {
    format!("{},\"id\":{}}}", base, id)
}

#[test]
fn initialize_with_a_number_id() {
    let got = talk(&[&with_id(INIT, "1")], &[]);
    assert_eq!(got.len(), 1);
    let r = &got[0];
    assert_eq!(r.get("id"), Some(&Json::Number("1".into())));
    let res = r.get("result").unwrap();
    assert_eq!(res.get("protocolVersion").and_then(Json::as_str), Some("2025-03-26"));
    assert!(res.get("capabilities").unwrap().get("tools").is_some());
    let info = res.get("serverInfo").unwrap();
    assert_eq!(info.get("name").and_then(Json::as_str), Some("lorisse"));
    assert_eq!(info.get("version").and_then(Json::as_str), Some("0.0.1"));
}

#[test]
fn initialize_with_a_string_id() {
    let got = talk(&[&with_id(INIT, "\"abc-7\"")], &[]);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].get("id").and_then(Json::as_str), Some("abc-7"));
    assert!(got[0].get("result").is_some());
}

#[test]
fn a_notification_is_not_answered() {
    let got = talk(
        &[
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
        ],
        &[],
    );
    assert_eq!(got.len(), 1, "only the ping is answered");
    assert_eq!(got[0].get("id"), Some(&Json::Number("2".into())));
    assert_eq!(got[0].get("result"), Some(&Json::Object(vec![])));
}

#[test]
fn tools_list_names_where() {
    let got = talk(&[r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#], &[]);
    let tools = match got[0].get("result").unwrap().get("tools").unwrap() {
        Json::Array(t) => t.clone(),
        _ => panic!("tools is an array"),
    };
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].get("name").and_then(Json::as_str), Some("where"));
    assert_eq!(
        tools[0].get("inputSchema").unwrap().get("type").and_then(Json::as_str),
        Some("object")
    );
}

#[test]
fn tools_call_answers_the_text() {
    let got = talk(
        &[r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"where","arguments":{}}}"#],
        &[
            ("CLAUDE_PLUGIN_ROOT", Some("C:/plugin/root")),
            ("CLAUDE_PROJECT_DIR", None),
        ],
    );
    let content = match got[0].get("result").unwrap().get("content").unwrap() {
        Json::Array(c) => c.clone(),
        _ => panic!("content is an array"),
    };
    assert_eq!(content[0].get("type").and_then(Json::as_str), Some("text"));
    let text = content[0].get("text").and_then(Json::as_str).unwrap();
    let cwd = std::env::current_dir().unwrap().display().to_string();
    assert!(text.starts_with("Lorisse is here.\n"), "{}", text);
    assert!(text.contains(&format!("Started in: {}", cwd)), "{}", text);
    assert!(text.contains("CLAUDE_PLUGIN_ROOT: C:/plugin/root"), "{}", text);
    assert!(text.contains("CLAUDE_PROJECT_DIR: not given"), "{}", text);
}

#[test]
fn an_unknown_method_is_refused() {
    let got = talk(&[r#"{"jsonrpc":"2.0","id":"x","method":"resources/list"}"#], &[]);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].get("id").and_then(Json::as_str), Some("x"));
    let err = got[0].get("error").unwrap();
    assert_eq!(err.get("code"), Some(&Json::Number("-32601".into())));
    assert!(got[0].get("result").is_none());
}
