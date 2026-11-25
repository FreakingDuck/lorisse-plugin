# store — the one door to a memory folder

> written by Lorisse 2026-11-24 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

The crate every read of a memory folder goes through, and later every write. It was written for a desktop app before the plugin existed and is being fitted to the plugin; this page stays short until that is done.

## The words

- **memory folder**: a folder of plain text files under git, one file a piece.
- **piece**: a memory, a task, a person, a tag, a raw capture in the inbox. A header of fields, then a text.
- **number**: a piece's name for life, `M-0001` for a memory, `T-0001` for a task. Never reused.
- **repository**: a memory folder can name several in its config; they read as one memory.
- **confidential**: a part of the folder the person marked so. It is read only by an opening that holds that right.

## The folder layout

```
crates/store/
├── src/        the crate
├── tests/      its tests, each on a copy of a fixture or of the sample
└── fixtures/   two small folders: one valid, one with broken pieces
```

## What it does today

- Opens a folder, reads its config and indexes every piece.
- Opens a folder for reading only (`Store::open_read`): the index is built in memory, no file is written, no git is run, and a writing call is refused. It is the opening the plugin uses.
- Reads one piece, lists pieces by kind, tag, state or date, searches titles, tags and texts, gives a piece's links both ways.
- Lists the inbox and the proposals waiting.
- Writes: a capture, a proposal and its acceptance, a direct change as one commit with its undo. The plugin uses none of the writing yet.
- Checks a folder and reports each problem found.

## The proposal file

An agent (or the owner) proposes by writing one YAML file per action in the inbox, named
after the capture it comes from: `inbox/<id>.sort.yaml`, then `inbox/<id>.2.sort.yaml`,
`inbox/<id>.3.sort.yaml` … (`Store::proposal_path` gives the next free name). The file is
the `Action` enum as YAML, `action:` naming the variant; leading `# why:` (one line, said to
the owner) and `# by:` (job, provider, model) comments are read by the app, any other
comment is ignored (`Store::apply` adds `# certainty: sure | unsure` on a job's proposal). **A key not shown here refuses the file** (the parser's message names
it). Layer words are singular: `memory`, `task`, `diary`, `person`, `conversation`, `tag`. Ids are the store's:
a sort's new id is given at accept, never written in the file. Accepting a file removes it;
skipping renames it `<name>.skipped.yaml`; the capture is retired when no file of it is
left.

Sort into a layer (`confidential`, `slice`, `set`, `sorted_by` optional; `layer: memory`
keeps the capture's id):

```yaml
# why: a thing to do before Friday
# by: sort anthropic claude-haiku-5-5
action: sort
id: M-0007
layer: task
confidential: false
slice: null
set:
  title: Call the bank about the card
  priority: high
  category: admin
  tags: [bank]
  links: [M-0001]
sorted_by: sorter
```

Sort into a person (`people/<slug of name>.md`; the capture's words under the first section):

```yaml
# why: a contact to keep
# by: sort anthropic claude-haiku-5-5
action: sort
id: M-0007
layer: person
confidential: true
set:
  name: Sam Example
  role: sells battery cells
  company: Example Co
  contact: sam@example.com ; 06 00 00 00 00
  tags: [battery]
  links: [M-0001]
sorted_by: sorter
```

Edit (`set`, `unset`, `body` optional; `body` replaces the whole body):

```yaml
# why: the next step is known
action: edit
id: T-0001
set:
  priority: high
  next: Write the proposal section
unset: [due]
```

Link (adds the targets not yet there):

```yaml
action: link
from: M-0001
to: [M-0004, people/robin]
```

Dedupe of two memories or two tasks (keeps the older id, retires the newer; a task retired
goes to the archive; every link to the dropped one turns to the kept one):

```yaml
action: dedupe
keep: M-0001
drop: M-0002
```

Dedupe of two people (keeps the one named `keep`, retires the other where it lies):

```yaml
action: dedupe
keep: people/robin
drop: people/noa
```

Archive (a task; one not done is set `done` and closed with `- Done <today>: <next, else
title>` under `## Notes / history` first):

```yaml
action: archive
id: T-0001
```

Retire (a memory; a test capture is retired where it lies; `behind` optional, the résumé it
sits behind, written as `retired_by`):

```yaml
action: retire
id: M-0007
```

```text
action: retire, id: M-0041, behind: M-0039
```

The answers proposed on a task (an edit setting `resolutions`, each `{text, does}`, `does` one of
`answer`, `drop`, `archive`; the store writes `answers` beside them as their texts;
`Piece::resolutions()` reads them, else `answers` as plain ones):

```text
action: edit
id: T-0004
set:
  question: "L'inscription : faite ?"
  resolutions:
    - { text: Faite, does: archive }
    - { text: Abandonner, does: drop }
    - { text: Plus tard, does: answer }
```

Move (a filed piece; `layer`, `confidential`, `set` optional; across `confidential/` the id is
kept; a memory to a task takes the next `T-` id, a task to a memory the next `M-` id, a memory to
a person `people/<slug of set.name>`; the old layer's own keys dropped and listed, the old piece
retired linking the new, every link to it turned):

```yaml
# why: it is a thing to do, not a memory
action: move
id: M-0004
layer: task
set:
  priority: high
```

Rename a tag (every piece carrying `from`, duplicates folded; `tags/<from>.md` renamed to
`tags/<slug of to>.md`, or merged into it when it exists; `why` optional):

```yaml
action: rename_tag
from: voice
to: audio
why: one word for the sound
```

Sort into a tag (`tags/<slug of set.tag>.md`: `tag`, `definition`, `aliases`; the capture named):

```text
action: sort, id: M-0007, layer: tag, set: { tag: famille, definition: le père, la mère }
```

## Chosen, not promised

- The full opening writes the folder's index and its counters; the opening for reading writes nothing. The plugin uses the second only, until the writing step.
- Git is run as a program, one call at a time; no git library.
- Five crates: chrono, serde, serde_json, serde_yaml_ng, thiserror.
