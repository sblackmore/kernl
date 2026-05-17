# Idioms for real `.knl` programs

## Multi-step `do`

Use **`if true`** … **`end`**:

```text
fn handle_list
  in stdin: str
  do if true
       let rows = state_lines stdin
       let mid = join_orders_json rows
       let inner = concat "{\"orders\":[" mid
       let inner2 = concat inner "]}"
       emit_keep "200" inner2
     end
```

## `concat` chains

**`concat a concat b c`** does **not** reliably parse as nested calls. Prefer:

```text
let p1 = concat "{\"id\":\"" f0
let p2 = concat p1 "\",\"customerId\":\""
…
```

Two **atoms** per **`concat`** call.

## Empty string lists

No **`[]`** literal. Common pattern:

```text
let z = "" | split "\t"
tail z
```

(for an empty **`[str]`** when **`split`** yields one segment — depends on data; used in demos after validating behavior.)

## Recursion on lists

- **`filter`/`map`** builtins are unavailable → use **`if` + `head`/`tail`/`cons`**.
- Pass **`sub n 1`** for index decrement: **`let nm = sub n 1`** then **`tail xs | list_nth nm`**.

## Side effects + return value

- **`print`** returns **`void`**.
- **`kernlc --run`** prints **`main`**’s result if it is **not** **`void`** — returning **`0`** from helpers can produce stray **`0`** in stdout. Use **`noop 0`** or **`void`**-typed tails.

## `match` dispatch

Keep arms one line; push logic into **`handle_*`** functions:

```text
do match op_line stdin
     "orders.list" => handle_list stdin
     _ => handle_unknown stdin
   end
```

## Types

- Annotate **`in`** params with concrete types (**`str`**, **`[str]`**, **`int`**) when possible — helps the typechecker and readers.
