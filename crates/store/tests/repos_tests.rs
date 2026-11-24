//! The repositories of VALIDATION.md § 3 (C-10.3 – C-10.11, C-5.5, C-5.6), over temporary folders
//! holding two git repositories, the inner one ignored by the outer, or two plain folders.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use lorisse_store::{
    Access, Action, Applied, Certainty, CommitState, Context, DirectWrite, Fields, Filter, Flags,
    IDS_FILE, Layer, PieceId, Ref, Role, Since, Source, Store, StoreError, Value, WriteMode,
};

// =============================================================================
// Helpers
// =============================================================================

/// A unique, empty folder under the temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let name = format!("lorisse_store_repos_{tag}_{}_{nanos}", std::process::id());
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

/// `git <args>` in `dir`; its output, the test failing on an error.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "git {args:?}: {said}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

fn memory(id: &str) -> String {
    format!(
        "---\nid: {id}\ntitle: Memory {id}\nkind: idea\ntags: []\nsource: chat\n\
         captured: 2026-10-13T22:30:00+02:00\nupdated: 2026-10-13\nstatus: sorted\n\
         links: []\n---\nBody of {id}.\n"
    )
}

/// `dir` made a repository of `files`, one commit, an identity of its own.
fn init(dir: &Path, files: &[(&str, &str)], who: &str) {
    for (rel, text) in files {
        write(dir, rel, text);
    }
    git(dir, &["init", "-q"]);
    for (key, value) in [
        ("user.name", who),
        ("user.email", &format!("{}@local", who.to_lowercase())),
        ("core.autocrlf", "false"),
        ("commit.gpgsign", "false"),
    ] {
        git(dir, &["config", key, value]);
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "start"]);
}

/// The data folder, a repository listing `rows`, with `M-0002` at the root; the inner
/// repository `folder` holding `M-0001`.
fn two(tag: &str, rows: &str, folder: &str) -> TempDir {
    let tmp = TempDir::new(tag);
    let config = format!("sources: [app, chat]\nrepos:\n{rows}");
    let root = [
        ("config.yaml", config.as_str()),
        (".gitignore", "index.json\nids.json\n"),
        ("memories/M-0002.md", &memory("M-0002")),
    ];
    init(&tmp.0, &root, "Owner");
    init(
        &tmp.0.join(folder),
        &[("memories/M-0001.md", &memory("M-0001"))],
        "Work",
    );
    tmp
}

const FAMILLE: &str = "  - { name: famille, folder: famille }\n";

fn id(text: &str) -> PieceId {
    PieceId::parse(text).unwrap()
}

fn context() -> Context {
    Context {
        write_mode: WriteMode::Direct,
        certainty: Certainty::Sure,
        why: "repos".into(),
        job: "keeper".into(),
        model: "claude-fable-5-1".into(),
        cost_usd: 0.002,
        first_of_turn: true,
        ..Context::default()
    }
}

fn edit(target: &str) -> Action {
    let mut set = Fields::new();
    set.insert(Value::from("title"), Value::from("Edited"));
    Action::Edit {
        id: id(target),
        set,
        unset: Vec::new(),
        body: None,
    }
}

fn direct(applied: Applied) -> DirectWrite {
    match applied {
        Applied::Direct(written) => *written,
        other => panic!("not written at once: {other:?}"),
    }
}

fn made(state: &CommitState) -> String {
    match state {
        CommitState::Made(id) => id.clone(),
        other => panic!("not committed: {other:?}"),
    }
}

fn lorisse_log(dir: &Path) -> Vec<String> {
    let out = git(dir, &["log", "--author=<lorisse@local>", "--format=%H %s"]);
    out.lines().map(String::from).collect()
}

fn subjects(dir: &Path) -> Vec<String> {
    git(dir, &["log", "--format=%s"])
        .lines()
        .map(String::from)
        .collect()
}

// =============================================================================
// C-10.3 – C-10.10: the repositories
// =============================================================================

