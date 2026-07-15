# Features

A capability matrix for Phorj — what works **today** versus what is **planned**. For runnable proof
of the "today" column, see [`examples/`](examples/README.md); for the forward plan see
[ROADMAP.md](ROADMAP.md); for things that are deliberately rejected-but-clean, see
[KNOWN_ISSUES.md](KNOWN_ISSUES.md).

## Language

| Feature | Status | Notes |
|---|---|---|
| Static types: `int`, `float`, `bool`, `string` | ✅ | checked at compile time |
| Raw bytes: `bytes` + `b"…"` literals (`\xHH`) | ✅ | octet sequences distinct from UTF-8 `string`; `Core.Bytes` interop (`fromString`/`toString`/`len`/`concat`/`slice`/`find`) |
| Typed HTML: `Html`/`Attr` + `Core.Html` kernel, builders & `html"…"` sugar | ✅ | distinct from `string` (XSS-safe by construction); kernel `text` (auto-escape) / `raw` (audited trust) / `render`; builders `element` / `voidElement` / `attribute` / `booleanAttribute` / `concat` + named per-tag helpers (`div`/`p`/`a`/`ul`/`li`/`br`/`img`/…, macro-baked); `html"<h1>{name}</h1>"` literal sugar — holes escape by type unless already `Html`, desugars to kernel calls (no new `Op`) |
| Empty collection construction `new List<T>()` / `new Map<K,V>()` | ✅ | mandatory-`new`, self-typed from the type arguments (DEC-214); a bare empty `[]` is rejected with `E-EMPTY-LITERAL` (no contextual inference) — non-empty literals `[1, 2, 3]` / `["a" => 1]` are unchanged |
| Generic lists: `List<T>` + list literals | ✅ | `[1, 2, 3]` |
| Immutable-by-default bindings | ✅ | no reassignment; fresh binding instead |
| Functions + recursion | ✅ | `function f(int n): int { … }`, `main()` entry point |
| Classes + fields + methods (`this`) | ✅ | |
| Constructor promotion | ✅ | `constructor(private int total) {}` |
| Enums with payloads | ✅ | `enum Shape { Circle(float r), Rect(float w, float h) }` |
| `match` (exhaustiveness-checked) | ✅ | over enum variants |
| String interpolation | ✅ | `"area = {area(s)}"` |
| `for … in` over lists | ✅ | `for (int s in [80, 30, 55]) { … }` |
| `if` / `else`, blocks, comparison, equality, `&&`/`||`, unary | ✅ | short-circuit logical ops |
| Checked arithmetic | ✅ | int overflow & div-by-zero → clean runtime error, never a panic |
| Local type inference: `var x = …;` | ✅ | inferred from the initializer; still fully static + immutable |
| Type aliases: `type Name = T;` | ✅ | compile-time only, erased in the PHP output |
| Indexing `xs[i]` | ✅ | bounds-checked; out-of-range → clean runtime fault, never a panic |
| Integer ranges `a..b` / `a..=b` | ✅ | materialize to `List<int>`; mainly `for (int i in 0..n)` |
| Expression `if` | ✅ | `var x = if (c) { 1 } else { 2 };` (value position; `else` required) |
| Lambdas / closures | ✅ | `function(int x) => x * 2` (expression body) and `function(int x): int { … }` (statement body, `: T` required); capture enclosing locals by value; may declare a checked exception `function(int x): int throws E => …` (DEC-222) |
| First-class function values | ✅ | a bare named function is a value (`twice(3, dbl)`); function types `(int) => int` and throwing function types `(int) => int throws E` (DEC-222 — calling such a value discharges `E` at the call site, like a named `throws` fn; a non-throwing fn is substitutable where a throwing one is expected); transpile to PHP arrow fn / `function(){} use()` / first-class callable |
| `Map<K, V>` literals `[k => v]` + indexing `m[k]` | ✅ | keys are `int`/`bool`/`string`; insertion-ordered; a missing key faults cleanly; transpiles to a PHP `[k => v]` array (M-RT S3) |
| `Core.Map` query: `keys`/`values`/`has`/`size`; `Core.List` `reverse`/`sum` | ✅ | the first generic stdlib natives — type params inferred at the call site, erased to PHP `array_keys`/`array_values`/`array_key_exists`/`count`/`array_reverse`/`array_sum` (M-RT S7b) |
| `Set<T>`: `Core.Set` `of`/`contains`/`size` + algebra `union`/`intersection`/`difference`/`isSubset` | ✅ | insertion-ordered, deduped (the Map discipline); generic, erases to `array_unique`/`in_array`/`count` (M-RT S7b); see `examples/guide/set-ops.phg` |
| `Core.List` `map`/`filter`/`reduce` (higher-order) | ✅ | take a closure argument, run once per element via one shared native body (the interpreter wraps `call_closure`; the VM a re-entrant `call_closure_value` — no new `Op`); generic, erase to PHP `array_map`/`array_values(array_filter(…))`/`array_reduce` (M-RT S7b-3) |
| tuples / map iteration | 🚧 M-RT | follow-ups on the shipped generic + higher-order native path |
| `decimal` primitive (`1.50d`) | ✅ | exact decimal arithmetic, distinct from `float`; `Core.Decimal` natives (M-NUM) |
| Security stdlib: `Core.Hash` `hmac`/`equals`/`hkdf`/`pbkdf2` + `Core.Random` `secureBytes`/`secureInt` | ✅ | MAC/KDF byte-identical to PHP (RFC KATs); CSPRNG quarantined from the PHP oracle (W3-4) |
| Null safety / optionals (`T?`) | ✅ | `??`, `?.`, `if (var x = opt)`, checked `opt!`, `match` over `T?`; non-optional `T` is never null (compile-time) |
| Pipe operator `\|>` | ✅ | `x \|> f ≡ f(x)`; left-associative, lowered to a call in the parser; transpiles to a plain PHP call |
| Type test `instanceof` | ✅ | `value instanceof T` → `bool` where `T` is a class **or interface** (M-RT S2); smart-casts the operand inside `if (x instanceof T)`; transpiles to PHP `instanceof` |
| Interfaces + `implements` / `extends` | ✅ | `interface I { method sigs }`, `class C implements I, J`, `interface K extends I`; nominal subtyping (a class flows into an interface-typed slot), polymorphic calls through an interface type; transpiles to a PHP `interface`/`implements`/`extends` (M-RT S2) |
| Erased generics `<T>` on free functions | ✅ | `function id<T>(T x): T`, inferred at the call site (incl. `List<T>` and `(T) => T` parameters); no monomorphization — type params erase to PHP `mixed`/`array`/`\Closure` before any backend (M-RT S7) |
| Erased generics `<T>` on methods | ✅ | `class U { function id<T>(T x): T … }`, inferred from the call's arguments; reuses the free-function machinery, erases identically (M-RT generics-all) |
| Generic types/classes (`Box<T>`) | ✅ | `class Box<T> { … }`, `class Pair<A, B> { … }`; the type parameter is inferred at construction (`Box(7)` ⇒ `Box<int>`) and recovered at every use site (`Box(7).get()` is `int`); no monomorphization — `<T>` erases to PHP `mixed` before any backend, an instance carries no runtime type argument (`instanceof Box<int>` ≡ `instanceof Box`) (M-RT generics-all) |
| Cross-package types — unified `import Pkg.Path.Type [as A]` | ✅ | a library package exports a `class`/`enum`/`interface`; another imports it with the same `import` used for modules (the loader classifies module-vs-type by path; the old `import type` form was retired 2026-07-03 and now fails to parse); injected `Core` types follow the qualified-by-leaf discipline (`Http.Router`, enforced by `E-INJECTED-TYPE-BARE`); nominal subtyping, `instanceof`, enum `match` all cross-package; erases to namespaced PHP FQNs (M-RT) |
| Union types `A \| B` + match-over-union | ✅ | `A \| B \| C` of classes/interfaces/primitives (`int \| string`); a value of any member flows in; reach a member via `instanceof` narrowing or **type patterns** `match s { Circle c => … }` (exhaustive over the member set, no new `Op` — reuses `Op::IsInstance`); transpiles to PHP 8.0 `A\|B` (M-RT S4) |
| Intersection types `A & B` | ✅ | members are interfaces plus at most one concrete class (two distinct classes are uninhabited → `E-INTERSECT-MULTI-CLASS`); a value satisfying all members flows in, and every member's methods are in scope (member access searches all members); shared-method signatures must agree (no overloading yet → `E-INTERSECT-SIG`); no new `Op`; transpiles to PHP 8.1 `A&B` (M-RT S5) |
| Method & function overloading (`foo(int)` / `foo(string)`) | ✅ | dynamic multiple dispatch on runtime argument types (also by arity); all overloads of a name share a return type (`E-OVERLOAD-RETURN`); lowers to one dispatching PHP method/function; byte-identical interpreter ≡ VM ≡ PHP (M-RT) |
| Inheritance: `extends`, `open`/`final`, override, `abstract`, multiple parents | ✅ | final-by-default (a class/method must be `open` to extend/override); single + **multiple** inheritance with explicit `use`/rename/exclude resolution (`E-MI-CONFLICT`); `abstract` classes & methods (`E-ABSTRACT-INSTANTIATE`/`-UNIMPL`); MI lowers to PHP interface + trait decomposition (M-RT S6) |
| **Sealed hierarchies** `sealed class`/`sealed interface` | ✅ | a closed subtype set (permitted implementors/subclasses = those declared program-wide), so `match` over the sealed BASE type is exhaustiveness-checked with **no `default` catch-all** (W5-3, DEC-179); a sealed class is extensible (implies `open`); an abstract/interface base needs only its subtypes covered, a concrete sealed class is itself a member. Compile-time-only — **erases** in PHP (plain interface/class + the shared `instanceof` chain, byte-identical) |
| Exceptions: `throws` / `throw` / `try`/`catch`/`finally` + `?`-propagation, `Result<T, E>` | ✅ | checked typed exceptions (a thrown type implements the built-in `Error` marker → PHP exception); `throws A \| B` declared sets, `?` propagates them, multi-`catch` dispatch by type; `Result<T, E>` value surface; faults/panics stay uncatchable (M-faults Slice 2) |
| Mutation: reassignment, element/field/static writes, `with`, property hooks | ✅ | immutable-by-default, `mutable` opt-in; reassignment `x = e`, compound `+= … ??=`, element set `xs[i]=e`/`m[k]=e` (copy-on-write value semantics), instance fields `o.f=e` (shared-mutable handles), `static`/`static mutable` class fields, functional `obj with { … }`, PHP-8.4 property hooks — **no tracing GC** (value/handle split + COW + `Rc`/`Drop`) (M-mut) |
| Traits (`trait` + `use` in classes, conflict resolution) | ✅ | shipped construct — see `examples/guide/traits.phg`, `trait-conflicts.phg`, `examples/project/mixins/`; final disposition tracked as MASTER-PLAN §7-OPEN |
| Operator overloading | 🔲 future | not yet a user-facing surface |
| Modules / packages | ✅ M5 | multi-file projects, folder=path, cross-package `import` + aliasing, namespaced PHP, **git dependencies** (`[require]` + `phg vendor` + `phorj.lock`, offline); transitive deps next |
| Concurrency (`spawn` + channels) | ✅ | uncolored, green-threaded (`corosensei`); native-only — the PHP leg is a hard error (`E-CONCURRENCY-NO-PHP`), see `examples/guide/concurrency.phg` |
| Identifier casing (enforced) | ✅ | camelCase functions/methods/params/vars (`E-NAME-CASE`), PascalCase classes/enums/variants/type aliases (`E-TYPE-CASE`), PascalCase package/folder + import segments + `as` aliases (`E-PKG-CASE`, 1:1 to PHP namespaces); front-end-only — never affects the generated PHP |

