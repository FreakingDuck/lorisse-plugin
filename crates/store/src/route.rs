//! Route — where a job's action goes: written at once, written and flagged, or a proposal.
//!
//! The table by kind first, then the overrides, the strictest winning: an external source,
//! the slice's mode and a confidential repository, the daily cap (the slice's mode and the cap hold back neither a
//! greeting's or noise's retire nor his tap on an answer that closes a task); `unsure` flags a direct write last.
//! Flow: `Action` + `Context` → `decide` (the kind, the diff, the facts) → `Routing`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::actions::{self, Action};
use crate::config::status;
use crate::diff::Diff;
use crate::error::StoreError;
use crate::frontmatter::key;
use crate::ids::PieceId;
use crate::layer::Layer;
use crate::links::Ref;
use crate::repos;
use crate::resolution::{Does, Resolution};
use crate::store::Store;

// =============================================================================
// Interface
// =============================================================================

/// What a job's action is, in the job's words: finer than the store's `Action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A capture made a memory, a task or a person.
    Sort,
    /// One dated line at the end of a piece.
    Note,
    /// A link added.
    Link,
    /// Fields set or removed.
    Edit,
    /// A piece retired.
    Retire,
    /// A piece's whole text rewritten.
    Rewrite,
    /// Two pieces merged.
    Dedupe,
    /// A part of a piece made its own.
    Split,
    /// A piece moved across layers or `confidential/`.
    Move,
    /// A task done and archived.
    Archive,
    /// A tag's definition.
    DefineTag,
    /// A tag renamed everywhere.
    RenameTag,
    /// A résumé of several pieces.
    Resume,
    /// A rule line.
    Rule,
    /// Anything that sends a mail.
    Mail,
    /// A memory filed at once, on the person's word, without a capture.
    Remember,
    /// A file of the machine copied beside a piece, on the person's word.
    Attach,
}

/// Where an action goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    /// Written at once, one commit.
    Direct,
    /// Written at once, one commit, and listed in "To check".
    DirectFlagged,
    /// A proposal file the owner accepts.
    Proposal,
}

/// Lorisse's own judgement on one action; it only routes, never stored in a piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Certainty {
    /// She is sure.
    Sure,
    /// She is not: a direct write is flagged.
    #[default]
    Unsure,
}

/// A slice's write mode, from `agent.yaml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteMode {
    /// Safe actions are written at once.
    Direct,
    /// Everything is a proposal.
    #[default]
    Proposal,
}

/// What the caller knows about one action; the store reads no `agent.yaml`.
#[derive(Debug, Clone, PartialEq)]
pub struct Context {
    /// The job's word for the action, when finer than the action itself (note, split …).
    pub kind: Option<Kind>,
    /// The source word of the capture the action comes from (`app`, `todoist`, `mail` …).
    pub origin: Option<String>,
    /// The capture the action comes from; names the proposal file and the commit's `source`.
    pub capture: Option<PieceId>,
    /// The slice's write mode.
    pub write_mode: WriteMode,
    /// Whether the day's spend has reached the cap.
    pub cap_reached: bool,
    /// Lorisse's certainty.
    pub certainty: Certainty,
    /// One line of why, said to the owner.
    pub why: String,
    /// Her question on a flagged piece; the why when none.
    pub question: Option<String>,
    /// Her proposed answers to that question, the likeliest first, each `text (certainty)`; kept on a flagged piece.
    pub answers: Vec<String>,
    /// The job (`file`, `talk`, `keeper` …).
    pub job: String,
    /// The provider.
    pub provider: String,
    /// The model.
    pub model: String,
    /// The call's cost, in USD.
    pub cost_usd: f64,
    /// Whether no commit of the turn carries its cost yet (default true); else the commit says where it is.
    pub first_of_turn: bool,
    /// His tap on one of her answers on the target task, its text; a drop or archive one closes it at once.
    pub tapped: Option<String>,
}

/// An action's route and the reason, in plain words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Routing {
    /// The kind the route was read from.
    pub kind: Kind,
    /// Where it goes.
    pub route: Route,
    /// Why, one line.
    pub reason: String,
}

/// The external source words: their captures never go direct.
pub const EXTERNAL: [&str; 4] = ["todoist", "mail", "document", "file"];

