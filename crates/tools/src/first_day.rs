//! The first day: `start` makes a new person's memory folder ready and this machine remembers
//! it; `import` files the dated lines another assistant gave of the person, in one step, and
//! gives back the lines that are instructions without following them.

use std::fs;
use std::path::{Path, PathBuf};

use lorisse_guard::{FIRST_RULES, RULES_FILE};
use lorisse_store::first;
use lorisse_store::{Dated, CONFIG_FILE, IDS_FILE};

use crate::folder::{self, Folder};
use crate::json::Json;
use crate::tools::{Answer, Memory, DATA_LINE, INTRO_FILE};

/// The memory folder's name in the home folder, when no place is named.
pub const FOLDER_NAME: &str = "Lorisse";

/// A new folder's settings: plain words, nothing of anyone else's.
const CONFIG: &str = "# The settings of this memory folder, in plain words: change them as you like.
kinds: [fact, decision, preference, idea, need]
memory_statuses: [raw, sorted, retired]
task_statuses: [todo, in-progress, waiting, done, retired]
priorities: [high, medium, low]
categories: [home, family, work, health, money, admin]
sources: [chat, import, phone, app]
person_sections: [What I know, How I handle]
";

/// A new folder's counters: no number given yet.
const IDS: &str = "{\n  \"M\": 0,\n  \"T\": 0\n}\n";

/// The empty folders a memory needs.
const FOLDERS: [&str; 3] = ["memories", "people", "tags"];

/// The request the person hands to another assistant (C-11.5), given whole in one block.
pub const REQUEST: &str = "I am moving my memory to a new assistant, and I would like to take with me what you know about me.
Please list everything you remember about me from our past conversations, one thing a line, each line in this form:
[YYYY-MM-DD] - what you remember
The date is the day you learned it; write [unknown] when you do not know it.
Put the lines under these five headings, in this order: Instructions, Identity, Career, Projects, Preferences.
Under Instructions, put how I asked you to answer me or to behave; under the four others, what you know of me.
Keep my own words where you can, and leave out passwords, keys and anything you only guess.
Give the whole list in one code block, and nothing else.";

/// The sentence before the block.
const HAND_OVER: &str = "To bring what another assistant already knows of you, copy the text below whole, give it to that assistant, then paste its answer here after \"Import this:\".";

/// What `start` expects, said when an argument is of the wrong kind.
pub const START_EXPECTED: &str = "start takes, each as a text and each only if given: \"place\", the full path of the folder to make, \"name\", \"remember\", what the person wants to remember, and \"answer_style\", how they like to be answered.";

/// The headings of the other assistant's answer, and the tag each category is kept as.
const CATEGORIES: [(&str, &str); 4] = [
    ("identity", "identity"),
    ("career", "career"),
    ("projects", "projects"),
    ("preferences", "preferences"),
];

/// A title's most words.
const TITLE_WORDS: usize = 8;

// =============================================================================
// start
// =============================================================================

/// The three answers and the place, as `start` was given them.
#[derive(Default)]
struct Asked {
    place: Option<String>,
    name: Option<String>,
    remember: Option<String>,
    style: Option<String>,
}

/// A text argument: `Ok(None)` when absent, null or blank, `Err` when of another kind.
fn text_arg(args: Option<&Json>, key: &str) -> Result<Option<String>, ()> {
    match args.and_then(|a| a.get(key)) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::Str(s)) => Ok(Some(one_line(s)).filter(|s| !s.is_empty())),
        Some(_) => Err(()),
    }
}

/// One line: whitespace folded to single spaces, trimmed.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A sentence: one line, ending with its stop.
fn sentence(text: &str) -> String {
    let text = one_line(text);
    match text.ends_with(['.', '!', '?']) {
        true => text,
        false => format!("{text}."),
    }
}

/// The intro written from the three answers; an answer not given says where to write it.
fn intro(asked: &Asked) -> String {
    let missing = "not written yet; write it on this line.";
    let name = match &asked.name {
        Some(name) => sentence(&format!("My name is {name}")),
        None => format!("My name: {missing}"),
    };
    let line = |label: &str, answer: &Option<String>| match answer {
        Some(text) => sentence(&format!("{label}: {text}")),
        None => format!("{label}: {missing}"),
    };
    format!(
        "# How Lorisse works for me\n\n{name}\n{}\n{}\n\nThis page is mine: Lorisse reads it at the start of each conversation, and I change it as I like.\n",
        line("What I want Lorisse to remember", &asked.remember),
        line("How I like to be answered", &asked.style)
    )
}

