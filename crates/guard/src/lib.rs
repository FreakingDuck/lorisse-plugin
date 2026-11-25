//! Lorisse's guard: before a tool call runs, pass, ask or never, by the person's rules.
//! Given the text a Claude `PreToolUse` hook receives and the memory folder, it gives the text
//! the hook prints, or nothing when the call is left to the Claude app.
//! It writes nothing, opens no connection and starts no program.

mod call;
mod rules;

use std::path::Path;

use serde_json::json;

pub use call::FILE_TOOLS;
pub use rules::{FIRST_RULES, RULES_FILE};

use rules::{List, Loaded, Ruled, Source};

/// The hook's answer to one call: the text to print, or `None` to leave the call to the app.
pub fn answer(received: &str, folder: Option<&Path>) -> Option<String> {
    std::panic::catch_unwind(|| decide(received, folder)).ok().flatten()
}

/// The call read, the rules read, the decision, the answer written.
fn decide(received: &str, folder: Option<&Path>) -> Option<String> {
    let call = call::read(received)?;
    if call.is_file_tool() {
        let folder = folder?;
        let target = call.target()?;
        return match call::same_file(&target, &folder.join(RULES_FILE)) {
            true => Some(printed("ask", &format!(
                "This call changes Lorisse's rules file, {RULES_FILE}: a change of the rules always asks."
            ))),
            false => None,
        };
    }
    let (server, tool) = call.split()?;
    match rules::load(folder) {
        Loaded::Unreadable(why) => Some(printed("ask", &format!(
            "Lorisse's rules file, {RULES_FILE}, cannot be read ({why}): every call of a connector or of Lorisse asks until it is mended."
        ))),
        Loaded::Rules(rules, source) => {
            let ruled = rules.decide(server, tool)?;
            Some(printed(decision(ruled.list), &reason(&ruled, source)))
        }
    }
}

/// The app's word for a list.
fn decision(list: List) -> &'static str {
    match list {
        List::Pass => "allow",
        List::Ask => "ask",
        List::Never => "deny",
    }
}

/// The reason, naming the rule: its list, its service, its tool, and whose rules.
fn reason(ruled: &Ruled, source: Source) -> String {
    let whose = match source {
        Source::Yours => format!("your rules ({RULES_FILE})"),
        Source::First => "the plugin's first set of rules".to_string(),
    };
    let rule = format!(
        "the tool {} of {} is under {}",
        ruled.tool,
        ruled.service,
        ruled.list.name()
    );
    match ruled.list {
        List::Pass => format!("Passed by {whose}: {rule}."),
        List::Ask => format!("Asked by {whose}: {rule}."),
        List::Never => format!("Refused by {whose}: {rule}."),
    }
}

/// The JSON a `PreToolUse` hook prints on its standard output.
fn printed(decision: &str, reason: &str) -> String {
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": decision,
            "permissionDecisionReason": reason,
        }
    })
    .to_string()
}