/// Noise: a capture of at most this many words.
pub const NOISE_WORDS: usize = 2;

/// Whether a text is keyboard noise: one or two words, each a filler (`ok`, `stays` …) or a lowercase run with no
/// vowel or one key held; never with a digit, a tag, a person, a mark or a link. A retire of such a capture typed in
/// the app routes direct, as a greeting's does.
pub fn is_noise(text: &str) -> bool {
    noise_of(text)
}

impl Kind {
    /// The word of the kind, as a commit's subject spells it.
    pub fn word(self) -> &'static str {
        word_of(self)
    }

    /// The kind a word names (`done` is an archive); `None` for another word.
    pub fn from_word(word: &str) -> Option<Kind> {
        kind_named(word)
    }

    /// The kind an action is by its shape alone: an edit with a body is a rewrite.
    pub fn of(action: &Action) -> Kind {
        kind_of_shape(action)
    }

    /// The table's route for the kind, before the facts and the overrides.
    pub fn route(self) -> Route {
        table(self)
    }
}

// =============================================================================
// Implementation
// =============================================================================

/// Bodies a retire may take at once: a greeting or a test, numbers aside.
const GREETINGS: [&str; 15] = [
    "test", "tests", "testing", "essai", "hello", "hi", "hey", "bonjour", "salut", "coucou",
    "allo", "ok", "yo", "ping", "lorisse",
];

/// The source of a capture typed in the app.
const APP: &str = "app";

const ALL: [Kind; 17] = [
    Kind::Sort,
    Kind::Note,
    Kind::Link,
    Kind::Edit,
    Kind::Retire,
    Kind::Rewrite,
    Kind::Dedupe,
    Kind::Split,
    Kind::Move,
    Kind::Archive,
    Kind::DefineTag,
    Kind::RenameTag,
    Kind::Resume,
    Kind::Rule,
    Kind::Mail,
    Kind::Remember,
    Kind::Attach,
];

impl Default for Context {
    fn default() -> Self {
        Context {
            kind: None,
            origin: None,
            capture: None,
            write_mode: WriteMode::default(),
            cap_reached: false,
            certainty: Certainty::default(),
            why: String::new(),
            question: None,
            answers: Vec::new(),
            job: String::new(),
            provider: String::new(),
            model: String::new(),
            cost_usd: 0.0,
            first_of_turn: true,
            tapped: None,
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.word())
    }
}

fn word_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Sort => "sort",
        Kind::Note => "note",
        Kind::Link => "link",
        Kind::Edit => "edit",
        Kind::Retire => "retire",
        Kind::Rewrite => "rewrite",
        Kind::Dedupe => "dedupe",
        Kind::Split => "split",
        Kind::Move => "move",
        Kind::Archive => "archive",
        Kind::DefineTag => "define_tag",
        Kind::RenameTag => "rename_tag",
        Kind::Resume => "resume",
        Kind::Rule => "rule",
        Kind::Mail => "mail",
        Kind::Remember => "remember",
        Kind::Attach => "attach",
    }
}

fn kind_named(word: &str) -> Option<Kind> {
    let word = word.trim();
    match word {
        "done" => Some(Kind::Archive),
        "résumé" => Some(Kind::Resume),
        _ => ALL.into_iter().find(|k| k.word() == word),
    }
}

fn kind_of_shape(action: &Action) -> Kind {
    match action {
        Action::Sort { layer, .. } if *layer == Layer::Tag => Kind::DefineTag,
        Action::Sort { .. } => Kind::Sort,
        Action::Edit { body: Some(_), .. } => Kind::Rewrite,
        Action::Edit { .. } => Kind::Edit,
        Action::Link { .. } => Kind::Link,
        Action::Dedupe { .. } => Kind::Dedupe,
        Action::Archive { .. } => Kind::Archive,
        Action::Retire { .. } => Kind::Retire,
        Action::Move { .. } => Kind::Move,
        Action::RenameTag { .. } => Kind::RenameTag,
    }
}

