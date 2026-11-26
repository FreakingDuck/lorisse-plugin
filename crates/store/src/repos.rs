//! Repos — the data folder's repositories: the root, then each row of `config.yaml`'s `repos:`.
//!
//! A row names a folder at the root holding a repository of its own (a clone or a junction),
//! its role, whether its pieces are confidential, the machines it lives on and the identity its
//! own commits carry. Flow: `config.yaml` → rows → the path rule's first segment, the
//! repository a piece's git calls run in (`Home`), `Store::repos`, the root's `.gitignore`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::StoreConfig;
use crate::git::{self, Who};
use crate::layer::{CONFIDENTIAL, Layer};
use crate::store::Store;

// =============================================================================
// Interface
// =============================================================================

/// The `folder` of the row that is the data folder itself.
pub const ROOT_FOLDER: &str = ".";

/// What a repository holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The owner's own life.
    #[default]
    Personal,
    /// An employer's or a client's.
    Work,
}

/// One row of `config.yaml`'s `repos:`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoRow {
    /// What the repository is (`perso`, `acme`), never the rule.
    pub name: String,
    /// `.` for the root, else one folder name at the root.
    pub folder: String,
    /// `personal` or `work`.
    #[serde(default)]
    pub role: Role,
    /// Its pieces are behind the gate and written by proposal only.
    #[serde(default)]
    pub confidential: bool,
    /// The machines it lives on (`COMPUTERNAME`); empty = every machine.
    #[serde(default)]
    pub machines: Vec<String>,
    /// The alias of the git identity that commits there; a name, never a key.
    #[serde(default)]
    pub key: Option<String>,
    /// `user.name` of the folder's own commits there; the folder's when absent.
    #[serde(default)]
    pub git_name: Option<String>,
    /// `user.email` of the folder's own commits there; the folder's when absent.
    #[serde(default)]
    pub git_email: Option<String>,
    /// The model providers allowed to see its pieces (`anthropic`, `gemini` …); empty = the gate's rule.
    #[serde(default)]
    pub providers: Vec<String>,
    /// The letters its new pieces' ids carry (`A` → `A-0001`), counted in its own `ids.json`; none = the root's numbering.
    #[serde(default)]
    pub prefix: Option<String>,
}

/// A row as this machine sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepoState {
    /// The row's name.
    pub name: String,
    /// Its folder, `.` for the root.
    pub folder: String,
    /// `personal` or `work`.
    pub role: Role,
    /// Its pieces are confidential.
    pub confidential: bool,
    /// Confidential, and open to the model provider the store was opened for: seen and written.
    pub open: bool,
    /// The identity's alias, when named.
    pub key: Option<String>,
    /// The prefix its memories and tasks are numbered with; `None` for the root's numbering.
    pub prefix: Option<String>,
    /// Listed for this machine: no `machines`, or this machine among them.
    pub here: bool,
    /// Its folder is at the root.
    pub present: bool,
    /// Its folder holds a git repository of its own.
    pub git: bool,
}

// =============================================================================
// Implementation
// =============================================================================

/// The row of a first path segment: a listed folder, else `confidential` as the alias of a
/// confidential row.
pub(crate) fn row_at(config: &StoreConfig, first: &str) -> Option<RepoRow> {
    let listed = config.repos.iter().filter(|r| r.folder != ROOT_FOLDER);
    if let Some(row) = listed.clone().find(|r| r.folder == first) {
        return Some(row.clone());
    }
    (first == CONFIDENTIAL).then(alias)
}

/// Whether a relative path lies in a confidential repository.
pub(crate) fn is_confidential(rel: &str, config: &StoreConfig) -> bool {
    let first = rel.split('/').next().unwrap_or("");
    row_at(config, first).is_some_and(|r| r.confidential)
}

/// The folder a new confidential piece goes to: the first confidential row's, else the alias.
pub(crate) fn confidential_home(config: &StoreConfig) -> &str {
    let listed = config.repos.iter().filter(|r| r.folder != ROOT_FOLDER);
    let mut rows = listed.filter(|r| r.confidential);
    rows.next().map_or(CONFIDENTIAL, |r| r.folder.as_str())
}

/// The row `confidential/` stands for when no row lists it.
fn alias() -> RepoRow {
    RepoRow {
        name: CONFIDENTIAL.into(),
        folder: CONFIDENTIAL.into(),
        role: Role::Work,
        confidential: true,
        machines: Vec::new(),
        key: None,
        git_name: None,
        git_email: None,
        providers: Vec::new(),
        prefix: None,
    }
}