/// C-10.3, C-10.5, C-10.10: a listed row is read, its folder written in the root's `.gitignore` once, one
/// commit of that line; a row absent here is reported, not an error.
#[test]
fn a_listed_repository_is_ignored_by_the_root_once_and_an_absent_one_reported() {
    let rows = format!(
        "{FAMILLE}  - {{ name: loin, folder: loin, role: work, machines: [nowhere-at-all] }}\n"
    );
    let tmp = two("ignore", &rows, "famille");
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let ignore = read(&tmp.0, ".gitignore");
    assert!(
        ignore.contains("famille/\n") && ignore.contains("loin/\n"),
        "{ignore}"
    );
    let said = subjects(&tmp.0);
    assert!(
        said.contains(&"gitignore: the repository famille".to_string()),
        "{said:?}"
    );
    assert!(
        said.contains(&"gitignore: the repository loin".to_string()),
        "{said:?}"
    );
    Store::open(&tmp.0, Access::Full).unwrap();
    assert_eq!(subjects(&tmp.0).len(), said.len(), "written once");
    let repos = store.repos();
    let names: Vec<&str> = repos.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names[1..], ["famille", "loin"]);
    assert!(repos[0].folder == "." && repos[0].git && repos[0].role == Role::Personal);
    assert!(repos[1].present && repos[1].git && repos[1].here);
    assert!(!repos[2].present && !repos[2].here && repos[2].role == Role::Work);
    let inner = store.list(&Filter::default());
    let m1 = inner.iter().find(|e| e.id == id("M-0001")).unwrap();
    assert_eq!(m1.repo.as_deref(), Some("famille"));
    assert!(!m1.confidential);
}

/// C-10.6, C-12.10: a direct write in the inner repository is committed there, never in the
/// outer; Undo reverts it there.
#[test]
fn a_piece_written_in_the_inner_repository_is_committed_and_undone_there() {
    let tmp = two("inner", FAMILLE, "famille");
    let inner = tmp.0.join("famille");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let before = read(&inner, "memories/M-0001.md");
    let written = direct(store.apply(edit("M-0001"), &context()).unwrap());
    let commit = made(&written.commit);
    assert!(written.also.is_empty());
    assert_eq!(lorisse_log(&inner).len(), 1);
    assert!(
        lorisse_log(&tmp.0).is_empty(),
        "nothing of hers in the outer"
    );
    assert!(git(&inner, &["show", "--stat", "--format=", &commit]).contains("memories/M-0001.md"));
    let listed = store.lorisse_commits(&Since::All).unwrap();
    assert_eq!(listed.len(), 1);
    store.undo(&commit).unwrap();
    assert_eq!(read(&inner, "memories/M-0001.md"), before);
    assert!(subjects(&inner)[0].starts_with("Revert"));
    assert!(store.lorisse_commits(&Since::All).unwrap()[0].undone);
    assert!(git(&inner, &["status", "--porcelain"]).trim().is_empty());
}

/// C-10.6: an action touching two repositories is two commits, the piece's first, the other
/// naming it for the cost; Undo of the first reverts its repository only.
#[test]
fn a_write_across_two_repositories_is_one_commit_in_each() {
    let tmp = two("across", FAMILLE, "famille");
    let inner = tmp.0.join("famille");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let dedupe = Action::Dedupe {
        keep: id("M-0001"),
        drop: id("M-0002"),
    };
    let written = direct(store.apply(dedupe, &context()).unwrap());
    let first = made(&written.commit);
    assert_eq!(written.also.len(), 1);
    assert_eq!(lorisse_log(&inner).len(), 1);
    assert_eq!(lorisse_log(&tmp.0).len(), 1);
    assert!(lorisse_log(&inner)[0].starts_with(&first));
    let body = git(
        &tmp.0,
        &["log", "-1", "--author=<lorisse@local>", "--format=%B"],
    );
    assert!(
        body.contains(&format!("cost: in {}", &first[..7])),
        "{body}"
    );
    assert_eq!(store.lorisse_commits(&Since::All).unwrap().len(), 2);
    let since = store
        .lorisse_commits(&Since::Commit(first.clone()))
        .unwrap();
    assert!(since.iter().all(|c| c.id != first));
    assert!(
        store
            .check()
            .unwrap()
            .findings
            .iter()
            .all(|f| !f.text.contains("Lorisse"))
    );
    store.undo(&first).unwrap();
    assert!(read(&tmp.0, "memories/M-0002.md").contains("status: retired"));
    assert!(!read(&inner, "memories/M-0001.md").contains("confidence: low"));
}

