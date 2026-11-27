# writing — a memory filed, a note, a link, a change taken back

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Four writing tools: `remember` files a new memory, `note` adds a dated line to a memory or a person, `link` links two pieces, `undo` takes back a change of Lorisse's. Each is at most one commit by Lorisse when the folder is under git, made by the store. How a writing call reads its arguments and refuses is shared with the other writing subjects, in `crates/tools/src/shared/`.
What it promises: `CONTRACT.md`. How that is checked: `VALIDATION.md`.

## The folder layout

```text
writing/
└── mod.rs        remember, note, link, undo, and the sentence for undo in a folder not under git
```

## The tools

| Tool | It takes | It gives |
|---|---|---|
| `remember` | a title and a text; a kind, tags, links and a repository if wanted | a new memory, filed as one commit; its number |
| `note` | a number or a person's name, and a text | one dated line added at the end of the piece, as one commit |
| `link` | two pieces | the first linked to the second, as one commit; a link already there changes nothing |
| `undo` | nothing, or the change to take back | Lorisse's last change taken back as a new commit; never a change made by someone else |