fn table(kind: Kind) -> Route {
    match kind {
        Kind::Sort
        | Kind::Note
        | Kind::Link
        | Kind::Edit
        | Kind::Retire
        | Kind::Remember
        | Kind::Attach => Route::Direct,
        Kind::Rewrite | Kind::Dedupe | Kind::Split => Route::DirectFlagged,
        Kind::Move
        | Kind::Archive
        | Kind::DefineTag
        | Kind::RenameTag
        | Kind::Resume
        | Kind::Rule
        | Kind::Mail => Route::Proposal,
    }
}

/// The route of an action and its diff; nothing is written.
pub(crate) fn decide(
    store: &Store,
    action: &Action,
    context: &Context,
) -> Result<(Routing, Diff), StoreError> {
    let kind = kind_given(store, action, context)?;
    let diff = actions::propose(store, action.clone())?;
    let tap = closing_tap(store, action, context);
    let (route, reason) = match (tap, action) {
        (true, Action::Archive { .. }) => (Route::Direct, TAP_REASON.into()),
        _ => by_facts(store, action, kind)?,
    };
    let mut routing = Routing {
        kind,
        route,
        reason,
    };
    let source = external(store, action, context);
    stricter(
        &mut routing,
        source.map(|w| format!("an external source ({w}): never direct")),
    );
    let confidential = touches_confidential(store, &diff);
    stricter(
        &mut routing,
        confidential.then(|| "under confidential/: by proposal".into()),
    );
    let exempt = tap || matches!(action, Action::Retire { id, .. } if greeting_capture(store, id));
    let by_proposal = context.write_mode == WriteMode::Proposal && !exempt;
    stricter(
        &mut routing,
        by_proposal.then(|| "the slice writes by proposal".into()),
    );
    let capped = context.cap_reached && !exempt;
    stricter(
        &mut routing,
        capped.then(|| "the daily cap is reached".into()),
    );
    if context.certainty == Certainty::Unsure && routing.route == Route::Direct {
        routing.route = Route::DirectFlagged;
        routing.reason = "Lorisse is unsure: written, to check".into();
    }
    Ok((routing, diff))
}

/// Turn the routing to a proposal for `reason`, unless it is one already.
fn stricter(routing: &mut Routing, reason: Option<String>) {
    if let Some(reason) = reason
        && routing.route != Route::Proposal
    {
        routing.route = Route::Proposal;
        routing.reason = reason;
    }
}

/// The kind the caller named, when it fits the action; a note that rewrites is a rewrite.
fn kind_given(store: &Store, action: &Action, context: &Context) -> Result<Kind, StoreError> {
    let shape = Kind::of(action);
    let Some(given) = context.kind else {
        return Ok(shape);
    };
    if given == shape || given.route() == Route::Proposal {
        return Ok(given);
    }
    match (given, action) {
        (
            Kind::Note,
            Action::Edit {
                id,
                set,
                unset,
                body: Some(body),
            },
        ) if set.is_empty() && unset.is_empty() => Ok(if appends(store, id, body) {
            Kind::Note
        } else {
            Kind::Rewrite
        }),
        (Kind::Note | Kind::Edit, Action::Edit { .. }) => Ok(shape),
        (Kind::Rewrite | Kind::Split, Action::Edit { .. }) | (Kind::Split, Action::Sort { .. }) => {
            Ok(given)
        }
        _ => Err(StoreError::Invalid(format!(
            "a {given} is not a {shape}: the action does not fit the word"
        ))),
    }
}

/// Whether `body` is the piece's own body with lines added at its end.
fn appends(store: &Store, id: &PieceId, body: &str) -> bool {
    let Ok(piece) = store.read(&Ref::from(id.clone())) else {
        return false;
    };
    let kept = piece.body().trim_end();
    body.starts_with(kept) && body.trim_end().len() > kept.len()
}

