//! The call, as the hook receives it: the tool's full name, its input, the session's folder.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// The app's file tools: a change of the rules file through them always asks.
pub const FILE_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];

/// One call, read.
#[derive(Debug, Clone)]
pub struct Call {
    pub name: String,
    pub input: Value,
    pub cwd: Option<String>,
}

/// The text the hook received, read; `None` when it is no `PreToolUse` call.
pub fn read(received: &str) -> Option<Call> {
    let value: Value = serde_json::from_str(received).ok()?;
    let object = value.as_object()?;
    if let Some(event) = object.get("hook_event_name") {
        if event.as_str() != Some("PreToolUse") {
            return None;
        }
    }
    let name = object.get("tool_name")?.as_str()?.trim().to_string();
    if name.is_empty() {
        return None;
    }
    Some(Call {
        name,
        input: object.get("tool_input").cloned().unwrap_or(Value::Null),
        cwd: object.get("cwd").and_then(Value::as_str).map(str::to_string),
    })
}

impl Call {
    /// The server's part and the tool: what precedes and what follows the last `__`.
    pub fn split(&self) -> Option<(&str, &str)> {
        let at = self.name.rfind("__")?;
        let (server, tool) = (&self.name[..at], &self.name[at + 2..]);
        match server.is_empty() || tool.is_empty() {
            true => None,
            false => Some((server, tool)),
        }
    }

    /// Whether this is one of the app's file tools.
    pub fn is_file_tool(&self) -> bool {
        FILE_TOOLS.contains(&self.name.as_str())
    }

    /// The file a file tool's call targets, made absolute against the session's folder.
    pub fn target(&self) -> Option<PathBuf> {
        let given = ["file_path", "notebook_path"]
            .iter()
            .find_map(|key| self.input.get(*key).and_then(Value::as_str))?;
        let path = PathBuf::from(given.trim());
        if path.is_absolute() || path.has_root() {
            return Some(path);
        }
        let base = match &self.cwd {
            Some(cwd) => PathBuf::from(cwd),
            None => std::env::current_dir().ok()?,
        };
        Some(base.join(path))
    }
}

/// A path as compared: forward slashes, `.` and `..` resolved by hand, capitals ignored.
fn compared(path: &Path) -> String {
    let text = path.display().to_string().replace('\\', "/");
    let text = text.strip_prefix("//?/").unwrap_or(&text).to_lowercase();
    let rooted = text.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in text.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    let joined = parts.join("/");
    match rooted {
        true => format!("/{joined}"),
        false => joined,
    }
}

/// Whether `target` is the file `wanted`: by its words, else by the folders the system resolves.
pub fn same_file(target: &Path, wanted: &Path) -> bool {
    if compared(target) == compared(wanted) {
        return true;
    }
    let resolved = |path: &Path| -> Option<String> {
        let parent = std::fs::canonicalize(path.parent()?).ok()?;
        Some(compared(&parent.join(path.file_name()?)))
    };
    match (resolved(target), resolved(wanted)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}
