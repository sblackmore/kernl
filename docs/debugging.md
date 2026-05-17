# Debugging native kernl binaries with GDB and LLDB

kernl can emit LLVM IR with **DWARF-like debug metadata** so native binaries remain inspectable in standard debuggers.

## Building with debug info

From the compiler crate root:

```bash
cargo run --release -- path/to/file.knl --target native --debug-info -o myprog
```

Ensure `runtime/libkernl_rt.a` is rebuilt after pulling changes (`cd ../runtime && make`).

## GDB quick reference

```bash
gdb ./myprog
```

Useful commands:

| Command | Action |
|--------|--------|
| `break main` | Stop at `main` (kernl `fn main` lowers to a native `main`) |
| `run` | Start the program |
| `bt` | Backtrace |
| `info locals` | Current frame locals |
| `list` | Source lines if file paths in DWARF resolve |

If source paths do not resolve, pass the directory containing the `.knl` file or use `directory` in GDB to add search paths.

### Example `.gdbinit` snippet

```
set pagination off
break main
run
```

## LLDB quick reference

```bash
lldb ./myprog
```

| Command | Action |
|--------|--------|
| `breakpoint set --name main` | Break at entry |
| `run` | Launch |
| `bt` | Backtrace |
| `frame variable` | Locals in frame |

## Interpreter debugger (`kernlc --run --debug`)

For **kernl source-level** stepping without a native binary, use the tree-walking executor:

```bash
cargo run --release -- examples/sum.knl --run --debug
```

You can set breakpoints on **function names** before execution starts; at each hit you get a small CLI (`continue`, `step`, `locals`, `bt`, etc.). This complements GDB/LLDB, which operate on lowered code.

## Profiling vs debugging

- **`--profile`** with **`--run`** — aggregate timings in the interpreter (no DWARF required).
- **`--instrument-llvm`** — inserts `__kernl_profile_enter` / `__kernl_profile_exit` into emitted LLVM IR for native builds; link against `kernl_profile.o` via `runtime/libkernl_rt.a`.
