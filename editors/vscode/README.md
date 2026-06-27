# Phorge — VS Code extension

A thin client that connects VS Code to the Phorge language server (`phg lsp`). All language features
live in the server; this extension only registers the `phorge` language (`*.phg`) and launches the
server over stdio.

## Features

Whatever `phg lsp` provides (currently):

- **Diagnostics** — type/parse errors and lints, live as you type (identical to `phg check`).
- **Hover** — the declaration signature of the symbol under the cursor.
- **Go-to-definition** — jump to a function / class / enum / interface / trait / type declaration.

## Prerequisites

- The `phg` binary on your `PATH` (or set `phorge.serverPath` in settings to its absolute path).
  Build it with `cargo build --release` (the binary is `target/release/phg`).

## Run it (Extension Development Host)

```sh
cd editors/vscode
npm install          # fetches vscode-languageclient
code .               # then press F5 → "Run Extension" to launch the dev host
```

Open any `.phg` file in the dev host; diagnostics, hover, and go-to-definition activate automatically.

## Package / install locally

```sh
npm install -g @vscode/vsce
cd editors/vscode
vsce package         # produces phorge-0.1.0.vsix
code --install-extension phorge-0.1.0.vsix
```

## Configuration

- `phorge.serverPath` (default `"phg"`) — path to the `phg` binary; the server is started as
  `phg lsp`.
