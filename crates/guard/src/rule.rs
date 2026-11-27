//! A rule written: one service, its tools and one list, in the person's `rules.yaml`, changed as
//! text: the tools taken out of the service's other lists, then added under the list asked,
//! every other byte of the file left as it was. Without a file, the first set is the text
//! changed. The new text is read back with the crate's own reader before it is handed over: a
//! text that does not read as the rule means is never written. And what the file rules, said.
//! Flow: `write_rule_with` → `changed` (`scan`, take out, add) → `parse` again → written.

use std::collections::{BTreeMap, BTreeSet};
use std::io::ErrorKind;
use std::path::Path;

use crate::rules::{self, plain, List, Loaded, Rules, Source, FIRST_RULES, RULES_FILE};

// =============================================================================
// Interface
// =============================================================================

/// What a rule written did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleWritten {
    /// The service, as the call named it.
    pub service: String,
    /// The list its tools went to.
    pub list: List,
    /// The file's text after the rule.
    pub text: String,
    /// Whether the file was written: `false` when it already said so.
    pub changed: bool,
    /// Whether the file was made by this rule, from the first set.
    pub made: bool,
    /// What the file now says for that service: `pass fetch, search; ask move`.
    pub said: String,
}

/// One service the person's file rules, and its tools under each list that names any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceRuled {
    /// The service, as the file first writes it.
    pub service: String,
    /// Each list naming tools of it, in the order pass, ask, never, with its tools as written.
    pub lists: Vec<(List, Vec<String>)>,
}

/// What the person's rules say of the other services than Lorisse's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ruling {
    /// No rules file: the first set holds, which rules Lorisse's own tools only.
    NoFile,
    /// The file is there and cannot be read, with why.
    Unreadable(String),
    /// The services it rules besides Lorisse's own, in the order it first names them.
    Services(Vec<ServiceRuled>),
}

/// The rule written in `folder`'s `rules.yaml` with the file system.
pub fn write_rule(folder: &Path, service: &str, tools: &[String], list: &str) -> Result<RuleWritten, String> {
    let path = folder.join(RULES_FILE);
    write_rule_with(folder, service, tools, list, |text| {
        std::fs::write(&path, text).map_err(|e| format!("Your rules file, {RULES_FILE}, could not be written ({e}): nothing was written."))
    })
}

/// The rule's new text made and read back, then handed to `write` when it changes the file; a
/// refusal is one sentence, and nothing is handed over.
pub fn write_rule_with<F>(folder: &Path, service: &str, tools: &[String], list: &str, write: F) -> Result<RuleWritten, String>
where
    F: FnOnce(&str) -> Result<(), String>,
{
    let Some(list) = List::named(list) else {
        return Err(format!(
            "A rule goes under pass, ask or never, and \"{}\" is none of them: nothing was written.",
            one_line(list)
        ));
    };
    let service = one_line(service);
    if service.is_empty() {
        return Err("A rule names the service its tools belong to: nothing was written.".to_string());
    }
    let mut named: Vec<String> = Vec::new();
    for tool in tools.iter().map(|t| one_line(t)).filter(|t| !t.is_empty()) {
        if !named.iter().any(|n| same_tool(n, &tool)) {
            named.push(tool);
        }
    }
    if named.is_empty() {
        return Err("A rule names at least one tool: nothing was written.".to_string());
    }
    let unreadable = |why: String| {
        format!("Your rules file, {RULES_FILE}, cannot be read ({why}): it is left as it is, and nothing was written.")
    };
    let (before, made) = match std::fs::read_to_string(folder.join(RULES_FILE)) {
        Ok(text) => (text, false),
        Err(e) if e.kind() == ErrorKind::NotFound => (FIRST_RULES.to_string(), true),
        Err(e) => return Err(unreadable(e.to_string())),
    };
    let read = rules::parse(&before).map_err(unreadable)?;
    let left = |why: &str| format!("Your rules file, {RULES_FILE}, is left as it is: {why}; nothing was written.");
    let after = changed(&before, &service, &named, list).map_err(|why| left(&why))?;
    let reread = rules::parse(&after).map_err(|_| left("the new text would not read"))?;
    if shape(&reread) != expected(&read, &service, &named, list) {
        return Err(left("the new text would not read as the rule means"));
    }
    let changed = made || after != before;
    if changed {
        write(&after)?;
    }
    let said = said_of(&reread, &service);
    Ok(RuleWritten { service, list, text: after, changed, made, said })
}