/// C-10.4, C-10.7, C-9.1: a confidential row gates its pieces by its flag, routes them by
/// proposal, and the owner's Fine there commits in it with the row's identity.
#[test]
fn a_confidential_row_is_gated_by_its_flag_and_commits_with_its_identity() {
    let row = "  - { name: acme, folder: acme, role: work, confidential: true, key: work, git_name: Work Key, git_email: work@corp }\n";
    let tmp = two("acme", row, "acme");
    let inner = tmp.0.join("acme");
    let free = Store::open(&tmp.0, Access::Free).unwrap();
    assert!(matches!(
        free.read(&Ref::from(id("M-0001"))),
        Err(StoreError::Confidential(_))
    ));
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let entry = store
        .list(&Filter::default())
        .into_iter()
        .find(|e| e.id == id("M-0001"))
        .cloned()
        .unwrap();
    assert!(entry.confidential && entry.repo.as_deref() == Some("acme"));
    let applied = store.apply(edit("M-0001"), &context()).unwrap();
    assert!(matches!(applied, Applied::Proposed { .. }), "{applied:?}");
    let fined = store.fine(&id("M-0001")).unwrap();
    let commit = made(&fined.commit);
    let who = git(&inner, &["log", "-1", "--format=%an <%ae>", &commit]);
    assert_eq!(who.trim(), "Work Key <work@corp>");
    assert_eq!(store.repos()[1].key.as_deref(), Some("work"));
}

/// C-10.8: `confidential/`, unlisted, is a confidential repository; one of its own takes its
/// commits, one without stays in the root's as before.
#[test]
fn the_confidential_folder_stays_an_alias_with_its_own_repository_or_the_roots() {
    let tmp = TempDir::new("alias");
    let root = [
        ("config.yaml", "sources: [app, chat]\n"),
        (".gitignore", "index.json\nids.json\nconfidential/\n"),
    ];
    init(&tmp.0, &root, "Owner");
    let secret = tmp.0.join("confidential");
    init(
        &secret,
        &[("memories/M-0001.md", &memory("M-0001"))],
        "Work",
    );
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let entry = store.list(&Filter::default())[0].clone();
    assert!(entry.confidential && entry.repo.as_deref() == Some("confidential"));
    assert_eq!(
        read(&tmp.0, ".gitignore"),
        "index.json\nids.json\nconfidential/\n"
    );
    let fined = store.fine(&id("M-0001")).unwrap();
    let who = git(
        &secret,
        &["log", "-1", "--format=%an", &made(&fined.commit)],
    );
    assert_eq!(who.trim(), "Work");
    assert_eq!(
        store.repos().last().map(|r| r.name.as_str()),
        Some("confidential")
    );
}

/// C-10.9: a repository mounted by a junction at the root is walked as a folder.
#[cfg(windows)]
#[test]
fn a_repository_mounted_by_a_junction_is_indexed() {
    let tmp = TempDir::new("junction");
    let elsewhere = TempDir::new("junction_target");
    init(
        &tmp.0,
        &[(
            "config.yaml",
            "repos:\n  - { name: acme, folder: acme, confidential: true }\n",
        )],
        "Owner",
    );
    init(
        &elsewhere.0,
        &[("memories/M-0001.md", &memory("M-0001"))],
        "Work",
    );
    let link = tmp.0.join("acme");
    let out = Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&link)
        .arg(&elsewhere.0)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let ids: Vec<String> = store
        .list(&Filter::default())
        .iter()
        .map(|e| e.id.to_string())
        .collect();
    assert_eq!(ids, ["M-0001"]);
    assert!(store.repos()[1].git);
    let _ = fs::remove_dir(&link);
}

