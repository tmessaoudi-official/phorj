# Phorj for PhpStorm / IntelliJ IDEA (and other JetBrains IDEs)

Phorj ships a single language server (`phg lsp`) and a single TextMate grammar
(`../vscode/syntaxes/phorj.tmLanguage.json`). JetBrains IDEs consume **both** without a compiled
plugin — using two built-in/marketplace mechanisms:

1. **Syntax highlighting** — JetBrains' native **TextMate Bundles** support reads the same grammar the
   VSCode extension uses.
2. **Language intelligence** (diagnostics, hover, signature help, go-to-definition, completion,
   document symbols, references, rename, formatting) — the **LSP4IJ** plugin runs `phg lsp` as an external language server.

This is the recommended, no-build path: identical behavior to the CLI (`phg check` / `phg format`) and to
the VSCode extension, because all three drive the *same* `phg lsp` server and the *same* grammar.

> A natively-compiled JetBrains plugin (richer integration, marketplace one-click install) is tracked as
> a follow-up — it would still wrap this same `phg lsp` server (JetBrains' own LSP API or LSP4IJ as a
> library). The setup below already delivers the full feature set today.

## Prerequisites

- The `phg` binary on your `PATH` (or note its absolute path). Build it with
  `cargo build --release` → `target/release/phg`.
- PhpStorm / IntelliJ **2023.2+**.

## 1. Syntax highlighting (TextMate bundle)

`Settings → Editor → TextMate Bundles → +` and select this repository's **`editors/vscode/`** directory.
JetBrains reads its `package.json` `grammars` entry and loads `syntaxes/phorj.tmLanguage.json`, so
`.phg` files are highlighted (keywords, types, strings + `{…}` interpolation, numbers, comments,
attributes). `/** … */` doc comments (DEC-419) carry their own scope
(`comment.block.documentation.phorj`), so a theme that styles documentation differently from an ordinary
`/* … */` block will show the difference here too. No VSCode required — the directory is just a standard TextMate-compatible bundle.

## 2. Language server (LSP4IJ)

1. Install **LSP4IJ** (`Settings → Plugins → Marketplace → "LSP4IJ"`).
2. `Settings → Languages & Frameworks → Language Servers → +` (a new *user-defined* language server):
   - **Name:** `Phorj`
   - **Command:** `phg lsp` (or `/absolute/path/to/phg lsp`)
   - **Mappings → File name patterns:** `*.phg` → language id `phorj`
3. Apply. Open any `.phg` file: diagnostics appear inline (identical to `phg check`), and hover
   (including a declaration's `/** … */` doc comment, rendered as markdown under its signature),
   signature help (parameter hints inside a call, `Ctrl+P`), go-to-definition (`Ctrl/Cmd+Click`),
   completion, structure view (document symbols), find-usages
   (references), rename, and reformat (`phg format`) all work through the server.

### Extensionless `#!…phg` shebang files (executable entries)

A phorj source may be an extensionless executable with a `#!/usr/bin/env phg` first line (the
tokenizer skips the shebang; `phg run ./bin/console` works). To light these up in PhpStorm:

- **Highlighting:** `Settings → Editor → File Types → Phorj` (or the TextMate bundle) → add a
  filename pattern for your entry (e.g. `console`), or register the name under the TextMate bundle.
- **Language server:** in the LSP4IJ mapping above, add a **File name pattern** for your extensionless
  entries (e.g. `console`, `bin/*`) → language id `phorj`, so the same `phg lsp` attaches. (LSP4IJ
  matches by name pattern; add each executable entry, or a `bin/*`-style glob.)
   - **Signature help** (the `(` and `,` trigger characters are advertised): your own functions and
     every `Core.*` native, with the argument being typed highlighted and the declaration's doc
     comment attached. It works while the file does not parse — inside an unclosed `(` it never does.
   - **Completion** (the `.` and `[` trigger characters are advertised, so it fires as you type)
     offers: `import Core.` → the importable Core module paths; `List.` / `Output.` → that Core
     module's members; `this.` / `myVar.` → the receiver's members, **stdlib types included**
     (`ServeConfig cfg` → `cfg.port`, `Request req` → `req.headers`; internal `private` fields and
     `static` methods are filtered, and your own class of the same name shadows the stdlib one);
     `#[` → the attribute names (`Entry`, `Config`, `Route`, `Deprecated`,
     `Invoke`, `ToString`, the DI set, plus your own `#[Attribute]`-marked classes), in both the bare
     (`#[Entry`) and canonical-path (`#[Core.Runtime.Entry`) spellings; plus in-scope top-level
     symbols, locals/params, and keywords. It is **parse-tolerant** — it works mid-edit on a buffer
     that does not yet parse (e.g. right after typing `Output.`).

### Notes

- **Formatting** routes to `phg format` (comment- and meaning-preserving); reformatting a file that does
  not parse is a no-op (the server never corrupts an in-progress buffer).
- **Find-usages is project-wide** (DEC-327 — it scans every project `.phg` on disk plus the other open
  buffers). **Rename is still single-document**: it returns edits for the current file only, so a
  cross-file rename must be finished by hand.
- Completion covers Core modules/members, import paths (Core **and** user packages), attribute names,
  declared-type instance members (`this.` / `myVar.`), and local symbols/keywords. An **inferred**
  receiver (`var x = …`) or a method chain still resolves to nothing — the deliberate conservative
  gate, since a wrong member list is worse than none.
- The server is **off the byte-identity spine** — it never runs the three execution backends, so it
  carries no interp/VM/PHP parity risk; its diagnostics equal `phg check` exactly.
