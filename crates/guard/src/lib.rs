//! Lorisse's guard: before a tool call runs, pass, ask or never, by the person's rules.
//! Given the text a Claude `PreToolUse` hook receives and the memory folder, it gives the text
//! the hook prints, or nothing when the call is left to the Claude app.
//! Its answer writes nothing, opens no connection and starts no program; `rule` alone writes,
//! one rule in the person's file, on their yes, and says what the file rules (`rule.rs`).

mod call;
mod rule;
mod rules;

use std::path::Path;

use serde_json::json;

pub use call::FILE_TOOLS;
pub use rule::{ruled, write_rule, write_rule_with, RuleWritten, Ruling, ServiceRuled};
pub use rules::{List, FIRST_RULES, RULES_FILE};

use rules::{Loaded, Ruled, Source};

/// The tool of Lorisse's that writes a rule: a call of it always asks.
pub const RULE_TOOL: &str = "rule";

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
    if rules::service_in(server, "lorisse") && tool.trim().eq_ignore_ascii_case(RULE_TOOL) {
        return Some(printed("ask", &format!(
            "You are about to change your own rules ({RULES_FILE}): the rule shown here decides what runs without asking you from now on, so writing it always asks, whatever your rules say."
        )));
    }
    match rules::load(folder) {
        Loaded::Unreadable(why) => Some(printed("ask", &format!(
            "Lorisse's rules file, {RULES_FILE}, cannot be read ({why}): every call of a connector or of Lorisse asks until it is mended."
        ))),
        Loaded::Rules(rules, source) => {
            let (ruled, whose) = rules::ruling(&rules, source, server, tool)?;
            Some(printed(decision(ruled.list), &reason(&ruled, whose)))
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
