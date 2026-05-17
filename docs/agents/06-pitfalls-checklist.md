# Pitfalls checklist (before finishing `.knl`)

Use this as a quick verification pass.

## Parser / syntax

- [ ] **`fn`** uses **`in`** on the next lines — not **`fn f arg`**.
- [ ] **`do`** has exactly one top-level expression (or one **`if true`** block).
- [ ] **`match`** arms don’t rely on multi-line bodies; helpers used instead.
- [ ] No **`(` … `)`** grouped calls unless you’ve verified the parser accepts that shape (usually **avoid**).

## Pipes

- [ ] No **`data | split … | unary`** chains — **`split`** lost the left input (right-associativity).
- [ ] No **`xs | head`**, **`xs | tail`**, **`xs | len`**, **`xs | row_id`** — use **`head xs`**, **`let h = head xs`** + **`row_id h`**, **`len xs`**, etc.
- [ ] **`noop`** invoked as **`noop 0`** (or other arg), not bare **`noop`**.

## Builtins

- [ ] No **`filter` / `map` / `reduce`** unless you added executor support or user defs.
- [ ] **`concat`** only joins **two** string atoms per call — nested **`concat`** via **`let`**.

## Strings / JSON

- [ ] Building JSON without a dedicated **`escape`** helper — assume IDs/fields are safe, or keep JSON assembly in the host.
- [ ] Numbers in JSON: use **`parse_int`** + **`show`** for digits without quotes.

## Host integration

- [ ] If **`main`** receives **`stdin`**, document protocol (newlines matter).
- [ ] Remember piped stdin for **`main(str)`** is trimmed **leading/trailing** on the **whole** buffer (`cli.rs` / **`main_call_args`** **`trim()`**). Under **`kernl`**, piping implies **`--invoke-stdin`** unless **`--no-stdin`**.

## Verification command

```bash
cargo run --manifest-path compiler/Cargo.toml --bin kernl -- path/to/file.knl
```

With stdin (either form):

```bash
printf '...\n' | cargo run --manifest-path compiler/Cargo.toml --bin kernl -- path/to/file.knl
printf '...\n' | cargo run --manifest-path compiler/Cargo.toml -- path/to/file.knl --invoke-stdin --run
```
