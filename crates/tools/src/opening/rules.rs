//! The person's rules, as the conversation meets them: what `plop` says is ruled already, and
//! `rule`, which writes the person's answer in their `rules.yaml` through the guard's writer,
//! then commits it through the store, as the other writing tools do.

use std::path::Path;

use lorisse_guard::{List, Ruling, ServiceRuled, RULES_FILE};

use crate::shared::{Answer, Memory};

/// A list longer than this is said by its count.
const NAMED_AT_MOST: usize = 4;

/// What `plop` says of the person's rules: the services ruled besides Lorisse's own.
pub(crate) fn ruled(root: &Path) -> String {
    match lorisse_guard::ruled(Some(root)) {
        Ruling::Unreadable(why) => format!(
            "The person's rules ({RULES_FILE}) cannot be read ({why}): every call of a connector or of Lorisse asks until it is mended."
        ),
        Ruling::Services(services) if !services.is_empty() => {
            let mut lines = vec![format!("Ruled already in the person's rules ({RULES_FILE}), besides Lorisse's own tools:")];
            lines.extend(services.iter().map(service_line));
            lines.join("\n")
        }
        _ => format!("No service besides Lorisse's own is ruled yet in the person's rules ({RULES_FILE})."),
    }
}

/// `- gmail: 7 tools pass; ask reply, forward`.
fn service_line(service: &ServiceRuled) -> String {
    let parts: Vec<String> = service
        .lists
        .iter()
        .map(|(list, tools)| match tools.len() > NAMED_AT_MOST {
            true => format!("{} tools {}", tools.len(), list.name()),
            false => format!("{} {}", list.name(), tools.join(", ")),
        })
        .collect();
    format!("- {}: {}", service.service, parts.join("; "))
}

/// `rule`: the person's answer written in their rules, one commit; what the file now says.
pub(crate) fn rule(memory: &mut Memory, root: &Path, service: &str, tools: &[String], list: &str) -> Answer {
    let word = List::named(list).map(List::name).unwrap_or("");
    let subject = format!("rule({}): {word} {}", service.trim(), tools.join(", "));
    let mut commit: Option<String> = None;
    let store = &mut memory.store;
    let written = lorisse_guard::write_rule_with(root, service, tools, list, |text| {
        match store.write_setting(RULES_FILE, text, &subject) {
            Ok(id) => {
                commit = id;
                Ok(())
            }
            Err(e) => Err(format!("Nothing was written: {}.", e.to_string().trim_end_matches('.'))),
        }
    });
    let done = match written {
        Ok(done) => done,
        Err(said) => return Answer::error(said),
    };
    if !done.changed {
        return Answer::ok(format!(
            "Your rules ({RULES_FILE}) already say for {}: {}; nothing was written.",
            done.service, done.said
        ));
    }
    let made = match done.made {
        true => ", the file made from the plugin's first set,",
        false => "",
    };
    let kept = match &commit {
        Some(id) => format!(", in one commit of {} ({}) that undo takes back", memory.root_name, id.chars().take(7).collect::<String>()),
        None => String::new(),
    };
    Answer::ok(format!("Your rules ({RULES_FILE}){made} now say for {}: {}{kept}.", done.service, done.said))
}
