# kernl Examples

Annotated examples showing how kernl programs work, from simple arithmetic to AI-native fluid functions.

## 1. Simple arithmetic

**`examples/clamp.knl`**

```
fn clamp
  in  val: int lo: int hi: int
  out result: int
  inv gte result lo
  inv lte result hi
  do  max lo min hi val
```

**What this does:**

Clamps `val` between `lo` and `hi`. If `val` is below `lo`, returns `lo`. If above `hi`, returns `hi`. Otherwise returns `val`.

**Key concepts:**
- **Multiple parameters** on one `in` line: `val: int lo: int hi: int`
- **Named return:** `out result: int` — the name `result` can be referenced in invariants
- **Two invariants** that together guarantee the result is within bounds
- **Nested calls:** `max lo min hi val` — `min` gets `hi` and `val`, `max` gets `lo` and the result of `min`

**Equivalent Python:**
```python
def clamp(val: int, lo: int, hi: int) -> int:
    result = max(lo, min(hi, val))
    assert result >= lo
    assert result <= hi
    return result
```

kernl: 30 tokens. Python: 44 tokens. **32% reduction.**

---

## 2. Pipes and composition

**`examples/sum.knl`**

```
fn sum_positive
  in  nums: [int]
  out result: int
  inv gte result 0
  do  filter nums gt 0 | reduce add
```

**What this does:**

Takes a list of integers, filters to only positive values, then sums them.

**Key concepts:**
- **List type:** `[int]` — a list of integers
- **Pipe composition:** `filter nums gt 0 | reduce add` reads left to right
  1. `filter nums gt 0` — keep elements where `gt element 0` is true
  2. `| reduce add` — pipe the filtered list into `reduce`, which folds with `add`
- **Operator as argument:** `gt 0` is a partially applied predicate; `add` is passed as the reduction function
- **Invariant on output:** the sum of positive numbers is always non-negative

**Why pipes matter for LLMs:**

Without pipes, this would be `reduce(add, filter(nums, gt, 0))`. Nested parens require the LLM to track opening and closing positions across token distances. Pipes eliminate this — every expression reads left to right with no nesting.

---

## 3. Structs and field access

**`examples/transfer.knl`**

```
struct Account
  id: uint
  balance: int
  owner: str
end

fn transfer
  in  amount: uint from: Account to: Account
  inv gte from.balance amount
  guarantee atomic
  do  debit from amount | credit to amount
```

**What this does:**

Defines an `Account` struct and a `transfer` function that moves funds between two accounts.

**Key concepts:**
- **Struct definition:** `struct Account ... end` — keyword-delimited, no braces
- **User-defined types in params:** `from: Account` uses the struct as a parameter type
- **Field access in invariants:** `from.balance` accesses the `balance` field — the invariant checks that the sender has sufficient funds
- **Guarantee clause:** `guarantee atomic` declares a semantic guarantee (future: the compiler or runtime enforces this)
- **Pipe as sequencing:** `debit from amount | credit to amount` — debit first, then credit

**Equivalent Rust:**
```rust
struct Account {
    id: u64,
    balance: i64,
    owner: String,
}

fn transfer(amount: u64, from: &mut Account, to: &mut Account) {
    assert!(from.balance >= amount as i64);
    from.balance -= amount as i64;
    to.balance += amount as i64;
}
```

kernl: 40 tokens. Rust: 68 tokens. **41% reduction.** And the kernl version explicitly declares atomicity as a contract, not just an implicit assumption.

---

## 4. Fluid mode (AI-native)

**`examples/recommend.knl`**

```
fn recommend
  mode fluid
  in  user: User context: Context
  intent "surface items user would engage with"
  confidence 0.85
  fallback popular_items context
```

**What this does:**

Declares a recommendation function whose behavior is specified by **intent** rather than implementation. At runtime, a resolver (which could be an LLM) determines the output. If the resolver's confidence is below 85%, the `fallback` executes instead.

**Key concepts:**
- **`mode fluid`** — switches from strict (default) to fluid verification
- **`intent`** — natural language specification of desired behavior
- **`confidence`** — minimum confidence threshold for the resolver
- **`fallback`** — deterministic fallback when confidence is insufficient
- **No `do` clause** — the body IS the intent + fallback; there's no hand-written implementation

**Why this matters:**

This is the construct that doesn't exist in any other language. Fluid mode bridges the gap between "tell the computer exactly what to do" and "tell the computer what you want." The spec is the contract, the resolver fills in the implementation, and the fallback guarantees predictable behavior when AI confidence is low.

---

## 5. Conditionals

**`benchmark/programs/fibonacci.knl`**

```
fn fib
  in n: int
  out result: int
  inv gte result 0
  do if lte n 1
    n
  else
    add fib sub n 1 fib sub n 2
  end
```

**What this does:**

Classic recursive Fibonacci. Returns `n` if `n <= 1`, otherwise returns `fib(n-1) + fib(n-2)`.

**Key concepts:**
- **If/else blocks** terminated by `end` — not braces
- **Operators in conditions:** `lte n 1` instead of `n <= 1`
- **Recursive calls:** `fib sub n 1` — call `fib` with `sub n 1` (which is `n - 1`)
- **Nested expression:** `add fib sub n 1 fib sub n 2` — this is `fib(n-1) + fib(n-2)` in prefix notation

**Equivalent Python:**
```python
def fib(n: int) -> int:
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)
```

Note: the `if` block in kernl is an **expression** that returns a value — the last expression in each branch is the result. No `return` keyword needed.

---

## Compilation examples

### Emit LLVM IR

```bash
$ kernlc examples/clamp.knl --target llvm
```

```llvm
define i64 @clamp(i64 %val, i64 %lo, i64 %hi) {
entry:
  %1 = call i64 @max(i64 %lo, i64 %min, i64 %hi, i64 %val)
  ret i64 %1
}
```

### Emit WebAssembly

```bash
$ kernlc examples/clamp.knl --target wasm
```

```wat
(module
  (func $clamp (param $val i64) (param $lo i64) (param $hi i64) (result i64)
    local.get $lo
    local.get $min
    local.get $hi
    local.get $val
    call $max
  )
  (export "clamp" (func $clamp))
)
```

### Dump AST

```bash
$ kernlc examples/clamp.knl --target debug
```

Prints the full parsed AST structure — useful for debugging or understanding how the parser interprets your code.

---

## Writing your own

1. Create a `.knl` file
2. Start with `fn <name>`
3. Add `in` parameters with types
4. Add `out` return binding
5. Add `inv` invariants (optional but recommended)
6. Add `do` implementation
7. Run `kernlc your_file.knl --target debug` to verify parsing

When in doubt, use **pipes** for composition and **named operators** for all arithmetic and comparison. Keep structures flat — avoid deeply nested expressions.

---

## Cloud deployment examples

For minimal **HTTP APIs on AWS Lambda** (and placeholders for other providers), see [`examples/cloud/README.md`](../examples/cloud/README.md).