## Backends & tooling

| Capability | Status | Command |
|---|---|---|
| Tree-walking interpreter (reference semantics) | ✅ | `phg run --tree-walker` |
| Bytecode compiler + stack VM (byte-identical) | ✅ | `phg run` |
| Backend benchmark (median-of-N, identity-gated) + memory (peak/current RSS, Linux) | ✅ | `phg benchmark` |
| Bytecode disassembler (per-function listings + descriptor tables) | ✅ | `phg disassemble` |
| Phorj → PHP transpiler (runs under real PHP) | ✅ | `phg transpile` |
| Type-check / parse / tokenize inspection | ✅ | `phg check` / `parse` / `tokenize`; `phg check --json` emits machine-readable diagnostics (stage/severity/message/line/col/code/hint) for editors/LSP |
| `--version` / `--help`, plus per-command help with examples | ✅ | `phg -v` / `-h` / `phg <cmd> --help` |
| Sharp diagnostics: caret-underlined span, did-you-mean hints, stable codes | ✅ | front-end errors |
| Diagnostic dictionary (look up a code) | ✅ | `phg explain <CODE>` |
| Program from stdin / inline / `--` | ✅ | `run -`, `run -e '…'`, `run -- <file>` |
| Vendor git dependencies (offline, lockfile-pinned) | ✅ | `phg vendor` |
| Test runner: `test "name" {}` blocks + `Core.Test` assertions (incl. `assertFaults`) | ✅ | `phg test [path…]` |
| Formatter: canonical-form, comment-preserving, meaning-preserving, **width-canonical wrapping** (100-col; wraps call/`new` args, collection & map literals, `match` arms, `.`-chains; DEC-187) | ✅ | `phg format [--check] [path… \| -]` |
| HTTP server: `handle(Request): Response` (pure Phorj) over a real socket; PHP `php -S` bridge | ✅ | `phg serve foo.phg` |
| `Core.Db`: multi-driver SQL (bundled SQLite default; Postgres `db-postgres`; MySQL/MariaDB `db-mysql`) — prepared statements, typed rows, `queryInto<T>`/`queryScalar`/`queryMap` hydration, lazy `streamInto<T>`, transactions + savepoints + retry, typed `DbError` taxonomy, `Secret` credentials, `W-SQL-INJECTION` lint | ✅ | native-only (`E-TRANSPILE-DB`, §14 LADDER); gated by `tests/db.rs` on both backends |
| `Core.Mail`: native mailer — injection-safe `Address`, chainable builder with auto-plaintext HTML alternative, CID inlines + attachments, SMTP (`Secret` auth, STARTTLS) / sendmail / file / null transports, DKIM, typed `MailError` taxonomy | ✅ | `--features mail`; native-only (`E-TRANSPILE-MAIL`); gated by `tests/mail.rs` |
| `Core.HttpClient`: sync HTTP/1.1 client — typed responses + failures, chunked bodies, redirects (303→GET), https (rustls + bundled roots), explicit timeouts, 64 MB cap, header-injection gate, URL-userinfo rejection | ✅ | `--features http-client`; native-only (`E-TRANSPILE-HTTPCLIENT`); gated by `tests/http_client.rs` fixture server |
| `Core.Fs`: typed filesystem — files + directories (recursive create, sorted `listDir`/`walk`, loud `removeDirAll`), catchable `FsError` taxonomy (`FsNotFound`/`FsPermissionDenied`/`FsDirNotEmpty`/…) | ✅ | std-only, always compiled; native-only for now (`E-TRANSPILE-FS`); gated by `tests/fs.rs` |
| Standalone executable (host) | ✅ | `phg build foo.phg` |
| Standalone executable (Linux cross + Windows) | 🔨 | `phg build --target … / --all` |
| Standalone executable (macOS) | 🔲 | reader ships; signed stub deferred to M2.5 Phase 3 |
| PHP → Phorj migration (inverse of the transpiler; best-effort draft, review required) | ✅ | `phg lift <file.php>` |
| Language server (diagnostics, hover, go-to-def, completion, symbols) + editor integrations | ✅ | `phg lsp`; clients in `editors/vscode/`, `editors/phpstorm/` |
| Debugger (interactive REPL + DAP transport) | ✅ | `phg debug [--dap]` |

## Project qualities

- **Std-first with exactly four vetted, feature-gated dependencies** — `argon2` (Argon2id),
  `regex` (`Core.Regex`), `ctrlc` (signals), `corosensei` (green threads); nothing else (see
  [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) and `docs/specs/UNIFIED-SPEC.md#external-dependency-policy`).
- **No `unsafe`** — `#![forbid(unsafe_code)]` crate-wide.
- **Never panics on input** — adversarial source *and* adversarial binaries are handled cleanly
  (invariant EV-7).
- **Differential-tested** — every example runs on both backends and must match byte-for-byte.
