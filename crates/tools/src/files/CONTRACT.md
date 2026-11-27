# files — contract

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`. Stage design: the clause ids may still change. Every clause here is moved word for word from the root's contract; the root's `VALIDATION.md` gives each one's id of before.

## 1. Role

The part of the plugin's program that keeps a document with the memory it is about.

## 2. Does and does not

- Keeps a document with the memory it is about: attach.

Does not:

- Open a file it keeps → the Claude app's own reader.
- Find the memory folder, read a piece → `reading`.
- Copy the file, make the commit → `store`.

## 3. Depends on

`store`, `tools`, `reading`. Nothing else from the repository.

## 4. Files

We want a document kept with what it is about: the quote with the works, the letter with the dispute.

- **C-4.1** `attach` puts a copy of a file of the person's machine in the memory folder, with the piece it is given for, as one commit. The original is left where it was.
- **C-4.2** `read` names the files a piece holds, each with its path in the folder, so that Claude can open one with the Claude app's own reader.
- **C-4.3** `search` finds a piece by the name of a file it holds.
- **C-4.4** A file over the folder's limit (20 MB unless the folder's settings say otherwise) is refused with its size, and nothing is copied.
- **C-4.5** A file is never replaced: one of the same name already with the piece makes the new one take a numbered name.
- **C-4.6** A key or a secret is never attached: a file under a `.ssh` folder, a `.pem`, a `.env`, or a name that ends in `_key` or `_token`, is refused with the reason.

## 5. Behaviour

none: the rows that hold for any call are in the contract of `tools`.

## 6. Wanted, not promised yet

- Files: with the Claude app's own connector to a file service, Lorisse files the documents that arrive and keeps a memory of each. No connector of ours.
- A large file kept by reference: the memories live in git, the large documents in the person's file service; `attach` of a file past the limit, or of one already in that service's folder, keeps its path instead of a copy.

## 7. Non-goals

none
