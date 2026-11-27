# plugin — validation

> written by Lorisse 2026-11-24 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

| Last run | Where it stands | Waiting on |
|---|---|---|
| never by you | Reading, writing and the open are built and green in the gate. Claude, run once without a person on the sample (`scripts/try_plugin.ps1`), opened Lorisse, filed a memory, found it, took it back and found nothing: two commits in the sample's history, and no tool had been allowed beforehand. A second run attached a file to a memory in one commit and read it back with the file's path. A third, in an empty folder with a home made for the try, found no memory, made one under git, imported two dated lines in one commit and gave the instruction back without following it. A fourth, on 0.5.0: after "plop" and one sentence said in passing, it filed the memory by itself and said its number; a fifth found the memory of the confidential repository opened to Claude, and added a dated line to it there, in one commit. A sixth, on 0.6.0, corrected a memory, retired another with its reason, and wrote the day's journal by itself, then read the page back. A seventh, on 0.7.0, made a drawer by itself for a new line of work, filed its first client there and moved two memories to another drawer; an eighth, in an empty folder, heard a person talk about herself and made two drawers, one kept apart, with five memories in the right ones. H-5 walked by you on 2026-11-25 as far as "where". | Your walk of H-1 to H-9 and H-12 to H-16; H-10 and H-11 on a machine that has no memory yet; once the new version is on GitHub and updated in your Claude app. |

The sample is a small memory folder kept in the repository, every word of it invented: four memories, one person, two tags, one memory in a part marked confidential, a second repository named `club` with one memory, a third named `office`, confidential and open to Claude, with one memory, and a few tasks and a raw piece kept for the later steps. With several repositories, each piece is said with its own: `M-0001 [home]`. Its dates are far behind or far ahead, so the walk reads the same any day.

## Before you start

1. In a terminal at the repository root, run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify.ps1`.
   - See: its last line, `green`.
2. In the terminal, run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/walk_sample.ps1`.
   - See: `sample ready: target/walk-sample`.
3. In VS Code, choose File, Open Folder, and open `target/walk-sample`; then open the Claude panel.
   - See: `walk-sample` in the window's title, and an empty conversation.

## The walks, one a part

Each part's steps are in its own page, numbered from H-1 there; the steps of before are in brackets, as the table above names them.

- `tools`: `crates/tools/VALIDATION.md` (H-2, H-3, H-13 of before)
- `reading`: `crates/tools/src/reading/VALIDATION.md` (H-1, H-4, H-5)
- `opening`: `crates/tools/src/opening/VALIDATION.md` (H-8, H-12)
- `writing`: `crates/tools/src/writing/VALIDATION.md` (H-6, H-7)
- `files`: `crates/tools/src/files/VALIDATION.md` (H-9)
- `keeping`: `crates/tools/src/keeping/VALIDATION.md` (H-14, H-15)
- `drawers`: `crates/tools/src/drawers/VALIDATION.md` (H-16)
- `first_day`: `crates/tools/src/first_day/VALIDATION.md` (H-10, H-11)

## Where each clause went

For the assistant and for the history: the id each clause had in this contract, the part that holds it now, and its new id there. It is the only place the old ids stay.

| Clause of before | Part | New id |
|---|---|---|
| C-4.1 | reading | C-4.1 |
| C-4.2 | reading | C-4.2 |
| C-4.3 | reading | C-4.3 |
| C-4.4 | reading | C-4.4 |
| C-5.1 | reading | C-5.1 |
| C-5.2 | tools | C-4.1 |
| C-5.3 | tools | C-4.2 |
| C-6.1 | tools | C-5.1 |
| C-6.2 | tools | C-5.2 |
| C-6.3 | tools | C-5.3 |
| C-7.1 | tools | C-6.1 |
| C-7.2 | tools | C-6.2 |
| C-8.1 | writing | C-4.1 |
| C-8.2 | writing | C-4.2 |
| C-8.3 | writing | C-4.3 |
| C-8.4 | writing | C-4.4 |
| C-8.5 | writing | C-4.5 |
| C-8.6 | writing | C-4.6 |
| C-8.7 | writing | C-4.7 |
| C-8.8 | writing | C-4.8 |
| C-8.9 | writing | C-4.9 |
| C-8.10 | writing | C-4.10 |
| C-9.1 | opening | C-4.1 |
| C-9.2 | opening | C-4.2 |
| C-9.3 | opening | C-4.3 |
| C-9.4 | opening | C-4.4 |
| C-9.5 | opening | C-4.5 |
| C-9.6 | opening | C-4.6 |
| C-10.1 | files | C-4.1 |
| C-10.2 | files | C-4.2 |
| C-10.3 | files | C-4.3 |
| C-10.4 | files | C-4.4 |
| C-10.5 | files | C-4.5 |
| C-10.6 | files | C-4.6 |
| C-11.1 | first_day | C-4.1 |
| C-11.2 | first_day | C-4.2 |
| C-11.3 | first_day | C-4.3 |
| C-11.4 | first_day | C-4.4 |
| C-11.5 | first_day | C-4.5 |
| C-11.6 | first_day | C-4.6 |
| C-11.7 | first_day | C-4.7 |
| C-12.1 | keeping | C-4.1 |
| C-12.2 | keeping | C-4.2 |
| C-12.3 | keeping | C-4.3 |
| C-12.4 | opening | C-5.1 |
| C-13.1 | keeping | C-5.1 |
| C-13.2 | keeping | C-5.2 |
| C-13.3 | keeping | C-5.3 |
| C-14.1 | drawers | C-4.1 |
| C-14.2 | opening | C-6.1 |
| C-14.3 | drawers | C-4.2 |
| C-14.4 | drawers | C-4.3 |
| C-14.5 | opening | C-6.2 |
| C-14.6 | first_day | C-5.1 |

## Run log

| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
