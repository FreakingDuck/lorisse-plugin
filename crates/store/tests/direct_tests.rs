//! The direct route of VALIDATION.md § 3 (C-12), over temporary folders that are real git repositories.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use lorisse_store::{
    Access, Action, Applied, Certainty, CommitState, Context, DirectWrite, Fields, Kind, Layer,
    PieceId, Ref, Route, Since, Source, Store, StoreError, Value, WriteMode,
};

// =============================================================================
// Helpers
// =============================================================================

/// A unique, empty folder under the temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let name = format!("lorisse_store_direct_{tag}_{}_{nanos}", std::process::id());
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

/// `git <args>` in `root`; its output, the test failing on an error.
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
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

/// A memory in the house style, linking `links`.
fn memory(id: &str, links: &str) -> String {
    format!(
        "---\nid: {id}\ntitle: Memory {id}\nkind: idea\ntags: []\nsource: chat\n\
         captured: 2026-10-13T22:30:00+02:00\nupdated: 2026-10-13\nstatus: sorted\n\
         links: [{links}]\n---\nBody of {id}.\n"
    )
}

/// A data folder that is a git repository, its identity the owner's for the test only.
fn repo(tag: &str, files: &[(&str, &str)]) -> TempDir {
    let tmp = TempDir::new(tag);
    let root = &tmp.0;
    write(
        root,
        "config.yaml",
        "sources: [app, chat, agent, todoist, mail]\n",
    );
    write(root, ".gitignore", "index.json\n");
    for (rel, text) in files {
        write(root, rel, text);
    }
    git(root, &["init", "-q"]);
    for (key, value) in [
        ("user.name", "Owner"),
        ("user.email", "owner@local"),
        ("core.autocrlf", "false"),
        ("commit.gpgsign", "false"),
    ] {
        git(root, &["config", key, value]);
    }
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "start"]);
    tmp
}

/// Every file under `root/<rel>` with its bytes; empty when the folder is absent.
fn snapshot(root: &Path, rel: &str) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut all = BTreeMap::new();
    let mut stack = vec![root.join(rel)];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
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

fn id(text: &str) -> PieceId {
    PieceId::parse(text).unwrap()
}

/// The context of a sure job on a slice written at once.
fn context(why: &str) -> Context {
    Context {
        write_mode: WriteMode::Direct,
        certainty: Certainty::Sure,
        why: why.into(),
        job: "keeper".into(),
        provider: "anthropic".into(),
        model: "claude-fable-5-1".into(),
        cost_usd: 0.002,
        ..Context::default()
    }
}

fn milk_task(capture: &PieceId) -> Action {
    let mut set = Fields::new();
    set.insert(Value::from("title"), Value::from("Acheter du lait"));
    set.insert(Value::from("due"), Value::from("2026-10-23"));
    set.insert(
        Value::from("tags"),
        Value::Sequence(vec![Value::from("courses")]),
    );
    Action::Sort {
        id: capture.clone(),
        layer: Layer::Task,
        confidential: false,
        slice: None,
        set,
        sorted_by: Some("keeper".into()),
    }
}

