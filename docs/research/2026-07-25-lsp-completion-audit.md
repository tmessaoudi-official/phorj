# LSP completion / editor-experience audit (2026-07-25)

Goal: generalise from the `Core.` → `Core.Core.Output` duplication fix (commit `66f940b`) — find
**every** other completion/editor issue so the LSP becomes a real language helper and explorer.

Method: read `src/lsp/{mod,catalog,scope,keywords,references,symbols}.rs`,
`src/lsp/completion/{mod,tests}.rs`, `editors/vscode/*`, and the native registry
(`src/native/mod.rs` + module files); then **drove the live `phg lsp` server** (release build) with
real JSON-RPC `didOpen` + `textDocument/completion` payloads for each scenario. Every "actual" below
is verified against real server output unless marked [Inferred].

## Baselines that WORK (verified, for contrast)

- `import Core.` → 46 module labels, each with a `textEdit` replacing the typed path (the `66f940b` fix). [Verified]
- `Output.` (module qualifier) → `capture, print, printLine`. [Verified]
- `this.` inside a class → own + inherited members (`go, name`). [Verified]
- Typed user-class local `Dog d = …; d.` → class members + inherited. [Verified — completion/tests.rs]
- General ctx on a broken buffer → top-level symbols + locals + imported qualifiers + keywords. [Verified]

## Ranked punch-list

| # | Sev | Context | Repro (buffer • cursor) | Expected | Actual | Fix sketch (file:line) |
|---|-----|---------|-------------------------|----------|--------|------------------------|
| 1 | **P1** | Builtin-typed receiver UFCS members | `string s = "hi";` then `s.` (cursor after dot) | `upperCase`, `length`, `trim`, … (the `Core.String` natives reachable UFCS-style `s.f()`≡`String.f(s)`) | **0 items** [Verified] | `completion/mod.rs:118-146` Ctx::Member — after `module_members(recv)` is empty and `class_members` returns empty, add a builtin/UFCS fallback: resolve the receiver's `Ty` and offer every `native::registry()` row whose **first param unifies with the receiver type** (the checker's `try_ufcs` rule — see `src/native/list.rs:152`, `checker::calls`). New `catalog::ufcs_members(recv_ty)`. |
| 2 | **P1** | Builtin generic receiver UFCS members | `List<int> xs = [1];` then `xs.` | `map`, `filter`, `length`, `push`, … | **0 items** [Verified] | Same as #1. `List<int>` resolves via `scope::named_head` → `"List"` but `catalog::class_members` only scans **user** classes (`catalog.rs:40-52`), so builtins fall through to empty. Map `List`→`Core.List`, `Map`→`Core.Map`, `Set`→`Core.Set`, `string`→`Core.String`, `int/float`→`Core.Math` in the UFCS filter, plus generic-subject natives. |
| 3 | **P1** | Member / function / variant import completion | `import Core.Output.` (trailing dot) or `import Core.Output.pr` | `printLine`, `print`, `capture` (bare-import leaves, DEC-197); for `import Core.Result.` → `Success`, `Failure` (variant imports, DEC-186) | **0 items** [Verified] | `completion/mod.rs:87-117` Ctx::Import only lists whole-module *paths* (`catalog::core_module_paths`); nothing enumerates a module's **leaf members**. Detect `prefix == "<KnownModule>."` (split at last `.`, test `module_members(head_leaf)` non-empty) and offer those leaves (natives + injected-enum variants) as import items with a `textEdit` replacing the partial leaf. Reuse `catalog::module_members` (`catalog.rs:88`). Submodule nesting already works (`import Core.Runtime.` → `Core.Runtime.Integer`) because it is a path prefix; leaf members are the gap. |
| 4 | **P1** | Prelude / injected-type member completion | `import Core.Http;` … `req.headers.` or a `Request` field; `Date`/`Instant`/`Uri` locals; `opt.` where `opt: Option<T>` | injected class members / combinators | **0 items** for injected class fields [Verified for Http]; `opt.`/`res.` covered once #1 lands (they are `Core.Option`/`Core.Result` module natives). | The catalog only sees the **user** program; injected/prelude classes (`Request`/`Response`/`Date`/`Uri`) are added by `cli::check_and_expand`'s prelude injection. Feed the injected prelude program into `class_members` (the documented follow-up in `catalog.rs:14-18` and `completion/mod.rs:12-13`). |
| 5 | **P1** | signatureHelp (parameter hints) | typing `Output.printLine(` | active-parameter popup with the native/function signature | **not implemented** — no `signatureHelpProvider` in capabilities [Verified: `mod.rs:461-462`] | Add `"signatureHelpProvider":{"triggerCharacters":["(",","]}` to `INITIALIZE_RESULT`, handle `textDocument/signatureHelp` in `Server::handle` (`mod.rs:52-85`); source signatures from `native::registry()` params + user `FunctionDecl`/`Constructor` params (already have `symbols::signature_text`). |
| 6 | **P2** | Group-import `{ … }` context | `import Core.Math.{ ab` | `abs`, `max`, … (Math members, filtered) | **dumps 55 general keywords/symbols** [Verified] | `context()` (`completion/mod.rs:157-198`) rejects the line as an import the instant it contains `{`/space/comma (the `chars().all(alphanumeric|.|_)` gate, `mod.rs:166-169`) and the member-scan then sees `ab` after a space → General. Add a group-import branch: if the line matches `import <ModulePath>.{ … <partial>` offer `module_members(module)` minus already-listed names. |
| 7 | **P2** | Wildcard-import context | `import Core.List.*` / `import X.* except {` | suggest `*` and `except`; after `except {` suggest member names | `*`/`except` never suggested; `import X.` offers only sub-paths [Verified: `import Core.List.*` → 0] | Extend Ctx::Import to also emit a `*` sentinel item and an `except` keyword item after a full module path; add an `except { … }` member branch analogous to #6. Note `except`/`*` semantics are Q-A (`examples/project/wildcard-imports/`). |
| 8 | **P2** | Member completion ignores visibility | `other.` where `other` is another package's class with `private`/`internal` members | only members visible at the call site | `catalog::collect_members` (`catalog.rs:57-78`) returns **all** members regardless of `private`/`protected`/`internal` | Thread the caller's package + access context into `class_members` and filter by `is_member_visibility`; at minimum drop `private` for non-`this` receivers. (Visibility keywords themselves ARE offered — `keywords.rs:24-37` lists `public/private/protected/internal/sealed/open/abstract` — so keyword surfacing is fine; the gap is member filtering.) |
| 9 | **P2** | No item documentation / detail | any completion item | hover-quality signature + doc on each item | `detail` is a generic literal (`"member"`, `"keyword"`, `"core module"`, `completion/mod.rs:141,262`); no `documentation`, no `completionItem/resolve` | Populate `detail` with the real signature (natives have params/ret in the registry; user symbols via `symbols::signature_text`) and add a lazy `completionItem/resolve` handler for docs. |
| 10 | **P2** | Inferred / chained receivers yield nothing | `var x = expr; x.` or `xs.map(f).` | members of the inferred type | **0 items** (conservative gate, by design) [Verified — completion/tests.rs `inferred_or_unknown_receiver_yields_nothing`] | Needs the type-checker's inferred type at the cursor, not just declared types (`scope::receiver_type_name` is declared-only, `scope.rs:218-236`). Larger follow-up: run `check` and query the expression type. Documented gate in `completion/mod.rs:10-13`. |
| 11 | **P3** | Import `textEdit` char offset is byte-based | `import Ácme.` (non-ASCII package) | UTF-16 range | `start_char = character - prefix.len()` uses **bytes** (`completion/mod.rs:95`); OK for ASCII (asserted safe in the header comment `mod.rs:80-81`) but wrong if a package name is non-ASCII | Compute the range in UTF-16 code units (chars) rather than bytes if non-ASCII package names ever become legal. Low risk today (PascalCase ASCII). |
| 12 | **P3** | Keyword completion context-insensitive | mid-expression `1 + <ctrl-space>` | value/identifier suggestions, not `class`/`trait` | all 55 keywords always appended in General ctx (`completion/mod.rs:261-263`) | Standard LSP behaviour (client prefix-filters); optionally gate structural keywords out of expression positions. |
| 13 | **P3** | No snippet / call-parens insertion | accept a function/method item | `printLine(${1})` snippet with cursor in parens | plain identifier inserted | Add `insertTextFormat:2` (Snippet) + `${…}` placeholders for callable items; optional (`.` trigger already re-requests, `mod.rs:462`). |

