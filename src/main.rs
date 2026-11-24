//! Lorisse's plugin program: an MCP server over stdio, one JSON-RPC message a line,
//! with one tool, `where`. It reads nothing but stdin and three values of its own
//! process, and writes nothing but its answers on stdout.

mod json;

use json::Json;
use std::io::{self, BufRead, Write};

const NAME: &str = "lorisse";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PROTOCOL: &str = "2025-06-18";

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(answer) = handle(&line) {
            if writeln!(out, "{}", answer).is_err() || out.flush().is_err() {
                break;
            }
        }
    }
}

/// One line in, at most one line out. A notification (no `id`) gets no answer.
fn handle(line: &str) -> Option<String> {
    let msg = match json::parse(line) {
        Ok(m) => m,
        Err(_) => return Some(error(Json::Null, -32700, "Parse error")),
    };
    let id = msg.get("id").cloned();
    let method = match msg.get("method").and_then(Json::as_str) {
        Some(m) => m,
        // A response from the client, or a message without a method.
        None => {
            return id.map(|id| error(id, -32600, "Invalid Request"));
        }
    };
    let id = id?; // a notification: never answered
    let params = msg.get("params");
    Some(match method {
        "initialize" => {
            let asked = params
                .and_then(|p| p.get("protocolVersion"))
                .and_then(Json::as_str)
                .unwrap_or(DEFAULT_PROTOCOL);
            result(
                id,
                Json::obj(vec![
                    ("protocolVersion", Json::str(asked)),
                    ("capabilities", Json::obj(vec![("tools", Json::obj(vec![]))])),
                    (
                        "serverInfo",
                        Json::obj(vec![("name", Json::str(NAME)), ("version", Json::str(VERSION))]),
                    ),
                ]),
            )
        }
        "ping" => result(id, Json::obj(vec![])),
        "tools/list" => result(
            id,
            Json::obj(vec![(
                "tools",
                Json::Array(vec![Json::obj(vec![
                    ("name", Json::str("where")),
                    (
                        "description",
                        Json::str("Says that Lorisse is here, the folder the program was started in, and the plugin and project folders it was given."),
                    ),
                    (
                        "inputSchema",
                        Json::obj(vec![
                            ("type", Json::str("object")),
                            ("properties", Json::obj(vec![])),
                        ]),
                    ),
                ])]),
            )]),
        ),
        "tools/call" => {
            let name = params.and_then(|p| p.get("name")).and_then(Json::as_str);
            match name {
                Some("where") => result(
                    id,
                    Json::obj(vec![
                        (
                            "content",
                            Json::Array(vec![Json::obj(vec![
                                ("type", Json::str("text")),
                                ("text", Json::Str(where_text())),
                            ])]),
                        ),
                        ("isError", Json::Bool(false)),
                    ]),
                ),
                Some(other) => error(id, -32602, &format!("Unknown tool: {}", other)),
                None => error(id, -32602, "Missing tool name"),
            }
        }
        _ => error(id, -32601, "Method not found"),
    })
}

fn where_text() -> String {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "not given".to_string());
    let var = |k: &str| std::env::var(k).unwrap_or_else(|_| "not given".to_string());
    format!(
        "Lorisse is here.\nStarted in: {}\nCLAUDE_PLUGIN_ROOT: {}\nCLAUDE_PROJECT_DIR: {}",
        cwd,
        var("CLAUDE_PLUGIN_ROOT"),
        var("CLAUDE_PROJECT_DIR")
    )
}

fn result(id: Json, value: Json) -> String {
    Json::obj(vec![("jsonrpc", Json::str("2.0")), ("id", id), ("result", value)]).to_string()
}

fn error(id: Json, code: i64, message: &str) -> String {
    Json::obj(vec![
        ("jsonrpc", Json::str("2.0")),
        ("id", id),
        (
            "error",
            Json::obj(vec![
                ("code", Json::Number(code.to_string())),
                ("message", Json::str(message)),
            ]),
        ),
    ])
    .to_string()
}
