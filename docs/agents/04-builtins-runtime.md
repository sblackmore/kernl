# Builtins: stdlib vs executor

Two layers:

1. **`compiler/src/stdlib/mod.rs`** — metadata for the **typechecker** (`infer_builtin_call`).
2. **`compiler/src/runtime/executor.rs`** — **`call_builtin_or_fn`** implements what actually runs under **`kernlc --run`**.

## Implemented at runtime (safe to use)

Use these in strict programs:

- **`print`**, **`noop`** (void; use **`noop x`** with any arg)
- **`len`** (list or string length)
- **`abs`**, **`max`** (int/int only in executor), **`min`** (int/int only), **`sub`** (int − int), **`range`**, **`sqrt`**
- **`concat`**, **`split`**, **`head`**, **`tail`**, **`cons`**, **`join`**
- **`parse_int`**, **`show`**

Details and edge cases:

- **`head`** / **`tail`** on empty list: **`head`** → empty **`str`**; **`tail`** → empty list.
- **`concat`** / **`split`**: non-string args yield empty string / empty list.
- **`join`**: skips non-string list elements.

## In stdlib metadata only — **not** executor builtins

These **fail at runtime** unless you define a user function with the same name:

- **`filter`**, **`map`**, **`reduce`**

Do **not** generate `.knl` that relies on them until implemented in **`call_builtin_or_fn`**.

## Overloads

- **`max`**, **`min`** have int and float variants in stdlib; executor **`max`/`min`** paths shown above may only cover **ints** — prefer ints for portable interpreter behavior.

## User functions

- **`call(name, args)`** loads **`fn name`** from the program. Recursion is OK if the function is defined in the same program.
