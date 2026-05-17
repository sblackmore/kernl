# Agent guide — writing kernl efficiently

Short, actionable notes for humans and coding agents working on **kernl** (`.knl` sources + `kernlc`). Read these **before** generating or refactoring substantial `.knl` programs.

## Read order

| Doc | Purpose |
|-----|---------|
| [01-language-model.md](01-language-model.md) | Strict vs fluid mode, what runs where |
| [02-parser-syntax.md](02-parser-syntax.md) | `fn` layout, `do`, `match`, `if`, comments |
| [03-pipes-and-calls.md](03-pipes-and-calls.md) | Pipe associativity, unary builtins, grouping limits |
| [04-builtins-runtime.md](04-builtins-runtime.md) | What exists in stdlib vs executor |
| [05-idioms.md](05-idioms.md) | Multi-step `do`, strings, lists, recursion |
| [06-pitfalls-checklist.md](06-pitfalls-checklist.md) | Copy-paste checklist before declaring “done” |
| [07-kernlc-cli.md](07-kernlc-cli.md) | `kernlc --run`, `--invoke-stdin`, testing |

## Repo docs outside this folder

- [../getting-started.md](../getting-started.md) — human-oriented intro
- [../examples.md](../examples.md) — example index
- [../architecture.md](../architecture.md) — compiler pipeline

## Cursor / rules

Optional: point `.cursor/rules` or project `AGENTS.md` at this directory so retrieval prefers these files when editing `.knl` or `kernlc`.
