//! The repository's sample memory folder, opened from a temporary copy: the store writes when it
//! opens, so `sample/` itself is never opened in place.

use std::fs;
use std::path::{Path, PathBuf};

use lorisse_store::{Access, Filter, Layer, Ref, Store, StoreError};

/// A copy of `sample/` in a fresh temporary folder, removed on drop.
struct Copy(PathBuf);

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn sample_copy(tag: &str) -> Copy {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let name = format!("lorisse_sample_{tag}_{}_{nanos}", std::process::id());
    let to = std::env::temp_dir().join(name);
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sample");
    copy_tree(&from, &to);
    Copy(to)
}

fn count(store: &Store, layer: Layer) -> usize {
    let filter = Filter {
        layer: Some(layer),
        ..Filter::default()
    };
    store.list(&filter).len()
}

#[test]
fn the_sample_opens_with_its_counts_and_its_check_is_clean() {
    let copy = sample_copy("full");
    let store = Store::open(&copy.0, Access::Full).unwrap();
    assert_eq!(
        count(&store, Layer::Memory),
        8,
        "four memories, the inbox's, the confidential one, the club's, the office's"
    );
    let club: Vec<String> = store
        .list(&Filter::default())
        .iter()
        .filter(|e| e.repo.as_deref() == Some("club"))
        .map(|e| e.id.to_string())
        .collect();
    assert_eq!(club, ["C-0001"]);
    assert_eq!(count(&store, Layer::Task), 5, "four open tasks and the archived one");
    assert_eq!(count(&store, Layer::Person), 1);
    assert_eq!(count(&store, Layer::Tag), 2);
    let inbox: Vec<String> = store.inbox().iter().map(|e| e.id.to_string()).collect();
    assert_eq!(inbox, ["M-0005"]);
    let flagged = store.flagged().unwrap();
    assert_eq!(flagged.len(), 1);
    assert_eq!(flagged[0].id.to_string(), "M-0003");
    assert_eq!(flagged[0].question, "Is this still planned for spring?");
    assert_eq!(flagged[0].answers, ["yes", "no"]);
    let archived = store.list(&Filter::default());
    let archived: Vec<String> = archived
        .iter()
        .filter(|e| e.archived)
        .map(|e| e.id.to_string())
        .collect();
    assert_eq!(archived, ["T-0005"]);
    assert_eq!(store.read(&Ref::new("M-0006")).unwrap().title(), "The surprise party");
    let report = store.check().unwrap();
    let findings: Vec<String> = report
        .findings
        .iter()
        .map(|f| format!("{:?}: {}", f.path, f.text))
        .collect();
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn an_opening_without_the_confidential_right_never_sees_m_0006() {
    let copy = sample_copy("free");
    let store = Store::open(&copy.0, Access::Free).unwrap();
    let ids: Vec<String> = store
        .list(&Filter::default())
        .iter()
        .map(|e| e.id.to_string())
        .collect();
    assert!(!ids.contains(&"M-0006".to_string()), "{ids:?}");
    assert!(store.find("surprise").is_empty());
    let err = store.read(&Ref::new("M-0006")).unwrap_err();
    assert!(matches!(err, StoreError::Confidential(_)), "{err}");
}
