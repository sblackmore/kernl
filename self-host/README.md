# kernl Self-Hosting Examples

These are kernl programs that implement parts of the kernl compiler in kernl itself.

## Purpose

- **Demonstrate self-description**: kernl can express its own lexical analysis, evaluation, and formatting rules
- **Test cases**: these programs exercise the language's core features — structs, pattern matching via `if`/`elif`, recursion, invariants, and named operators
- **Documentation**: each file serves as a readable specification of a compiler phase

## Files

| File | Description |
|---|---|
| `tokenizer.knl` | Simplified lexical analyzer — classifies characters and identifies keywords |
| `parser.knl` | Parser-shaped helpers — expression and binding shapes (spec-oriented) |
| `typechecker.knl` | Type-checking sketches — unary/binary ops and simple constraints |
| `optimizer.knl` | Optimization patterns — constant folding and branch elimination rules |
| `evaluator.knl` | Expression evaluator with arithmetic ops, factorial, and fibonacci |
| `formatter.knl` | Source code formatter — tracks indentation for block-structured code |

## Current Status

- **Lexical analysis** (`tokenizer.knl`): character classification, keyword detection, token tagging
- **Parsing & types** (`parser.knl`, `typechecker.knl`): illustrative fragments aligned with the Rust compiler; not a drop-in replacement
- **Optimizer** (`optimizer.knl`): documented rewrite rules mirroring `optimize` passes
- **Expression evaluation** (`evaluator.knl`): binary ops, unary ops, recursive functions with invariants
- **Source formatting** (`formatter.knl`): indent/dedent tracking for kernl's block keywords

These programs parse and type-check with the kernl compiler. Together they exercise enums, matching, contracts, and pipe-heavy style toward a fuller self-hosted toolchain.
