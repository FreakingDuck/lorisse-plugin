//! The scenarios of VALIDATION.md § 3, over a copy of the fixtures in a temporary folder.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use lorisse_store::{
    Access, Action, Change, Diff, Fields, Filter, Flags, IDS_FILE, Layer, Piece, PieceId, Ref,
    Source, Speaker, Store, StoreError, Value,
};

// =============================================================================
// Helpers
// =============================================================================

/// A unique, empty folder under the temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let name = format!("lorisse_store_{tag}_{}_{nanos}", std::process::id());
        let path = std::env::temp_dir().join(name);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
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

/// A temporary copy of `fixtures/<name>`.
fn fixture(name: &str, tag: &str) -> TempDir {
    let tmp = TempDir::new(tag);
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    copy_tree(&from, &tmp.0);
    tmp
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

/// Every file under `root` with its bytes.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut all = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                all.insert(path.clone(), fs::read(&path).unwrap());
            }
        }
    }
    all
}

/// A memory in the house style.
fn memory(id: &str, title: &str, rest: &str) -> String {
    format!(
        "---\nid: {id}\ntitle: {title}\nkind: idea\ntags: []\nsource: chat\n\
         captured: 2026-10-13T22:30:00+02:00\nupdated: 2026-10-13\n{rest}---\nBody of {id}.\n"
    )
}

fn id(text: &str) -> PieceId {
    PieceId::parse(text).unwrap()
}

fn r(text: &str) -> Ref {
    Ref::new(text)
}

fn ids(entries: &[&lorisse_store::Entry]) -> Vec<String> {
    entries.iter().map(|e| e.id.to_string()).collect()
}

fn fields(pairs: &[(&str, &str)]) -> Fields {
    let mut map = Fields::new();
    for (k, v) in pairs {
        map.insert(Value::from(*k), Value::from(*v));
    }
    map
}

fn edit(target: &str, pairs: &[(&str, &str)]) -> Action {
    Action::Edit {
        id: id(target),
        set: fields(pairs),
        unset: Vec::new(),
        body: None,
    }
}

fn sort(target: &str, layer: Layer, pairs: &[(&str, &str)]) -> Action {
    Action::Sort {
        id: id(target),
        layer,
        confidential: false,
        slice: None,
        set: fields(pairs),
        sorted_by: Some("owner".into()),
    }
}

fn piece(root: &Path, rel: &str) -> Piece {
    Piece::parse(Path::new(rel), &read(root, rel)).unwrap()
}

// =============================================================================
// C-4: the piece
// =============================================================================

#[test]
fn every_fixture_round_trips_byte_for_byte() {
    let tmp = fixture("valid", "roundtrip");
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let entries = store.list(&Filter::default());
    assert_eq!(entries.len(), 10, "{:?}", ids(&entries));
    for entry in entries {
        let text = fs::read_to_string(tmp.0.join(&entry.path)).unwrap();
        let parsed = Piece::parse(&entry.path, &text).unwrap();
        assert_eq!(parsed.render(), text, "{}", entry.path.display());
        let again = Piece::parse(&entry.path, &parsed.render()).unwrap();
        assert_eq!(again, parsed);
        assert_eq!(store.read(&Ref::from(entry.id.clone())).unwrap(), parsed);
    }
}

#[test]
fn a_broken_file_names_its_path_and_reason_and_the_rest_still_opens() {
    let tmp = fixture("broken", "broken");
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let listed = ids(&store.list(&Filter::default()));
    assert_eq!(listed, ["M-0013", "M-0014"]);
    let report = store.check().unwrap();
    let lines: Vec<String> = report.findings.iter().map(|f| f.to_string()).collect();
    let has = |path: &str, word: &str| lines.iter().any(|l| l.contains(path) && l.contains(word));
    assert!(has("memories/M-0010.md", "fence"), "{lines:#?}");
    assert!(has("memories/M-0011.md", "updated"), "{lines:#?}");
    assert!(has("memories/M-0012.md", "inbox/M-0012.md"), "{lines:#?}");
    assert!(has("inbox/M-0012.md", "memories/M-0012.md"), "{lines:#?}");
    assert!(has("memories/M-0013.md", "kind"), "{lines:#?}");
    assert!(has("memories/M-0013.md", "slice"), "{lines:#?}");
    assert!(!lines.iter().any(|l| l.contains("M-0014")), "{lines:#?}");
}

#[test]
fn slice_layer_and_confidential_come_from_the_path_alone() {
    let tmp = TempDir::new("place");
    let text = "---\ntitle: Same text\nkind: idea\nstatus: sorted\nslice: other\n---\nBody.\n";
    for rel in [
        "memories/a.md",
        "confidential/memories/b.md",
        "work/memories/c.md",
        "confidential/work/memories/d.md",
    ] {
        write(&tmp.0, rel, text);
    }
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let entries = store.list(&Filter::default());
    let places: Vec<(String, Option<&str>, bool, Layer)> = entries
        .iter()
        .map(|e| {
            let path = e.path.to_string_lossy().into_owned();
            (path, e.slice.as_deref(), e.confidential, e.layer)
        })
        .collect();
    assert_eq!(
        places,
        [
            ("memories/a.md".into(), None, false, Layer::Memory),
            (
                "confidential/memories/b.md".into(),
                None,
                true,
                Layer::Memory
            ),
            (
                "work/memories/c.md".into(),
                Some("work"),
                false,
                Layer::Memory
            ),
            (
                "confidential/work/memories/d.md".into(),
                Some("work"),
                true,
                Layer::Memory
            ),
        ]
    );
    let work = Filter {
        slice: Some("work".into()),
        ..Filter::default()
    };
    assert_eq!(store.list(&work).len(), 2);
    let report = store.check().unwrap();
    let slice_findings = report
        .findings
        .iter()
        .filter(|f| f.to_string().contains("slice"))
        .count();
    assert_eq!(slice_findings, 4, "{report}");
}

#[test]
fn a_value_outside_the_lists_is_reported_and_refused() {
    let tmp = fixture("valid", "lists");
    write(
        &tmp.0,
        "memories/M-0030.md",
        &memory("M-0030", "Odd kind", "status: sorted\nlinks: []\n").replace("idea", "mood"),
    );
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let report = store.check().unwrap().to_string();
    assert!(report.contains("M-0030") && report.contains("kind") && report.contains("mood"));
    let diff = store.propose(edit("M-0001", &[("kind", "mood")])).unwrap();
    let before = snapshot(&tmp.0);
    let err = store.accept(diff).unwrap_err();
    assert!(matches!(err, StoreError::Invalid(_)), "{err}");
    assert!(err.to_string().contains("mood"), "{err}");
    assert_eq!(snapshot(&tmp.0), before);
}

#[test]
fn unknown_keys_survive_a_rewrite() {
    let tmp = fixture("valid", "unknown");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let diff = store
        .propose(edit("M-0004", &[("title", "A new title")]))
        .unwrap();
    store.accept(diff).unwrap();
    let text = read(&tmp.0, "memories/M-0004.md");
    let head: Vec<&str> = text.lines().collect();
    let close = head
        .iter()
        .skip(1)
        .position(|l| l.trim_end() == "---")
        .unwrap()
        + 1;
    assert_eq!(head[close - 1].trim_end(), "mood: calm", "{text}");
    assert_eq!(head[close - 2].trim_end(), "links: [T-0001]", "{text}");
    assert!(text.contains("title: A new title"));
    assert_eq!(piece(&tmp.0, "memories/M-0004.md").render(), text);
}

// =============================================================================
// C-5: ids
// =============================================================================

#[test]
fn the_next_id_is_the_largest_number_plus_one_over_every_folder() {
    let tmp = TempDir::new("next");
    write(
        &tmp.0,
        "confidential/memories/M-0003.md",
        &memory("M-0003", "Secret", "status: sorted\nlinks: []\n"),
    );
    write(
        &tmp.0,
        "inbox/M-0002.md",
        &memory("M-0002", "Raw", "status: raw\nlinks: []\n"),
    );
    write(
        &tmp.0,
        "memories/M-0001.md",
        &memory("M-0001", "Old", "status: retired\nlinks: []\n"),
    );
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    assert_eq!(store.next_id("M", None), id("M-0004"));
    assert_eq!(store.next_id("T", None), id("T-0001"));
}

