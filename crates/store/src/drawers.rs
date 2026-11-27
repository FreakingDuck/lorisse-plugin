//! Drawers — the plugin's sorting calls: a repository made in the memory folder (a drawer, when
//! Lorisse speaks to the person), and memories moved from one repository to another.
//!
//! A drawer is a folder at the root with its own counter (`ids.json`, a prefix no layer and no
//! other row uses) and one line added under `repos:` in `config.yaml`, every other line of the
//! person's file left as it was; one commit by Lorisse of the two files. A move keeps each
//! memory's number, text, header and links, and carries the files beside it; the number stays
//! counted in the counter it was given from, so it is never given twice. One commit a call.
//! Flow: `Store::drawer` → `drawer` → the settings' text + the counter → one commit;
//! `Store::move_to` → `move_to` → files moved → one commit; `undo` reads a drawer's subject here.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::attach;
use crate::config::{self, CONFIG_FILE};
use crate::error::StoreError;
use crate::filing::{self, BODY};
use crate::git::{self, Who};
use crate::ids::PieceId;
use crate::index::{IDS_FILE, io_error, write_atomic};
use crate::layer::{CONFIDENTIAL, Layer};
use crate::piece::Piece;
use crate::repos::{self, Home, ROOT_FOLDER};
use crate::route::Kind;
use crate::store::Store;

// =============================================================================
// Interface
// =============================================================================

/// What a drawer call did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drawn {
    /// The repository's name, as its row carries it.
    pub name: String,
    /// Its folder at the root of the memory folder.
    pub folder: String,
    /// The letters its memories are numbered with (`G` → `G-0001`).
    pub prefix: String,
    /// Made kept apart: confidential, and open to Claude.
    pub apart: bool,
    /// Lorisse's commit, its full id; `None` when the folder is not under git.
    pub commit: Option<String>,
}

/// What a move did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    /// The memories moved, in the order asked.
    pub ids: Vec<PieceId>,
    /// The memories asked that were in that repository already: not touched.
    pub already: Vec<PieceId>,
    /// The repository they are in now, by its row's name.
    pub repo: String,
    /// Lorisse's one commit of the whole call; `None` when nothing moved or no git holds them.
    pub commit: Option<String>,
}

/// The model provider an apart drawer is opened to.
pub const APART_PROVIDER: &str = "anthropic";

// =============================================================================
// Implementation: the drawer
// =============================================================================

/// The word a drawer's commit subject starts with: `drawer(<folder>): ...`.
const DRAWER_WORD: &str = "drawer";

/// The folder a drawer's commit made, read from its subject; `None` for any other commit.
pub(crate) fn folder_of_subject(subject: &str) -> Option<String> {
    let rest = subject.strip_prefix(DRAWER_WORD)?.strip_prefix('(')?;
    let (folder, _) = rest.split_once("): ")?;
    let plain = !folder.is_empty() && !folder.contains(['/', '\\']) && !folder.starts_with('.');
    plain.then(|| folder.to_string())
}

/// Every file under `dir`, relative to it, forward slashes; none when it is absent.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), String::new())];
    while let Some((d, rel)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push((entry.path(), child)),
                Ok(_) => out.push(child),
                Err(_) => {}
            }
        }
    }
    out
}

/// Refuse the undo of a drawer that holds anything but its counter.
pub(crate) fn refuse_filled(store: &Store, folder: &str) -> Result<(), StoreError> {
    let held = files_under(&store.root.join(folder));
    if held.iter().any(|f| f != IDS_FILE) {
        let name = store.config.repos.iter().find(|r| r.folder == folder).map_or(folder, |r| r.name.as_str());
        return Err(StoreError::Invalid(format!(
            "something was filed in the repository {name} since it was made, and undo takes a drawer back only while it holds nothing: move it out or undo it first"
        )));
    }
    Ok(())
}

/// A drawer's folder removed after its undo, when no file is left in it; a failure is silent.
pub(crate) fn remove_if_empty(dir: &Path) {
    if dir.is_dir() && files_under(dir).is_empty() {
        let _ = fs::remove_dir_all(dir);
    }
}

