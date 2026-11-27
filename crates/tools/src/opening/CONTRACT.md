# opening — contract

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`. Stage design: the clause ids may still change. Every clause here is moved word for word from the root's contract; the root's `VALIDATION.md` gives each one's id of before.

## 1. Role

The part of the plugin's program that starts a conversation: `plop`, with the person's intro, the state of the memory and the rules of keeping, and the instructions given when the Claude app connects.

## 2. Does and does not

- Opens a conversation with the person's own intro, the state of the memory and the rules of keeping, when the person says "Lorisse" or "plop": plop.

Does not:

- Find the memory folder → `reading`.
- File, change or sort what the rules of keeping speak of → `writing`, `keeping`, `drawers`.
- Decide when a tool is called, or how a piece is filed → `keeper`.

## 3. Depends on

`store`, `tools`, `reading`. Nothing else from the repository.

## 4. Starting a conversation

We want one word, "Lorisse" or "plop", to start Lorisse as the person set it up, on any machine, without retyping who they are and how they work.

- **C-4.1** `plop` gives the person's intro: the text of the page `intro.md` at the root of the memory folder, whole. The person changes it as any file of theirs, and the next `plop` gives the new text. Without that page, it gives a short one of the plugin's own and says where to write theirs.
- **C-4.2** After the intro, `plop` gives the state: the memory folder, its repositories, how many memories and people it holds, and the five memories filed last.
- **C-4.3** The intro is the person's own word on how Lorisse works for them, and is said as such; every other text of the folder stays data (`tools` C-5.2).
- **C-4.4** `plop` ends with the rules of keeping, in a few lines: what is filed without being asked, starting with what the conversation already holds; what is never filed; how a filing is said; with several repositories, that a memory goes to the one its subject belongs to, and that what comes from a confidential one is never filed in another. So each conversation opened with it starts with the rules in front of Claude, whatever else it has read.
- **C-4.5** When the Claude app connects to the program, the program gives it its own short instructions, which the app keeps in front of Claude for the whole conversation: what Lorisse is, to search it when the person asks what they know, and that filing by itself starts once `plop` has been called in the conversation; before that, Lorisse files only when asked.
- **C-4.6** Every text the program gives Claude says the same of filing: the instructions at the connection, the description of each writing tool, and the rules of `plop`. None says "only when asked" where another says "by yourself".

## 5. Keeping the memory true

We want a memory that stays right as life changes: what was wrong is corrected, what no longer holds is put aside, and nothing is lost.

- **C-5.1** The rules `plop` gives say when: a fact that changed is changed, not filed a second time; what no longer holds is retired, never left beside its contrary.

## 6. The drawers

We want a person to talk and Lorisse to sort: the house, the work, a client, a project each in its own repository, which Lorisse calls a drawer when it speaks, made without the person having to know what a repository is, and never a trap: what was put in the wrong one is moved.

- **C-6.1** `plop` says each repository with what it is for, so that a memory is filed where its subject lives.
- **C-6.2** The rules `plop` gives say when: a subject that keeps coming and has no repository of its own gets one, said in one line; a piece found in the wrong one is moved; the person is never asked to choose where a thing goes.

## 7. Behaviour

none: the rows that hold for any call are in the contract of `tools`.

## 8. Wanted, not promised yet

none

## 9. Non-goals

none
