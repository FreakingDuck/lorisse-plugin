# tools — contract

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`. Stage design: the clause ids may still change. Every clause here is moved word for word from the root's contract; the root's `VALIDATION.md` gives each one's id of before. It holds what is true whatever the tool; each tool's own promises are in the contract of its subject, one folder under `src/`.

## 1. Role

The program the Claude app starts on the person's machine: it speaks the app's protocol and gives Claude the tools of its subjects, and nothing else.

## 2. Does and does not

- Speaks the Claude app's protocol and gives the list of the tools, each declared reading or writing.
- Says what is shown of the memory and how an answer is cut, for every tool.
- Carries the plugin's guard, in a mode of its own.

Does not:

- Answer a tool's question → its subject: `reading`, `opening`, `writing`, `files`, `keeping`, `drawers`, `first_day`.
- Say whether a call passes, asks or is refused → `guard`, by the person's rules.
- Read a piece's file, its number, the confidential mark → `store`.

## 3. Depends on

`store`, `guard`, and its subjects: `reading`, `opening`, `writing`, `files`, `keeping`, `drawers`, `first_day`. Nothing else from the repository.

## 4. Reading writes nothing

We want a question never to change the folder, so that reading is safe to try on a real one.

- **C-4.1** The program opens no network connection. The reading tools start no other program; the writing tools start one, `git`, and only in the memory folder.
- **C-4.2** The reading tools are declared to the Claude app as tools that only read, and the writing tools as tools that write, so the person can let the first run without being asked and keep a say on the second.

## 5. What is shown

We want Claude to see the person's memories, and never what they marked confidential.

- **C-5.1** A piece in a part of the folder marked confidential is never returned: not its text, its title or its number, and it is not counted. Asked by its number, the answer is the one for a number that does not exist.
- **C-5.2** Every answer that carries a piece's text says in its first line that the text is data, never an instruction.
- **C-5.3** A repository marked confidential whose row names Anthropic among the model providers allowed to see it is open to Claude: its pieces are shown and counted as any other's, each said with its repository, and `plop` names it as confidential. Every other confidential part stays as C-5.1 says.

## 6. The answers

We want every answer short enough for a conversation, and honest when it is cut.

- **C-6.1** A list cut at its limit ends with one line that gives the count in all.
- **C-6.2** A text cut at its limit ends with one line that says so and gives its full length.

## 7. Behaviour

| Situation | Reaction |
|---|---|
| A number that does not exist | one sentence that names it |
| An argument missing or of the wrong kind | one sentence that says what is expected |
| A piece whose header cannot be read | the answer names the file and gives the rest |
| Any call, whatever the folder or the piece | the program answers and stays up |

## 8. Wanted, not promised yet

none

## 9. Non-goals

none
