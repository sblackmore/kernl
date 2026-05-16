# Compiler Architecture

This document describes the internal architecture of `kernlc`, the kernl compiler. It's intended for contributors who want to understand how the compiler works and where to add new features.

## Overview

The compiler is a **single-pass pipeline** — source text flows through five phases, each transforming the representation:

```
Source (.knl)
    │
    ▼
┌─────────────────┐
│  Lexer           │  source text → token stream
│  src/lexer/      │
└────────┬─────────┘
         │  Vec<Spanned>
         ▼
┌─────────────────┐
│  Parser          │  token stream → AST
│  src/parser/     │
└────────┬─────────┘
         │  Program (AST)
         ▼
┌─────────────────┐
│  Type Checker    │  AST → type errors
│  src/typeck/     │
└────────┬─────────┘
         │  Vec<TypeError>
         ▼
┌─────────────────┐
│  Verifier        │  AST → verification errors
│  src/verify/     │
└────────┬─────────┘
         │  Vec<VerifyError>
         ▼
┌─────────────────┐
│  Code Generator  │  AST → output (LLVM IR / WAT / Debug)
│  src/codegen/    │
└─────────────────┘
```

The entire pipeline is orchestrated by `compile()` in `src/lib.rs`.

## Phase 1: Lexer

**Files:** `src/lexer/mod.rs`, `src/lexer/token.rs`

The lexer converts UTF-8 source text into a flat vector of tokens. Each token is paired with a `Span` for error reporting (line, column, byte offset, length).

### Token categories

| Category | Examples | Purpose |
|----------|---------|---------|
| Keywords | `fn`, `in`, `out`, `inv`, `do`, `struct`, `end` | Structural delimiters |
| Operators | `add`, `sub`, `gt`, `eq`, `not` | Named arithmetic/logic ops |
| Punctuation | `:`, `\|`, `=`, `[`, `]`, `.`, `?`, `@` | Syntax markers |
| Literals | `42`, `3.14`, `"hello"`, `true` | Values |
| Identifiers | `x`, `Account`, `sum_positive` | Names |
| Structure | `Newline`, `Comment`, `Eof` | Formatting |

### Key design decision

Newlines are **significant tokens**. They delimit function clauses (`in`, `out`, `inv`, `do`). The parser uses them to determine where one clause ends and the next begins, without requiring any explicit delimiter.

### Keyword resolution

The lexer uses `Token::keyword_from_str()` to resolve identifiers into keywords or operators at lex time. The string `"add"` becomes `Token::Add`, not `Token::Ident("add")`. This keeps the parser simple — it can match on token variants directly.

## Phase 2: Parser

**Files:** `src/parser/mod.rs`, `src/parser/ast.rs`

The parser is a **recursive descent parser** that produces an abstract syntax tree (AST). The grammar is designed to be LL(1) — at most one token of lookahead is needed for any parse decision.

### AST structure

```
Program
  └── Vec<Item>
        ├── Function { name, params, returns, invariants, mode, body, ... }
        ├── StructDef { name, fields }
        ├── ModuleDecl { name }
        └── UseDecl { path }
```

Expressions (`Expr`) are the core:

```
Expr
  ├── IntLit, FloatLit, StrLit, BoolLit
  ├── Ident(name)
  ├── Op(op, args)          # named operator with fixed arity
  ├── Call(name, args)      # function call with greedy args
  ├── Pipe(left, right)     # composition: left | right
  ├── Field(base, name)     # struct field access: x.y
  ├── Temporal(expr, tag)   # temporal reference: x@pre
  ├── Let { name, ty, value, mutable }
  ├── If { condition, then, elif_branches, else }
  ├── Each { binding, iter, body }
  ├── While { condition, body }
  └── Block(exprs)
```

### Key design decision: operator arity

The parser has three levels of expression parsing:

1. **`parse_atom()`** — simple values only. Literals, identifiers, field access. No argument consumption.
2. **`parse_arg()`** — atom or operator-with-atoms. Used when parsing function call arguments.
3. **`parse_primary()`** — full expressions. Operators, function calls, control flow.

Operators have **fixed arity**: binary ops (like `add`, `gt`) consume exactly 2 atoms; `not` consumes 1. This is critical for unambiguous prefix parsing without delimiters.

