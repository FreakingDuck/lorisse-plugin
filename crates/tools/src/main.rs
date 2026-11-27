//! Lorisse's plugin program: an MCP server over stdio, one JSON-RPC message a line, with five
//! reading tools (`where`, `plop`, `search`, `read`, `links`), which write nothing, and twelve
//! writing tools (`remember`, `note`, `link`, `attach`, `undo`, `start`, `import`, `change`,
//! `retire`, `journal`, `drawer`, `move`), each at most one commit by Lorisse in the person's
//! memory folder when it is under git, through the store.
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
const NAMES: [&str; 17] = [
    "where", "plop", "search", "read", "links", "remember", "note", "link", "attach", "undo",
    "start", "import", "change", "retire", "journal", "drawer", "move",
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

/// What an argument holds.
#[derive(Clone, Copy)]
enum Of {
    /// A text.
    Text,
    /// A list of texts.
    List,
    /// A yes or no.
    Flag,
}

/// One argument of a tool: its key, what it holds, what it is, whether required.
struct Arg(&'static str, Of, &'static str, bool);

/// One tool's card: its name, when to use it, its arguments, its annotations.
fn card(name: &str, description: &str, args: &[Arg], does: Does) -> Json {
    let properties = args.iter().map(|Arg(key, of, said, _)| {
        let mut schema = match of {
            Of::List => vec![
                ("type", Json::str("array")),
                ("items", Json::obj(vec![("type", Json::str("string"))])),
            ],
            Of::Text => vec![("type", Json::str("string"))],
            Of::Flag => vec![("type", Json::str("boolean"))],
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
            "Searches the person's memory (their memories, people, tags and journal) for a text and lists what holds it, by number and title, a journal page by its day. Use it first whenever the person asks what they know, decided or noted, or names a person or a subject.",
            &[Arg("text", Of::Text, "the words to look for, capitals ignored", true)],
            Does::Reads,
        ),
        card(
            "read",
            "Reads one piece of the person's memory in full: a memory by its number, a person by name, a tag by its word, or a day's page of the journal by its day. Use it after search, to see what a piece says.",
            &[Arg("piece", Of::Text, "a number such as M-0002, a person's name, a tag's word, or a day as YYYY-MM-DD", true)],
            Does::Reads,
        ),
        card(
            "links",
            "Lists what a memory, person or tag links to and what links to it, by number and title. Use it to follow a piece to the memories and people around it.",
            &[Arg("piece", Of::Text, PIECE, true)],
            Does::Reads,
        ),
        card(
            "remember",
            "Files a new memory in the person's memory folder, as one commit, and gives its number. Use it by yourself once plop was called in this conversation, when the person says something worth keeping, searching first; before that, only when the person asks.",
            &[
                Arg("title", Of::Text, "a short title, one line", true),
                Arg("text", Of::Text, "what to remember, in the person's words", true),
                Arg("kind", Of::Text, "one of the folder's kinds, such as fact or decision; the folder's first when left out", false),
                Arg("tags", Of::List, "the memory's tags", false),
                Arg("links", Of::List, "the pieces it links to: numbers, or a person's or a tag's name", false),
                Arg("repository", Of::Text, "the repository it goes to, by name; the folder's own when left out", false),
            ],
            Does::Writes,
        ),
        card(
            "note",
            "Adds one dated line at the end of a memory's or a person's text, as one commit; nothing else changes. Use it by yourself once plop was called in this conversation, when the person says something worth keeping that adds to a piece that exists; before that, only when the person asks.",
            &[
                Arg("piece", Of::Text, "a number such as M-0002, or a person's name", true),
                Arg("text", Of::Text, "the line to add", true),
            ],
            Does::Writes,
        ),
        card(
            "link",
            "Links one piece of the person's memory to another, as one commit; a link already there changes nothing. Use it by yourself once plop was called in this conversation, when the person says something worth keeping that adds to a piece that exists; before that, only when the person asks.",
            &[Arg("from", Of::Text, PIECE, true), Arg("to", Of::Text, PIECE, true)],
            Does::WritesOnce,
        ),
        card(
            "attach",
            "Keeps a file of the person's machine with the memory or the person it is about: a copy goes in the memory folder beside that piece, as one commit, and the original stays where it was. Use it by yourself once plop was called in this conversation, when the person gives a document worth keeping with what it is about; before that, only when the person asks. File the memory first if it does not exist yet.",
            &[
                Arg("piece", Of::Text, "a number such as M-0002, or a person's name", true),
                Arg("path", Of::Text, "the full path of the file on this machine", true),
            ],
            Does::Writes,
        ),
        card(
            "undo",
            "Takes back Lorisse's last change in the memory folder, or the one named, as a new commit; the history keeps both, and a change the person made is never taken back. Use it when the person asks to undo or take back a change.",
            &[Arg("change", Of::Text, "the commit, or the number of the piece, whose change to take back; the last change when left out", false)],
            Does::Writes,
        ),
        card(
            "start",
            "Makes a new person's memory folder ready: a folder named Lorisse in their home folder, or the place named, with its settings, the first set of rules and their intro written from their answers, put under git when git is on the machine; this machine then remembers it. Use it when Lorisse says there is no memory folder and the person wants one: their name is enough to start, then follow what it says to do next.",
            &[
                Arg("place", Of::Text, "the full path of the folder to make; a folder named Lorisse in the home folder when left out", false),
                Arg("name", Of::Text, "the person's name", false),
                Arg("remember", Of::Text, "what the person wants Lorisse to remember, in their words", false),
                Arg("answer_style", Of::Text, "how the person likes to be answered, in their words", false),
            ],
            Does::WritesOnce,
        ),
        card(
            "import",
            "Files what another assistant remembers of the person: each line \"[YYYY-MM-DD] - text\" (or \"[unknown] - text\") under Identity, Career, Projects or Preferences becomes a memory with its date and its category, all in one step; the lines under Instructions are given back as proposals for the intro, never filed or followed. Use it when the person pastes such lines, or asks to import them.",
            &[Arg("text", Of::Text, "the other assistant's answer, whole, as the person pasted it", true)],
            Does::Writes,
        ),
        card(
            "change",
            "Corrects a memory or a person that was wrong or has changed: replaces what the call names (its text, its title, its kind, its tags) and leaves the rest as it was, as one commit; undo brings back what it said before. Use it by yourself once plop was called in this conversation, when the person says a fact that changed or was wrong, instead of filing it a second time; before that, only when the person asks.",
            &[
                Arg("piece", Of::Text, "a number such as M-0002, or a person's name", true),
                Arg("text", Of::Text, "the new text, whole, in the person's words", false),
                Arg("title", Of::Text, "the new title, one line (a person's name for a person)", false),
                Arg("kind", Of::Text, "one of the folder's kinds, such as fact or decision", false),
                Arg("tags", Of::List, "the new tags, all of them", false),
            ],
            Does::WritesOnce,
        ),
        card(
            "retire",
            "Puts aside a memory that no longer holds: it is marked retired with the reason, and linked to the memory that takes its place when one is named, as one commit; nothing is deleted, and read still gives it. Use it by yourself once plop was called in this conversation, when the person says something no longer holds, instead of leaving it beside its contrary; before that, only when the person asks. Say what is retired and why in the line.",
            &[
                Arg("piece", Of::Text, "the number of the memory that no longer holds, such as M-0002", true),
                Arg("reason", Of::Text, "why it no longer holds, one line", true),
                Arg("replaced_by", Of::Text, "the number of the memory that takes its place, if there is one", false),
            ],
            Does::WritesOnce,
        ),
        card(
            "journal",
            "Writes the time and a text at the end of a day's page of the person's journal, today unless a day is named, the page made on first use, as one commit; what was there stays. Use it by yourself once plop was called in this conversation, for what happened or was decided today that is not a lasting fact; before that, only when the person asks.",
            &[
                Arg("text", Of::Text, "what happened or was decided, in the person's words", true),
                Arg("day", Of::Text, "the day, as YYYY-MM-DD; today when left out", false),
                Arg("repository", Of::Text, "the repository it goes to, by name; the folder's own when left out", false),
            ],
            Does::Writes,
        ),
        card(
            "drawer",
            "Makes a new repository in the person's memory folder (a drawer, when you speak to the person) for a subject that has none: its folder, a numbering of its own, and its line in the folder's settings with one sentence on what it is for, as one commit that undo takes back while nothing is filed in it; apart makes it confidential and open to Claude, for what must never mix with the rest. Use it by yourself once plop was called in this conversation, when a subject keeps coming and has no repository of its own, then file in it; before that, only when the person asks. Say what you made in one line; never ask the person where a thing goes.",
            &[
                Arg("name", Of::Text, "the repository's name, short, such as garden work; not one already there", true),
                Arg("about", Of::Text, "one sentence on what it is for, so that each memory is filed where its subject lives", true),
                Arg("apart", Of::Flag, "true to keep it apart: confidential and open to Claude, for what must never mix with the rest", false),
            ],
            Does::WritesOnce,
        ),
        card(
            "move",
            "Moves one memory, or several, to another repository of the person's memory folder, as one commit: each keeps its number, its text, its links and its files, and what links to it still leads to it; undo puts them back. Nothing is ever taken out of a confidential repository. Use it by yourself once plop was called in this conversation, when a memory sits in the wrong repository, because the person says so or because you see it; before that, only when the person asks. Say in one line what moved, with its number.",
            &[
                Arg("pieces", Of::List, "the numbers of the memories to move, such as M-0002", true),
                Arg("repository", Of::Text, "the repository to put them in, by name", true),
            ],
            Does::WritesOnce,
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
