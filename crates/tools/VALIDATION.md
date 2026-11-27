# tools — validation

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

This part is validated two ways:

- **By the gate**: every tool is asked directly, without Claude, on the sample folder, and the sample is compared before and after. Promise by promise, it is in the table at the end.
- **By hand**: the steps below, in the Claude panel of VS Code, on a copy of the sample: what only a person can see, Claude asking Lorisse by itself and saying what it answers.

## Before you start

The moves of the root's `VALIDATION.md`, under its own "Before you start": the gate, the sample made, `target/walk-sample` opened in VS Code with the Claude panel.

## In the Claude panel, on the sample

- [ ] **H-1** A memory is found, read, and followed to what it links to.
  It proves C-5.2 as far as a person sees it, and that the three questions work through Claude: a memory is of use only if it comes back when asked.
  1. Type "Find boiler".
     - See: "The boiler's contract", "Boiler serviced in March", and the person Sam Example.
  2. Type "Read M-0002 and say what it links to".
     - See: the text of "Boiler serviced in March", and its link to "The boiler's contract".

- [ ] **H-2** A confidential piece does not exist for Claude.
  It proves C-5.1: what you marked confidential never reaches the conversation.
  1. Type "Read M-0006".
     - See: Lorisse says no piece has the number M-0006.

## Writing, on the sample

- [ ] **H-3** A confidential repository you opened to Claude is read and written.
  It proves C-5.3 and `writing` C-4.7: your work memory serves you in a work conversation, and stays in its own repository.
  1. Type "Find printer".
     - See: "The printer's lease", said with its repository, `office`.
  2. Type "Remember, for the office, that the lease ends in June".
     - See: Lorisse says it is filed, with a number that starts with `O-`.

## For the gate: one row a promise

For the gate, which checks that each promise of the contract has one row here, and for the assistant; nothing here is yours to read.

| Clause | What is checked | Automated | By hand |
|---|---|---|---|
| C-4.1 | no connection, no other program | 1 check · the program's dependencies | — |
| C-4.2 | each tool's declaration: reads only, or writes | 1 check · output | — |
| C-5.1 | the confidential piece, by every tool | 4 checks · output | H-2 |
| C-5.2 | the first line of an answer that carries a text | 2 checks · output | H-1 |
| C-5.3 | a confidential repository open to Claude: its piece found and read with its repository, counted and named confidential by `plop`; one without Anthropic among its providers still hidden | 3 checks · output | H-3 |
| C-6.1 | a list past its limit | 1 check · output | — |
| C-6.2 | a text past its limit | 1 check · output | — |

## Run log

| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