Function calls are **greedy**: they consume all remaining args (atoms or ops) until a newline, pipe, or end of scope.

```
add x 1                    → Op(Add, [Ident("x"), IntLit(1)])
filter nums gt 0           → Call("filter", [Ident("nums"), Op(Gt, [IntLit(0)])])
filter nums gt 0 | reduce add → Pipe(Call(...), Call("reduce", [Op(Add, [])]))
```

## Phase 3: Type Checker

**Files:** `src/typeck/mod.rs`

The type checker performs three tasks:

1. **Type resolution** — converts AST types (`Type::Named("int")`) to resolved types (`Ty::Int`). Verifies that all referenced types exist.
2. **Expression inference** — infers the type of every expression in the AST.
3. **Constraint checking** — verifies invariant expressions are boolean, return types match body types.

### Type environment

The checker builds a `TypeChecker` struct containing:
- `structs: HashMap<String, Vec<(String, Ty)>>` — struct definitions with field types
- `functions: HashMap<String, FnSig>` — function signatures

### Unknown type handling

When a type cannot be resolved (e.g., a call to an unknown function), the checker returns `Ty::Unknown` and does not flag an error. This enables **partial checking** — you can type-check a file that references external functions without requiring all definitions to be present.

## Phase 4: Verifier

**Files:** `src/verify/mod.rs`

The verifier checks mode-specific constraints:
- **Fluid functions** must declare an `intent` clause
- **Strict functions** with invariants are flagged for future static proof obligations

This is currently minimal. The intended evolution is toward full formal verification of strict-mode invariants at compile time.

## Phase 5: Code Generator

**Files:** `src/codegen/mod.rs`, `src/codegen/llvm.rs`, `src/codegen/wasm.rs`

### Target: Debug

Pretty-prints the AST using Rust's `Debug` trait. Useful for inspecting what the parser produces.

### Target: LLVM IR

Emits textual LLVM IR (`.ll` files). The emitter:
- Maps kernl types to LLVM types (`int` → `i64`, `float` → `double`, `bool` → `i1`, `str` → `i8*`)
- Maps operators to LLVM instructions (`add` → `add i64`, `gt` → `icmp sgt i64`)
- Emits `define` blocks for functions, `%Type = type { ... }` for structs
- Generates `declare` for unresolved external functions
- Desugars pipe chains into sequential `call` instructions

### Target: WASM (WAT)

Emits WebAssembly Text format. The emitter:
- Maps kernl types to WASM value types (`int` → `i64`, `float` → `f64`, `bool` → `i32`)
- Maps operators to WASM instructions (`add` → `i64.add`, `gt` → `i64.gt_s`)
- Emits stack-based instructions (arguments are pushed, then the instruction pops and pushes)
- Generates `(export ...)` for all functions
- Handles control flow with `(if ...)`, `(block (loop ...))` constructs

## Adding a new feature

### New keyword or operator

1. Add the token to `Token` enum in `src/lexer/token.rs`
2. Add the string mapping in `Token::keyword_from_str()`
3. Handle the token in the parser (whichever parse method is appropriate)
4. Add the AST node if needed in `src/parser/ast.rs`
5. Handle it in the type checker
6. Handle it in both code generators
7. Add tests at every level

### New compilation target

1. Create a new file `src/codegen/your_target.rs`
2. Add the target variant to `Target` enum in `src/codegen/mod.rs`
3. Add the dispatch in `Codegen::emit()`
4. Add the CLI flag in `src/main.rs`

### New type or type feature

1. Add the type to `Type` enum in `src/parser/ast.rs`
2. Add parsing in `parse_type()` in `src/parser/mod.rs`
3. Add resolution in `resolve_type()` in `src/typeck/mod.rs`
4. Add LLVM/WASM type mapping in the code generators

## Testing

Tests live as `#[cfg(test)]` modules within each source file.

```bash
cargo test                      # all tests
cargo test lexer                # lexer tests only
cargo test parser               # parser tests only
cargo test typeck               # type checker tests only
cargo test codegen::llvm        # LLVM codegen tests only
cargo test codegen::wasm        # WASM codegen tests only
```

Each test module has a helper function that runs the full pipeline up to that phase, so tests are self-contained and easy to write.