/// The address of the person's commits: their name's letters, at a domain that is no real one.
fn address(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        match c.is_ascii_alphanumeric() {
            true => slug.push(c),
            false if !slug.is_empty() && !slug.ends_with('-') => slug.push('-'),
            false => {}
        }
    }
    let slug = slug.trim_end_matches('-');
    let slug = if slug.is_empty() { "me" } else { slug };
    format!("{slug}@memory.invalid")
}

/// The last lines of `start`: the sentence, then the request in one fenced block.
fn hand_over() -> String {
    format!("{HAND_OVER}\n```\n{REQUEST}\n```")
}

/// This machine told where the memory folder is.
fn remember_folder(home: &Path, place: &Path) -> Result<PathBuf, String> {
    let file = home.join(folder::REMEMBERED);
    let text = format!("{}\n", place.display());
    fs::write(&file, text).map_err(|e| format!("{}: {e}", folder::shown(&file)))?;
    Ok(file)
}

/// The folder's files and folders written; the first error said.
fn make(place: &Path, asked: &Asked) -> Result<(), String> {
    let at = |p: &Path, e: std::io::Error| format!("{}: {e}", folder::shown(&p.to_path_buf()));
    fs::create_dir_all(place).map_err(|e| at(place, e))?;
    for dir in FOLDERS {
        let path = place.join(dir);
        fs::create_dir_all(&path).map_err(|e| at(&path, e))?;
    }
    let files = [
        (CONFIG_FILE, CONFIG.to_string()),
        (IDS_FILE, IDS.to_string()),
        (RULES_FILE, FIRST_RULES.to_string()),
        (INTRO_FILE, intro(asked)),
    ];
    for (name, text) in files {
        let path = place.join(name);
        fs::write(&path, text).map_err(|e| at(&path, e))?;
    }
    Ok(())
}

/// `start`: a new memory folder made ready, or the one this machine already has named.
pub fn start(args: Option<&Json>) -> Answer {
    let read = || -> Result<Asked, ()> {
        Ok(Asked {
            place: text_arg(args, "place")?,
            name: text_arg(args, "name")?,
            remember: text_arg(args, "remember")?,
            style: text_arg(args, "answer_style")?,
        })
    };
    let Ok(asked) = read() else {
        return Answer::error(START_EXPECTED.to_string());
    };
    if let Folder::At(path, found) = folder::find() {
        return Answer::ok(format!(
            "This machine already has a memory folder: {}, found by {}; nothing was made.",
            folder::shown(&path),
            found.words()
        ));
    }
    let Some(home) = folder::home().filter(|h| h.is_dir()) else {
        return Answer::error("Lorisse cannot find this machine's home folder, where it remembers the memory folder: nothing was made.".to_string());
    };
    let place = match &asked.place {
        Some(given) if Path::new(given).is_absolute() => PathBuf::from(given),
        Some(given) => home.join(given),
        None => home.join(FOLDER_NAME),
    };
    let shown = folder::shown(&place);
    if folder::is_memory_folder(&place) {
        return match remember_folder(&home, &place) {
            Ok(_) => Answer::ok(format!(
                "The memory folder {shown} was already there: it is kept as it is, and this machine now remembers it, so Lorisse finds it in every conversation.\n\n{}",
                hand_over()
            )),
            Err(said) => Answer::error(format!("The memory folder {shown} is there, but this machine cannot remember it: {said}.")),
        };
    }
    if place.exists() && !place.is_dir() {
        return Answer::error(format!("{shown} is a file, not a folder: nothing was made."));
    }
    let holds_files = fs::read_dir(&place).map(|mut d| d.next().is_some()).unwrap_or(false);
    if holds_files {
        return Answer::error(format!(
            "{shown} already holds files and is not a memory folder: Lorisse never takes such a place over; name an empty folder or a new one. Nothing was made."
        ));
    }
    if let Err(said) = make(&place, &asked) {
        return Answer::error(format!("The memory folder could not be made: {said}."));
    }
    let mut lines = vec![
        format!("The memory folder is ready: {shown}."),
        format!(
            "It holds its settings ({CONFIG_FILE}), its counters ({IDS_FILE}), the first set of rules ({RULES_FILE}), the person's intro ({INTRO_FILE}) and the empty folders {}.",
            FOLDERS.join(", ")
        ),
    ];
    lines.push(under_git(&place, &asked));
    match remember_folder(&home, &place) {
        Ok(file) => lines.push(format!(
            "This machine remembers it, in {}: Lorisse finds it in every conversation, with no setting to fill.",
            folder::shown(&file)
        )),
        Err(said) => lines.push(format!("This machine could not remember it ({said}): give it in the plugin's setting \"memory_folder\".")),
    }
    lines.push(String::new());
    lines.push(hand_over());
    Answer::ok(lines.join("\n"))
}

