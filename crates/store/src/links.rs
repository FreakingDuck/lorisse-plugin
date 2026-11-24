//! Ref and Links — a link's text, its resolution through the index, both ends of it.
//!
//! A link is an id (`M-0001`), a path-like name (`people/robin`), a path cut to its bare
//! id, or a person's name; files hold one way, the index answers both.
//! Flow: `links:` → `Ref` → `bare` → `resolve` → `Links`, `dangling`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::actions::slug;
use crate::config::status::RETIRED;
use crate::ids::PieceId;
use crate::index::{Entry, Index};
use crate::layer::Layer;
use crate::slice::MD;

// =============================================================================
// Interface
// =============================================================================

/// The text of a link, as written in `links:`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ref(String);

/// Both ends of a piece's links.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Links {
    /// What the piece links to, as written.
    pub out: Vec<Ref>,
    /// The visible pieces that link to it, in id order.
    pub incoming: Vec<PieceId>,
}

impl Ref {
    /// A link from its text.
    pub fn new(text: &str) -> Ref {
        Ref(text.trim().to_string())
    }

    /// The link's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The id the link names, if it reads as one, a path read as its bare id.
    pub fn id(&self) -> Option<PieceId> {
        PieceId::parse(&bare_text(&self.0))
    }

    /// The link as a bare id: `../confidential/memories/M-0021.md` → `M-0021`.
    pub fn bare(&self) -> Ref {
        Ref(bare_text(&self.0))
    }
}

// =============================================================================
// Implementation
// =============================================================================

impl From<PieceId> for Ref {
    fn from(id: PieceId) -> Self {
        Ref(id.to_string())
    }
}

impl fmt::Display for Ref {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A link's text with a path cut to its id: `.md`, `.` and `..` dropped, a numbered last
/// segment alone, else the last two segments; a text with no path is kept.
fn bare_text(text: &str) -> String {
    if !text.contains('/') && !text.ends_with(MD) {
        return text.to_string();
    }
    let text = text.strip_suffix(MD).unwrap_or(text);
    let segments: Vec<&str> = text
        .split('/')
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .collect();
    match segments.as_slice() {
        [.., last] if crate::ids::numbered(last).is_some() => last.to_string(),
        [.., folder, stem] => format!("{folder}/{stem}"),
        [only] => only.to_string(),
        [] => text.to_string(),
    }
}

/// Whether `link` names `entry`: its id, the last two segments of its path, or a person's
/// name or slug.
pub(crate) fn names(link: &Ref, entry: &Entry, archive: &str) -> bool {
    match link.id() {
        Some(id @ PieceId::Numbered { .. }) => entry.id == id,
        Some(named) => crate::ids::named_from_path(&entry.path_text(), archive) == Some(named),
        None => entry.layer == Layer::Person && is_person(link.as_str(), entry),
    }
}

/// Whether `text` is the person's name (any case) or the slug of its file.
fn is_person(text: &str, entry: &Entry) -> bool {
    let slug = slug(text);
    let path = entry.path_text();
    let stem = path.rsplit('/').next().and_then(|f| f.strip_suffix(MD));
    entry.title.to_lowercase() == text.to_lowercase() || (!slug.is_empty() && stem == Some(&slug))
}

/// The visible entry a link names.
pub(crate) fn resolve<'a>(index: &'a Index, link: &Ref, archive: &str) -> Option<&'a Entry> {
    index.entries.iter().find(|e| names(link, e, archive))
}

/// Whether a link names a piece behind the gate, a name read as a named piece's slug.
pub(crate) fn is_hidden(index: &Index, link: &Ref) -> bool {
    match link.id() {
        Some(id) => index.hidden.contains(&id),
        None => {
            let slug = slug(link.as_str());
            let named = |id: &PieceId| matches!(id, PieceId::Named { stem, .. } if *stem == slug);
            !slug.is_empty() && index.hidden.iter().any(named)
        }
    }
}

/// Both ends of the piece `link` names.
pub(crate) fn links_of(index: &Index, link: &Ref, archive: &str) -> Links {
    let Some(target) = resolve(index, link, archive) else {
        return Links::default();
    };
    let incoming = index
        .entries
        .iter()
        .filter(|e| e.links.iter().any(|l| names(l, target, archive)))
        .map(|e| e.id.clone())
        .collect();
    Links {
        out: target.links.clone(),
        incoming,
    }
}

/// Every visible link that names no piece, visible or hidden.
pub(crate) fn dangling_links(index: &Index, archive: &str) -> Vec<(PieceId, Ref)> {
    let mut out = Vec::new();
    for entry in &index.entries {
        for link in &entry.links {
            if resolve(index, link, archive).is_none() && !is_hidden(index, link) {
                out.push((entry.id.clone(), link.clone()));
            }
        }
    }
    out
}

/// Every link from a live piece (not retired, not archived) to a retired one.
pub(crate) fn retired_links(index: &Index, archive: &str) -> Vec<(PieceId, Ref)> {
    let retired = |e: &Entry| e.status.as_deref() == Some(RETIRED);
    let mut out = Vec::new();
    for entry in index.entries.iter().filter(|e| !e.archived && !retired(e)) {
        for link in &entry.links {
            if resolve(index, link, archive).is_some_and(retired) {
                out.push((entry.id.clone(), link.clone()));
            }
        }
    }
    out
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ref_reads_its_id_and_keeps_its_text() {
        assert_eq!(Ref::new(" M-01 ").as_str(), "M-01");
        assert_eq!(Ref::new("M-01").id(), PieceId::parse("M-0001"));
        assert_eq!(
            Ref::from(PieceId::parse("T-3").unwrap()).to_string(),
            "T-0003"
        );
        assert_eq!(Ref::new("issue#12").id(), None);
    }

    #[test]
    fn a_path_is_cut_to_its_bare_id_and_a_plain_text_is_kept() {
        let bare = |t: &str| Ref::new(t).bare().as_str().to_string();
        assert_eq!(bare("../confidential/memories/M-0021"), "M-0021");
        assert_eq!(bare("memories/M-0021.md"), "M-0021");
        assert_eq!(bare("work/tasks/T-0027"), "T-0027");
        assert_eq!(bare("../confidential/people/robin.md"), "people/robin");
        assert_eq!(bare("./journal/2026-10-14.md"), "journal/2026-10-14");
        assert_eq!(bare("M-0003"), "M-0003");
        assert_eq!(bare("Robin"), "Robin");
        assert_eq!(bare("issue#12"), "issue#12");
        assert_eq!(Ref::new("memories/M-21.md").id(), PieceId::parse("M-0021"));
    }
}