/// Every confidential row naming `provider` among its `providers` (any case), the
/// `confidential/` folder never, read as open for this store; the names of the rows opened.
pub(crate) fn open_rows(config: &mut StoreConfig, provider: &str) -> Vec<String> {
    let wanted = provider.trim();
    let mut opened = Vec::new();
    for row in config.repos.iter_mut() {
        let named = row.providers.iter().any(|p| p.trim().eq_ignore_ascii_case(wanted));
        let listed = row.folder != ROOT_FOLDER && row.folder != CONFIDENTIAL;
        if row.confidential && named && listed && !wanted.is_empty() {
            row.confidential = false;
            opened.push(row.name.clone());
        }
    }
    opened
}

/// The rows numbering their own pieces: each one's folder and prefix.
pub(crate) fn numbered_rows(config: &StoreConfig) -> Vec<(&str, &str)> {
    let listed = config.repos.iter().filter(|r| r.folder != ROOT_FOLDER);
    let numbered = listed.filter_map(|r| Some((r.folder.as_str(), r.prefix.as_deref()?)));
    numbered.collect()
}

/// The prefix of the row at `folder`, when it numbers its own pieces.
pub(crate) fn prefix_at<'a>(config: &'a StoreConfig, folder: &str) -> Option<&'a str> {
    let rows = numbered_rows(config);
    rows.into_iter().find(|(f, _)| *f == folder).map(|(_, p)| p)
}

/// The prefix of the row named `name`, when it numbers its own pieces.
pub(crate) fn prefix_named<'a>(config: &'a StoreConfig, name: &str) -> Option<&'a str> {
    let row = config.repos.iter().find(|r| r.name.trim() == name.trim())?;
    prefix_at(config, &row.folder)
}

/// The numbering an id of `prefix` at `rel` belongs to: a row's prefix, or `None` for the root's.
///
/// A row's own prefix belongs to it wherever it sits (a capture in the root's inbox); any other
/// id belongs to the row whose folder holds it, else to the root.
pub(crate) fn numbering_of(
    config: &StoreConfig,
    rel: &str,
    prefix: Option<&str>,
) -> Option<String> {
    let rows = numbered_rows(config);
    if let Some(own) = rows.iter().find(|(_, p)| Some(*p) == prefix) {
        return Some(own.1.to_string());
    }
    let first = rel.split('/').next().unwrap_or("");
    prefix_at(config, first).map(str::to_string)
}

/// Whether `row` is the alias, not a listed row.
fn is_alias(row: &RepoRow, config: &StoreConfig) -> bool {
    row.folder == CONFIDENTIAL && !config.repos.iter().any(|r| r.folder == CONFIDENTIAL)
}

/// The root's row: the listed one, else the data folder named by its folder.
fn root_row(config: &StoreConfig, root: &Path) -> RepoRow {
    if let Some(row) = config.repos.iter().find(|r| r.folder == ROOT_FOLDER) {
        return row.clone();
    }
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned());
    RepoRow {
        name: name.unwrap_or_else(|| "root".into()),
        folder: ROOT_FOLDER.into(),
        ..alias()
    }
    .personal()
}

impl RepoRow {
    fn personal(self) -> RepoRow {
        RepoRow {
            role: Role::Personal,
            confidential: false,
            ..self
        }
    }
}

/// This machine's name, as `machines:` spells it.
fn machine() -> String {
    let name = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME"));
    name.unwrap_or_default()
}

/// The root, then every listed row, then the alias when its folder is there.
pub(crate) fn states(store: &Store) -> Vec<RepoState> {
    let config = &store.config;
    let mut rows = vec![root_row(config, &store.root)];
    rows.extend(
        config
            .repos
            .iter()
            .filter(|r| r.folder != ROOT_FOLDER)
            .cloned(),
    );
    let listed = |f: &str| config.repos.iter().any(|r| r.folder == f);
    if !listed(CONFIDENTIAL) && store.root.join(CONFIDENTIAL).is_dir() {
        rows.push(alias());
    }
    let this = machine().to_lowercase();
    rows.into_iter()
        .map(|row| {
            let dir = store.root.join(&row.folder);
            let open = store.opened.contains(&row.name);
            RepoState {
                open,
                here: row.machines.is_empty()
                    || row.machines.iter().any(|m| m.to_lowercase() == this),
                present: dir.is_dir(),
                git: git::is_repo(&dir),
                name: row.name,
                folder: row.folder,
                role: row.role,
                confidential: row.confidential || open,
                key: row.key,
                prefix: row.prefix,
            }
        })
        .collect()
}

