# Lorisse

A memory that is yours, in files you can read.

Lorisse is a plugin for the Claude app. You talk; it files what matters (a decision, a thing to do, a person) as plain files in a folder on your own machine, and finds them again when you ask.

## What this repository is

The package the Claude app installs, and nothing else. It holds no memory: your memories live in a folder of your own, on your machine, that this repository never sees.

A plugin is three things in one folder:

| Piece | Where | What it is |
|---|---|---|
| the program | `src/` (its source), `server/` (built) | a small program the Claude app starts on your machine. It gives Claude a short list of tools and nothing else. It is the only thing that will ever read or write your memory folder. |
| the skill | `skills/lorisse/` | one page that tells Claude when to use the tools, and the rules to follow |
| the cards | `.claude-plugin/`, `.mcp.json` | the plugin's name and version, and the line that says how to start the program |

The rest is for building: `tests/`, `Cargo.toml`, and `.github/workflows/build.yml`, which builds the program for Windows and for a Mac and puts both in `server/`.

## Where it stands

Version 0.0.1 is the first of five steps. It is linked to no memory yet: it proves that the Claude app accepts the plugin and that Claude calls the program by itself.

| Step | What the program can do | State |
|---|---|---|
| 1. the way in | one tool, `where`: it answers "Lorisse is here" and the folder it runs in. It reads nothing, writes nothing. | done |
| 2. reading | open your memory folder: search, read a piece, list what waits, "what now". Nothing can be written. | next |
| 3. writing | file a memory, a task, a note, a link; each change is one step in the folder's history, and can be undone. Anything heavy is a proposal you accept. | after |
| 4. the keeper | the skill that holds the rules of filing: what is filed at once, what stays a proposal, what is never done | after |
| 5. the first day | an empty folder made ready for a new person | after |

## How the memory is kept

A folder you choose, under git: one file for one memory, one file for one task, one file for one person. Plain text you can open and read without Lorisse. Nothing is ever deleted; a piece that no longer holds is retired, and stays readable.

Claude never writes that folder free-hand. It asks the program, and the program checks the rules before it writes.

## Install

The steps, for Windows and for a Mac: [lorisse.ai/install](https://lorisse.ai/install/).

In short, in Claude Code:

```
/plugin marketplace add FreakingDuck/lorisse-plugin
/plugin install lorisse@lorisse
```

Then type "Hello Lorisse, where are you?".

The program runs on your own machine. Cowork and Claude Code start it; a plain chat does not. It opens no network connection.

## Build it yourself

```
cargo test
cargo build --release
```

Copy `target/release/lorisse` (or `lorisse.exe`) to `server/`. The program uses the Rust standard library only: no dependency.

## License

MIT, see `LICENSE`.
