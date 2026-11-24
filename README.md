# Lorisse

A memory that is yours, in files you can read.

Lorisse is a plugin for the Claude app. You talk; it files what matters (a decision, a thing to do, a person) as plain files in a folder on your own machine, and finds them again when you ask.

## Where it stands

This is the first version, 0.0.1. It proves the way in: one tool, `where`, which answers when you say hello and tells the folder it runs in. It reads nothing and writes nothing. The filing tools come next, one small step at a time.

## Install

The steps, for Windows and for a Mac: [lorisse.ai/install](https://lorisse.ai/install/).

In short, in Claude Code:

```
/plugin marketplace add FreakingDuck/lorisse-plugin
/plugin install lorisse@lorisse
```

Then type "Hello Lorisse, where are you?".

Lorisse's tool is a small program that runs on your own machine. Cowork and Claude Code start it; a plain chat does not.

## What is inside

| Where | What |
|---|---|
| `src/` | the program, in Rust, the standard library only: no dependency |
| `server/` | the built programs, `lorisse.exe` for Windows and `lorisse` for a Mac (both Apple chips and Intel) |
| `skills/lorisse/` | the skill that tells Claude when to call the tool |
| `.mcp.json` | how the Claude app starts the program |
| `.claude-plugin/` | the plugin's card, and a marketplace of one |
| `.github/workflows/build.yml` | builds and tests the two programs from `src/` and commits them to `server/` |

The program speaks the Model Context Protocol on its standard input and output, one message a line. It opens no network connection.

## Build it yourself

```
cargo test
cargo build --release
```

Copy `target/release/lorisse` (or `lorisse.exe`) to `server/`.

## License

MIT, see `LICENSE`.