/// The table's route narrowed by what the files say: a retire, a dedupe, a sort's layer.
fn by_facts(store: &Store, action: &Action, kind: Kind) -> Result<(Route, String), StoreError> {
    let named = format!("the table: a {kind}");
    let route = kind.route();
    let out = match action {
        Action::Retire { id, .. } if route == Route::Direct && !greeting_capture(store, id) => (
            Route::Proposal,
            "a retire of a piece that is not a greeting, a test or noise typed in the app".into(),
        ),
        Action::Dedupe { keep, .. } if keep.prefix().is_none() => {
            (Route::Proposal, "a dedupe of people: by proposal".into())
        }
        Action::Sort { layer, .. }
            if route != Route::Proposal
                && !matches!(layer, Layer::Memory | Layer::Task | Layer::Person) =>
        {
            (
                Route::Proposal,
                format!("a sort into a {layer}: by proposal"),
            )
        }
        _ => match route {
            Route::Direct => (route, format!("{named}, written at once")),
            Route::DirectFlagged => (route, format!("{named}, written and to check")),
            Route::Proposal => (route, format!("{named}, by proposal")),
        },
    };
    Ok(out)
}

/// The reason of an archive his tap closes.
const TAP_REASON: &str = "his tap on her answer that closes the task: written at once";

/// Whether the context's tap names one of the target task's own answers marked `drop` or `archive`.
fn closing_tap(store: &Store, action: &Action, context: &Context) -> bool {
    let Some(tapped) = context.tapped.as_deref().map(str::trim) else {
        return false;
    };
    let target = action.target();
    let task = store
        .index
        .get(&target)
        .is_some_and(|e| e.layer == Layer::Task);
    let Ok(piece) = store.read(&Ref::from(target)) else {
        return false;
    };
    let closes = |r: &Resolution| r.does != Does::Answer && r.text.eq_ignore_ascii_case(tapped);
    task && piece.resolutions().iter().any(closes)
}

/// A raw capture of the inbox, typed in the app, whose words are a greeting or a test.
fn greeting_capture(store: &Store, id: &PieceId) -> bool {
    let Some(entry) = store.index.get(id) else {
        return false;
    };
    let inbox = format!("{}/", store.config.inbox);
    let raw = entry.status.as_deref().is_none_or(|s| s == status::RAW);
    if !raw || !entry.path_text().starts_with(&inbox) {
        return false;
    }
    let Ok(piece) = store.read(&Ref::from(id.clone())) else {
        return false;
    };
    let body = piece.body();
    piece.text(key::SOURCE) == Some(APP) && (is_greeting(body) || is_noise(body))
}

/// One to three words, each a greeting, a test word or a number.
pub(crate) fn is_greeting(text: &str) -> bool {
    let low = text.to_lowercase();
    let words: Vec<&str> = low
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let plain = |w: &&str| GREETINGS.contains(w) || w.chars().all(|c| c.is_ascii_digit());
    !words.is_empty() && words.len() <= 3 && words.iter().all(plain)
}

/// Words a noise capture is made of: a lone filler or a stray keystroke.
const FILLERS: [&str; 18] = [
    "ok", "okay", "oki", "k", "kk", "stays", "hm", "hmm", "mmm", "euh", "bof", "lol", "xd", "x",
    "xx", "xxx", "asdf", "qsdf",
];

/// The marks that make a short text more than noise: a tag, a person, a mark, a link, a path.
const NOT_NOISE: [char; 5] = ['#', '@', '!', '/', ':'];

/// The vowels a word of letters needs to be more than a keyboard run.
const VOWELS: &str = "aeiouyàâäéèêëîïôöùûüœæ";

fn noise_of(text: &str) -> bool {
    if text.contains(NOT_NOISE) || text.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    let words: Vec<&str> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let mash = |w: &str| {
        let mut chars = w.chars();
        let first = chars.next();
        let one_key = first.is_some_and(|f| chars.all(|c| c == f));
        let lower = w.chars().all(char::is_lowercase);
        lower && (one_key || !w.chars().any(|c| VOWELS.contains(c)))
    };
    let noise = |w: &&str| FILLERS.contains(&w.to_lowercase().as_str()) || mash(w);
    !words.is_empty() && words.len() <= NOISE_WORDS && words.iter().all(noise)
}

/// The external source the action comes from, by the caller's word or by its capture's file.
fn external(store: &Store, action: &Action, context: &Context) -> Option<String> {
    if let Some(word) = context.origin.as_deref().map(str::trim)
        && EXTERNAL.contains(&word)
    {
        return Some(word.to_string());
    }
    let own = match action {
        Action::Sort { id, .. } | Action::Retire { id, .. } => Some(id.clone()),
        _ => None,
    };
    let captures = context.capture.iter().cloned().chain(own);
    captures
        .into_iter()
        .find_map(|id| external_capture(store, &id))
}

