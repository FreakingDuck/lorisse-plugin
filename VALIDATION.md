# plugin — validation

> written by Lorisse 2026-11-24 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

| Last run | Where it stands | Waiting on |
|---|---|---|
| never by you | Reading, writing and the open are built and green in the gate. Claude, run once without a person on the sample (`scripts/try_plugin.ps1`), opened Lorisse, filed a memory, found it, took it back and found nothing: two commits in the sample's history, and no tool had been allowed beforehand. A second run attached a file to a memory in one commit and read it back with the file's path. A third, in an empty folder with a home made for the try, found no memory, made one under git, imported two dated lines in one commit and gave the instruction back without following it. A fourth, on 0.5.0: after "plop" and one sentence said in passing, it filed the memory by itself and said its number; a fifth found the memory of the confidential repository opened to Claude, and added a dated line to it there, in one commit. H-5 walked by you on 2026-11-25 as far as "where". | Your walk of H-1 to H-9, H-12 and H-13; H-10 and H-11 on a machine that has no memory yet; once the new version is on GitHub and updated in your Claude app. |

This part is validated two ways:

- **By the gate**: every tool is asked directly, without Claude, on the sample folder, and the sample is compared before and after. Promise by promise, it is in the table at the end.
- **By hand**: the steps below, in the Claude panel of VS Code, on a copy of the sample: what only a person can see, Claude asking Lorisse by itself and saying what it answers.

The sample is a small memory folder kept in the repository, every word of it invented: four memories, one person, two tags, one memory in a part marked confidential, a second repository named `club` with one memory, a third named `office`, confidential and open to Claude, with one memory, and a few tasks and a raw piece kept for the later steps. With several repositories, each piece is said with its own: `M-0001 [home]`. Its dates are far behind or far ahead, so the walk reads the same any day.

## Before you start

1. In a terminal at the repository root, run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify.ps1`.
   - See: its last line, `green`.
2. In the terminal, run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/walk_sample.ps1`.
   - See: `sample ready: target/walk-sample`.
3. In VS Code, choose File, Open Folder, and open `target/walk-sample`; then open the Claude panel.
   - See: `walk-sample` in the window's title, and an empty conversation.

## In the Claude panel, on the sample

- [ ] **H-1** Lorisse finds the folder the session is opened on.
  It proves C-4.1: without it, Lorisse would answer from no folder, or from the wrong one.
  1. Type "Lorisse, where are you?".
     - See: a path that ends in `walk-sample`, and the words "the folder this session is opened on".

- [ ] **H-2** A memory is found, read, and followed to what it links to.
  It proves C-6.2 as far as a person sees it, and that the three questions work through Claude: a memory is of use only if it comes back when asked.
  1. Type "Find boiler".
     - See: "The boiler's contract", "Boiler serviced in March", and the person Sam Example.
  2. Type "Read M-0002 and say what it links to".
     - See: the text of "Boiler serviced in March", and its link to "The boiler's contract".

- [ ] **H-3** A confidential piece does not exist for Claude.
  It proves C-6.1: what you marked confidential never reaches the conversation.
  1. Type "Read M-0006".
     - See: Lorisse says no piece has the number M-0006.

- [ ] **H-4** After these questions, the folder is as it was.
  It proves C-5.1: a question never changes your folder.
  1. In the terminal, run `git -C target/walk-sample status`.
     - See: `nothing to commit, working tree clean`.

## Writing, on the sample

- [ ] **H-6** A memory is filed in one sentence, as one step of the folder's history.
  It proves C-8.1 and C-8.4: what you ask Lorisse to remember is in a file of yours, and you can see when it got there.
  1. Type "Remember that the gutter was cleaned in October".
     - See: Lorisse says it is filed, and gives its number.
  2. In the terminal, run `git -C target/walk-sample log --oneline -1`.
     - See: one line that names that number.

- [ ] **H-7** The last change is taken back.
  It proves C-8.5: a memory filed by mistake costs you one sentence.
  1. Type "Undo that".
     - See: Lorisse says the change is taken back.
  2. Type "Find gutter".
     - See: Lorisse answers that nothing matches.

- [ ] **H-8** One word opens Lorisse with your own intro.
  It proves C-9.1 and C-9.2: you set Lorisse up once, in a page of yours, and every conversation starts from it.
  1. Type "plop".
     - See: the sample's intro, then the folder, its three repositories, the memories filed last, and the rules of keeping.

- [ ] **H-9** A file is kept with the memory it belongs to.
  It proves C-10.1 and C-10.2: a document is found again from what it is about.
  1. Type "Attach the file README.md of the lorisse-plugin repository to the boiler's contract".
     - See: Lorisse says the file is attached to M-0001.
  2. Type "Read M-0001".
     - See: the memory, and the file `README.md` named with its path in the sample.

- [ ] **H-12** After "plop", Lorisse files by itself.
  It proves C-9.4 and C-9.6: you never dictate what to keep.
  1. In a new conversation, type "plop", then "By the way, the chimney sweep comes on the 3rd of March".
     - See: an answer that ends with one line saying it is filed, with its number.

- [ ] **H-13** A confidential repository you opened to Claude is read and written.
  It proves C-6.3 and C-8.7: your work memory serves you in a work conversation, and stays in its own repository.
  1. Type "Find printer".
     - See: "The printer's lease", said with its repository, `office`.
  2. Type "Remember, for the office, that the lease ends in June".
     - See: Lorisse says it is filed, with a number that starts with `O-`.

