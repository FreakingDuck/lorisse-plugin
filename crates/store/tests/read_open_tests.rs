//! The opening for reading (`Store::open_read`), on temporary copies of the sample: it writes
//! nothing, creates nothing, runs no git, and refuses every writing call.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::NaiveDate;
use lorisse_store::{Access, Filter, Layer, Ref, Source, Store, StoreError};

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
    let name = format!("lorisse_read_{tag}_{}_{nanos}", std::process::id());
    let to = std::env::temp_dir().join(name);
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sample");
    copy_tree(&from, &to);
    Copy(to)
}

/// Every file under `dir` (the `.git` folder included) with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                stack.push(entry.path());
            } else {
                out.insert(entry.path(), fs::read(entry.path()).unwrap());
            }
        }
    }
    out
}

fn ids(store: &Store) -> Vec<String> {
    let all = store.list(&Filter::default());
    all.iter().map(|e| e.id.to_string()).collect()
}

#[test]
fn the_reading_opening_sees_what_the_writing_one_sees_and_writes_nothing() {
    let copy = sample_copy("same");
    let before = snapshot(&copy.0);
    let store = Store::open_read(&copy.0, Access::Full).unwrap();
    assert!(store.is_read_only());
    let filter = Filter {
        layer: Some(Layer::Memory),
        ..Filter::default()
    };
    assert_eq!(store.list(&filter).len(), 7);
    assert_eq!(store.read(&Ref::new("M-0002")).unwrap().title(), "Boiler serviced in March");
    assert!(!store.find("boiler").is_empty());
    assert!(!store.search("tulips").unwrap().is_empty());
    assert_eq!(store.links(&Ref::new("M-0001")).incoming.len(), 3);
    assert_eq!(store.tags().len(), 2);
    assert!(store.repos().iter().any(|r| r.name == "club"));
    assert!(store.unreadable().is_empty());
    assert_eq!(store.next_id("M", None).to_string(), "M-0007");
    assert_eq!(store.next_id("M", Some("club")).to_string(), "C-0002");
    let read_ids = ids(&store);
    assert_eq!(snapshot(&copy.0), before, "no file written, none created");
    let other = sample_copy("same_writing");
    let written = Store::open(&other.0, Access::Full).unwrap();
    assert_eq!(read_ids, ids(&written));
}

#[test]
fn every_writing_call_on_it_is_refused_and_nothing_is_written() {
    let copy = sample_copy("refused");
    let before = snapshot(&copy.0);
    let mut store = Store::open_read(&copy.0, Access::Full).unwrap();
    let day = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
    let refused = [
        store.capture("a text", Source::new("chat")).map(|_| ()),
        store.rebuild_index().map(|_| ()),
        store.tag_define("home", "the house").map(|_| ()),
        store.diary_append(day, "a line").map(|_| ()),
    ];
    for result in refused {
        let err = result.unwrap_err();
        assert!(matches!(err, StoreError::ReadOnly(_)), "{err}");
        assert!(err.to_string().contains("opened for reading"), "{err}");
    }
    assert_eq!(snapshot(&copy.0), before);
}

#[test]
fn in_a_git_folder_it_runs_no_git_and_writes_no_gitignore() {
    let copy = sample_copy("git");
    let git = |args: &[&str]| {
        let out = Command::new("git").arg("-C").arg(&copy.0).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    git(&["-c", "core.autocrlf=false", "add", "-A"]);
    git(&[
        "-c",
        "user.name=sample",
        "-c",
        "user.email=sample@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-q",
        "-m",
        "the sample",
    ]);
    let before = snapshot(&copy.0);
    let store = Store::open_read(&copy.0, Access::Free).unwrap();
    assert!(!ids(&store).contains(&"M-0006".to_string()));
    assert!(store.in_inbox(&Ref::new("M-0005")));
    assert!(!store.in_inbox(&Ref::new("M-0006")));
    let err = store.lorisse_commits(&lorisse_store::Since::All).unwrap_err();
    assert!(matches!(err, StoreError::ReadOnly(_)), "{err}");
    assert!(!copy.0.join(".gitignore").exists());
    assert_eq!(snapshot(&copy.0), before, "the git folder itself untouched");
}

#[test]
fn a_folder_that_is_not_there_is_an_error_and_is_not_made() {
    let copy = sample_copy("absent");
    let absent = copy.0.join("nowhere");
    let err = Store::open_read(&absent, Access::Free).unwrap_err();
    assert!(matches!(err, StoreError::Io(_)), "{err}");
    assert!(!absent.exists());
}