/// The folder put under git when git is there; the one sentence that says what was done.
fn under_git(place: &Path, asked: &Asked) -> String {
    if !first::git_found() {
        return "Git is not on this machine: with it, every change would be kept in a history and any one could be taken back; it can be added later.".to_string();
    }
    let name = asked.name.clone().unwrap_or_else(|| "Me".to_string());
    let subject = "start: the memory folder made ready";
    match first::put_under_git(place, &name, &address(&name), subject) {
        Ok(_) => format!(
            "It is under git, on this machine only: its first state is one commit signed as {name}, and each change Lorisse makes will be one more, which undo can take back."
        ),
        Err(e) => format!("Git could not put it under git ({e}): the folder is ready all the same, without a history."),
    }
}

// =============================================================================
// import
// =============================================================================

/// Where a line of the answer falls.
#[derive(Clone, Copy, PartialEq)]
enum Under {
    Nothing,
    Instructions,
    Category(&'static str),
    Unknown,
}

/// A heading's words: `## Identity`, `**Identity**`, `Identity:`; `None` when the line is none.
fn heading(line: &str) -> Option<Under> {
    if line.contains('[') {
        return None;
    }
    let word = line.trim_start_matches('#').trim().trim_matches(['*', '_', ':']).trim().to_lowercase();
    if word == "instructions" || word == "instruction" {
        return Some(Under::Instructions);
    }
    let found = CATEGORIES.iter().find(|(name, _)| word == *name || word == name.trim_end_matches('s'));
    if let Some((_, tag)) = found {
        return Some(Under::Category(tag));
    }
    let marked = line.starts_with('#') || line.ends_with(':') || (line.starts_with("**") && line.ends_with("**"));
    marked.then_some(Under::Unknown)
}

/// Whether `text` is a day `YYYY-MM-DD` that exists.
pub(crate) fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit()).then(|| s.parse::<u32>().ok()).flatten();
    let (Some(year), Some(month), Some(day)) = (digits(&text[..4]), digits(&text[5..7]), digits(&text[8..])) else {
        return false;
    };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    (1..=12).contains(&month) && day >= 1 && day <= days[month as usize - 1]
}

/// A line `[YYYY-MM-DD] - text` or `[unknown] - text`: its date and its text.
fn dated(line: &str) -> Option<(Option<String>, String)> {
    let line = line.trim_start_matches(['-', '*', '+']).trim_start();
    let rest = line.strip_prefix('[')?;
    let (inside, text) = rest.split_once(']')?;
    let inside = inside.trim();
    let date = match inside.eq_ignore_ascii_case("unknown") {
        true => None,
        false if is_date(inside) => Some(inside.to_string()),
        false => return None,
    };
    let text = text.trim_start().trim_start_matches(['-', ':', '\u{2013}', '\u{2014}']);
    let text = text.trim();
    (!text.is_empty()).then(|| (date, text.to_string()))
}

