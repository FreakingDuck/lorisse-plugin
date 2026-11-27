# files — a document kept with its memory

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

`attach` keeps a copy of a file of the person's machine beside the memory or the person it is about, as one commit; the original stays where it was. The store copies the file and refuses a key, a secret or a file past the limit. Reading a piece names its files, and a search finds a piece by a file's name: that code is `reading`'s.
What it promises: `CONTRACT.md`. How that is checked: `VALIDATION.md`.

## The folder layout

```text
files/
└── mod.rs        attach, and the sentence for a file not copied
```

## The tools

| Tool | It takes | It gives |
|---|---|---|
| `attach` | a memory or a person, and the path of a file on your machine | a copy of the file kept beside the piece (`memories/M-0001/invoice.pdf`), as one commit; the original is left where it was. 20 MB at most unless your folder's settings say otherwise (`attach_limit_mb`); never a key or a secret |