/// A capture's external source word, or `file` when it carries files.
fn external_capture(store: &Store, id: &PieceId) -> Option<String> {
    let piece = store.read(&Ref::from(id.clone())).ok()?;
    let source = piece.text(key::SOURCE).map(str::trim).unwrap_or("");
    if EXTERNAL.contains(&source) {
        return Some(source.to_string());
    }
    (!piece.files().is_empty()).then(|| "file".to_string())
}

/// Whether a change of the diff lies in a confidential repository (its row's flag).
fn touches_confidential(store: &Store, diff: &Diff) -> bool {
    let first = |p: &std::path::Path| {
        let text = p.to_string_lossy().replace('\\', "/");
        repos::is_confidential(&text, &store.config)
    };
    diff.changes
        .iter()
        .any(|c| first(&c.path) || c.from.as_deref().is_some_and(first))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_gives_each_kind_its_route() {
        let direct = [
            Kind::Sort,
            Kind::Note,
            Kind::Link,
            Kind::Edit,
            Kind::Retire,
            Kind::Remember,
            Kind::Attach,
        ];
        let flagged = [Kind::Rewrite, Kind::Dedupe, Kind::Split];
        assert!(direct.iter().all(|k| k.route() == Route::Direct));
        assert!(flagged.iter().all(|k| k.route() == Route::DirectFlagged));
        let rest = ALL
            .iter()
            .filter(|k| !direct.contains(k) && !flagged.contains(k));
        assert!(rest.clone().all(|k| k.route() == Route::Proposal));
        assert_eq!(rest.count(), 7);
    }

    #[test]
    fn every_kind_reads_back_from_its_word() {
        for kind in ALL {
            assert_eq!(Kind::from_word(kind.word()), Some(kind));
        }
        assert_eq!(Kind::from_word("done"), Some(Kind::Archive));
        assert_eq!(Kind::from_word("delete"), None);
        let yaml: Kind = serde_yaml_ng::from_str("define_tag").unwrap();
        assert_eq!(yaml, Kind::DefineTag);
    }

    #[test]
    fn an_edit_with_a_body_is_a_rewrite_and_a_sort_into_a_tag_a_definition() {
        let id = PieceId::parse("M-1").unwrap();
        let edit = Action::Edit {
            id: id.clone(),
            set: Default::default(),
            unset: Vec::new(),
            body: Some("x".into()),
        };
        assert_eq!(Kind::of(&edit), Kind::Rewrite);
        let tag = Action::Sort {
            id,
            layer: Layer::Tag,
            confidential: false,
            slice: None,
            set: Default::default(),
            sorted_by: None,
        };
        assert_eq!(Kind::of(&tag), Kind::DefineTag);
    }

    #[test]
    fn a_greeting_or_a_test_is_short_and_known() {
        for text in ["test", "Test 2", "bonjour !", "hello lorisse", "  coucou\n"] {
            assert!(is_greeting(text), "{text}");
        }
        for text in [
            "",
            "acheter du lait",
            "test de la banque demain matin",
            "rappeler",
        ] {
            assert!(!is_greeting(text), "{text}");
        }
    }

    #[test]
    fn noise_is_one_or_two_fillers_or_lowercase_runs_and_never_a_word_with_a_mark() {
        for text in [
            "stays", "ok", " OK \n", "sdfg", "kkkk", "ok stays", "qsdf jkl",
        ] {
            assert!(is_noise(text), "{text}");
        }
        for text in [
            "",
            "Lait",
            "pneus",
            "RDV",
            "SNCF",
            "ok ok ok",
            "stays #bois",
            "ok 14h",
            "appeler Paul",
        ] {
            assert!(!is_noise(text), "{text}");
        }
    }

    #[test]
    fn the_route_order_puts_the_proposal_strictest() {
        assert!(Route::Direct < Route::DirectFlagged);
        assert!(Route::DirectFlagged < Route::Proposal);
        assert_eq!(Certainty::default(), Certainty::Unsure);
        assert_eq!(WriteMode::default(), WriteMode::Proposal);
    }
}
