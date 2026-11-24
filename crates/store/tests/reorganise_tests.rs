//! The reorganise scenarios of VALIDATION.md § 3 (move, the tag layer, rename_tag), over a copy of the fixtures.

use std::fs;
use std::path::{Path, PathBuf};

use lorisse_store::{Access, Action, Fields, Filter, Layer, PieceId, Source, Store, StoreError, Value};

// =============================================================================
// Helpers
// =============================================================================

/// A unique, empty folder under the temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let name = format!("lorisse_store_re_{tag}_{}_{nanos}", std::process::id());
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

/// A temporary copy of `fixtures/valid`.
fn valid(tag: &str) -> TempDir {
    let tmp = TempDir::new(tag);
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/valid");
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

fn id(text: &str) -> PieceId {
    PieceId::parse(text).unwrap()
}

fn fields(pairs: &[(&str, &str)]) -> Fields {
    let mut map = Fields::new();
    for (k, v) in pairs {
        map.insert(Value::from(*k), Value::from(*v));
    }
    map
}

fn moved(target: &str, layer: Option<Layer>, confidential: Option<bool>, set: Fields) -> Action {
    Action::Move {
        id: id(target),
        layer,
        confidential,
        set,
    }
}

fn rename(from: &str, to: &str) -> Action {
    Action::RenameTag {
        from: from.into(),
        to: to.into(),
        why: None,
    }
}

// =============================================================================
// Move
// =============================================================================

#[test]
fn a_move_into_confidential_keeps_the_id_and_every_link_to_it() {
    let tmp = valid("moveconf");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let diff = store
        .propose(moved("M-0001", None, Some(true), Fields::new()))
        .unwrap();
    assert!(diff.dropped.is_empty());
    assert_eq!(
        diff.lines()[0],
        "move memories/M-0001.md → confidential/memories/M-0001.md"
    );
    store.accept(diff).unwrap();
    assert!(!tmp.0.join("memories/M-0001.md").exists());
    assert!(read(&tmp.0, "confidential/memories/M-0001.md").contains("id: M-0001"));
    assert!(read(&tmp.0, "tasks/T-0001.md").contains("links: [M-0001]"));
    let links = store.links(&lorisse_store::Ref::new("M-0001"));
    assert!(links.incoming.contains(&id("T-0001")), "{links:?}");
    let back = store
        .propose(moved("M-0001", None, Some(false), Fields::new()))
        .unwrap();
    store.accept(back).unwrap();
    assert!(tmp.0.join("memories/M-0001.md").is_file());
}

#[test]
fn a_memory_becomes_a_task_with_the_next_id_its_kind_dropped_the_old_retired_and_relinked() {
    let tmp = valid("movetask");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let set = fields(&[("priority", "high")]);
    let diff = store
        .propose(moved("M-0001", Some(Layer::Task), None, set))
        .unwrap();
    assert_eq!(diff.dropped, ["kind"]);
    assert_eq!(diff.lines()[0], "dropped: kind");
    let written = store.accept(diff).unwrap();
    assert!(written.contains(&id("T-0003")), "{written:?}");
    let task = read(&tmp.0, "tasks/T-0003.md");
    assert!(
        task.contains("id: T-0003\ntitle: Daily spoken brief\nstatus: todo\n"),
        "{task}"
    );
    assert!(
        task.contains("priority: high") && !task.contains("kind:"),
        "{task}"
    );
    assert!(task.contains("tags: [brief, voice]"), "{task}");
    let old = read(&tmp.0, "memories/M-0001.md");
    assert!(
        old.contains("status: retired") && old.contains("links: [T-0003]"),
        "{old}"
    );
    assert!(read(&tmp.0, "tasks/T-0001.md").contains("links: [T-0003]"));
    assert!(read(&tmp.0, "memories/M-0002.md").contains("links: [T-0003, people/robin]"));
}

#[test]
fn a_task_becomes_a_memory_its_task_keys_dropped_and_the_old_task_archived_retired() {
    let tmp = valid("movememory");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let set = fields(&[("kind", "fact")]);
    let diff = store
        .propose(moved("T-0001", Some(Layer::Memory), None, set))
        .unwrap();
    assert_eq!(
        diff.dropped,
        ["priority", "category", "owner", "due", "created"]
    );
    store.accept(diff).unwrap();
    let memory = read(&tmp.0, "memories/M-0008.md");
    assert!(
        memory.contains("kind: fact") && memory.contains("status: sorted"),
        "{memory}"
    );
    assert!(
        !memory.contains("priority") && memory.contains("## Context"),
        "{memory}"
    );
    let archived = read(&tmp.0, "archive/T-0001.md");
    assert!(archived.contains("status: retired") && archived.contains("links: [M-0008, M-0001]"));
    assert!(!tmp.0.join("tasks/T-0001.md").exists());
}

#[test]
fn a_memory_becomes_a_person_named_by_its_set_and_a_pair_the_store_does_not_move_is_refused() {
    let tmp = valid("moveperson");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let set = fields(&[("name", "Jean Brief"), ("role", "voice")]);
    let diff = store
        .propose(moved("M-0001", Some(Layer::Person), None, set))
        .unwrap();
    store.accept(diff).unwrap();
    let person = read(&tmp.0, "people/jean-brief.md");
    assert!(
        person.contains("name: Jean Brief") && person.contains("role: voice"),
        "{person}"
    );
    assert!(
        person.contains("## What I know") && !person.contains("title:"),
        "{person}"
    );
    let refused = |action: Action, word: &str| {
        let err = store.propose(action).unwrap_err();
        assert!(err.to_string().contains(word), "{err}");
    };
    refused(
        moved("M-0002", Some(Layer::Diary), None, Fields::new()),
        "not a memory to a diary",
    );
    refused(
        moved("M-0002", None, Some(false), Fields::new()),
        "nothing to move",
    );
    refused(
        moved("M-0007", Some(Layer::Task), None, Fields::new()),
        "sorted, not moved",
    );
    let text = "action: move\nid: M-0002\nlayer: task\nset:\n  priority: high\n";
    let read_back: Action = serde_yaml_ng::from_str(text).unwrap();
    assert!(matches!(
        read_back,
        Action::Move {
            layer: Some(Layer::Task),
            confidential: None,
            ..
        }
    ));
}

// =============================================================================
// The tag layer
// =============================================================================

const BOIS: &str = "---\ntag: bois\ndefinition: le bois de chauffage\n---\n";

#[test]
fn a_tag_file_is_indexed_as_the_tag_layer_and_the_tags_count_their_pieces() {
    let tmp = valid("tagindex");
    write(
        &tmp.0,
        "tags/brief.md",
        "---\ntag: brief\ndefinition: le point du jour\naliases: [daily]\n---\n",
    );
    write(
        &tmp.0,
        "memories/M-0009.md",
        "---\nid: M-0009\ntitle: x\ntags: [daily, brief]\nstatus: sorted\n---\nx\n",
    );
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let tags = Filter {
        layer: Some(Layer::Tag),
        ..Filter::default()
    };
    let listed = store.list(&tags);
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (listed[0].id.to_string(), listed[0].title.as_str()),
        ("tags/brief".into(), "brief")
    );
    let all = store.tags();
    let brief = all.iter().find(|t| t.slug == "brief").unwrap();
    assert_eq!(brief.count, 2, "M-0001 and M-0009, the alias counted once");
    assert_eq!(brief.definition.as_deref(), Some("le point du jour"));
    let work = all.iter().find(|t| t.name == "work").unwrap();
    assert_eq!((work.count, work.id.clone()), (1, None));
    let report = store.check().unwrap().to_string();
    assert!(
        !report.contains("tags/brief"),
        "a tag file in the house style: {report}"
    );
}

