# Agents working on kernl

## `.knl` source files

Before changing or adding kernl programs, read **`docs/agents/README.md`** and follow its index (parser quirks, pipes, builtins vs executor, pitfalls, CLI).

- Normative spec: **`spec/LANGUAGE.md`**
- Human tutorial: **`docs/getting-started.md`** (links to agent docs in “Next steps”)

## Compiler / runtime built-ins

If you change **`compiler/src/runtime/executor.rs`** or **`compiler/src/stdlib/mod.rs`**, update **`docs/agents/04-builtins-runtime.md`** in the same change when behavior or the supported builtin set changes.