/// The rows' rules: names and folders once each, one root at most and never confidential, a
/// folder one plain name the path rule does not use, a prefix of its own.
pub(crate) fn check_rows(config: &StoreConfig) -> Result<(), String> {
    let rows = &config.repos;
    for (i, row) in rows.iter().enumerate() {
        let name = row.name.trim();
        if name.is_empty() {
            return Err("repos: a row without a name".into());
        }
        let earlier = &rows[..i];
        if earlier.iter().any(|r| r.name.trim() == name) {
            return Err(format!("repos: the name {name} twice"));
        }
        if earlier.iter().any(|r| r.folder == row.folder) {
            return Err(format!("repos: the folder {} twice", row.folder));
        }
        check_folder(row, config)?;
        check_providers(row)?;
        check_prefix(row, earlier)?;
    }
    Ok(())
}

/// A prefix is one or more upper-case letters, never a layer's, never the root's, never two rows'.
fn check_prefix(row: &RepoRow, earlier: &[RepoRow]) -> Result<(), String> {
    let Some(prefix) = row.prefix.as_deref() else {
        return Ok(());
    };
    let name = &row.name;
    let letters = !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_uppercase());
    let layer = Layer::ALL.iter().any(|l| l.prefix() == Some(prefix));
    let twice = earlier.iter().any(|r| r.prefix.as_deref() == Some(prefix));
    let reason = if row.folder == ROOT_FOLDER {
        "the root takes no prefix".to_string()
    } else if !letters {
        format!("the prefix {prefix:?} must be upper-case letters")
    } else if layer {
        format!("the prefix {prefix} is a layer's")
    } else if twice {
        format!("the prefix {prefix} twice")
    } else {
        return Ok(());
    };
    Err(format!("repos: {name}: {reason}"))
}

/// Each provider named once, none blank.
fn check_providers(row: &RepoRow) -> Result<(), String> {
    for (i, provider) in row.providers.iter().enumerate() {
        let name = provider.trim();
        if name.is_empty() {
            return Err(format!("repos: {}: a blank provider", row.name));
        }
        if row.providers[..i].iter().any(|p| p.trim() == name) {
            return Err(format!("repos: {}: the provider {name} twice", row.name));
        }
    }
    Ok(())
}

fn check_folder(row: &RepoRow, config: &StoreConfig) -> Result<(), String> {
    let folder = row.folder.as_str();
    if folder == ROOT_FOLDER {
        return match row.confidential {
            true => Err(format!(
                "repos: {}: the root is never confidential",
                row.name
            )),
            false => Ok(()),
        };
    }
    let plain = !folder.is_empty() && !folder.starts_with('.') && !folder.contains(['/', '\\']);
    let taken = [config.inbox.as_str(), config.archive.as_str()].contains(&folder)
        || config.layer_of(folder).is_some();
    match (plain, taken) {
        (false, _) => Err(format!(
            "repos: {}: the folder must be one name at the root",
            row.name
        )),
        (_, true) => Err(format!(
            "repos: {}: {folder} is a folder the path rule uses",
            row.name
        )),
        _ => Ok(()),
    }
}

/// A repository the store's git calls run in, with the identity of its own commits.
#[derive(Debug, Clone)]
pub(crate) struct Home {
    /// The row's name.
    pub(crate) name: String,
    /// The repository's folder.
    pub(crate) dir: PathBuf,
    /// `""` for the root, else `<folder>/`.
    prefix: String,
    /// `user.name` and `user.email` the row gives.
    identity: Option<(String, String)>,
}

impl Home {
    fn of(root: &Path, row: &RepoRow) -> Home {
        let at_root = row.folder == ROOT_FOLDER;
        let identity = match (&row.git_name, &row.git_email) {
            (Some(n), Some(e)) => Some((n.clone(), e.clone())),
            _ => None,
        };
        Home {
            name: row.name.clone(),
            dir: if at_root {
                root.to_path_buf()
            } else {
                root.join(&row.folder)
            },
            prefix: if at_root {
                String::new()
            } else {
                format!("{}/", row.folder)
            },
            identity,
        }
    }

    /// The folder's own signature here: the row's identity, else the folder's.
    pub(crate) fn owner(&self) -> Who<'_> {
        match &self.identity {
            Some((name, email)) => Who::Named { name, email },
            None => Who::Folder,
        }
    }

    /// A path from the data folder, made relative to this repository.
    pub(crate) fn inner(&self, full: &str) -> String {
        full.strip_prefix(&self.prefix).unwrap_or(full).to_string()
    }

    /// A path of this repository, made relative to the data folder.
    pub(crate) fn outer(&self, rel: &str) -> String {
        format!("{}{rel}", self.prefix)
    }
}