#[test]
fn an_id_is_compared_by_number_and_grows_past_9999() {
    let one = id("M-01");
    assert_eq!(
        one,
        PieceId::Numbered {
            prefix: "M".into(),
            n: 1
        }
    );
    assert_eq!(one.to_string(), "M-0001");
    assert!(id("M-9999") < id("M-10000"));
    assert!(id("M-0002") < id("people/robin"));
    let tmp = TempDir::new("grow");
    write(
        &tmp.0,
        "memories/M-10000.md",
        &memory("M-10000", "Big", "status: sorted\nlinks: []\n"),
    );
    write(
        &tmp.0,
        "memories/M-9999.md",
        &memory("M-9999", "Small", "status: sorted\nlinks: []\n"),
    );
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    assert_eq!(ids(&store.list(&Filter::default())), ["M-9999", "M-10000"]);
    assert_eq!(store.next_id("M", None).to_string(), "M-10001");
    let tmp2 = TempDir::new("grow2");
    write(
        &tmp2.0,
        "memories/M-9999.md",
        &memory("M-9999", "Small", "status: sorted\nlinks: []\n"),
    );
    let store = Store::open(&tmp2.0, Access::Full).unwrap();
    assert_eq!(store.next_id("M", None).to_string(), "M-10000");
}

#[test]
fn a_retired_or_archived_id_is_never_given_again() {
    let tmp = TempDir::new("retired");
    write(
        &tmp.0,
        "memories/M-0001.md",
        &memory("M-0001", "Gone", "status: retired\nlinks: []\n"),
    );
    write(
        &tmp.0,
        "archive/T-0004.md",
        "---\nid: T-0004\ntitle: Done\nstatus: done\nupdated: 2026-10-01\nlinks: []\n---\n",
    );
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    assert_eq!(store.next_id("M", None), id("M-0002"));
    assert_eq!(store.next_id("T", None), id("T-0005"));
    let piece = store.capture("new", Source::new("console")).unwrap();
    assert_eq!(piece.id(), Some(id("M-0002")));
}

#[test]
fn an_id_whose_file_is_gone_is_never_given_again_without_git() {
    let tmp = TempDir::new("freed");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    assert!(!tmp.0.join(IDS_FILE).exists(), "nothing numbered yet");
    let piece = store.capture("first", Source::new("console")).unwrap();
    assert_eq!(piece.id(), Some(id("M-0001")));
    fs::remove_file(tmp.0.join("inbox/M-0001.md")).unwrap();
    let mut store = Store::open(&tmp.0, Access::Free).unwrap();
    assert_eq!(store.next_id("M", None), id("M-0002"));
    let piece = store.capture("second", Source::new("console")).unwrap();
    assert_eq!(piece.id(), Some(id("M-0002")));
    assert_eq!(read(&tmp.0, IDS_FILE), "{\n  \"M\": 2\n}\n");
}

// =============================================================================
// C-6: capture
// =============================================================================

#[test]
fn a_capture_lands_raw_with_its_text_untouched() {
    let tmp = TempDir::new("capture");
    assert!(!tmp.0.join("inbox").exists());
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let text = "  first line  \r\nsecond: a colon\r\n---\nthird  ";
    let piece = store.capture(text, Source::new("console")).unwrap();
    assert_eq!(piece.id(), Some(id("M-0001")));
    assert_eq!(piece.title(), "first line");
    assert_eq!(piece.status(), Some("raw"));
    assert_eq!(piece.body(), format!("{text}\n"));
    let file = read(&tmp.0, "inbox/M-0001.md");
    assert!(file.ends_with(&format!("---\n{text}\n")), "{file}");
    assert!(file.contains("\nsource: console\n") && file.contains("\nstatus: raw\n"));
    let back = store.read(&r("M-0001")).unwrap();
    assert_eq!(back.body(), piece.body());
    assert_eq!(ids(&store.inbox()), ["M-0001"]);
    let err = store.capture("x", Source::new("fax")).unwrap_err();
    assert!(matches!(err, StoreError::Invalid(_)), "{err}");
}

#[test]
fn a_sort_into_another_layer_retires_the_capture_and_links_both() {
    let tmp = fixture("valid", "sortlayer");
    write(
        &tmp.0,
        "inbox/M-0007.sort.yaml",
        "action: sort\nid: M-0007\nlayer: task\nset:\n  title: Call the bank\n  \
         priority: high\n  category: admin\nsorted_by: sorter\n",
    );
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let mut proposals = store.proposals().unwrap();
    assert_eq!(proposals.len(), 1);
    let (target, action) = proposals.remove(0);
    assert_eq!(target, id("M-0007"));
    let body = piece(&tmp.0, "inbox/M-0007.md").body().to_string();
    let diff = store.propose(action).unwrap();
    let written = store.accept(diff).unwrap();
    assert!(written.contains(&id("T-0003")) && written.contains(&id("M-0007")));
    let task = piece(&tmp.0, "tasks/T-0003.md");
    assert_eq!(task.get("capture"), Some(&Value::from("M-0007")));
    assert_eq!(task.status(), Some("todo"));
    assert_eq!(task.title(), "Call the bank");
    assert_eq!(task.body(), body);
    let retired = piece(&tmp.0, "memories/M-0007.md");
    assert_eq!(retired.status(), Some("retired"));
    assert_eq!(retired.links()[0], r("T-0003"));
    assert_eq!(retired.get("sorted_by"), Some(&Value::from("sorter")));
    assert!(!tmp.0.join("inbox/M-0007.md").exists());
    assert!(!tmp.0.join("inbox/M-0007.sort.yaml").exists());
    assert!(store.inbox().is_empty());
    assert_eq!(store.links(&r("T-0003")).incoming, [id("M-0007")]);
}

// =============================================================================
// C-7: propose and accept
// =============================================================================

#[test]
fn propose_writes_nothing() {
    let tmp = fixture("valid", "propose");
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let before = snapshot(&tmp.0);
    let actions = vec![
        sort("M-0007", Layer::Memory, &[("kind", "fact")]),
        sort("M-0007", Layer::Task, &[("title", "Bank")]),
        edit("M-0001", &[("title", "Other")]),
        Action::Link {
            from: id("M-0001"),
            to: vec![r("T-0001")],
        },
        Action::Dedupe {
            keep: id("M-0001"),
            drop: id("M-0002"),
        },
        Action::Retire {
            id: id("M-0004"),
            behind: None,
        },
    ];
    for action in actions {
        let diff = store.propose(action).unwrap();
        assert!(!diff.changes.is_empty());
    }
    assert_eq!(snapshot(&tmp.0), before);
}

#[test]
fn an_accepted_diff_writes_exactly_its_after_text() {
    let tmp = fixture("valid", "accept");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let action = sort(
        "M-0007",
        Layer::Memory,
        &[("title", "Call the bank"), ("kind", "need")],
    );
    let diff = store.propose(action).unwrap();
    let lines = diff.lines();
    assert!(lines.contains(&"move inbox/M-0007.md → memories/M-0007.md".to_string()));
    assert!(lines.contains(&"+status: sorted".to_string()), "{lines:#?}");
    assert!(lines.contains(&"-status: raw".to_string()), "{lines:#?}");
    let changes: Vec<Change> = diff.changes.clone();
    assert_eq!(store.accept(diff).unwrap(), [id("M-0007")]);
    for change in &changes {
        let after = change.after.as_deref().unwrap();
        assert_eq!(fs::read_to_string(tmp.0.join(&change.path)).unwrap(), after);
        if let Some(from) = &change.from {
            assert!(!tmp.0.join(from).exists());
        }
    }
    let index = read(&tmp.0, "index.json");
    assert!(index.contains("memories/M-0007.md") && !index.contains("inbox/M-0007.md"));
    let leftovers = snapshot(&tmp.0)
        .keys()
        .filter(|p| p.extension().is_some_and(|e| e == "tmp"))
        .count();
    assert_eq!(leftovers, 0);
    let moved = store.read(&r("M-0007")).unwrap();
    assert_eq!(moved.status(), Some("sorted"));
    assert_eq!(moved.kind(), Some("need"));
    assert!(moved.get("sorted").is_some());
}

