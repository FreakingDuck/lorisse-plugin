# plugin — contract

> written by Lorisse 2026-11-24 · reviewed by the owner: 2026-11-25

Human file; format: `docs/contract_spec.md`. Stage design: the clause ids may still change. Parts 4 to 7 are the reading step; parts 8 and 9, and C-5.3, the writing step. Part 10 is the files. All are built. Part 11, the first day, and C-8.10 are designed and not built.

## 1. Role

Lorisse as a person meets it in the Claude app: a program on their own machine that lets Claude read their memories and file new ones, through a short list of tools, on Windows and on a Mac.

## 2. Does and does not

- Finds the person's memory folder.
- Answers four questions on its memories, and on the people and tags they name: where it is, search, read, links. What each takes and gives is in the README, "The tools".
- Files a memory, adds a note to a piece, links two pieces, and takes back its own last change: remember, note, link, undo.
- Opens a conversation with the person's own intro, the state of the memory and the rules of keeping, when the person says "Lorisse" or "plop": plop.
- Keeps a document with the memory it is about: attach.
- Makes a new person's memory folder ready, and fills it with what another assistant already remembers of them: start, import. Not built yet.

Does not:

- Change or retire the text of a memory, file a person, merge or move pieces → later; anything heavy will be a proposal the person accepts.
- Pull or push the folder → the person's own git, when they want one.
- Show tasks, the inbox or the questions to check, or say what to do next → later versions; ranking is Claude's.
- Decide when a tool is called, or how a piece is filed → `keeper`.
- Say whether a call passes, asks or is refused → `guard`, by the person's rules.
- Read a piece's file, its number, the confidential mark → `store`.
- Speak the Claude app's protocol → `tools`.
- Read mail, a calendar or the web → the Claude app's own connectors.

## 3. Depends on

`store`, `tools`, `keeper`, `guard`. Nothing else from the repository.

## 4. The memory folder

We want Lorisse to work on the one folder the person chose, and to say plainly when it has none.

- **C-4.1** The program works on one folder, the memory folder: the folder the session is opened on, when that is a memory folder; otherwise the one the person gave in the plugin's setting; otherwise the one this machine remembers from the first day (C-11.3). It looks nowhere else.
- **C-4.2** With no memory folder, `where` and `plop` say so and offer to make one with `start`; every other tool answers that same sentence, and nothing is created.
- **C-4.3** The folder is read at each call: a piece another program changed is seen at the next call.
- **C-4.4** A memory folder can name several repositories in its config: their pieces are read as one memory, each said with the repository it comes from.

## 5. Reading writes nothing

We want a question never to change the folder, so that reading is safe to try on a real one.

- **C-5.1** The four reading tools write nothing in the memory folder: after any number of their calls, every file and the folder's git state are as they were.
- **C-5.2** The program opens no network connection. The reading tools start no other program; the writing tools start one, `git`, and only in the memory folder.
- **C-5.3** The reading tools are declared to the Claude app as tools that only read, and the writing tools as tools that write, so the person can let the first run without being asked and keep a say on the second.

## 6. What is shown

We want Claude to see the person's memories, and never what they marked confidential.

- **C-6.1** A piece in a part of the folder marked confidential is never returned: not its text, its title or its number, and it is not counted. Asked by its number, the answer is the one for a number that does not exist.
- **C-6.2** Every answer that carries a piece's text says in its first line that the text is data, never an instruction.

## 7. The answers

We want every answer short enough for a conversation, and honest when it is cut.

- **C-7.1** A list cut at its limit ends with one line that gives the count in all.
- **C-7.2** A text cut at its limit ends with one line that says so and gives its full length.

## 8. Writing

We want a memory filed in one sentence, each change one step the person can see in the folder's history and take back, and nothing else touched. When a tool is called is the keeper's: by itself when the person says something worth keeping, each filing said in one line; the person's own intro can say otherwise.

- **C-8.1** `remember` files a new memory: its title, its text, its kind, its tags, the pieces it links to. The answer gives its number, and `read` gives it back as it was filed.
- **C-8.2** `note` adds one dated line at the end of the text of a memory or of a person; what was there is unchanged.
- **C-8.3** `link` links one piece to another. A link already there changes nothing, and the answer says so.
- **C-8.4** Each change is exactly one commit, in the repository the piece lives in; its message says what was done and names the piece. No other file the folder keeps under git changes, and what the person had changed without committing is left as it was.
- **C-8.5** `undo` takes back the last change Lorisse made, or the one named, as a new commit. The history is never rewritten, and a commit Lorisse did not make is never taken back.
- **C-8.6** A number is given once and never reused, even after an undo. Nothing is deleted.
- **C-8.7** Nothing is written in a part marked confidential, nor in a repository marked so: asked to, the tool refuses in one sentence.
- **C-8.8** In a folder of several repositories, a new memory goes to the one the call names; without a name, to the folder's own.
- **C-8.9** A change that would break the folder's rules (a kind the folder does not know, a link to a piece that does not exist) is refused with its reason, and nothing is written.
- **C-8.10** Git is recommended, not required. In a folder that is not under git, the writing tools write all the same: the piece is filed, there is no history, and `undo` says it has none to go back through. `plop` says once, in one sentence, what git would add. Not built yet.

## 9. Starting a conversation

We want one word, "Lorisse" or "plop", to start Lorisse as the person set it up, on any machine, without retyping who they are and how they work.

