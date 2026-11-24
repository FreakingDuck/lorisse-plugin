//! Report — what `check` finds in the folder, one line per finding.
//!
//! The check re-walks the files: the index holds nothing about problems.
//! Flow: `scan` → problems, values outside the lists, path fields, renderings, links → `Report`.

use std::fmt;
use std::path::PathBuf;

use crate::config::{StoreConfig, list};
use crate::frontmatter::{PATH_KEYS, key};
use crate::ids::PieceId;
use crate::index::{Entry, Scan, proposal_part, proposal_stem};
use crate::kept;
use crate::layer::Layer;
use crate::links;
use crate::piece::Piece;

// =============================================================================
// Interface
// =============================================================================

/// One thing off in the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The file, relative to the root, when there is one.
    pub path: Option<PathBuf>,
    /// The piece, when it has an id.
    pub id: Option<PieceId>,
    /// What is off.
    pub text: String,
}

/// Every finding of one check.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// The findings, in walk order.
    pub findings: Vec<Finding>,
}

impl Report {
    /// Whether nothing was found.
    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }

    /// How many findings.
    pub fn len(&self) -> usize {
        self.findings.len()
    }
}

// =============================================================================
// Implementation
// =============================================================================

impl Finding {
    /// A finding on the file at `rel`.
    pub(crate) fn at(rel: &str, id: Option<PieceId>, text: String) -> Finding {
        Finding {
            path: Some(PathBuf::from(rel)),
            id,
            text,
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.path, &self.id) {
            (Some(path), Some(id)) => write!(f, "{} ({id}): {}", path.display(), self.text),
            (Some(path), None) => write!(f, "{}: {}", path.display(), self.text),
            (None, Some(id)) => write!(f, "{id}: {}", self.text),
            (None, None) => f.write_str(&self.text),
        }
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for finding in &self.findings {
            writeln!(f, "{finding}")?;
        }
        Ok(())
    }
}

/// The report of one walk.
pub(crate) fn report(scan: Scan, config: &StoreConfig) -> Report {
    let mut findings = scan.problems;
    for (entry, piece, text) in &scan.pieces {
        let finding = |text: String| Finding::at(&entry.path_text(), Some(entry.id.clone()), text);
        findings.extend(
            outside_lists(entry.layer, piece, config)
                .into_iter()
                .map(finding),
        );
        for name in PATH_KEYS.iter().filter(|k| piece.get(k).is_some()) {
            findings.push(finding(format!(
                "{name}: a field the path decides; ignored"
            )));
        }
        if piece.render() != *text {
            findings.push(finding("rendering differs".into()));
        }
    }
    for (from, link) in links::dangling_links(&scan.index, &config.archive) {
        let entry = scan.index.get(&from);
        let path = entry.map(Entry::path_text).unwrap_or_default();
        findings.push(Finding::at(
            &path,
            Some(from.clone()),
            dangling_text(&from, &link),
        ));
    }
    for (from, link) in links::retired_links(&scan.index, &config.archive) {
        let path = scan.index.get(&from).map(Entry::path_text);
        let text = format!("{from} links to {link}: a retired piece");
        findings.push(Finding::at(&path.unwrap_or_default(), Some(from), text));
    }
    for rel in &scan.proposals {
        let target = proposal_stem(rel, config)
            .and_then(proposal_part)
            .map(|(id, _)| id);
        if !target
            .is_some_and(|id| scan.index.get(&id).is_some() || scan.index.hidden.contains(&id))
        {
            findings.push(Finding::at(
                rel,
                None,
                "a proposal with no matching piece".into(),
            ));
        }
    }
    for rel in &scan.unplaced {
        findings.push(Finding::at(
            rel,
            None,
            "archived with no layer prefix; skipped".into(),
        ));
    }
    Report { findings }
}

fn dangling_text(from: &PieceId, link: &links::Ref) -> String {
    format!("{from} links to {link}: no such piece")
}

/// `key: value is not in list` for every value outside the configuration's lists.
pub(crate) fn outside_lists(layer: Layer, piece: &Piece, config: &StoreConfig) -> Vec<String> {
    let mut rules = vec![(key::SOURCE, list::SOURCES)];
    match layer {
        Layer::Memory => rules.extend([
            (key::KIND, list::KINDS),
            (key::STATUS, list::MEMORY_STATUSES),
        ]),
        Layer::Task => rules.extend([
            (key::STATUS, list::TASK_STATUSES),
            (key::PRIORITY, list::PRIORITIES),
            (key::CATEGORY, list::CATEGORIES),
        ]),
        Layer::Diary | Layer::Person | Layer::Conversation | Layer::Tag => {}
    }
    let mut out = Vec::new();
    for (name, list_name) in rules {
        let Some(value) = piece.get(name) else {
            continue;
        };
        if value.is_null() || (name == key::KIND && value.as_str() == Some(kept::CONVERSATION)) {
            continue;
        }
        let text = value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| format!("{value:?}"));
        if !config.list(list_name).contains(&text) {
            out.push(format!("{name}: {text} is not in {list_name}"));
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
    use std::path::Path;

    #[test]
    fn a_value_outside_a_list_names_the_key_the_value_and_the_list() {
        let text = "---\nid: T-1\ntitle: x\nstatus: later\npriority: high\nsource: fax\n---\n";
        let piece = Piece::parse(Path::new("tasks/T-1.md"), text).unwrap();
        let found = outside_lists(Layer::Task, &piece, &StoreConfig::default());
        assert_eq!(
            found,
            [
                "source: fax is not in sources",
                "status: later is not in task_statuses"
            ]
        );
        let finding = Finding::at("tasks/T-1.md", PieceId::parse("T-1"), found[0].clone());
        assert_eq!(
            finding.to_string(),
            "tasks/T-1.md (T-0001): source: fax is not in sources"
        );
    }
}