/// A name's folder: its letters and digits in small letters, any other run made one `-`.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        match c.is_ascii_alphanumeric() {
            true => out.push(c),
            false if !out.is_empty() && !out.ends_with('-') => out.push('-'),
            false => {}
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { DRAWER_WORD.to_string() } else { out }
}

/// A folder for the drawer: its name's, numbered when that one is taken or used by the path rule.
fn free_folder(store: &Store, name: &str) -> String {
    let base = slug(name);
    let config = &store.config;
    let taken = |f: &str| {
        config.layer_of(f).is_some()
            || f == config.inbox
            || f == config.archive
            || f == CONFIDENTIAL
            || config.repos.iter().any(|r| r.folder.eq_ignore_ascii_case(f))
            || store.root.join(f).exists()
    };
    if !taken(&base) {
        return base;
    }
    (2..).map(|n| format!("{base}-{n}")).find(|f| !taken(f)).unwrap_or(base)
}

/// The prefixes already in use: the layers', every row's, and every one a number was ever given in.
fn used_prefixes(store: &Store) -> BTreeSet<String> {
    let mut used: BTreeSet<String> = Layer::ALL.iter().filter_map(|l| l.prefix()).map(String::from).collect();
    used.extend(store.config.repos.iter().filter_map(|r| r.prefix.clone()));
    used.extend(store.index.max.keys().cloned());
    used.extend(store.index.hidden.iter().filter_map(|id| id.prefix().map(String::from)));
    used.extend(store.index.entries.iter().filter_map(|e| e.id.prefix().map(String::from)));
    used
}

/// The prefix of a new drawer: its name's first letter; else that letter and one of the name's
/// next letters; else any free letter; else any free pair of letters.
fn free_prefix(store: &Store, name: &str) -> Result<String, StoreError> {
    let used = used_prefixes(store);
    let letters: Vec<char> = name.chars().filter(char::is_ascii_alphabetic).map(|c| c.to_ascii_uppercase()).collect();
    let mut candidates: Vec<String> = Vec::new();
    if let Some(first) = letters.first() {
        candidates.push(first.to_string());
        candidates.extend(letters[1..].iter().map(|c| format!("{first}{c}")));
    }
    let alphabet: Vec<char> = ('A'..='Z').collect();
    candidates.extend(alphabet.iter().map(char::to_string));
    for a in &alphabet {
        candidates.extend(alphabet.iter().map(|b| format!("{a}{b}")));
    }
    candidates
        .into_iter()
        .find(|c| !used.contains(c))
        .ok_or_else(|| StoreError::Invalid("no free prefix is left for a new repository".into()))
}

/// A YAML scalar for a flow mapping: plain when it is only letters, digits, spaces, `_ . -`
/// and not a word YAML reads as something else; otherwise double-quoted.
fn scalar(text: &str) -> String {
    let plain = text.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && text.chars().all(|c| c.is_ascii_alphanumeric() || " _.-".contains(c))
        && !text.ends_with(' ')
        && !["true", "false", "null", "yes", "no", "on", "off", "y", "n"].contains(&text.to_ascii_lowercase().as_str())
        && text.parse::<f64>().is_err();
    match plain {
        true => text.to_string(),
        false => quoted(text),
    }
}

/// A double-quoted YAML scalar.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The fields of a new row, in the order they are written.
fn row_fields(name: &str, folder: &str, prefix: &str, about: &str, apart: bool) -> Vec<(&'static str, String)> {
    let mut fields = vec![("name", scalar(name)), ("folder", scalar(folder))];
    if apart {
        fields.push(("confidential", "true".into()));
        fields.push(("providers", format!("[{APART_PROVIDER}]")));
    }
    fields.push(("prefix", prefix.to_string()));
    fields.push(("about", quoted(about)));
    fields
}

/// One row in the flow form the sample uses: `  - { name: n, folder: f }`.
fn flow_row(indent: &str, fields: &[(&str, String)], nl: &str) -> String {
    let inside: Vec<String> = fields.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    format!("{indent}- {{ {} }}{nl}", inside.join(", "))
}