#[test]
fn a_file_changed_since_the_diff_is_refused() {
    let tmp = fixture("valid", "changed");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let diff = store.propose(edit("M-0001", &[("title", "New")])).unwrap();
    let path = tmp.0.join("memories/M-0001.md");
    let edited = fs::read_to_string(&path).unwrap() + "A line added by hand.\n";
    fs::write(&path, edited).unwrap();
    let before = snapshot(&tmp.0);
    let err = store.accept(diff).unwrap_err();
    assert!(matches!(err, StoreError::Changed(_)), "{err}");
    assert!(err.to_string().contains("changed since"), "{err}");
    assert_eq!(snapshot(&tmp.0), before);
}

#[test]
fn a_create_never_overwrites() {
    let tmp = fixture("valid", "create");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let diff = store
        .propose(sort("M-0007", Layer::Task, &[("title", "Bank")]))
        .unwrap();
    assert!(diff.changes.iter().any(|c| c.before.is_none()));
    write(&tmp.0, "tasks/T-0003.md", "written meanwhile\n");
    let before = snapshot(&tmp.0);
    let err = store.accept(diff).unwrap_err();
    assert!(matches!(err, StoreError::Exists(_)), "{err}");
    assert_eq!(snapshot(&tmp.0), before);
}

#[test]
fn a_dedupe_keeps_the_older_id_and_retires_the_newer() {
    let tmp = fixture("valid", "dedupe");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let reverse = Action::Dedupe {
        keep: id("M-0002"),
        drop: id("M-0001"),
    };
    let err = store.propose(reverse).unwrap_err();
    assert!(matches!(err, StoreError::Invalid(_)), "{err}");
    let diff = store
        .propose(Action::Dedupe {
            keep: id("M-0001"),
            drop: id("M-0002"),
        })
        .unwrap();
    store.accept(diff).unwrap();
    let kept = piece(&tmp.0, "memories/M-0001.md");
    assert_eq!(kept.tags(), ["brief", "voice", "work"]);
    assert_eq!(kept.links(), [r("people/robin")]);
    let dropped = piece(&tmp.0, "memories/M-0002.md");
    assert_eq!(dropped.status(), Some("retired"));
    assert_eq!(dropped.links()[0], r("M-0001"));
}

#[test]
fn a_dedupe_rewrites_every_link_to_the_dropped_piece_and_the_check_sees_none_left() {
    let tmp = fixture("valid", "relink");
    write(
        &tmp.0,
        "memories/M-0005.md",
        &memory(
            "M-0005",
            "Points at two",
            "status: sorted\nlinks: [M-0002, M-0001, memories/M-0002.md]\n",
        ),
    );
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let diff = store
        .propose(Action::Dedupe {
            keep: id("M-0001"),
            drop: id("M-0002"),
        })
        .unwrap();
    assert_eq!(diff.changes.len(), 3, "{:?}", diff.lines());
    store.accept(diff).unwrap();
    assert_eq!(piece(&tmp.0, "memories/M-0005.md").links(), [r("M-0001")]);
    assert!(
        store
            .check()
            .unwrap()
            .findings
            .iter()
            .all(|f| !f.text.contains("retired"))
    );
    let retire = store
        .propose(Action::Retire {
            id: id("M-0001"),
            behind: None,
        })
        .unwrap();
    store.accept(retire).unwrap();
    let report = store.check().unwrap().to_string();
    assert!(
        report.contains("M-0005 links to M-0001: a retired piece"),
        "{report}"
    );
    assert!(report.contains("T-0001 links to M-0001"), "{report}");
    assert!(
        !report.contains("M-0002 links to"),
        "a retired piece reports nothing: {report}"
    );
}

#[test]
fn a_dedupe_of_two_tasks_archives_the_newer_retired_and_relinks_its_pieces() {
    let tmp = fixture("valid", "dedupetasks");
    let text = "---\nid: T-0003\ntitle: Write the store again\nstatus: todo\ntags: [store]\n\
        links: [M-0002, T-0001]\n---\nSame thing.\n";
    write(&tmp.0, "tasks/T-0003.md", text);
    write(
        &tmp.0,
        "memories/M-0005.md",
        &memory(
            "M-0005",
            "About the task",
            "status: sorted\nlinks: [T-0003]\n",
        ),
    );
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let newer = store.propose(Action::Dedupe {
        keep: id("T-0003"),
        drop: id("T-0001"),
    });
    assert!(matches!(newer, Err(StoreError::Invalid(_))), "{newer:?}");
    let mixed = store.propose(Action::Dedupe {
        keep: id("M-0001"),
        drop: id("T-0003"),
    });
    assert!(matches!(mixed, Err(StoreError::Invalid(_))), "{mixed:?}");
    let diff = store
        .propose(Action::Dedupe {
            keep: id("T-0001"),
            drop: id("T-0003"),
        })
        .unwrap();
    store.accept(diff).unwrap();
    let kept = piece(&tmp.0, "tasks/T-0001.md");
    assert_eq!(kept.tags(), ["store"]);
    assert_eq!(kept.links(), [r("M-0001"), r("M-0002")]);
    assert!(!tmp.0.join("tasks/T-0003.md").exists());
    let dropped = piece(&tmp.0, "archive/T-0003.md");
    assert_eq!(dropped.status(), Some("retired"));
    assert_eq!(dropped.links(), [r("T-0001"), r("M-0002")]);
    assert_eq!(piece(&tmp.0, "memories/M-0005.md").links(), [r("T-0001")]);
    assert!(
        store.check().unwrap().is_empty(),
        "{}",
        store.check().unwrap()
    );
}

#[test]
fn a_dedupe_of_two_people_keeps_the_one_named_and_relinks_by_name_too() {
    let tmp = fixture("valid", "dedupepeople");
    write(
        &tmp.0,
        "memories/M-0005.md",
        &memory(
            "M-0005",
            "Budget talk",
            "status: sorted\nlinks: [Noa, people/robin]\n",
        ),
    );
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let diff = store
        .propose(Action::Dedupe {
            keep: id("people/robin"),
            drop: id("people/noa"),
        })
        .unwrap();
    store.accept(diff).unwrap();
    let kept = piece(&tmp.0, "confidential/people/robin.md");
    assert_eq!(kept.links(), [r("M-0003")]);
    assert!(kept.get("tags").is_none());
    let dropped = piece(&tmp.0, "confidential/people/noa.md");
    assert_eq!(dropped.status(), Some("retired"));
    assert_eq!(dropped.links(), [r("people/robin")]);
    assert_eq!(
        piece(&tmp.0, "memories/M-0005.md").links(),
        [r("people/robin")]
    );
    let diary = read(&tmp.0, "confidential/journal/2026-10-14.md");
    assert!(diary.contains("links: [M-0003, people/robin]"), "{diary}");
    let diary = store.propose(Action::Dedupe {
        keep: id("people/robin"),
        drop: id("journal/2026-10-14"),
    });
    assert!(matches!(diary, Err(StoreError::Invalid(_))), "{diary:?}");
}