/// What the person's rules file says of the other services than Lorisse's own.
pub fn ruled(folder: Option<&Path>) -> Ruling {
    let rules = match rules::load(folder) {
        Loaded::Unreadable(why) => return Ruling::Unreadable(why),
        Loaded::Rules(_, Source::First) => return Ruling::NoFile,
        Loaded::Rules(rules, Source::Yours) => rules,
    };
    let mut out: Vec<ServiceRuled> = Vec::new();
    for list in List::ALL {
        for entry in rules.of(list) {
            if plain(&entry.name) == "lorisse" || plain(&entry.name).is_empty() {
                continue;
            }
            let at = match out.iter().position(|s| plain(&s.service) == plain(&entry.name)) {
                Some(at) => at,
                None => {
                    out.push(ServiceRuled { service: entry.name.trim().to_string(), lists: Vec::new() });
                    out.len() - 1
                }
            };
            let lists = &mut out[at].lists;
            if !lists.iter().any(|(l, _)| *l == list) {
                lists.push((list, Vec::new()));
            }
            let Some((_, tools)) = lists.iter_mut().find(|(l, _)| *l == list) else { continue };
            for tool in entry.tools.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
                if !tools.iter().any(|t| same_tool(t, tool)) {
                    tools.push(tool.to_string());
                }
            }
        }
    }
    for service in &mut out {
        service.lists.retain(|(_, tools)| !tools.is_empty());
    }
    out.retain(|s| !s.lists.is_empty());
    Ruling::Services(out)
}

// =============================================================================
// Implementation: what the rule means, read
// =============================================================================

/// The file as the guard reads it: each list, each service by its plain name, its tools.
type Shape = BTreeMap<&'static str, BTreeMap<String, BTreeSet<String>>>;

/// A tool's name as compared: spaces around cut, capitals ignored.
fn same_tool(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn shape(rules: &Rules) -> Shape {
    let mut out = Shape::new();
    for list in List::ALL {
        let services = out.entry(list.name()).or_default();
        for entry in rules.of(list) {
            let tools = services.entry(plain(&entry.name)).or_default();
            tools.extend(entry.tools.iter().map(|t| t.trim().to_ascii_lowercase()));
        }
        services.retain(|_, tools| !tools.is_empty());
    }
    out
}

/// The file before, with the rule: the tools out of the service's other lists, in the one asked.
fn expected(before: &Rules, service: &str, tools: &[String], list: List) -> Shape {
    let mut out = shape(before);
    let key = plain(service);
    let lowered: Vec<String> = tools.iter().map(|t| t.trim().to_ascii_lowercase()).collect();
    for other in List::ALL {
        let services = out.entry(other.name()).or_default();
        let held = services.entry(key.clone()).or_default();
        match other == list {
            true => held.extend(lowered.iter().cloned()),
            false => held.retain(|t| !lowered.contains(t)),
        }
        services.retain(|_, tools| !tools.is_empty());
    }
    out
}

/// What the file says for a service: `pass fetch, search; ask move`.
fn said_of(rules: &Rules, service: &str) -> String {
    let key = plain(service);
    let mut parts = Vec::new();
    for list in List::ALL {
        let mut tools: Vec<&str> = Vec::new();
        let entries = rules.of(list).iter().filter(|e| plain(&e.name) == key);
        for tool in entries.flat_map(|e| e.tools.iter()).map(|t| t.trim()) {
            if !tool.is_empty() && !tools.iter().any(|t| same_tool(t, tool)) {
                tools.push(tool);
            }
        }
        if !tools.is_empty() {
            parts.push(format!("{} {}", list.name(), tools.join(", ")));
        }
    }
    match parts.is_empty() {
        true => "nothing".to_string(),
        false => parts.join("; "),
    }
}

// =============================================================================
// Implementation: the text changed
// =============================================================================

/// One line of the file: its text, and its ending as it was (`\n`, `\r\n`, or none at the end).
type Lines = Vec<(String, String)>;

/// How a service's tools are written.
#[derive(Debug, Clone)]
enum Tools {
    /// `name: [a, b]` on one line: where `[` and `]` stand in it.
    Flow { open: usize, close: usize },
    /// `name:` then one `- tool` a line: each item's line and its tool.
    Items(Vec<(usize, String)>),
    /// `name:` alone, `~` or `null`: no tool.
    Empty,
}

/// A service under a list: its name, its line, its indent, its tools.
#[derive(Debug, Clone)]
struct Entry {
    name: String,
    line: usize,
    indent: String,
    tools: Tools,
}

/// A list: its line, its last line that holds a service or a tool, its services.
#[derive(Debug, Clone)]
struct Block {
    list: List,
    key: usize,
    last: usize,
    /// Written `list: {}`, `~` or `null` on its own line.
    inline: bool,
    entries: Vec<Entry>,
}

fn split(text: &str) -> Lines {
    text.split_inclusive('\n')
        .map(|l| match l.strip_suffix("\r\n").or_else(|| l.strip_suffix('\n')) {
            Some(bare) => (bare.to_string(), l[bare.len()..].to_string()),
            None => (l.to_string(), String::new()),
        })
        .collect()
}

/// Where a comment starts on a line, from `from` on, quotes respected; the line's end if none.
fn comment_at(line: &str, from: usize) -> usize {
    let (mut single, mut double, mut before) = (false, false, true);
    for (at, c) in line.char_indices().filter(|(at, _)| *at >= from) {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '#' if !single && !double && before => return at,
            _ => {}
        }
        before = c == ' ' || c == '\t';
    }
    line.len()
}

