# writing — contract

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`. Stage design: the clause ids may still change. Every clause here is moved word for word from the root's contract; the root's `VALIDATION.md` gives each one's id of before.

## 1. Role

The part of the plugin's program that files a memory, adds a note to a piece, links two pieces, and takes back its own last change.

## 2. Does and does not

- Files a memory, adds a note to a piece, links two pieces, and takes back its own last change: remember, note, link, undo.

Does not:

- Keep a document with a piece → `files`.
- Correct or retire a memory, write the journal → `keeping`.
- Make a repository, move a piece → `drawers`.
- Decide when a tool is called, or how a piece is filed → `keeper`.
- Number a piece, write its file, make the commit → `store`.

## 3. Depends on

`store`, `tools`, `reading`. Nothing else from the repository.

## 4. Writing

We want a memory filed in one sentence, each change one step the person can see in the folder's history and take back, and nothing else touched. When a tool is called is the keeper's: by itself when the person says something worth keeping, each filing said in one line; the person's own intro can say otherwise.

- **C-4.1** `remember` files a new memory: its title, its text, its kind, its tags, the pieces it links to. The answer gives its number, and `read` gives it back as it was filed.
- **C-4.2** `note` adds one dated line at the end of the text of a memory or of a person; what was there is unchanged.
- **C-4.3** `link` links one piece to another. A link already there changes nothing, and the answer says so.
- **C-4.4** Each change is exactly one commit, in the repository the piece lives in; its message says what was done and names the piece. No other file the folder keeps under git changes, and what the person had changed without committing is left as it was. When that repository keeps a history of its own and takes its numbers from the folder's counter, the counter is one more commit, in the folder's own history. After a change, and after its undo, Lorisse leaves nothing of its own uncommitted.
- **C-4.5** `undo` takes back the last change Lorisse made, or the one named, as a new commit. The history is never rewritten, and a commit Lorisse did not make is never taken back.
- **C-4.6** A number is given once and never reused, even after an undo. Nothing is deleted.
- **C-4.7** Nothing is written in a part marked confidential, nor in a repository marked so, unless that repository is open to Claude (`tools` C-5.3): asked to, the tool refuses in one sentence. In a repository open to Claude, the writing tools work as in any other.
- **C-4.8** In a folder of several repositories, a new memory goes to the one the call names; without a name, to the folder's own.
- **C-4.9** A change that would break the folder's rules (a kind the folder does not know, a link to a piece that does not exist) is refused with its reason, and nothing is written.
- **C-4.10** Git is recommended, not required. In a folder that is not under git, the writing tools write all the same: the piece is filed, there is no history, and `undo` says it has none to go back through. `plop` says once, in one sentence, what git would add.

## 5. Behaviour

| Situation | Reaction |
|---|---|
| A writing tool, the folder under git, but no git on the machine or no name set for its commits | refused: one sentence that names what is missing; nothing written |
| `undo` with no change of Lorisse's to take back | one sentence that says so |
| A change that fails halfway | the folder is as it was before the call |

## 6. Wanted, not promised yet

- Filing a person; proposals for anything heavy (merge two pieces, move one, rename a tag).

## 7. Non-goals

none
