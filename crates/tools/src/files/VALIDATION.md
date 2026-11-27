# files — validation

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

This part is validated two ways:

- **By the gate**: every tool is asked directly, without Claude, on the sample folder, and the sample is compared before and after. Promise by promise, it is in the table at the end.
- **By hand**: the steps below, in the Claude panel of VS Code, on a copy of the sample: what only a person can see, Claude asking Lorisse by itself and saying what it answers.

## Before you start

The moves of the root's `VALIDATION.md`, under its own "Before you start": the gate, the sample made, `target/walk-sample` opened in VS Code with the Claude panel.

## Writing, on the sample

- [ ] **H-1** A file is kept with the memory it belongs to.
  It proves C-4.1 and C-4.2: a document is found again from what it is about.
  1. Type "Attach the file README.md of the lorisse-plugin repository to the boiler's contract".
     - See: Lorisse says the file is attached to M-0001.
  2. Type "Read M-0001".
     - See: the memory, and the file `README.md` named with its path in the sample.

## For the gate: one row a promise

For the gate, which checks that each promise of the contract has one row here, and for the assistant; nothing here is yours to read.

| Clause | What is checked | Automated | By hand |
|---|---|---|---|
| C-4.1 | a file attached: its copy, its commit, the original untouched | 2 checks · writes | H-1 |
| C-4.2 | the files of a piece named by `read`, with their paths | 1 check · output | H-1 |
| C-4.3 | a piece found by its file's name | 1 check · output | — |
| C-4.4 | a file over the limit | 1 check · output | — |
| C-4.5 | a second file of the same name | 1 check · writes | — |
| C-4.6 | a key file, a secret's name | 2 checks · output | — |

## Run log

| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