/// The root's repository, when the root is one.
pub(crate) fn root_home(store: &Store) -> Option<Home> {
    let row = root_row(&store.config, &store.root);
    git::is_repo(&store.root).then(|| Home::of(&store.root, &row))
}

/// The repository holding `rel`: its row's folder when it is one; the root for a piece at the
/// root or under an alias with no repository of its own; `None` otherwise.
pub(crate) fn home_of(store: &Store, rel: &str) -> Option<Home> {
    let first = rel.split('/').next().unwrap_or("");
    match row_at(&store.config, first) {
        Some(row) if git::is_repo(&store.root.join(&row.folder)) => {
            Some(Home::of(&store.root, &row))
        }
        Some(row) if !is_alias(&row, &store.config) => None,
        _ => root_home(store),
    }
}

/// The repository a write at `rel` commits in: its row's own when it holds one, else the root's.
pub(crate) fn home_for_write(store: &Store, rel: &str) -> Option<Home> {
    home_of(store, rel).or_else(|| root_home(store))
}

/// Every repository present: the root, then each row's folder holding one.
pub(crate) fn homes(store: &Store) -> Vec<Home> {
    let mut out: Vec<Home> = root_home(store).into_iter().collect();
    for state in states(store)
        .into_iter()
        .filter(|s| s.folder != ROOT_FOLDER && s.git)
    {
        if let Some(row) = row_at(&store.config, &state.folder) {
            out.push(Home::of(&store.root, &row));
        }
    }
    out
}

/// The paths grouped by the repository holding them, each made relative to it, in the order
/// first met; a path no repository holds is left out.
pub(crate) fn split(store: &Store, paths: &[String]) -> Vec<(Home, Vec<String>)> {
    let mut out: Vec<(Home, Vec<String>)> = Vec::new();
    for path in paths {
        let Some(home) = home_of(store, path) else {
            continue;
        };
        let inner = home.inner(path);
        match out.iter_mut().find(|(h, _)| h.dir == home.dir) {
            Some((_, list)) => list.push(inner),
            None => out.push((home, vec![inner])),
        }
    }
    out
}

/// The root's `.gitignore` file name.
const GITIGNORE: &str = ".gitignore";

/// Each listed row's folder written in the root's `.gitignore` when missing, one commit of
/// that line each (none while the file has changes of the owner's); a failure is silent.
pub(crate) fn ignore_rows(store: &Store) {
    let Some(root) = root_home(store) else {
        return;
    };
    let rows = store
        .config
        .repos
        .iter()
        .filter(|r| r.folder != ROOT_FOLDER);
    for row in rows {
        let _ = ignore_one(store, &root, row);
    }
}

