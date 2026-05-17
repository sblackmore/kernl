# kernl

### The AI-native programming language: designed for LLMs, not humans.

[Website](https://kernl-lang.org/) | [Getting Started](docs/getting-started.md) | [Language Spec](spec/LANGUAGE.md) | [Examples](docs/examples.md) | [Architecture](docs/architecture.md) | [Contributing](CONTRIBUTING.md)

---

This is the main source code repository for kernl. It contains the compiler (`kernlc`), the language specification, benchmark suite, and documentation.

## Why kernl?

Every programming language in existence was designed for humans. Python optimizes for readability. Rust optimizes for safety. C optimizes for hardware control. **None of them optimize for LLM generation** — and now that's the bottleneck.

When an AI agent writes code, it generates tokens. Every unnecessary token is:
- **Cost** — you pay per token
- **Latency** — more tokens = slower generation
- **Error surface** — every brace, semicolon, and scope boundary is an opportunity to hallucinate

kernl eliminates the noise.

- **Token efficiency** — 40% fewer tokens than Rust, 25% fewer than Python on equivalent programs. Every token carries semantic weight.
- **Flat structure** — no deep nesting. LLMs degrade on long-range bracket matching; kernl uses keyword-delimited blocks and pipe composition instead.
- **Verification-native** — invariants and contracts are first-class language constructs, not annotations bolted on. The compiler checks them before emitting code.
- **Intent-first** — declare *what* you want, the compiler resolves *how*. Fluid mode lets functions express intent in natural language with confidence thresholds.

## Quick start

**Requirements:** [Rust toolchain](https://rustup.rs/) (1.80+)

```bash
git clone https://github.com/kernl-lang/kernl.git
cd kernl/compiler
cargo build
```

Write your first program (`hello.knl`):

```
fn add_one
  in x: int
  out result: int
  do add x 1
```

Compile it:

```bash
cargo run -- hello.knl --target llvm      # emit LLVM IR
cargo run -- hello.knl --target wasm      # emit WebAssembly Text
cargo run -- hello.knl --target wasm-bin  # emit binary .wasm file
cargo run -- hello.knl --target debug     # dump AST
```

See the full [Getting Started guide](docs/getting-started.md) for a walkthrough of the language.

## The language

kernl programs are flat, declarative, and independently verifiable at the line level.

```
fn sum_positive
  in  nums: [int]
  out result: int
  inv gte result 0
  do  filter nums gt 0 | reduce add
```

**`fn`** declares a function. **`in`** / **`out`** define typed parameters and return values. **`inv`** declares invariants that must hold. **`do`** is the implementation. The **pipe `|`** composes operations left to right.

Operators are **named, not symbolic** — `add` instead of `+`, `gte` instead of `>=`. This eliminates an entire class of LLM generation errors caused by confusing `>=`, `=>`, `->`, and `>>`.

### Two modes

**Strict** (default) — spec is fully resolved before execution. Every invariant is checked statically. Deterministic output. For systems, protocols, financial logic, safety-critical code.

**Fluid** — spec is partially resolved, execution fills in the rest. For agents, recommendations, context-sensitive behavior.

```
fn recommend
  mode fluid
  in  user: User context: Context
  intent "surface items user would engage with"
  confidence 0.85
  fallback popular_items context
```

Same syntax. Same toolchain. Same compiler. Different verification guarantees. See the [Language Specification](spec/LANGUAGE.md) for the full grammar.

## Benchmarks

kernl programs use fewer tokens than equivalent Python or Rust — meaning LLMs generate them faster, cheaper, and with fewer hallucination opportunities.

| Program | kernl | Python | Rust | kernl vs Rust |
|---------|------:|-------:|-----:|--------------:|
| add_one | 14 | 15 | 15 | -7% |
| clamp | 30 | 44 | 54 | **-44%** |
| fibonacci | 31 | 38 | 44 | **-30%** |
| sum_positive | 24 | 32 | 52 | **-54%** |
| transfer | 40 | 56 | 68 | **-41%** |
| **Total** | **139** | **185** | **233** | **-40%** |

*Token counts measured by whitespace/punctuation splitting. The gap widens on programs with invariants, structs, and composition — exactly the patterns that matter most.*

<details>
<summary><strong>Side-by-side: clamp</strong></summary>

**kernl** (30 tokens)
```
fn clamp
  in val: int lo: int hi: int
  out result: int
  inv gte result lo
  inv lte result hi
  do max lo min hi val
```

**Python** (44 tokens)
```python
def clamp(val: int, lo: int, hi: int) -> int:
    result = max(lo, min(hi, val))
    assert result >= lo
    assert result <= hi
    return result
```

**Rust** (54 tokens)
```rust
fn clamp(val: i64, lo: i64, hi: i64) -> i64 {
    let result = lo.max(hi.min(val));
    assert!(result >= lo);
    assert!(result <= hi);
    result
}
```

Every brace, semicolon, `assert!()` macro, `let` binding, and `->` return arrow is a token the LLM has to generate correctly. kernl eliminates them.

</details>

Run the benchmark yourself:

```bash
cd benchmark
cargo run
```

## Compilation targets

```bash
kernlc file.knl --target debug      # dump parsed AST
kernlc file.knl --target llvm       # emit LLVM IR (.ll)
kernlc file.knl --target wasm       # emit WebAssembly Text (.wat)
kernlc file.knl --target wasm-bin   # emit binary WebAssembly (.wasm)
kernlc file.knl --target native     # compile to native binary (requires llc or clang)
kernlc file.knl --target native --cross aarch64-unknown-linux-gnu  # cross-compile
kernlc file.knl --verify            # formally verify invariants + contracts via Z3
kernlc file.knl --run               # interpret directly (with live LLM resolver for fluid mode)
kernlc file.knl --debug-info --target llvm  # emit LLVM IR with DWARF metadata
kernlc --repl                       # interactive REPL
kernlc --list-targets               # show cross-compilation targets
```

The `native` target runs the full pipeline: kernl → LLVM IR → object code → linked binary. It links against `libkernl_rt.a` for stdlib functions. `--cross` enables cross-compilation to ARM, RISC-V, and more. The `--verify` flag checks invariants and contracts with an SMT solver. `--run` interprets the program directly, including fluid mode LLM resolution.

## Project structure

```
kernl/
├── spec/              language specification
│   └── LANGUAGE.md    full grammar, types, operators, modes
├── compiler/          the kernl compiler, written in Rust
│   └── src/
│       ├── lexer/     tokenizer
│       ├── parser/    recursive descent parser + AST
│       ├── stdlib/    built-in function definitions
│       ├── semantic/  scope resolution + semantic analysis
│       ├── typeck/    Hindley-Milner type inference
│       ├── verify/    spec verification (strict + fluid)
│       ├── codegen/   LLVM IR / WASM / native backends + optimizer
│       ├── smt/       formal verification via SMT solver + contracts
│       ├── driver/    native compilation driver + cross-compilation
│       ├── runtime/   fluid mode runtime, LLM executor
│       ├── repl/      interactive REPL
│       ├── incremental/ file-level compilation cache
│       └── modules/   module resolution + dependency graph
├── runtime/           C runtime library (libkernl_rt.a)
├── pkg/               package manager (`kernl` CLI)
├── lsp/               language server (LSP)
├── resolver/          fluid mode resolver daemon
├── registry-server/   package registry HTTP server
├── editors/vscode/    VS Code extension
├── self-host/         kernl programs implementing parts of the compiler
├── benchmark/         LLM token benchmark suite vs Python/Rust
├── examples/          .knl samples; see examples/cloud for AWS/GCP/Azure deploy examples
└── docs/              guides and documentation
```

## Building from source

See [INSTALL.md](INSTALL.md) for detailed instructions.

```bash
cd compiler
cargo build --release    # optimized build
cargo test               # compiler tests (see also pkg/, registry-server/, lsp/)
```

## Getting help

- **Language guide:** [docs/getting-started.md](docs/getting-started.md)
- **Debugging (GDB / LLDB / `--debug`):** [docs/debugging.md](docs/debugging.md)
- **Full specification:** [spec/LANGUAGE.md](spec/LANGUAGE.md)
- **Annotated examples:** [docs/examples.md](docs/examples.md)
- **Compiler architecture:** [docs/architecture.md](docs/architecture.md)
- **Issues:** [github.com/kernl-lang/kernl/issues](https://github.com/kernl-lang/kernl/issues)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to get involved.

Whether you're interested in language design, compiler engineering, formal verification, or benchmarking — there's meaningful work to do. This is early-stage and everything is open.

## Status

Active development. The compiler is a complete toolchain with native compilation, cross-compilation, formal verification, an interpreter, and full editor support.

**What works today:**
- Full lexer and recursive descent parser for the kernl grammar, including **algebraic data types** (`enum` … `end`), **`match` / patterns**, **`req` / `ens` contracts**, and **async-shaped** keywords (`mode async`, `spawn`, `await`, `send`, `recv`)
- **Standard library** — builtins with native C runtime (`runtime/libkernl_rt.a`, now includes **profiling stubs** `kernl_profile.c` for `--instrument-llvm`)
- **Semantic analysis** — scope resolution, undefined variable detection, duplicate binding checks, shadowing warnings
- **Hindley-Milner type inference** — unification, pipes, `match` exhaustiveness on enums
- **Constant folding & DCE**
- **LLVM IR emission** including `match` / enum lowering; **DWARF metadata** (`--debug-info`); **LLVM opt** pipelines (`--opt-passes`, `-O0`–`O3`)
- **Native & cross-compilation** via `--target native` and `--cross <triple>`
- **WebAssembly** text and binary emission
- **Formal verification** — SMT (`--verify`) and **Lean / Coq skeleton export** (`--export-lean`, `--export-coq`), including generated **inductive types** from `enum` definitions
- **Interpreter** — `--run` with optional **`--profile`** and interactive **`--debug`** (source-oriented breakpoints)
- **LLVM IR instrumentation** — **`--instrument-llvm`** inserts `__kernl_profile_*` calls (link `libkernl_rt.a`)
- **REPL**, **incremental cache**, **modules** (`mod` / `use`), **package manager** CLI, **registry server** with **Docker / Compose** ([registry-server/HOSTING.md](registry-server/HOSTING.md)), **resolver daemon**, **LSP**, **VS Code** extension
- **Self-hosting examples** — tokenizer, **parser**, **typechecker**, **optimizer**, evaluator, formatter ([self-host/README.md](self-host/README.md))
- **244 tests** in the compiler crate; additional tests in pkg, registry-server, resolver, LSP, benchmark

**What's next:**
- Deeper async runtime (scheduling, real channels, cancellation)
- Richer pattern matching (nested patterns, guards)
- Source-level stepping mapped cleanly to DWARF line tables across all codegen paths
- Hardened registry (HTTPS-first client defaults, OAuth/API keys, CDN for tarballs)
- Expand self-hosted phases until they compile the full language
- Richer proof exports (definitions for `match`, lemmas linking `ens` to extracted code)

## License

kernl is distributed under the terms of the MIT license. See [LICENSE](LICENSE) for details.
