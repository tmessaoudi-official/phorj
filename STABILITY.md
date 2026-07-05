# Stability tiers

Phorj's public surface is split into three tiers. The policy attached to each tier — and what changes
are allowed when — is defined in [`SEMVER.md`](SEMVER.md).

- **stable** — intended to last. In `0.x` it may still change, but only with a documented `### Breaking`
  CHANGELOG note; at `1.0` it freezes under strict SemVer. Every stable construct is exercised by the
  [conformance corpus](conformance/), so a regression (or an undocumented removal) fails CI.
- **experimental** — usable today, but the design is still settling. May change or be removed in a minor
  release (with a CHANGELOG note), and is exempt from the 1.0 freeze until it graduates to *stable*.
- **deprecated** — slated for removal. Using a deprecated stdlib symbol emits the **`W-DEPRECATED`**
  lint naming its replacement and removal version (see [`docs/DEPRECATION.md`](docs/DEPRECATION.md)).

> The tier is a statement about the *interface*, not the implementation. The `Op` set, bytecode format,
> AST, and emitted-PHP shape are always internal (see SEMVER "What compatible means").

## Language constructs

### stable
- **Modules & packages** — `package` declaration, the unified `import` (modules and types alike;
  the former `import type` form was retired 2026-07-03 and no longer parses), import aliasing
  (`as`), folder = package path, the reserved `package Main` entry.
- **Functions** — typed parameters, default parameters, return types, `void`/`never`, recursion,
  lambdas (`function(int x) => …`), first-class function values, the pipe operator `|>`.
- **Bindings & flow** — `var` local type inference, `type` aliases, immutable-by-default + `mutable`,
  `if`/`else`, expression-`if`, `for … in`, `foreach (… as …)`, `while`/`do`, C-`for`,
  `break`/`continue`, return-on-all-paths totality.
- **Types** — `int`, `float`, `bool`, `string`, `bytes`, `decimal`; `List<T>`, `Map<K,V>`, `Set<T>`,
  fixed-length `[T; N]`; optionals `T?` with `??`, `?.`, `if`-let, `opt!`; union `A | B` and
  intersection `A & B`; erased generics `<T>` on functions, methods, classes, and enums.
- **Classes & objects** — constructor promotion, methods, `this`, fields (incl. `static`), property
  hooks, visibility (`public`/`private`/`protected`), `instanceof` with smart-cast.
- **Inheritance & reuse** — `extends` (single + multiple), `open` (final-by-default), `abstract`,
  `interface` + `implements`, `trait` + `use`, method/function/static **overloading**.
- **Static methods** — `ClassName.method(...)`, inherited and trait-supplied statics, overloaded
  statics.
- **Enums & pattern matching** — variant + literal + type patterns, `match` arm guards (`when`), struct
  destructuring, let-destructuring, exhaustiveness.
- **Operators** — arithmetic (`+ - * / % **`), comparison, logical, bitwise (`& | ^ ~ << >>`), string
  interpolation/concatenation, raw strings, `"""` text blocks, ranges `a..b` / `a..=b`.
- **Errors** — checked exceptions `throws`, `throw`, `try`/`catch`/`finally`, `?`-propagation;
  uncatchable faults/panics.

### experimental
- **HTML templating** — `Core.Html` + the `html"…"` template literal (XSS-safe builders).
- **Reflection** — `Core.Reflection` (runtime kind/type queries).
- **Cast operator** — `value as Type` over the full primitive/union matrix (fallibility-typed).
- **`Secret<T>`** — the opaque, non-printable wrapper (security primitive; surface still settling).
- **HTTP router, middleware & route attributes** — the `Core.Http` `Router` (path params, literal>param
  precedence, `use` middleware, `group` sub-routers) + the `#[Route(...)]` attribute +
  `Http.autoRouter()` desugar (M6 W2 + W2-ext: path params, `use` middleware, `group` sub-routers,
  `{name:regex}` route constraints, `#[Route]` on static methods; plus the W3 concurrent server
  (`phg serve --workers N`, bounded thread pool). The web layer is largely in place; remaining work is
  refinement (optional segments, instance controllers, perf).

## Stdlib modules

### stable
`Core.Output`, `Core.Math`, `Core.String`, `Core.Bytes`, `Core.Conversion`, `Core.Decimal`, `Core.List`,
`Core.Map`, `Core.Set`, `Core.Json`, `Core.Hash`, `Core.Encoding`, `Core.Url`, `Core.Validation`,
`Core.Csv`, `Core.Random`, `Core.File`.

### experimental
`Core.Regex` (depends on the `regex` crate), `Core.Cryptography` (Argon2id; depends on `argon2`),
`Core.Reflection`, `Core.Html`, `Core.Environment`, `Core.Process`, `Core.Http` (the web layer is largely in
place; refinement ongoing).

Two further vetted dependencies power language-level surfaces rather than stdlib modules: `ctrlc`
(signal handling) and `corosensei` (green-thread concurrency — `spawn`/channels). The full
dependency set is exactly these four; see `docs/specs/UNIFIED-SPEC.md#external-dependency-policy`.

## CLI commands

### stable
`run`, `runvm`, `check`, `transpile`, `build`, `test`, `format`, `explain`, `benchmark`,
`disassemble`, `parse`, `tokenize`, plus the source forms (`<file>`, `-`/stdin, `-e`/`--eval`, `--`)
and `-h`/`-v`.

### experimental
`lift` (PHP → Phorj draft — *review required*, inherently lossy), `serve` (HTTP server),
`vendor` (git dependencies; transitive deps deferred), `lsp` (language server; the query layer is
growing), `debug` (interactive debugger REPL + DAP transport).

## deprecated

*None yet.* When a construct or stdlib symbol is deprecated it will be listed here with its replacement
and the version in which it will be removed, and its use will emit `W-DEPRECATED`.