/// The colon that ends a key on a line, a quoted key's colon after its closing quote.
fn colon_of(line: &str) -> Option<usize> {
    let start = line.len() - line.trim_start().len();
    let rest = &line[start..];
    let skip = match rest.chars().next() {
        Some(q @ ('"' | '\'')) => rest[1..].find(q).map(|at| at + 2)?,
        _ => 0,
    };
    rest[skip..].find(':').map(|at| start + skip + at)
}

/// A scalar as YAML reads it: quotes taken off.
fn unquote(raw: &str) -> String {
    let raw = raw.trim();
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        return raw[1..raw.len() - 1].replace("\\\"", "\"").replace("\\\\", "\\");
    }
    if raw.len() >= 2 && raw.starts_with('\'') && raw.ends_with('\'') {
        return raw[1..raw.len() - 1].replace("''", "'");
    }
    raw.to_string()
}

/// A name written plain when YAML reads it back as the same text, else in double quotes.
fn scalar(text: &str) -> String {
    let plain = text.chars().next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && text.chars().all(|c| c.is_ascii_alphanumeric() || "_.- ".contains(c))
        && !text.ends_with(' ')
        && !["true", "false", "null", "yes", "no", "on", "off", "y", "n", "~"].contains(&text.to_ascii_lowercase().as_str())
        && text.parse::<f64>().is_err();
    match plain {
        true => text.to_string(),
        false => format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"")),
    }
}

/// The items between `[` and `]`, as written, split at the commas outside quotes.
fn flow_items(inside: &str) -> Vec<String> {
    let (mut out, mut item) = (Vec::new(), String::new());
    let (mut single, mut double) = (false, false);
    for c in inside.chars() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            ',' if !single && !double => {
                out.push(std::mem::take(&mut item));
                continue;
            }
            _ => {}
        }
        item.push(c);
    }
    out.push(item);
    out.into_iter().map(|i| i.trim().to_string()).filter(|i| !i.is_empty()).collect()
}

/// The lists of the file and their services, as written; a shape this does not change refused.
fn scan(lines: &Lines) -> Result<Vec<Block>, String> {
    let mut blocks: Vec<Block> = Vec::new();
    for (i, (line, _)) in lines.iter().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t == "---" || t == "..." {
            continue;
        }
        let colon = colon_of(line);
        let value = |at: usize| line[at + 1..comment_at(line, at + 1)].trim().to_string();
        if !line.starts_with([' ', '\t']) {
            let at = colon.ok_or("one of its lines is not a list's name")?;
            let word = unquote(&line[..at]);
            let list = List::named(&word).ok_or(format!("{word} is not a list"))?;
            let inline = match value(at).as_str() {
                "" => false,
                "{}" | "~" | "null" => true,
                _ => return Err(format!("its list {} is written on one line, and Lorisse changes a list written one service a line", list.name())),
            };
            blocks.push(Block { list, key: i, last: i, inline, entries: Vec::new() });
            continue;
        }
        let Some(block) = blocks.last_mut() else {
            return Err("one of its lines stands under no list".to_string());
        };
        block.last = i;
        if t == "-" || t.starts_with("- ") {
            let dash = line.len() - line.trim_start().len();
            let tool = unquote(&line[dash + 1..comment_at(line, dash + 1)]);
            match block.entries.last_mut().map(|e| &mut e.tools) {
                Some(Tools::Items(items)) => items.push((i, tool)),
                Some(tools @ Tools::Empty) => *tools = Tools::Items(vec![(i, tool)]),
                _ => return Err("one of its tools stands under no service written one tool a line".to_string()),
            }
            continue;
        }
        let at = colon.ok_or("one of its lines is neither a service nor a tool")?;
        let name = unquote(&line[..at]);
        let written = value(at);
        let tools = match written.as_str() {
            "" | "~" | "null" => Tools::Empty,
            v if v.starts_with('[') && v.ends_with(']') => {
                let end = comment_at(line, at + 1);
                let open = at + 1 + line[at + 1..].find('[').unwrap_or(0);
                let close = line[..end].rfind(']').unwrap_or(open);
                Tools::Flow { open, close }
            }
            _ => return Err(format!("the tools of {name} are written neither on one line in brackets nor one a line")),
        };
        let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        block.entries.push(Entry { name, line: i, indent, tools });
    }
    Ok(blocks)
}

