# keeping — contract

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`. Stage design: the clause ids may still change. Every clause here is moved word for word from the root's contract; the root's `VALIDATION.md` gives each one's id of before.

## 1. Role

The part of the plugin's program that keeps the memory true, a memory corrected or put aside, and keeps the days in the journal.

## 2. Does and does not

- Corrects a memory that was wrong, and puts aside one that no longer holds: change, retire.
- Keeps the days: what happened and what was decided, a page a day: journal.

Does not:

- Say in the rules of `plop` when a fact is changed or retired → `opening`.
- File a new memory, take a change back → `writing`.
- Write the piece's file, make the commit → `store`.

## 3. Depends on

`store`, `tools`, `reading`. Nothing else from the repository.

## 4. Keeping the memory true

We want a memory that stays right as life changes: what was wrong is corrected, what no longer holds is put aside, and nothing is lost.

- **C-4.1** `change` replaces what the call names of a memory or of a person: its text, its title, its kind, its tags. What the call does not name is unchanged. It is one commit; the text before stays in the folder's history, and `undo` brings it back.
- **C-4.2** `retire` marks a memory as no longer holding, with the reason given, as one commit. Nothing is deleted: `read` still gives it, said as retired with its reason; `search` gives it after the others, said as retired; the count `plop` gives leaves it out.
- **C-4.3** `retire` can name the memory that takes its place: the two are linked, and reading the retired one names the other.

## 5. The journal

We want the days kept as well as the facts: what happened and what was decided, a page a day, in the person's own folder.

- **C-5.1** `journal` adds an entry to the page of a day, today unless the call names another: the time and the text, at the end of the page. What was there is unchanged. It is one commit, and `undo` takes it back. With several repositories it goes to the one the call names, otherwise to the folder's own.
- **C-5.2** `read`, given a day, gives that day's page. `search` finds in the journal as in the memories, each match said with its day.
- **C-5.3** `plop` says, after the state, the last day the journal was written.

## 6. Behaviour

| Situation | Reaction |
|---|---|
| `retire` of a memory already retired | one sentence that says so; nothing written |
| `read` of a day with no page | one sentence that says so |

## 7. Wanted, not promised yet

- A tidy-up the person asks for now and then: what looks said twice, what contradicts, what was never linked, each as a proposal.

## 8. Non-goals

none
