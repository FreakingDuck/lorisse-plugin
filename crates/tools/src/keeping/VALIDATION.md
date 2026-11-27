# keeping — validation

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

This part is validated two ways:

- **By the gate**: every tool is asked directly, without Claude, on the sample folder, and the sample is compared before and after. Promise by promise, it is in the table at the end.
- **By hand**: the steps below, in the Claude panel of VS Code, on a copy of the sample: what only a person can see, Claude asking Lorisse by itself and saying what it answers.

## Before you start

The moves of the root's `VALIDATION.md`, under its own "Before you start": the gate, the sample made, `target/walk-sample` opened in VS Code with the Claude panel.

## Writing, on the sample

- [ ] **H-1** A memory is corrected, and one that no longer holds is put aside.
  It proves C-4.1 and C-4.2: your memory stays true without anything being lost.
  1. Type "The boiler was serviced in April, not in March: correct it".
     - See: Lorisse says M-0002 is changed.
  2. Type "The boiler's contract is over: retire it", then "Find boiler".
     - See: Lorisse says M-0001 is retired; in the list it comes last, said as retired.

- [ ] **H-2** The day is kept.
  It proves C-5.1 and C-5.2: what happened today is found again by its day.
  1. Type "Write in the journal that the plumber came this morning", then "Read today's journal".
     - See: Lorisse says it is written; then today's page, with that line and its time.

## For the gate: one row a promise

For the gate, which checks that each promise of the contract has one row here, and for the assistant; nothing here is yours to read.

| Clause | What is checked | Automated | By hand |
|---|---|---|---|
| C-4.1 | a memory's text changed and read back, a title and tags changed with the text left; a person's text; one commit, and the text before back after an undo | 3 checks · writes | H-1 |
| C-4.2 | a memory retired with its reason: still read, said as retired; last in a search; left out of the count | 3 checks · writes | H-1 |
| C-4.3 | a memory retired for another: the two linked, the other named | 1 check · writes | — |
| C-5.1 | an entry added to today's page and to a named day, what was there left; one commit and its undo; in a named repository | 3 checks · writes | H-2 |
| C-5.2 | a day's page read; an entry found by a search, said with its day | 2 checks · output | H-2 |
| C-5.3 | the last day written, said by `plop` | 1 check · output | — |

## Run log

| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