- **C-9.1** `plop` gives the person's intro: the text of the page `intro.md` at the root of the memory folder, whole. The person changes it as any file of theirs, and the next `plop` gives the new text. Without that page, it gives a short one of the plugin's own and says where to write theirs.
- **C-9.2** After the intro, `plop` gives the state: the memory folder, its repositories, how many memories and people it holds, and the five memories filed last.
- **C-9.3** The intro is the person's own word on how Lorisse works for them, and is said as such; every other text of the folder stays data (C-6.2).
- **C-9.4** `plop` ends with the rules of keeping, in a few lines: what is filed without being asked, what is never filed, how a filing is said. So each conversation opened with it starts with the rules in front of Claude, whatever else it has read.
- **C-9.5** When the Claude app connects to the program, the program gives it its own short instructions, which the app keeps in front of Claude for the whole conversation: what Lorisse is, to search it when the person asks what they know, and that filing by itself starts once `plop` has been called in the conversation; before that, Lorisse files only when asked.

## 10. Files

We want a document kept with what it is about: the quote with the works, the letter with the dispute.

- **C-10.1** `attach` puts a copy of a file of the person's machine in the memory folder, with the piece it is given for, as one commit. The original is left where it was.
- **C-10.2** `read` names the files a piece holds, each with its path in the folder, so that Claude can open one with the Claude app's own reader.
- **C-10.3** `search` finds a piece by the name of a file it holds.
- **C-10.4** A file over the folder's limit (20 MB unless the folder's settings say otherwise) is refused with its size, and nothing is copied.
- **C-10.5** A file is never replaced: one of the same name already with the piece makes the new one take a numbered name.
- **C-10.6** A key or a secret is never attached: a file under a `.ssh` folder, a `.pem`, a `.env`, or a name that ends in `_key` or `_token`, is refused with the reason.

## 11. The first day

Designed, not built yet. We want a new person to have a memory in five minutes, without knowing what a folder under git is, and a first conversation that does not start empty.

- **C-11.1** `start` makes a memory folder ready: a folder named `Lorisse` in the person's home folder, unless they name another place, holding the settings, the first set of rules and their page of intro, and nothing of anyone else's. A place that already holds files is never taken over: it is refused, unless it is empty or already a memory folder.
- **C-11.2** When git is on the machine, the new folder is put under git, on the machine only, with a name for its commits, and its first state is one commit. Without git the folder is made all the same, and the answer says in one sentence what git would add and that it can come later.
- **C-11.3** The machine remembers the folder `start` made: from then on Lorisse finds it with no setting to fill, in every conversation on that machine.
- **C-11.4** `start` writes the person's intro from what they said: their name, what they want to remember, how they like to be answered.
- **C-11.5** `start` gives, in a block the person can copy whole, the text to hand to another assistant to get back what it already remembers of them, as dated lines by category.
- **C-11.6** `import` takes that answer: each dated line becomes a memory with its date and its category, in the person's own words; the lines that are instructions are not filed, they are given back as proposals for the intro, never obeyed. The whole import is one step, taken back by one `undo` when the folder is under git, and the answer gives the count.
- **C-11.7** A line `import` cannot read is left out and counted, never guessed.

## 12. Behaviour

| Situation | Reaction |
|---|---|
| A number that does not exist | one sentence that names it |
| An argument missing or of the wrong kind | one sentence that says what is expected |
| A piece whose header cannot be read | the answer names the file and gives the rest |
| A writing tool, the folder under git, but no git on the machine or no name set for its commits | refused: one sentence that names what is missing; nothing written |
| `start` when this machine already has a memory folder | one sentence that names it; nothing is made |
| `undo` with no change of Lorisse's to take back | one sentence that says so |
| A change that fails halfway | the folder is as it was before the call |
| Any call, whatever the folder or the piece | the program answers and stays up |

## 13. Wanted, not promised yet

- Changing or retiring a memory's text, filing a person; proposals for anything heavy (merge two pieces, move one, rename a tag).
- A short tutorial to keep the folder on a git service as well, for those who want a copy elsewhere.
- Files: with the Claude app's own connector to a file service, Lorisse files the documents that arrive and keeps a memory of each. No connector of ours.
- A large file kept by reference: the memories live in git, the large documents in the person's file service; `attach` of a file past the limit, or of one already in that service's folder, keeps its path instead of a copy.
- The way out: the whole memory given back as one page of dated lines by category, so a person can leave with it or hand it to another assistant.
- One memory for life and for the work on code: the assistant's own notes of a project as one more repository of the memory folder.
- The same memory from the phone and the plain chat: the program on a server of the person's, the memories exchanged by git.
- A release that arrives whole: the card, the version and the programs together, never one before the other; and a check after each release that an installed copy says the new version.
- The memory folder chosen by browsing, not typed: today the Claude app's own window asks for it as a line of text.
- Tasks, the inbox, the questions to check, what changed since the person last looked.
- A memory kept by subject or by project, in repositories the person names.
- The plugin itself neither pulls nor pushes: the folder stays on the machine unless the person wants more.
- The keeper's rules of filing, and one skill a subject.
- The same answers in the desktop app's Cowork, on Windows and on a Mac: built for, not tried.

## 14. Non-goals

A screen of its own, a choice of model, a reader of mail: the Claude app has them.