/// One row in the block form, each key on its own line under the dash.
fn block_row(indent: &str, fields: &[(&str, String)], nl: &str) -> String {
    let mut out = String::new();
    for (i, (k, v)) in fields.iter().enumerate() {
        match i {
            0 => out.push_str(&format!("{indent}- {k}: {v}{nl}")),
            _ => out.push_str(&format!("{indent}  {k}: {v}{nl}")),
        }
    }
    out
}

/// Whether a line is `repos:` at the left margin, and what follows it on that line, comment cut.
fn repos_key(line: &str) -> Option<String> {
    let rest = line.strip_prefix("repos:")?;
    let value = match rest.find(" #") {
        Some(at) => &rest[..at],
        None if rest.trim_start().starts_with('#') => "",
        None => rest,
    };
    Some(value.trim().to_string())
}

/// The settings' text with the new row added under `repos:`, every other line as it was: after
/// the last row, in the form and the indent of the rows already there; with no `repos:` yet, the
/// root's own row and the new one at the end of the file.
pub(crate) fn with_row(text: &str, root: &str, fields: &[(&str, String)]) -> Result<String, String> {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let bare = |l: &str| l.trim_end_matches(['\n', '\r']).to_string();
    let root_fields = vec![("name", scalar(root)), ("folder", ROOT_FOLDER.to_string())];
    let Some(at) = lines.iter().position(|l| repos_key(&bare(l)).is_some()) else {
        let mut out = text.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(nl);
        }
        out.push_str(&format!("repos:{nl}"));
        out.push_str(&flow_row("  ", &root_fields, nl));
        out.push_str(&flow_row("  ", fields, nl));
        return Ok(out);
    };
    let value = repos_key(&bare(lines[at])).unwrap_or_default();
    if value == "[]" {
        let mut out: String = lines[..at].concat();
        out.push_str(&format!("repos:{nl}"));
        out.push_str(&flow_row("  ", &root_fields, nl));
        out.push_str(&flow_row("  ", fields, nl));
        out.push_str(&lines[at + 1..].concat());
        return Ok(out);
    }
    if !value.is_empty() {
        return Err("its list of repositories is written on one line, and Lorisse adds a repository only to a list written one row a line".into());
    }
    let mut last = at;
    let mut first_row: Option<&str> = None;
    for (j, line) in lines.iter().enumerate().skip(at + 1) {
        let l = bare(line);
        let t = l.trim_start();
        if t.is_empty() || (t.starts_with('#') && l.starts_with([' ', '\t'])) {
            continue;
        }
        if l.starts_with([' ', '\t', '-']) {
            last = j;
            if t.starts_with("- ") && first_row.is_none() {
                first_row = Some(line);
            }
            continue;
        }
        break;
    }
    let row = match first_row {
        None => {
            let mut rows = flow_row("  ", &root_fields, nl);
            rows.push_str(&flow_row("  ", fields, nl));
            rows
        }
        Some(line) => {
            let l = bare(line);
            let indent: String = l.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
            let after = l.trim_start()[2..].trim_start();
            match after.starts_with('{') {
                true => flow_row(&indent, fields, nl),
                false => block_row(&indent, fields, nl),
            }
        }
    };
    let mut out: String = lines[..=last].concat();
    if !out.ends_with('\n') {
        out.push_str(nl);
    }
    out.push_str(&row);
    out.push_str(&lines[last + 1..].concat());
    Ok(out)
}

/// Whether `name` is the name of a repository there, capitals ignored; the name there.
pub(crate) fn taken_name(store: &Store, name: &str) -> Option<String> {
    let wanted = name.trim().to_lowercase();
    let root = filing::root_name(store);
    if root.trim().to_lowercase() == wanted {
        return Some(root);
    }
    let rows = store.config.repos.iter().map(|r| r.name.trim().to_string());
    let listed = store.repos().into_iter().map(|r| r.name).filter(|n| n != CONFIDENTIAL);
    rows.chain(listed).find(|n| n.to_lowercase() == wanted)
}