#[test]
fn an_archive_closes_a_task_not_done_then_moves_it_and_a_done_one_only_moves() {
    let tmp = fixture("valid", "archive");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let memory = store.propose(Action::Archive { id: id("M-0001") });
    assert!(matches!(memory, Err(StoreError::Invalid(_))), "{memory:?}");
    let again = store.propose(Action::Archive { id: id("T-0002") });
    assert!(matches!(again, Err(StoreError::Invalid(_))), "{again:?}");
    let diff = store.propose(Action::Archive { id: id("T-0001") }).unwrap();
    store.accept(diff).unwrap();
    assert!(!tmp.0.join("tasks/T-0001.md").exists());
    let archived = piece(&tmp.0, "archive/T-0001.md");
    assert_eq!(archived.id(), Some(id("T-0001")));
    assert_eq!(archived.status(), Some("done"));
    let today = chrono::Local::now().date_naive();
    let want = format!("## Next action\n\n## Notes / history\n- Done {today}: Write the store\n");
    assert!(archived.body().ends_with(&want), "{}", archived.body());
    let entry = store.list(&Filter::default());
    let entry = entry.iter().find(|e| e.id == id("T-0001")).unwrap();
    assert!(entry.archived && entry.layer == Layer::Task);
    let text = "---\nid: T-0003\ntitle: Done before\nstatus: done\n---\nBody.\n";
    write(&tmp.0, "tasks/T-0003.md", text);
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let diff = store.propose(Action::Archive { id: id("T-0003") }).unwrap();
    store.accept(diff).unwrap();
    let moved = piece(&tmp.0, "archive/T-0003.md");
    assert_eq!((moved.status(), moved.body()), (Some("done"), "Body.\n"));
}

// =============================================================================
// C-8: index, links, search
// =============================================================================

#[test]
fn the_index_rebuilds_from_the_files() {
    let tmp = fixture("valid", "index");
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let first: Vec<lorisse_store::Entry> = store
        .list(&Filter::default())
        .into_iter()
        .cloned()
        .collect();
    let json = tmp.0.join("index.json");
    assert!(json.is_file());
    fs::remove_file(&json).unwrap();
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let again: Vec<lorisse_store::Entry> = store
        .list(&Filter::default())
        .into_iter()
        .cloned()
        .collect();
    assert_eq!(again, first);
    assert!(json.is_file());
    assert_eq!(store.rebuild_index().unwrap(), 10);
    let parsed: Vec<lorisse_store::Entry> = serde_json::from_str(&read(&tmp.0, "index.json")).unwrap();
    assert_eq!(parsed, first);
}

#[test]
fn a_dangling_link_is_found_with_both_ends() {
    let tmp = TempDir::new("dangling");
    write(
        &tmp.0,
        "memories/M-0001.md",
        &memory(
            "M-0001",
            "Lonely",
            "status: sorted\nlinks: [M-9999, people/nobody]\n",
        ),
    );
    write(
        &tmp.0,
        "people/somebody.md",
        "---\nname: Somebody\nkind: fact\nlinks: [M-0001]\n---\n",
    );
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    assert_eq!(
        store.dangling(),
        [
            (id("M-0001"), r("M-9999")),
            (id("M-0001"), r("people/nobody"))
        ]
    );
    let report = store.check().unwrap().to_string();
    assert!(
        report.contains("M-0001") && report.contains("M-9999"),
        "{report}"
    );
    assert!(report.contains("people/nobody"), "{report}");
    assert_eq!(store.links(&r("people/somebody")).out, [r("M-0001")]);
}

#[test]
fn links_are_known_both_ways() {
    let tmp = fixture("valid", "links");
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let one = store.links(&r("M-0001"));
    assert_eq!(one.incoming, [id("M-0002"), id("T-0001")]);
    let two = store.links(&r("M-0002"));
    assert_eq!(two.out, [r("M-0001"), r("people/robin")]);
    let robin = store.links(&r("people/robin"));
    assert_eq!(robin.incoming, [id("M-0002"), id("journal/2026-10-14")]);
    assert_eq!(robin.out, [r("M-0003"), r("people/noa")]);
    assert!(store.dangling().is_empty());
}

#[test]
fn search_is_case_insensitive_substring_with_titles_first() {
    let tmp = TempDir::new("search");
    let plain = "status: sorted\nlinks: []\n";
    write(
        &tmp.0,
        "memories/M-0004.md",
        &memory("M-0004", "Daily BRIEF", plain),
    );
    write(
        &tmp.0,
        "memories/M-0001.md",
        &memory("M-0001", "A brief note", plain),
    );
    let tagged = memory("M-0002", "Other", plain).replace("tags: []", "tags: [Brief]");
    write(&tmp.0, "memories/M-0002.md", &tagged);
    let body =
        memory("M-0003", "Third", plain).replace("Body of M-0003.", "line one\nthe bRiEf here");
    write(&tmp.0, "memories/M-0003.md", &body);
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    assert_eq!(ids(&store.find("brief")), ["M-0001", "M-0004", "M-0002"]);
    let hits = store.search("BRIEF").unwrap();
    let order: Vec<String> = hits.iter().map(|h| h.id.to_string()).collect();
    assert_eq!(order, ["M-0001", "M-0004", "M-0002", "M-0003"]);
    assert_eq!(hits[3].line.as_deref(), Some("the bRiEf here"));
    assert_eq!(hits[0].title, "A brief note");
    assert!(hits[0].line.is_none());
}

// =============================================================================
// C-9: the gate
// =============================================================================

#[test]
fn a_free_store_never_sees_confidential_or_inbox() {
    let tmp = fixture("valid", "free");
    let mut store = Store::open(&tmp.0, Access::Free).unwrap();
    let listed = ids(&store.list(&Filter::default()));
    assert_eq!(listed, ["M-0001", "M-0002", "M-0004", "T-0001", "T-0002"]);
    assert!(store.inbox().is_empty());
    assert!(store.find("").iter().all(|e| !e.confidential));
    assert!(
        store
            .search("Robin")
            .unwrap()
            .iter()
            .all(|h| h.id == id("M-0002"))
    );
    assert!(store.search("budget").unwrap().is_empty());
    assert!(store.dangling().is_empty());
    assert!(store.proposals().unwrap().is_empty());
    assert_eq!(
        store.links(&r("M-0002")).out,
        [r("M-0001"), r("people/robin")]
    );
    assert!(store.links(&r("people/robin")).out.is_empty());
    for hidden in ["M-0003", "M-0007", "people/robin", "journal/2026-10-14"] {
        let err = store.read(&r(hidden)).unwrap_err();
        assert!(matches!(err, StoreError::Confidential(_)), "{err}");
        assert_eq!(err.to_string(), format!("confidential: {hidden}"));
    }
    let sort = store.propose(sort("M-0007", Layer::Memory, &[]));
    assert!(matches!(sort, Err(StoreError::Confidential(_))), "{sort:?}");
    let mut moved = sort_action("M-0001");
    if let Action::Sort { confidential, .. } = &mut moved {
        *confidential = true;
    }
    let up = store.propose(moved);
    assert!(matches!(up, Err(StoreError::Confidential(_))), "{up:?}");
    let forged = Diff {
        action: Action::Retire {
            id: id("M-0003"),
            behind: None,
        },
        made_at: chrono::Local::now().fixed_offset(),
        changes: vec![Change {
            path: PathBuf::from("confidential/memories/M-0003.md"),
            from: None,
            before: Some(read(&tmp.0, "confidential/memories/M-0003.md")),
            after: Some("---\nid: M-0003\n---\n".into()),
        }],
        dropped: Vec::new(),
    };
    let before = snapshot(&tmp.0);
    let err = store.accept(forged).unwrap_err();
    assert!(matches!(err, StoreError::Confidential(_)), "{err}");
    assert_eq!(snapshot(&tmp.0), before);
    let index = read(&tmp.0, "index.json");
    for secret in ["confidential/", "M-0003", "M-0007", "Noa", "inbox/"] {
        assert!(!index.contains(secret), "{secret} in {index}");
    }
}

fn sort_action(target: &str) -> Action {
    sort(target, Layer::Memory, &[])
}

#[test]
fn a_free_store_still_computes_the_next_id() {
    let tmp = fixture("valid", "freenext");
    write(
        &tmp.0,
        "confidential/memories/M-0020.md",
        "not even a piece\n",
    );
    let mut store = Store::open(&tmp.0, Access::Free).unwrap();
    assert_eq!(store.next_id("M", None), id("M-0021"));
    assert_eq!(store.next_id("T", None), id("T-0003"));
    let captured = store
        .capture("from a free model", Source::new("agent"))
        .unwrap();
    assert_eq!(captured.id(), Some(id("M-0021")));
    assert!(tmp.0.join("inbox/M-0021.md").is_file());
    assert!(store.inbox().is_empty());
}

