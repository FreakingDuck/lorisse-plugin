//! First — a new memory folder put under git: on the machine only, a name and an address set
//! for its commits in that folder alone, its first state one commit signed as the person's.
//! Flow: a folder made ready → `git_found` → `put_under_git` → the first commit's id.

use std::path::Path;

use crate::error::StoreError;
use crate::git::{self, Who};

/// Whether a `git` program can be started on this machine.
pub fn git_found() -> bool {
    git::available()
}

/// `git init` in `root`, `user.name` and `user.email` set in that folder only, then every file
/// in one first commit with `subject`, signed as the person; the commit's full id.
///
/// # Errors
/// [`StoreError::Git`] naming the git call that failed.
pub fn put_under_git(root: &Path, name: &str, email: &str, subject: &str) -> Result<String, StoreError> {
    git::run(root, &["init", "-q"], Who::Folder)?;
    git::run(root, &["config", "user.name", name], Who::Folder)?;
    git::run(root, &["config", "user.email", email], Who::Folder)?;
    git::run(root, &["add", "-A"], Who::Folder)?;
    git::run(root, &["commit", "-q", "-m", subject], Who::Folder)?;
    git::head(root)
}