pub(crate) fn drawer(store: &mut Store, name: &str, about: &str, apart: bool) -> Result<Drawn, StoreError> {
    let name = filing::one_line(name);
    let about = filing::one_line(about);
    if name.is_empty() || about.is_empty() {
        return Err(StoreError::Invalid("a repository needs a name and one sentence on what it is for".into()));
    }
    if name.eq_ignore_ascii_case(CONFIDENTIAL) {
        return Err(StoreError::Invalid(format!(
            "{CONFIDENTIAL} is the name of the folder's confidential part, never a repository's: choose another name"
        )));
    }
    if let Some(there) = taken_name(store, &name) {
        return Err(StoreError::Invalid(format!("the name {name} is taken: the repository {there} is there")));
    }
    let folder = free_folder(store, &name);
    let prefix = free_prefix(store, &name)?;
    let config_path = store.root.join(CONFIG_FILE);
    let before = match fs::read_to_string(&config_path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(io_error(&config_path, e)),
    };
    let fields = row_fields(&name, &folder, &prefix, &about, apart);
    let root = filing::root_name(store);
    let after = with_row(&before, &root, &fields)
        .map_err(|why| StoreError::Invalid(format!("{CONFIG_FILE} is left as it was: {why}")))?;
    let read = config::parse_text(&after)
        .map_err(|e| StoreError::Invalid(format!("{CONFIG_FILE} is left as it was, the new line would not read: {e}")))?;
    let row = read.repos.iter().find(|r| r.name.trim() == name && r.folder == folder);
    if row.is_none_or(|r| r.prefix.as_deref() != Some(prefix.as_str()) || r.about.as_deref() != Some(about.as_str())) {
        return Err(StoreError::Invalid(format!("{CONFIG_FILE} is left as it was: the new line would not read as written")));
    }
    let counter = format!("{folder}/{IDS_FILE}");
    let ids = format!("{{\n  \"{prefix}\": 0\n}}\n");
    let home = filing::ready(store, CONFIG_FILE)?;
    filing::refuse_dirty(home.as_ref(), CONFIG_FILE)?;
    let subject = format!("{DRAWER_WORD}({folder}): the repository {name} made, numbered {prefix}");
    let changes = [(CONFIG_FILE.to_string(), Some(after)), (counter, Some(ids))];
    let made = filing::write_and_commit(store, home.as_ref(), &changes, &subject);
    let commit = match made {
        Ok(commit) => commit,
        Err(e) => {
            remove_if_empty(&store.root.join(&folder));
            return Err(e);
        }
    };
    Ok(Drawn { name, folder, prefix, apart, commit })
}

// =============================================================================
// Implementation: the move
// =============================================================================

/// The repository a move goes to, by name, capitals ignored: its folder (`None` the root) and
/// its row's name.
fn target(store: &Store, name: &str) -> Result<(Option<String>, String), StoreError> {
    let name = name.trim();
    if name.eq_ignore_ascii_case(CONFIDENTIAL) {
        return Err(StoreError::Confidential(name.to_string()));
    }
    let root = filing::root_name(store);
    let row = store.config.repos.iter().find(|r| r.name.trim().eq_ignore_ascii_case(name));
    let Some(row) = row else {
        if root.eq_ignore_ascii_case(name) {
            return Ok((None, root));
        }
        let seen = store.repos().into_iter().filter(|r| (!r.confidential || r.open) && r.present);
        let names: Vec<String> = seen.map(|r| r.name).collect();
        return Err(StoreError::Invalid(format!(
            "no repository is named {name}: the repositories are {}",
            names.join(", ")
        )));
    };
    if row.confidential {
        return Err(StoreError::Confidential(name.to_string()));
    }
    if row.folder == ROOT_FOLDER {
        return Ok((None, row.name.clone()));
    }
    if !store.root.join(&row.folder).is_dir() {
        return Err(StoreError::Invalid(format!("the repository {} is not on this machine", row.name)));
    }
    Ok((Some(row.folder.clone()), row.name.clone()))
}

/// One memory on its way: its number, its file now and after, its new text, its files' moves.
struct Going {
    id: PieceId,
    from: String,
    to: String,
    text: String,
    /// Each file beside it: where it is, where it goes; both from the root.
    files: Vec<(String, String)>,
}

/// The counter file of a numbering, from the root: the root's, or the numbered row's.
fn counter_of(store: &Store, numbering: Option<&str>) -> String {
    let rows = repos::numbered_rows(&store.config);
    match numbering.and_then(|p| rows.iter().find(|(_, own)| *own == p)) {
        Some((folder, _)) => format!("{folder}/{IDS_FILE}"),
        None => IDS_FILE.to_string(),
    }
}

