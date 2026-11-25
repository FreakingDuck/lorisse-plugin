//! StoreConfig — the data folder's `config.yaml`: layer folders, enumerations, sources.
//!
//! The only place a `config.yaml` key is spelled. The file is optional, every key has a
//! default, an unknown key is refused with its name.
//! Flow: `config.yaml` → `StoreConfig::load` → the path rule, the checks, the defaults.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::error::StoreError;
use crate::ids::MIN_WIDTH;
use crate::layer::Layer;
use crate::repos::{self, RepoRow};

// =============================================================================
// Interface
// =============================================================================

/// The configuration file's name at the root.
pub const CONFIG_FILE: &str = "config.yaml";

/// The parsed `config.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StoreConfig {
    /// Folder name → layer, added to the default map.
    pub layers: BTreeMap<String, Layer>,
    /// The capture folder.
    pub inbox: String,
    /// The archive folder name.
    pub archive: String,
    /// Allowed memory `kind` values.
    pub kinds: Vec<String>,
    /// Allowed memory `status` values; the first is a capture's.
    pub memory_statuses: Vec<String>,
    /// Allowed task `status` values; the first is a new task's.
    pub task_statuses: Vec<String>,
    /// Allowed task `priority` values.
    pub priorities: Vec<String>,
    /// Allowed task `category` values.
    pub categories: Vec<String>,
    /// Allowed `source` values.
    pub sources: Vec<String>,
    /// The `owner` a new task is given; empty = none.
    pub owner: String,
    /// The minimum number of digits of a new id.
    pub id_width: usize,
    /// A new person's two body headings: what I know, how I handle.
    pub person_sections: [String; 2],
    /// The repositories under the data folder; empty = the root alone.
    pub repos: Vec<RepoRow>,
    /// The largest file `attach` copies in, in MB.
    pub attach_limit_mb: u64,
}

/// The largest file `attach` copies in when the folder's settings say nothing, in MB.
pub const ATTACH_LIMIT_MB: u64 = 20;

impl StoreConfig {
    /// Read `config.yaml` under `root`; the defaults when there is none.
    ///
    /// # Errors
    /// [`StoreError::Config`] naming an unknown key or a bad value; [`StoreError::Io`].
    pub fn load(root: &Path) -> Result<StoreConfig, StoreError> {
        load_file(&root.join(CONFIG_FILE))
    }

    /// The layer a folder name holds, if the map names it.
    pub fn layer_of(&self, folder: &str) -> Option<Layer> {
        self.layers.get(folder).copied()
    }

    /// The first folder holding `layer`, if any.
    pub fn folder_of(&self, layer: Layer) -> Option<&str> {
        self.layers
            .iter()
            .find(|(_, l)| **l == layer)
            .map(|(f, _)| f.as_str())
    }
}

// =============================================================================
// Implementation
// =============================================================================

/// The status words the store itself writes.
pub(crate) mod status {
    /// A capture not yet sorted.
    pub(crate) const RAW: &str = "raw";
    /// A memory sorted by an accepted diff.
    pub(crate) const SORTED: &str = "sorted";
    /// A piece kept but out of use: a memory, a dropped task or person.
    pub(crate) const RETIRED: &str = "retired";
    /// A finished task.
    pub(crate) const DONE: &str = "done";
}