## Notes & non-issues

- **The reported P0 is genuinely fixed and no sibling P0 remains.** Member items insert a plain leaf
  label (`Output.` → `printLine`), which the client places after the `.` word boundary — no
  duplication. Import items now carry the path-replacing `textEdit`. I found **no other live
  insertion-corruption bug**; the remaining findings are missing-context / over-offer, not wrong-text. [Verified]
- **`.` is a completion trigger character** (`mod.rs:462` → `triggerCharacters:["."]`), so member and
  import completion re-fire after a dot. The VS Code client (`editors/vscode/extension.js`) is a thin
  `vscode-languageclient` wrapper that auto-registers from server capabilities — no client-side config
  causes the dup or would block the fixes above. [Verified]
- **Diagnostics ≡ `phg check` (DEC-252) holds.** `diagnostics_for_uri` routes through
  `cli::front_end_diagnostics` (the same front-end `phg check` runs), with the unified loader path when
  the buffer has user imports and is a real file (`mod.rs:471-494`). Hover, go-to-definition,
  references (project-wide for top-level, DEC-327), document-highlight, rename, formatting, and
  document-symbols all exist and are intuitive. The missing editor primitive is **signatureHelp**
  (finding #5). [Verified — `mod.rs:52-63`, `mod.rs:461-462`]
- **Priority for the fix pass:** #1+#2 (builtin/UFCS receiver members) is the single highest-leverage
  gap — UFCS (`guide/ufcs.phg`) is a headline feature and the most common `.` a user types after a
  variable is on a `string`/`List`/`Map`, which currently returns nothing. #3 (member/variant import
  completion) is the second, because bare imports (DEC-197/186) are the idiomatic import style and the
  path completion stops exactly at the leaf the user needs.
- **Single shared mechanism** underlies #1, #2, #4-partial, #6: the checker's UFCS first-param
  unification. A `catalog::ufcs_members(recv_ty)` built on `native::registry()` first-param types (the
  same registry the checker uses) keeps completion aligned by construction — the catalog's stated
  design goal (`catalog.rs:1-9`).
