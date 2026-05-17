# `kernlc` CLI (agents)

## Common invocation

```bash
cargo run --manifest-path /path/to/kernl/compiler/Cargo.toml -- program.knl --run
```

Release binary (after `cargo build --release`):

```bash
./kernlc program.knl --run
```

## Flags relevant to agents

| Flag | Meaning |
|------|---------|
| **`--run`** | Interpret with **`executor`** (strict mode path). |
| **`--invoke-stdin`** | With **`--run`**: read **all stdin** into **`main`**’s **single `str`** parameter (must be exactly one **`str`** param). |
| **`--resolver-endpoint`**, **`--resolver-model`** | Fluid HTTP resolver (OpenAI-compatible chat). |
| **`--verify`** | SMT / verification path — separate workflow. |
| Default (no codegen flags) | Often AST/debug output depending on driver — check **`main.rs`** help. |

## Entrypoint

- **`main`** is preferred if present; else first function — see **`run_program`** in `compiler/src/main.rs`.

## Debugging failures

1. **Parse / type errors** — compiler stdout/stderr.
2. **`runtime error: undefined variable`** — usually pipe/`match`/mis-call issues (see agent docs).
3. **`kernlc` exited non-zero** — host should capture **stderr** from subprocess (Lambda bootstrap pattern).

## Cross-compile (Lambda)

- Example: **`examples/cloud/aws/order-api-hello-lambda/build-lambda.sh`** uses **`cargo zigbuild`** for **`x86_64-unknown-linux-gnu`**.
