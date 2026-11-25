//! Lorisse's plugin program: an MCP server over stdio, one JSON-RPC message a line, with five
//! reading tools (`where`, `plop`, `search`, `read`, `links`), which write nothing, and seven
//! writing tools (`remember`, `note`, `link`, `attach`, `undo`, `start`, `import`), each at most
//! one commit by Lorisse in the person's memory folder when it is under git, through the store.
//! Each tool is declared read-only or writing.
//! Started with the word `guard`, it is the plugin's guard instead: one tool call judged.

mod first_day;
mod folder;
mod json;
mod tools;
mod writing;

use json::Json;
use std::io::{self, BufRead, Read, Write};
use std::panic::{self, AssertUnwindSafe};

const NAME: &str = "lorisse";
const DEFAULT_PROTOCOL: &str = "2025-06-18";

/// The guard mode: what the Claude app sends before a tool call comes in on stdin, and at most
/// one answer goes out. It always ends well: a guard that fails leaves the call to the app.
fn guard() {
    let mut received = String::new();
    if io::stdin().read_to_string(&mut received).is_err() {
        return;
    }
    let answer = panic::catch_unwind(|| {
        let folder = match folder::find() {
            folder::Folder::At(path, _) => Some(path),
            _ => None,
        };
        lorisse_guard::answer(&received, folder.as_deref())
    });
    if let Ok(Some(answer)) = answer {
        println!("{answer}");
    }
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("guard") {
        guard();
        return;
    }
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
                    ("instructions", Json::str(tools::INSTRUCTIONS)),
                ]),
            )
        }
        "ping" => result(id, Json::obj(vec![])),
        "tools/list" => result(id, Json::obj(vec![("tools", Json::Array(tool_list()))])),
        "tools/call" => {
            let name = params.and_then(|p| p.get("name")).and_then(Json::as_str);
            let args = params.and_then(|p| p.get("arguments"));
            match name {
                Some(tool) if NAMES.contains(&tool) => result(id, answer(tool, args)),
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
        "plop" => tools::plop(),
        "start" => first_day::start(args),
        "search" | "read" | "links" => tools::call(tool, arg),
        _ => writing::call(tool, args),
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

/// Every tool, in the order `tools/list` gives them.
const NAMES: [&str; 12] = [
    "where", "plop", "search", "read", "links", "remember", "note", "link", "attach", "undo",
    "start", "import",
];

/// What a tool does to the folder, as its annotations declare it.
#[derive(Clone, Copy, PartialEq)]
enum Does {
    /// Reads only.
    Reads,
    /// Writes, adding only, a second same call changing something again.
    Writes,
    /// Writes, adding only, a second same call changing nothing.
    WritesOnce,
}

/// One argument of a tool: its key, a text or a list of texts, what it is, whether required.
struct Arg(&'static str, bool, &'static str, bool);

/// One tool's card: its name, when to use it, its arguments, its annotations.
fn card(name: &str, description: &str, args: &[Arg], does: Does) -> Json {
    let properties = args.iter().map(|Arg(key, list, said, _)| {
        let mut schema = match list {
            true => vec![
                ("type", Json::str("array")),
                ("items", Json::obj(vec![("type", Json::str("string"))])),
            ],
            false => vec![("type", Json::str("string"))],
        };
        schema.push(("description", Json::str(said)));
        (*key, Json::obj(schema))
    });
    let required = args.iter().filter(|a| a.3).map(|a| Json::str(a.0)).collect();
    let mut hints = vec![("readOnlyHint", Json::Bool(does == Does::Reads))];
    if does != Does::Reads {
        hints.push(("destructiveHint", Json::Bool(false)));
        hints.push(("idempotentHint", Json::Bool(does == Does::WritesOnce)));
    }
    hints.push(("openWorldHint", Json::Bool(false)));
    Json::obj(vec![
        ("name", Json::str(name)),
        ("description", Json::str(description)),
        (
            "inputSchema",
            Json::obj(vec![
                ("type", Json::str("object")),
                ("properties", Json::obj(properties.collect())),
                ("required", Json::Array(required)),
            ]),
        ),
        ("annotations", Json::obj(hints)),
    ])
}

const PIECE: &str = "a number such as M-0002, or a person's or a tag's name";

fn tool_list() -> Vec<Json> {
    vec![
        card(
            "where",
            "Says which memory folder Lorisse reads, how it was found, and Lorisse's version. Use it when the person asks where Lorisse is or which folder it reads.",
            &[],
            Does::Reads,
        ),
        card(
            "plop",
            "Starts Lorisse: gives the person's own intro (how Lorisse works for them), then the state of their memory: the folder, its repositories, the counts, the memories filed last, then the rules of keeping. Use it when the person says \"Lorisse\" or \"plop\", alone or to start a conversation, and follow the intro it gives.",
            &[],
            Does::Reads,
        ),
        card(
            "search",
            "Searches the person's memory (their memories, people and tags) for a text and lists what holds it, by number and title. Use it first whenever the person asks what they know, decided or noted, or names a person or a subject.",
            &[Arg("text", false, "the words to look for, capitals ignored", true)],
            Does::Reads,
        ),
        card(
            "read",
            "Reads one piece of the person's memory in full: a memory by its number, a person by name, or a tag by its word. Use it after search, to see what a piece says.",
            &[Arg("piece", false, "a number such as M-0002, a person's name, or a tag's word", true)],
            Does::Reads,
        ),
        card(
            "links",
            "Lists what a memory, person or tag links to and what links to it, by number and title. Use it to follow a piece to the memories and people around it.",
            &[Arg("piece", false, PIECE, true)],
            Does::Reads,
        ),
        card(
            "remember",
            "Files a new memory in the person's memory folder, as one commit, and gives its number. Use it only when the person asks to remember something, or says yes to your offer to file it.",
            &[
                Arg("title", false, "a short title, one line", true),
                Arg("text", false, "what to remember, in the person's words", true),
                Arg("kind", false, "one of the folder's kinds, such as fact or decision; the folder's first when left out", false),
                Arg("tags", true, "the memory's tags", false),
                Arg("links", true, "the pieces it links to: numbers, or a person's or a tag's name", false),
                Arg("repository", false, "the repository it goes to, by name; the folder's own when left out", false),
            ],
            Does::Writes,
        ),
        card(
            "note",
            "Adds one dated line at the end of a memory's or a person's text, as one commit; nothing else changes. Use it only when the person asks to add something to a piece.",
            &[
                Arg("piece", false, "a number such as M-0002, or a person's name", true),
                Arg("text", false, "the line to add", true),
            ],
            Does::Writes,
        ),
        card(
            "link",
            "Links one piece of the person's memory to another, as one commit; a link already there changes nothing. Use it only when the person asks to link two pieces.",
            &[Arg("from", false, PIECE, true), Arg("to", false, PIECE, true)],
            Does::WritesOnce,
        ),
        card(
            "attach",
            "Keeps a file of the person's machine with the memory or the person it is about: a copy goes in the memory folder beside that piece, as one commit, and the original stays where it was. Use it when the person gives a document's path or says to keep a file; file the memory first if it does not exist yet.",
            &[
                Arg("piece", false, "a number such as M-0002, or a person's name", true),
                Arg("path", false, "the full path of the file on this machine", true),
            ],
            Does::Writes,
        ),
        card(
            "undo",
            "Takes back Lorisse's last change in the memory folder, or the one named, as a new commit; the history keeps both, and a change the person made is never taken back. Use it when the person asks to undo or take back a change.",
            &[Arg("change", false, "the commit, or the number of the piece, whose change to take back; the last change when left out", false)],
            Does::Writes,
        ),
        card(
            "start",
            "Makes a new person's memory folder ready: a folder named Lorisse in their home folder, or the place named, with its settings, the first set of rules and their intro written from their answers, put under git when git is on the machine; this machine then remembers it. Use it when Lorisse says there is no memory folder and the person wants one, once you have asked their name, what they want to remember and how they like to be answered.",
            &[
                Arg("place", false, "the full path of the folder to make; a folder named Lorisse in the home folder when left out", false),
                Arg("name", false, "the person's name", false),
                Arg("remember", false, "what the person wants Lorisse to remember, in their words", false),
                Arg("answer_style", false, "how the person likes to be answered, in their words", false),
            ],
            Does::WritesOnce,
        ),
        card(
            "import",
            "Files what another assistant remembers of the person: each line \"[YYYY-MM-DD] - text\" (or \"[unknown] - text\") under Identity, Career, Projects or Preferences becomes a memory with its date and its category, all in one step; the lines under Instructions are given back as proposals for the intro, never filed or followed. Use it when the person pastes such lines, or asks to import them.",
            &[Arg("text", false, "the other assistant's answer, whole, as the person pasted it", true)],
            Does::Writes,
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
