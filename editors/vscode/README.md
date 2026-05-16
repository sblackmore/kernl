# kernl for VS Code

Language support for [kernl](https://github.com/kernl-lang/kernl) — the AI-native programming language.

## Features

- **Syntax highlighting** for `.knl` files
- **Diagnostics** — real-time error reporting from the kernl compiler
- **Hover information** — type and documentation on hover
- **Completion** — context-aware code completion

## Requirements

The `kernl-lsp` binary must be available on your `PATH`, or configured explicitly via the `kernl.serverPath` setting.

## Configuration

| Setting              | Default | Description                                                        |
|----------------------|---------|--------------------------------------------------------------------|
| `kernl.serverPath`   | `""`    | Path to the `kernl-lsp` binary. If empty, searches `PATH`.        |
| `kernl.trace.server` | `"off"` | Traces communication between VS Code and the kernl language server.|

## Building from source

```bash
cd editors/vscode
npm install
npm run compile
```

To install the extension locally, copy or symlink this directory into `~/.vscode/extensions/kernl`.