#[test]
fn the_owners_definition_makes_or_rewrites_the_tag_file_and_a_capture_sorts_into_a_tag() {
    let tmp = valid("tagdefine");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let made = store.tag_define("Bois", "le bois de chauffage").unwrap();
    assert_eq!(made.to_string(), "tags/bois");
    assert_eq!(
        read(&tmp.0, "tags/bois.md"),
        "---\ntag: Bois\ndefinition: le bois de chauffage\n---\n"
    );
    store.tag_define("bois", "le bois, coupé").unwrap();
    assert!(read(&tmp.0, "tags/bois.md").contains("definition: le bois, coupé"));
    assert!(store.tag_define("bois", " ").is_err());
    assert!(store.tag_define("bois", "deux\nlignes").is_err());
    let capture = store
        .capture("la famille : le père, la mère", Source::new("agent"))
        .unwrap();
    let cid = capture.id().unwrap();
    let sort = Action::Sort {
        id: cid.clone(),
        layer: Layer::Tag,
        confidential: false,
        slice: None,
        set: fields(&[("tag", "famille"), ("definition", "le père, la mère")]),
        sorted_by: None,
    };
    let diff = store.propose(sort).unwrap();
    store.accept(diff).unwrap();
    let tag = read(&tmp.0, "tags/famille.md");
    assert!(
        tag.starts_with("---\ntag: famille\ndefinition: le père, la mère\n"),
        "{tag}"
    );
    assert!(
        tag.contains(&format!("capture: {cid}")) && tag.ends_with("---\n"),
        "{tag}"
    );
    let retired = read(&tmp.0, &format!("memories/{cid}.md"));
    assert!(retired.contains("links: [tags/famille]"), "{retired}");
}

