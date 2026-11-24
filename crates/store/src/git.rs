//! Git — the store's few git calls on one repository: status, add, commit, revert, log.
//!
//! One `git` process per call, in the repository's folder, paths relative to it; no push, no
//! history rewrite. Lorisse's commits carry her name; the folder's own commits and reverts carry
//! the folder's, or the identity its `repos:` row names.
//! Flow: paths → `dirty` / `commit` → a commit id; a commit id → `revert`; `log` → text.

use std::path::Path;
use std::process::Command;

use crate::error::StoreError;

// =============================================================================
// Implementation
// =============================================================================

/// The author of Lorisse's commits.
pub(crate) const LORISSE_NAME: &str = "Lorisse";

/// Her mail, the key `git log --author` finds her by.
pub(crate) const LORISSE_EMAIL: &str = "lorisse@local";

/// Who signs a commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Who<'a> {
    /// Lorisse, whatever the folder's identity.
    Lorisse,
    /// The folder's own identity: the owner's.
    Folder,
    /// The identity a `repos:` row names for the folder's own commits.
    Named {
        /// `user.name`.
        name: &'a str,
        /// `user.email`.
        email: &'a str,
    },
}

/// Windows: no console window for a git started from the app.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Whether the root holds its own repository.
pub(crate) fn is_repo(root: &Path) -> bool {
    root.join(".git").exists()
}

/// Run `git <args>` in the root; its standard output, or its error as a message.
pub(crate) fn run(root: &Path, args: &[&str], who: Who) -> Result<String, StoreError> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .args(["-c", "core.quotepath=false"]);
    for name in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"] {
        command.env_remove(name);
    }
    match who {
        Who::Lorisse => sign(&mut command, LORISSE_NAME, LORISSE_EMAIL),
        Who::Named { name, email } => sign(&mut command, name, email),
        Who::Folder => {}
    }
    hide_window(&mut command);
    let out = command
        .args(args)
        .output()
        .map_err(|e| StoreError::Git(format!("git could not start: {e}")))?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let what = args.first().copied().unwrap_or("");
        return Err(StoreError::Git(format!("git {what}: {said}")));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Author and committer set to `name <email>`, whatever the folder's own identity.
fn sign(command: &mut Command, name: &str, email: &str) {
    let mail = format!("user.email={email}");
    let user = format!("user.name={name}");
    command.args(["-c", &user, "-c", &mail]);
    for (key, value) in [
        ("GIT_AUTHOR_NAME", name),
        ("GIT_AUTHOR_EMAIL", email),
        ("GIT_COMMITTER_NAME", name),
        ("GIT_COMMITTER_EMAIL", email),
    ] {
        command.env(key, value);
    }
}

#[cfg(windows)]
fn hide_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_window(_command: &mut Command) {}

/// The given paths git sees changed or untracked.
pub(crate) fn dirty(root: &Path, paths: &[String]) -> Result<Vec<String>, StoreError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = vec![
        "status",
        "--porcelain",
        "-z",
        "--no-renames",
        "--untracked-files=all",
        "--",
    ];
    args.extend(paths.iter().map(String::as_str));
    let out = run(root, &args, Who::Folder)?;
    let entries = out.split('\0').filter(|e| e.len() > 3);
    Ok(entries.map(|e| e[3..].to_string()).collect())
}

/// The given paths git tracks.
fn tracked(root: &Path, paths: &[String]) -> Result<Vec<String>, StoreError> {
    let mut args = vec!["ls-files", "-z", "--"];
    args.extend(paths.iter().map(String::as_str));
    let out = run(root, &args, Who::Folder)?;
    Ok(out
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect())
}

/// Commit these paths alone, whatever else is staged; `None` when none is there or tracked.
pub(crate) fn commit(
    root: &Path,
    paths: &[String],
    message: &[&str],
    who: Who,
) -> Result<Option<String>, StoreError> {
    let tracked = tracked(root, paths)?;
    let known = |p: &&String| root.join(p.as_str()).exists() || tracked.contains(p);
    let list: Vec<&str> = paths.iter().filter(known).map(String::as_str).collect();
    if list.is_empty() {
        return Ok(None);
    }
    let mut add = vec!["add", "-A", "--"];
    add.extend(&list);
    run(root, &add, who)?;
    let mut args = vec!["commit", "--only", "--quiet"];
    for part in message.iter().copied() {
        args.extend(["-m", part]);
    }
    args.push("--");
    args.extend(&list);
    run(root, &args, who)?;
    head(root).map(Some)
}

