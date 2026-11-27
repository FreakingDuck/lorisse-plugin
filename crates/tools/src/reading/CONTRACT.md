# reading — contract

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`. Stage design: the clause ids may still change. Every clause here is moved word for word from the root's contract; the root's `VALIDATION.md` gives each one's id of before.

## 1. Role

The part of the plugin's program that finds the person's memory folder and answers the four questions on it: where, search, read, links.

## 2. Does and does not

- Finds the person's memory folder.
- Answers four questions on its memories, and on the people and tags they name: where it is, search, read, links. What each takes and gives is in the README, "The tools".

Does not:

- Write in the folder → `writing`, `files`, `keeping`, `drawers`, `first_day`.
- Open a conversation → `opening`.
- Say what is shown and how an answer is cut → `tools`.
- Read a piece's file, its number, the confidential mark → `store`.

## 3. Depends on

`store`, `tools`. Nothing else from the repository.

## 4. The memory folder

We want Lorisse to work on the one folder the person chose, and to say plainly when it has none.

- **C-4.1** The program works on one folder, the memory folder: the folder the session is opened on, when that is a memory folder; otherwise the one the person gave in the plugin's setting; otherwise the one this machine remembers from the first day (`first_day` C-4.3). It looks nowhere else.
- **C-4.2** With no memory folder, `where` and `plop` say so and offer to make one with `start`; every other tool answers that same sentence, and nothing is created.
- **C-4.3** The folder is read at each call: a piece another program changed is seen at the next call.
- **C-4.4** A memory folder can name several repositories in its config: their pieces are read as one memory, each said with the repository it comes from.

## 5. Reading writes nothing

We want a question never to change the folder, so that reading is safe to try on a real one.

- **C-5.1** The four reading tools write nothing in the memory folder: after any number of their calls, every file and the folder's git state are as they were.

## 6. Behaviour

| Situation | Reaction |
|---|---|
| The setting names one repository of a memory, not the folder that holds them | one sentence that says so, and where the setting is changed; `start` is not offered |

## 7. Wanted, not promised yet

- The memory folder chosen by browsing, not typed: today the Claude app's own window asks for it as a line of text.

## 8. Non-goals

none
