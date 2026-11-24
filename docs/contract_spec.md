# Contract spec — how a part's three documents are written

> written by Claude (Fable 5.1) 2026-09-22 · reviewed by the owner: 2026-11-23, ok

Every part (a row of the table with its `path`, whatever the stack) carries three files, read in this order by a person who has to trust it:
`README.md` (how it is built, for the developer), `CONTRACT.md` (what it must do, for the
reviewer), `VALIDATION.md` (how each promise is checked and when it last passed). This file
is the format of the three. **Human files**: the owner writes and corrects them; an
assistant only drafts.

**The owner's marks are the review state.** Whether a part's three documents are drafted,
read or agreed is never typed in the table: it is the owner's mark on each document's review
line (read, ok, to rework, with his date; `review_line.md`), and the run log of
`VALIDATION.md` says when the steps were walked. A part he has not read yet needs no tracking
beyond its stage and what the agent may touch; `to rework` with his `// ` notes is what the
assistant acts on; his `ok` on each of the three documents and a run in the log make the part `desk`.

## README.md — the two kinds

- **A part's README is a map.** It is for the person who reads the code once with this page as the only map: what is where, never why. The rules below are the map's.
- **A way in** is the README at a repository's root, or of a thing a person installs. It says what the thing is, how to install it, how to start and where to read next, and nothing of how it is built.

## README.md — rules

1. **True to the code.** Every name the page quotes exists in the code, and the page is written after reading the code, never from the contract or from memory. The owner reviews a part through its three pages: a README that is wrong makes his review wrong.
2. **It opens the same way in every part**: the title with one line of what the part does; the review line; one paragraph, what the part is and who calls it; one line pointing to `CONTRACT.md` for what it promises and to `VALIDATION.md` for how that is checked.
3. **The same sections in the same order**, so a reader finds the same thing at the same place: the words, the folder layout, the flow, the API. Then, when the part has them: the tool, device or protocol it drives, and what is chosen, not promised.
4. **The words**: the three to six nouns a reader meets in this part, one line each, in the order he meets them. The part's words, never the language's: "a job is what a step does" is one; "a module" is not.
5. **The folder layout**: a text tree in the order things load or run, one line a top-level file. A folder's files are named only where their order matters; the tests take one line.
6. **The flow**: a drawing of the one path a call or a run takes, boxes and arrows, under eight lines. Then at most five numbered lines in plain words, each opening on the boxes it explains.
7. **The API**: one bullet a consumer, the entry points it uses, by name only. Then the commands that run the part and its tests, in a code block. Where the consumer is a screen, a table of one row a screen: what it asks, what it is told.
8. **Chosen, not promised**: the last section, under that heading. It holds the choices of the build: the settings with their values, the exact names and formats, the mappings, the measured numbers, as a list or a table. They work and they are finished; the contract does not promise them (its rule 2), so another choice can be made later without the contract changing. The section opens by saying so, and that it is for whoever works on the code.
9. **What it never holds** has its place elsewhere:

   | Not in the README | Goes to |
   |---|---|
   | why it is built so | the contract |
   | how it came to be, a date | git |
   | the tests themselves | the validation |
   | what each screen does, the owner's walk | the contract and the validation |
   | what a function does, line by line | the code's own comments |
   | a paragraph that argues | nowhere |

10. **Under eighty lines.** A longer one is a list of everything that was built: cut it back to the map. No gate fails a README; the owner's read does.

## README.md — template

````markdown
# <part> — <what it does, one line>

> written by <who> <date> · reviewed by the owner: not yet

<One paragraph: what the part is and who calls it.>
What it promises: `CONTRACT.md`. How that is checked: `VALIDATION.md`.

## The words
- **<noun>**: <one line>

## The folder layout
```text
<part>/
├── <file>        <what it holds, one line>
└── <folder>/     <its files, named where their order matters>
```

## The flow
```text
<input> ─▶ <box> ─▶ <box> ─▶ <output>
```
1. **<box> ─▶ <box>**: <what happens there, in plain words>

## The API
- **<a consumer>**: <the entry points it uses, names only>

```text
<the command that runs it>
<the command that runs its tests>
```

## Chosen, not promised
The choices of this build, for whoever works on the code. The contract does not promise them: another choice can be made later without the contract changing.
- <a setting>: <its value>, <what it is for>
````

## CONTRACT.md — rules

1. **A clause is a promise a person can check** from the terminal, a client or a file, without
   reading the code. One falsifiable sentence.
