# Pipes (`|`) and function calls

Sources: `executor.rs` (`Expr::Pipe`), `parser/mod.rs` (`parse_expr`).

## Pipe semantics

- **`left | right`** evaluates **`left`**, then passes its value as the **first argument** to **`right`** **only if** **`right`** is a **`Call(name, extra_args)`**.
- If **`right`** is a bare identifier (e.g. **`head`**, **`len`**, **`row_id`**), it is **variable lookup**, **not** a zero-arg call → **`undefined variable`** at runtime.

## Pipes are right-associative

```text
a | split "\n" | head
```

parses conceptually as:

```text
a | (split "\n" | head)
```

The inner **`split "\n" | head`** is wrong: **`split`** loses **`a`** as text input. **Always** bind split results first:

```text
if true
  let p = stdin | split "\n"
  head p
end
```

## Unary / single-arg builtins after data

Prefer **explicit calls** with atoms:

| Avoid | Prefer |
|-------|--------|
| `xs \| head` | `head xs` |
| `xs \| tail` | `tail xs` |
| `xs \| len` | `len xs` |
| `line \| row_id` | `let h = head rows` then `row_id h` |

## Binary builtins + pipe (good)

When **`right`** is **`Call`** with remaining args, pipe fills first arg:

```text
stdin | split "\n"
parts | list_nth 0
tail xs | list_nth nm
new_row | cons tail_rows
xs | tail | snoc line
```

## Zero-argument “calls”

- **`noop`** is declared with a dummy parameter — use **`noop 0`**, not **`noop`**, or the parser treats **`noop`** as an identifier in expression position.

## `show` / `parse_int`

- Avoid **`… \| show`** if **`show`** parses as bare ident — use **`let t = …`** then **`show t`**, or **`show parse_int s`** only when **`parse_int s`** is parsed as a single **`Call`** argument to **`show`** (prefer **`let`** for clarity).