fn edit(target: &str, key: &str, value: &str) -> Action {
    let mut set = Fields::new();
    set.insert(Value::from(key), Value::from(value));
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

fn commit_id(written: &DirectWrite) -> String {
    match &written.commit {
        CommitState::Made(id) => id.clone(),
        other => panic!("not committed: {other:?}"),
    }
}

/// The lines of `git log` of Lorisse's commits in `format`.
fn lorisse_log(root: &Path, format: &str) -> Vec<String> {
    let format = format!("--format={format}");
    let out = git(root, &["log", "--author=<lorisse@local>", &format]);
    out.lines().map(String::from).collect()
}

/// A raw capture typed in the app, the sentence of done-when 1; the store opened after it.
fn milk(tag: &str, source: &str) -> (TempDir, Store, PieceId) {
    let tmp = repo(tag, &[]);
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let piece = store
        .capture("acheter du lait demain", Source::new(source))
        .unwrap();
    let capture = piece.id().unwrap();
    (tmp, store, capture)
}

// =============================================================================
// C-12: the route
// =============================================================================

/// Done-when 1: a typed capture becomes a task, `due` set, `confidence: low`, the capture
/// retired and linked, in one commit of Lorisse, with no accept.
#[test]
fn a_typed_capture_becomes_a_task_in_one_commit_of_lorisse_without_an_accept() {
    let (tmp, mut store, capture) = milk("milk", "app");
    let root = &tmp.0;
    let mut ctx = context("une course pour demain");
    ctx.capture = Some(capture.clone());
    ctx.origin = Some("app".into());
    let written = direct(store.apply(milk_task(&capture), &ctx).unwrap());
    assert_eq!(written.routing.route, Route::Direct);
    let task = read(root, "tasks/T-0001.md");
    assert!(task.contains("due: 2026-10-23\n"), "{task}");
    assert!(task.contains("capture: M-0001\n"), "{task}");
    assert!(task.contains("confidence: low\n"), "{task}");
    let retired = read(root, "memories/M-0001.md");
    assert!(retired.contains("status: retired\n"), "{retired}");
    assert!(retired.contains("links: [T-0001]\n"), "{retired}");
    assert!(!root.join("inbox/M-0001.md").exists());
    assert!(store.proposal_files().unwrap().is_empty());
    let id = commit_id(&written);
    assert_eq!(lorisse_log(root, "%H"), std::slice::from_ref(&id));
    let who = git(root, &["log", "-1", "--format=%an <%ae>", &id]);
    assert_eq!(who.trim(), "Lorisse <lorisse@local>");
    let message = git(root, &["log", "-1", "--format=%B", &id]);
    assert!(message.starts_with("sort(M-0001, T-0001, tags/courses): une course pour demain\n\njob: keeper\ncertainty: sure\nsource: M-0001\nmodel: claude-fable-5-1  cost: 0.0020 USD"), "{message}");
    let files = git(
        root,
        &["show", "--name-only", "--no-renames", "--format=", &id],
    );
    let mut files: Vec<&str> = files.lines().filter(|l| !l.is_empty()).collect();
    files.sort();
    let want = [
        "inbox/M-0001.md",
        "memories/M-0001.md",
        "tags/courses.md",
        "tasks/T-0001.md",
    ];
    assert_eq!(files, want);
    let base = git(root, &["log", "-1", "--format=%ae %s", &format!("{id}~1")]);
    assert!(base.starts_with("owner@local files as they were"), "{base}");
    assert!(
        store.check().unwrap().is_empty(),
        "{}",
        store.check().unwrap()
    );
}

/// Done-when 2: the same sentence from a forwarded mail, or from an origin the caller names,
/// is a proposal file, never a task.
#[test]
fn an_external_source_goes_to_a_proposal_never_to_a_task() {
    let (tmp, mut store, capture) = milk("mail", "mail");
    let root = &tmp.0;
    let mut ctx = context("une course");
    ctx.capture = Some(capture.clone());
    let applied = store.apply(milk_task(&capture), &ctx).unwrap();
    let Applied::Proposed { path, routing } = applied else {
        panic!("{applied:?}");
    };
    assert!(
        routing.reason.contains("external source (mail)"),
        "{}",
        routing.reason
    );
    assert_eq!(path, PathBuf::from("inbox/M-0001.sort.yaml"));
    let file = read(root, "inbox/M-0001.sort.yaml");
    let head = "# why: une course\n# by: keeper anthropic claude-fable-5-1\n# certainty: sure\naction: sort\n";
    assert!(file.starts_with(head), "{file}");
    assert!(!root.join("tasks").exists());
    assert!(lorisse_log(root, "%H").is_empty());
    let (_typed, mut typed_store, typed) = milk("todoist", "app");
    let mut ctx = context("une course");
    ctx.origin = Some("todoist".into());
    let applied = typed_store.apply(milk_task(&typed), &ctx).unwrap();
    assert!(matches!(applied, Applied::Proposed { .. }), "{applied:?}");
}

/// Done-when 3: undo on that commit restores the inbox exactly as it was.
#[test]
fn undo_restores_the_inbox_exactly_as_before() {
    let (tmp, mut store, capture) = milk("undo", "app");
    let root = &tmp.0;
    let before = snapshot(root, "inbox");
    let mut ctx = context("une course");
    ctx.capture = Some(capture.clone());
    let written = direct(store.apply(milk_task(&capture), &ctx).unwrap());
    let commit = commit_id(&written);
    let revert = store.undo(&commit).unwrap();
    assert_ne!(revert, commit);
    assert_eq!(snapshot(root, "inbox"), before);
    assert!(snapshot(root, "tasks").is_empty());
    assert!(!root.join("memories/M-0001.md").exists());
    assert_eq!(store.inbox().len(), 1);
    let listed = store.lorisse_commits(&Since::All).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].undone);
    let again = store.undo(&commit).unwrap_err();
    assert!(again.to_string().contains("undone already"), "{again}");
}

/// C-5.3: the task an undo removed keeps its number; the same sort again makes the next one.
#[test]
fn a_sort_undone_then_made_again_gets_a_new_id_never_the_freed_one() {
    let (tmp, mut store, capture) = milk("reuse", "app");
    let root = &tmp.0;
    let mut ctx = context("une course");
    ctx.capture = Some(capture.clone());
    let first = direct(store.apply(milk_task(&capture), &ctx).unwrap());
    assert!(root.join("tasks/T-0001.md").exists());
    store.undo(&commit_id(&first)).unwrap();
    assert!(!root.join("tasks/T-0001.md").exists());
    let mut store = Store::open(root, Access::Full).unwrap();
    assert_eq!(store.next_id("T", None), id("T-0002"));
    direct(store.apply(milk_task(&capture), &ctx).unwrap());
    assert!(root.join("tasks/T-0002.md").exists());
    assert!(!root.join("tasks/T-0001.md").exists());
}