/// A counter's text raised to every number given; `None` when it already keeps them all.
fn raised(path: &Path, numbers: &BTreeMap<String, u64>) -> Option<String> {
    let mut kept: BTreeMap<String, u64> = fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let mut changed = false;
    for (prefix, n) in numbers {
        let high = kept.entry(prefix.clone()).or_default();
        if *high < *n {
            *high = *n;
            changed = true;
        }
    }
    let text = serde_json::to_string_pretty(&kept).ok()?;
    changed.then(|| text + "\n")
}

pub(crate) fn move_to(store: &mut Store, ids: &[PieceId], repo: &str) -> Result<Moved, StoreError> {
    if ids.is_empty() {
        return Err(StoreError::Invalid("a move names at least one memory".into()));
    }
    let (folder, name) = target(store, repo)?;
    let states = store.repos();
    let root_name = filing::root_name(store);
    let mut going: Vec<Going> = Vec::new();
    let mut already: Vec<PieceId> = Vec::new();
    let mut seen: Vec<PieceId> = Vec::new();
    for id in ids {
        let entry = filing::visible(store, &id.to_string())?.clone();
        if seen.contains(&entry.id) {
            continue;
        }
        seen.push(entry.id.clone());
        let from = entry.path_text();
        if entry.layer != Layer::Memory {
            return Err(StoreError::Invalid(format!("{} is a {}: only a memory is moved", entry.id, entry.layer)));
        }
        if store.in_inbox(&crate::links::Ref::from(entry.id.clone())) || entry.archived {
            return Err(StoreError::Invalid(format!(
                "{} is not a filed memory (a capture in the inbox, or archived): only a filed memory is moved",
                entry.id
            )));
        }
        let from_repo = entry.repo.clone().unwrap_or_else(|| root_name.clone());
        if from_repo.eq_ignore_ascii_case(&name) {
            already.push(entry.id.clone());
            continue;
        }
        let apart = states.iter().any(|s| s.name == from_repo && s.confidential);
        if apart {
            return Err(StoreError::Invalid(format!(
                "{} is in the confidential repository {from_repo}, and Lorisse never takes a piece out of a confidential repository",
                entry.id
            )));
        }
        let source = store.config.repos.iter().find(|r| r.folder != ROOT_FOLDER && Some(&r.name) == entry.repo.as_ref());
        let inside = match source {
            Some(row) => from.strip_prefix(&format!("{}/", row.folder)).unwrap_or(&from).to_string(),
            None => from.clone(),
        };
        let to = match &folder {
            Some(f) => format!("{f}/{inside}"),
            None => inside,
        };
        if store.root.join(&to).exists() {
            return Err(StoreError::Exists(PathBuf::from(&to)));
        }
        let path = store.root.join(&from);
        let text = fs::read_to_string(&path).map_err(|e| io_error(&path, e))?;
        let mut piece = Piece::parse(Path::new(&from), &text)?;
        let moves = attach::relocate(&mut piece, Path::new(&from), Path::new(&to))?;
        let slashed = |p: &Path| p.to_string_lossy().replace('\\', "/");
        let files: Vec<(String, String)> = moves
            .iter()
            .filter_map(|c| Some((slashed(c.from.as_deref()?), slashed(&c.path))))
            .collect();
        for (old, new) in &files {
            if !store.root.join(old).is_file() {
                return Err(StoreError::Invalid(format!("{old}, a file of {}, is not there", entry.id)));
            }
            if store.root.join(new).exists() {
                return Err(StoreError::Exists(PathBuf::from(new)));
            }
        }
        let text = if files.is_empty() { text } else { piece.render() };
        going.push(Going { id: entry.id.clone(), from, to, text, files });
    }
    if going.is_empty() {
        return Ok(Moved { ids: Vec::new(), already, repo: name, commit: None });
    }
    // A number stays counted where it was given: the counter of its numbering keeps it when the
    // file, in its new place, would no longer count it.
    let mut counters: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for g in &going {
        let PieceId::Numbered { prefix, n } = &g.id else { continue };
        let before = repos::numbering_of(&store.config, &g.from, Some(prefix));
        let after = repos::numbering_of(&store.config, &g.to, Some(prefix));
        if before == after {
            continue;
        }
        let numbers = counters.entry(counter_of(store, before.as_deref())).or_default();
        let high = numbers.entry(prefix.clone()).or_default();
        *high = (*high).max(*n);
    }
    let mut texts: Vec<(String, String)> = Vec::new();
    for (file, numbers) in &counters {
        if let Some(text) = raised(&store.root.join(file), numbers) {
            texts.push((file.clone(), text));
        }
    }
    // Every path the call touches, from the root, and the one history that holds them all.
    let mut paths: Vec<String> = Vec::new();
    for g in &going {
        paths.push(g.from.clone());
        paths.push(g.to.clone());
        for (old, new) in &g.files {
            paths.push(old.clone());
            paths.push(new.clone());
        }
    }
    paths.extend(texts.iter().map(|(f, _)| f.clone()));
    let home = one_home(store, &paths)?;
    for g in &going {
        filing::refuse_dirty(home.as_ref(), &g.from)?;
        for (old, _) in &g.files {
            filing::refuse_dirty(home.as_ref(), old)?;
        }
    }
    for (file, _) in &texts {
        filing::refuse_dirty(home.as_ref(), file)?;
    }
    let named: Vec<String> = going.iter().map(|g| g.id.padded(store.config.id_width)).collect();
    let subject = format!("{}({}): to {name}", Kind::Move, named.join(", "));
    let commit = move_and_commit(store, home.as_ref(), &going, &texts, &paths, &subject)?;
    Ok(Moved { ids: going.into_iter().map(|g| g.id).collect(), already, repo: name, commit })
}

