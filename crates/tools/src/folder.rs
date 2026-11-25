//! The memory folder: the folder the session is opened on when it is a memory folder, else the
//! plugin's setting; nowhere else. Read from the process's environment at each call.

use std::path::PathBuf;

use lorisse_store::CONFIG_FILE;

/// The variable the plugin's setting reaches the program as.
pub const SETTING: &str = "LORISSE_MEMORY_FOLDER";

/// The same setting as the Claude app may give it to a plugin's hook.
const OPTION: &str = "CLAUDE_PLUGIN_OPTION_MEMORY_FOLDER";

/// The variable the Claude app gives with the session's folder.
const PROJECT: &str = "CLAUDE_PROJECT_DIR";

/// How the folder was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    Setting,
    Session,
}

/// The outcome of the search for a memory folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Folder {
    /// A memory folder, and how it was found.
    At(PathBuf, Found),
    /// A folder given by the setting that is no memory folder.
    NotMemory(PathBuf),
    /// The session's folder is no memory folder, and no setting.
    None,
}

impl Found {
    /// The words `where` says.
    pub fn words(self) -> &'static str {
        match self {
            Found::Setting => "the plugin's setting",
            Found::Session => "the folder this session is opened on",
        }
    }
}

/// The one sentence for no memory folder.
pub const NO_FOLDER: &str = "Lorisse has no memory folder: give one in the plugin's setting \
\"memory_folder\", or open the session on your memory folder.";

/// The one sentence for a setting that names no memory folder.
pub fn not_memory(path: &PathBuf) -> String {
    format!(
        "The folder given in the plugin's setting, {}, is not a memory folder: it has no {CONFIG_FILE} at its root.",
        shown(path)
    )
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
        Some(path) if is_memory_folder(&path) => Folder::At(path, Found::Setting),
        Some(path) => Folder::NotMemory(path),
        None => Folder::None,
    }
}