/// Done-when 4: a dedupe undone restores both pieces and every link it rewrote.
#[test]
fn a_dedupe_undone_restores_both_pieces_and_every_link() {
    let files = [
        ("memories/M-0001.md", memory("M-0001", "")),
        ("memories/M-0002.md", memory("M-0002", "")),
        ("memories/M-0003.md", memory("M-0003", "M-0002")),
        ("memories/M-0004.md", memory("M-0004", "M-0002")),
        ("memories/M-0005.md", memory("M-0005", "M-0002")),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(r, t)| (*r, t.as_str())).collect();
    let tmp = repo("dedupe", &files);
    let root = &tmp.0;
    let before = snapshot(root, "memories");
    let mut store = Store::open(root, Access::Full).unwrap();
    let dedupe = Action::Dedupe {
        keep: id("M-0001"),
        drop: id("M-0002"),
    };
    let mut ctx = context("deux fois la même");
    ctx.question = Some("Est-ce bien la même chose ?".into());
    let written = direct(store.apply(dedupe, &ctx).unwrap());
    assert_eq!(written.routing.route, Route::DirectFlagged);
    for other in ["M-0003", "M-0004", "M-0005"] {
        let text = read(root, &format!("memories/{other}.md"));
        assert!(text.contains("links: [M-0001]\n"), "{text}");
    }
    assert!(read(root, "memories/M-0002.md").contains("status: retired\n"));
    let kept = read(root, "memories/M-0001.md");
    assert!(
        kept.contains("to_check: Est-ce bien la même chose ?\n"),
        "{kept}"
    );
    let flagged = store.to_check().unwrap();
    assert_eq!(
        flagged,
        [(id("M-0001"), "Est-ce bien la même chose ?".to_string())]
    );
    store.undo(&commit_id(&written)).unwrap();
    assert_eq!(snapshot(root, "memories"), before);
    assert!(store.to_check().unwrap().is_empty());
}

/// Done-when 5 and 6: a shared slice, `confidential/` and the cap reached write proposals only.
#[test]
fn a_slice_by_proposal_confidential_or_the_cap_reached_writes_proposals_only() {
    let (tmp, mut store, capture) = milk("slices", "app");
    let root = &tmp.0;
    let mut shared = context("une course");
    shared.write_mode = WriteMode::Proposal;
    let routing = store.route(&milk_task(&capture), &shared).unwrap();
    assert_eq!(routing.route, Route::Proposal);
    assert_eq!(routing.reason, "the slice writes by proposal");
    let mut capped = context("une course");
    capped.cap_reached = true;
    let routing = store.route(&milk_task(&capture), &capped).unwrap();
    assert_eq!(routing.reason, "the daily cap is reached");
    let mut secret = milk_task(&capture);
    if let Action::Sort { confidential, .. } = &mut secret {
        *confidential = true;
    }
    let applied = store.apply(secret, &context("une course")).unwrap();
    let Applied::Proposed { routing, .. } = applied else {
        panic!("{applied:?}");
    };
    assert!(
        routing.reason.contains("confidential/"),
        "{}",
        routing.reason
    );
    assert!(!root.join("confidential").exists());
    assert!(lorisse_log(root, "%H").is_empty());
}

/// Done-when 7: Lorisse never writes `checked` or `validated`; nothing is written then.
#[test]
fn lorisse_never_writes_checked_or_validated() {
    let tmp = repo(
        "validated",
        &[("memories/M-0001.md", &memory("M-0001", ""))],
    );
    let root = &tmp.0;
    let mut store = Store::open(root, Access::Full).unwrap();
    let before = snapshot(root, "memories");
    for word in ["validated", "checked"] {
        let refused = edit("M-0001", "confidence", word);
        let err = store.apply(refused, &context("sûr")).unwrap_err();
        assert!(matches!(err, StoreError::Invalid(_)), "{err}");
        assert!(err.to_string().contains(word), "{err}");
    }
    assert_eq!(snapshot(root, "memories"), before);
    let low = edit("M-0001", "confidence", "low");
    direct(store.apply(low, &context("bas")).unwrap());
}

/// `low` lands on a piece the action makes or edits; a piece only relinked keeps its own.
#[test]
fn a_validated_piece_only_relinked_keeps_validated_and_an_edited_one_drops_to_low() {
    let validated = |id: &str, links: &str| {
        memory(id, links).replace("\n---\nBody", "\nconfidence: validated\n---\nBody")
    };
    let files = [
        ("memories/M-0001.md", validated("M-0001", "")),
        ("memories/M-0002.md", memory("M-0002", "")),
        ("memories/M-0003.md", validated("M-0003", "M-0002")),
        ("memories/M-0004.md", validated("M-0004", "")),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(r, t)| (*r, t.as_str())).collect();
    let tmp = repo("relink", &files);
    let root = &tmp.0;
    let mut store = Store::open(root, Access::Full).unwrap();
    let link = Action::Link {
        from: id("M-0004"),
        to: vec![Ref::new("M-0001")],
    };
    direct(store.apply(link, &context("liés")).unwrap());
    let linked = read(root, "memories/M-0004.md");
    assert!(linked.contains("links: [M-0001]\n"), "{linked}");
    assert!(linked.contains("confidence: validated\n"), "{linked}");
    let dedupe = Action::Dedupe {
        keep: id("M-0001"),
        drop: id("M-0002"),
    };
    direct(store.apply(dedupe, &context("la même")).unwrap());
    let relinked = read(root, "memories/M-0003.md");
    assert!(relinked.contains("links: [M-0001]\n"), "{relinked}");
    assert!(relinked.contains("confidence: validated\n"), "{relinked}");
    assert!(read(root, "memories/M-0002.md").contains("confidence: low\n"));
    let edited = edit("M-0004", "title", "Autre titre");
    direct(store.apply(edited, &context("renommé")).unwrap());
    let renamed = read(root, "memories/M-0004.md");
    assert!(renamed.contains("confidence: low\n"), "{renamed}");
    assert!(
        store.check().unwrap().is_empty(),
        "{}",
        store.check().unwrap()
    );
}