/// The one repository whose history holds every path, once nothing blocks a write; `None` when
/// no git holds them. Two histories are refused: a move is one commit.
fn one_home(store: &Store, paths: &[String]) -> Result<Option<Home>, StoreError> {
    let mut found: Option<Option<Home>> = None;
    for path in paths {
        let home = filing::ready(store, path)?;
        match &found {
            None => found = Some(home),
            Some(first) => {
                let same = match (first, &home) {
                    (Some(a), Some(b)) => a.dir == b.dir,
                    (None, None) => true,
                    _ => false,
                };
                if !same {
                    return Err(StoreError::Invalid(
                        "the two repositories keep separate histories, and a move is one commit in one history: move it by hand".into(),
                    ));
                }
            }
        }
    }
    Ok(found.flatten())
}

/// The folders made for the new places, the deepest first, so an empty one can be removed.
fn new_dirs(root: &Path, paths: &[String]) -> Vec<PathBuf> {
    let mut made: Vec<PathBuf> = Vec::new();
    for path in paths {
        let mut dir = root.join(path).parent().map(Path::to_path_buf);
        while let Some(d) = dir {
            if d == root || d.exists() || made.contains(&d) {
                break;
            }
            made.push(d.clone());
            dir = d.parent().map(Path::to_path_buf);
        }
    }
    made.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    made
}

