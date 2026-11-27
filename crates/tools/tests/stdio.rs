//! The protocol: the program started and asked over stdin and stdout, as a client would.

mod common;

use common::json::Json;
use common::{program, talk_with};

const INIT: &str = r#"{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}"#;

fn with_id(base: &str, id: &str) -> String {
    format!("{},\"id\":{}}}", base, id)
}

fn talk(lines: &[&str]) -> Vec<Json> {
    let lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    talk_with(program(&[], None), &lines)
}

#[test]
fn initialize_with_a_number_id() {
    let got = talk(&[&with_id(INIT, "1")]);
    assert_eq!(got.len(), 1);
    let r = &got[0];
    assert_eq!(r.get("id"), Some(&Json::Number("1".into())));
    let res = r.get("result").unwrap();
    assert_eq!(res.get("protocolVersion").and_then(Json::as_str), Some("2025-03-26"));
    assert!(res.get("capabilities").unwrap().get("tools").is_some());
    let info = res.get("serverInfo").unwrap();
    assert_eq!(info.get("name").and_then(Json::as_str), Some("lorisse"));
    assert_eq!(info.get("version").and_then(Json::as_str), Some("0.7.0"));
}

#[test]
fn initialize_with_a_string_id() {
    let got = talk(&[&with_id(INIT, "\"abc-7\"")]);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].get("id").and_then(Json::as_str), Some("abc-7"));
    assert!(got[0].get("result").is_some());
}

#[test]
fn a_notification_is_not_answered() {
    let got = talk(&[
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
    ]);
    assert_eq!(got.len(), 1, "only the ping is answered");
    assert_eq!(got[0].get("id"), Some(&Json::Number("2".into())));
    assert_eq!(got[0].get("result"), Some(&Json::Object(vec![])));
}

#[test]
fn tools_list_names_the_seventeen_tools_each_with_its_description() {
    let got = talk(&[r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#]);
    let tools = match got[0].get("result").unwrap().get("tools").unwrap() {
        Json::Array(t) => t.clone(),
        _ => panic!("tools is an array"),
    };
    let names: Vec<&str> = tools.iter().filter_map(|t| t.get("name").and_then(Json::as_str)).collect();
    assert_eq!(
        names,
        ["where", "plop", "search", "read", "links", "remember", "note", "link", "attach", "undo", "start", "import", "change", "retire", "journal", "drawer", "move"]
    );
    for tool in &tools {
        let said = tool.get("description").and_then(Json::as_str).unwrap();
        assert!(said.contains("Use it"), "{said}");
        let schema = tool.get("inputSchema").unwrap();
        assert_eq!(schema.get("type").and_then(Json::as_str), Some("object"));
    }
}

#[test]
fn a_call_of_a_tool_named_open_is_answered_as_an_unknown_tool() {
    let got = talk(&[r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"open","arguments":{}}}"#]);
    assert_eq!(got.len(), 1);
    assert!(got[0].get("result").is_none());
    let err = got[0].get("error").unwrap();
    assert_eq!(err.get("code"), Some(&Json::Number("-32602".into())));
    assert_eq!(err.get("message").and_then(Json::as_str), Some("Unknown tool: open"));
}

#[test]
fn an_unknown_method_is_refused() {
    let got = talk(&[r#"{"jsonrpc":"2.0","id":"x","method":"resources/list"}"#]);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].get("id").and_then(Json::as_str), Some("x"));
    let err = got[0].get("error").unwrap();
    assert_eq!(err.get("code"), Some(&Json::Number("-32601".into())));
    assert!(got[0].get("result").is_none());
}