2. **The contract names no function, type, flag or file.** The code is the how; the README
   says it. A file format the part writes or reads is a promise and may be spelled.
   **The test for a clause: would it still be true if the part were written again from
   nothing?** If yes, it is a promise. If it would change with the code (how things are
   wired, a value someone tunes, the wording of a message), it describes the build and goes
   to the README. "What is saved is still there after a restart" is a promise; "it saves
   every 30 seconds" is a setting.
3. **Each theme opens with what we want and why**, in the owner's words, then its clauses.
4. **Ids `C-<section>.<n>` are never renumbered** once the part's `stage` in `status.yaml` is
   `validated`; while it is `design` they may change, and the header says so. A new clause
   goes at the end of its section; a new theme goes after the last theme.
5. **No status in the contract**: no tick, date or verdict. That is `VALIDATION.md`, one row
   per clause.
6. **Intent with no code behind it** goes to "Wanted, not promised yet", never into a clause.
   An issue found on a locked part is another thing: what would be changed today if the part
   were not locked. It goes to the project's backlog and is fixed when the part is reopened.
7. **Sections come in the fixed order below.** An empty section stays, with "none".
8. **The size signal**: a Role that needs two sentences of purpose is two parts.
9. **"Does not:" stands as its own paragraph**, a blank line before it and its list after it:
   without the blank line a markdown renderer folds it into the last bullet above.
10. **A clause is one promise in one or two short sentences.** What another part owns is
    named, never restated: a part that follows the method's review line or its notes says so
    and names the page. How a thing looks or is stored is not a promise: a list of colours, a
    key of a file, a layout's mechanics go to the README, and a screen's look to the design
    the owner okayed.
11. **Human-sized.** A contract the owner does not read to its end promises nothing. Past a
    few themes and a few dozen clauses the part is several parts: split it, each with its own
    three pages.

## CONTRACT.md — template

```markdown
# <part> — contract

> written by <who> <date> · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

## 1. Role
<one paragraph: what it is, for whom, on what platform>

## 2. Does and does not
- <what it offers, in words>

Does not:
- <thing> → `<part that owns it>`

## 3. Depends on
`<part>`. Nothing else from the repository.

## 4. <Theme>              (one section per theme, 4, 5, 6 …)
<what we want and why>
- **C-4.1** <one checkable sentence>

## <n>. Behaviour
| Situation | Reaction |
|---|---|

## <n>. Wanted, not promised yet
- <intent, one line>

## <n>. Non-goals
<one line>
```

## VALIDATION.md — the page, top to bottom

The page is the continuation of the README (how it works) and the contract (what it promises): the walk is where the owner sees the promises hold. Two pages to look at: `page/VALIDATION.md` for a screen, `method/VALIDATION.md` for a terminal.

1. **The review line**, as on every page.
2. **The last run, one row**: `| Last run | Where it stands | Waiting on |`. It is the first thing he reads: is everything fine, or is something waiting on him. Short. A run seldom walks every step: "Where it stands" names the steps that run walked, then the steps left out and the ones to walk again, so the row is true for the whole page and not for that one run.
3. **The ways it is validated**, one line a way, opened by a sentence that counts them ("This part is validated two ways:"): by the gate (which cases, in one sentence); by hand at the desk (where the steps run); on the bench, when the part drives hardware (the `R-n` steps, and what they need). Said once here, never repeated under each step.
4. **Before you start**: numbered moves, each with what he sees. The gate first, then the sample the walk runs on.
5. **The steps**, under headings without numbers.
6. **For the gate: one row a promise.** One table at the end, under a heading that says whose it is, with one sentence: it is for the gate and the assistant, nothing there is the owner's to read.
7. **The run log.**

## VALIDATION.md — rules