/// Her question on a task and its answers are her marks, not its content: the owner's confidence stays.
#[test]
fn her_question_on_a_validated_task_keeps_validated_and_a_new_urgency_drops_it_to_low() {
    let task = "---\nid: T-0001\ntitle: Pneus\nstatus: todo\nurgency: week\nconfidence: validated\n---\nBody.\n";
    let tmp = repo("marks", &[("tasks/T-0001.md", task)]);
    let root = &tmp.0;
    let mut store = Store::open(root, Access::Full).unwrap();
    let mut set = Fields::new();
    set.insert(Value::from("question"), Value::from("Pneus : reçus ?"));
    let answers = ["Reçus", "Montés", "Plus tard"].map(Value::from).to_vec();
    set.insert(Value::from("answers"), Value::Sequence(answers));
    let asked = Action::Edit {
        id: id("T-0001"),
        set,
        unset: Vec::new(),
        body: None,
    };
    direct(store.apply(asked, &context("sa question")).unwrap());
    let file = read(root, "tasks/T-0001.md");
    assert!(
        file.contains("reçus ?")
            && file.contains("Plus tard")
            && file.contains("confidence: validated\n"),
        "{file}"
    );
    let later = edit("T-0001", "urgency", "now");
    direct(store.apply(later, &context("plus tard")).unwrap());
    assert!(read(root, "tasks/T-0001.md").contains("confidence: low\n"));
}

/// The Memory room's Fine: `validated` in the owner's name, one commit of his; a job writing it is still refused.
#[test]
fn the_owners_fine_writes_validated_in_one_commit_of_his_and_a_job_writing_validated_is_still_refused()
 {
    let flagged = memory("M-0001", "").replace("\n---\nBody", "\nto_check: Quel Paul ?\n---\nBody");
    let tmp = repo(
        "fine",
        &[
            ("memories/M-0001.md", &flagged),
            ("people/paul.md", "---\nname: Paul\n---\nx\n"),
            ("tags/lait.md", "---\ntag: lait\n---\n"),
        ],
    );
    let root = &tmp.0;
    let mut store = Store::open(root, Access::Full).unwrap();
    let job = edit("M-0001", "confidence", "validated");
    let err = store.apply(job, &context("validé")).unwrap_err();
    assert!(err.to_string().contains("validated"), "{err}");
    let fined = store.fine(&id("M-0001")).unwrap();
    let CommitState::Made(commit) = &fined.commit else {
        panic!("{:?}", fined.commit);
    };
    let file = read(root, "memories/M-0001.md");
    assert!(
        file.contains("confidence: validated\n") && !file.contains("to_check"),
        "{file}"
    );
    let author = git(root, &["log", "-1", "--format=%ae %s", commit]);
    assert_eq!(
        author.trim(),
        "owner@local fine(M-0001): validated by the owner"
    );
    assert!(lorisse_log(root, "%H").is_empty(), "never Lorisse's");
    assert!(store.to_check().unwrap().is_empty());
    assert!(store.fine(&id("people/paul")).is_ok(), "a person takes it");
    let tag = store.fine(&id("tags/lait")).unwrap_err();
    assert!(matches!(tag, StoreError::Invalid(_)), "{tag}");
    assert!(matches!(
        store.fine(&id("M-0099")),
        Err(StoreError::Unknown(_))
    ));
    assert!(
        store.check().unwrap().is_empty(),
        "{}",
        store.check().unwrap()
    );
}

/// Her label is her mark: a validated piece labelled stays validated; her answers sit beside her question.
#[test]
fn her_label_keeps_the_owners_confidence_and_her_proposed_answers_sit_beside_her_question() {
    let validated =
        memory("M-0001", "").replace("\n---\nBody", "\nconfidence: validated\n---\nBody");
    let tmp = repo("label", &[("memories/M-0001.md", &validated)]);
    let root = &tmp.0;
    let mut store = Store::open(root, Access::Full).unwrap();
    direct(
        store
            .apply(
                edit("M-0001", "label", "Bureau debout"),
                &context("son nom"),
            )
            .unwrap(),
    );
    let file = read(root, "memories/M-0001.md");
    assert!(
        file.contains("label: Bureau debout\n") && file.contains("confidence: validated\n"),
        "{file}"
    );
    let unsure = Context {
        certainty: Certainty::Unsure,
        question: Some("Quel bureau ?".into()),
        answers: vec!["le debout (70)".into(), "  ".into(), "l'ancien (20)".into()],
        ..context("pas sûre")
    };
    direct(
        store
            .apply(edit("M-0001", "kind", "need"), &unsure)
            .unwrap(),
    );
    let flagged = store.flagged().unwrap();
    assert_eq!(flagged.len(), 1);
    assert_eq!(flagged[0].question, "Quel bureau ?");
    assert_eq!(flagged[0].answers, ["le debout (70)", "l'ancien (20)"]);
    store.fine(&id("M-0001")).unwrap();
    let file = read(root, "memories/M-0001.md");
    assert!(!file.contains("check_answers"), "{file}");
}

