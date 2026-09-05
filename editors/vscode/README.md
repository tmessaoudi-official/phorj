# Phorj — VS Code extension

A thin client that connects VS Code to the Phorj language server (`phg lsp`), plus a TextMate grammar
for syntax highlighting. The language *intelligence* lives entirely in the server; this extension
registers the `phorj` language (`*.phg`, **and extensionless files whose first line is a `#!…phg`
shebang** — the `./bin/console`-style executable entry, via the language's `firstLine` match), ships
the grammar (`syntaxes/phorj.tmLanguage.json`), and launches the server over stdio. Because the
client selects documents by language id (not a `*.phg` glob), a shebang'd extensionless file gets the
full server (diagnostics, completion, hover, …) exactly like a `.phg` file. (The same grammar +
server power the JetBrains/PhpStorm setup — see `../phpstorm/README.md`.)

## Features

- **Doc comments** — `/** … */` (DEC-419) is a documentation comment: it is highlighted as documentation
  and shown on hover under the declaration's signature, plus in the completion detail pane. A plain
  `/* … */` is deliberately NOT documentation.
- **Syntax highlighting** — keywords, types, strings with `{…}` interpolation, numbers, comments, and
  `#[…]` attributes (TextMate grammar, no server needed).
- **Diagnostics** — type/parse errors and lints, live as you type (identical to `phg check`).
- **Hover** — the declaration signature of the symbol under the cursor.
- **Signature help** — inside a call's parentheses, the callee's signature with the argument being
  typed highlighted; fires on `(` and `,`. Covers your own functions (same file, or a sibling file in
  the package), and every `Core.*` native straight from the registry (`String.repeat("a", |` →
  `function String.repeat(string, int): string`). Works while the buffer does not parse — which,
  inside an unclosed `(`, is always — and carries the declaration's `/** … */` doc like hover does.
- **Go-to-definition** — jump to a function / class / enum / interface / trait / type declaration.
- **Completion** — top-level symbols, in-scope locals/params, and keywords; `import Core.` → importable
  module paths (Core + your own packages); `List.` / `this.` / `myVar.` → that receiver's members —
  including the members of a **stdlib** type, so `ServeConfig cfg` → `cfg.port` and `Request req` →
  `req.headers` complete (internal `private` fields and `static` methods are filtered out, and your own
  class of the same name shadows the stdlib one); and
  `#[` → **attribute names** (`Entry`, `Config`, `Route`, `Deprecated`, `Invoke`, `ToString`, the DI
  set, plus your own `#[Attribute]`-marked classes), offered in both the bare (`#[Entry`) and
  canonical-path (`#[Core.Runtime.Entry`) spellings. `.` and `[` are advertised as trigger characters,
  so it fires as you type.
- **Document symbols** — the file outline (classes/enums carry their members).
- **Workspace symbols** — `Ctrl/Cmd+T` searches every top-level declaration across the open buffers
  AND every `.phg` on disk under the workspace (case-insensitive substring; members of the matching
  classes/enums are included, tagged with their container).
- **Folding** — every multi-line top-level declaration and every multi-line class/trait member folds
  from its first line to its closing brace (one-line declarations fold nothing).
- **Find references** + **document highlight** — every use of the symbol under the cursor
  (scope-accurate). For a top-level symbol this is **project-wide** (DEC-327): every project `.phg` on
  disk plus the other open buffers.
- **Rename** — rename a symbol and all its uses **in the current file** (find-references is
  project-wide, but rename returns edits for one document, so a cross-file rename needs finishing by
  hand).
- **Formatting** — reformat via `phg format` (comment- and meaning-preserving).

## Prerequisites

- The `phg` binary on your `PATH` (or set `phorj.serverPath` in settings to its absolute path).
  Build it with `cargo build --release` (the binary is `target/release/phg`).

## Run it (Extension Development Host)

```sh
cd editors/vscode
npm install          # fetches vscode-languageclient
code .               # then press F5 → "Run Extension" to launch the dev host
```

Open any `.phg` file in the dev host; diagnostics, hover, signature help, go-to-definition, folding and
`Ctrl/Cmd+T` workspace symbols activate automatically.

## Package / install locally

```sh
npm install -g @vscode/vsce
cd editors/vscode
vsce package         # produces phorj-0.7.0.vsix
code --install-extension phorj-0.7.0.vsix
```

## Configuration

- `phorj.serverPath` (default `"phg"`) — path to the `phg` binary; the server is started as
  `phg lsp`.
