# writing — validation

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

This part is validated two ways:

- **By the gate**: every tool is asked directly, without Claude, on the sample folder, and the sample is compared before and after. Promise by promise, it is in the table at the end.
- **By hand**: the steps below, in the Claude panel of VS Code, on a copy of the sample: what only a person can see, Claude asking Lorisse by itself and saying what it answers.

## Before you start

The moves of the root's `VALIDATION.md`, under its own "Before you start": the gate, the sample made, `target/walk-sample` opened in VS Code with the Claude panel.

## Writing, on the sample

- [ ] **H-1** A memory is filed in one sentence, as one step of the folder's history.
  It proves C-4.1 and C-4.4: what you ask Lorisse to remember is in a file of yours, and you can see when it got there.
  1. Type "Remember that the gutter was cleaned in October".
     - See: Lorisse says it is filed, and gives its number.
  2. In the terminal, run `git -C target/walk-sample log --oneline -1`.
     - See: one line that names that number.

- [ ] **H-2** The last change is taken back.
  It proves C-4.5: a memory filed by mistake costs you one sentence.
  1. Type "Undo that".
     - See: Lorisse says the change is taken back.
  2. Type "Find gutter".
     - See: Lorisse answers that nothing matches.

## For the gate: one row a promise

For the gate, which checks that each promise of the contract has one row here, and for the assistant; nothing here is yours to read.

| Clause | What is checked | Automated | By hand |
|---|---|---|---|
| C-4.1 | a memory filed, then read back | 2 checks · writes | H-1 |
| C-4.2 | a note on a memory and on a person | 2 checks · writes | — |
| C-4.3 | a link, and the same link twice | 2 checks · writes | — |
| C-4.4 | one commit a change, its message, the person's own uncommitted change left; a repository with its own history and the folder's counter: nothing uncommitted after a change and after its undo | 4 checks · writes | H-1 |
| C-4.5 | the last change taken back, a named one, a commit that is not Lorisse's | 3 checks · writes | H-2 |
| C-4.6 | the number after an undo | 1 check · writes | — |
| C-4.7 | a write asked in the confidential part, and in a confidential repository not open to Claude; a memory filed, noted and taken back in one that is open | 3 checks · writes | `tools` H-3 |
| C-4.8 | a memory filed in the named repository, and without a name | 2 checks · writes | — |
| C-4.9 | an unknown kind, a link to no piece | 2 checks · output | — |
| C-4.10 | a memory filed, a note and an undo asked in a folder that is not under git; the one sentence of `plop` | 3 checks · writes | — |

## Run log

| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