// =============================================================================
// rename_tag
// =============================================================================

#[test]
fn a_rename_touches_every_piece_carrying_the_tag_folds_a_duplicate_and_renames_its_file() {
    let tmp = valid("tagrename");
    write(&tmp.0, "tags/bois.md", BOIS);
    write(
        &tmp.0,
        "memories/M-0009.md",
        "---\nid: M-0009\ntitle: x\ntags: [bois, chauffage]\nstatus: sorted\n---\nx\n",
    );
    write(
        &tmp.0,
        "memories/M-0010.md",
        "---\nid: M-0010\ntitle: y\ntags: [Bois]\nstatus: sorted\n---\ny\n",
    );
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let diff = store.propose(rename("bois", "chauffage")).unwrap();
    assert_eq!(diff.changes.len(), 3, "two pieces and the file");
    assert_eq!(diff.action.target().to_string(), "tags/chauffage");
    store.accept(diff).unwrap();
    assert!(read(&tmp.0, "memories/M-0009.md").contains("tags: [chauffage]\n"));
    assert!(read(&tmp.0, "memories/M-0010.md").contains("tags: [chauffage]\n"));
    let file = read(&tmp.0, "tags/chauffage.md");
    assert!(
        file.contains("tag: chauffage\ndefinition: le bois de chauffage\naliases: [bois]\n"),
        "{file}"
    );
    assert!(!tmp.0.join("tags/bois.md").exists());
    let err = store.propose(rename("bois", "x")).unwrap_err();
    assert!(err.to_string().contains("no piece carries"), "{err}");
}

#[test]
fn a_rename_into_a_tag_with_a_file_merges_the_two_and_a_changed_file_refuses_it_whole() {
    let tmp = valid("tagmerge");
    write(&tmp.0, "tags/bois.md", BOIS);
    write(&tmp.0, "tags/chauffage.md", "---\ntag: chauffage\n---\n");
    write(
        &tmp.0,
        "memories/M-0009.md",
        "---\nid: M-0009\ntitle: x\ntags: [bois]\nstatus: sorted\n---\nx\n",
    );
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let stale = store.propose(rename("bois", "chauffage")).unwrap();
    write(
        &tmp.0,
        "memories/M-0009.md",
        "---\nid: M-0009\ntitle: x changed\ntags: [bois]\nstatus: sorted\n---\nx\n",
    );
    let err = store.accept(stale).unwrap_err();
    assert!(matches!(err, StoreError::Changed(_)), "{err}");
    assert!(tmp.0.join("tags/bois.md").exists(), "nothing written");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let diff = store.propose(rename("bois", "chauffage")).unwrap();
    assert!(
        diff.lines().iter().any(|l| l == "delete tags/bois.md"),
        "{:?}",
        diff.lines()
    );
    store.accept(diff).unwrap();
    let file = read(&tmp.0, "tags/chauffage.md");
    assert!(
        file.contains("definition: le bois de chauffage\naliases: [bois]"),
        "{file}"
    );
    assert!(!tmp.0.join("tags/bois.md").exists());
    let text = "action: rename_tag\nfrom: bois\nto: chauffage\nwhy: un seul mot\n";
    assert!(matches!(
        serde_yaml_ng::from_str::<Action>(text).unwrap(),
        Action::RenameTag { .. }
    ));
}
