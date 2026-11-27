# opening — validation

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

This part is validated two ways:

- **By the gate**: every tool is asked directly, without Claude, on the sample folder, and the sample is compared before and after. Promise by promise, it is in the table at the end.
- **By hand**: the steps below, in the Claude panel of VS Code, on a copy of the sample: what only a person can see, Claude asking Lorisse by itself and saying what it answers.

## Before you start

The moves of the root's `VALIDATION.md`, under its own "Before you start": the gate, the sample made, `target/walk-sample` opened in VS Code with the Claude panel.

## Writing, on the sample

- [ ] **H-1** One word opens Lorisse with your own intro.
  It proves C-4.1 and C-4.2: you set Lorisse up once, in a page of yours, and every conversation starts from it.
  1. Type "plop".
     - See: the sample's intro, then the folder, its three repositories, the memories filed last, and the rules of keeping.

- [ ] **H-2** After "plop", Lorisse files by itself.
  It proves C-4.4 and C-4.6: you never dictate what to keep.
  1. In a new conversation, type "plop", then "By the way, the chimney sweep comes on the 3rd of March".
     - See: an answer that ends with one line saying it is filed, with its number.

## For the gate: one row a promise

For the gate, which checks that each promise of the contract has one row here, and for the assistant; nothing here is yours to read.

| Clause | What is checked | Automated | By hand |
|---|---|---|---|
| C-4.1 | the intro page given whole, changed between two calls, absent | 3 checks · output | H-1 |
| C-4.2 | the state after the intro: the folder, the repositories, the counts, the five last | 1 check · output | H-1 |
| C-4.3 | the intro said as the person's word, a memory's text as data | 1 check · output | — |
| C-4.4 | the rules of keeping at the end of `plop`; what they say of the conversation so far and of several repositories | 2 checks · output | H-1, H-2 |
| C-4.5 | the instructions given when the app connects | 1 check · output | — |
| C-4.6 | the instructions, each writing tool's description and the rules say the same of filing | 1 check · output | H-2 |
| C-5.1 | the two rules among those `plop` gives | 1 check · output | — |
| C-6.1 | each repository said with what it is for by `plop` | 1 check · output | `drawers` H-1 |
| C-6.2 | the three rules among those `plop` gives | 1 check · output | — |

## Run log

| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
