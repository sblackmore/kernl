# kernl Language Specification

**Version:** 0.1.0-draft

## Design principles

1. **Token efficiency** — minimize tokens required to express a program. Every token must carry semantic weight.
2. **Flat structure** — no deep nesting. Blocks are keyword-delimited, not brace-delimited.
3. **Line independence** — each line is parseable and verifiable without requiring surrounding context.
4. **Intent-first** — programs declare what they want, not how to achieve it. The compiler resolves mechanics.
5. **Verification-native** — invariants and contracts are first-class, not annotations bolted on after the fact.

## File extension

`.knl`

## Comments

```
# single line comment
```

No multi-line comment syntax. Comments are discouraged in LLM-generated code — the spec is the documentation.

## Functions

```
fn <name>
  in  <param>: <type> [<param>: <type> ...]
  out <name>: <type>
  inv <invariant expression>
  do  <implementation>
```

- `fn` — declares a function
- `in` — input parameters (space-separated `name: type` pairs)
- `out` — return binding and type
- `inv` — invariant (zero or more, each on its own line)
- `do` — implementation expression

All clauses are optional except `fn` and `do`.

### Example

```
fn clamp
  in val: int lo: int hi: int
  out result: int
  inv result >= lo
  inv result <= hi
  do  max lo (min hi val)
```

## Types

### Primitives

| Type     | Description             |
|----------|-------------------------|
| `int`    | signed integer (64-bit) |
| `uint`   | unsigned integer        |
| `float`  | 64-bit floating point   |
| `bool`   | true / false            |
| `str`    | UTF-8 string            |
| `void`   | no value                |

### Compound

| Syntax        | Description       |
|---------------|-------------------|
| `[T]`         | list of T         |
| `{K: V}`      | map from K to V   |
| `(T, U)`      | tuple             |
| `T?`          | optional (T or nothing) |

## Operators

Operators are named, not symbolic, to reduce ambiguity in LLM generation:

| Operator | Meaning            |
|----------|--------------------|
| `add`    | addition           |
| `sub`    | subtraction        |
| `mul`    | multiplication     |
| `div`    | division           |
| `mod`    | modulo             |
| `eq`     | equality           |
| `neq`    | not equal          |
| `gt`     | greater than       |
| `lt`     | less than          |
| `gte`    | greater or equal   |
| `lte`    | less or equal      |
| `and`    | logical and        |
| `or`     | logical or         |
| `not`    | logical not        |

## Pipe

The pipe `|` chains expressions left to right:

```
filter nums gt 0 | reduce add
```

Equivalent to `reduce(add, filter(nums, gt, 0))` — but flat, not nested.

## Bindings

```
let x: int = 42
let name: str = "kernl"
```

Bindings are immutable by default.

```
mut counter: int = 0
```

`mut` declares a mutable binding.

## Conditionals

```
if <condition>
  <body>
elif <condition>
  <body>
else
  <body>
end
```

`end` terminates blocks — no brace matching required.

## Iteration

```
each item in collection
  <body>
end
```

```
while <condition>
  <body>
end
```

## Structs

```
struct Account
  id: uint
  balance: int
  owner: str
end
```

## Mode annotations

### Strict (default)

All types resolved at compile time. All invariants checked statically. No ambiguity permitted.

### Fluid

```
fn recommend
  mode fluid
  in user: User context: Context
  intent "surface items user would engage with"
  confidence 0.85
  fallback popular_items context
```

- `mode fluid` — marks this function for runtime resolution
- `intent` — natural language description of desired behavior
- `confidence` — minimum confidence threshold for runtime resolution
- `fallback` — expression to use when confidence threshold is not met

Fluid functions compile to a runtime that can invoke external resolvers (including LLMs) for underspecified regions.

## Algebraic types (enums)

Enums are sums of variants; each variant may carry zero or more typed fields.

```
enum OptionInt
  None
  Some int
end
```

Construct values with `EnumName VariantName` followed by field expressions (parentheses optional when arity matches):

```
OptionInt None
OptionInt Some 42
```

## Pattern matching

```
match scrutinee
  VariantName x y =>
    body_expr
  OtherVariant =>
    body_expr
  _ =>
    default_body
end
```

- `_` is a wildcard pattern.
- Literal patterns (`42`, `true`, `"hi"`) are supported where the scrutinee type matches.
- Tuple patterns `(a, b)` match tuple scrutinees.

Arms are checked for exhaustiveness against enum variants when the scrutinee is a known enum type.

## Contracts

Beyond `inv`, functions may declare pre- and postconditions:

```
fn safe_div
  in a: int b: int
  out result: int
  req neq b 0
  ens eq (mul result b) a
  do div a b
```

- `req` — precondition (must hold at entry)
- `ens` — postcondition (must hold for the result binding after `do`)

These participate in SMT verification (`kernlc --verify`) and in proof export (`--export-lean`, `--export-coq`).

## Async and concurrency

```
fn compute
  mode async
  in x: int
  out r: int
  do add x 1
```

- `mode async` — function body is evaluated eagerly in the interpreter but wrapped as a **future** value; full scheduling is implementation-defined.

Structured concurrency keywords:

- `spawn expr` — start work (interpreter: evaluates inner expression and wraps as `Future`).
- `await expr` — unwrap a future (interpreter: evaluates inner value and strips `Future`).
- `send ch val` / `recv ch` — channel send/receive (reserved for runtime expansion).

## Modules

```
mod math
use io.print
```

- `mod` declares the current module
- `use` imports from another module

## Reserved for future

- `test` — inline test blocks
- `proof` — formal proof annotations beside export pipelines
- `target` — compilation target hints in source
