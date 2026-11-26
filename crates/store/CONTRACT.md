# store — contract

> written by Lorisse 2026-11-24 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`. Stage design: the clause ids may still change. This page holds what the plugin's reading step relies on; the promises of writing are written when the writing step comes.

## 1. Role

The one door to a memory folder: the only code that reads a piece's file, for the plugin's program, on Windows and on a Mac.

## 2. Does and does not

- Reads a memory folder: one piece, a list, a search, the links.
- Keeps what is confidential from an opening that has no right to it.
- Opens a folder for reading only, without writing in it.

Does not:

- Speak to the Claude app → `tools`.
- Decide what is filed, or when → `keeper`.

## 3. Depends on

Nothing else from the repository.

## 4. Reading

We want every read of a folder to go through one place, so a rule on what may be read holds everywhere.

- **C-4.1** A piece is read by its number, by its path, or, for a person, by name. One that does not exist is said unknown.
- **C-4.2** A search finds a piece by a text in its title, its tags or its body, capitals ignored, a match in a title first.
- **C-4.3** The links of a piece are given both ways: the pieces it names and the pieces that name it.

## 5. What is confidential

We want a part the person marked confidential to stay out of every answer, not only out of a read.

- **C-5.1** An opening without the confidential right does not see a confidential piece: it is not read, not listed, not found by a search and not counted.
- **C-5.2** An opening made for a named model provider sees a confidential repository whose row names that provider, and reads and writes it as any other; its state still says it is confidential. Every other confidential part stays as C-5.1 says.

## 6. An opening for reading

We want a folder to be read without a trace, so that a question is safe on a person's real folder.

- **C-6.1** An opening for reading writes no file in the folder, creates none, and starts no other program.
- **C-6.2** On an opening for reading, every call that would write is refused, and the refusal says the opening only reads.

## 7. Behaviour

| Situation | Reaction |
|---|---|
| a piece whose header cannot be read | the folder still opens; the check names the file |
| an opening for reading on a folder that is not there | refused; nothing is created |

## 8. Wanted, not promised yet

- The promises of writing: a number never reused, one change one commit, the undo, the proposals.

## 9. Non-goals

A screen, a model, a network connection.