// =============================================================================
// C-12: the table, the flags, the tags
// =============================================================================

#[test]
fn the_table_routes_each_action_and_the_facts_narrow_it() {
    let files = [
        ("memories/M-0001.md", memory("M-0001", "")),
        ("memories/M-0002.md", memory("M-0002", "")),
        ("people/ana.md", "---\nname: Ana\n---\n".to_string()),
        ("people/bob.md", "---\nname: Bob\n---\n".to_string()),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(r, t)| (*r, t.as_str())).collect();
    let tmp = repo("table", &files);
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let hello = store
        .capture("Bonjour !", Source::new("app"))
        .unwrap()
        .id()
        .unwrap();
    let errand = store
        .capture("le garage", Source::new("app"))
        .unwrap()
        .id()
        .unwrap();
    let ctx = context("x");
    let route = |action: Action, ctx: &Context| store.route(&action, ctx).unwrap().route;
    assert_eq!(
        route(
            Action::Retire {
                id: hello.clone(),
                behind: None
            },
            &ctx
        ),
        Route::Direct
    );
    assert_eq!(
        route(
            Action::Retire {
                id: errand,
                behind: None
            },
            &ctx
        ),
        Route::Proposal
    );
    let people = Action::Dedupe {
        keep: id("people/ana"),
        drop: id("people/bob"),
    };
    assert_eq!(route(people, &ctx), Route::Proposal);
    let archive = Action::Archive { id: id("M-0001") };
    assert!(store.route(&archive, &ctx).is_err(), "a memory is no task");
    let moved = Action::Move {
        id: id("M-0001"),
        layer: Some(Layer::Task),
        confidential: None,
        set: Fields::new(),
    };
    assert_eq!(route(moved, &ctx), Route::Proposal);
    let rename = Action::RenameTag {
        from: "x".into(),
        to: "y".into(),
        why: None,
    };
    assert!(store.route(&rename, &ctx).is_err(), "no piece carries x");
    let link = Action::Link {
        from: id("M-0001"),
        to: vec![Ref::new("M-0002")],
    };
    assert_eq!(route(link.clone(), &ctx), Route::Direct);
    let mut unsure = context("x");
    unsure.certainty = Certainty::Unsure;
    assert_eq!(route(link, &unsure), Route::DirectFlagged);
    let body = |text: &str| Action::Edit {
        id: id("M-0001"),
        set: Fields::new(),
        unset: Vec::new(),
        body: Some(text.into()),
    };
    assert_eq!(route(body("Tout autre.\n"), &ctx), Route::DirectFlagged);
    let mut note = context("x");
    note.kind = Some(Kind::Note);
    let added = body("Body of M-0001.\n- 2026-10-22: appelé\n");
    assert_eq!(store.route(&added, &note).unwrap().kind, Kind::Note);
    assert_eq!(route(added, &note), Route::Direct);
    assert_eq!(
        store.route(&body("Autre.\n"), &note).unwrap().kind,
        Kind::Rewrite
    );
    note.kind = Some(Kind::Note);
    let err = store.route(&milk_task(&hello), &note).unwrap_err();
    assert!(err.to_string().contains("a note is not a sort"), "{err}");
    let mut rule = context("x");
    rule.kind = Some(Kind::Rule);
    assert_eq!(route(milk_task(&hello), &rule), Route::Proposal);
}

#[test]
fn an_unsure_sort_is_written_and_flagged_with_her_question() {
    let (tmp, mut store, capture) = milk("unsure", "app");
    let mut ctx = context("une course ?");
    ctx.certainty = Certainty::Unsure;
    ctx.capture = Some(capture.clone());
    let written = direct(store.apply(milk_task(&capture), &ctx).unwrap());
    assert_eq!(written.routing.route, Route::DirectFlagged);
    let task = read(&tmp.0, "tasks/T-0001.md");
    assert!(task.contains("to_check: une course ?\n"), "{task}");
    let retired = read(&tmp.0, "memories/M-0001.md");
    assert!(!retired.contains("to_check"), "{retired}");
    assert_eq!(
        store.to_check().unwrap(),
        [(id("T-0001"), "une course ?".to_string())]
    );
    let message = git(&tmp.0, &["log", "-1", "--format=%B"]);
    assert!(message.contains("\ncertainty: unsure\n"), "{message}");
}

