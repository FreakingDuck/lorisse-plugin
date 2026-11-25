//! Lorisse's plugin program: an MCP server over stdio, one JSON-RPC message a line, with four
//! reading tools: `where`, `search`, `read`, `links`. It reads the person's memory folder through
//! the store opened for reading at each call, and writes nothing but its answers on stdout.

mod folder;
mod json;
mod tools;

use json::Json;
use std::io::{self, BufRead, Write};
use std::panic::{self, AssertUnwindSafe};

const NAME: &str = "lorisse";
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
                        Json::obj(vec![
                            ("name", Json::str(NAME)),
                            ("version", Json::str(tools::VERSION)),
                        ]),
                    ),
                ]),
            )
        }
        "ping" => result(id, Json::obj(vec![])),
        "tools/list" => result(id, Json::obj(vec![("tools", Json::Array(tool_list()))])),
        "tools/call" => {
            let name = params.and_then(|p| p.get("name")).and_then(Json::as_str);
            let args = params.and_then(|p| p.get("arguments"));
            match name {
                Some(tool @ ("where" | "search" | "read" | "links")) => {
                    result(id, answer(tool, args))
                }
                Some(other) => error(id, -32602, &format!("Unknown tool: {}", other)),
                None => error(id, -32602, "Missing tool name"),
            }
        }
        _ => error(id, -32601, "Method not found"),
    })
}

/// A tool's answer as the protocol carries it; a fault inside a tool is one sentence, never a stop.
fn answer(tool: &str, args: Option<&Json>) -> Json {
    let key = if tool == "search" { "text" } else { "piece" };
    let arg = args.and_then(|a| a.get(key)).and_then(Json::as_str);
    let run = AssertUnwindSafe(|| match tool {
        "where" => tools::where_is(),
        _ => tools::call(tool, arg),
    });
    let (text, is_error) = match panic::catch_unwind(run) {
        Ok(a) => (a.text, a.is_error),
        Err(_) => (format!("Lorisse could not answer {tool} this time."), true),
    };
    Json::obj(vec![
        (
            "content",
            Json::Array(vec![Json::obj(vec![
                ("type", Json::str("text")),
                ("text", Json::Str(text)),
            ])]),
        ),
        ("isError", Json::Bool(is_error)),
    ])
}

/// One tool's card: its name, when to use it, its one text argument.
fn card(name: &str, description: &str, arg: Option<(&str, &str)>) -> Json {
    let (properties, required) = match arg {
        Some((key, said)) => (
            Json::obj(vec![(
                key,
                Json::obj(vec![("type", Json::str("string")), ("description", Json::str(said))]),
            )]),
            Json::Array(vec![Json::str(key)]),
        ),
        None => (Json::obj(vec![]), Json::Array(vec![])),
    };
    Json::obj(vec![
        ("name", Json::str(name)),
        ("description", Json::str(description)),
        (
            "inputSchema",
            Json::obj(vec![
                ("type", Json::str("object")),
                ("properties", properties),
                ("required", required),
            ]),
        ),
    ])
}

fn tool_list() -> Vec<Json> {
    vec![
        card(
            "where",
            "Says which memory folder Lorisse reads, how it was found, and Lorisse's version. Use it when the person greets Lorisse or asks where Lorisse is.",
            None,
        ),
        card(
            "search",
            "Searches the person's memory (their memories, people and tags) for a text and lists what holds it, by number and title. Use it first whenever the person asks what they know, decided or noted, or names a person or a subject.",
            Some(("text", "the words to look for, capitals ignored")),
        ),
        card(
            "read",
            "Reads one piece of the person's memory in full: a memory by its number, a person by name, or a tag by its word. Use it after search, to see what a piece says.",
            Some(("piece", "a number such as M-0002, a person's name, or a tag's word")),
        ),
        card(
            "links",
            "Lists what a memory, person or tag links to and what links to it, by number and title. Use it to follow a piece to the memories and people around it.",
            Some(("piece", "a number such as M-0002, or a person's or a tag's name")),
        ),
    ]
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