/// The lines given put after line `at`; a last line without its ending gets the file's.
fn insert_after(lines: &mut Lines, at: usize, new: Vec<String>, nl: &str) {
    let mut ending = nl.to_string();
    if lines[at].1.is_empty() {
        lines[at].1 = nl.to_string();
        ending = String::new();
    }
    let count = new.len();
    for (k, text) in new.into_iter().enumerate() {
        let end = if k + 1 == count { ending.clone() } else { nl.to_string() };
        lines.insert(at + 1 + k, (text, end));
    }
}

/// A flow line written again with these items, its spacing inside the brackets kept.
fn with_items(line: &str, open: usize, close: usize, items: &[String]) -> String {
    if items.is_empty() {
        return format!("{}{}", &line[..=open], &line[close..]);
    }
    let inside = &line[open + 1..close];
    let lead = &inside[..inside.len() - inside.trim_start().len()];
    let trail = &inside[inside.trim_end().len()..];
    format!("{}{lead}{}{trail}{}", &line[..=open], items.join(", "), &line[close..])
}

/// The service's tools named in `tools` taken out of one entry; whether any was.
fn take_out(lines: &mut Lines, entry: &Entry, tools: &[String]) -> bool {
    match &entry.tools {
        Tools::Flow { open, close } => {
            let line = lines[entry.line].0.clone();
            let items = flow_items(&line[open + 1..*close]);
            let kept: Vec<String> = items.iter().filter(|i| !tools.iter().any(|t| same_tool(t, &unquote(i)))).cloned().collect();
            if kept.len() == items.len() {
                return false;
            }
            lines[entry.line].0 = with_items(&line, *open, *close, &kept);
            true
        }
        Tools::Items(items) => {
            let gone: Vec<usize> = items.iter().filter(|(_, t)| tools.iter().any(|w| same_tool(w, t))).map(|(i, _)| *i).collect();
            for at in gone.iter().rev() {
                lines.remove(*at);
            }
            !gone.is_empty()
        }
        Tools::Empty => false,
    }
}

/// The tools of an entry, as YAML reads them.
fn tools_of(lines: &Lines, entry: &Entry) -> Vec<String> {
    match &entry.tools {
        Tools::Flow { open, close } => flow_items(&lines[entry.line].0[open + 1..*close]).iter().map(|i| unquote(i)).collect(),
        Tools::Items(items) => items.iter().map(|(_, t)| t.clone()).collect(),
        Tools::Empty => Vec::new(),
    }
}

