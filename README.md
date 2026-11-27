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
| the cards | `plugin/.claude-plugin/`, `plugin/.mcp.json`, `plugin/hooks/` | the plugin's name and version, the line that says how to start the program, and the line that starts it as the guard before a tool call |

The rest is the workshop:

| Where | What |
|---|---|
| `crates/tools/` | the program's source: the Claude app's protocol and the tools |
| `crates/store/` | the one door to a memory folder: how a piece is read, numbered and written |
| `crates/guard/` | the guard: the person's rules read before each tool call |
| `sample/` | a small memory folder, every word invented, that the checks and the walk run on |
| `status.yaml` | the table: one row per part, where it stands, what comes next |
| `scripts/verify.ps1` | the gate: every check of the repository in one command |
| `.github/workflows/build.yml` | builds the program for Windows and for a Mac and puts both in `plugin/server/` |

## Where it stands

Lorisse reads a memory folder, files in it, and opens a conversation with your own intro. Your rules say which tool calls run without asking.

| Step | What the program can do | State |
|---|---|---|
| 1. the way in | one tool, `where`: the Claude app accepts the plugin and Claude calls the program by itself | done, version 0.0.1 |
| 2. reading | open your memory folder: search your memories, read one, follow its links | done, version 0.1.0 |
| 3. writing | file a memory, a note, a link; each change is one commit in the folder's history, and can be taken back. `plop` starts a conversation with your intro. | built, version 0.2.0 |
| 4. the guard | your rules, in a file of your folder: what passes alone, what asks, what never happens, for Lorisse's tools and for the Claude app's connectors | built, version 0.2.0 |
| 5. the files | a document kept with the memory it is about; reading a memory names its files, a search finds a memory by a file's name. The program gives Claude its rules when it connects, and `plop` repeats them. | built, version 0.3.0 |
| 6. the first day | a new person's folder made ready in one conversation, remembered by the machine, and filled with what another assistant already remembers of them; git recommended, no longer required | built, version 0.4.0 |
| 7. a first clean version | filing by itself said the same in every text Claude reads, starting with what the conversation already holds; a confidential repository you open to Claude, read and written; nothing left uncommitted; a plain sentence when the setting names the wrong folder | built, version 0.5.0 |
| 8. keeping it true | a memory corrected, one that no longer holds put aside and never deleted, and the journal, a page a day | built, version 0.6.0 |
| 9. the drawers | Lorisse sorts by itself: a repository made for a subject that has none, a piece moved when it sits in the wrong one, and a first day where you talk about yourself and it makes the drawers | built, version 0.7.0 |
| 7. the keeper | the skill that holds the rules of filing: what is filed at once, what stays a proposal, what is never done | after |

Tasks, the inbox and "what to do next" come after the memory itself is in place.

Your folder stays on your machine. Keeping it on GitHub as well is your choice, never required: the program neither pulls nor pushes.

## Your memory folder

Lorisse reads one folder. Where you are wins: the folder the session is opened on, when it is a memory folder (it holds a `config.yaml` at its root). Anywhere else, the one you gave in the plugin's setting, "Memory folder"; and without a setting, the one this machine remembers from the first day (one line in `.lorisse-folder`, in your home folder). It looks nowhere else. Each machine keeps its own. A memory folder can name several repositories in its config; they read as one memory.

The folder to give is the one that holds `config.yaml`, never one of its repositories. To change it later: in the Claude app, Customize, Plugins, Lorisse. A conversation already open keeps the old setting; a new one reads the new.

A repository can be marked confidential in the config (`confidential: true`): Claude then sees nothing of it. To keep it apart and still work in it with Claude, name Anthropic among the providers its row allows:

```
repos:
  - { name: home, folder: . }
  - { name: office, folder: office, confidential: true, providers: [anthropic], prefix: O }
```

Its memories are then read and written as the others, numbered on their own (`O-0001`), and never filed in another repository.

## The tools

What Claude can ask the program. Memories, people and tags are read; a task or a piece still in the inbox is not shown yet, and what you marked confidential never is, unless it is a repository you opened to Claude (above).