// =============================================================================
// C-10: configuration
// =============================================================================

#[test]
fn the_default_config_reads_todays_folder() {
    let tmp = fixture("valid", "default");
    assert!(!tmp.0.join("config.yaml").exists());
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    for layer in [Layer::Memory, Layer::Task, Layer::Diary, Layer::Person] {
        let filter = Filter {
            layer: Some(layer),
            ..Filter::default()
        };
        assert!(!store.list(&filter).is_empty(), "{layer}");
    }
    assert!(store.list(&Filter::default()).iter().any(|e| e.archived));
    let confidential = Filter {
        confidential: Some(true),
        ..Filter::default()
    };
    assert_eq!(store.list(&confidential).len(), 4);
    let report = store.check().unwrap();
    assert!(report.is_empty(), "{report}");
}

#[test]
fn an_unknown_config_key_is_refused_with_its_name() {
    let tmp = TempDir::new("badconfig");
    write(&tmp.0, "config.yaml", "kindz: []\n");
    let err = Store::open(&tmp.0, Access::Full).unwrap_err();
    assert!(matches!(err, StoreError::Config(_)), "{err}");
    assert!(err.to_string().contains("kindz"), "{err}");
    let tmp = TempDir::new("notes");
    write(&tmp.0, "config.yaml", "layers: { notes: memory }\n");
    write(
        &tmp.0,
        "notes/M-0001.md",
        &memory("M-0001", "A note", "status: sorted\nlinks: []\n"),
    );
    write(
        &tmp.0,
        "tasks/T-0001.md",
        "---
id: T-0001
title: Still a task
status: todo
links: []
---
",
    );
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let entries = store.list(&Filter::default());
    assert_eq!(ids(&entries), ["M-0001", "T-0001"]);
    assert_eq!(entries[0].layer, Layer::Memory);
    assert_eq!(entries[0].path, PathBuf::from("notes/M-0001.md"));
    assert_eq!(entries[1].layer, Layer::Task);
    assert_eq!(entries[1].path, PathBuf::from("tasks/T-0001.md"));
}

// =============================================================================
// Flags, files, proposals per file
// =============================================================================

