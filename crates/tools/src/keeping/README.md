# keeping — the memory kept true, and the journal

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

`change` corrects a memory or a person, `retire` puts aside a memory that no longer holds, with its reason, and `journal` writes the time and a text at the end of a day's page. Each is one commit when the folder is under git. The folder also holds what `read`, `search` and `plop` say of a retired memory and of the journal's pages.
What it promises: `CONTRACT.md`. How that is checked: `VALIDATION.md`.

## The words

- **retired**: a memory that no longer holds, kept with its reason, never deleted.
- **a day's page**: the journal's page of one day, `journal/2026-11-27.md`.

## The folder layout

```text
keeping/
└── mod.rs        change, retire, journal; what read says of a retired memory; a day's page read and named
```

## The tools

| Tool | It takes | It gives |
|---|---|---|
| `change` | a memory or a person, and what to replace: its text, its title, its kind, its tags | the piece corrected, as one commit; what is not named stays; `undo` brings back what it said before |
| `retire` | a memory, a reason, and the memory that takes its place if there is one | the memory marked retired with its reason, as one commit; nothing is deleted: it is still read, and comes last in a search |
| `journal` | a text; a day and a repository if wanted | the time and the text added at the end of that day's page (`journal/2026-11-27.md`), today unless a day is named, as one commit. `read` takes a day and gives its page |
