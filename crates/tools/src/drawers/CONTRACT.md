# drawers — contract

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`. Stage design: the clause ids may still change. Every clause here is moved word for word from the root's contract; the root's `VALIDATION.md` gives each one's id of before.

## 1. Role

The part of the plugin's program that makes a repository for a subject that has none, and moves what was put in the wrong one.

## 2. Does and does not

- Sorts by itself: makes a repository for a subject that has none, and moves what was put in the wrong one: drawer, move.

Does not:

- Say each repository with what it is for, and the rules of sorting → `opening`.
- Tell Claude on a first day to make the repositories from what the person says → `first_day`.
- Write the repository's folder and its line in the settings, make the commit → `store`.

## 3. Depends on

`store`, `tools`, `reading`. Nothing else from the repository.

## 4. The drawers

We want a person to talk and Lorisse to sort: the house, the work, a client, a project each in its own repository, which Lorisse calls a drawer when it speaks, made without the person having to know what a repository is, and never a trap: what was put in the wrong one is moved.

- **C-4.1** `drawer` makes a new repository in the memory folder from a name and one sentence on what it is for: its folder, a numbering of its own, and its line in the folder's settings, as one commit that `undo` takes back. A name already taken is refused; nothing the person wrote in the settings is changed.
- **C-4.2** `move` puts one memory, or several, in another repository: each keeps its number, its text, its links and its files, and what links to it still leads to it. It is one commit, and `undo` puts them back.
- **C-4.3** `drawer` can make a repository kept apart: marked confidential and open to Claude (`tools` C-5.3), for what must never mix with the rest. `move` never takes a piece out of a confidential repository: asked to, it refuses in one sentence.

## 5. Behaviour

| Situation | Reaction |
|---|---|
| `drawer` with a name already taken | one sentence that names the repository there; nothing written |
| `move` of a piece to the repository it is in | one sentence that says so; nothing written |

## 6. Wanted, not promised yet

- A repository renamed, or two made one.

## 7. Non-goals

none