/// A file outside the data folder, as the app hands it over.
fn outside(dir: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.0.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn no_flags() -> Flags {
    Flags::default()
}

#[test]
fn inline_flags_land_as_fields_and_the_text_stays_as_typed() {
    let tmp = TempDir::new("flags");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let text = "Call Noa #bench before Friday @noa !urgent.\nissue#12, a@b.c ## no";
    let piece = store.capture(text, Source::new("console")).unwrap();
    assert_eq!(piece.body(), format!("{text}\n"));
    assert_eq!(piece.tags(), ["bench"]);
    let file = read(&tmp.0, "inbox/M-0001.md");
    let fields = "\ntags: [bench]\npeople: [noa]\nmarks: [urgent]\n";
    assert!(file.contains(fields), "{file}");
    let given = Flags {
        tags: vec!["bench".into(), "lab".into()],
        people: Vec::new(),
        marks: vec!["later".into()],
    };
    let source = Source::new("app");
    let piece = store.capture_with("plain #x", source, &given, &[]).unwrap();
    assert_eq!(piece.tags(), ["x", "bench", "lab"]);
    assert!(piece.get("people").is_none());
    let later = Value::Sequence(vec![Value::from("later")]);
    assert_eq!(piece.get("marks"), Some(&later));
    let plain = store.capture("no flags", Source::new("console")).unwrap();
    let extra = ["people", "marks", "files"];
    assert!(plain.keys().iter().all(|k| !extra.contains(k)));
}

#[test]
fn a_captured_file_is_copied_beside_the_piece_and_listed() {
    let tmp = TempDir::new("files");
    let src = TempDir::new("files_src");
    let photo = outside(&src, "probe.JPG", b"\xff\xd8 not really a jpeg");
    let notes = outside(&src, "notes.txt", b"n");
    let blob = outside(&src, "data.bin", b"b");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let given = [photo.clone(), notes, blob];
    let captured = store
        .capture_with("with files", Source::new("app"), &no_flags(), &given)
        .unwrap();
    let files = captured.files();
    let kinds: Vec<(&str, &str, &str)> = files
        .iter()
        .map(|f| (f.name.as_str(), f.path.as_str(), f.kind.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("probe.JPG", "inbox/M-0001/probe.JPG", "image"),
            ("notes.txt", "inbox/M-0001/notes.txt", "text"),
            ("data.bin", "inbox/M-0001/data.bin", "file"),
        ]
    );
    let copied = fs::read(tmp.0.join("inbox/M-0001/probe.JPG")).unwrap();
    assert_eq!(copied, fs::read(&photo).unwrap());
    assert_eq!(store.files(&r("M-0001")).unwrap(), files);
    let reread = piece(&tmp.0, "inbox/M-0001.md");
    assert_eq!(reread.render(), read(&tmp.0, "inbox/M-0001.md"));
    assert!(store.check().unwrap().is_empty());
    let before = snapshot(&tmp.0);
    let twice = [photo.clone(), photo];
    let twice = store.capture_with("x", Source::new("app"), &no_flags(), &twice);
    assert!(matches!(twice, Err(StoreError::Invalid(_))), "{twice:?}");
    let missing = [src.0.join("gone.png")];
    let gone = store.capture_with("x", Source::new("app"), &no_flags(), &missing);
    assert!(matches!(gone, Err(StoreError::Io(_))), "{gone:?}");
    assert_eq!(snapshot(&tmp.0), before);
}

#[test]
fn a_move_takes_the_files_along_and_deletes_nothing_on_a_failure() {
    let tmp = fixture("valid", "movefiles");
    let src = TempDir::new("movefiles_src");
    let photo = outside(&src, "probe.jpg", b"jpeg");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let a = store
        .capture_with(
            "a",
            Source::new("app"),
            &no_flags(),
            std::slice::from_ref(&photo),
        )
        .unwrap();
    let a = a.id().unwrap().to_string();
    let attached = tmp.0.join(format!("inbox/{a}/probe.jpg"));
    let to_memory = || sort(&a, Layer::Memory, &[("kind", "fact")]);
    let diff = store.propose(to_memory()).unwrap();
    let header = format!("move inbox/{a}/probe.jpg → memories/{a}/probe.jpg");
    assert!(diff.lines().contains(&header), "{:#?}", diff.lines());
    assert!(diff.changes.iter().any(Change::is_file_move));
    fs::remove_file(&attached).unwrap();
    let gone = snapshot(&tmp.0);
    let err = store.accept(diff).unwrap_err();
    assert!(matches!(err, StoreError::Changed(_)), "{err}");
    assert_eq!(snapshot(&tmp.0), gone);
    fs::write(&attached, b"jpeg").unwrap();
    write(&tmp.0, &format!("memories/{a}/probe.jpg"), "in the way");
    let in_the_way = snapshot(&tmp.0);
    let diff = store.propose(to_memory()).unwrap();
    let err = store.accept(diff).unwrap_err();
    assert!(matches!(err, StoreError::Exists(_)), "{err}");
    assert_eq!(snapshot(&tmp.0), in_the_way);
    fs::remove_dir_all(tmp.0.join(format!("memories/{a}"))).unwrap();
    let diff = store.propose(to_memory()).unwrap();
    store.accept(diff).unwrap();
    let moved = tmp.0.join(format!("memories/{a}/probe.jpg"));
    assert_eq!(fs::read(moved).unwrap(), b"jpeg");
    assert!(!tmp.0.join(format!("inbox/{a}")).exists());
    let listed = store.files(&r(&a)).unwrap();
    assert_eq!(listed[0].path, format!("memories/{a}/probe.jpg"));
    let b = store
        .capture_with("b", Source::new("app"), &no_flags(), &[photo])
        .unwrap();
    let b = b.id().unwrap().to_string();
    let diff = store
        .propose(sort(&b, Layer::Task, &[("title", "Bench")]))
        .unwrap();
    let written = store.accept(diff).unwrap();
    let task = written.iter().find(|i| i.prefix() == Some("T")).unwrap();
    let task = task.to_string();
    let in_task = tmp.0.join(format!("tasks/{task}/probe.jpg"));
    assert_eq!(fs::read(in_task).unwrap(), b"jpeg");
    let listed = store.files(&r(&task)).unwrap();
    assert_eq!(listed[0].path, format!("tasks/{task}/probe.jpg"));
    assert!(store.files(&r(&b)).unwrap().is_empty());
    assert!(!tmp.0.join(format!("inbox/{b}")).exists());
}

#[test]
fn each_proposal_file_is_read_on_its_own() {
    let tmp = fixture("valid", "eachproposal");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let second = store.capture("second", Source::new("console")).unwrap();
    let second = second.id().unwrap().to_string();
    let broken = "action: sort\nid: M-0007\nlayer: [unclosed\n";
    write(&tmp.0, "inbox/M-0007.sort.yaml", broken);
    let good = format!("action: sort\nid: {second}\nlayer: memory\n");
    write(&tmp.0, &format!("inbox/{second}.sort.yaml"), &good);
    write(&tmp.0, "inbox/M-0007/x.sort.yaml", "not a proposal");
    let list = store.proposal_files().unwrap();
    assert_eq!(list.len(), 2, "{list:#?}");
    let bad = list.iter().find(|p| p.id == id("M-0007")).unwrap();
    assert!(bad.action.is_none());
    assert!(!bad.error.as_deref().unwrap_or("").is_empty());
    assert_eq!(bad.path, PathBuf::from("inbox/M-0007.sort.yaml"));
    let ok = list.iter().find(|p| p.id == id(&second)).unwrap();
    assert!(ok.error.is_none() && ok.action.is_some());
    assert!(matches!(store.proposals(), Err(StoreError::Yaml { .. })));
    let free = Store::open(&tmp.0, Access::Free).unwrap();
    assert!(free.proposal_files().unwrap().is_empty());
}

#[test]
fn a_free_store_never_sees_the_files_of_a_gated_piece() {
    let tmp = fixture("valid", "freefiles");
    let src = TempDir::new("freefiles_src");
    let photo = outside(&src, "probe.jpg", b"jpeg");
    let mut full = Store::open(&tmp.0, Access::Full).unwrap();
    let piece = full
        .capture_with("secret", Source::new("app"), &no_flags(), &[photo])
        .unwrap();
    let pid = piece.id().unwrap().to_string();
    let mut free = Store::open(&tmp.0, Access::Free).unwrap();
    let err = free.files(&r(&pid)).unwrap_err();
    assert_eq!(err.to_string(), format!("confidential: {pid}"));
    let forged = Diff {
        action: Action::Retire {
            id: id("M-0001"),
            behind: None,
        },
        made_at: chrono::Local::now().fixed_offset(),
        changes: vec![Change {
            path: PathBuf::from("memories/M-0001/probe.jpg"),
            from: Some(PathBuf::from(format!("inbox/{pid}/probe.jpg"))),
            before: None,
            after: None,
        }],
        dropped: Vec::new(),
    };
    let before = snapshot(&tmp.0);
    let err = free.accept(forged).unwrap_err();
    assert!(matches!(err, StoreError::Confidential(_)), "{err}");
    assert_eq!(snapshot(&tmp.0), before);
    assert!(!read(&tmp.0, "index.json").contains("probe.jpg"));
}

// =============================================================================
// Several files per capture, the person, the diary, the inbox, links as paths
// =============================================================================

/// A task sort of `M-0007`, as an agent writes it.
const TASK_SORT: &str = "# why: a thing to do\n# by: sort anthropic claude-haiku-5-5\n\
    action: sort\nid: M-0007\nlayer: task\nset:\n  title: Call the bank\nsorted_by: sorter\n";

fn day(y: i32, m: u32, d: u32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

/// The files under `inbox/`.
fn inbox_files(root: &Path) -> Vec<PathBuf> {
    snapshot(&root.join("inbox")).into_keys().collect()
}

/// Accept the first pending proposal file as it is; the ids written.
fn accept_first(store: &mut Store) -> Vec<PieceId> {
    let file = store.proposal_files().unwrap().remove(0);
    let diff = store.propose(file.action.unwrap()).unwrap();
    store.accept(diff).unwrap()
}

#[test]
fn a_diary_note_makes_the_day_once_and_appends_a_timed_line() {
    let tmp = fixture("valid", "diary");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let inbox = inbox_files(&tmp.0);
    let date = day(2026, 10, 20);
    let id = store
        .diary_append(date, "Q: next?\nLorisse: the van.")
        .unwrap();
    assert_eq!(id.to_string(), "journal/2026-10-20");
    store.diary_append(date, "Bois.").unwrap();
    let text = read(&tmp.0, "journal/2026-10-20.md");
    assert!(
        text.starts_with("---\ndate: 2026-10-20\nkind: diary\nupdated: "),
        "{text}"
    );
    let body = piece(&tmp.0, "journal/2026-10-20.md").body().to_string();
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines.len(), 3, "{body}");
    let timed = |line: &str, rest: &str| line.starts_with("- ") && line[7..] == *rest;
    assert!(
        timed(lines[0], " Q: next?") && timed(lines[2], " Bois."),
        "{body}"
    );
    assert_eq!(lines[1], "  Lorisse: the van.");
    assert_eq!(inbox_files(&tmp.0), inbox, "nothing lands in the inbox");
    let read_back = store.read(&r("journal/2026-10-20")).unwrap();
    assert_eq!(read_back.kind(), Some("diary"));
    store.diary_append(day(2026, 10, 14), "Later.").unwrap();
    let held = read(&tmp.0, "confidential/journal/2026-10-14.md");
    assert!(
        held.contains("called twice.\n\n- ") && held.ends_with(" Later.\n"),
        "{held}"
    );
    assert!(!tmp.0.join("journal/2026-10-14.md").exists());
    let mut free = Store::open(&tmp.0, Access::Free).unwrap();
    let err = free.diary_append(day(2026, 10, 14), "x").unwrap_err();
    assert_eq!(err.to_string(), "confidential: journal/2026-10-14");
    let blank = store.diary_append(date, "  \n").unwrap_err();
    assert!(matches!(blank, StoreError::Invalid(_)), "{blank}");
}

#[test]
fn a_conversation_line_makes_the_day_once_and_names_who_speaks() {
    let tmp = fixture("valid", "conversation");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let inbox = inbox_files(&tmp.0);
    let date = day(2026, 10, 20);
    let id = store
        .conversation_append(date, Speaker::You, "Je suis perdu.")
        .unwrap();
    assert_eq!(id.to_string(), "conversations/2026-10-20");
    store
        .conversation_append(date, Speaker::Lorisse, "Trois choses.\nLa première.")
        .unwrap();
    let text = read(&tmp.0, "conversations/2026-10-20.md");
    assert!(
        text.starts_with("---\ndate: 2026-10-20\nkind: conversation\nupdated: "),
        "{text}"
    );
    let body = piece(&tmp.0, "conversations/2026-10-20.md")
        .body()
        .to_string();
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines.len(), 3, "{body}");
    assert_eq!(&lines[0][7..], " you: Je suis perdu.");
    assert_eq!(&lines[1][7..], " lorisse: Trois choses.");
    assert_eq!(lines[2], "  La première.");
    assert_eq!(inbox_files(&tmp.0), inbox, "nothing lands in the inbox");
    let days = store.list(&Filter {
        layer: Some(Layer::Conversation),
        ..Filter::default()
    });
    assert_eq!(days.len(), 1);
    assert!(store.list(&Filter::default()).len() > 1);
    let blank = store
        .conversation_append(date, Speaker::You, " \n")
        .unwrap_err();
    assert!(matches!(blank, StoreError::Invalid(_)), "{blank}");
    write(
        &tmp.0,
        "confidential/conversations/2026-10-14.md",
        "---\ndate: 2026-10-14\nkind: conversation\n---\n- 09:00 you: x\n",
    );
    let mut free = Store::open(&tmp.0, Access::Free).unwrap();
    let err = free
        .conversation_append(day(2026, 10, 14), Speaker::You, "y")
        .unwrap_err();
    assert_eq!(err.to_string(), "confidential: conversations/2026-10-14");
    let mut full = Store::open(&tmp.0, Access::Full).unwrap();
    full.conversation_append(day(2026, 10, 14), Speaker::Lorisse, "z")
        .unwrap();
    let held = read(&tmp.0, "confidential/conversations/2026-10-14.md");
    assert!(held.ends_with(" lorisse: z\n"), "{held}");
}

