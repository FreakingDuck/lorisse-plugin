# plugin — contract

> written by Lorisse 2026-11-24 · reviewed by the owner: 2026-11-25

Human file; format: `docs/contract_spec.md`. Stage design: the clause ids may still change. Each promise is in the contract of its part: `crates/tools/` for what holds whatever the tool, and one folder a subject under `crates/tools/src/`. The table "Where each clause went", at the end of `VALIDATION.md`, gives each clause's id of before.

## 1. Role

Lorisse as a person meets it in the Claude app: a program on their own machine that lets Claude read their memories and file new ones, through a short list of tools, on Windows and on a Mac.

## 2. Does and does not

- Finds the person's memory folder → `reading`.
- Answers four questions on its memories, and on the people and tags they name: where it is, search, read, links. What each takes and gives is in the README, "The tools" → `reading`.
- Files a memory, adds a note to a piece, links two pieces, and takes back its own last change: remember, note, link, undo → `writing`.
- Opens a conversation with the person's own intro, the state of the memory and the rules of keeping, when the person says "Lorisse" or "plop": plop → `opening`.
- Keeps a document with the memory it is about: attach → `files`.
- Makes a new person's memory folder ready, and fills it with what another assistant already remembers of them: start, import → `first_day`.
- Corrects a memory that was wrong, and puts aside one that no longer holds: change, retire → `keeping`.
- Keeps the days: what happened and what was decided, a page a day: journal → `keeping`.
- Sorts by itself: makes a repository for a subject that has none, and moves what was put in the wrong one: drawer, move → `drawers`.

Does not:

- File a person, merge or move pieces → later; anything heavy will be a proposal the person accepts.
- Pull or push the folder → the person's own git, when they want one.
- Show tasks, the inbox or the questions to check, or say what to do next → later versions; ranking is Claude's.
- Decide when a tool is called, or how a piece is filed → `keeper`.
- Say whether a call passes, asks or is refused → `guard`, by the person's rules.
- Read a piece's file, its number, the confidential mark → `store`.
- Speak the Claude app's protocol → `tools`.
- Read mail, a calendar or the web → the Claude app's own connectors.

## 3. Depends on

`store`, `tools`, `keeper`, `guard`, and the subjects of `tools`, each a folder under `crates/tools/src/`: `reading`, `opening`, `writing`, `files`, `keeping`, `drawers`, `first_day`. Nothing else from the repository.

Themes: none: each promise is in the contract of its part.

## 4. Behaviour

none: each row is in the contract of its part.

## 5. Wanted, not promised yet

- A short tutorial to keep the folder on a git service as well, for those who want a copy elsewhere.
- The way out: the whole memory given back as one page of dated lines by category, so a person can leave with it or hand it to another assistant.
- One memory for life and for the work on code: the assistant's own notes of a project as one more repository of the memory folder.
- The same memory from the phone and the plain chat: the program on a server of the person's, the memories exchanged by git.
- A release that arrives whole: the card, the version and the programs together, never one before the other; and a check after each release that an installed copy says the new version.
- Tasks, the inbox, the questions to check, what changed since the person last looked.
- The plugin itself neither pulls nor pushes: the folder stays on the machine unless the person wants more.
- The keeper's rules of filing, and one skill a subject.
- The same answers in the desktop app's Cowork, on Windows and on a Mac: built for, not tried.

## 6. Non-goals

A screen of its own, a choice of model, a reader of mail: the Claude app has them.
