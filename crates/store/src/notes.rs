//! Notes — a piece's `## Notes / history` section and a task's closing line.
//!
//! The one place the closing line `- Done <day>: <next, else title>` is written: the app's Done
//! and the store's archive of a task not yet done both read it here.
//! Flow: a piece → `closing_line` → `with_note` → the body with the line under its notes.

use chrono::NaiveDate;

use crate::frontmatter::key;
use crate::piece::{Piece, scalar_text};

// =============================================================================
// Interface
// =============================================================================

/// The heading a dated note goes under.
pub const NOTES: &str = "## Notes / history";

/// The heading whose first paragraph is a task's next step when it has no `next` field.
pub const NEXT_ACTION: &str = "## Next action";

/// `body` with `line` (one or more lines) after the last line of its notes, the section added when missing.
pub fn with_note(body: &str, line: &str) -> String {
    note_appended(body, line)
}

/// The first paragraph under `## Next action`, on one line.
pub fn next_action(body: &str) -> Option<String> {
    first_paragraph(body)
}

/// `- Done <day>: <the task's next, else its next action, else its title>`, on one line.
pub fn closing_line(piece: &Piece, day: NaiveDate) -> String {
    format!("- Done {day}: {}", closed_what(piece))
}

/// The task's body with its closing line under its notes.
pub fn closed_body(piece: &Piece, day: NaiveDate) -> String {
    with_note(piece.body(), &closing_line(piece, day))
}

// =============================================================================
// Implementation
// =============================================================================

fn closed_what(piece: &Piece) -> String {
    let next = piece.get(key::NEXT).and_then(scalar_text);
    let what = next
        .filter(|n| !n.trim().is_empty())
        .or_else(|| next_action(piece.body()))
        .unwrap_or_else(|| piece.title().to_string());
    what.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn first_paragraph(body: &str) -> Option<String> {
    let mut lines = body
        .lines()
        .skip_while(|l| l.trim_end() != NEXT_ACTION)
        .skip(1);
    let lines = lines.by_ref().skip_while(|l| l.trim().is_empty());
    let para: Vec<&str> = lines
        .take_while(|l| !l.trim().is_empty() && !l.starts_with("## "))
        .map(str::trim)
        .collect();
    (!para.is_empty()).then(|| para.join(" "))
}

fn note_appended(body: &str, line: &str) -> String {
    let cr = if body.contains("\r\n") { "\r" } else { "" };
    let line = &line.replace('\n', &format!("{cr}\n"));
    let mut lines: Vec<String> = body.split('\n').map(str::to_string).collect();
    let Some(head) = lines.iter().position(|l| l.trim_end() == NOTES) else {
        let nl = format!("{cr}\n");
        let mut out = body.trim_end().to_string();
        if !out.is_empty() {
            out.push_str(&nl);
            out.push_str(&nl);
        }
        return format!("{out}{NOTES}{nl}{line}{nl}");
    };
    let rest = lines[head + 1..].iter().position(|l| l.starts_with("## "));
    let end = rest.map_or(lines.len(), |i| head + 1 + i);
    let last = (head..end).rev().find(|&i| !lines[i].trim().is_empty());
    lines.insert(last.unwrap_or(head) + 1, format!("{line}{cr}"));
    lines.join("\n")
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn task(fields: &str, body: &str) -> Piece {
        let text =
            format!("---\nid: T-0001\ntitle: Book  the van\nstatus: todo\n{fields}---\n{body}");
        Piece::parse(Path::new("tasks/T-0001.md"), &text).unwrap()
    }

    #[test]
    fn the_closing_line_says_the_next_else_the_next_action_else_the_title() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
        let next = task("next: call  Robin\n", "## Next action\nOther.\n");
        assert_eq!(closing_line(&next, day), "- Done 2026-10-16: call Robin");
        let action = task(
            "next: ' '\n",
            "## Next action\n\nCall Noa,\nbook Thursday.\n\n## Notes\n",
        );
        assert_eq!(
            closing_line(&action, day),
            "- Done 2026-10-16: Call Noa, book Thursday."
        );
        assert_eq!(
            closing_line(&task("", ""), day),
            "- Done 2026-10-16: Book the van"
        );
    }

    #[test]
    fn a_note_goes_after_the_last_line_of_its_section_or_in_a_new_one() {
        let body = "## Context\nx\n\n## Notes / history\n- 2026-10-01: made\n\n## After\ny\n";
        let want =
            "## Context\nx\n\n## Notes / history\n- 2026-10-01: made\n- new\n\n## After\ny\n";
        assert_eq!(with_note(body, "- new"), want);
        assert_eq!(
            with_note("Some text.\n", "- a"),
            "Some text.\n\n## Notes / history\n- a\n"
        );
        assert_eq!(with_note("", "- a"), "## Notes / history\n- a\n");
        assert_eq!(
            with_note("x\r\n", "- a\n  b"),
            "x\r\n\r\n## Notes / history\r\n- a\r\n  b\r\n"
        );
    }
}