/// C-10.11: a row's `providers:` rides on each of its pieces' entries; the root's and a row
/// without the list carry none; a provider named twice is refused naming the row.
#[test]
fn a_row_s_providers_ride_on_its_pieces_entries_and_a_name_twice_is_refused() {
    let row =
        "  - { name: acme, folder: acme, confidential: true, providers: [anthropic, gemini] }\n";
    let tmp = two("providers", row, "acme");
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let entries = store.list(&Filter::default());
    let of = |text: &str| entries.iter().find(|e| e.id == id(text)).unwrap();
    assert_eq!(of("M-0001").providers, ["anthropic", "gemini"]);
    assert!(of("M-0002").providers.is_empty());
    let plain = two("noproviders", FAMILLE, "famille");
    let store = Store::open(&plain.0, Access::Full).unwrap();
    let entries = store.list(&Filter::default());
    assert!(entries.iter().all(|e| e.providers.is_empty()));
    let twice = "  - { name: acme, folder: acme, providers: [gemini, gemini] }\n";
    let bad = two("twice", twice, "acme");
    let error = Store::open(&bad.0, Access::Full).unwrap_err().to_string();
    assert!(
        error.contains("gemini twice") && error.contains("acme"),
        "{error}"
    );
}

// =============================================================================
// C-5.5, C-5.6: the ids per repository
// =============================================================================

/// The root and a confidential work row numbering its own pieces with `A`.
const WORK: &str = "  - { name: perso, folder: . }\n  - { name: acme, folder: acme, role: work, confidential: true, prefix: A }\n";

/// A data folder with no git listing `rows`, the folders `present` made.
fn plain(tag: &str, rows: &str, present: &[&str]) -> TempDir {
    let tmp = TempDir::new(tag);
    let config = format!("sources: [app, chat]\nrepos:\n{rows}");
    write(&tmp.0, "config.yaml", &config);
    for folder in present {
        fs::create_dir_all(tmp.0.join(folder)).unwrap();
    }
    tmp
}

fn sort_to(target: &PieceId, layer: Layer, confidential: bool) -> Action {
    let mut set = Fields::new();
    if layer == Layer::Memory {
        set.insert(Value::from("kind"), Value::from("idea"));
    }
    Action::Sort {
        id: target.clone(),
        layer,
        confidential,
        slice: None,
        set,
        sorted_by: Some("owner".into()),
    }
}

fn sorted(store: &mut Store, action: Action) -> Vec<PieceId> {
    let diff = store.propose(action).unwrap();
    store.accept(diff).unwrap()
}

fn findings(store: &Store) -> Vec<String> {
    let report = store.check().unwrap();
    report.findings.into_iter().map(|f| f.text).collect()
}

/// C-5.1, C-5.3, C-5.5: a personal sort gives `M-0001`, a work sort `A-0001`, each repository's
/// task its own next number; the root's `ids.json` never carries `A`, the row's holds it.
#[test]
fn two_sorts_in_two_repositories_give_m_0001_and_a_0001() {
    let tmp = plain("persort", WORK, &["acme"]);
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let home = store.capture("perso", Source::new("app")).unwrap();
    let home = home.id().unwrap();
    assert_eq!(home, id("M-0001"));
    sorted(&mut store, sort_to(&home, Layer::Memory, false));
    let no = Flags::default();
    let work = store
        .capture_in("acme", "work", Source::new("app"), &no, &[])
        .unwrap();
    let work = work.id().unwrap();
    assert_eq!(work, id("A-0001"));
    assert!(tmp.0.join("inbox/A-0001.md").exists());
    sorted(&mut store, sort_to(&work, Layer::Memory, true));
    assert!(tmp.0.join("acme/memories/A-0001.md").exists());
    let task = store
        .capture_in("acme", "a work task", Source::new("app"), &no, &[])
        .unwrap();
    let written = sorted(&mut store, sort_to(&task.id().unwrap(), Layer::Task, true));
    assert!(written.contains(&id("A-0003")), "{written:?}");
    assert!(tmp.0.join("acme/tasks/A-0003.md").exists());
    sorted(&mut store, Action::Archive { id: id("A-0003") });
    assert!(tmp.0.join("acme/tasks/archive/A-0003.md").exists());
    let archived = store.list(&Filter::default());
    let archived = archived.iter().find(|e| e.id == id("A-0003")).unwrap();
    assert!(archived.archived && archived.layer == Layer::Task);
    assert_eq!(store.next_id("T", None), id("T-0001"));
    assert_eq!(store.next_id("M", None), id("M-0002"));
    assert_eq!(store.next_id("T", Some("acme")), id("A-0004"));
    assert_eq!(read(&tmp.0, IDS_FILE), "{\n  \"M\": 1\n}\n");
    assert_eq!(read(&tmp.0, "acme/ids.json"), "{\n  \"A\": 3\n}\n");
    let unknown = store.capture_in("nowhere", "x", Source::new("app"), &no, &[]);
    assert!(
        matches!(unknown, Err(StoreError::Invalid(_))),
        "{unknown:?}"
    );
}