/// The list names a check finding quotes.
pub(crate) mod list {
    /// Memory kinds.
    pub(crate) const KINDS: &str = "kinds";
    /// Memory statuses.
    pub(crate) const MEMORY_STATUSES: &str = "memory_statuses";
    /// Task statuses.
    pub(crate) const TASK_STATUSES: &str = "task_statuses";
    /// Task priorities.
    pub(crate) const PRIORITIES: &str = "priorities";
    /// Task categories.
    pub(crate) const CATEGORIES: &str = "categories";
    /// Capture sources.
    pub(crate) const SOURCES: &str = "sources";
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| w.to_string()).collect()
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            layers: Layer::default_folders(),
            inbox: "inbox".into(),
            archive: "archive".into(),
            kinds: words(&["fact", "decision", "preference", "idea", "need"]),
            memory_statuses: words(&[status::RAW, status::SORTED, status::RETIRED]),
            task_statuses: words(&[
                "todo",
                "in-progress",
                "waiting",
                status::DONE,
                status::RETIRED,
            ]),
            priorities: words(&["high", "medium", "low"]),
            categories: words(&["home", "family", "work", "health", "money", "admin"]),
            sources: words(&["phone", "chat", "agent", "console", "app"]),
            owner: String::new(),
            id_width: MIN_WIDTH,
            person_sections: ["What I know".into(), "How I handle".into()],
            repos: Vec::new(),
            attach_limit_mb: ATTACH_LIMIT_MB,
        }
    }
}

impl StoreConfig {
    /// The values of the list named `name`.
    pub(crate) fn list(&self, name: &str) -> &[String] {
        match name {
            list::KINDS => &self.kinds,
            list::MEMORY_STATUSES => &self.memory_statuses,
            list::TASK_STATUSES => &self.task_statuses,
            list::PRIORITIES => &self.priorities,
            list::CATEGORIES => &self.categories,
            list::SOURCES => &self.sources,
            _ => &[],
        }
    }
}

fn load_file(path: &Path) -> Result<StoreConfig, StoreError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(StoreConfig::default()),
        Err(e) => return Err(StoreError::Io(format!("{}: {e}", path.display()))),
    };
    if text.trim().is_empty() {
        return Ok(StoreConfig::default());
    }
    parse_text(&text)
}

/// The config of a `config.yaml` text; its `layers` added to the default map.
fn parse_text(text: &str) -> Result<StoreConfig, StoreError> {
    let mut config: StoreConfig = serde_yaml_ng::from_str(text)
        .map_err(|e| StoreError::Config(format!("{CONFIG_FILE}: {e}")))?;
    let mut layers = Layer::default_folders();
    layers.append(&mut config.layers);
    config.layers = layers;
    if config.inbox.is_empty() || config.archive.is_empty() {
        return Err(StoreError::Config(format!(
            "{CONFIG_FILE}: an empty folder name"
        )));
    }
    repos::check_rows(&config).map_err(|e| StoreError::Config(format!("{CONFIG_FILE}: {e}")))?;
    Ok(config)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partial_file_keeps_the_other_defaults() {
        let config: StoreConfig = serde_yaml_ng::from_str("owner: Name\nid_width: 5\n").unwrap();
        assert_eq!(config.owner, "Name");
        assert_eq!(config.id_width, 5);
        assert_eq!(config.inbox, "inbox");
        assert_eq!(config.folder_of(Layer::Task), Some("tasks"));
        assert_eq!(config.person_sections[1], "How I handle");
        let french = "person_sections: [Ce que je sais, Comment je gère]\n";
        let french: StoreConfig = serde_yaml_ng::from_str(french).unwrap();
        assert_eq!(french.person_sections[0], "Ce que je sais");
    }

    #[test]
    fn a_folder_without_settings_gets_plain_categories() {
        let config = StoreConfig::default();
        assert_eq!(config.categories, ["home", "family", "work", "health", "money", "admin"]);
    }

    #[test]
    fn a_layers_map_adds_to_the_default_folders() {
        let config = parse_text(
            "layers: { notes: memory, tasks: diary }
",
        )
        .unwrap();
        assert_eq!(config.layer_of("notes"), Some(Layer::Memory));
        assert_eq!(config.layer_of("tasks"), Some(Layer::Diary));
        assert_eq!(config.layer_of("people"), Some(Layer::Person));
        assert_eq!(config.folder_of(Layer::Memory), Some("memories"));
    }

    #[test]
    fn an_unknown_layer_name_is_refused() {
        let err = serde_yaml_ng::from_str::<StoreConfig>("layers: { notes: mood }\n").unwrap_err();
        assert!(err.to_string().contains("mood"), "{err}");
    }
}