#[test]
fn a_new_tag_gets_a_provisional_file_and_a_known_one_none() {
    let files = [
        ("memories/M-0001.md", memory("M-0001", "")),
        (
            "tags/maison.md",
            "---\ntag: maison\ndefinition: la maison\n---\n".to_string(),
        ),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(r, t)| (*r, t.as_str())).collect();
    let tmp = repo("tags", &files);
    let root = &tmp.0;
    let mut store = Store::open(root, Access::Full).unwrap();
    let mut set = Fields::new();
    let tags = vec![Value::from("Maison"), Value::from("Jardin d'hiver")];
    set.insert(Value::from("tags"), Value::Sequence(tags));
    let action = Action::Edit {
        id: id("M-0001"),
        set,
        unset: Vec::new(),
        body: None,
    };
    let written = direct(store.apply(action, &context("deux pièces")).unwrap());
    assert!(written.ids.contains(&id("tags/jardin-d-hiver")));
    let made = read(root, "tags/jardin-d-hiver.md");
    let want = "---\ntag: Jardin d'hiver\ndefinition: \"\"\nprovisional: true\n---\n";
    assert_eq!(made, want);
    assert!(!root.join("tags/maison-2.md").exists());
    let listed = store.tags();
    let tag = listed.iter().find(|t| t.slug == "jardin-d-hiver").unwrap();
    assert!(tag.provisional && tag.definition.is_none());
    assert!(
        !listed
            .iter()
            .find(|t| t.slug == "maison")
            .unwrap()
            .provisional
    );
    store.tag_define("Jardin d'hiver", "la véranda").unwrap();
    assert!(!read(root, "tags/jardin-d-hiver.md").contains("provisional"));
}

// =============================================================================
// C-12: undo, since you last looked, no git
// =============================================================================

#[test]
fn undo_is_refused_while_a_later_commit_touched_the_same_file() {
    let (tmp, mut store, capture) = milk("later", "app");
    let mut ctx = context("une course");
    ctx.capture = Some(capture.clone());
    let first = commit_id(&direct(store.apply(milk_task(&capture), &ctx).unwrap()));
    let next = edit("T-0001", "next", "passer au marché");
    let second = commit_id(&direct(store.apply(next, &context("le marché")).unwrap()));
    let err = store.undo(&first).unwrap_err();
    let said = err.to_string();
    assert!(
        said.contains("tasks/T-0001.md changed in a later commit"),
        "{said}"
    );
    let start = git(&tmp.0, &["rev-list", "--max-parents=0", "HEAD"]);
    let err = store.undo(start.trim()).unwrap_err();
    assert!(
        err.to_string().contains("not a commit of Lorisse's"),
        "{err}"
    );
    store.undo(&second).unwrap();
    store.undo(&first).unwrap();
    assert!(!tmp.0.join("tasks/T-0001.md").exists());
}

#[test]
fn since_you_last_looked_lists_her_commits_newest_first_and_never_the_owners() {
    let (_tmp, mut store, capture) = milk("since", "app");
    let mut ctx = context("une course");
    ctx.capture = Some(capture.clone());
    let first = commit_id(&direct(store.apply(milk_task(&capture), &ctx).unwrap()));
    let next = edit("T-0001", "next", "le marché");
    let second = commit_id(&direct(store.apply(next, &context("le marché")).unwrap()));
    let all = store.lorisse_commits(&Since::All).unwrap();
    let ids: Vec<&str> = all.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, [second.as_str(), first.as_str()]);
    let newest = &all[0];
    assert_eq!(
        (newest.action.as_str(), newest.why.as_str()),
        ("edit", "le marché")
    );
    assert_eq!(newest.ids, ["T-0001"]);
    assert_eq!(
        (newest.job.as_str(), newest.certainty.as_str()),
        ("keeper", "sure")
    );
    assert_eq!(all[1].source, "M-0001");
    let after = store
        .lorisse_commits(&Since::Commit(first[..8].into()))
        .unwrap();
    assert_eq!(after.len(), 1);
    let today = chrono::Local::now().date_naive();
    assert_eq!(store.lorisse_commits(&Since::Date(today)).unwrap().len(), 2);
    let tomorrow = today.succ_opt().unwrap();
    assert!(
        store
            .lorisse_commits(&Since::Date(tomorrow))
            .unwrap()
            .is_empty()
    );
}

/// A turn's cost is on its first commit; the next ones of the turn point to it.
#[test]
fn a_turn_s_cost_is_on_its_first_commit_and_the_next_ones_say_where() {
    let (_tmp, mut store, capture) = milk("cost", "app");
    let mut ctx = context("une course");
    ctx.capture = Some(capture.clone());
    let first = commit_id(&direct(store.apply(milk_task(&capture), &ctx).unwrap()));
    let mut later = context("le marché");
    later.first_of_turn = false;
    let next = edit("T-0001", "next", "le marché");
    direct(store.apply(next.clone(), &later).unwrap());
    direct(
        store
            .apply(edit("T-0001", "next", "la ferme"), &later)
            .unwrap(),
    );
    let all = store.lorisse_commits(&Since::All).unwrap();
    let short = &first[..7];
    assert_eq!(all[2].cost, "0.0020 USD");
    assert_eq!(all[2].cost_in, None);
    for commit in &all[..2] {
        assert_eq!(
            (commit.cost.as_str(), commit.cost_in.as_deref()),
            ("", Some(short))
        );
    }
    let next_turn = commit_id(&direct(store.apply(next, &context("encore")).unwrap()));
    let newest = &store.lorisse_commits(&Since::All).unwrap()[0];
    assert_eq!(
        (newest.id.as_str(), newest.cost.as_str()),
        (next_turn.as_str(), "0.0020 USD")
    );
}

