# Installing kernl

## Prerequisites

kernl's compiler is written in Rust. You need the Rust toolchain installed.

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Or visit [rustup.rs](https://rustup.rs/) for platform-specific instructions.

**Minimum Rust version:** 1.80.0

Verify your installation:

```bash
rustc --version
cargo --version
```

## Building the compiler

### Clone the repository

```bash
git clone https://github.com/kernl-lang/kernl.git
cd kernl
```

### Build (debug)

```bash
cd compiler
cargo build
```

The binary is at `compiler/target/debug/kernlc`.

### Build (release)

```bash
cd compiler
cargo build --release
```

The optimized binary is at `compiler/target/release/kernlc`.

### Run tests

```bash
cd compiler
cargo test
```

You should see all 27 tests pass across the lexer, parser, type checker, and code generation modules.

### Install to PATH (optional)

```bash
cd compiler
cargo install --path .
```

This installs `kernlc` to `~/.cargo/bin/`, which is typically already in your PATH. After this, you can run:

```bash
kernlc file.knl --target llvm
```

from any directory.

## Building the benchmark suite

```bash
cd benchmark
cargo build
cargo run
```

This compares equivalent programs in kernl, Python, and Rust across token count, character count, and estimated GPT-4 token usage.

## Verifying your installation

Create a file called `test.knl`:

```
fn double
  in x: int
  out result: int
  do mul x 2
```

Then run:

```bash
kernlc test.knl --target debug   # should print the AST
kernlc test.knl --target llvm    # should print LLVM IR
kernlc test.knl --target wasm    # should print WAT
```

If all three produce output without errors, your installation is working correctly.

## Optional: LLVM toolchain

To compile LLVM IR output to native binaries, install LLVM:

**macOS:**
```bash
brew install llvm
```

**Ubuntu/Debian:**
```bash
sudo apt install llvm
```

Then:
```bash
kernlc program.knl --target llvm > program.ll
llc program.ll -o program.o
clang program.o -o program
```

## Optional: WebAssembly toolchain

To convert WAT output to binary WASM, install WABT (WebAssembly Binary Toolkit):

**macOS:**
```bash
brew install wabt
```

**Ubuntu/Debian:**
```bash
sudo apt install wabt
```

Then:
```bash
kernlc program.knl --target wasm > program.wat
wat2wasm program.wat -o program.wasm
```

## Troubleshooting

**`cargo build` fails with edition errors:**
Update your Rust toolchain: `rustup update`

**Tests fail:**
Please open an issue at [github.com/kernl-lang/kernl/issues](https://github.com/kernl-lang/kernl/issues) with the output of `cargo test` and `rustc --version`.

**Permission denied on `cargo install`:**
Ensure `~/.cargo/bin` exists and is writable: `mkdir -p ~/.cargo/bin`