/// The files written at their new places and removed from the old, the counters raised, then one
/// commit by Lorisse of every path when git holds them; on any failure every file is put back
/// and unstaged, and the folders made for the move removed. The commit's full id, `None`
/// without git.
fn move_and_commit(
    store: &Store,
    home: Option<&Home>,
    going: &[Going],
    texts: &[(String, String)],
    paths: &[String],
    subject: &str,
) -> Result<Option<String>, StoreError> {
    let root = &store.root;
    let saved = filing::save(paths.iter().map(|p| root.join(p)));
    let dirs = new_dirs(root, paths);
    let written = (|| -> Result<(), StoreError> {
        for g in going {
            for (old, new) in &g.files {
                let (from, to) = (root.join(old), root.join(new));
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
                }
                fs::copy(&from, &to).map_err(|e| io_error(&to, e))?;
                fs::remove_file(&from).map_err(|e| io_error(&from, e))?;
            }
            write_atomic(&root.join(&g.to), &g.text)?;
            let from = root.join(&g.from);
            fs::remove_file(&from).map_err(|e| io_error(&from, e))?;
        }
        for (file, text) in texts {
            write_atomic(&root.join(file), text)?;
        }
        Ok(())
    })();
    let back = |e: StoreError, inner: &[String]| {
        if let Some(home) = home {
            filing::unstage(home, inner);
        }
        filing::restore(&saved);
        for dir in &dirs {
            let _ = fs::remove_dir(dir);
        }
        e
    };
    let inner: Vec<String> = match home {
        Some(home) => paths.iter().map(|p| home.inner(p)).collect(),
        None => Vec::new(),
    };
    if let Err(e) = written {
        return Err(back(StoreError::Io(format!("the move could not be written, the folder is as it was: {e}")), &inner));
    }
    let commit = match home {
        Some(home) => match git::commit(&home.dir, &inner, &[subject, BODY], Who::Lorisse) {
            Ok(Some(id)) => Some(id),
            Ok(None) => return Err(back(filing::failed(StoreError::Git("nothing to commit".into())), &inner)),
            Err(e) => return Err(back(filing::failed(e), &inner)),
        },
        None => None,
    };
    for g in going {
        for (old, _) in &g.files {
            if let Some(dir) = root.join(old).parent() {
                let _ = fs::remove_dir(dir);
            }
        }
    }
    Ok(commit)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Vec<(&'static str, String)> {
        row_fields("Garden work", "garden-work", "G", "The garden design clients, their plans and budgets.", false)
    }

    #[test]
    fn a_row_is_added_after_the_last_one_in_their_form_and_every_other_line_is_kept() {
        let text = "# mine\nkinds: [fact]\nrepos:\n  - { name: home, folder: . }   # the root\n\n  # a note\n  - { name: club, folder: club, prefix: C }\nsources: [chat]  # trailing\n";
        let after = with_row(text, "home", &fields()).unwrap();
        let want = "# mine\nkinds: [fact]\nrepos:\n  - { name: home, folder: . }   # the root\n\n  # a note\n  - { name: club, folder: club, prefix: C }\n  - { name: Garden work, folder: garden-work, prefix: G, about: \"The garden design clients, their plans and budgets.\" }\nsources: [chat]  # trailing\n";
        assert_eq!(after, want);
        let block = "repos:\n- name: home\n  folder: .\n";
        let after = with_row(block, "home", &fields()).unwrap();
        assert_eq!(after, "repos:\n- name: home\n  folder: .\n- name: Garden work\n  folder: garden-work\n  prefix: G\n  about: \"The garden design clients, their plans and budgets.\"\n");
        assert!(config::parse_text(&after).is_ok());
    }

    #[test]
    fn without_repos_the_root_s_row_and_the_new_one_go_at_the_end() {
        let after = with_row("kinds: [fact]", "Lorisse", &fields()).unwrap();
        assert!(after.starts_with("kinds: [fact]\nrepos:\n  - { name: Lorisse, folder: . }\n  - { name: Garden work"), "{after}");
        let empty = with_row("kinds: [fact]\r\nrepos: []  # none yet\r\n", "home", &fields()).unwrap();
        assert!(empty.starts_with("kinds: [fact]\r\nrepos:\r\n  - { name: home, folder: . }\r\n"), "{empty}");
        assert!(with_row("repos: [{name: a, folder: a}]\n", "home", &fields()).is_err());
    }

    #[test]
    fn a_name_is_quoted_when_yaml_would_read_it_otherwise() {
        assert_eq!(scalar("garden"), "garden");
        assert_eq!(scalar("yes"), "\"yes\"");
        assert_eq!(scalar("a, b"), "\"a, b\"");
        assert_eq!(scalar("12"), "\"12\"");
        assert_eq!(quoted("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(slug("Garden work!"), "garden-work");
        assert_eq!(slug("!!"), "drawer");
    }

    #[test]
    fn a_drawer_s_subject_names_its_folder() {
        assert_eq!(folder_of_subject("drawer(garden): the repository Garden made, numbered G").as_deref(), Some("garden"));
        assert_eq!(folder_of_subject("remember(M-0001): x"), None);
        assert_eq!(folder_of_subject("drawer(../x): y"), None);
    }
}