/// A greeting typed in the app is retired at once, even on a slice by proposal or past the cap.
#[test]
fn a_greeting_s_retire_is_direct_on_a_slice_by_proposal_and_past_the_cap() {
    let (tmp, mut store, _) = milk("hello", "app");
    let hello = store.capture("Bonjour !", Source::new("app")).unwrap();
    let hello = hello.id().unwrap();
    let mut ctx = context("une salutation");
    ctx.write_mode = WriteMode::Proposal;
    ctx.cap_reached = true;
    let retire = Action::Retire {
        id: hello.clone(),
        behind: None,
    };
    let written = direct(store.apply(retire, &ctx).unwrap());
    assert_eq!(written.routing.route, Route::Direct);
    assert!(store.inbox().iter().all(|e| e.id != hello));
    assert!(lorisse_log(&tmp.0, "%s")[0].starts_with("retire("));
    let errand = store.inbox()[0].id.clone();
    let routing = store
        .route(
            &Action::Retire {
                id: errand,
                behind: None,
            },
            &ctx,
        )
        .unwrap();
    assert_eq!(routing.route, Route::Proposal);
}

/// The noise rule: a capture of keyboard noise typed in the app retires at once, undone by a revert;
/// the same word from a mail stays a proposal.
#[test]
fn a_noise_capture_s_retire_is_direct_and_undone_and_a_mail_s_stays_a_proposal() {
    let (tmp, mut store, _) = milk("noise", "app");
    let stays = store
        .capture("stays", Source::new("app"))
        .unwrap()
        .id()
        .unwrap();
    let mut ctx = context("du bruit");
    ctx.write_mode = WriteMode::Proposal;
    let retire = Action::Retire {
        id: stays.clone(),
        behind: None,
    };
    let written = direct(store.apply(retire, &ctx).unwrap());
    assert_eq!(written.routing.route, Route::Direct);
    assert!(store.inbox().iter().all(|e| e.id != stays));
    store.undo(&commit_id(&written)).unwrap();
    let store = Store::open(&tmp.0, Access::Full).unwrap();
    assert!(
        store.inbox().iter().any(|e| e.id == stays),
        "back after undo"
    );
    let mut store = store;
    let mailed = store
        .capture("ok", Source::new("mail"))
        .unwrap()
        .id()
        .unwrap();
    let routing = store
        .route(
            &Action::Retire {
                id: mailed,
                behind: None,
            },
            &ctx,
        )
        .unwrap();
    assert_eq!(routing.route, Route::Proposal);
}

/// `retired_by`: a retire behind a résumé names it on the retired piece; behind itself or nothing, refused.
#[test]
fn a_retire_behind_a_resume_writes_retired_by_and_refuses_an_unknown_resume() {
    let tmp = repo(
        "retiredby",
        &[
            ("memories/M-0001.md", &memory("M-0001", "M-0002")),
            ("memories/M-0002.md", &memory("M-0002", "")),
        ],
    );
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let behind = |b: &str| Action::Retire {
        id: id("M-0002"),
        behind: Some(id(b)),
    };
    for bad in ["M-0002", "M-0009"] {
        let err = store.propose(behind(bad)).unwrap_err();
        assert!(err.to_string().contains("retired behind"), "{err}");
    }
    let diff = store.propose(behind("M-0001")).unwrap();
    store.accept(diff).unwrap();
    let text = read(&tmp.0, "memories/M-0002.md");
    assert!(
        text.contains("status: retired") && text.contains("retired_by: M-0001"),
        "{text}"
    );
    let yaml = "action: retire\nid: M-0002\nbehind: M-0001\n";
    let action: Action = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(action, behind("M-0001"));
}