/// The text with the rule: the tools out of the service's other lists, then under the list.
fn changed(text: &str, service: &str, tools: &[String], list: List) -> Result<String, String> {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines = split(text);
    let key = plain(service);
    loop {
        let blocks = scan(&lines)?;
        let held = |e: &&Entry| plain(&e.name) == key && tools_of(&lines, e).iter().any(|t| tools.iter().any(|w| same_tool(w, t)));
        let found = blocks.iter().filter(|b| b.list != list).flat_map(|b| b.entries.iter()).find(held).cloned();
        match found {
            Some(entry) if take_out(&mut lines, &entry, tools) => continue,
            Some(_) => return Err("a tool could not be taken out of another list".to_string()),
            None => break,
        }
    }
    let blocks = scan(&lines)?;
    let indent = blocks.iter().flat_map(|b| b.entries.iter()).map(|e| e.indent.clone()).next().unwrap_or_else(|| "  ".to_string());
    let Some(block) = blocks.iter().find(|b| b.list == list) else {
        if lines.last().is_some_and(|(t, e)| e.is_empty() && !t.is_empty()) {
            lines.last_mut().map(|l| l.1 = nl.to_string());
        }
        let row = format!("{indent}{}: [{}]", scalar(service), names(tools));
        lines.push((format!("{}:", list.name()), nl.to_string()));
        lines.push((row, nl.to_string()));
        return Ok(joined(&lines));
    };
    let entry = block.entries.iter().find(|e| plain(&e.name) == key);
    let Some(entry) = entry else {
        let indent = block.entries.first().map(|e| e.indent.clone()).unwrap_or(indent);
        if block.inline {
            let line = lines[block.key].0.clone();
            lines[block.key].0 = bare_key(&line);
        }
        let row = format!("{indent}{}: [{}]", scalar(service), names(tools));
        insert_after(&mut lines, block.last, vec![row], nl);
        return Ok(joined(&lines));
    };
    let there = tools_of(&lines, entry);
    let missing: Vec<String> = tools.iter().filter(|t| !there.iter().any(|h| same_tool(h, t))).cloned().collect();
    if missing.is_empty() {
        return Ok(joined(&lines));
    }
    let line = lines[entry.line].0.clone();
    match &entry.tools {
        Tools::Flow { open, close } => {
            let mut items = flow_items(&line[open + 1..*close]);
            items.extend(missing.iter().map(|t| scalar(t)));
            lines[entry.line].0 = with_items(&line, *open, *close, &items);
        }
        Tools::Items(items) => {
            let (last, _) = items.last().cloned().unwrap_or((entry.line, String::new()));
            let lead: String = lines[last].0.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
            let new = missing.iter().map(|t| format!("{lead}- {}", scalar(t))).collect();
            insert_after(&mut lines, last, new, nl);
        }
        Tools::Empty => {
            let at = colon_of(&line).unwrap_or(line.len().saturating_sub(1));
            let comment = &line[comment_at(&line, at + 1)..];
            let gap = if comment.is_empty() { "" } else { " " };
            lines[entry.line].0 = format!("{} [{}]{gap}{comment}", &line[..=at], names(&missing));
        }
    }
    Ok(joined(&lines))
}

/// A list's line written `list:`, its comment kept.
fn bare_key(line: &str) -> String {
    let at = colon_of(line).unwrap_or(line.len().saturating_sub(1));
    let comment = &line[comment_at(line, at + 1)..];
    let gap = if comment.is_empty() { "" } else { " " };
    format!("{}{gap}{comment}", &line[..=at])
}

fn names(tools: &[String]) -> String {
    tools.iter().map(|t| scalar(t)).collect::<Vec<_>>().join(", ")
}

fn joined(lines: &Lines) -> String {
    lines.iter().map(|(t, e)| format!("{t}{e}")).collect()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(text: &str, service: &str, tools: &[&str], list: List) -> String {
        let tools: Vec<String> = tools.iter().map(|t| t.to_string()).collect();
        changed(text, service, &tools, list).unwrap()
    }

    #[test]
    fn a_service_is_added_under_a_list_and_a_list_made_when_absent() {
        let lf = FIRST_RULES.replace("\r\n", "\n");
        let first = rule(&lf, "dropbox", &["fetch", "search"], List::Pass);
        assert!(first.ends_with("  dropbox: [fetch, search]\nask: {}\nnever: {}\n"), "{first}");
        let asked = rule(&lf, "dropbox", &["move"], List::Ask);
        assert!(asked.ends_with("ask:\n  dropbox: [move]\nnever: {}\n"), "{asked}");
        let made = rule("pass:\n  lorisse: [where]", "gmail", &["send_message"], List::Never);
        assert_eq!(made, "pass:\n  lorisse: [where]\nnever:\n  gmail: [send_message]\n");
    }

    #[test]
    fn a_tool_moves_from_a_flow_list_and_from_a_block_list() {
        let text = "pass:\n  dropbox: [ fetch, move ]   # mine\nask:\n  dropbox:\n    - copy\n    - delete\n";
        let after = rule(text, "Dropbox", &["move", "delete"], List::Never);
        assert_eq!(after, "pass:\n  dropbox: [ fetch ]   # mine\nask:\n  dropbox:\n    - copy\nnever:\n  Dropbox: [move, delete]\n");
        let added = rule(text, "dropbox", &["paste"], List::Ask);
        assert!(added.contains("    - delete\n    - paste\n"), "{added}");
    }

    #[test]
    fn a_shape_this_does_not_change_is_refused() {
        let one_line = "pass: {dropbox: [fetch]}\n";
        assert!(changed(one_line, "dropbox", &["move".into()], List::Pass).is_err());
        assert_eq!(scalar("send_message"), "send_message");
        assert_eq!(scalar("yes"), "\"yes\"");
        assert_eq!(scalar("google calendar"), "google calendar");
        assert_eq!(scalar("a, b"), "\"a, b\"");
    }
}
