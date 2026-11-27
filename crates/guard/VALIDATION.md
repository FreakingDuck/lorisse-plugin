# guard — validation

> written by Lorisse 2026-11-25 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

| Last run | Where it stands | Waiting on |
|---|---|---|
| never by you | Built, wired into the plugin and green in the gate. In one run of Claude Code without a person, with no tool allowed beforehand, every tool of Lorisse ran: the guard let them pass. Cowork is not tried. | Your walk of H-1 to H-3, once the new version is on GitHub and updated in your Claude app. |

This part is validated two ways:

- **By the gate**: the guard is asked directly, without the Claude app, with a call and a rules file, and its answer is compared. Promise by promise, it is in the table at the end.
- **By hand**: the steps below: what only a person can see, a prompt that no longer comes, in Claude Code and in Cowork.

## Before you start

1. In a terminal at the repository root, run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify.ps1`.
   - See: its last line, `green`.
2. In the terminal, run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/walk_sample.ps1`.
   - See: `sample ready: target/walk-sample`.

## In the Claude panel, on the sample

- [ ] **H-1** A tool the rules let pass runs without a prompt.
  It proves C-5.1: the rules you wrote spare you the prompt.
  1. Open `target/walk-sample` in VS Code, open the Claude panel, and type "Find boiler".
     - See: the answer, with no prompt before it.

- [ ] **H-2** A tool the rules refuse is refused, with the rule.
  It proves C-5.1: what you wrote as never does not happen.
  1. In `target/walk-sample/rules.yaml`, move `search` under `never`, and type "Find boiler" again.
     - See: Lorisse says the search was refused by your rules.

## In Cowork

- [ ] **H-3** Cowork obeys the guard.
  It proves C-5.1 where Anthropic's pages are silent.
  1. In the Claude app, open Cowork on `target/walk-sample` and type "Find boiler".
     - See: the answer, with no prompt before it.

## For the gate: one row a promise

For the gate and for the assistant; nothing here is yours to read.

| Clause | What is checked | Automated | By hand |
|---|---|---|---|
| C-4.1 | a rules file read, changed between two calls | 2 checks · input | H-2 |
| C-4.2 | a folder without the file: the first set | 2 checks · output | H-1 |
| C-4.3 | a tool in two lists | 1 check · output | — |
| C-4.4 | a call no rule names | 1 check · output | — |
| C-4.5 | a tool of Lorisse's the person's file does not name: passes by the first set; the same tool named under ask, under never: the person's file; a tool of another service not named: left to the app | 3 checks · output | — |
| C-5.1 | the three answers, the rule named in a refusal | 3 checks · output | H-1, H-2, H-3 |
| C-5.2 | nothing passes unnamed; a file that cannot be read | 2 checks · output | — |
| C-5.3 | a file tool writing the rules file | 1 check · output | — |
| C-5.4 | the folder before and after; the crate's dependencies | 1 check · writes | — |

## Run log

| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