/// `resolutions`: an edit setting them writes `answers` as their texts, keeps the task's confidence (her marks), and
/// a `does` outside answer · drop · archive is refused.
#[test]
fn an_edit_of_resolutions_derives_answers_keeps_the_confidence_and_refuses_an_unknown_does() {
    let task = "---\nid: T-0001\ntitle: L'inscription école\nstatus: todo\nconfidence: validated\n\
                ---\nÀ faire.\n";
    let tmp = repo("resolutions", &[("tasks/T-0001.md", task)]);
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let one = |text: &str, does: &str| {
        let mut m = Fields::new();
        m.insert(Value::from("text"), Value::from(text));
        m.insert(Value::from("does"), Value::from(does));
        Value::Mapping(m)
    };
    let edit = |list: Vec<Value>| {
        let mut set = Fields::new();
        set.insert(Value::from("question"), Value::from("Faite ?"));
        set.insert(Value::from("resolutions"), Value::Sequence(list));
        Action::Edit {
            id: id("T-0001"),
            set,
            unset: Vec::new(),
            body: None,
        }
    };
    let list = vec![
        one("Oui, faite", "archive"),
        one("Abandonner", "drop"),
        one("Plus tard", "answer"),
    ];
    direct(store.apply(edit(list), &context("sa question")).unwrap());
    let text = read(&tmp.0, "tasks/T-0001.md");
    assert!(
        text.contains("answers: [\"Oui, faite\", Abandonner, Plus tard]"),
        "{text}"
    );
    assert!(text.contains("confidence: validated"), "{text}");
    let piece = store.read(&Ref::new("T-0001")).unwrap();
    let does: Vec<&str> = piece.resolutions().iter().map(|r| r.does.word()).collect();
    assert_eq!(does, ["archive", "drop", "answer"]);
    let err = store
        .propose(edit(vec![one("Effacer", "delete")]))
        .unwrap_err();
    assert!(err.to_string().contains("delete"), "{err}");
    let archive = Action::Archive { id: id("T-0001") };
    let mut ctx = context("son abandon");
    ctx.write_mode = WriteMode::Proposal;
    let route = |store: &Store, tapped: Option<&str>| {
        let ctx = Context {
            tapped: tapped.map(String::from),
            ..ctx.clone()
        };
        store.route(&archive, &ctx).unwrap().route
    };
    assert_eq!(route(&store, None), Route::Proposal, "her archive waits");
    assert_eq!(
        route(&store, Some("Plus tard")),
        Route::Proposal,
        "an answer closes nothing"
    );
    assert_eq!(
        route(&store, Some("abandonner")),
        Route::Direct,
        "his tap on drop"
    );
    let ctx = Context {
        tapped: Some("Oui, faite".into()),
        ..ctx
    };
    let written = direct(store.apply(archive.clone(), &ctx).unwrap());
    assert!(written.routing.reason.contains("his tap"));
    assert!(
        !tmp.0.join("tasks/T-0001.md").exists(),
        "moved to the archive"
    );
}

#[test]
fn a_folder_without_git_is_written_and_says_not_committed() {
    let tmp = TempDir::new("nogit");
    let mut store = Store::open(&tmp.0, Access::Full).unwrap();
    let capture = store
        .capture("acheter du lait", Source::new("app"))
        .unwrap();
    let capture = capture.id().unwrap();
    let written = direct(
        store
            .apply(milk_task(&capture), &context("une course"))
            .unwrap(),
    );
    let CommitState::NotMade(why) = &written.commit else {
        panic!("{:?}", written.commit);
    };
    assert!(why.contains("not a git repository"), "{why}");
    assert!(tmp.0.join("tasks/T-0001.md").exists());
    assert!(!store.is_git());
    assert!(store.lorisse_commits(&Since::All).unwrap().is_empty());
    assert!(store.undo("HEAD").is_err());
}

// =============================================================================
// C-12: the check
// =============================================================================

/// The check reports a commit of Lorisse above `low`, one of two actions, one with no source link,
/// and one that is not an action at all.
#[test]
fn the_check_reports_her_commits_above_low_with_two_actions_or_without_a_source() {
    let task = |id: &str| {
        format!(
            "---\nid: {id}\ntitle: Task {id}\nstatus: todo\nupdated: 2026-10-13\nlinks: []\n---\n"
        )
    };
    let files = [
        ("memories/M-0001.md", memory("M-0001", "")),
        ("tasks/T-0005.md", task("T-0005")),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(r, t)| (*r, t.as_str())).collect();
    let tmp = repo("check", &files);
    let root = &tmp.0;
    let as_lorisse = |subject: &str| {
        git(root, &["add", "-A"]);
        let args = ["-c", "user.name=Lorisse", "-c", "user.email=lorisse@local"];
        let mut all: Vec<&str> = args.to_vec();
        all.extend(["commit", "-q", "-m", subject]);
        git(root, &all);
    };
    let above = task("T-0005").replace("links: []\n", "links: []\nconfidence: validated\n");
    write(root, "tasks/T-0005.md", &above);
    as_lorisse("edit(T-0005): validé");
    write(root, "tasks/T-0006.md", &task("T-0006"));
    write(root, "memories/M-0001.md", &memory("M-0001", "T-0006"));
    as_lorisse("sort(M-0001): deux actions");
    write(root, "memories/M-0001.md", &memory("M-0001", ""));
    as_lorisse("tidy up");
    let store = Store::open(root, Access::Full).unwrap();
    let report = store.check().unwrap().to_string();
    assert!(
        report.contains("tasks/T-0005.md (T-0005): written by Lorisse"),
        "{report}"
    );
    assert!(
        report.contains("with confidence: validated, above low"),
        "{report}"
    );
    assert!(
        report.contains("tasks/T-0006.md (T-0006): commit"),
        "{report}"
    );
    assert!(
        report.contains("touches a piece its action does not name: more than one action"),
        "{report}"
    );
    assert!(report.contains("without its source link"), "{report}");
    assert!(report.contains("is not one action: tidy up"), "{report}");
}