/// Whether a text has a secret's shape: a password or a key named, a long key, a card number.
pub(crate) fn looks_secret(text: &str) -> bool {
    let lower = text.to_lowercase();
    let words = ["password", "passwd", "passcode", "api key", "api_key", "secret key", "private key", "access token", "-----begin"];
    if words.iter().any(|w| lower.contains(w)) {
        return true;
    }
    let key_like = text.split_whitespace().any(|w| {
        let w = w.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        w.len() >= 20
            && w.chars().all(|c| c.is_ascii_alphanumeric() || "-_+/=".contains(c))
            && w.chars().any(|c| c.is_ascii_digit())
            && w.chars().any(|c| c.is_ascii_alphabetic())
    });
    let mut run = 0;
    let mut longest = 0;
    for c in text.chars().filter(|c| *c != ' ' && *c != '-') {
        run = if c.is_ascii_digit() { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    key_like || longest >= 13
}

/// A short title from a text's first words.
fn title_of(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut title = words.iter().take(TITLE_WORDS).copied().collect::<Vec<_>>().join(" ");
    title = title.trim_end_matches(['.', ',', ';', ':', '!', '?']).to_string();
    if let Some((at, _)) = title.char_indices().nth(60) {
        title.truncate(at);
    }
    match words.len() > TITLE_WORDS {
        true => format!("{}...", title.trim_end()),
        false => title,
    }
}

/// What the answer's lines give: the memories, the instructions, the lines left out.
#[derive(Default)]
struct Read {
    memories: Vec<Dated>,
    instructions: Vec<String>,
    unreadable: usize,
    secrets: usize,
}

fn read_lines(text: &str) -> Read {
    let mut read = Read::default();
    let mut under = Under::Nothing;
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("```") {
            continue;
        }
        if let Some(found) = heading(line) {
            under = found;
            continue;
        }
        let Some((date, text)) = dated(line) else {
            read.unreadable += 1;
            continue;
        };
        match under {
            Under::Instructions => read.instructions.push(text),
            Under::Category(_) if looks_secret(&text) => read.secrets += 1,
            Under::Category(tag) => read.memories.push(Dated { title: title_of(&text), text, tag: tag.to_string(), date }),
            Under::Nothing | Under::Unknown => read.unreadable += 1,
        }
    }
    read
}

/// A count with its word, one or many.
fn counted(n: usize, one: &str, many: &str) -> String {
    match n {
        1 => format!("1 {one}"),
        _ => format!("{n} {many}"),
    }
}

/// `import`: the dated lines filed in one step, the instructions given back, the rest counted.
pub fn import(memory: &mut Memory, text: &str) -> Answer {
    let read = read_lines(text);
    let mut lines = Vec::new();
    if read.memories.is_empty() {
        lines.push("No memory was filed: no dated line was found under Identity, Career, Projects or Preferences.".to_string());
    } else {
        let done = match memory.store.import(&read.memories) {
            Ok(done) => done,
            Err(e) => return Answer::error(format!("Nothing was written: {}.", e.to_string().trim_end_matches('.'))),
        };
        let count = counted(done.ids.len(), "memory", "memories");
        lines.push(match &done.commit {
            Some(commit) => format!(
                "Imported: {count} filed, in one commit of {} ({}); one undo takes the whole import back.",
                done.repo,
                commit.chars().take(7).collect::<String>()
            ),
            None => format!("Imported: {count} filed."),
        });
        lines.push(DATA_LINE.to_string());
        for (id, line) in done.ids.iter().zip(&read.memories) {
            let date = line.date.as_deref().unwrap_or("date unknown");
            let label = memory.label_of(id, &done.repo);
            lines.push(format!("- {label} · {date} · {} · {}", line.tag, line.title));
        }
    }
    let left = read.unreadable + read.secrets;
    lines.push(match left {
        0 => "Nothing was left out.".to_string(),
        _ => format!(
            "Left out: {}, never guessed: {} that could not be read, {} with a secret's shape.",
            counted(left, "line", "lines"),
            read.unreadable,
            read.secrets
        ),
    });
    match read.instructions.is_empty() {
        true => lines.push("No line was under Instructions.".to_string()),
        false => {
            lines.push(format!(
                "Given back, not filed and not followed: {} under Instructions, proposals for the person's intro ({INTRO_FILE}), to add only if the person says so:",
                counted(read.instructions.len(), "line", "lines")
            ));
            lines.extend(read.instructions.iter().map(|i| format!("- {i}")));
        }
    }
    Answer::ok(lines.join("\n"))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_read_with_its_date_or_none() {
        assert_eq!(dated("[2024-02-29] - Lives by the sea."), Some((Some("2024-02-29".into()), "Lives by the sea.".into())));
        assert_eq!(dated("- [unknown] - Likes tea"), Some((None, "Likes tea".into())));
        assert_eq!(dated("[2023-02-29] - no such day"), None);
        assert_eq!(dated("Likes tea"), None);
        assert_eq!(dated("[2024-01-01] - "), None);
    }

    #[test]
    fn a_heading_is_found_in_its_usual_shapes() {
        assert!(matches!(heading("## Identity"), Some(Under::Category("identity"))));
        assert!(matches!(heading("**Career**"), Some(Under::Category("career"))));
        assert!(matches!(heading("Instructions:"), Some(Under::Instructions)));
        assert!(matches!(heading("# Hobbies"), Some(Under::Unknown)));
        assert!(heading("[2024-01-01] - Projects: a boat").is_none());
    }

    #[test]
    fn a_title_is_the_first_words() {
        assert_eq!(title_of("Lives by the sea."), "Lives by the sea");
        assert_eq!(title_of("one two three four five six seven eight nine"), "one two three four five six seven eight...");
    }

    #[test]
    fn a_secret_s_shape_is_seen() {
        assert!(looks_secret("My bank password is hunter2"));
        assert!(looks_secret("card 4111 1111 1111 1111"));
        assert!(looks_secret("key sk1234567890abcdefghijklmnop"));
        assert!(!looks_secret("Works as a nurse since 2019"));
    }
}
