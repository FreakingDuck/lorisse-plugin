# guard — what passes alone, what asks, what never happens

> written by Lorisse 2026-11-25 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`.

The crate is built: given what the Claude app sends before a tool call, and the memory folder, it gives the answer to print. It is not wired into the plugin yet: the program's guard mode and `plugin/hooks/hooks.json` come next.

## The words

- **call**: Claude asking to use one tool, once.
- **service**: who the tool belongs to: `lorisse`, or a connector of the Claude app (`dropbox`, `gmail`).
- **rule**: one tool of one service, in one of three lists: pass, ask, never.
- **first set**: the rules the plugin ships, used when the folder has none.

## How it is made

The Claude app can start a program before each tool call and take its answer: a hook. The plugin ships one hook, in `plugin/hooks/hooks.json`, that starts the same program as the tools, in its guard mode. The program finds the memory folder as the tools do, reads `rules.yaml`, and answers.

A service is matched by its name as a whole word inside the tool's full name, capitals ignored: `dropbox` matches the tools of the Claude app's Dropbox connector, `lorisse` those of this plugin; `mail` does not match `gmail`.

A rules file that cannot be read (a broken layout, a list other than pass, ask and never) makes every call of a connector or of Lorisse ask, with the reason. An empty file is no rule at all.

## The rules file, as it reads

```yaml
pass:
  lorisse: [where, open, search, read, links, remember, note, link, undo]
  dropbox: [search, list_folder, fetch, file_preview, get_file_metadata]
ask:
  dropbox: [move, copy, create_folder, create_file]
never:
  dropbox: [delete, create_shared_link]
  gmail:   [send_message, reply, forward]
```

## Chosen, not promised

- One program for the tools and the guard: one file to build and to ship for each machine.
- The rules live in the memory folder, not in a machine's settings: they follow the folder.
- Anthropic's pages say a plugin's hook is loaded in Claude Code and in Cowork, and that its "pass" skips the prompt. They do not say whether Cowork obeys it: the walk tries it.
