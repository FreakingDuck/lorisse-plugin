//! The rules: `rules.yaml` at the root of the memory folder, else the plugin's first set.
//! Three lists, pass, ask and never, each a map from a service to its tools.

use std::io::ErrorKind;
use std::path::Path;

use serde_yaml_ng::Value;

/// The rules file, at the root of the memory folder.
pub const RULES_FILE: &str = "rules.yaml";

/// The plugin's first set, carried in the program.
pub const FIRST_RULES: &str = include_str!("../first_rules.yaml");

/// One of the three lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum List {
    Pass,
    Ask,
    Never,
}

impl List {
    /// The three lists, in the order the file writes them.
    pub const ALL: [List; 3] = [List::Pass, List::Ask, List::Never];

    /// The list a word names, capitals and spaces around ignored; `None` for any other word.
    pub fn named(word: &str) -> Option<List> {
        let word = word.trim();
        List::ALL.into_iter().find(|l| l.name().eq_ignore_ascii_case(word))
    }

    /// The list's name, as the file writes it.
    pub fn name(self) -> &'static str {
        match self {
            List::Pass => "pass",
            List::Ask => "ask",
            List::Never => "never",
        }
    }
}

/// Where the rules of a call came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The folder's own `rules.yaml`.
    Yours,
    /// The plugin's first set.
    First,
}

/// One service and its tools, as written.
#[derive(Debug, Clone, Default)]
pub(crate) struct Service {
    pub(crate) name: String,
    pub(crate) tools: Vec<String>,
}

/// The three lists, read.
#[derive(Debug, Clone, Default)]
pub struct Rules {
    pub(crate) pass: Vec<Service>,
    pub(crate) ask: Vec<Service>,
    pub(crate) never: Vec<Service>,
}

/// The rule that names a call: its list, its service and its tool as written.
#[derive(Debug, Clone)]
pub struct Ruled {
    pub list: List,
    pub service: String,
    pub tool: String,
}

/// The outcome of reading the rules.
#[derive(Debug, Clone)]
pub enum Loaded {
    Rules(Rules, Source),
    /// The file is there and cannot be read, with why.
    Unreadable(String),
}

/// The rules of this call: read again at each call, never written.
pub fn load(folder: Option<&Path>) -> Loaded {
    let Some(folder) = folder else {
        return first();
    };
    match std::fs::read_to_string(folder.join(RULES_FILE)) {
        Ok(text) => match parse(&text) {
            Ok(rules) => Loaded::Rules(rules, Source::Yours),
            Err(why) => Loaded::Unreadable(why),
        },
        Err(e) if e.kind() == ErrorKind::NotFound => first(),
        Err(e) => Loaded::Unreadable(e.to_string()),
    }
}

/// The first set; should it not parse, no rule at all, so nothing passes.
fn first() -> Loaded {
    Loaded::Rules(parse(FIRST_RULES).unwrap_or_default(), Source::First)
}

/// The text of a rules file, read; an unknown list or a value of another shape is an error.
pub fn parse(text: &str) -> Result<Rules, String> {
    let value: Value = serde_yaml_ng::from_str(text).map_err(|e| e.to_string())?;
    let mut rules = Rules::default();
    let map = match value {
        Value::Null => return Ok(rules),
        Value::Mapping(map) => map,
        _ => return Err("the file is not the three lists pass, ask, never".to_string()),
    };
    for (key, services) in map {
        let services = read_services(&key, services)?;
        match key.as_str() {
            Some("pass") => rules.pass.extend(services),
            Some("ask") => rules.ask.extend(services),
            Some("never") => rules.never.extend(services),
            _ => return Err(format!("an unknown list: {}", shown(&key))),
        }
    }
    Ok(rules)
}

/// One list's services: a map from a service's name to a list of tools.
fn read_services(list: &Value, value: Value) -> Result<Vec<Service>, String> {
    let map = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Mapping(map) => map,
        _ => return Err(format!("the list {} is not a map of services", shown(list))),
    };
    let mut out = Vec::new();
    for (name, tools) in map {
        let Some(name) = name.as_str().map(str::to_string) else {
            return Err(format!("a service of {} is not a name", shown(list)));
        };
        let tools = match tools {
            Value::Null => Vec::new(),
            Value::Sequence(items) => {
                let mut tools = Vec::new();
                for item in items {
                    match item.as_str() {
                        Some(tool) => tools.push(tool.to_string()),
                        None => return Err(format!("a tool of {name} is not a name")),
                    }
                }
                tools
            }
            _ => return Err(format!("the tools of {name} are not a list")),
        };
        out.push(Service { name, tools });
    }
    Ok(out)
}

/// A key as it is said in a reason.
fn shown(value: &Value) -> String {
    match value.as_str() {
        Some(text) => text.to_string(),
        None => serde_yaml_ng::to_string(value)
            .map(|t| t.trim().to_string())
            .unwrap_or_default(),
    }
}

impl Rules {
    /// The services of one list.
    pub(crate) fn of(&self, list: List) -> &Vec<Service> {
        match list {
            List::Pass => &self.pass,
            List::Ask => &self.ask,
            List::Never => &self.never,
        }
    }

    /// The strictest rule naming this tool of this server: never, then ask, then pass.
    pub fn decide(&self, server: &str, tool: &str) -> Option<Ruled> {
        let lists = [
            (List::Never, &self.never),
            (List::Ask, &self.ask),
            (List::Pass, &self.pass),
        ];
        for (list, services) in lists {
            for service in services.iter() {
                if !service_in(server, &service.name) {
                    continue;
                }
                if let Some(named) = service.tools.iter().find(|t| t.trim().eq_ignore_ascii_case(tool)) {
                    return Some(Ruled {
                        list,
                        service: service.name.clone(),
                        tool: named.trim().to_string(),
                    });
                }
            }
        }
        None
    }
}

/// The rule of a call and whose it is: the person's file first; a tool it names nowhere is
/// ruled by the first set, which names Lorisse's own tools only, so any other service's stays
/// unruled.
pub fn ruling(rules: &Rules, source: Source, server: &str, tool: &str) -> Option<(Ruled, Source)> {
    if let Some(ruled) = rules.decide(server, tool) {
        return Some((ruled, source));
    }
    if source == Source::First {
        return None;
    }
    let Loaded::Rules(first, _) = first() else {
        return None;
    };
    first.decide(server, tool).map(|ruled| (ruled, Source::First))
}

/// Spaces and dashes are one with underscores, capitals ignored.
pub(crate) fn plain(text: &str) -> String {
    text.trim()
        .chars()
        .map(|c| match c {
            ' ' | '-' => '_',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

/// A service is named inside the server's part of the tool's name, as whole words.
pub fn service_in(server: &str, service: &str) -> bool {
    let server = plain(server);
    let service = plain(service);
    if service.is_empty() {
        return false;
    }
    let bytes = server.as_bytes();
    let mut from = 0;
    while let Some(at) = server.get(from..).and_then(|rest| rest.find(&service)) {
        let start = from + at;
        let end = start + service.len();
        let before = start == 0 || bytes.get(start - 1) == Some(&b'_');
        let after = end == bytes.len() || bytes.get(end) == Some(&b'_');
        if before && after {
            return true;
        }
        from = start + 1;
        while from < server.len() && !server.is_char_boundary(from) {
            from += 1;
        }
    }
    false
}
