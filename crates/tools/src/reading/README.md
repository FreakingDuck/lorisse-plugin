# reading — the memory folder found, and the four questions on it

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

The memory folder is found at each call, and four tools ask it questions: where it is, search, read, links. They open the folder for reading only and write nothing. `read` also gives a day of the journal, names the files a piece holds, and says a retired memory as such; `search` finds a piece by a file's name.
What it promises: `CONTRACT.md`. How that is checked: `VALIDATION.md`.

## The words

- **memory folder**: the folder that holds `config.yaml` at its root, never one of its repositories.
- **how it was found**: the folder the session is opened on, the plugin's setting, or the folder this machine remembers from the first day.

## The folder layout

```text
reading/
├── folder.rs     the memory folder of a call: the session's folder, else the setting, else the one remembered
└── mod.rs        where, search, read, links, and what search and read say of a piece
```

## The tools

| Tool | It takes | It gives |
|---|---|---|
| `where` | nothing | the memory folder and how it was found (the plugin's setting, or the folder the session is opened on), and the plugin's version |
| `search` | a text | the memories, people and tags whose title, tags or text hold it, capitals ignored, a match in a title first: number, title, the line that matched. Ten at most. |
| `read` | a number, a person's name or a tag's name | every field of the piece's header, then its text, cut past 6000 characters |
| `links` | a number | the pieces it links to and the pieces that link to it, each with number and title; a link that leads to no piece is said as such |