#[test]
fn two_conversation_days_are_never_deduped() {
    let tmp = fixture("valid", "convdedupe");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    store
        .conversation_append(day(2026, 10, 20), Speaker::You, "a")
        .unwrap();
    store
        .conversation_append(day(2026, 10, 21), Speaker::You, "b")
        .unwrap();
    let err = store
        .propose(Action::Dedupe {
            keep: id("conversations/2026-10-20"),
            drop: id("conversations/2026-10-21"),
        })
        .unwrap_err();
    assert!(matches!(err, StoreError::Invalid(_)), "{err:?}");
}

#[test]
fn the_inbox_lists_raw_pieces_and_the_retired_apart() {
    let tmp = fixture("valid", "inboxraw");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let test = store.capture("a test", Source::new("console")).unwrap();
    let test = test.id().unwrap();
    assert_eq!(store.inbox().len(), 2);
    assert!(store.retired_in_inbox().is_empty());
    let retire = format!("action: retire\nid: {test}\n");
    write(&tmp.0, &format!("inbox/{test}.sort.yaml"), &retire);
    accept_first(&mut store);
    assert_eq!(ids(&store.inbox()), ["M-0007"]);
    assert_eq!(ids(&store.retired_in_inbox()), [test.to_string()]);
    assert!(tmp.0.join(format!("inbox/{test}.md")).exists());
    assert!(!tmp.0.join(format!("inbox/{test}.sort.yaml")).exists());
}

#[test]
fn several_proposal_files_of_one_capture_are_listed_in_order() {
    let tmp = fixture("valid", "parts");
    let retire = "action: retire\nid: M-0007\n";
    for name in ["M-0007.10", "M-0007.2", "M-0007", "M-0007.1"] {
        write(&tmp.0, &format!("inbox/{name}.sort.yaml"), retire);
    }
    write(&tmp.0, "inbox/M-0007.3.skipped.yaml", retire);
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let list = store.proposal_files().unwrap();
    let parts: Vec<(String, u32)> = list.iter().map(|p| (p.id.to_string(), p.part)).collect();
    let m7 = || "M-0007".to_string();
    assert_eq!(parts, [(m7(), 1), (m7(), 2), (m7(), 10)]);
    assert_eq!(list[1].path, PathBuf::from("inbox/M-0007.2.sort.yaml"));
    assert_eq!(store.proposals().unwrap().len(), 3);
    let report = store.check().unwrap().to_string();
    assert!(report.contains("inbox/M-0007.1.sort.yaml: a proposal with no matching piece"));
    assert!(!report.contains("M-0007.2.sort.yaml"), "{report}");
    let next = store.proposal_path(&id("M-0007"));
    assert_eq!(next, PathBuf::from("inbox/M-0007.4.sort.yaml"));
    let fresh = store.proposal_path(&id("M-1"));
    assert_eq!(fresh, PathBuf::from("inbox/M-0001.sort.yaml"));
}

#[test]
fn a_named_id_s_proposal_files_one_folder_down_are_listed_with_their_parts() {
    let tmp = fixture("valid", "namedparts");
    let edit = "action: edit\nid: tags/bois\nset:\n  definition: le bois\n";
    let path = Store::open(&tmp.0, Access::Full)
        .unwrap()
        .proposal_path(&id("tags/bois"));
    assert_eq!(path, PathBuf::from("inbox/tags/bois.sort.yaml"));
    write(&tmp.0, "inbox/tags/bois.sort.yaml", edit);
    write(&tmp.0, "inbox/tags/bois.2.sort.yaml", edit);
    write(&tmp.0, "inbox/M-0007/x.sort.yaml", edit);
    write(&tmp.0, "inbox/a/b/c.sort.yaml", edit);
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let list = store.proposal_files().unwrap();
    let parts: Vec<(String, u32)> = list.iter().map(|p| (p.id.to_string(), p.part)).collect();
    let bois = || "tags/bois".to_string();
    assert_eq!(
        parts,
        [(bois(), 1), (bois(), 2)],
        "a number's folder and two deep are not"
    );
    assert!(list.iter().all(|p| p.action.is_some()), "{list:?}");
    let next = store.proposal_path(&id("tags/bois"));
    assert_eq!(next, PathBuf::from("inbox/tags/bois.3.sort.yaml"));
}

#[test]
fn each_sort_of_one_capture_makes_its_piece_and_the_last_retires_it() {
    let tmp = fixture("valid", "multisort");
    write(&tmp.0, "inbox/M-0007.sort.yaml", TASK_SORT);
    let person = "action: sort\nid: M-0007\nlayer: person\nset:\n  name: Martin Dupont\n";
    write(&tmp.0, "inbox/M-0007.2.sort.yaml", person);
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let written = accept_first(&mut store);
    assert_eq!(written, [id("M-0007"), id("T-0003")]);
    let capture = piece(&tmp.0, "inbox/M-0007.md");
    assert_eq!(capture.status(), Some("raw"));
    assert_eq!(capture.links(), [r("T-0003")]);
    assert!(!tmp.0.join("inbox/M-0007.sort.yaml").exists());
    assert!(
        tmp.0.join("inbox/M-0007.2.sort.yaml").exists(),
        "the other file stays"
    );
    assert_eq!(ids(&store.inbox()), ["M-0007"]);
    let task = piece(&tmp.0, "tasks/T-0003.md");
    assert_eq!(task.get("capture"), Some(&Value::from("M-0007")));
    accept_first(&mut store);
    let person = piece(&tmp.0, "people/martin-dupont.md");
    assert_eq!(person.get("capture"), Some(&Value::from("M-0007")));
    let retired = piece(&tmp.0, "memories/M-0007.md");
    assert_eq!(retired.status(), Some("retired"));
    assert_eq!(retired.links(), [r("people/martin-dupont"), r("T-0003")]);
    assert!(!tmp.0.join("inbox/M-0007.md").exists());
    assert!(store.proposal_files().unwrap().is_empty() && store.inbox().is_empty());
    let other = store
        .capture("a fact and a task", Source::new("app"))
        .unwrap();
    let other = other.id().unwrap();
    let memory = format!("action: sort\nid: {other}\nlayer: memory\nset:\n  kind: fact\n");
    write(&tmp.0, &format!("inbox/{other}.sort.yaml"), &memory);
    let task = format!("action: sort\nid: {other}\nlayer: task\n");
    write(&tmp.0, &format!("inbox/{other}.2.sort.yaml"), &task);
    accept_first(&mut store);
    accept_first(&mut store);
    let kept = piece(&tmp.0, &format!("memories/{other}.md"));
    assert_eq!(
        kept.status(),
        Some("sorted"),
        "a sorted capture is never retired"
    );
    assert_eq!(kept.links(), [r("T-0004")]);
}

