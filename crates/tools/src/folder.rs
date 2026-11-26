//! The memory folder: the folder the session is opened on when it is a memory folder, else the
//! plugin's setting, else the one this machine remembers from `start`; nowhere else. Read from
//! the process's environment at each call.

use std::fs;
use std::path::PathBuf;

use lorisse_store::{CONFIG_FILE, IDS_FILE};

/// The variable the plugin's setting reaches the program as.
pub const SETTING: &str = "LORISSE_MEMORY_FOLDER";

/// The same setting as the Claude app may give it to a plugin's hook.
const OPTION: &str = "CLAUDE_PLUGIN_OPTION_MEMORY_FOLDER";

/// The variable the Claude app gives with the session's folder.
const PROJECT: &str = "CLAUDE_PROJECT_DIR";

/// The home folder, when given; else the person's own.
pub const HOME: &str = "LORISSE_HOME";

/// The one small file, at the root of the home folder, that holds the folder `start` made.
pub const REMEMBERED: &str = ".lorisse-folder";

/// How the folder was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    Setting,
    Session,
    Remembered,
}

/// The outcome of the search for a memory folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Folder {
    /// A memory folder, and how it was found.
    At(PathBuf, Found),
    /// A folder given by the setting that is no memory folder.
    NotMemory(PathBuf),
    /// The session's folder is no memory folder, no setting, nothing remembered.
    None,
}

impl Found {
    /// The words `where` says.
    pub fn words(self) -> &'static str {
        match self {
            Found::Setting => "the plugin's setting",
            Found::Session => "the folder this session is opened on",
            Found::Remembered => "the folder this machine remembers from the first day",
        }
    }
}

/// The one sentence for no memory folder.
pub const NO_FOLDER: &str = "Lorisse has no memory folder yet: the tool start makes one ready \
in a minute, or give one in the plugin's setting \"memory_folder\", or open the session on your memory folder.";

/// Where the person changes the setting, the end of the sentences on a wrong setting.
const WHERE_SET: &str = "the setting is changed in the Claude app under Customize, Plugins, Lorisse, and a new conversation reads it";

/// The one sentence for a setting that names no memory folder: one repository of a memory
/// (a `memories` folder or an `ids.json`, no settings), or a folder that is nothing of the kind.
pub fn not_memory(path: &PathBuf) -> String {
    let one_repository = path.join("memories").is_dir() || path.join(IDS_FILE).is_file();
    match one_repository {
        true => format!(
            "The folder given in the plugin's setting, {}, looks like one repository of a memory, not the folder that holds them all: give the folder that holds {CONFIG_FILE}; {WHERE_SET}.",
            shown(path)
        ),
        false => format!(
            "The folder given in the plugin's setting, {}, is not a memory folder: it has no {CONFIG_FILE} at its root; {WHERE_SET}.",
            shown(path)
        ),
    }
}

/// A path as it is said: forward slashes, no trailing separator.
pub fn shown(path: &PathBuf) -> String {
    let text = path.display().to_string().replace('\\', "/");
    let trimmed = text.trim_end_matches('/');
    match trimmed.is_empty() {
        true => text,
        false => trimmed.to_string(),
    }
}

/// The rule for "this is a memory folder": a folder holding the store's `config.yaml` at its root.
pub fn is_memory_folder(path: &PathBuf) -> bool {
    path.join(CONFIG_FILE).is_file()
}

/// A variable's value, or `None` when absent, blank, or still an unreplaced `${...}`.
fn given(name: &str) -> Option<String> {
    let value = std::env::var_os(name)?;
    let value = value.to_string_lossy().trim().to_string();
    if value.is_empty() || value.contains("${") {
        return None;
    }
    Some(value)
}

/// The home folder: `LORISSE_HOME` when given, else the person's own.
pub fn home() -> Option<PathBuf> {
    given(HOME)
        .or_else(|| given("USERPROFILE"))
        .or_else(|| given("HOME"))
        .map(PathBuf::from)
}

/// The file that holds the folder this machine remembers.
pub fn remembered_file() -> Option<PathBuf> {
    home().map(|h| h.join(REMEMBERED))
}

/// The folder this machine remembers, when its file names one.
fn remembered() -> Option<PathBuf> {
    let text = fs::read_to_string(remembered_file()?).ok()?;
    let line = text.lines().next()?.trim();
    (!line.is_empty()).then(|| PathBuf::from(line))
}

/// The memory folder of this call.
pub fn find() -> Folder {
    let session = given(PROJECT)
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok());
    if let Some(path) = session {
        if is_memory_folder(&path) {
            return Folder::At(path, Found::Session);
        }
    }
    match given(SETTING).or_else(|| given(OPTION)).map(PathBuf::from) {
        Some(path) if is_memory_folder(&path) => return Folder::At(path, Found::Setting),
        Some(path) => return Folder::NotMemory(path),
        None => {}
    }
    match remembered() {
        Some(path) if is_memory_folder(&path) => Folder::At(path, Found::Remembered),
        _ => Folder::None,
    }
}
