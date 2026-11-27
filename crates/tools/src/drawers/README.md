# drawers — a repository made by Lorisse, a piece moved

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

`drawer` makes a new repository of the memory folder for a subject that has none, kept apart on a word; `move` puts memories in another repository, each with its number. Each is one commit, made in the folder's own history, which `undo` takes back.
What it promises: `CONTRACT.md`. How that is checked: `VALIDATION.md`.

## The words

- **drawer**: a repository of the memory folder, as Lorisse calls it when it speaks to the person.
- **kept apart**: a repository marked confidential and open to Claude, for what must never mix with the rest.

## The folder layout

```text
drawers/
└── mod.rs        drawer, move, and the numbers said in words
```

## The tools

| Tool | It takes | It gives |
|---|---|---|
| `drawer` | a name and one sentence on what it is for; `apart` if it must never mix with the rest | a new repository of your memory folder, which Lorisse calls a drawer: its folder, its own numbers (`G-0001`), its line in `config.yaml` with every line of yours left as it was, as one commit. Lorisse makes one by itself when a subject keeps coming and has no place |
| `move` | one memory or several, and the repository to put them in | each moved with its number, its text, its links and its files, as one commit; nothing is ever taken out of a confidential repository |