#[test]
fn the_last_file_of_a_capture_retires_it_where_it_lies() {
    let tmp = fixture("valid", "lastfile");
    write(&tmp.0, "inbox/M-0007.sort.yaml", TASK_SORT);
    let edit = "action: edit\nid: T-0001\nset:\n  next: Ask the bank\n";
    write(&tmp.0, "inbox/M-0007.2.sort.yaml", edit);
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    accept_first(&mut store);
    let written = accept_first(&mut store);
    assert_eq!(written, [id("M-0007"), id("T-0001")]);
    let task = piece(&tmp.0, "tasks/T-0001.md");
    assert_eq!(task.get("next"), Some(&Value::from("Ask the bank")));
    let capture = piece(&tmp.0, "inbox/M-0007.md");
    assert_eq!(capture.status(), Some("retired"));
    assert_eq!(capture.links(), [r("T-0003"), r("T-0001")]);
    assert!(store.inbox().is_empty());
    assert_eq!(ids(&store.retired_in_inbox()), ["M-0007"]);
    assert!(store.proposal_files().unwrap().is_empty());
}

#[test]
fn a_skipped_proposal_file_is_kept_and_no_longer_listed() {
    let tmp = fixture("valid", "skip");
    write(&tmp.0, "inbox/M-0007.sort.yaml", TASK_SORT);
    let link = "action: link\nfrom: M-0001\nto: [M-0002]\n";
    write(&tmp.0, "inbox/M-0007.2.sort.yaml", link);
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    accept_first(&mut store);
    let file = store.proposal_files().unwrap().remove(0);
    assert_eq!(store.skip(&file).unwrap(), [id("M-0007")]);
    assert_eq!(read(&tmp.0, "inbox/M-0007.2.skipped.yaml"), link);
    assert!(!tmp.0.join("inbox/M-0007.2.sort.yaml").exists());
    assert!(store.proposal_files().unwrap().is_empty());
    assert_eq!(piece(&tmp.0, "inbox/M-0007.md").status(), Some("retired"));
    let lone = store.capture("only one file", Source::new("app")).unwrap();
    let lone = lone.id().unwrap();
    let retire = format!("action: retire\nid: {lone}\n");
    write(&tmp.0, &format!("inbox/{lone}.sort.yaml"), &retire);
    let file = store.proposal_files().unwrap().remove(0);
    assert!(store.skip(&file).unwrap().is_empty());
    assert_eq!(
        ids(&store.inbox()),
        [lone.to_string()],
        "nothing came of it: raw"
    );
    let mut free = Store::open(&tmp.0, Access::Free).unwrap();
    let err = free.skip(&file).unwrap_err();
    assert!(matches!(err, StoreError::Confidential(_)), "{err}");
}

/// The `yaml` blocks of the README's "The proposal file" section.
fn readme_examples() -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    let text = fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let start = text.find("## The proposal file").unwrap();
    let section = &text[start..];
    let end = section[3..].find("\n## ").map_or(section.len(), |i| i + 3);
    let blocks = section[..end].split("```yaml\n").skip(1);
    blocks
        .map(|b| b.split("```").next().unwrap().to_string())
        .collect()
}

#[test]
fn every_proposal_example_of_the_readme_parses_as_its_action() {
    let examples = readme_examples();
    let mut kinds = Vec::new();
    for text in &examples {
        let action: Action =
            serde_yaml_ng::from_str(text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        let kind = match &action {
            Action::Sort { layer, .. } => format!("sort {layer}"),
            other => format!("{other:?}")
                .split_whitespace()
                .next()
                .unwrap()
                .to_lowercase(),
        };
        kinds.push(kind);
        let typo = format!("{text}why: x\n");
        let err = serde_yaml_ng::from_str::<Action>(&typo)
            .unwrap_err()
            .to_string();
        assert!(err.contains("why"), "{err}");
        let tmp = fixture("valid", "readme");
        let store = Store::open(&tmp.0, Access::Full).unwrap();
        store
            .propose(action)
            .unwrap_or_else(|e| panic!("{e}\n{text}"));
    }
    let want = [
        "sort task",
        "sort person",
        "edit",
        "link",
        "dedupe",
        "dedupe",
        "archive",
        "retire",
        "move",
        "renametag",
    ];
    assert_eq!(kinds, want);
    assert!(examples[0].starts_with("# why: ") && examples[0].contains("\n# by: "));
}

#[test]
fn a_sort_into_a_person_writes_its_fields_and_two_sections() {
    let tmp = fixture("valid", "person");
    let sort = "action: sort\nid: M-0007\nlayer: person\nset:\n  name: Sam Example\n  \
        role: sells cells\n  company: Example Co\n  contact: sam@example.com ; 06 00\n  \
        tags: [battery]\n  links: [../confidential/memories/M-0003.md]\n";
    write(&tmp.0, "inbox/M-0007.sort.yaml", sort);
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    accept_first(&mut store);
    let text = read(&tmp.0, "people/sam-example.md");
    let head = "---\nname: Sam Example\nkind: fact\nrole: sells cells\ncompany: Example Co\n\
        contact: sam@example.com ; 06 00\ntags: [battery]\ncapture: M-0007\n";
    assert!(text.starts_with(head), "{text}");
    assert!(text.contains("\nlinks: [M-0003]\n"), "{text}");
    let body =
        "## What I know\n\nCall the bank about the card\nbefore Friday.\n\n## How I handle\n";
    assert!(text.ends_with(&format!("---\n{body}")), "{text}");
    let again = store.capture("same man", Source::new("app")).unwrap();
    let again = again.id().unwrap();
    let sort =
        format!("action: sort\nid: {again}\nlayer: person\nset:\n  name: Sam EXAMPLE\n");
    write(&tmp.0, &format!("inbox/{again}.sort.yaml"), &sort);
    let file = store.proposal_files().unwrap().remove(0);
    let diff = store.propose(file.action.unwrap()).unwrap();
    let err = store.accept(diff).unwrap_err();
    assert!(matches!(err, StoreError::Exists(_)), "{err}");
}

#[test]
fn a_link_written_as_a_path_names_the_bare_id() {
    let tmp = fixture("valid", "barelinks");
    let links = "links: [../confidential/memories/M-0003, memories/M-0001.md, \
        ../confidential/people/robin.md]\n";
    let text = memory("M-0005", "Paths", links);
    write(&tmp.0, "memories/M-0005.md", &text);
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let entries = store.list(&Filter::default());
    let entry = entries.iter().find(|e| e.id == id("M-0005")).unwrap();
    assert_eq!(entry.links, [r("M-0003"), r("M-0001"), r("people/robin")]);
    assert!(
        store
            .dangling()
            .iter()
            .all(|(from, _)| *from != id("M-0005"))
    );
    assert!(store.links(&r("M-0001")).incoming.contains(&id("M-0005")));
    assert_eq!(
        read(&tmp.0, "memories/M-0005.md"),
        text,
        "a read rewrites nothing"
    );
    let diff = store.propose(edit("M-0005", &[("title", "Bare")])).unwrap();
    store.accept(diff).unwrap();
    let after = read(&tmp.0, "memories/M-0005.md");
    assert!(
        after.contains("\nlinks: [M-0003, M-0001, people/robin]\n"),
        "{after}"
    );
}

#[test]
fn a_person_is_found_and_linked_by_name_or_slug() {
    let tmp = fixture("valid", "byname");
    let text = memory("M-0005", "Names", "links: [Robin, noa]\n");
    write(&tmp.0, "memories/M-0005.md", &text);
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    assert_eq!(store.read(&r("Robin")).unwrap().title(), "Robin");
    assert_eq!(store.read(&r("ROBIN")).unwrap().title(), "Robin");
    assert_eq!(store.read(&r("noa")).unwrap().title(), "Noa");
    assert!(
        store
            .dangling()
            .iter()
            .all(|(from, _)| *from != id("M-0005"))
    );
    let incoming = store.links(&r("people/robin")).incoming;
    assert!(incoming.contains(&id("M-0005")), "{incoming:?}");
    let free = Store::open(&tmp.0, Access::Free).unwrap();
    let hidden = free.read(&r("Robin"));
    assert!(
        matches!(hidden, Err(StoreError::Confidential(_))),
        "{hidden:?}"
    );
    assert!(
        free.dangling()
            .iter()
            .all(|(from, _)| *from != id("M-0005"))
    );
}
