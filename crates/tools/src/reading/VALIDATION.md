# reading — validation

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

This part is validated two ways:

- **By the gate**: every tool is asked directly, without Claude, on the sample folder, and the sample is compared before and after. Promise by promise, it is in the table at the end.
- **By hand**: the steps below, in the Claude panel of VS Code, on a copy of the sample: what only a person can see, Claude asking Lorisse by itself and saying what it answers.

## Before you start

The moves of the root's `VALIDATION.md`, under its own "Before you start": the gate, the sample made, `target/walk-sample` opened in VS Code with the Claude panel.

## In the Claude panel, on the sample

- [ ] **H-1** Lorisse finds the folder the session is opened on.
  It proves C-4.1: without it, Lorisse would answer from no folder, or from the wrong one.
  1. Type "Lorisse, where are you?".
     - See: a path that ends in `walk-sample`, and the words "the folder this session is opened on".

- [ ] **H-2** After these questions, the folder is as it was.
  It proves C-5.1: a question never changes your folder.
  1. In the terminal, run `git -C target/walk-sample status`.
     - See: `nothing to commit, working tree clean`.

## On your own folder

- [ ] **H-3** Lorisse answers from the folder you gave in the plugin's setting.
  It proves C-4.1: from a session opened on a code project, your folder is still found.
  1. In the Claude app, install or update Lorisse. A small window asks to save the plugin's settings: type your folder under "Memory folder" and choose Save.
     - See: the window closes, and Lorisse is listed with its version.
  2. Open a session on a folder that is not your memory, and type "Lorisse, where are you?".
     - See: your memory folder, the words "the plugin's setting", and the version you installed.

## For the gate: one row a promise

For the gate, which checks that each promise of the contract has one row here, and for the assistant; nothing here is yours to read.

| Clause | What is checked | Automated | By hand |
|---|---|---|---|
| C-4.1 | the session's folder when it is a memory folder, else the setting | 3 checks · input (the setting, the session's folder, both) | H-1, H-3 |
| C-4.2 | no folder: one sentence, nothing created | 2 checks · output | — |
| C-4.3 | a piece changed between two calls | 1 check · input | — |
| C-4.4 | a second repository named in the sample's config | 2 checks · output (its piece found, its repository said) | — |
| C-5.1 | the sample before and after every tool | 1 check · writes | H-2 |

## Run log

| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
