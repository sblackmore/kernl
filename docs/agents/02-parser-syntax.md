# Parser and surface syntax

Sources: `compiler/src/parser/mod.rs`, `compiler/src/lexer/mod.rs`.

## Function shape

- **`fn name`** on its own line; **`in`** parameters on following lines (not `fn name arg` — that is invalid).
- Parameter syntax: **`param: type`**. Types include **`int`**, **`float`**, **`bool`**, **`str`**, **`[T]`** (e.g. **`[str]`**).

Good:

```text
fn op_line
  in stdin: str
  do …
```

Bad:

```text
fn op_line stdin
  in stdin: str
```

## `do` body is one expression

- **`do`** accepts a **single** `parse_expr`.
- For multiple steps, wrap in **`if true`** … **`end`** with a block of expressions, or delegate to helper functions.

```text
do if true
     let rows = state_lines stdin
     emit_keep "200" inner
   end
```

## `match`

- **`match scrutinee`** then arms: **`pattern => expr`** (pattern literal or `_`).
- **Arm bodies stop at the newline** for that arm’s expression list — long actions belong in **helper calls**, not multi-line arms.

```text
do match op_line stdin
     "health" => handle_health stdin
     _ => handle_unknown stdin
   end
```

Avoid **`match (call …)`** if **`(` … `)`** grouping is needed for calls — **parenthesized call groups are not supported** the way you might expect; prefer **`let`** + **`match name`** (see pipes doc).

## `if`

- **`if cond`** then body (multiple expressions until **`elif` / `else` / `end`**).
- No **`then`** keyword; **`else`** branch optional.

## Operators

- Comparisons and arithmetic use **prefix-style operators** with atoms: **`eq a b`**, **`gt a b`**, **`add a b`**, etc. (see lexer/parser — not always Infix `a + b`).
- Integer subtraction builtin: **`sub a b`** when lexer rejects `-` in your position.

## Comments

- **`#`** to end of line.

## Strings

- **`"…"`** with escapes **`\"`**, **`\\`**, **`\n`**, **`\t`**.

## No grouping parentheses for calls

- **`(foo bar)`** is **not** a general parenthesized expression.
- Do not rely on **`concat (a) (b)`** — use **`let`** steps (see idioms).
