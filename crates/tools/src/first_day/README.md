# first_day — a new person's folder, and what another assistant remembers

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

`start` makes a new person's memory folder ready, puts it under git when git is on the machine, writes their intro from their answers, and this machine then remembers it; it ends with the text to hand to another assistant. `import` files that assistant's dated lines in one step and gives back the lines that are instructions, never followed.
What it promises: `CONTRACT.md`. How that is checked: `VALIDATION.md`.

## The words

- **the folder remembered**: one line in `.lorisse-folder`, in the home folder, naming the folder `start` made.
- **dated line**: `[YYYY-MM-DD] - text`, or `[unknown] - text`, under Instructions, Identity, Career, Projects or Preferences.

## The folder layout

```text
first_day/
└── mod.rs        start (the folder made, put under git, remembered), import (the dated lines read and filed), and their unit tests
```

## The tools

| Tool | It takes | It gives |
|---|---|---|
| `start` | nothing, or a place, your name, what you want to remember, how you like to be answered | your memory folder made ready (`Lorisse` in your home folder unless you name a place), under git when git is on the machine, with your page of intro; this machine remembers it. It ends with a text to copy for another assistant, to get back what it remembers of you |
| `import` | that assistant's answer: dated lines under Instructions, Identity, Career, Projects, Preferences | each line filed as a memory with its date, in one step; the instructions given back as proposals for your intro, never followed; the count of what was filed and left out |
