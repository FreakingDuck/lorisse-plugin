# first_day — contract

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`. Stage design: the clause ids may still change. Every clause here is moved word for word from the root's contract; the root's `VALIDATION.md` gives each one's id of before.

## 1. Role

The part of the plugin's program that makes a new person's memory folder ready, and fills it with what another assistant already remembers of them.

## 2. Does and does not

- Makes a new person's memory folder ready, and fills it with what another assistant already remembers of them: start, import.

Does not:

- Find the memory folder once it is made → `reading`.
- Give the intro back at the start of a conversation → `opening`.
- Make a repository → `drawers`, called by Claude.
- Write the first set of rules → `guard` ships it; start copies it.

## 3. Depends on

`store`, `guard`, `tools`, `reading`, `opening`. Nothing else from the repository.

## 4. The first day

We want a new person to have a memory in five minutes, without knowing what a folder under git is, and a first conversation that does not start empty.

- **C-4.1** `start` makes a memory folder ready: a folder named `Lorisse` in the person's home folder, unless they name another place, holding the settings, the first set of rules and their page of intro, and nothing of anyone else's. A place that already holds files is never taken over: it is refused, unless it is empty or already a memory folder.
- **C-4.2** When git is on the machine, the new folder is put under git, on the machine only, with a name for its commits, and its first state is one commit. Without git the folder is made all the same, and the answer says in one sentence what git would add and that it can come later.
- **C-4.3** The machine remembers the folder `start` made: from then on Lorisse finds it with no setting to fill, in every conversation on that machine.
- **C-4.4** `start` writes the person's intro from what they said: their name, what they want to remember, how they like to be answered.
- **C-4.5** `start` gives, in a block the person can copy whole, the text to hand to another assistant to get back what it already remembers of them, as dated lines by category.
- **C-4.6** `import` takes that answer: each dated line becomes a memory with its date and its category, in the person's own words; the lines that are instructions are not filed, they are given back as proposals for the intro, never obeyed. The whole import is one step, taken back by one `undo` when the folder is under git, and the answer gives the count.
- **C-4.7** A line `import` cannot read is left out and counted, never guessed.

## 5. The drawers

We want a person to talk and Lorisse to sort: the house, the work, a client, a project each in its own repository, which Lorisse calls a drawer when it speaks, made without the person having to know what a repository is, and never a trap: what was put in the wrong one is moved.

- **C-5.1** On the first day, `start` tells Claude to let the person talk about themselves in open questions (who they are, what fills their days, what they would hate to lose, what must never mix), and to make the repositories from what they say, by itself.

## 6. Behaviour

| Situation | Reaction |
|---|---|
| `start` when this machine already has a memory folder | one sentence that names it; nothing is made |

## 7. Wanted, not promised yet

none

## 8. Non-goals

none
