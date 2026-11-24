//! Place — a piece's repository, slice, layer, confidentiality and archive state, read off its path.
//!
//! The path rule: `[<repo folder>/] [<slice>/] <layer folder>/
//! [archive/] <stem>.md`, plus `inbox/<stem>.md` and `[<repo folder>/]archive/<stem>.md` (layer
//! from the id prefix; a row with its own prefix archives under the layer folder). The first segment is looked up in `repos:`; a row's flag makes its pieces
//! confidential. A field in the file never changes a place. Flow: path → `Place::of` → `Entry`.

use crate::config::StoreConfig;
use crate::ids;
use crate::index::Entry;
use crate::layer::{CONFIDENTIAL, Layer};
use crate::repos;

// =============================================================================
// Implementation
// =============================================================================

/// Where a piece sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Place {
    /// The repository folder; `None` at the root, or, for a new place, the home of its flag.
    pub(crate) repo: Option<String>,
    /// The slice folder, if any.
    pub(crate) slice: Option<String>,
    /// The layer, from the folder (or the id prefix in a root archive).
    pub(crate) layer: Layer,
    /// In a confidential repository.
    pub(crate) confidential: bool,
    /// Under an archive folder.
    pub(crate) archived: bool,
    /// In the inbox.
    pub(crate) inbox: bool,
}

/// The extension of a piece file.
pub(crate) const MD: &str = ".md";

impl Place {
    /// The place of a relative path (forward slashes), or `None` when it is not a piece.
    pub(crate) fn of(rel: &str, config: &StoreConfig) -> Option<Place> {
        let segments: Vec<&str> = rel.split('/').collect();
        let (file, dirs) = segments.split_last()?;
        let stem = file.strip_suffix(MD)?;
        if stem.is_empty() || segments.iter().any(|s| s.starts_with('.')) {
            return None;
        }
        let row = dirs
            .split_first()
            .and_then(|(first, rest)| repos::row_at(config, first).map(|row| (row, rest)));
        match row {
            Some((row, rest)) => place_in(rest, stem, Some(row.folder), row.confidential, config),
            None => place_in(dirs, stem, None, false, config),
        }
    }

    /// The folder of this place, relative to the root (forward slashes).
    pub(crate) fn folder(&self, config: &StoreConfig) -> Option<String> {
        folder_of(self, config)
    }

    /// The repository folder this place lands in: its own, else the home of its flag; `None` at the root.
    pub(crate) fn home<'a>(&'a self, config: &'a StoreConfig) -> Option<&'a str> {
        match &self.repo {
            Some(repo) => Some(repo),
            None if self.confidential => Some(repos::confidential_home(config)),
            None => None,
        }
    }

    /// The place of a new piece of `layer`: at the root, or in the home of confidential pieces.
    pub(crate) fn new(layer: Layer, confidential: bool, slice: Option<String>) -> Place {
        Place {
            repo: None,
            slice,
            layer,
            confidential,
            archived: false,
            inbox: false,
        }
    }

    /// A place beside `entry`: its repository and slice kept while its flag is.
    pub(crate) fn beside(
        entry: &Entry,
        layer: Layer,
        confidential: bool,
        config: &StoreConfig,
    ) -> Place {
        let kept = confidential == entry.confidential;
        let repo = kept.then(|| Place::of(&entry.path_text(), config)?.repo);
        Place {
            repo: repo.flatten(),
            ..Place::new(layer, confidential, entry.slice.clone())
        }
    }
}

/// Whether a path sits behind the gate: in a confidential repository or in the inbox.
pub(crate) fn is_gated(rel: &str, config: &StoreConfig) -> bool {
    let first = rel.split('/').next().unwrap_or("");
    repos::is_confidential(rel, config) || first == config.inbox
}

/// Whether `name` can be a slice folder.
fn is_slice(name: &str, config: &StoreConfig) -> bool {
    let reserved = [CONFIDENTIAL, config.inbox.as_str(), config.archive.as_str()];
    !reserved.contains(&name) && config.layer_of(name).is_none()
}

