//! lorisse-store —the data folder's one door: pieces as Markdown files, the inbox, reviewed
//! diffs, the index, the confidential gate, the repositories under the folder.
//!
//! Reading order: `piece.rs`, `repos.rs`, `slice.rs` (the path rule), `store.rs`, `actions.rs`,
//! `diff.rs`, then `route.rs` and `direct.rs` (a job's action written at once, one commit each), `kept.rs` (a
//! conversation of the app stored as one memory, updated in place).
//! Flow: files → `Store::open` → `Entry`s → `propose(Action)` → `Diff` → `accept` → files.
//! A piece's files sit beside it (`<stem>/<name>`, `attach.rs`) and move with it.

#![warn(missing_docs)]

pub mod actions;
pub mod attach;
pub mod check;
pub mod config;
mod diary;
pub mod diff;
pub mod direct;
pub mod error;
pub mod filing;
pub mod first;
pub mod flags;
mod frontmatter;
mod git;
pub mod ids;
pub mod inbox;
pub mod index;
pub mod kept;
pub mod layer;
pub mod links;
pub mod notes;
pub mod piece;
pub mod repos;
pub mod resolution;
pub mod route;
pub mod search;
mod slice;
pub mod store;
pub mod tags;

pub use actions::{Action, Fields};
pub use attach::Attachment;
pub use check::{Finding, Report};
pub use config::{CONFIG_FILE, StoreConfig};
pub use diary::Speaker;
pub use diff::{Change, Diff};
pub use direct::{Applied, CommitState, DirectWrite, Fined, Flagged, LorisseCommit, Since};
pub use error::StoreError;
pub use filing::{Attached, Blocked, Dated, Imported, Journaled, NewMemory, Revised, Revision, Undone, Written};
pub use flags::Flags;
pub use frontmatter::key;
pub use ids::PieceId;
pub use inbox::{Proposal, Source};
pub use index::{Entry, IDS_FILE, INDEX_FILE};
pub use kept::{CONVERSATION, Conversation, Kept};
pub use layer::Layer;
pub use links::{Links, Ref};
pub use piece::Piece;
pub use repos::{RepoRow, RepoState, Role};
pub use resolution::{DOES, Does, Resolution};
pub use route::{
    Certainty, Context, EXTERNAL, Kind, NOISE_WORDS, Route, Routing, WriteMode, is_noise,
};
pub use search::Hit;
pub use serde_yaml_ng::Value;
pub use store::{Access, Filter, Store};
pub use tags::TagInfo;
