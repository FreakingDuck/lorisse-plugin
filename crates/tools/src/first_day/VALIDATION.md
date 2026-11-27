# first_day — validation

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

This part is validated two ways:

- **By the gate**: every tool is asked directly, without Claude, on the sample folder, and the sample is compared before and after. Promise by promise, it is in the table at the end.
- **By hand**: the steps below, in the Claude panel of VS Code: what only a person can see, Claude asking Lorisse by itself and saying what it answers.

## Before you start

The moves of the root's `VALIDATION.md`, under its own "Before you start": the gate, then the steps below on a machine that has no memory yet.

## The first day, on a machine that has no memory yet

- [ ] **H-1** A new person has a memory in one conversation.
  It proves C-4.1, C-4.3 and C-4.4: nothing to know about folders or git to begin.
  1. Type "plop".
     - See: Lorisse says there is no memory yet and offers to make one.
  2. Answer yes, then tell it about yourself as it asks: your name, what fills your days, what must never mix.
     - See: Lorisse says the folder is ready, where it is, and which drawers it made from what you said.
  3. Open a new conversation and type "plop".
     - See: your intro, with the three answers you gave.

- [ ] **H-2** The first day does not start empty.
  It proves C-4.6: what another assistant already knows of you comes with you.
  1. Paste three dated lines of your own after "Import this:".
     - See: Lorisse says three memories are filed, with their dates.

## For the gate: one row a promise

For the gate, which checks that each promise of the contract has one row here, and for the assistant; nothing here is yours to read.

| Clause | What is checked | Automated | By hand |
|---|---|---|---|
| C-4.1 | a folder made in a home folder of the test's own, in a named place; a place that holds files refused | 3 checks · writes | H-1 |
| C-4.2 | under git with one commit when git is there; made all the same with an empty PATH, with the sentence | 2 checks · writes | — |
| C-4.3 | the folder found afterwards with no setting, from another folder | 1 check · input | H-1 |
| C-4.4 | the intro written from the three answers, given back by `plop` | 1 check · output | H-1 |
| C-4.5 | the text to copy, whole, in its block | 1 check · output | — |
| C-4.6 | dated lines filed with their dates and categories; the instructions given back, not filed; one undo for the whole | 3 checks · writes | H-2 |
| C-4.7 | a line that cannot be read, counted and left out | 1 check · output | — |
| C-5.1 | what `start` tells Claude to ask and to do | 1 check · output | H-1 |

## Run log

| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