/// C-10.10: each repository's state carries its prefix; the root's and a row without one carry none.
#[test]
fn a_repository_s_state_carries_its_prefix() {
    let tmp = plain("perstate", &format!("{WORK}{FAMILLE}"), &["acme"]);
    let repos = Store::open(&tmp.0, Access::Full).unwrap().repos();
    let prefixes: Vec<(&str, Option<&str>)> = repos
        .iter()
        .map(|r| (r.folder.as_str(), r.prefix.as_deref()))
        .collect();
    assert_eq!(
        prefixes,
        [(".", None), ("acme", Some("A")), ("famille", None)]
    );
}

/// C-5.3, C-5.5: a row's counter is read from its folder and raised there, a freed number never
/// given again; an absent row's folder is never made.
#[test]
fn a_row_s_ids_json_is_raised_and_kept_in_its_own_folder() {
    let rows = format!("{WORK}  - {{ name: loin, folder: loin, prefix: L }}\n");
    let tmp = plain("perkeep", &rows, &["acme"]);
    write(&tmp.0, "acme/ids.json", "{\"A\": 12}");
    write(&tmp.0, "acme/memories/A-0003.md", &memory("A-0003"));
    write(&tmp.0, "acme/memories/M-0040.md", &memory("M-0040"));
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    assert_eq!(store.next_id("M", Some("acme")), id("A-0013"));
    assert_eq!(
        store.next_id("M", None),
        id("M-0001"),
        "an old M- of the row counts there only"
    );
    let no = Flags::default();
    let piece = store
        .capture_in("acme", "w", Source::new("app"), &no, &[])
        .unwrap();
    assert_eq!(piece.id(), Some(id("A-0013")));
    fs::remove_file(tmp.0.join("inbox/A-0013.md")).unwrap();
    let store = Store::open(&tmp.0, Access::Free).unwrap();
    assert_eq!(store.next_id("M", Some("acme")), id("A-0014"));
    assert_eq!(read(&tmp.0, "acme/ids.json"), "{\n  \"A\": 13\n}\n");
    assert!(
        !tmp.0.join(IDS_FILE).exists(),
        "nothing numbered at the root"
    );
    assert!(
        !tmp.0.join("loin").exists(),
        "an absent row's folder is never made"
    );
}

/// C-5.6: an id twice inside one repository is a finding; the same number in two repositories
/// is two ids, both listed; a row without a prefix shares the root's numbering and its rule.
#[test]
fn an_id_twice_in_one_repository_is_flagged_and_the_same_number_in_two_is_not() {
    let rows = format!("{WORK}{FAMILLE}");
    let tmp = plain("pertwice", &rows, &["acme", "famille"]);
    write(&tmp.0, "memories/M-0305.md", &memory("M-0305"));
    write(&tmp.0, "acme/memories/M-0305.md", &memory("M-0305"));
    write(&tmp.0, "acme/memories/A-0001.md", &memory("A-0001"));
    write(&tmp.0, "acme/work/memories/A-0001.md", &memory("A-0001"));
    write(&tmp.0, "memories/M-0002.md", &memory("M-0002"));
    write(&tmp.0, "famille/memories/M-0002.md", &memory("M-0002"));
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    let said = findings(&store);
    let twice = |text: &str| {
        said.iter()
            .filter(|f| f.contains(&format!("id {text} used twice")))
            .count()
    };
    assert_eq!(twice("M-0305"), 0, "{said:?}");
    assert_eq!(twice("A-0001"), 2, "{said:?}");
    assert_eq!(twice("M-0002"), 2, "{said:?}");
    let listed = store.list(&Filter::default());
    let m305: Vec<_> = listed.iter().filter(|e| e.id == id("M-0305")).collect();
    assert_eq!(m305.len(), 2);
    assert!(
        listed
            .iter()
            .all(|e| e.id != id("A-0001") && e.id != id("M-0002"))
    );
}
