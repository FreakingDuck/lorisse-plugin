# drawers — validation

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

This part is validated two ways:

- **By the gate**: every tool is asked directly, without Claude, on the sample folder, and the sample is compared before and after. Promise by promise, it is in the table at the end.
- **By hand**: the steps below, in the Claude panel of VS Code, on a copy of the sample: what only a person can see, Claude asking Lorisse by itself and saying what it answers.

## Before you start

The moves of the root's `VALIDATION.md`, under its own "Before you start": the gate, the sample made, `target/walk-sample` opened in VS Code with the Claude panel.

## Writing, on the sample

- [ ] **H-1** Lorisse sorts by itself, and what is in the wrong drawer is moved.
  It proves C-4.1 and C-4.2: you talk, and never have to say where a thing goes.
  1. Type "plop", then "I am starting to take clients for garden design. The first one is the house on the hill, her budget is 4000".
     - See: Lorisse says it made a drawer for that work, and that the client is filed in it.
  2. Type "The boiler has nothing to do with the house: put it with the garden work".
     - See: Lorisse says which memory moved, with the same number as before.

## For the gate: one row a promise

For the gate, which checks that each promise of the contract has one row here, and for the assistant; nothing here is yours to read.

| Clause | What is checked | Automated | By hand |
|---|---|---|---|
| C-4.1 | a repository made: its folder, its numbering, its line in the settings with the lines there left as they were; one commit and its undo; a memory filed in it at once | 3 checks · writes | H-1 |
| C-4.2 | a memory moved, several at once: the number, the text, the links and the files kept, a link to it still followed; one commit and its undo | 3 checks · writes | H-1 |
| C-4.3 | a repository made confidential and open to Claude, read and written; a move out of a confidential one refused | 2 checks · writes | — |

## Run log

| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
