# guard — contract

> written by Lorisse 2026-11-25 · reviewed by the owner: not yet

Human file; format: `docs/contract_spec.md`. Stage design: nothing here is built, and the clause ids may still change.

## 1. Role

The plugin's guard: before a tool call runs, it says whether the call passes, asks or is refused, by the person's own rules, in Claude Code and in Cowork, on Windows and on a Mac.

## 2. Does and does not

- Reads the person's rules in their memory folder.
- Answers pass, ask or never for a call of one of Lorisse's tools, or of a tool of one of the Claude app's connectors (a file service, the mail, a calendar).
- Gives a first set of rules to a folder that has none.

Does not:

- Run the call, or show the prompt → the Claude app.
- Change the rules → the person, in their file.
- Lift a refusal the person or an organisation set in the Claude app's own settings → the Claude app; that refusal always wins.
- Decide what is filed, or when → `keeper`.

## 3. Depends on

`tools` (the program that carries it, and the memory folder it found). Nothing else from the repository.

## 4. The rules

We want the person to say once what runs without asking, in a file of theirs that follows their memory to every machine.

- **C-4.1** The rules are one file, `rules.yaml`, at the root of the memory folder: three lists, pass, ask and never, each naming tools under the service they belong to. It is read at each call: a change holds from the next call on.
- **C-4.2** A folder without that file is ruled by the plugin's first set: Lorisse's own tools pass, and every other call is left to the Claude app. The first set is a file of the plugin the person can copy into their folder and change.
- **C-4.3** A tool named in two lists takes the strictest: never, then ask, then pass.
- **C-4.4** A call no rule names is left to the Claude app, as if there were no guard.
- **C-4.5** A tool of Lorisse's own that the person's file does not name is ruled by the first set: a tool a new version brings passes without the person having to add it to a file written before. A tool of Lorisse's the person's file names, under any list, follows the person's file.

## 5. The answer

We want fewer prompts and no surprise: what passes alone is only what the person wrote down.

- **C-5.1** Pass: the call runs without a prompt. Ask: the Claude app asks the person. Never: the call is refused, and the refusal names the rule.
- **C-5.2** Nothing of another service passes that the rules do not name as pass; of Lorisse's own tools, only what the first set lets pass (C-4.5). A rules file that cannot be read makes every call ask, and the reason says the file cannot be read.
- **C-5.3** A change of the rules file itself by one of the app's file tools always asks, whatever the rules say.
- **C-5.4** The guard writes nothing in the memory folder and opens no network connection.

## 6. Behaviour

| Situation | Reaction |
|---|---|
| No memory folder | the first set holds |
| A rule that names a tool or a service nobody has | ignored; the other rules hold |
| The rules file cannot be read | every call asks, with the reason |
| Any call, whatever its shape | the guard answers; a guard that fails leaves the call to the Claude app |

## 7. Wanted, not promised yet

- The rule asked once, in the conversation: the first time the person asks for something a service nobody ruled yet would do (read the mail, list the files), Lorisse asks in plain words how they want it from now on (ask each time, pass, never) and what it may do there, writes it in the rules file with a tool of its own, and from then on asks only for what goes past it. No service passes by default.
- What asked is kept, and Lorisse proposes the rule that would have let it pass, as the dev tool does.
- Rules by repository or by subject.
- A change of the rules file by a shell command is not seen today: only the app's file tools are.

## 8. Non-goals

Replacing the Claude app's own settings; deciding for an organisation.