fn place_in(
    dirs: &[&str],
    stem: &str,
    repo: Option<String>,
    confidential: bool,
    config: &StoreConfig,
) -> Option<Place> {
    let layer = |name: &str| config.layer_of(name);
    let archive = config.archive.as_str();
    let at_root = repo.is_none();
    let place = |slice: Option<&str>, layer: Layer, archived: bool| Place {
        repo: repo.clone(),
        slice: slice.map(str::to_string),
        layer,
        confidential,
        archived,
        inbox: false,
    };
    match dirs {
        [inbox] if at_root && *inbox == config.inbox => Some(Place {
            inbox: true,
            ..place(None, Layer::Memory, false)
        }),
        [dir] if *dir == archive => {
            let prefix = ids::numbered(stem)?.prefix()?.to_string();
            Some(place(None, Layer::from_prefix(&prefix)?, true))
        }
        [dir] => Some(place(None, layer(dir)?, false)),
        [dir, arch] if *arch == archive && layer(dir).is_some() => {
            Some(place(None, layer(dir)?, true))
        }
        [slice, dir] if is_slice(slice, config) => Some(place(Some(slice), layer(dir)?, false)),
        [slice, dir, arch] if *arch == archive && is_slice(slice, config) => {
            Some(place(Some(slice), layer(dir)?, true))
        }
        _ => None,
    }
}

fn folder_of(place: &Place, config: &StoreConfig) -> Option<String> {
    if place.inbox {
        return Some(config.inbox.clone());
    }
    let mut parts: Vec<&str> = Vec::new();
    let home = place.home(config);
    parts.extend(home);
    let numbered = home.and_then(|f| repos::prefix_at(config, f)).is_some();
    match (&place.slice, place.archived) {
        (None, true) if !numbered => parts.push(&config.archive),
        (slice, archived) => {
            if let Some(slice) = slice {
                parts.push(slice);
            }
            parts.push(config.folder_of(place.layer)?);
            if archived {
                parts.push(&config.archive);
            }
        }
    }
    Some(parts.join("/"))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn of(rel: &str) -> Option<Place> {
        Place::of(rel, &StoreConfig::default())
    }

    #[test]
    fn the_path_rule_reads_every_folder_of_today_and_tomorrow() {
        let inbox = of("inbox/M-0007.md").unwrap();
        assert!(inbox.inbox && inbox.layer == Layer::Memory);
        let archived = of("archive/T-0002.md").unwrap();
        assert!(archived.archived && archived.layer == Layer::Task);
        let person = of("confidential/people/robin.md").unwrap();
        assert!(person.confidential && person.layer == Layer::Person);
        let sliced = of("confidential/work/tasks/archive/T-1.md").unwrap();
        assert_eq!(sliced.slice.as_deref(), Some("work"));
        assert!(sliced.archived && sliced.confidential);
        for skipped in [
            "CAKEBOARD.md",
            "worktime/2026-10.md",
            "archive/notes.md",
            "confidential/inbox/M-1.md",
            ".git/memories/M-1.md",
            "memories/M-1.txt",
            "work/other/M-1.md",
        ] {
            assert_eq!(of(skipped), None, "{skipped}");
        }
    }

    #[test]
    fn a_place_gives_back_its_folder() {
        let config = StoreConfig::default();
        for rel in [
            "inbox/M-1.md",
            "memories/M-1.md",
            "archive/T-1.md",
            "confidential/journal/2026-10-14.md",
            "work/tasks/archive/T-1.md",
        ] {
            let place = of(rel).unwrap();
            let folder = place.folder(&config).unwrap();
            assert_eq!(format!("{folder}/{}", rel.rsplit('/').next().unwrap()), rel);
        }
    }

    #[test]
    fn a_listed_repository_is_the_first_segment_and_its_flag_the_rule() {
        let yaml = "repos:\n  - { name: acme, folder: acme, role: work, confidential: true }\n  - { name: famille, folder: famille }\n";
        let config: StoreConfig = serde_yaml_ng::from_str(yaml).unwrap();
        let work = Place::of("acme/work/tasks/T-1.md", &config).unwrap();
        assert_eq!(
            (work.repo.as_deref(), work.slice.as_deref()),
            (Some("acme"), Some("work"))
        );
        assert!(work.confidential && is_gated("acme/work/tasks/T-1.md", &config));
        let home = Place::of("famille/memories/M-1.md", &config).unwrap();
        assert!(!home.confidential && !is_gated("famille/memories/M-1.md", &config));
        assert_eq!(Place::of("acme/inbox/M-1.md", &config), None);
        let new = Place::new(Layer::Memory, true, Some("work".into()));
        assert_eq!(new.folder(&config).as_deref(), Some("acme/work/memories"));
        let alias = Place::of("confidential/memories/M-1.md", &config).unwrap();
        assert!(alias.confidential && alias.repo.as_deref() == Some(CONFIDENTIAL));
    }
}