| Tool | It takes | It gives |
|---|---|---|
| `where` | nothing | the memory folder and how it was found (the plugin's setting, or the folder the session is opened on), and the plugin's version |
| `search` | a text | the memories, people and tags whose title, tags or text hold it, capitals ignored, a match in a title first: number, title, the line that matched. Ten at most. |
| `read` | a number, a person's name or a tag's name | every field of the piece's header, then its text, cut past 6000 characters |
| `links` | a number | the pieces it links to and the pieces that link to it, each with number and title; a link that leads to no piece is said as such |
| `plop` | nothing | your intro (the page `intro.md` at the root of your folder), then the state: the folder, its repositories, how many memories and people, the five filed last |
| `remember` | a title and a text; a kind, tags, links and a repository if wanted | a new memory, filed as one commit; its number |
| `note` | a number or a person's name, and a text | one dated line added at the end of the piece, as one commit |
| `link` | two pieces | the first linked to the second, as one commit; a link already there changes nothing |
| `attach` | a memory or a person, and the path of a file on your machine | a copy of the file kept beside the piece (`memories/M-0001/invoice.pdf`), as one commit; the original is left where it was. 20 MB at most unless your folder's settings say otherwise (`attach_limit_mb`); never a key or a secret |
| `undo` | nothing, or the change to take back | Lorisse's last change taken back as a new commit; never a change made by someone else |
| `change` | a memory or a person, and what to replace: its text, its title, its kind, its tags | the piece corrected, as one commit; what is not named stays; `undo` brings back what it said before |
| `retire` | a memory, a reason, and the memory that takes its place if there is one | the memory marked retired with its reason, as one commit; nothing is deleted: it is still read, and comes last in a search |
| `drawer` | a name and one sentence on what it is for; `apart` if it must never mix with the rest | a new repository of your memory folder, which Lorisse calls a drawer: its folder, its own numbers (`G-0001`), its line in `config.yaml` with every line of yours left as it was, as one commit. Lorisse makes one by itself when a subject keeps coming and has no place |
| `move` | one memory or several, and the repository to put them in | each moved with its number, its text, its links and its files, as one commit; nothing is ever taken out of a confidential repository |
| `journal` | a text; a day and a repository if wanted | the time and the text added at the end of that day's page (`journal/2026-11-27.md`), today unless a day is named, as one commit. `read` takes a day and gives its page |

| `start` | nothing, or a place, your name, what you want to remember, how you like to be answered | your memory folder made ready (`Lorisse` in your home folder unless you name a place), under git when git is on the machine, with your page of intro; this machine remembers it. It ends with a text to copy for another assistant, to get back what it remembers of you |
| `import` | that assistant's answer: dated lines under Instructions, Identity, Career, Projects, Preferences | each line filed as a memory with its date, in one step; the instructions given back as proposals for your intro, never followed; the count of what was filed and left out |

Git is recommended, not required. With it, each change of Lorisse's is one commit signed `lorisse@local`, so you can tell them from yours, and any one can be taken back. Without it everything is filed all the same, with no history and no undo.

## Your rules

A file of yours, `rules.yaml` at the root of the memory folder, says which tool calls pass without asking, which ask, and which never happen, for Lorisse's tools and for the Claude app's connectors. Without it, Lorisse's own tools pass and everything else stays as the Claude app does it. How it reads: `crates/guard/README.md`.

## How the memory is kept

A folder you choose, under git: one file for one memory, one file for one task, one file for one person. Plain text you can open and read without Lorisse. Nothing is ever deleted; a piece that no longer holds is retired, and stays readable.

Claude never writes that folder free-hand. It asks the program, and the program checks the rules before it writes.

## Install

The steps, for Windows and for a Mac: [lorisse.ai/install](https://lorisse.ai/install/).

In short, in Claude Code:

```
/plugin marketplace add whysoserious-wtf/lorisse-plugin
/plugin install lorisse@lorisse
```

Then open a new conversation and type "plop". With no memory yet, Lorisse offers to make one; with one, it answers with your folder, its repositories and the memories filed last.

To update, in a terminal:

```
claude plugin marketplace update lorisse
claude plugin update lorisse@lorisse
```

Then close the app and open it again: while it runs, it holds the old program.

The program runs on your own machine. Cowork and Claude Code start it; a plain chat does not. It opens no network connection.

## Build it yourself

```
cargo test --workspace
cargo build --release -p lorisse
```

The program is `target/release/lorisse` (or `lorisse.exe`); copy it to `plugin/server/` to try it. The program's own code uses the Rust standard library only; the store it reads through brings five crates: chrono, serde, serde_json, serde_yaml_ng and thiserror.

## License

MIT, see `LICENSE`.
