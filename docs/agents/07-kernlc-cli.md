# `kernlc` / `kernl` CLI (agents)

## Script runner (`kernl`)

After `cargo build --bins`, use the **`kernl`** binary for minimal friction:

```bash
printf '...\n' | ./kernl program.knl
```

- Implies **`--run`** (no need to pass it).
- If **`main`** has exactly one **`str`** parameter and stdin is **piped** (not a TTY), stdin is bound automatically (same as **`kernlc --invoke-stdin --run`**). Use **`kernl … --no-stdin`** to force an empty string.

Full compiler / codegen paths stay on **`kernlc`**, or **`kernl program.knl --compile …`**.

## Common `kernlc` invocation

```bash
cargo run --manifest-path /path/to/kernl/compiler/Cargo.toml --bin kernlc -- program.knl --run
```

Release binaries (after `cargo build --release`):

```bash
./kernl program.knl           # script mode
./kernlc program.knl --run    # explicit
```

## Flags relevant to agents

| Flag | Meaning |
|------|---------|
| **`kernl` binary** | Implicit **`--run`**; optional auto-stdin binding when piping (see above). |
| **`--compile`** | With **`kernl`**: disable implicit run; behave like **`kernlc`** for emit targets. |
| **`--run`** | Interpret with **`executor`** (strict mode path). |
| **`--invoke-stdin`** | With **`--run`**: read **all stdin** into **`main`**’s **single `str`** parameter (must be exactly one **`str`** param). Redundant when piping under **`kernl`** unless **`--no-stdin`**. |
| **`--resolver-endpoint`**, **`--resolver-model`** | Fluid HTTP resolver (OpenAI-compatible chat). |
| **`--verify`** | SMT / verification path — separate workflow. |
| Default (**`kernlc`**, no codegen flags) | AST/debug output — see **`kernlc --help`**. |

## Entrypoint

- **`main`** is preferred if present; else first function — see **`run_program`** in `compiler/src/cli.rs`.

## Debugging failures

1. **Parse / type errors** — compiler stdout/stderr.
2. **`runtime error: undefined variable`** — usually pipe/`match`/mis-call issues (see agent docs).
3. **`kernlc` exited non-zero** — host should capture **stderr** from subprocess (Lambda bootstrap pattern).

## Cross-compile (Lambda)

- Example: **`examples/cloud/aws/order-api-hello-lambda/build-lambda.sh`** uses **`cargo zigbuild`** for **`x86_64-unknown-linux-gnu`**.
