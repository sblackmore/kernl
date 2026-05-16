# Contributing to kernl

Thank you for your interest in contributing to kernl. This is an early-stage project and contributions of all kinds are welcome — code, language design ideas, documentation, benchmarks, and thoughtful criticism.

## Getting started

### Prerequisites

- [Rust](https://rustup.rs/) 1.80 or later
- Git

### Setup

```bash
git clone https://github.com/kernl-lang/kernl.git
cd kernl/compiler
cargo build
cargo test
```

All 27 tests should pass. If they don't, please open an issue.

### Project layout

```
kernl/
├── spec/              language specification (the source of truth)
├── compiler/          the kernl compiler (Rust)
│   └── src/
│       ├── lexer/     tokenizer: source → tokens
│       ├── parser/    parser: tokens → AST
│       ├── typeck/    type checker: AST → typed AST
│       ├── verify/    verification: check invariants
│       └── codegen/   code generation: AST → LLVM IR / WASM
├── benchmark/         token benchmark suite
├── examples/          .knl example programs
├── docs/              documentation
└── spec/              language specification
```

## How to contribute

### Reporting bugs

Open an issue with:
1. The `.knl` source that triggers the bug
2. The command you ran (`kernlc file.knl --target llvm`)
3. What you expected
4. What actually happened

### Suggesting language changes

Language design changes should start as a discussion in Issues. Include:
1. The problem you're trying to solve
2. Your proposed syntax or semantics
3. How it affects token efficiency (the primary design constraint)
4. How it affects LLM generation accuracy

### Submitting code

1. Fork the repository
2. Create a feature branch (`git checkout -b my-feature`)
3. Make your changes
4. Add tests for new functionality
5. Run `cargo test` and `cargo clippy` — all tests must pass
6. Commit with a clear message explaining *why*, not just *what*
7. Open a pull request against `main`

### What to work on

Areas where contributions are especially impactful:

| Area | Difficulty | Description |
|------|-----------|-------------|
| **Standard library** | Medium | Implement builtins: `filter`, `reduce`, `map`, `max`, `min` |
| **LLVM backend** | Hard | Improve LLVM IR emission with proper optimization passes |
| **WASM runtime** | Hard | Binary WASM emission and runtime linking |
| **Fluid mode runtime** | Hard | Runtime resolver for fluid functions (LLM integration) |
| **Type inference** | Medium | Extend the type checker with full Hindley-Milner inference |
| **Error messages** | Easy | Improve diagnostic output from all compiler phases |
| **Benchmarks** | Easy | Add more benchmark programs comparing kernl to other languages |
| **Documentation** | Easy | Improve guides, add tutorials, fix typos |
| **Formal verification** | Research | Explore proving invariants at compile time |

## Compiler architecture

The compiler is a single-pass pipeline:

```
Source (.knl)
    │
    ▼
┌─────────┐
│  Lexer   │  source → tokens
└────┬─────┘
     │
     ▼
┌─────────┐
│ Parser   │  tokens → AST
└────┬─────┘
     │
     ▼
┌─────────┐
│ TypeChk  │  AST → type-checked AST
└────┬─────┘
     │
     ▼
┌─────────┐
│ Verify   │  check invariants + mode constraints
└────┬─────┘
     │
     ▼
┌─────────┐
│ Codegen  │  AST → LLVM IR / WASM / Debug
└─────────┘
```

### Lexer (`src/lexer/`)

Converts source text into a flat stream of tokens. Keywords, named operators, punctuation, literals, and identifiers are all distinct token types. Newlines are significant tokens (they delimit clauses).

### Parser (`src/parser/`)

Recursive descent parser. Key design decision: **operators have fixed arity** (binary ops consume exactly 2 atoms, unary ops consume 1). Function calls are greedy (consume all remaining args until newline or pipe). This resolves the ambiguity inherent in prefix notation without delimiters.

### Type checker (`src/typeck/`)

Builds a type environment from struct definitions and function signatures, then checks:
- All referenced types exist
- Invariant expressions resolve to `bool`
- Body return type matches declared return type
- Field access is valid on struct types

### Verifier (`src/verify/`)

Checks mode-specific constraints:
- Fluid functions must have an `intent` clause
- Future: static invariant proofs for strict mode

### Code generation (`src/codegen/`)

Three targets:
- **Debug** — pretty-prints the AST
- **LLVM IR** — emits textual LLVM IR with proper type mapping, operator lowering, and extern declarations for unresolved calls
- **WASM** — emits WebAssembly Text (WAT) with stack-based instruction emission

## Code style

- Follow standard Rust conventions (`rustfmt`, `clippy`)
- Keep functions short and focused
- Tests go in `#[cfg(test)] mod tests` blocks within each module
- Error types implement `Display` and `Error`
- No unnecessary dependencies — the compiler should build with zero external crates

## Commit messages

Write commit messages that explain *why* the change was made, not just what changed.

```
Good: "Fix operator arity to prevent greedy arg consumption in nested expressions"
Bad:  "Update parser"
```

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