## The first day, on a machine that has no memory yet

- [ ] **H-10** A new person has a memory in one conversation.
  It proves C-11.1, C-11.3 and C-11.4: nothing to know about folders or git to begin.
  1. Type "plop".
     - See: Lorisse says there is no memory yet and offers to make one.
  2. Answer yes, then its three questions.
     - See: Lorisse says the folder is ready, and where it is.
  3. Open a new conversation and type "plop".
     - See: your intro, with the three answers you gave.

- [ ] **H-11** The first day does not start empty.
  It proves C-11.6: what another assistant already knows of you comes with you.
  1. Paste three dated lines of your own after "Import this:".
     - See: Lorisse says three memories are filed, with their dates.

## On your own folder

- [ ] **H-5** Lorisse answers from the folder you gave in the plugin's setting.
  It proves C-4.1: from a session opened on a code project, your folder is still found.
  1. In the Claude app, install or update Lorisse. A small window asks to save the plugin's settings: type your folder under "Memory folder" and choose Save.
     - See: the window closes, and Lorisse is listed with its version.
  2. Open a session on a folder that is not your memory, and type "Lorisse, where are you?".
     - See: your memory folder, the words "the plugin's setting", and the version you installed.

## For the gate: one row a promise

For the gate, which checks that each promise of the contract has one row here, and for the assistant; nothing here is yours to read.

| Clause | What is checked | Automated | By hand |
|---|---|---|---|
| C-4.1 | the session's folder when it is a memory folder, else the setting | 3 checks · input (the setting, the session's folder, both) | H-1, H-5 |
| C-4.2 | no folder: one sentence, nothing created | 2 checks · output | — |
| C-4.3 | a piece changed between two calls | 1 check · input | — |
| C-4.4 | a second repository named in the sample's config | 2 checks · output (its piece found, its repository said) | — |
| C-5.1 | the sample before and after every tool | 1 check · writes | H-4 |
| C-5.2 | no connection, no other program | 1 check · the program's dependencies | — |
| C-5.3 | each tool's declaration: reads only, or writes | 1 check · output | — |
| C-6.1 | the confidential piece, by every tool | 4 checks · output | H-3 |
| C-6.2 | the first line of an answer that carries a text | 2 checks · output | H-2 |
| C-6.3 | a confidential repository open to Claude: its piece found and read with its repository, counted and named confidential by `plop`; one without Anthropic among its providers still hidden | 3 checks · output | H-13 |
| C-7.1 | a list past its limit | 1 check · output | — |
| C-7.2 | a text past its limit | 1 check · output | — |
| C-8.1 | a memory filed, then read back | 2 checks · writes | H-6 |
| C-8.2 | a note on a memory and on a person | 2 checks · writes | — |
| C-8.3 | a link, and the same link twice | 2 checks · writes | — |
| C-8.4 | one commit a change, its message, the person's own uncommitted change left; a repository with its own history and the folder's counter: nothing uncommitted after a change and after its undo | 4 checks · writes | H-6 |
| C-8.5 | the last change taken back, a named one, a commit that is not Lorisse's | 3 checks · writes | H-7 |
| C-8.6 | the number after an undo | 1 check · writes | — |
| C-8.7 | a write asked in the confidential part, and in a confidential repository not open to Claude; a memory filed, noted and taken back in one that is open | 3 checks · writes | H-13 |
| C-8.8 | a memory filed in the named repository, and without a name | 2 checks · writes | — |
| C-8.9 | an unknown kind, a link to no piece | 2 checks · output | — |
| C-8.10 | a memory filed, a note and an undo asked in a folder that is not under git; the one sentence of `plop` | 3 checks · writes | — |
| C-9.1 | the intro page given whole, changed between two calls, absent | 3 checks · output | H-8 |
| C-9.2 | the state after the intro: the folder, the repositories, the counts, the five last | 1 check · output | H-8 |
| C-9.3 | the intro said as the person's word, a memory's text as data | 1 check · output | — |
| C-9.4 | the rules of keeping at the end of `plop`; what they say of the conversation so far and of several repositories | 2 checks · output | H-8, H-12 |
| C-9.5 | the instructions given when the app connects | 1 check · output | — |
| C-9.6 | the instructions, each writing tool's description and the rules say the same of filing | 1 check · output | H-12 |
| C-10.1 | a file attached: its copy, its commit, the original untouched | 2 checks · writes | H-9 |
| C-10.2 | the files of a piece named by `read`, with their paths | 1 check · output | H-9 |
| C-10.3 | a piece found by its file's name | 1 check · output | — |
| C-10.4 | a file over the limit | 1 check · output | — |
| C-10.5 | a second file of the same name | 1 check · writes | — |
| C-10.6 | a key file, a secret's name | 2 checks · output | — |
| C-11.1 | a folder made in a home folder of the test's own, in a named place; a place that holds files refused | 3 checks · writes | H-10 |
| C-11.2 | under git with one commit when git is there; made all the same with an empty PATH, with the sentence | 2 checks · writes | — |
| C-11.3 | the folder found afterwards with no setting, from another folder | 1 check · input | H-10 |
| C-11.4 | the intro written from the three answers, given back by `plop` | 1 check · output | H-10 |
| C-11.5 | the text to copy, whole, in its block | 1 check · output | — |
| C-11.6 | dated lines filed with their dates and categories; the instructions given back, not filed; one undo for the whole | 3 checks · writes | H-11 |
| C-11.7 | a line that cannot be read, counted and left out | 1 check · output | — |

## Run log

| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