fn ignore_one(store: &Store, root: &Home, row: &RepoRow) -> Result<(), crate::StoreError> {
    let path = store.root.join(GITIGNORE);
    let text = fs::read_to_string(&path).unwrap_or_default();
    let named = |l: &str| l.trim().trim_matches('/') == row.folder;
    if text.lines().any(named) {
        return Ok(());
    }
    let pending = !git::dirty(&store.root, &[GITIGNORE.to_string()])?.is_empty();
    let sep = if text.is_empty() || text.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let line = format!("{text}{sep}{}/\n", row.folder);
    fs::write(&path, line).map_err(|e| crate::index::io_error(&path, e))?;
    if pending {
        return Ok(());
    }
    let subject = format!("gitignore: the repository {}", row.name);
    git::commit(
        &store.root,
        &[GITIGNORE.to_string()],
        &[&subject],
        root.owner(),
    )
    .map(|_| ())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn config(yaml: &str) -> StoreConfig {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    #[test]
    fn a_listed_folder_is_a_row_and_confidential_stays_an_alias_until_listed() {
        let listed =
            config("repos:\n  - { name: acme, folder: acme, role: work, confidential: true }\n");
        assert_eq!(row_at(&listed, "acme").map(|r| r.name), Some("acme".into()));
        assert!(is_confidential("acme/memories/M-1.md", &listed));
        assert!(is_confidential("confidential/memories/M-1.md", &listed));
        assert_eq!(confidential_home(&listed), "acme");
        assert_eq!(confidential_home(&StoreConfig::default()), CONFIDENTIAL);
        assert!(!is_confidential("memories/M-1.md", &listed));
        let open = config("repos:\n  - { name: famille, folder: famille }\n");
        assert!(!is_confidential("famille/memories/M-1.md", &open));
        assert_eq!(
            row_at(&open, "famille").map(|r| r.role),
            Some(Role::Personal)
        );
    }

    #[test]
    fn a_bad_row_is_refused_with_its_name() {
        let refused = [
            (
                "repos:\n  - { name: a, folder: x }\n  - { name: a, folder: y }\n",
                "twice",
            ),
            (
                "repos:\n  - { name: a, folder: x }\n  - { name: b, folder: x }\n",
                "twice",
            ),
            (
                "repos:\n  - { name: p, folder: ., confidential: true }\n",
                "never confidential",
            ),
            ("repos:\n  - { name: a, folder: a/b }\n", "one name"),
            ("repos:\n  - { name: a, folder: memories }\n", "path rule"),
            ("repos:\n  - { name: ' ', folder: a }\n", "without a name"),
        ];
        for (yaml, said) in refused {
            let err = check_rows(&config(yaml)).unwrap_err();
            assert!(err.contains(said), "{yaml}: {err}");
        }
        assert!(
            serde_yaml_ng::from_str::<StoreConfig>(
                "repos:\n  - { name: a, folder: a, colour: red }\n"
            )
            .is_err()
        );
        assert!(
            check_rows(&config(
                "repos:\n  - { name: p, folder: . }\n  - { name: a, folder: a }\n"
            ))
            .is_ok()
        );
    }

    #[test]
    fn a_row_may_name_its_providers_once_each() {
        let listed = config(
            "repos:\n  - { name: acme, folder: acme, confidential: true, providers: [anthropic, gemini] }\n",
        );
        assert!(check_rows(&listed).is_ok());
        let row = row_at(&listed, "acme").unwrap();
        assert_eq!(row.providers, ["anthropic", "gemini"]);
        assert!(
            row_at(&config("repos:\n  - { name: a, folder: a }\n"), "a")
                .unwrap()
                .providers
                .is_empty()
        );
        let twice = config("repos:\n  - { name: a, folder: a, providers: [gemini, gemini] }\n");
        let err = check_rows(&twice).unwrap_err();
        assert!(err.contains("gemini twice") && err.contains("a:"), "{err}");
        let blank = config("repos:\n  - { name: a, folder: a, providers: [' '] }\n");
        assert!(check_rows(&blank).unwrap_err().contains("blank provider"));
    }

    #[test]
    fn a_bad_prefix_is_refused_naming_its_row() {
        let refused = [
            (
                "  - { name: p, folder: ., prefix: P }\n",
                "root takes no prefix",
            ),
            ("  - { name: w, folder: w, prefix: a }\n", "upper-case"),
            ("  - { name: w, folder: w, prefix: A1 }\n", "upper-case"),
            ("  - { name: w, folder: w, prefix: '' }\n", "upper-case"),
            ("  - { name: w, folder: w, prefix: M }\n", "layer's"),
            ("  - { name: w, folder: w, prefix: T }\n", "layer's"),
            (
                "  - { name: v, folder: v, prefix: AB }\n  - { name: w, folder: w, prefix: AB }\n",
                "AB twice",
            ),
        ];
        for (rows, said) in refused {
            let err = check_rows(&config(&format!("repos:\n{rows}"))).unwrap_err();
            assert!(err.contains(said) && err.contains(": "), "{rows}: {err}");
        }
        let good = config(
            "repos:\n  - { name: p, folder: . }\n  - { name: w, folder: w, prefix: A }\n  - { name: f, folder: f }\n",
        );
        assert!(check_rows(&good).is_ok());
        assert_eq!(numbered_rows(&good), [("w", "A")]);
        assert_eq!(prefix_named(&good, "w"), Some("A"));
        assert_eq!(prefix_named(&good, "f"), None);
        assert_eq!(
            numbering_of(&good, "inbox/A-0001.md", Some("A")).as_deref(),
            Some("A")
        );
        assert_eq!(
            numbering_of(&good, "w/memories/M-0007.md", Some("M")).as_deref(),
            Some("A")
        );
        assert_eq!(numbering_of(&good, "f/memories/M-0007.md", Some("M")), None);
    }

    #[test]
    fn a_home_turns_paths_both_ways() {
        let row = RepoRow {
            git_name: Some("Work".into()),
            git_email: Some("work@corp".into()),
            ..alias()
        };
        let home = Home::of(Path::new("/data"), &row);
        assert_eq!(
            home.inner("confidential/memories/M-1.md"),
            "memories/M-1.md"
        );
        assert_eq!(
            home.outer("memories/M-1.md"),
            "confidential/memories/M-1.md"
        );
        assert_eq!(
            home.owner(),
            Who::Named {
                name: "Work",
                email: "work@corp"
            }
        );
    }
}
