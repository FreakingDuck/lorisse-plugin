//! StoreError — the one error type of the crate.
//!
//! Every message names what a person needs to find the problem: the path, the id, the key.

use std::path::PathBuf;

// =============================================================================
// Interface
// =============================================================================

/// Everything that can go wrong opening, reading, proposing or accepting.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// A file or folder could not be read or written; names the path and the cause.
    #[error("io: {0}")]
    Io(String),
    /// A frontmatter or a proposal is not YAML.
    #[error("yaml: {}: {reason}", .path.display())]
    Yaml {
        /// The file, relative to the root.
        path: PathBuf,
        /// What the parser said.
        reason: String,
    },
    /// A file is YAML but not a piece: a fence, a key, a date.
    #[error("format: {}: {reason}", .path.display())]
    Format {
        /// The file, relative to the root.
        path: PathBuf,
        /// What is wrong, naming the key.
        reason: String,
    },
    /// No piece has this id.
    #[error("no such piece: {0}")]
    Unknown(String),
    /// The piece is behind the gate; only its id is named.
    #[error("confidential: {0}")]
    Confidential(String),
    /// The file differs from the diff's before-text.
    #[error("{}: changed since the diff was made", .0.display())]
    Changed(PathBuf),
    /// A create would overwrite this file.
    #[error("{}: already exists", .0.display())]
    Exists(PathBuf),
    /// `config.yaml` is not usable.
    #[error("config: {0}")]
    Config(String),
    /// A value or an action the configuration or the rules refuse.
    #[error("invalid: {0}")]
    Invalid(String),
    /// A git call failed; names the call and what git said.
    #[error("{0}")]
    Git(String),
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_names_the_path_and_the_reason() {
        let e = StoreError::Format {
            path: PathBuf::from("memories/M-0001.md"),
            reason: "updated: not a date".into(),
        };
        assert_eq!(
            e.to_string(),
            "format: memories/M-0001.md: updated: not a date"
        );
        let changed = StoreError::Changed(PathBuf::from("inbox/M-0007.md"));
        assert_eq!(
            changed.to_string(),
            "inbox/M-0007.md: changed since the diff was made"
        );
        assert_eq!(
            StoreError::Confidential("M-0003".into()).to_string(),
            "confidential: M-0003"
        );
    }
}
