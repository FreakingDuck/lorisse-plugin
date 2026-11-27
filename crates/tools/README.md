# tools — the program the Claude app starts

> written by the cut pass 2026-11-28 · reviewed by the owner: not yet

The program the Claude app starts on the person's machine: an MCP server over stdio, one JSON-RPC message a line, that gives Claude seventeen tools and nothing else. Each tool belongs to a subject, one folder under `src/` with its own three pages; this crate holds what is common: the protocol, the list of the tools, the guard mode, and what the subjects share.
What it promises: `CONTRACT.md`. How that is checked: `VALIDATION.md`.

## The words

- **subject**: one folder under `src/`, a part with its README, CONTRACT and VALIDATION, and the tools it gives.
- **the guard mode**: the same program started with the word `guard`: one tool call judged by the person's rules, through the `guard` crate.

## The folder layout

```text
crates/tools/
├── Cargo.toml        the program `lorisse`, version 0.7.0; the store and the guard, nothing else
├── src/
│   ├── main.rs       the protocol, the list of the tools with their declarations, the guard mode
│   ├── json.rs       a small JSON value, parser and writer, the standard library only
│   ├── shared/       the answer's shape, the limits, the memory folder opened at a call; write.rs, the writing call
│   ├── reading/      where, search, read, links; folder.rs, the memory folder found
│   ├── opening/      plop and the instructions at the connection
│   ├── writing/      remember, note, link, undo
│   ├── files/        attach
│   ├── keeping/      change, retire, journal
│   ├── drawers/      drawer, move
│   └── first_day/    start, import
└── tests/            the program started and asked over stdin, without Claude, on copies of the sample
```

## The flow

```text
the Claude app ─▶ main.rs ─▶ the subject's tool ─▶ shared (the folder opened) ─▶ store ─▶ the answer
```

1. **the Claude app ─▶ main.rs**: one line in, the method read; `tools/list` gives the cards, `tools/call` names a tool.
2. **main.rs ─▶ the subject's tool**: a reading tool is called with its argument; a writing tool goes through the writing call of `shared/write.rs`, which reads its arguments and checks that git can commit.
3. **shared ─▶ store**: the folder is opened for reading or for writing at each call, for the provider `anthropic`.

## The API

- **the Claude app**: `initialize`, `ping`, `tools/list`, `tools/call`, over stdin and stdout.
- **the plugin's hook**: the program started with the word `guard`.

```text
cargo build --offline -p lorisse
cargo test --offline -p lorisse
```

## Chosen, not promised

The choices of this build, for whoever works on the code. The contract does not promise them: another choice can be made later without the contract changing.
- A list's limit: ten. A text's limit: 6000 characters. A matched line's limit: 200 characters.
- The protocol's version when the app names none: `2025-06-18`.
- The program's own code uses the Rust standard library only, beside the store and the guard.
- The old `src/first_day.rs`, `src/folder.rs`, `src/tools.rs` and `src/writing.rs` are no longer compiled; `main.rs` names the paths of `first_day` and `writing` until they are removed.
