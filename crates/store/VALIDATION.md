# store — validation

> written by Lorisse 2026-11-24 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

| Last run | Where it stands | Waiting on |
|---|---|---|
| never by hand | The crate's tests are green in the gate; it has no step by hand. | Nothing. |

This part is validated one way:

- **By the gate**: the crate's tests, each on a copy of a fixture or of the sample folder, never on a person's folder. Promise by promise, it is in the table below.

## Before you start

1. In a terminal at the repository root, run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify.ps1`.
   - See: its last line, `green`.

## For the gate: one row a promise

For the gate and for the assistant; nothing here is yours to read.

| Clause | What is checked | Automated | By hand |
|---|---|---|---|
| C-4.1 | a piece by number, by path, by name; an unknown one | the crate's tests · output | — |
| C-4.2 | a search in titles, tags and bodies | the crate's tests · output | — |
| C-4.3 | the links both ways | the crate's tests · output | — |
| C-5.1 | the confidential piece, by read, list, search and count | the crate's tests · output, and one on the sample | — |
| C-6.1 | a copy of the sample before and after an opening for reading and its reads | 2 checks · writes | — |
| C-6.2 | each writing call on an opening for reading | 1 check · output | — |

## Run log

| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
