# lorisse-plugin: how to work here

Lorisse as a plugin of the Claude app. The repository is the workshop; the folder `plugin/` is what the Claude app installs, and nothing else ships.

## Read first

`status.yaml` (the table: one row per part, what comes next), then the three pages of the part you touch: README, CONTRACT, VALIDATION. The root's three pages are the plugin as a person meets it.

## The layout

| Where | What |
|---|---|
| `plugin/` | what the Claude app installs: the plugin's card, the line that starts the program, the skills, the built programs in `server/` |
| `.claude-plugin/marketplace.json` | the marketplace of one, pointing at `plugin/` |
| `crates/store/` | the one door to the memory folder |
| `crates/tools/` | the program the Claude app starts: the protocol and the tools |
| `sample/` | a small memory folder made for the checks and the walk; no person's data |
| `scripts/` | the gate (`verify.ps1`), the sample's copy for the walk (`walk_sample.ps1`), and the method's checks, copied whole from the method and never edited here |
| `docs/contract_spec.md` | how a part's three pages are written, copied from the method |
| `.github/workflows/build.yml` | builds the program for Windows and for a Mac and commits both to `plugin/server/` |

## The rules

- **The method**: this repository follows the method's revision r5. A part is a folder with README, CONTRACT, VALIDATION on top, written by `docs/contract_spec.md`; `status.yaml` is the table; the copies of the method's pieces are compared by the gate, and a copy that differs is ported from the method, never mended here.
- **The review line**: under every page's title, `> written by <who> <date> · reviewed by the owner: <not yet, or his date and his word>`. Only the owner writes his mark. His `// ` lines in a page are notes: answer under each with a `// Lorisse: ` line, never remove his.
- **The gate**: `powershell -NoProfile -ExecutionPolicy Bypass -File C:/software/lorisse-plugin/scripts/verify.ps1`, green before any commit that touches code.
- **The sample only.** No check, no test and no walk ever opens a person's own memory folder. The program never writes in a memory folder before the writing step's contract says how.
- **A release is a version raised.** While `plugin/.claude-plugin/plugin.json` names a version, an installed copy stays as it is until that string changes. Raise it at each release, and never set a version in the marketplace entry.
- **The built programs are GitHub's.** `plugin/server/` is written by the workflow from the source alone. A local build copied there is for a try, never committed.
- **A dependency is announced** to the owner by name and version before it enters `Cargo.toml`.
- **The pages are Lorisse's.** A builder writes code and tests; the contract, the validation and `status.yaml` are written before the pass and are not a builder's to change.
- **Nothing is pushed** without the owner's word. A pass works on `agent/<name>`, in a worktree under `.claude/worktrees/<name>` inside this repository, never in a folder beside it, and commits once when the gate is green. At the merge its worktree is removed and its branch deleted.
- **Scripts**: plain ASCII, a help block on top.
- No name of a person, no employer, no address in anything written here. "Another user", never "colleague".