/// The full id of `HEAD`.
pub(crate) fn head(root: &Path) -> Result<String, StoreError> {
    resolve(root, "HEAD")
}

/// The full id of a commit named by a full or short id.
pub(crate) fn resolve(root: &Path, name: &str) -> Result<String, StoreError> {
    let spec = format!("{name}^{{commit}}");
    let out = run(
        root,
        &["rev-parse", "--verify", "--quiet", &spec],
        Who::Folder,
    );
    let out = out.map_err(|_| StoreError::Invalid(format!("no such commit: {name}")))?;
    Ok(out.trim().to_string())
}

/// Whether the repository has a first commit.
pub(crate) fn has_head(root: &Path) -> bool {
    head(root).is_ok()
}

/// The files a commit touched, renames as a delete and a create.
pub(crate) fn files_of(root: &Path, commit: &str) -> Result<Vec<String>, StoreError> {
    let args = [
        "diff-tree",
        "--root",
        "--no-commit-id",
        "--name-only",
        "-r",
        "--no-renames",
        "-z",
        commit,
    ];
    let out = run(root, &args, Who::Folder)?;
    Ok(out
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect())
}

/// The full ids of the commits after `commit` touching `path`, newest first.
pub(crate) fn later_on(root: &Path, commit: &str, path: &str) -> Result<Vec<String>, StoreError> {
    let range = format!("{commit}..HEAD");
    let out = run(
        root,
        &["log", "--format=%H", &range, "--", path],
        Who::Folder,
    )?;
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

/// The paths staged in the index.
pub(crate) fn staged(root: &Path) -> Result<Vec<String>, StoreError> {
    let out = run(
        root,
        &["diff", "--cached", "--name-only", "-z"],
        Who::Folder,
    )?;
    Ok(out
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect())
}

/// Revert one commit with the folder's identity (`who`); the new commit's id.
pub(crate) fn revert(root: &Path, commit: &str, who: Who) -> Result<String, StoreError> {
    run(root, &["revert", "--no-edit", commit], who)?;
    head(root)
}

/// The author date of a commit, ISO 8601.
pub(crate) fn date_of(root: &Path, commit: &str) -> Result<String, StoreError> {
    let out = run(root, &["log", "-1", "--format=%aI", commit], Who::Folder)?;
    Ok(out.trim().to_string())
}

/// The full ids of the commits a revert names (`This reverts commit <id>.`).
pub(crate) fn reverted(root: &Path) -> Result<Vec<String>, StoreError> {
    Ok(reverts(root)?.into_iter().map(|(_, gone)| gone).collect())
}

/// Each revert commit's full id with the id of the commit it reverts.
pub(crate) fn reverts(root: &Path) -> Result<Vec<(String, String)>, StoreError> {
    if !has_head(root) {
        return Ok(Vec::new());
    }
    let args = ["log", "--format=%H%x1f%B%x1e", "--grep=This reverts commit"];
    let out = run(root, &args, Who::Folder)?;
    let marker = "This reverts commit ";
    let pair = |record: &str| {
        let (id, message) = record.trim().split_once('\u{1f}')?;
        let rest = &message[message.find(marker)? + marker.len()..];
        let gone: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
        (gone.len() >= 7).then(|| (id.to_string(), gone))
    };
    Ok(out.split('\u{1e}').filter_map(pair).collect())
}

/// The author mail of a commit.
pub(crate) fn author_mail(root: &Path, commit: &str) -> Result<String, StoreError> {
    let out = run(root, &["log", "-1", "--format=%ae", commit], Who::Folder)?;
    Ok(out.trim().to_string())
}

/// `git log` of Lorisse's commits with `extra` arguments; empty before the first commit.
pub(crate) fn lorisse_log(root: &Path, extra: &[&str]) -> Result<String, StoreError> {
    if !has_head(root) {
        return Ok(String::new());
    }
    let author = format!("--author=<{LORISSE_EMAIL}>");
    let mut args = vec!["log", author.as_str(), "--no-color"];
    args.extend(extra);
    run(root, &args, Who::Folder)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_without_its_own_git_folder_is_no_repository() {
        let here = std::env::temp_dir().join("lorisse_store_no_git_here");
        assert!(!is_repo(&here));
    }
}
