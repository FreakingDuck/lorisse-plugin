# opening — the word that starts a conversation

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

`plop` opens Lorisse as the person set it up: their own intro, else the plugin's, then the state of the memory, the services their rules already rule, the last day the journal was written, and the rules of keeping. The instructions the program gives the Claude app when it connects live here too, so that every text Claude reads says the same of filing. So does `rule`, which writes the person's answer in their rules; its promises are the guard's (`crates/guard`, part 6).
What it promises: `CONTRACT.md`. How that is checked: `VALIDATION.md`.

## The words

- **intro**: the page `intro.md` at the root of the memory folder, the person's own word on how Lorisse works for them.
- **the rules of keeping**: the last lines of `plop`, what is filed by itself and how.

## The folder layout

```text
opening/
├── mod.rs        plop, the intro's lines, the rules of keeping, the instructions, the state of the memory
└── rules.rs      what plop says the person's rules rule; rule, through the guard's writer and one commit of the store
```

## The tools

| Tool | It takes | It gives |
|---|---|---|
| `plop` | nothing | your intro (the page `intro.md` at the root of your folder), then the state: the folder, its repositories, how many memories and people, the five filed last; then the services your rules already rule |
| `rule` | a service, its tools, and pass, ask or never | the rule written in your `rules.yaml`, one commit; what the file now says for that service |