1. **A part's validation walks that part alone.** Another part may be a tool to set a state; it is never the thing checked. A step that checks another part moves to that part's page.
2. **A walk runs on a sample made for it.** One script makes it, the same at each run. No step depends on the day, on the state of the real repository or on the owner's real marks, and no step changes them: the walk can be run at any time. On the bench the sample is a setup: "Before you start" names the board, its wiring and what is flashed on it, and each `R-n` step starts from that state and says how to come back to it.
3. **The steps follow what he does, not the contract.** For a screen: the screens in the order he meets them, one heading a screen. For a terminal: one heading a thing he runs. The contract's order lives in the table at the end.
4. **A step is a title, what it proves, then moves.**
   - the title, a checkbox line: `- [ ] **H-n** <what the step shows, one line>` (the page's generator counts the steps by that shape);
   - under it, one sentence: "It proves C-x and C-y: why it matters", what would go wrong without it;
   - then numbered moves, each one thing done and, under it, `- See:` one thing seen.
5. **Every move is asked the same way.** "In the terminal, run `<the command, whole>`." "Open `<file>` as text." Nothing to guess, nothing to put together from two lines.
6. **A See says what is there.** The line quoted, or what is on the screen. Never an absence ("no icon beside it"): what must not be there is the gate's to check. Never a count of steps or of cases: it goes stale.
7. **By hand is a walk, not a list.** Fifteen minutes, a dozen steps at most, each one a thing a human must see or hear (a look, a sound, a phone width, a real server). A step a test can prove is not a by-hand step: it becomes a test, or goes. A new feature replaces or reworks a step, never pushes past the dozen: a list nobody runs validates nothing.
8. **A step that cannot be run says so**: `- Waits on:` what it needs (a sample to grow, an adapter, a board, another person), in place of its moves. The line goes when the step is run.
9. **One row a promise, for the gate.** One row a clause: `| Clause | What is checked | Automated | By hand |`. A clause with no automated test has a by-hand step, on purpose, and the row says so.
10. **Human-mind-sized.** The page is read by a person, so the table never names a test function or a test's assertion. Its Automated column says a count and a kind: "the gate's step 2", "3 checks · input (a bad file, an empty one, a missing key)", "2 checks · output (the line printed, the exit code)", "1 check · writes", or "—". The names stay in the code.
11. **The run log is the only status**: date, who, which steps, verdict, one short line of what the next pass must know. No tick lives anywhere else. A failed try and its fix the same day are one row, the fix's; measured facts (times, sizes) go to the README or the contract's Behaviour table; the session file keeps the long form.
12. **The ids hold.** `H-n` needs no hardware, `R-n` needs the bench; never renumbered once the part's `stage` is `validated`; in `design` the list is rewritten clean, no "retired" step.
13. **A named test is locked**: it exists, and it is never edited to make a change pass unless its clause changes in the same checkpoint.
14. **A suite's module says what it proves, not how.** The sentence a suite module prints first names the thing checked and why it matters, in the operator's words ("the decoder gives the example file's 25 lines back exactly, even when the bytes arrive 100 at a time, as a serial port cuts them"); the mechanics stay in the lines under it. The suite prints the verdict and the lines behind it. Two sentences at most.

A step, as it reads:

```markdown
- [ ] **H-2** The review report gives each document one row: its state, then your mark.
  It proves C-5.1 and C-5.2: you see at a glance which documents wait for you.
  1. In the terminal, run `powershell -NoProfile -ExecutionPolicy Bypass -File method/checks/check_reviews.ps1 -Root target/walk-sample`.
     - See: one row a document. The three of `alpha` say `reviewed`, a day, `ok`.
  2. Read the last line.
     - See: `reviews: 19 file(s): 12 reviewed, 2 changed since, 5 never reviewed`.
```

## VALIDATION.md — template

```markdown
# <part> — validation

> written by <who> <date> · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

| Last run | Where it stands | Waiting on |
|---|---|---|
| <date> | <the steps that run walked; the steps left out or to walk again> | <what waits on the owner, or nothing> |

This part is validated two ways:

- **By the gate**: <which cases, one sentence>. Promise by promise, it is in the table at the end.
- **By hand**: the steps below, <where they run>, on <the sample>.
- **On the bench** (a part with hardware; the sentence above then says three): the `R-n` steps, on <the board and its setup>.

## Before you start

1. In a terminal at the repository root, run `<the gate's command>`.
   - See: its last line, `<the green line>`.
2. In the terminal, run `<the command that makes the sample>`.
   - See: <what it prints>.

## <A screen, or a thing run>

- [ ] **H-1** <what the step shows, one line>.
  It proves C-4.1 and C-4.2: <why it matters>.
  1. <one thing done, the command whole>.
     - See: <one thing seen, quoted>.

## For the gate: one row a promise

For the gate, which checks that each promise of the contract has one row here, and for the assistant; nothing here is yours to read.

| Clause | What is checked | Automated | By hand |
|---|---|---|---|
| C-4.1 | <the promise in a few words> | 3 checks · input (a bad file, an empty one, a missing key) | H-1 |

## Run log
| Date | Who | Steps | Verdict | Notes |
|---|---|---|---|---|
```
