//! Layer — what kind of piece a file is: memory, task, diary day, person, conversation day or tag.
//!
//! The layer comes from the folder a file sits in, through the config's `layers` map; this
//! file holds the prefixes, the default map and the name of the confidential folder.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// =============================================================================
// Interface
// =============================================================================

/// The folder at the root whose pieces are confidential.
pub const CONFIDENTIAL: &str = "confidential";

/// The default folder of the tag pieces.
pub const TAGS: &str = "tags";

/// The kind of a piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    /// A memory, `M-` ids.
    Memory,
    /// A task, `T-` ids.
    Task,
    /// A diary day, named by its date.
    Diary,
    /// A person, named by a slug of the name.
    Person,
    /// A day of the conversation with Lorisse, named by its date.
    Conversation,
    /// A tag: its word, its definition, named by a slug of the word.
    Tag,
}

impl Layer {
    /// Every layer, in order.
    pub const ALL: [Layer; 6] = [
        Layer::Memory,
        Layer::Task,
        Layer::Diary,
        Layer::Person,
        Layer::Conversation,
        Layer::Tag,
    ];

    /// The id prefix of a numbered layer: `M`, `T`; `None` for a named one.
    pub fn prefix(self) -> Option<&'static str> {
        prefix_of(self)
    }

    /// The layer whose ids carry this prefix.
    pub fn from_prefix(prefix: &str) -> Option<Layer> {
        Layer::ALL.into_iter().find(|l| l.prefix() == Some(prefix))
    }

    /// The default `layers` map of `config.yaml`: folder → layer.
    pub fn default_folders() -> BTreeMap<String, Layer> {
        default_map()
    }
}

// =============================================================================
// Implementation
// =============================================================================

fn prefix_of(layer: Layer) -> Option<&'static str> {
    match layer {
        Layer::Memory => Some("M"),
        Layer::Task => Some("T"),
        Layer::Diary | Layer::Person | Layer::Conversation | Layer::Tag => None,
    }
}

fn default_map() -> BTreeMap<String, Layer> {
    [
        ("memories", Layer::Memory),
        ("tasks", Layer::Task),
        ("journal", Layer::Diary),
        ("people", Layer::Person),
        ("conversations", Layer::Conversation),
        (TAGS, Layer::Tag),
    ]
    .into_iter()
    .map(|(folder, layer)| (folder.to_string(), layer))
    .collect()
}

/// The `kind:` a new named piece of this layer is given.
pub(crate) fn default_kind(layer: Layer) -> Option<&'static str> {
    match layer {
        Layer::Diary => Some("diary"),
        Layer::Person => Some("fact"),
        Layer::Conversation => Some("conversation"),
        Layer::Memory | Layer::Task | Layer::Tag => None,
    }
}

/// A layer whose pieces are days named by their date.
pub(crate) fn is_day(layer: Layer) -> bool {
    matches!(layer, Layer::Diary | Layer::Conversation)
}

/// The lowercase name of a layer.
fn name_of(layer: Layer) -> &'static str {
    match layer {
        Layer::Memory => "memory",
        Layer::Task => "task",
        Layer::Diary => "diary",
        Layer::Person => "person",
        Layer::Conversation => "conversation",
        Layer::Tag => "tag",
    }
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(name_of(*self))
    }
}

impl FromStr for Layer {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Layer::ALL
            .into_iter()
            .find(|l| name_of(*l) == s)
            .ok_or_else(|| format!("unknown layer: {s}"))
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layer_reads_back_its_name_and_its_prefix() {
        for layer in Layer::ALL {
            assert_eq!(layer.to_string().parse::<Layer>(), Ok(layer));
        }
        assert_eq!(Layer::from_prefix("T"), Some(Layer::Task));
        assert_eq!(Layer::Diary.prefix(), None);
        assert!("mood".parse::<Layer>().is_err());
        assert_eq!(Layer::default_folders()["journal"], Layer::Diary);
        assert_eq!(
            Layer::default_folders()["conversations"],
            Layer::Conversation
        );
        assert!(is_day(Layer::Conversation) && !is_day(Layer::Person));
        assert_eq!(Layer::default_folders()[TAGS], Layer::Tag);
        assert_eq!(Layer::Tag.prefix(), None);
    }
}
