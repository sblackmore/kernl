# Language model (what kernl programs are)

## Strict mode (default)

- **`fn name`** with **`in` parameters**, optional **`out`**, **`do` expression**.
- Evaluated by the **Rust interpreter** in `kernlc --run` (`compiler/src/runtime/executor.rs`).
- No package imports, no FFI, no arbitrary HTTP from `.knl` except via **fluid** resolver (see below).
- Control flow: **`if` / `elif` / `else` / `end`**, **`match` / `end`**, **`each x in xs … end`**, **`while … end`**, **`let` / `mut let`** (see syntax doc).

## Fluid mode

```text
fn foo
  mode fluid
  in …
  intent "natural language intent"
  confidence 0.85
  fallback …
```

- Calls go through a **resolver** (stub or HTTP/OpenAI-compatible chat). Not general-purpose “call any API from kernl”.
- Use when you intentionally want LLM-backed behavior; use **strict** for deterministic logic (CLI tools, Lambda handlers, transforms).

## Embedded / codegen targets

- `kernlc` can emit **LLVM**, **WASM**, etc.; strict interpreter behavior is the reference for “does this `.knl` make sense?” unless you are targeting codegen specifically.

## Practical split for production-shaped demos

- **kernl**: routing, parsing simple protocols, business rules, formatting output.
- **Host** (Rust, Node, …): HTTP adapters, AWS SDK, persistence, secrets.
