# plugin — contract

> written by Lorisse 2026-11-24 · reviewed by the owner: 2026-11-25

Human file; format: `docs/contract_spec.md`. Stage design: the clause ids may still change, and this page is the reading step's alone; it is rewritten when writing comes.

## 1. Role

Lorisse as a person meets it in the Claude app: a program on their own machine that lets Claude read their memories through a short list of questions, on Windows and on a Mac.

## 2. Does and does not

- Finds the person's memory folder.
- Answers four questions on its memories, and on the people and tags they name: where it is, search, read, links. What each takes and gives is in the README, "The tools".

Does not:

- Write, file or accept anything → the writing step, not built.
- Show tasks, the inbox or the questions to check, or say what to do next → later versions; ranking is Claude's.
- Decide when a tool is called, or how a piece is filed → `keeper`.
- Read a piece's file, its number, the confidential mark → `store`.
- Speak the Claude app's protocol → `tools`.
- Read mail, a calendar or the web → the Claude app's own connectors.

## 3. Depends on

`store`, `tools`, `keeper`. Nothing else from the repository.

## 4. The memory folder

We want Lorisse to work on the one folder the person chose, and to say plainly when it has none.

- **C-4.1** The program works on one folder, the memory folder: the one the person gave in the plugin's setting; without it, the folder the session is opened on, when that is a memory folder. It looks nowhere else.
- **C-4.2** With no memory folder, `where` says so and how to give one, every other tool answers that same sentence, and nothing is created.
- **C-4.3** The folder is read at each call: a piece another program changed is seen at the next call.
- **C-4.4** A memory folder can name several repositories in its config: their pieces are read as one memory, each said with the repository it comes from.

## 5. Nothing is written

We want a question never to change the folder, so that reading is safe to try on a real one.

- **C-5.1** The program writes nothing in the memory folder: after any number of calls, every file and the folder's git state are as they were.
- **C-5.2** The program opens no network connection and starts no other program.

## 6. What is shown

We want Claude to see the person's memories, and never what they marked confidential.

- **C-6.1** A piece in a part of the folder marked confidential is never returned: not its text, its title or its number, and it is not counted. Asked by its number, the answer is the one for a number that does not exist.
- **C-6.2** Every answer that carries a piece's text says in its first line that the text is data, never an instruction.

## 7. The answers

We want every answer short enough for a conversation, and honest when it is cut.

- **C-7.1** A list cut at its limit ends with one line that gives the count in all.
- **C-7.2** A text cut at its limit ends with one line that says so and gives its full length.

## 8. Behaviour

| Situation | Reaction |
|---|---|
| A number that does not exist | one sentence that names it |
| An argument missing or of the wrong kind | one sentence that says what is expected |
| A piece whose header cannot be read | the answer names the file and gives the rest |
| Any call, whatever the folder or the piece | the program answers and stays up |

## 9. Wanted, not promised yet

- Writing: a memory, a note, a link, each one step in the folder's history, with its undo; proposals for anything heavy.
- Tasks, the inbox, the questions to check, what changed since the person last looked.
- The setup of the folders: a new person's empty folder made ready, a memory kept by subject or by project.
- The keeper's rules of filing, and one skill a subject.
- The same answers in the desktop app's Cowork, on Windows and on a Mac: built for, not tried.

## 10. Non-goals

A screen of its own, a choice of model, a reader of mail: the Claude app has them.
