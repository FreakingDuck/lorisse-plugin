//! Find and search — case-insensitive substring matches, title hits first.
//!
//! `find` reads the index only (titles, then tags); `search` also reads the bodies and the
//! names of the files a piece holds.
//! Flow: text → lowercase → titles → tags → bodies or file names → `Hit`s in id order per group.

use std::fs;
use std::path::Path;

use crate::error::StoreError;
use crate::ids::PieceId;
use crate::index::{Entry, Index, io_error};
use crate::piece::Piece;

// =============================================================================
// Interface
// =============================================================================

/// One search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// The piece.
    pub id: PieceId,
    /// Its title.
    pub title: String,
    /// The first body line that matched; `None` for a title or tag hit.
    pub line: Option<String>,
    /// The name of a file the piece holds that matched, when no body line did.
    pub file: Option<String>,
}

// =============================================================================
// Implementation
// =============================================================================

/// The entries whose title, then whose tags, hold `text`.
pub(crate) fn find<'a>(index: &'a Index, text: &str) -> Vec<&'a Entry> {
    let needle = text.to_lowercase();
    let (titled, rest): (Vec<&Entry>, Vec<&Entry>) = index
        .entries
        .iter()
        .partition(|e| e.title.to_lowercase().contains(&needle));
    let tagged = rest
        .into_iter()
        .filter(|e| e.tags.iter().any(|t| t.to_lowercase().contains(&needle)));
    titled.into_iter().chain(tagged).collect()
}

/// `find`'s hits, then the pieces whose body holds `text`.
pub(crate) fn search(root: &Path, index: &Index, text: &str) -> Result<Vec<Hit>, StoreError> {
    let needle = text.to_lowercase();
    let found = find(index, text);
    let mut hits: Vec<Hit> = found.iter().map(|e| hit(e, None)).collect();
    for entry in index
        .entries
        .iter()
        .filter(|e| !found.iter().any(|f| f.id == e.id))
    {
        let path = root.join(&entry.path);
        let text = fs::read_to_string(&path).map_err(|e| io_error(&path, e))?;
        let Ok(piece) = Piece::parse(&entry.path, &text) else {
            continue;
        };
        let line = piece
            .body()
            .lines()
            .find(|l| l.to_lowercase().contains(&needle));
        if let Some(line) = line {
            hits.push(hit(entry, Some(line.to_string())));
            continue;
        }
        let files = piece.files();
        if let Some(file) = files.iter().find(|f| f.name.to_lowercase().contains(&needle)) {
            hits.push(Hit {
                file: Some(file.name.clone()),
                ..hit(entry, None)
            });
        }
    }
    Ok(hits)
}

fn hit(entry: &Entry, line: Option<String>) -> Hit {
    Hit {
        id: entry.id.clone(),
        title: entry.title.clone(),
        line,
        file: None,
    }
}
