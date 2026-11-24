# Lorisse

A memory that is yours, in files you can read.

Lorisse is a plugin for the Claude app. You talk; it files what matters (a decision, a thing to do, a person) as plain files in a folder on your own machine, and finds them again when you ask.

## What this repository is

The workshop where the plugin is made. It holds no memory: your memories live in a folder of your own, on your machine, that this repository never sees.

The folder `plugin/` is what the Claude app installs, and nothing else ships. It is three things:

| Piece | Where | What it is |
|---|---|---|
| the program | `plugin/server/` | a small program the Claude app starts on your machine. It gives Claude a short list of tools and nothing else. It is the only thing that will ever read or write your memory folder. |
| the skill | `plugin/skills/lorisse/` | one page that tells Claude when to use the tools, and the rules to follow |
| the cards | `plugin/.claude-plugin/`, `plugin/.mcp.json` | the plugin's name and version, and the line that says how to start the program |

The rest is the workshop:

| Where | What |
|---|---|
| `crates/tools/` | the program's source: the Claude app's protocol and the tools |
| `crates/store/` | the one door to a memory folder: how a piece is read, numbered and, later, written |
| `sample/` | a small memory folder, every word invented, that the checks and the walk run on |
| `status.yaml` | the table: one row per part, where it stands, what comes next |
| `scripts/verify.ps1` | the gate: every check of the repository in one command |
| `.github/workflows/build.yml` | builds the program for Windows and for a Mac and puts both in `plugin/server/` |

## Where it stands

Version 0.0.1 is the first of five steps. It is linked to no memory yet: it proves that the Claude app accepts the plugin and that Claude calls the program by itself.

| Step | What the program can do | State |
|---|---|---|
| 1. the way in | one tool, `where`: it answers "Lorisse is here" and the folder it runs in. It reads nothing, writes nothing. | done |
| 2. reading | open your memory folder: search your memories, read one, follow its links. Nothing can be written. | next |
| 3. writing | file a memory, a note, a link; each change is one step in the folder's history, and can be undone. Anything heavy is a proposal you accept. | after |
| 4. the keeper | the skill that holds the rules of filing: what is filed at once, what stays a proposal, what is never done | after |
| 5. the first day | an empty folder made ready for a new person | after |

Tasks, the inbox and "what to do next" come after the memory itself is in place.

## The tools

What Claude can ask the program. `where` exists today; the three others come with the reading step.

| Tool | It takes | It gives |
|---|---|---|
| `where` | nothing | the memory folder and how it was found (the plugin's setting, or the folder the session is opened on), and the plugin's version |
| `search` | a text | the memories and people whose title, tags or text hold it, capitals ignored, a match in a title first: number, title, the line that matched. Ten at most. |
| `read` | a number, or a person's name | every field of the piece's header, then its text, cut past 6000 characters |
| `links` | a number | the pieces it links to and the pieces that link to it, each with number and title; a link that leads to no piece is said as such |

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
cargo test --workspace
cargo build --release -p lorisse
```

The program is `target/release/lorisse` (or `lorisse.exe`); copy it to `plugin/server/` to try it. The program itself uses the Rust standard library only; the store brings five crates: chrono, serde, serde_json, serde_yaml_ng and thiserror.

## License

MIT, see `LICENSE`.
