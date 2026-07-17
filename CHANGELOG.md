# Changelog

All notable changes to Phorj. Format follows [Keep a Changelog](https://keepachangelog.com/);
the project is pre-1.0 and unpublished, so versions track milestone progress, not a release
cadence. Milestones and their status live in `docs/MILESTONES.md`.

## [Unreleased]

### Added — DEC-281: `Core.Input` — the stdin module (Output's twin)

Piped/redirected data is finally readable: `cat file | phg run s.phg` / `phg run s.phg < file`.
Full surface (developer-ruled): `Input.readAll(): string` (lossy UTF-8) / `readAllBytes(): bytes`
(exact) / `readLine(): string?` (exactly ONE `\n`/`\r\n` terminator stripped; `null` at EOF) /
`lines(): InputLines` (a DEC-257 `Iterator<string>` — foreach-able, one line per pull) /
`isInteractive(): bool` (terminal vs pipe — the "print usage instead of hanging" guard). Impure
natives (`Core.Native.Input`, differential-quarantined like `Core.Process`; validated by
`tests/stdin.rs` on both backends under an injectable-stdin seam) but FULLY transpilable — the
PHP legs read the CLI `STDIN` (single-terminator strip via PCRE, byte-identical to the Rust leg;
verified 3-leg on a CR/LF-tricky corpus). Under `phg serve`, stdin is disabled before workers
run (web input is the `Request`): reads behave as an exhausted pipe. Import-gated
(`import Core.Input;` — nothing in the wind). Example: `cli/stdin-filter.phg`.

### Added — DEC-258: the Db column-naming COMBINED model + variant default parameters

The naming strategy is now a real VALUE fact. `naming` is a promoted field on `Database`
(`new Database(dsn, new Naming.SnakeToCamel())` sets the whole connection; the constructor
default is `new Naming.Exact()` — enabled by defaults now accepting ZERO-payload enum-variant
constructions as compile-time constants, a general DEC-249/236 extension); `prepare` copies it
onto every `Statement`, and `namingStrategy(...)` became a real copy-builder (the documented
stored-statement-reverts-to-Exact footgun is gone). Three cooperating tiers: statically-traceable
strategies (chain literal, or a connection proven immutable + literal-constructed in the same
function) are BAKED at compile time — zero runtime cost, byte-for-byte the old behavior;
untraceable ones (connection through a parameter/field/call, stored `Statement`, runtime `Naming`
value) emit BOTH baked helper variants plus a dispatcher branching on `stmt.naming` — one branch
per hydration call, never per-row string work. `E-DB-NAMING-NOT-CONST` is RETIRED — nothing is
rejected, nothing silently downgrades. Example: `db/naming.phg` (extended); tests: 4 new tiers
in `tests/db.rs` + variant-default cases in `checker/tests/default_params.rs`.

### Added — DEC-256: the Unicode string tier on `Core.String`

Two tiers, one module (developer override of the initial `Core.Unicode` split — everything
stays under `Core.String`; the transpilability boundary is per-FUNCTION, not per-module):
**transpilable** `String.codepointLength(s): int` + `String.codepoints(s): List<int>` (the
Unicode scalar-value view; `String.length` stays byte-oriented, `strlen` parity — PHP legs are
PCRE `/u` counting and a pure-PHP UTF-8 byte decode, no ini extensions); **native-only**
`String.unicodeUpper`/`unicodeLower` (full Unicode case mapping, std `char` tables) +
`String.graphemeLength`/`graphemes` (UAX #29 clusters via the vetted, feature-gated
`unicode-segmentation` crate — the `unicode` cargo feature, on by default). Calling a
native-only function transpiles to `E-TRANSPILE-UNICODE` (§14 LADDER — mbstring/intl are ini
extensions, forbidden; importing `Core.String` stays transpilable). Examples:
`guide/unicode-codepoints.phg` (three-leg) + `guide/unicode-native.phg` (run≡runvm).

### Added — DEC-242: the `Cookie` value class (`Core.Http`)

`new Cookie(name, value)` — an immutable, safe-by-default cookie value (Secure; HttpOnly;
SameSite=Lax; Path=/) with chainable copy-builders `.path(p)`, `.secure(b)`, `.httpOnly(b)`,
`.partitioned(b)` (CHIPS, opt-in) and a canonical `render()` (fixed attribute order).
`Response.withCookie` now takes a `Cookie` (BREAKING: formerly `(name, value)` strings);
`Response.withCookies(List<Cookie>)` folds a jar — one `Set-Cookie` header per entry.
`Core.SessionModule` builds its sid cookie through `Cookie` internally (`.secure(false)` —
local dev serve is plain http). `Cookie`/`SameSite` are import-gated (`import Core.Http.Cookie;`
— nothing in the wind). Example: `web/response-builders.phg` (reworked, three-leg).

### Changed — DEC-191 addendum: `#[Entry]` is import-gated

`#[Entry]` now requires `import Core.Runtime.Entry;` like every other injected symbol
("nothing in the wind" — the `#[UncheckedOverflow]` precedent; a bare `#[Entry]` is
`E-INJECTED-TYPE-BARE` with the member-import hint). Compiler-synthesized entries
(`phg test`, lifted drafts, web bridge) are zero-span-exempt; the lifter emits the import
in its drafts. Also ruled: NO manual-function-run CLI affordance — subcommand dispatch is
userland inside the one entry ("everything will be orchestrated by the Entry").

### Added — DEC-243: `String.levenshtein` + `String.similarText[Percent]`

PHP-parity string-distance natives, byte-oriented exactly like PHP's `levenshtein()` and
`similar_text()` (Wagner–Fischer / Oliver's algorithm). PHP's by-reference `$percent` out-param
becomes the honest value-returning `similarTextPercent(a, b): float` (Phorj has no by-ref
params; the PHP leg emits a pure Tier-1 IIFE — META-7 trade, disclosed). Three-leg
byte-identical incl. float formatting (`88.88888888888889`); `examples/guide/string-similarity.phg`.

### Changed — DEC-191: `#[Entry]` — attribute-declared entry points (fully breaking)

The magic `main` (CLI) and `handle` (web) names are RETIRED: a program's entries are declared by
`#[Entry]`, on a top-level function or a class `static` method, with the ROLE inferred from the
signature — `(): void`, `(): int`, `(List<string>): void|int` = the CLI entry (`phg run`);
`(Request): Response` = the web handler (`phg serve`, the respond bridge now wraps the attributed
handler by its actual path). An `int` return IS the process exit status; entries MAY declare
`throws` (supersedes the old main-no-throws rule — an escaped fault exits 1 / answers 500). One
CLI + one web entry may coexist; duplicates of a role are `E-MULTIPLE-ENTRY`; a non-role
signature is `E-ENTRY-SIG`; an instance method is `E-ENTRY-TARGET` (all in `phg explain`). The
entry's NAME is free — every backend (interpreter, VM incl. static-init preludes, transpiler
bootstrap, DAP, test runner, lifter output) resolves the attribute, never a name. Migration:
275 examples + the whole test corpus attributed (the name `main` kept for minimal diffs);
`phg lift` emits `#[Entry]` on entries it produces. FOUND ALONG THE WAY (KNOWN_ISSUES
§span-collision): a latent P1 — injected-prelude spans share the user file's span space, so
span-keyed rewrite maps can collide (reproduced as an offset-sensitive run≠runvm on
`examples/db/transaction-closure.phg`); the real fix (per-module span re-basing) is queued.

### Added — DEC-275: `E-ERROR-NAME` — throwable types must read as throwable

Any class that implements `Error` — directly, via a parent class, or via interface extends —
must be named `*Error` or `*Exception` (both accepted: `Error` matches the stdlib's own bases,
`Exception` the PHP habit). Enforced at compile time for stdlib and user code alike; the
motivating ambiguity was `catch (InvalidUrl e)` reading like a value type at every site. The
stdlib sweep renamed the 27 remaining unsuffixed condition types (mechanical stem-keeping):
`InvalidUrlError`, `HttpTimeoutError`, `TimeoutError`, `UniqueViolationError`,
`AuthFailedError`, `MailIoError`, `UriMalformedError`, the full `UriBad*Error` family, … —
native error sentinels renamed in lockstep. Self-documented via `phg explain E-ERROR-NAME`.

### Changed — DEC-276/277/278/279: the naming mega-slice (breaking renames)

Earned shortcuts expanded, everywhere: `Core.Db` → `Core.DatabaseModule` (class `Database`,
`DatabaseError`/`DatabaseStream`/`DatabaseHandle`), `Core.Fs` → `Core.FileSystemModule` (class
`FileSystem`; the error family takes the DEC-275 suffix: `FileSystemNotFoundError`,
`FileSystemPermissionDeniedError`, …), `Core.Uri`/`Session`/`Debug`/`HttpClient`/`Iterator` →
`*Module` (the namesake rule — `import Core.UriModule.Uri;` is fully explicit),
`Core.DI` → `Core.DependencyInjection`, `Core.Reflect` surface unified on `Core.Reflection`,
`HcHandle` → `HttpClientHandle`, CLI `--addr` → `--address` (old spelling = hidden alias).
The seven raw-native `*Sys` modules nest under **`Core.Native.*`** (`Core.Native.Database`,
`Core.Native.FileSystem`, …) and are **whole-module-import only** (`E-IMPORT-NATIVE-MEMBER` —
developer-ratified: raw-layer usage stays qualified and greppable); the §14 ladder gate now also
covers importing them directly (previously a silently-diverging-PHP hole). `Core.Url` merged
into the Uri module as `Uri.encodeForm/encodeComponent/decodeForm/decodeComponent`; the old
`Core.Url` paths are the first shipping `W-DEPRECATED` entries (removal 0.7.0; STABILITY.md).
Backends resolve qualified natives **import-map-first** (import aliases now work on every
backend — the transpiler ignored them before; a prelude class never leaf-captures its same-leaf
Native module). No old→new hint table (developer-ruled: everything in-repo is migrated).

### Deprecated

The four `Core.Url` natives (use `Core.UriModule` — `Uri.encodeComponent` etc.); the CLI
spelling `--addr` (use `--address`).

### Added — DEC-280: untyped foreach key/value bindings + the lift catch-up

`foreach (m as k => v)` is now legal — both bindings inferred from the Map, exactly like the
single-binding form infers its element (typed and MIXED spellings stay legal:
`foreach (m as string k => v)`). Invariant-7 hardening rode along: inferred foreach bindings
(BOTH forms — the single-binding form had the same latent gap) are now materialized into the
AST post-check (`materialize_for_binds`), so the VM compiler and the transpiler's kind analysis
see the concrete types the checker proved — an inferred `v` is a first-class arithmetic operand
(`v * 2` differential-pinned in `examples/guide/foreach.phg`).

**Lift catch-up (Invariant 17 debt):** (1) PHP 8.4 `private(set)`/`protected(set)` properties
now lift 1:1 onto the DEC-241 modifiers (bare set-visibility reads as public, PHP semantics);
the lift printer learned the modifiers too (it silently dropped them before). (2) PHP's
`foreach ($m as $k => $v)` upgrades from Tier-2-reject to Tier-1 — lifted as the new inferred
form, each such loop carrying a greppable inline review marker (developer-ruled):
`// lift: key/value types inferred — spell them out for an explicit header`. (3) The lift
printer's two-binding `For` arm no longer silently drops the value binding.

### Changed — DEC-257 slice 3: Db streams implement `Core.Iterator` (breaking reshape)

`RowStream` and `DbStream<T>` drop the nullable-pull `next(): T?` and implement the ruled
protocol: `hasNext(): bool throws DbError` (pulls one raw row ahead and caches it — the pull is
where the driver can fail) and `next(): T throws DbError` (hands over the row / hydrates it;
past the end it FAULTS "iterator exhausted" — the DEC-257 misuse contract, pinned on both
backends). Streams are now **foreach-able**: `for (Row r in stmt.stream())` and
`for (User u in stmt.streamInto<User>())` just work. Laziness is exact: hydration happens only
in `next()` — the laziness-proof test still passes unchanged. Migration:
`while (var r = s.next()?)` loops become foreach (or manual `hasNext()/next()`). Breaking,
pre-1.0, developer-ruled ("full reshape — one blessed pull protocol"). The `Core.Iterator`
registry row sits AFTER `Core.Db`'s (the injection fold resolves dependencies in row order —
documented at the row).

### Added — DEC-257 slice 2: `Core.Iterator<T>` — the pull-iteration protocol

`import Core.Iterator;` injects `interface Iterator<T> { function hasNext(): bool; function
next(): T; }` (shape developer-ruled: the two-method form makes nullable ELEMENT types fully
sound — null is a value, never a termination signal — proven live by an `Iterator<string?>`
in the guide example). Any implementor is foreach-able: the checker lowers `for (T x in it)`
into a `hasNext()/next()` while-pull BLOCK before any backend (`rewrite_foreach`), so the
interpreter, VM, and transpiled PHP run the identical loop — byte-identity by construction.
Interface-typed values iterate too (`function total(Iterator<int> it)`). Throwing iterators
auto-propagate (ruled): the loop is legal when each `hasNext`/`next` fault is caught by an
enclosing `try` OR declared by the enclosing function; otherwise a targeted `E-CALL-UNHANDLED`
at the loop site. Contract: `next()` past exhaustion is a fault ("iterator exhausted") —
foreach never triggers it. PHP leg: the injected interface emits as `Iterator_` (PHP preloads
root `Iterator`; the RoundingMode mangle precedent — PHP-only rename, stdout byte-identical,
Phorj code always says `Iterator`). The injection fold now merges `Item::Interface` (it
silently dropped interfaces before) and injected interfaces are exempt from the DEC-202
builtin-name rejection (`InterfaceDecl.injected`, mirroring enums). Db streams reshape onto
the protocol in slice 3.

### Added — DEC-257 slice 1: generic interfaces

`interface Producer<T> { function produce(): T; }` — interfaces may declare type parameters
(bounds stay parser-rejected for now). A class implements at a type (`implements Producer<int>`,
`E-TYPE-ARG-COUNT` on wrong arity) and conformance (`E-IFACE-SIG`) compares the SUBSTITUTED
signatures; a generic class implements through its own parameter (`Boxed<T> implements
Producer<T>` — the instance's argument flows through). Interface-typed values carry their
arguments: calls through `Producer<int> p` type at `int` (not the raw `T`), and assignability is
argument-invariant (`Ints implements Producer<int>` never flows into `Producer<string>`;
inherited-only generic implements falls back to the name path — documented deferral). Everything
erases before the backends, exactly like class/enum/function generics; `phg format` round-trips
the new syntax (`interface I<T>`, `implements I<int>`) idempotently. This is the prerequisite
spine for the ruled `Core.Iterator<T>` protocol (slices 2–3: foreach over iterators + Db stream
reshape). Five new checker tests + a three-leg-verified guide example.

### Changed — playground: two-pane presentation (Phorj vs PHP), honest JIT labeling

The playground's result tabs collapse from interpreter/VM/PHP to exactly two: **Phorj** (the
bytecode VM — what `phg run` executes) and **PHP** (php-wasm). The separate interpreter pane is
gone from the UI (it remains the correctness oracle in `tests/differential.rs`); the badge is now
a two-way Phorj ≡ PHP comparison. Honest labeling (developer-ruled): no "(jit)" claim in-browser —
native code generation cannot execute on wasm on either leg — with a visible note: "JIT executes
natively in the CLI — in-browser runs use the VM / php-wasm; published benchmark numbers come from
native runs." (`playground/web/{index.html,main.js,worker.js,style.css}` + README.)

### Added — DEC-250: Optional<enum> variant patterns (the DEC-183 caveat, closed)

A `match` over an optional enum (`Status?`) now accepts the enum's variant patterns directly —
no unwrap step — and is **exhaustive** once every variant AND `null` are covered (arm order is
free; `default` still covers whatever remains). Previously the checker rejected variant patterns
on a `T?` scrutinee outright and always demanded a wildcard, undermining the exhaustive-matching
flagship for the extremely common optional-enum shape (`find(id): Status?`). Checker-only change
(`src/checker/matches.rs`): the `Pattern::Variant` arm unwraps `Optional(Named(enum))`, and the
exhaustiveness pass gains an enum-optional case requiring variants + `null`. All three backends
already executed the shape correctly once admitted — byte-identical `run ≡ runvm ≡ real PHP 8.5`.
Two caveat-pinning tests flipped to capability tests; three new checker tests; guide example
`examples/guide/optional-enum-match.phg`.

### Changed — editors: grammar catch-up + vsix 0.3.3 (DEC-181 same-change rule, resynced)

The shared TextMate grammar (`editors/vscode/syntaxes/phorj.tmLanguage.json`, consumed by both
VSCode and PhpStorm) caught up with this run's syntax additions: `private(set)`/`protected(set)`
asymmetric-visibility modifiers (dedicated rule), and `foreach`/`default` keywords. Extension
version 0.3.2 → 0.3.3, vsix rebuilt. Going forward the DEC-181 editors-both-same-change rule is
a per-slice checklist item again (this batch repaid the 4-slice drift).

### Added — DEC-274: the sugar-gate discipline (settled everywhere)

Desk ruling unifying how method-position sugar is enabled, for natives and user libraries alike:
a MODULE import (`import Core.String;`) enables both `String.upperCase(s)` and `s.upperCase()`
for every function of the module (ratifying today's behavior); a FUNCTION import
(`import Core.List.reverse [as rev];`) now enables the method form too — `xs.reverse()` /
`xs.rev()` — alongside DEC-197's bare call (aliased imports match on the alias and rewrite to
the native's real name); no import compiles none of it (nothing-in-the-wind). Also confirmed:
the subject binds the FIRST parameter (extra args follow in order; chains compose), and plain
free functions remain the declaration form. cli tests pin the positive matrix and the
no-import rejection on both backends.

### Added — DEC-234: member-error namespacing (`catch (Db.Timeout e)`, `throw new Mail.TlsError(…)`)

Every injected Core module's member types are now writable module-qualified in every type
position — catch clauses, `throws` clauses, annotations — and in `new Qual.Member(…)`
construction (including when the qualifier names both the module and its main class:
`new Uri.UriMalformed(…)` routes ahead of the static-method branch only under `new`, so
`Uri.parse(…)` statics are untouched). Root cause was a hardcoded qualifier table predating the
UA-L2 registry (it knew only Http/Time/Decimal); the collapse now consults `module_of`, so new
modules get the qualified spelling for free. Bare member-imported names (`import
Core.Db.Timeout;` → `catch (Timeout e)`) remain the working alias per the ruled transition.

### Ratified — DEC-244: UFCS is the extension-method story

Developer ruling (no new syntax): phorj's existing UFCS — any in-scope free function whose first
parameter matches the receiver's type is callable in method position — IS the extension-method
feature. It already covers what PHP 8.6 still only drafts: scalar receivers (`5.doubled()`),
user-class receivers, extra arguments, and chains, all statically checked, rewritten to plain
calls before every backend, and import-gated (nothing-in-the-wind). Shipped as documentation +
goldens: `examples/guide/extension-methods.phg` (three-leg gated) + FEATURES/spec rows.

### Added — DEC-241: asymmetric visibility (`public private(set)` / `protected(set)`)

A founding-spec v0.1 promise recovered by the reopen audit: a `mutable` field, promoted
constructor parameter, or static may declare a SET visibility narrower than its read visibility —
public reads, writes only inside the owning class (`private(set)`) or the owner + subclasses
(`protected(set)`). Enforced at every write site (instance/static assignment and `with { … }`
overrides — `E-ASSIGN-SET-VISIBILITY`), validated at declaration (`mutable` required —
`E-SET-VIS-IMMUTABLE`; writes never more visible than reads — `E-SET-VIS-WIDER`), inherited with
the declaring owner preserved, and transpiled 1:1 to PHP 8.4's own asymmetric-visibility syntax
(compile-time enforcement here, PHP re-enforces at runtime for free).
`examples/guide/asymmetric-visibility.phg` gates it three-leg.

### Added — DEC-245: intersections resolve shared methods as an overload set

Member access on `A & B` now merges each method name's signatures across the members: identical
signatures dedupe, DISTINCT parameter lists coexist and dispatch through the existing overload
machinery by argument types (a class can legally implement both interfaces — the old
require-agreement rule couldn't express it). The one genuinely uninhabitable combination — same
parameters with different returns, which no class can implement and no call-site selector can
disambiguate — keeps the (narrowed) `E-INTERSECT-SIG`. Runtime dispatch is unchanged (the value
is a concrete instance). `examples/guide/intersection-overloads.phg` gates it three-leg.

### Added — DEC-249: method default parameters (+ the Db `transaction(fn, retries = 0)` surface)

Instance and static methods now take default parameter values — the DEC-236 machinery (trailing-
only, literal-only, type-assignable; the call-site fill makes every backend see full arity)
extended to method dispatch, with defaults riding the method signature so inherited methods get
them for free. A generic method may default its NON-generic params (`pick<T>(T v, int n = 2)` —
the fill appends concrete literals before inference); a default on a generic-TYPED param stays
the DEC-236 clean deferral, as does omitting defaulted args on a null-safe `?.` call. With the
language wall down, `Core.Db`'s recorded surface PENDING resolved the ambitious way:
`db.transaction(fn, int retries = 0)` is the single transaction method (run-once by default,
retry-on-`SerializationFailure` when `retries > 0`) and the stopgap `transactionRetry` is retired.

### Fixed — default-parameter fills restored stale (pre-erasure) argument subtrees

A recorded fill is a CHECK-TIME clone of the call (provided args + appended defaults). It was
applied by the LAST rewrite pass, so a lambda argument whose throws-`?` had already been erased
(or whose `new` had been unwrapped) was restored stale — `db.transaction(fn)` with a `?`-using
closure faulted at runtime. Two root fixes: fills now splice FIRST (`apply_default_fills`, a new
fixpoint pass ahead of every other rewrite, so spliced subtrees flow through the whole chain like
hand-written code), and the throws-`?` eraser now unwraps to its LIVE inner call (the recorded
entry is a marker only — splicing its stale clone was the same defect mirrored). Both directions
are locked by the db closure-transaction tests.

### Added — DEC-253: nullable unions `(A | B)?` / `A | B | null`

Both spellings are the same type (the formatter canonicalizes to `(A | B)?`; a lone non-null
remainder prints `T?`): `null` parses as a union-member marker, the checker resolves either form
to optional-of-union, and the whole optional toolkit — `??`, `?.`, if-var narrowing, `match`
with member + `null` arms — is inherited unchanged. Standalone `null` in type position is a
clean `E-NULL-TYPE` (with `phg explain` entry). The PHP emission is the native `A|B|null` union
PHP itself uses (other optionals keep their historical `mixed` fallback — a recorded
transpile-modernization follow-up). `examples/guide/nullable-unions.phg` gates all of it
three-leg.

### Fixed — statement-position `match` transpiled to unparseable PHP

A `match` used as a statement (arms run for effect: `match (e) { X() => Output.printLine(…) };`)
emitted a native `match (true) { cond => echo …, }` — but `echo` is a PHP *statement*, so the
whole emitted file was a parse error. Never caught: every differential-gated example used match
as an expression, so the PHP leg never exercised the statement form. Statement-position matches
now lower through the `instanceof`/`===` if-chain (`MatchTarget::Discard`), where statement arm
bodies are legal; pinned by a transpile regression test and the nullable-unions example.

### Added — DEC-240: `Core.Uri` — RFC 3986, typed errors, PHP-8.5 native twin

One immutable `Uri` class (`import Core.Uri.Uri;`) whose transpile twin is PHP 8.5's always-on
`Uri\Rfc3986\Uri` — full byte-identity with NO ladder quarantine:

- **Kernel** (std-only Rust, `src/native/uri/`): strict RFC 3986 parse, per-component validation
  (IPv6 + IPvFuture literals included), twin-faithful normalization (ASCII-unreserved-only
  percent-decoding with hex uppercasing; dot-segment removal that keeps an unmatched leading `..`
  only on scheme-less relative paths; `getHost` lowercases IPv6 as written while `toString`
  expands to eight 4-digit hextets; i64 port limit, zero-strip, empty-port round-trip), §5.2
  reference resolution. Every behavior probed live against php-8.5.8 and pinned by 12 kernel
  tests over the captured corpus (`docs/research/2026-07-16-uri-twin-probes.md`).
- **Surface**: `Uri.parse(s)` throws the typed `UriError` taxonomy — per-component subclasses
  (`UriBadScheme`/`UriBadHost`/`UriBadPort`/`UriPortOutOfRange`/`UriBaseNotAbsolute`/…) that beat
  PHP's single `InvalidUriException` while keeping the MESSAGES twin-identical. Normalized
  getters + the `raw*` family (as-written), `username`/`password` split, `int?` port, strict
  (non-encoding) withers returning fresh `Uri`s, `resolve(ref)`,
  `equals`/`equalsIncludingFragment` (fragment-excluded default, like the twin), `toString`
  (normalized) vs `toRawString`.
- **PHP leg**: the emitted program wraps the real extension via tiny `__phorj_uri*` helpers
  (exception → the same `<<E>>`-sentinel messages the Rust natives produce), so on PHP the
  extension IS the implementation. Three-leg byte-identity verified end-to-end;
  `examples/guide/uri.phg` is differential-gated.

### Added — DEC-239: the pipe `|>` package (PHP-8.5-aligned + phorj-only ergonomics)

The full ruled pipe package, in five slices:

- **`Expr::Pipe` is a real AST node** expanded out by `checker::lower_pipes` (the FIRST front-end
  pass — Invariant-5 discipline, like `new`/`html`/aliases; no desugar pass, checker, or backend
  ever sees it). This also fixed a formatter fidelity defect: `phg format` used to rewrite
  `x |> f` into `f(x)` because the parser lowered pipes before the printer ever saw them; pipes,
  placeholders, and pipe lambdas now round-trip verbatim.
- **Precedence fix**: `|>` moved from loosest to PHP 8.5's exact slot — tighter than comparison
  (`x |> f == 6` is now `(x |> f) == 6`), looser than shifts/arithmetic (`10 + 6 |> inc` → 17).
  Every relation was probed live against php-8.5.8 (tighter than `== < & ?? &&`, looser than
  `+ <<`); parser tests pin all seven.
- **Bare-`%` placeholder** (phorj-only — PHP cannot reposition the piped parameter):
  `x |> f(%, 2)` ≡ `f(x, 2)`, whole-argument slots of the pipe's top-level call only; several
  `%` slots evaluate the piped value ONCE (a synthesized single-evaluation IIFE with a
  collision-scanned `phorjPipe<n>` param). `f(% + 1)` / nested `g(%)` / bare `x |> %` are
  parse-time `E-PIPE-PLACEHOLDER` (span-exact, with a use-a-lambda hint + `phg explain` entry).
  Modulo is untouched — `%` is a placeholder only in operand position inside a pipe RHS.
- **Contextually-typed pipe lambda**: `x |> (v => v * 2 + 1)` — the param type flows from the
  piped value (DEC-201 contextual-typing precedent). The checker infers it at the IIFE call
  site, rejects piping `void` (`E-VOID-CAPTURE` — PHP silently coerces void→null), and the
  inferred type is materialized into the AST after checking so the VM compiler and transpiler
  specialize exactly as proved (Invariant 7; `run≡runvm` pinned by test). A pipe lambda stranded
  outside pipe application (`x |> (v => v) + 1` — the `+` binds to the lambda, uniform RHS
  grammar) is a targeted `E-PIPE-LAMBDA-CONTEXT` with a parenthesize hint. The ergonomic
  alternative (trailing tight-ops binding to the pipe result) is a recorded PENDING developer
  fork — erroring now is the additive-relaxable choice.
- **Surfaces**: `examples/guide/pipe.phg` (three-leg byte-identical, differential-gated);
  FEATURES.md row rewritten; `phg lift` now names `|>` in a clear Tier-2 rejection (it lexed
  `|` + `>` and reported "found Gt"). Compile-time single-arg arity and void-mid-chain rejection
  are pinned as recorded phorj-better divergences (PHP defers both to runtime).

### Added — DEC-222: throwing-closure function types

The closure parallel of DEC-221 (throwing constructors). A function TYPE and a lambda can now
declare a checked exception, so a closure can `throw` / `?`-propagate and a call of it discharges
the exception at the call site, exactly like a named `function … throws E`:

- **Surface**: `(int) => string throws MyError` on a function-type annotation; `function(int n):
  int throws E => …` (and the block-body form) on a lambda literal. Absent clause ⇒ non-throwing.
- **Checker**: a lambda body is checked with its DECLARED throws in context (no more forced
  `E-THROW-UNDECLARED` inside a throwing lambda); a call of a `throws E` function value routes `E`
  through the same discharge path as a named throwing call (`E-CALL-UNHANDLED` unless caught /
  `?`-propagated). No inference — a throwing lambda declares its throws, like a named fn/ctor.
- **Variance** (the sound rule): a function throwing FEWER exceptions is substitutable where one
  throwing MORE is expected — every exception `from` may throw must be `<:` some member of `to`'s
  set. So a plain `() => T` passes where `() => T throws E` is expected; the reverse is rejected.
- Checker/parser-only — no runtime change (the throw is the existing `Op::Throw`), so
  `run ≡ runvm ≡ php` stays byte-identical. Example: `examples/guide/throwing-closures.phg`.

### Added — DEC-208 slice C: closure-form transactions `db.transaction(fn)` + retry (unblocked by DEC-222)

The closure form of `Core.Db` transactions, the language dependency DEC-222 was built for:

- **Surface**: `db.transaction(function(): T throws DbError { … })` — BEGIN, run the closure,
  COMMIT on a normal return (returning the closure's VALUE), auto-ROLLBACK + **re-throw the
  ORIGINAL typed error** on a throw. A NESTED `db.transaction` opens a SAVEPOINT (composable
  partial rollback, reusing the slice-C depth). BOTH this closure form AND the manual
  `begin`/`commit`/`rollback`/`rollbackQuiet` (slice C) are supported — developer ruled BOTH.
- **Retry**: `db.transactionRetry(fn, retries)` re-runs the whole transaction on the transient
  `SerializationFailure` only; any other `DbError` (and an exhausted budget) propagates immediately.
- **Mechanism**: a `HigherOrder` native (`DbSys.transaction`) invokes the closure re-entrantly on
  the calling backend. Throw preservation is the load-bearing part — a closure throw reaches the
  native as `Err(THROW_SENTINEL)` with the thrown value in the backend's `pending_throw`;
  `rollback_inner` is pure `rusqlite` (never re-enters the backend), so `pending_throw` survives and
  returning the same `Err` unchanged lets the backend rebuild the ORIGINAL typed `DbError`. The
  retry loop lives in the PRELUDE (only phorj source can `catch` the typed error — `pending_throw`
  is invisible to a native).
- **Surface deviation (PENDING adjudication)**: the spec illustrates one method
  `db.transaction(retries: N, fn)`, but the language has no named args, no method default params, and
  no generic-method overloading — so retry is realized as a distinct `db.transactionRetry(fn,
  retries)` (developer to confirm the name/shape). Isolation levels remain deferred.
- Spine-quarantined (`Core.Db`, `pure:false`); `run ≡ runvm` holds (shared native/closure bodies).
  Example `examples/db/transaction-closure.phg`; fixtures in `tests/db.rs` (both backends).

### Added — JIT W9 + S8: the sqlbuild builder pipeline compiles end to end (borrowed-arg clone-at-boundary, Return frame teardown, deferred pad seeding, flattened JoinClause)

The whole `Core.Sql` immutable-builder shape — union Dyn wheres, joins, `toQuery()`,
`sql()`/`params()` reads, try/catch, the bench loop — now stays on the unboxed JIT path.
Four levers, each closing a fixpoint- or ownership-structural wall the sqlbuild probe
isolated:

- **W9a — borrowed handle args CLONE at the call boundary** (PHP value semantics via the
  existing `rt_u_clone_value`): every `this.field` forwarded into the next builder step
  (`new SelectQuery(this.tableName, …)`, `this.next(this.cols, …)`) was a compile-time
  BORROWED arg, denied wholesale — so no builder sig ever recorded and every ctor param
  stayed Unknown. Owned/const words still move free; maps stay Owned-only (no clone repr).
- **W9b — Return frame teardown**: `Op::Return` now releases every owned cell left below
  the (already-secured) return value — the `frag` temp in `withCond` used to force an
  "ambiguous ownership" decline (and owned temps silently leaked before). A BORROWED
  instance return keeps the exact transfer census (its single backing cell must survive).
- **W9c — deferred catch-pad seeding**: `PushHandler` no longer fails when the graph's
  thrown class is unknown — it keeps walking the try body (recording the discoveries that
  REACH the `Throw` sites, e.g. `qualify` behind the builder chain) and holds the error at
  the walk's end. Failing at the marker deadlocked the fixpoint the same way the union
  param did.
- **S8 — JoinClause flattened** (prelude): it carries the parent `SelectQuery`'s FIELDS
  (14 fields, wide two-slot instance) instead of the parent instance — an instance-kind
  ctor arg was un-analyzable and the word would dangle once the chain frees the receiver
  after `.on()`.

Also: the int-list accumulator append arm now falls through to the general clone arm for
non-int shapes (a str-list `out = append(out, q)` loop declined the whole graph);
`GetField` joins the fault-exit pre-scan (a `return this.field;` body's Return-clone had
no counted fault source — a latent `fault_if` panic these graphs exposed); borrowed
`DynList` returns clone (repr 5) and the entry decode materializes DynList returns; a
whole-graph decline now names the failing function in its message. Delivery:
`phg_run_hook_hits_the_jit_on_the_sqlbuild_builder_pipeline` (the full builder chain,
hits > 0 + byte-identity). Full oracle 1967/1967 with the PHP leg required.

### Added — JIT W-slice 7: union params as tagged two-word Dyn cells (the sqlbuild gate's last widening lever)

A declared scalar-union param (`string | int | float | bool` — the `Core.Sql` `whereEq`/
`whereGt` value shape) now stays in the unboxed JIT subset as a `Kind::Dyn` register pair:
the PAYLOAD in the I64 space, the runtime TAG in the enum-tag space (EnumInt precedent;
0 = int, 1 = float-bits, 2 = bool, 3 = str-handle). The ABI is kind-driven — a Dyn param
crosses every call as TWO i64 words, expanded by the one `pop_call_args` shared by
`Call`/`CallValue`/`CallMethod` from the same `abi_param_kinds` single source the signature
builder reads. Consumers: the tag-dispatched `rt_u_list_append_dyn` helper (a Dyn element or
`DynList` receiver → a fresh boxed `List<union>`), `List.length` (now ANY list kind and ANY
ownership — an OWNED operand is measured then freed, the `List.length(q.params())` shape),
`DynList` instance fields (ctor stores, borrow reads, kinded release), and `DynList`
call-arg moves / clone-returns. Dyn cells are MOVE-ONLY (a borrowed copy would alias the
owned str payload — double free); multi-use / `SetLocal` / `Pop` / `Return` of a Dyn stay
fail-closed declines.

The load-bearing piece is the **declared-union seed**: the compiler stamps
`Function::dyn_params` (a checker fact — which param slots are scalar-only unions) and the
fixpoint seeds those params `Dyn` directly. Without it the sqlbuild chain DEADLOCKS: a
mid-chain method that both takes and appends the union param (`withCond`) can never finish
its round-1 walk, so its return kind never lands, so the later chain sites that would
contribute the other scalar family to the join are never reached — call-site discovery
alone cannot see the union.

Two latent object-vertical bugs found by the W7 audit are fixed in the same change:
a LIST/map field read off a DYING owned temp (`new P(..).cols`) TAKES the word but the
receiver's field-release walk only excluded `Str` fields — the taken word was freed under
the reader (recycled-slot reuse could hand the consumer a DIFFERENT live value: wrong
bytes, not a redo); and `str_field_layout_slots` did not list `DynList` fields (an instance
owning a `List<union>` leaked it on death). Emit↔analyze mirror drifts closed: `GetLocal`'s
movable set (DynList), `arm_list_len`'s accepted kinds, `SetField`'s value gate.

Delivery: `phg_run_hook_hits_the_jit_on_union_dyn_params` (Int/Str/Bool sites → genuine
Dyn; appends through a `List<union>` field across a temp-receiver builder chain; hits > 0 +
byte-identity over 2000 iterations) + `phg_run_hook_takes_list_fields_from_dying_temp_receivers`
(the take-and-skip regression). Full oracle 1966/1966 with the PHP leg required.

### Added — forin lever-3 pointer-walk iteration — **0.73× → 2.30× WIN** (protocol median, 3× best-of-7)

The for-in desugar's harness cells become RAW POINTERS at emit: at the `IterElems; Const(0)`
init site over a runtime-FLAT int list, the elems cell becomes the END pointer
(`Kind::IterEnd`) and the j cell the element CURSOR (`Kind::IterPtr`) — every harness op then
strength-reduces per-op with NO region rewriting: `Len` = identity re-push (the pointer IS the
bound), the header `Lt` = ONE unsigned compare, `xs[j]` = ONE load (the loop guard is the
bounds proof), `j + 1` = `+64` (the slot stride; the analyze mirror verifies the increment
literal is exactly 1). Generic arith/comparison arms explicitly REJECT iter kinds, so a
desugar drift can never leak pointer math into user-visible values. **MUTATION GUARD** (also
closes a latent byte-identity hazard the ACL builders introduced): a slot that feeds
`IterElems` must never be written in the same function — the VM's for-in iterates a SNAPSHOT,
while an in-place ACL append/reseed would grow or recycle the record UNDER the walker; any
overlap declines the whole function to the VM (test proves the decline + byte-identity). The
guard also implies an iterated slot can never hold an ACL at runtime, so the walk is flat-only
(boxed → code-5 VM redo, disclosed). forin **0.73× → 2.30×** (rounds 2.30/2.82/1.66 vs pinned
fresh docker php:8.5-cli+JIT); delivery-path test proves `hits > 0` + byte-identity; baseline
ratcheted. **ALL FOUR fundamentals-sweep losses are now WINs** (listappend 1.66 · mapinsert
1.06 · hofpipe 6.46 · forin 2.30) — 21/21 micros ≥ 1.0×.

### Added — hofpipe capturing-closure + HOF-loop vertical — **0.19× → 6.46× WIN** (protocol median, 3× best-of-7)

Higher-order pipelines enter the unboxed JIT. Two pieces: (1) **`Kind::FnCap1`** — a
ONE-int-capture lambda whose stack cell IS the capture word (`MakeClosure` pops one capture and
re-tags it in place at the same depth: no closure object, no aux register space, zero
allocation); consumers direct-call the target with the capture PREPENDED as arg 0 — the VM's
`[caps.., args..]` lambda frame (a lambda's `arity` already folds captures in, so signatures
need no adjustment). (2) **HOF loop arms** — `List.map`/`List.count` with a static `Fn`/`FnCap1`
lower to ONE native loop: a uniform `(addr, stride)` walk over the input (flat list 64-byte
slots / ACL builder packed i64s; boxed → code-5 VM redo, the disclosed v1 gap), a direct call
per element, and map → an ACL builder output (inline cap-checked pushes) / count → a register
sum of the 0/1 predicate results. **Bool returns** joined the subset (`ret_kind` records Bool,
`run_unboxed` decodes `Value::Bool`) — the count predicate's shape; unproven-param returns stay
rejected. Throwing graphs keep HOFs on the VM (fail closed); analyze mirrors every arm.
hofpipe **0.19× → 6.46×** (rounds 6.59/6.46/6.46 vs pinned fresh docker php:8.5-cli+JIT —
zend's `array_map` allocates a closure + array per iteration, the JIT loop allocates nothing);
delivery-path test proves `hits > 0` + byte-identity with a live varying capture; baseline
ratcheted.

### Added — mapinsert AMB map-builder vertical — **0.02× → 1.06× WIN** (protocol median, 3× best-of-7)

`m[k] = v` (`Op::SetIndexLocal`) on a uniquely-owned `Map<string,int>` local enters the unboxed
JIT: the first write CONVERTS the sealed flat map into an **AMB builder record** (`UB_TAG_AMB`,
shared record pool; layout `[log2][count][packed {canon,value} table][rank canons]` — ranks keep
PHP's insertion order, overwrite keeps the original rank). The write is FULLY INLINE for
canonized slot keys: packed-table probe walk (the mapget shape) → HIT = one value-word store;
EMPTY at load ≤ 1/2 with rank capacity = **inline INSERT** (entry + rank + count++, four
stores — zend-hash add). `rt_u_map_builder_set` is the one slow leg (conversion, canon-0 keys,
growth-rebuild); `rt_u_map_get` gained an AMB arm and `arm_index_map` an inline AMB read leg
(same probe over the record table). Aliasing is impossible in the subset (SetLocal of borrowed
handles stays denied), so in-place mutation matches the VM's `Rc::make_mut` refcount-1 COW path
byte-for-byte; analyze mirrors every arm fail-closed. **BUILDER-RESEED peephole** (both
verticals): `m = [k => v]` / `xs = [v]` literal RESETS over a live builder slot reuse a record
via `rt_u_map_builder_seed` / `rt_u_list_acc_reseed` instead of bump-sealing — without it every
reset leaked 1–3 never-recycled arena slots and a 1M-iteration run walked off the 4096-slot
arena into a permanent code-5 VM redo (mapinsert's observed 1M cliff; listappend was at 95%
arena — ~4M iters from the same cliff). mapinsert **0.02× → 1.06×** (rounds 1.06/1.06/1.10 vs
pinned fresh docker php:8.5-cli+JIT); listappend re-verified 1.68/1.65/1.68; delivery-path test
proves `hits > 0` + byte-identity across reset cycles; baseline ratcheted.

### Added — listappend ACC-list-builder vertical — **0.01× → 1.66× WIN** (protocol median, 3× best-of-7)

The strbuild ACC recipe applied to collection writes: at a proven `accumulator_site`
(`xs = List.append(xs, v)` — the lhs is the dying borrow of the very slot the following
`SetLocal` rewrites, so the pure-append clone is unobservable), the unboxed JIT consumes the
list into an **ACL builder record** (`UB_TAG_ACL`, same `{ptr,len,cap}` record pool as the
string ACC; elements are consecutive raw i64s) and pushes IN PLACE: inline cap-check + ONE
8-byte store + len bump — php's `$xs[] =`. `rt_u_list_acc_append` is the one slow leg
(first-append conversion from a flat/boxed list, capacity growth, table exhaustion → code-5
VM redo). `List.length` (`arm_list_len`) gained an inline ACL len-word read (the
every-iteration `>= 256` reset probe costs one load), `rt_u_index_int` an ACL bounds+load arm
(`xs[0]`/`xs[255]`), and the release ladders recycle the record while KEEPING its grown buffer
across `xs = [0]` resets (php's buffer-reuse trick — the same `UbCtx::release` ladder as ACC).
Analyze mirrors every arm fail-closed (`List.length` borrowed-only; `List.append` only at
accumulator sites — anywhere else stays on the VM so clone semantics remain observable).
Delivery-path test proves `hits > 0` + byte-identity on the exact micro shape across several
reset cycles. listappend **0.01× → 1.66×** (self-timed 673M → 2.35M ns; rounds 1.69/1.66/1.62
vs pinned fresh docker php:8.5-cli+JIT); baseline ratcheted.

### Added — Fundamentals sweep + for-in vertical + task-9 v2 — **forin 0.01× → 0.73×, listindex → 1.61×**

The coverage-driven sweep added four MACRO-realistic micros (21 total) and found four VM-bound
catastrophic losses: **listappend 0.01×** (immutable `List.append` clones the whole list per
call), **forin 0.01×** (the for-in desugar = `IterElems` + an indexed while — ~13 VM-dispatch
ops per element), **mapinsert 0.03×**, **hofpipe 0.19×** (none of those shapes were in the
unboxed subset). Two slices shipped against them: (1) **for-in in the unboxed JIT** —
`IterElems` on a borrowed flat list is an IDENTITY re-push (sealed lists are immutable within
the subset; zero instructions) and `Len` reads the element count from the handle bits (helper
for boxed lists). (2) **Task-9 v2** — the interval pass admits NESTED counted loops: inner
`j < T` guards where `T` is a const or the `Len` of a compile-time-known collection, counters
pinned to `[0, T]` (refined to `[0, T-1]` between the passed guard and the increment), site
growth multiplied by the enclosing trip counts, the outer counter self-proven by shape — and
**in-bounds `Index` elision**: an index interval provably inside `[0, len)` drops the bounds
branch at emit. forin fell 172 → ~2.4 ns/element (0.73×; the documented next lever is
strength-reduced pointer-bump flat iteration); listindex rides the bounds elision to 1.61×;
every prior WIN holds. Also recorded (KNOWN_ISSUES, pending adjudication): empty collection
literals take no contextual type and no `List.empty()`/`Map.empty()` constructors exist.

### Added — Task 9: accumulator overflow-check elision — **ALL 17 micros now ≥ 1.0× vs php+JIT**

The checked-add price (the measured single root cause of the last three losses) is gone where
it can be PROVEN gone: a new fail-closed interval pass (`src/jit/range_acc.rs`) analyzes a
counted loop in i128 and elides the `*_overflow` + sticky accumulation for every
`AddI`/`SubI`/`MulI` whose result provably fits i64 — bounded ACCUMULATOR chains
(`acc = acc + m[k] + xs[idx]` — growth tracked through the chain to the `SetLocal`),
counter-AFFINE terms (`i * 3 - 1`), and expression-dividend `RemI`-by-pow2 (provably
non-negative → the single `band`). Trip count and counter ride a bound `G`: a const loop bound
is exact; a never-written PARAM bound gets an ENTRY GUARD (`param > G` → code-5 decline, the
VM runs that call unspecialized — `G` from a `2^31 → 2^24 → 2^20` ladder, largest that
verifies). Accumulator envelopes are `acc0 + G·envelope` (envelope includes 0); an
env-stability second walk rejects hidden growing slots; every out-of-scope shape (computed
bounds, body branches, unknown ops) keeps full checking. When everything speculated is proven,
the sticky variable itself disappears — the intadd endgame. Fault behavior is unchanged by
construction (elision only where overflow is impossible; declines redo on the VM, which
faults canonically — covered by a genuine-overflow parity test).

**Measured (exit-bar protocol, 3 × best-of-7, pinned, interleaved, fresh docker
php:8.5-cli+JIT):** intadd 0.68 → **1.48× WIN** (checked-default now BEATS php's unchecked
adds) · mapget 0.88 → **1.01× WIN** · listindex 0.97 → **1.47× WIN**. With floatmul (1.00)
and floatloop (1.01) medians holding, **every one of the 17 micros meets the
beat-or-match bar — the PERF-100% flip phase is complete.** Five new tests cover the proofs,
the guard-decline path, the rejection shapes, and overflow-fault parity.

### Changed — Ω-8 vertical: packed flat-map buckets — mapget 0.82× → 0.88×, residue measured

The flat-map bucket table now stores PACKED 16-byte `{canon: u64, value: i64}` entries
(canon 0 = empty — a real canon is never 0) instead of u32 pair indices: a probe hit is the
canon compare plus one ADJACENT value load (one cache line), where the old walk chased a
3-deep dependent chain (bucket u32 → pair-slot canon → value slot). Seal writes the packed
table; the helper's linear pair walk is unchanged. Measured (3 × best-of-7 protocol):
**mapget 0.82 → 0.88/0.89/0.88 — consistent +7%, still short of the bar.** The remaining gap
is now precisely accounted for: an isolation run (`#[UncheckedOverflow]` variant, pinned,
interleaved) puts the loop's two checked int-adds at **1.5M ns of the 11.9M VM leg — removing
them lands within noise of php's 10.5M**. Verdict: the probe levers are exhausted (bucket+canon
interning → fused tag check → packed buckets); the mapget/listindex (0.97) tail is the
checked-add price, and task 9 (range-proof overflow-check elision, ruled ACTIVE) is the
closing lever for both plus intadd itself.

### Added — Ω-8 vertical: ACC-record string accumulator — **strbuild 0.42× → 2.27× WIN**

The classic `s = s + x` accumulator (templating, log lines, serialization — the pattern where
php's refcount-1 in-place append historically dominates) now runs on a php-smart_str-analog
**accumulator record**: a new `UB_TAG_ACC` handle indexes a JIT-visible `{ptr,len,cap}` record
table (`UbCtx` header offset 40, 16 pre-allocated records), and the proven `accumulator_site`
peephole emits a fully-inline append — load the record, cap-check, ONE bounded 3×8-byte copy at
`ptr+len`, store the new length; no call. The `rt_u_acc_append` helper is the slow leg only:
first-append conversion (fn entry / after every `s = ""` reset — where a recycled record
REUSES its grown buffer, php's capacity trick), doubling growth, and non-slot rhs; record
exhaustion falls back to the plain concat path. `String.length` on a borrowed accumulator
reads the record's len word inline (the `> 512` reset probe costs one load). The ACC tag
deliberately omits `UB_TAG_OWNED` so the existing release ladders route it to the helper,
which recycles the record and keeps the buffer. `emit_unboxed/concat.rs` split out of
`verticals.rs` (M-Decomp, both files back under the cap). New JIT test pins exact accumulated
bytes via a map probe plus reset/growth cycles, hits>0.

**Measured (exit-bar protocol, 3 × best-of-7, pinned, interleaved, fresh docker
php:8.5-cli+JIT):** strbuild medians 2.22/2.27/2.30 → **2.27× WIN** (was 0.42; VM leg 56M →
9.5M ns). No regressions — webish 2.13 · interp 2.54 · stringconcat 1.9 · trycatch 34 hold;
floatloop's 1.01 median now ratchet-protected; floatmul's noisy 0.93 emit sample aligned to
its 1.01 protocol median.

### Added — Ω-8 vertical: fully-inline mixed interpolation — **webish 0.68× → 2.24× WIN, interp → 2.65×**

The fused `rt_u_concat_mix` helper call (one C call per interpolation) is replaced, for the hot
shape, by pure Cranelift IR: every `Str` part slot-tagged (one AND + branch over the handles)
and a ≤22-byte total build the result entirely inline. Each `Int` part renders backward into a
private 48-byte stack scratch — the exact `as_display` decimal bytes, with a branchless sign
(the '-' is always stored at the byte before the digits and only enters the piece when the
start steps back over it; `i64::MIN` renders correctly via `ineg`'s wrap) — then all parts join
into a fresh arena slot with bounded 3×8-byte over-copies at a running cursor, hash+canon
zeroed after (the same "punt" marker the helper writes, so bytes AND metadata are identical).
Untagged (heap) parts or >22-byte totals still take the one fused helper call. New JIT test
proves hits>0 and exact rendered bytes through a map probe (a wrong render would miss the key
on the JIT leg only), covering sign/zero/`i64::MIN`/`MAX` and both paths in one loop.

**Measured (exit-bar protocol: 3 × best-of-7, pinned, interleaved, fresh docker
php:8.5-cli+JIT):** webish medians 2.37/2.31/2.22 → **2.31× WIN** (was 0.68), interp
2.59/2.80/2.98 → **2.80× WIN** (was 1.03); no regressions (stringconcat 1.94, trycatch 32.5,
mapget 0.87, strbuild 0.42). Ratchet re-emitted; two noisy snapshot entries were aligned to the
protocol medians rather than trusted (strbuild's lucky 1.08 → 0.42 to avoid arming a phantom
flip-block; floatmul's 0.985 → 1.00 to keep the won parity protected). Also fixed the two
clippy errors the trycatch commit left (pre-commit runs no clippy; pre-push does).

### Added — Ω-8 vertical: native throw/catch in the unboxed JIT — **trycatch 0.37× → 33.4× WIN**

Try/catch is now compiled natively in the unboxed JIT, in three gated sub-slices. (1) **Str
fields in instances**: a per-class field-kind table joins the fixpoint (derived from
`MakeInstance` operand kinds; all sites must agree, Int|Str only); `GetField` of a Str field
yields a borrowed handle (the instance keeps ownership), `SetField` releases the old field word
first, and instance release is kind-directed — str-fielded classes free their owned field words
before the slot is recycled (the runtime OWNED bit makes const-field frees no-ops). (2) **String
ctor args**: Str arguments (Owned/ConstBorrow) may cross into instance-returning callees — a
unique `GetLocal` transfers ownership (the slot dies), call sites inject a per-fn `param_over`
kind-override table, and analysis facts now flow out through a `UbDiscovery` out-param so they
survive held failures, breaking the caller/ctor fixpoint deadlock. A str-fielded
construct+method loop dropped 847M → 15.5M ns (**55×**). (3) **Native throw/catch**: thrown
values ride the existing (value, code) multi-return as **code 6** with the payload handle;
try-regions are compile-time `handler_ranges` walked lexically by analysis (catch pads become
edges in `reachable`/leaders); a throw with an active local handler truncates the compile-time
stack to the handler height (releasing dropped OWNED cells) and jumps to the pad — no ABI
crossing; without one it returns code 6, which propagates through the existing fault-exit
forwarding (VM boundary = redo, preserving escape semantics). Calls inside a try dispatch
3-way (continue / jump-to-pad / fault-exit), and the pad's `IsInstance` is kind-static so it
constant-folds away.

**Measured (pinned, interleaved, fresh docker php:8.5-cli+JIT):** trycatch 906M → 11.8M ns
self-timed — **0.37× LOSS → 29.97× WIN**, ratcheted at **33.39×**. Full map after: **11 WINs /
17 micros** (interp also flipped to 1.03× WIN); remaining losses strbuild 0.43 · webish 0.68 ·
intadd 0.73 (checked-default price; unchecked = WON) · mapget 0.80.

### Added — Ω-8 unboxed verticals waves 1–3: enums, closures, objects, mixed concat, coverage micros

The session-3 verticals that took the map from 5 to 9 WINs, all default-deny with VM fallback
and byte-identity preserved. **Enums**: `Kind::EnumInt` register pairs (payload word + a tag in
`evars` space) make `MakeEnum`/`MatchTag`/`GetEnumField(0)` zero-alloc; `Fault` is a terminator
in `reachable` — enum 0.01× → 1.7× WIN. **Closures**: capture-free `MakeClosure` is fully
static (`Kind::Fn(target)`), `CallValue` becomes a direct call — closurecall 0.03× → 2× WIN.
**Objects**: flat-arena instances (`Kind::Inst(class)`, fields at fixed slot offsets, static
method dispatch with `this` as arg 0, ctor ownership-transfer return) resolved through a
`resolve_unboxed_graph` fixpoint — methodcall 0.03× → 2.8× WIN, objalloc 0.14× → 9× WIN.
**Mixed concat**: `Concat(n)` accepts Int operands via `rt_u_int_to_str` rendering and a fused
zero-alloc `rt_u_concat_mix` (one call, stack-joined parts) — interp 0.11× → parity-then-WIN,
webish 0.05× → 0.68. **Coverage wave**: exact float-comparison lowering
(`partial_cmp`/`eq_val` ↔ FloatCC), handle-slot writes (`Own::ConstBorrow` + leader joins), and
a fused string-accumulator peephole (positional `accumulator_site` proof → in-place
`rt_u_concat` append on a uniquely-owned heap lhs) + two new base micros, floatloop (1.0× WIN)
and strbuild (0.11 → 0.43). Perf lesson recorded: hot-path result slots write hash 0/canon 0 —
canon registration only pays where content gets probed. Alongside: P-2c emit-quality levers
(fused map tag checks, single-branch Pop release, int-list vertical `Kind::IntList` flat i64
slots — listindex 0.03× → 0.95, inline `Conversion.toFloat`/`truncate` — floatarith 0.03× →
4.2× WIN, range-proven RemI-by-pow2), and the perf-gate fix that un-phantomed measurement:
microbench sampling is now **interleaved + core-pinned** (batched sampling had manufactured a
5.4× phantom flip under ambient load).

### Changed — M-Decomp repo-wide sweep + MSRV 1.82

Every source file over the 800-line soft cap was decomposed by cohesion (M-Decomp pattern:
`foo/mod.rs` + sub-files, `pub(super)` for moved methods) — ~30 splits across jit, compiler,
checker, ast, parser, lift, native, serve, chunk, transpile, interpreter — leaving only 4
by-design exceptions (explain, emit_unboxed dispatch, runtime_php, vm exec_op). One regression
caught and restored in-sweep: the interpreter's `#[cfg(test)]` module dropped by a split.
MSRV raised 1.74 → 1.82 (`Option::is_none_or` usage made it real; `rust-version` now matches).

### Added — P-2a-inline: SSO string ops inline in Cranelift IR — **beats php+JIT 1.71× (gate-2 WIN)**

The P-2a spike's verdict (helper-call granularity ~2× short of php) is resolved: the string hot
paths are now emitted **inline** in the unboxed JIT. `UbCtx` became `#[repr(C)]` with a
JIT-visible header (arena base, free-stack base, free-top, bump, cap at fixed offsets) over an
arena of **64-byte string slots** (`len:u8` + ≤22 data + slack so bounded 3×8-byte over-copies
never cross a neighbour). Handles gained runtime tags: `SLOT` (arena index; `OWNED` marks it
recyclable), `FLAT` (a `MakeList`-sealed list of all-short strings flattened into consecutive
slots), untagged (boxed `Value` — long consts, heap results). Inline fast paths: `Index` on a
flat list = unsigned bounds check + base+idx (zero copy, borrowed slot); `Concat` = len loads,
≤22 check, inline free-stack alloc, bounded copies; `String.length` = one byte load; free =
free-stack push. Every op keeps a helper slow path (untagged operands, >22-byte results,
non-flat lists), short string consts are pre-seeded as pinned arena slots, and arena exhaustion
funnels to code 5 (redo on VM) — the side-effect-free fallback invariant is untouched.

**Measured (gate-2, interleaved best-of-7, fresh docker php:8.5-cli+JIT):** real `phg run`
stringconcat **20.9M ns vs php 35.8M ns = 1.71× WIN** (ceiling spike predicted 1.74×). The
journey: 948M (pre-P-1a VM) → 739M (P-1a PhStr) → ~130M (P-2a helpers) → **~19-21M (inline)**,
checksum-identical throughout; full gate green (1928 tests, PHP oracle). Per the 2026-07-11
ruling, the gate-2 WIN unlocks P-2b (mapget vertical) and P-2c (default-deny rollout).

### Added — P-2a: JIT handle-space string vertical (spike; measured, FLAGGED vs php+JIT)

The unboxed JIT gains a **handle space**: `Kind::Str`/`Kind::StrList` operands are `i64` indices
into a per-run `UbCtx` table (pinned interned consts + free-list-recycled temps, so a hot loop's
steady state allocates nothing), with compile-time ownership (Owned/Borrowed — part of the leader
consistency check, so a merge-edge mismatch falls back to the VM rather than double-freeing).
New default-deny verticals: `Const(Str)` (a pinned-handle `iconst`, zero calls), `MakeList` of
strings, list `Index` (VM-exact bounds; out-of-range → code 5 → the VM redo renders the canonical
fault), `Concat(2)` through the single-sourced `PhStr::concat` kernel, `Core.String.length`, and
`Pop`. The unboxed ABI gains a leading `ctx` pointer (null for pure-numeric graphs); the unboxed
module now compiles at `opt_level=speed`. `stringconcat.bench()` is JIT-eligible — proven `hits>0`
plus long/multibyte and fault-path oracle tests. Handle ops mutate only the private per-run table,
preserving the side-effect-free fault-redo invariant.

**Measured (gate-2, interleaved, fresh docker php:8.5-cli+JIT):** real `phg run` stringconcat
self-timed 948M (pre-P-1a) → ~130M ns (≈7×), but php sits at ~34M — **LOSS 0.28×, flagged**.
Verdict recorded in MASTER-PLAN Ω-8: helper-call granularity (~5 calls/iter) has a ~25-30ns/iter
floor vs php's ~17; the WIN requires inlining the SSO concat fast path in Cranelift IR
(P-2a-inline). P-2b/P-2c stay gated until that WIN, per the 2026-07-11 ruling.

### Changed — P-1a: `PhStr` string value representation (SSO + cached hash; perf build, front of Fable run)

`Value::Str` (and `HKey::Str`) moved from `String` to the new `crate::phstr::PhStr` — a 24-byte
two-variant representation (`Value` stays 32 bytes, statically asserted): `Inline{len,buf[22]}`
holds runtime-built strings ≤ 22 bytes with **zero heap traffic** (short-string concat allocates
nothing), and `Heap(Rc<HeapStr{hash:Cell<u64>,s}>)` shares literals/long strings with a
**lazily-cached FNV-1a hash** (the zend_string trick). Compiler const-pool literals, `match`
pattern literals, and the const-folder intern via `PhStr::literal` (heap + precomputed hash), so
every occurrence of a literal clones by refcount bump and a map lookup by a literal key never
re-hashes. `string + string` routes through the single-sourced `PhStr::concat` kernel in both
backends (Invariant 4), with a two-`Str` fast path in the VM's `Op::Concat`. Equality/ordering are
byte-wise (≡ code-point order for UTF-8), `String.length` stays byte-length, `Display`/`Debug`
render exactly like `String`, and all fault strings are unchanged — **byte-identity holds**: the
full gate is green with the PHP oracle required (1925 tests, 28 suites). Measured (interleaved
before/after, best-of-7, release): `stringconcat` **1.28×**, `mapget` **1.19×**, `webish` 1.08×,
`interp` 1.07× — no micro regressed; `fibrec` JIT WIN vs docker php+JIT intact (1.59×). The
php+JIT beat on string/collection micros is P-2a's gate (JIT handle-space helper ops), for which
this representation is the prerequisite.

### Changed — UA-L2: injected-prelude → `Core.*` registry unification (Wave D, step 1)

The eight chained `inject_*_prelude` functions and the hand-synced `enforce_injected::module_of`
match now derive from a single data-driven registry, `cli::CORE_MODULES` — one row per virtual Core
module (`{ module, qualifier, src, respond_bridge, member_gated, bare_types }`). A new
`inject_core_modules` fold replaces the former eight-call chain in `check_and_expand_reified`, and
`checker::enforce_injected::module_of` delegates to a registry-derived `cli::core_module_of`. Adding a
Core module (the upcoming `Core.Db`/HTTP expansions) is now **one table row**, not edits scattered
across four hand-synced places. Prepares the registry before the DB/HTTP waves multiply it (RULED
B2-2; depth = registry-unification, developer-ruled 2026-07-10; full loader-unification deferred).

**Byte-identical by construction and by proof.** The row schema keeps two concerns separate: the
shadow-check names come from the parsed prelude source (so a user's own `DateTime`/`Json`/… still
shadows), while `module_of`'s `bare_types` are seeded EXPLICITLY to the pre-UA-L2 set (`Core.Time`
excludes `DateTime`; single-type value modules `Json`/`Option`/`Result`/`Regex`/`Secret` carry none).
Registry order matches the old chain exactly (load-bearing: `HTTP_PRELUDE` transitively
`import Core.Regex`, and Http runs before Regex). Verified by a throwaway corpus-equivalence test
asserting `old_chain(prog) ≡ inject_core_modules(prog)` structurally (item order + spans) over the
whole example corpus, then cut over and deleted; the differential harness is the ongoing guard. No new
`Op`/`Value`, no backend change. Gate green: 1585 unit + 144 differential (run≡runvm≡php-8.5.8) +
clippy (both feature configs) + fmt + release build.

**Discovered + disclosed** (KNOWN_ISSUES, separate adjudication): bare `Core.Time.DateTime` is not
import-gated by the injected-type discipline while its siblings `Date`/`Duration`/`Instant` are — a
latent inconsistency, preserved byte-identically here.

### Changed — `src/native/text.rs` split (M-Decomp, Invariant 13)

The `String.format` renderer cluster (`FormatDirective`, `parse_format_directive`, `pad_format`, the
`%g` helpers, `format_g_body`, `text_format`) moved out of the over-cap `text.rs` (1185 lines) into a
sibling module `src/native/text_format.rs` (with its tests in `text_format_tests.rs`). `text.rs` drops
to 824 lines. Pure structural refactor — zero behavior change, gate identical.

### Added — `String.format` positional args `%N$` (slice 4b — Wave C complete)

`%N$` selects value N (1-based), so a template can reorder and reuse values (`%2$s %1$s`, `%1$s %1$s`) —
the i18n case. Positional composes with flags/width/precision (`%1$05d`, `%2$-6.3s`). Developer-ruled
strict semantics (Invariant 15): unlike PHP, Phorj rejects mixing positional with sequential directives
(`E-FORMAT-MIXED-POSITIONAL`), faults on a value that is never referenced (`E-FORMAT-ARG-COUNT`), and
faults on an out-of-range or zero index — matching Phorj's existing exact-count strictness. The argnum
prefix parses via a cloned-iterator lookahead (digits followed by `$`, else they are flags/width). The
renderer, the transpiled PHP mirror `__phorj_format`, and the compile-time checker gate all enforce the
same rules, so `run`/`runvm`/PHP stay byte-identical. This completes the Wave C `String.format` conversion
set (`%s %d %f %e %E %g %G %x %X %o %b %%` + flags/width/precision/positional); the `%c` char conversion
and radix precision remain. No new `Op`/`Value`.

### Added — `String.format` precision on `%s` (slice 4a)

`%.Ns` now truncates a string to N characters, and width composes (`%8.3s` truncates then pads). Unlike
PHP `sprintf`, which byte-truncates and can split a multi-byte UTF-8 char into mojibake, Phorj truncates
at char boundaries (≤N bytes, never splitting a char) — a developer-ruled legibility choice (Invariant 15)
that all three backends honor identically, so `run`/`runvm`/transpiled-PHP stay byte-identical (the PHP
helper `__phorj_format` char-truncates too rather than delegating to `sprintf`'s byte truncation). This is
byte-identical to PHP's native `sprintf` for ASCII; on multibyte it is a documented LADDER divergence.
Precision on `%d` is **deliberately rejected** (`E-FORMAT-UNSUPPORTED`): PHP silently ignores it, which is
exactly the surprise Phorj's strict renderer removes. `%N$` positional args are slice 4b.

### Added — `String.format` shortest-repr `%g`/`%G` (slice 3c)

`String.format` now supports `%g`/`%G` (int/float operand), with a `.precision` (significant digits,
default 6). The renderer reproduces PHP `sprintf`'s C-printf `%g` byte-for-byte: round `|f|` to P
significant digits via Rust `{:.*e}`, read the exponent X, and if `-4 ≤ X < P` render fixed-style
(decimal placed in the rounded digits by string manipulation — no double-rounding — then trailing zeros
and the dot stripped fully), else scientific-style (mantissa keeps at least `.0` — a PHP quirk vs C — and
the exponent re-stamped to PHP's always-signed min-1-digit form). `%G` upper-cases only the separator.
Unlike `%e`/`%f`, `%g` signs by the IEEE sign bit, so `-0.0` → `-0`. The PHP mirror folds `%g`/`%G` into
the float branch (delegates the raw directive to native `sprintf`). Verified by an exhaustive
structured+random sweep of the Rust renderer vs php-8.5.8 (341k comparisons — branch boundaries, digit-gain
roundings, half-to-even, subnormals, ±0.0, precision `.0`–`.17` — zero diffs), a curated subset baked as
oracle-string unit tests, and a run≡php diff on the example. `%N$` positional + precision on `%s`/`%d` remain.
No new `Op`/`Value`.

### Added — `String.format` scientific `%e`/`%E` (slice 3b)

`String.format` now supports the scientific conversions `%e`/`%E` (int/float operand), with a `.precision`
(default 6) and the existing flags/width. The renderer reproduces PHP `sprintf` byte-for-byte: Rust
`{:.*e}` on the magnitude supplies the mantissa and round-half-to-even, then the exponent is re-stamped to
PHP's form — **always signed, minimum one digit, no leading zeros** (`e+3`/`e+20`/`e-1`/`e+100`), unlike
C/Rust's minimum-two-digit exponent. `%E` upper-cases only the separator. The sign is by value (`< 0.0`),
so `%e` leaves `-0.0` unsigned (matching PHP). The PHP mirror `__phorj_format` folds `%e`/`%E` into the
float branch and delegates the raw directive to native `sprintf`. A non-number operand faults cleanly (the
phorj strictness upgrade over PHP's silent coercion). `examples/guide/string-format.phg` +
`text_format_scientific_matches_php_byte_for_byte` (oracle strings from php-8.5.8). `%g`/`%G` come in slice
3c. No new `Op`/`Value`.

### Fixed — `String.format` `%f` signs by value, not the IEEE sign bit

`%f` computed its sign with `is_sign_negative()`, which is true for `-0.0` — so `String.format("%f", -0.0)`
rendered `-0.000000` on the Rust backends while transpiled PHP rendered `0.000000` (a latent run≠php
byte-identity break shipped in slice 2, untested — no example used `-0.0`). PHP signs a `%f` iff the value
is `< 0.0` (`-0.0` unsigned; a value that rounds to zero keeps its sign, e.g. `%.2f` of -0.001 → `-0.00`).
The rule is now `f < 0.0` — the same rule `%e` uses. Regression test + example line.

### Added — DI `#[Transient]` lifetime (DI v1 slice 4b)

A class marked `#[Transient]` (or `#[DI.Transient]`) opts OUT of the default-shared DI lifetime: the graph
builds a FRESH instance at each injection point instead of sharing one per resolution root. A shared
dependency of a transient stays shared. To support this, the resolved graph is now a **`Built` tree** and
the synthesized factory is emitted by **let-floating** it — shared nodes hoisted to `var`s once (in
topological order), transient nodes inlined fresh at each use — with construction kind (`new` vs
`#[Provides]`) and sharing (shared vs transient) fully orthogonal. For an all-shared graph the emitted PHP
is byte-identical to before (regression-guarded against the shipped `di.phg` / `di-field-injection.phg` /
`di-provides.phg`). Cycle detection is unchanged (transients are still cycle-checked). `#[Transient]` off
a class is `E-TRANSIENT-ARGS` for stray args; import-gated like the other DI symbols.
`examples/guide/di-transient.phg` (output `own 1 1 | shared 1 2` distinguishes correct from both failure
modes) + a runtime test asserting the same. No new `Op`/`Value`.

### Added — DI `#[Provides]` factories (DI v1 slice 4a)

A `#[Provides]` (or qualified `#[DI.Provides]`) **static method** whose return type is `T` now teaches the
DI graph to construct `T` by calling that method instead of `new T(…)`. The method's own parameters are
autowired, and a provider takes **precedence** over both `new T` and single-impl-interface auto-bind — so
it injects a type you don't own, a type whose constructor needs a config value the graph can't wire, or
binds an interface to a chosen implementation (the multi-impl disambiguator). Provider modules are plain
classes (scanned even when not `#[Injectable]`). Two providers for the same type is `E-DI-AMBIGUOUS`;
`#[Provides]` off a static method / without a return type is `E-PROVIDES-TARGET`; import-gated like the
other DI symbols (`E-INJECTED-TYPE-BARE`). The synthesized factory emits `Owner::method(deps)` — byte-
identical `run ≡ runvm ≡ php`. `examples/guide/di-provides.phg`. No new `Op`/`Value`.

### Added — DI field injection (DI v1 slice 3)

An **injectable-typed instance field with no initializer** is now auto-wired at construction. Mechanism
(the ruled "synthesized-ctor" model): `desugar_di` folds each such field into its class's constructor as
an appended **promoted parameter** (synthesizing an empty-body constructor if the class has none), so the
field is set once (stays immutable) and is resolved/shared/cycle-checked by the SAME graph machinery as a
constructor dependency — and it transpiles to an ordinary PHP promoted-constructor property
(byte-identical `run ≡ runvm ≡ php`). A field WITH an initializer is user-provided (left alone); a
non-injectable-typed field is an ordinary field. Field-injection cycles are caught (`E-DI-CYCLE`) — the
synthesized-ctor model makes them unbreakable, as designed. `examples/guide/di-field-injection.phg`
demonstrates a `Clock` shared between a ctor-injected and a field-injected holder. No new `Op`/`Value`.

### Added — `String.format` integer-radix conversions (slice 3a)

`String.format` (DEC-199 PHP-`%`-sprintf) now supports the integer-radix conversions `%x`/`%X` (hex),
`%o` (octal), and `%b` (binary), with the existing flags/width. They are UNSIGNED — a negative int
renders as its 64-bit two's-complement bit pattern (`%x` of -1 → `ffffffffffffffff`), exactly matching
PHP `sprintf` on a 64-bit build (`n as u64` is the bridge); a non-int value is a clean fault, and
precision on a radix conversion is rejected (`E-FORMAT-UNSUPPORTED`, later slice). The Rust renderer,
the compile-time gate (shared `parse_format_directive`), and the transpiled `__phorj_format` PHP helper
(delegates the raw directive to `sprintf`) all agree — byte-identical `run ≡ runvm ≡ php-8.5.8`, verified
across positive/negative/zero/width/zero-pad/left-justify. `%e`/`%g` (scientific) remain a later slice.
`examples/guide/string-format.phg` extended.

### Changed — DI follows the import discipline + annotation-driven `inject()` (DI v1 §7 + slice 2)

**Fix (nothing in the wind):** DI v1 slice 1 shipped `#[Injectable]` and `inject` as **ambient** symbols
(recognized with no import) — a violation of the locked "everything is imported" discipline. They now
live in `Core.DI` and obey the same rule as `Core.Http`: the bare surface (`#[Injectable]`, `inject`) via
member-import (`import Core.DI.Injectable;` / `import Core.DI.inject;`), or qualified
(`#[DI.Injectable]`, `DI.inject<T>()`) via `import Core.DI;`. An un-imported bare attribute is
`E-INJECTED-TYPE-BARE`; an un-imported explicit `inject<T>()`/`DI.inject<T>()` is the new `E-DI-NO-IMPORT`.
`inject` is **no longer a keyword** — it is freed as an ordinary identifier when `Core.DI` is not imported
(a user function named `inject` works). The parser recognizes only the explicit turbofish forms
(`inject<T>()`, `DI.inject<T>()`); the no-turbofish forms parse as ordinary calls and `desugar_di` converts
them import-awarely.

**Feature (slice 2):** annotation-driven `inject()` — the target type is inferred from the position (a
typed `var` declaration, a `return`, or a lambda return type) instead of an explicit `<T>`: `App app =
inject();`, `function build(): App { return inject(); }`. Draws on the same graph resolver, so it expands
to the identical `phorjInject<T>()` factory — byte-identical `run ≡ runvm ≡ real PHP 8.5`. Not an
annotation source: call-argument / parameter-default positions, and `Optional`/generic targets (→
`E-DI-MISSING`) — see `KNOWN_ISSUES.md`. `#[Provides]`/`#[Transient]`/field injection remain later slices.
`examples/guide/di.phg` now demonstrates both forms. No new `Op`/`Value`; no backend change.

### Added — user-defined attributes are usable (DEC-194 slice 2b-3)

A class marked `#[Attribute]` can now be **applied** as `#[Tag("...")]` on a class or function, and the
use is fully validated at **compile time** (stronger than PHP, which only fails when the attribute is
reflected): the argument count must match the attribute class's constructor (`E-ATTRIBUTE-ARITY`) **and each
argument's type must be assignable to the matching constructor parameter** (`E-ATTRIBUTE-ARG-TYPE` — e.g.
`#[Tag(123)]` where `Tag(string label)` is rejected), and an undeclared attribute is `E-UNKNOWN-ATTRIBUTE`. `ClassInfo` gained `is_user_attribute` (set in the collect
pass); a shared `check_user_attribute_use` handles both the function/method and class attribute-check sites.
Attributes remain inert metadata (no runtime effect yet), so `phg run` ≡ `phg runvm` ≡ transpiled PHP stay
byte-identical — the transpiler drops the (unread) attribute. Valid on all targets this slice; per-target
restriction rides the `#[Attribute(targets: […])]` form (needs named arguments). Ships
`examples/guide/user-attributes.phg`. **Fix:** the formatter now emits **class-level** attributes (a shared
`item_attrs` printer for functions and classes) — a 2a regression where `phg format` silently stripped a
class's `#[…]`, which the fmt-idempotence gate guards against.

### Added — the `#[Attribute]` marker declares a user attribute (DEC-194 slice 2b-1)

A class carrying the built-in `#[Attribute]` marker (`import Core.Runtime.Attribute;`, or the qualified
`#[Runtime.Attribute]` via `import Core.Runtime;`) is now recognized as a **user-defined attribute type** —
the one attribute that legally targets a class. It obeys the two-mode "nothing in the wind" import
discipline (a bare unimported `#[Attribute]` is `E-INJECTED-TYPE-BARE`), single-sourced in
`Attribute::is_attribute_marker`, and `enforce_injected` now walks class-level attributes (closing the gap
where a class's own `#[…]` skipped the import check). This slice accepts the **bare** marker (the class
becomes an attribute valid on all targets, non-repeatable); the `targets: […]` / `repeatable` arguments
are a clean `E-ATTRIBUTE-ARGS` "not yet" (2b-2), and *using* a declared attribute (`#[Tag]` on a target)
plus reflection/transpile land in later slices. No runtime behaviour change — attributes remain inert
metadata.

### Added — attributes parse on `class` declarations (DEC-194 user-attribute system, slice 2a)

Groundwork for the user-defined attribute system. `#[…]` attributes previously parsed only on a free
`function` (and, inside a class, a method); they now also parse on a top-level **`class`** declaration
and are carried on `ClassDecl.attrs`. No attribute *targets* a class yet — the built-ins `#[Route]`
(route handler) and `#[UncheckedOverflow]` (free function) are not class-valid, and user-declarable
attributes land in a later slice — so a class attribute is **validated and rejected** with a check-stage
`E-ATTR-TARGET` (moved from the old parse-stage rejection), never silently accepted. Attributes on an
enum/interface/trait/import still parse-reject until their target slices land. Pure plumbing: no runtime
behaviour change; every existing program is unaffected.

### Changed — `#[Unchecked]` renamed to `#[UncheckedOverflow]` under `Core.Runtime.*`

The opt-in wrapping-integer-arithmetic attribute moved from the flat `Core.Unchecked` marker module to
the structured `Core.Runtime.Integer.UncheckedOverflow` (perf/runtime knobs now live under a
`Core.Runtime.*` namespace; `Core.Runtime` already held `monotonicNanos`). The attribute is now a
proper injected attribute-**type** (like `#[Route]`), gated by the ratified two-mode "nothing in the
wind" import discipline instead of a bespoke string match:

- **member import → bare:** `import Core.Runtime.Integer.UncheckedOverflow;` → `#[UncheckedOverflow]`
- **module import → qualified:** `import Core.Runtime.Integer;` → `#[Integer.UncheckedOverflow]`
- unimported bare use → `E-INJECTED-TYPE-BARE`; the old `#[Unchecked]` → `E-UNKNOWN-ATTRIBUTE`.

The rename is legibility-only — the leaf `UncheckedOverflow` is self-sufficient and signals the safety
opt-out (a check is removed), where bare `Unchecked` was ambiguous. Semantics, codegen, faults, and the
`E-TRANSPILE-UNCHECKED` §14 quarantine are unchanged; attribute recognition is single-sourced in
`Attribute::is_unchecked_overflow` (checker, compiler, interpreter, transpiler all consult it, so the
four can never drift). `examples/guide/unchecked.phg` + docs migrated. Byte-identity preserved.

### Added — JIT slice b3b: `phg run` wired to the JIT (the perf win reaches the CLI)

The unboxed JIT is now reachable from `phg run` / `phg benchmark` — the native codegen that **beats
release php+JIT on recursive-int workloads** is no longer test-only. The VM's `Op::Call` gained a
hot-function hook (feature
`jit`): when a callee (and its transitive call graph) is unboxed-eligible, it is compiled **once per
program** to native code and run through the unboxed path instead of pushing a VM frame. `fib` in
`examples/fib.phg` now executes natively under a jit-built binary.

- **Unboxed-only, by design.** Only the unboxed path is routed (the actual perf win); the boxed
  codegen stays the byte-identity oracle, never a runtime — kernel-call-per-op would add fault/depth
  risk for no speedup. `main` prints, so it is never eligible; the `Op::Call` hook is what reaches the
  hot leaf.
- **VM-fallback owns all fault rendering.** On any JIT fault the (side-effect-free, per the
  eligibility invariant) function is re-executed on the VM, which reproduces the fault *with* the
  source line and stack trace a bare JIT fault string lacks. Over-faulting is safe; the one lethal
  case — an under-fault that returns a value where the VM overflows — is closed by seeding the JIT
  depth counter from the VM's live frame count (`start_depth = frames.len() + 1`, now threaded into
  `run_unboxed`).
- **Compile-once cache.** A shared `JitCache` (`Rc<RefCell<_>>`) amortizes Cranelift compilation
  across every `Vm` built for one program — `phg benchmark` spins a fresh `Vm` per iteration, so a
  per-`Vm` cache would time cold compile against php's warmed JIT.
- **Result.** `scripts/microbench.sh` (phorj vs a real `php:8.5-cli`+JIT in Docker, output-identity
  gated): the recursive-fib micro `fibrec` is a **WIN vs release php+JIT** (~2.4× best-case on a
  shared box — the robust claim is the WIN, not the magnitude; per-feature WIN/LOSS is what the G-8
  ratchet gates). The iterative micros still LOSE because they use `mutable`/`while` (`SetLocal`,
  outside the unboxed subset) and remain on the VM — widening the subset is future work.
- **Verification.** The differential harness runs byte-identically under `--features jit` (144 examples,
  run ≡ tree-walker ≡ PHP 8.5.8) — every eligible call is now exercised through the JIT. A hit-counter
  test proves the native path is actually taken (a silent 100%-fallback would false-green), and a
  linear-recursion test bracketing `MAX_CALL_DEPTH` through the real `cmd_run` path proves the
  overflow threshold matches the interpreter oracle (and that 4096 native frames don't blow the
  production stack). Still `#[cfg(feature = "jit")]`; the stock non-jit `phg` is byte-for-byte
  unchanged. (Open, developer-owned: ship jit-on-by-default?)

### Added — JIT codegen slice 1 (Cranelift): pure-int leaf functions compile & run natively

First codegen of the Cranelift JIT backend (dependency-policy domain #7, perf mandate G-8). `src/jit/`
gains `compile_and_run`: it lowers a **default-deny int-arithmetic leaf subset** of a compiled
function's bytecode — `Const`(int) / `GetLocal` / `AddI` / `SubI` / `MulI` / `DivI` / `RemI` /
`Return`, straight-line — to native machine code via Cranelift, then runs it through the
`finalize -> transmute -> call` path. Arithmetic threads **boxed `Value`s through the single-sourced
`value.rs` kernels** (`int_add`, …), so overflow / divide-by-zero faults carry the **same canonical
strings as the VM by construction** (Invariant 4); anything outside the subset is rejected with
`JitError::Unsupported` (the caller falls back to the VM — the seed of the eligibility predicate).
**Not yet wired into `phg run`** — the `phg run` cutover plus control-flow branches/loops and a
differential example that provably exercises the JIT are the next (wiring) slice; this commit is the
substrate and its verification only.

- **Deps:** `cranelift` / `cranelift-jit` / `cranelift-module` 0.133, behind the non-default `jit`
  feature, non-wasm target (mirrors `corosensei`). Verified building on the pinned toolchain (1.96.0).
- **Unsafe island landed:** crate roots `#![forbid(unsafe_code)]` -> `#![deny(unsafe_code)]`
  (`src/lib.rs`, `src/main.rs`); the single audited allow-island lives in `src/jit/mod.rs`. The CI
  `unsafe-island` gate confines it.
- **CI:** a new `jit` job builds + lints + tests `-p phorj --features jit`. The default `gate` job's
  `cargo test --workspace` does **not** compile the `jit` feature, so without this job the JIT code
  would rot unverified — a structural false-green. `-p phorj` (not `--workspace`) because the
  `playground` member has no `jit` feature.
- **Tests (`--features jit`):** JIT value matches the VM oracle for int arithmetic; integer overflow
  and divide-by-zero surface the exact single-sourced kernel fault strings; a non-int function is
  default-denied.
- **Perf:** none claimed. The code is unwired and unmeasured; the design spike's ~3×-over-php+JIT is a
  *hypothesis* for the wired path, to be measured under `phg run` in the wiring slice (Invariant 11).

### Changed — dependency policy amended: native codegen (JIT) admitted as domain #7 (scaffold only)

The external dependency policy (`docs/specs/UNIFIED-SPEC.md` §"External dependency policy") gains a
**7th admitted domain — native codegen (`cranelift-jit`)** — the ruled path to the G-8 perf mandate
(the bytecode VM is ~28× slower than release-php+JIT on hot numeric loops; only native codegen closes
it). This is a *mandate-driven* exception to the policy's "no performance crates" rule: beating
release-php+JIT per feature is provably impossible on a `std`-only bytecode VM under `forbid(unsafe)`.
The JIT lives **in-tree** at `src/jit/` (it couples to `Op`/`Value`/chunk — a separate crate would
force those `pub` + create a dependency cycle) and introduces phorj's **first first-party `unsafe`**,
confined to a `src/jit/` island: the crate root drops `#![forbid(unsafe_code)]` → `#![deny(unsafe_code)]`
with a single audited `#![allow(unsafe_code)]` there, and a CI `unsafe-island` gate fails the build if
an `allow(unsafe_code)` escape appears anywhere outside `src/jit/`. **That scaffold commit added only
the policy, the CI gate, and an empty `src/jit/`** — the `cranelift` crate and the `forbid`→`deny`
change then landed with JIT codegen slice 1 (see the entry above). See `docs/plans/perf-wave.plan.md`.

### Changed — `phg serve` runs on the bytecode VM by default (`--tree-walker` for the interpreter)

`phg serve` now compiles the program and runs each request's `respond(bytes): bytes` on the bytecode
VM instead of the tree-walking interpreter — **byte-identical** output (asserted by dual-backend tests
in `tests/serve.rs`, single-threaded AND through the multi-worker pool, since serve is outside the
differential harness) and **faster**: measured **~2.3× lower end-to-end latency** per request on a
representative handler over keep-alive (17.1 µs vs 39.6 µs median, release binary; the handler-compute
gain is larger — the fixed socket round-trip is in both numbers). `--tree-walker` selects the
interpreter oracle (also required to serve an *overloaded* `respond`, which the VM path rejects).

New VM primitive `Vm::run_entry(entry, args) -> (Value, String)` — call a resolved top-level function
by index with captured return value + stdout, the VM analog of `interpreter::call_named` (the shared
dispatch loop is now `run_to_completion`, with `run_main` a thin wrapper — byte-identical, differential
green). Each serve worker compiles its own program (a `BytecodeProgram` holds `Rc` state and can't
cross threads), amortised over its requests. A serve/web program with no `main` (its entry is
`respond`) gets an inert synthesized `main` so it compiles — never invoked. Still ~25× slower than a
tuned PHP+JIT (the per-feature perf mandate is unmet until the JIT backend; `docs/plans/perf-wave.plan.md`).

### Added — call-argument expected-type threading for list/map literals (Wave C foundation)

A list/map **literal** passed directly as a call argument now threads the parameter's collection type,
so `f([1, "x"])` type-checks against a `List<int | string>` parameter (each element checked against
the union) instead of being bottom-up inferred as `List<int>` and rejected with "elements must share
one type." This is the call-argument counterpart of the existing declaration-initializer / return
threading (DEC-178 / UA-1.6), and the foundation the upcoming `String.format` (W3-5) rides on. Only
CONCRETE parameter types thread (guarded by `ty_has_param`); generic callees stay on the existing
unification path — a homogeneous literal to a generic callee (`Set.of([1,2,3])`) works as before,
while a heterogeneous one (`Set.of([1,"x"])`, needing bidirectional inference of `T`) stays deferred.
Checker-only, byte-identical.

### Fixed — `String.split(s, "")` byte-identity + new `String.characters` (output-parity pass)

The output-parity sweep found another latent byte-identity break: `String.split(s, "")` (empty
separator) returned a per-char-with-empty-ends list on the Rust backends but **faulted** in transpiled
PHP (`explode("")` throws `ValueError`). An empty separator is ill-defined, so it now **faults** on all
backends (consistent with PHP). To split a string into its characters, use the new
**`String.characters(s) -> List<string>`** — code-point-safe (`"café"` → `["c","a","f","é"]`, like
`String.reverse`; erases to `preg_split('//u', …)`), parallel to `String.lines`. Non-empty separators
are unchanged.

### Fixed — `Conversion.truncate`/`round` byte-identity on out-of-range floats (fault-parity pass)

The correct-lens fault-parity pass found a latent byte-identity break: `Conversion.truncate`/`round`
emitted a raw `(int)`/`(int)round` cast, so an out-of-i64-range float (e.g. `1.0e30`) produced
DIFFERENT output — the Rust backends saturated (`i64::MAX`) while transpiled PHP wrapped
(`5076964154930102272` + a warning). Now both `truncate` and `round` **fault** on NaN/±∞/out-of-range
(consistent with `floatToIntExact`; via throwing `__phorj_trunc`/`__phorj_round` PHP helpers), so
`run ≡ runvm ≡ real PHP`. In-range conversions are unchanged. Callers wanting graceful overflow handling
use `toInt(float) -> int?` (null on out-of-range) — unchanged. Behavior change: `truncate`/`round` are
now partial (can fault) instead of silently returning a wrong int. (Findings:
`docs/research/fault-parity-pass-2026-07-05.md`.)

### Changed — fault intrinsics now require an explicit import (DEC-196 Q3, breaking)

The four fault intrinsics are no longer import-free. They live in two reserved language-core modules
and follow the same two-mode discipline as types and enum variants:

- **`Core.Assert`** = { `assert` } — the conditional runtime check.
- **`Core.Abort`** = { `panic`, `todo`, `unreachable` } — the unconditional aborts.

Two import modes:

- **whole-module import → QUALIFIED call:** `import Core.Assert;` → `Assert.assert(cond)`;
  `import Core.Abort;` → `Abort.panic("m")` / `Abort.todo()` / `Abort.unreachable()`.
- **member import → BARE call:** `import Core.Abort.panic;` → `panic("m")` (grouped:
  `import Core.Abort.{ panic, todo };`).

Any intrinsic call not covered by the matching import is **`E-UNIMPORTED`** (this keeps "nothing in
the wind": a bare intrinsic requires an explicit member import naming it). The two forms lower
identically — the qualified form is normalized to the bare intrinsic before any backend — so
`run ≡ runvm ≡ real PHP` byte-identity is preserved. `assert` stays distinct from the `Core.Test.assert`
unit-test native. New example `examples/guide/intrinsic-imports.phg`; `phg explain E-UNIMPORTED`.

### Changed — `String.uppercase`/`lowercase` renamed to `upperCase`/`lowerCase` (DEC-196 Q2, breaking)

Enforcing camelCase everywhere (Invariant 12): the two all-lowercase compound native names
`String.uppercase` and `String.lowercase` are renamed to `String.upperCase` / `String.lowerCase`.
Behaviour is unchanged — the PHP transpile still emits `strtoupper`/`strtolower` and the interpreter
logic is untouched; this is a name-only breaking change. UFCS calls follow (`s.upperCase()`). The
`.phg` corpus was already 100% camelCase-clean (constants stay `SCREAMING_SNAKE_CASE`), so the change
collapsed to these two natives. The `charter_function_names_are_lowercamel` test gained a curated
regression guard so these specific compounds cannot silently return (`substring`/`capitalize` etc.
remain legitimate single words — an all-lowercase name is not mechanically decidable as a compound).

### Housekeeping — examples/ layout + doc-name reconciliation (DEC-196 Q1)

Cleanup pass from the 2026-07-05 examples/conformance audit:

- Renamed `examples/fmt/` → `examples/format/` and `examples/bench/` (incl. `manual/`) →
  `examples/benchmark/`, matching the real CLI verbs (`phg format`, `phg benchmark`). Updated every
  reference (`bench/baseline.json`, `playground/web/gen_examples.py` `SKIP_DIRS`, `tests/runtime.rs`,
  `examples/README.md`, `docs/MILESTONES.md`) and regenerated `playground/web/examples.js`.
- `phg benchmark`'s report header now prints `phg benchmark — …` (was `phg bench — …`).
- Swept dead-verb prose (`phg fmt`/`phg bench`/`phg disasm`) → full verbs in `src/**` rustdoc and the
  moved example READMEs/comments (module/file/function names unchanged).
- `examples/web/core-http.phg` now imports `Core.String` explicitly (was relying on the Http prelude).
- `STABILITY.md` module names reconciled to the real registry names (`Core.Output`/`String`/
  `Conversion`/`Validation`/`Reflection`/`Environment`/`Cryptography`).
- Removed the superseded `docs/plans/wave0-remainder.plan.md` straggler (MASTER-PLAN is the sole SSOT).

### Changed — `phg format` is now width-canonical (DEC-187)

The formatter gained a **width-aware layout engine**: a new Wadler/prettier document IR
(`src/fmt/doc.rs` — `Text`/`Line`/`SoftLine`/`Concat`/`Nest`/`Group` + a `fits` solver + a
column-budget renderer) behind the printer's expression layer (`expr()` now builds a `Doc`; a thin
flat wrapper keeps every non-wrapping context byte-identical). Statement values are rendered against a
**100-column budget**: call / `new` / `parent` argument lists, collection and map literals, `match`
arms, and `.`/`?.` **method chains** (≥2 links) break one element per line when the line overflows,
and stay inline when they fit.

This **revises DEC-187's original "expand-only" ruling** (developer-adjudicated at the start of this
session): layout is re-derived purely from width like `prettier`/`rustfmt`/`gofmt` — author-inserted
line breaks are **not** preserved (a gratuitously hand-broken short chain now collapses). The reason:
width-canonical is idempotent by construction (`fmt(fmt(x)) == fmt(x)`) and needs no source access,
which the print-from-AST design deliberately lacks; honouring author breaks would have fought that
invariant. Interpolation holes (`"{…}"`) are **never** broken — a newline there would change the
string value (meaning preservation wins over the budget). Statements, comments, and declaration
headers stay imperative (the hybrid seam); declaration parameter lists, binary-operator chains, class
headers, and control-flow conditions are tracked follow-ups (`KNOWN_ISSUES.md`).

The whole example + selftest corpus was reformatted to canonical form (35 files), and the corpus
dogfood (`tests/fmt.rs`) was strengthened from idempotency-only to `fmt(src) == src` (folds UA-0.8).
Ships `examples/format/showcase.phg` + `examples/format/README.md`. `phg lsp` document formatting reuses
`fmt::format`, so both editors get width-canonical formatting for free. Byte-identical
`run ≡ runvm ≡ real PHP 8.5.8` across every reformatted example (differential harness); 8 doc-core
unit tests + 4 width-canonical behaviour tests + the corpus dogfood, full gate green.

### Added — Wave B foundation: canonical `Core.Option` / `Core.Result` (DEC-182)

The two canonical error/absence types ship as **compiler-injected** enums (same pattern as
`Core.Json`), gated on `import Core.Option;` / `import Core.Result;`. The first *generic* injected
enums — `T`/`E` are checked as type parameters then erased before any backend, so run/runvm/PHP stay
byte-identical.

- **B-1 (types):** `inject_option_prelude` / `inject_result_prelude` (`src/cli/mod.rs`) inject
  `enum Option<T> { None, Some(T value) }` and `enum Result<T, E> { Success(T value), Failure(E error) }`.
  Variants are reached **qualified only** (`Option.Some`, `Result.Failure`; bare use is
  `E-INJECTED-VARIANT-BARE`). A user-declared same-name enum shadows and skips the injection.
  `Option<T>` is DISTINCT from the built-in `T?` (explicit conversion, no implicit coercion).
  Examples `guide/core-option.phg` + `guide/core-result.phg`.
- **B-2a (Option combinators + conversions):** `Core.Option` module natives (`src/native/option.rs`)
  reached UFCS-style (`opt.map(f)` → `Option.map(opt, f)`, same dispatch as `list.map`, since enums
  have no methods): `map` / `andThen` / `filter` (higher-order) + `getOrElse` (eager default) +
  `Option.ofNullable(T?)` / `toNullable() -> T?` (the explicit `T?`↔`Option` bridge). Erase to gated
  `__phorj_option_*` PHP helpers over the injected `Some`/`None` classes. Example
  `guide/option-combinators.phg`.
- **Fix (pre-existing crash, surfaced by `andThen`):** a `new` inside an argument subtree relocated by
  the UFCS rewrite (`xs.map(function(x) => new C(x))`, any UFCS call with a constructing lambda/arg)
  bypassed `unwrap_new` and panicked the interpreter/compiler with a surviving `Expr::New`.
  `rewrite_ufcs`'s walker now strips `Expr::New` (incl. the qualified-variant callee rewrite) in
  relocated subtrees.
- **Inference:** `unify` now binds a type parameter from a non-null argument against an `Optional(T)`
  parameter (`Option.ofNullable(42)` infers `T = int`), aligning it with the existing
  `(other, Optional(t))` assignability rule.
- **B-2b (Result combinators, DEC-185):** the full ruled `Core.Result` combinator set (`src/native/result.rs`),
  reached UFCS-style (`res.map(f)` → `Result.map(res, f)`): `map((T)->U)` · `mapErr((E)->F)` (remaps the
  error type) · `andThen((T)->Result<U,E>)` (success bind — threads the error `E` through the callback) ·
  `orElse((E)->Result<T,F>)` (error bind / recovery) · `getOrElse(T)` (eager default) · `toOption() ->
  Option<T>` (Result→Option bridge, drops the error) · `isSuccess()` / `isFailure()`. `filter` is
  deliberately omitted (no error to synthesize on `false`). Erase to gated `__phorj_result_*` PHP helpers
  over the injected `Success`/`Failure` classes (`isSuccess`/`isFailure` emit an inline `instanceof`).
  Example `guide/result-combinators.phg` (byte-identical run/runvm/PHP), 7 native unit tests.
- **Guard (`E-RESULT-TOOPTION-NEEDS-OPTION`):** `Result.toOption` produces a `Core.Option` value whose
  `Some`/`None` PHP classes exist only when `Core.Option` is injected — so using it without
  `import Core.Option;` type-checked and ran on the interpreter/VM but fataled in transpiled PHP (a
  byte-identity break). The checker now rejects it up front (both the UFCS and qualified call forms), so
  every backend refuses in lockstep; `phg explain` entry + 3 checker tests.

### Added — Wave B B-2c: variant imports (DEC-186)

Bring a compiler-injected enum's variants into bare (or aliased) scope, so they need not be written
qualified. Two parts:

- **Part 1 (parser):** variant-path imports `import Core.Result.Success [as MyOk];` and path-first
  brace **groups** `import Core.Result.{ Success, Failure as Xzs };` (single-level prefix; trailing
  comma + multi-line allowed; empty group is `E-IMPORT-GROUP-EMPTY`). A group desugars to one
  `Item::Import` per member (parser `pending_items` buffer).
- **Part 2 (binding):** a pre-check pass (`resolve_variant_imports`, wired in `check_and_expand_reified`)
  rewrites every imported-variant use — bare or `as`-aliased, in **construction** (`new Success(1)`) and
  **`match` patterns** (`Success(v) =>`, `Fail(e) =>`) — to the qualified `Enum.Variant` form, reusing
  the proven byte-identical qualified-variant machinery (so `unwrap_new` still emits the bare backend
  variant; no bespoke rename). Zero-payload variants keep the existing parens rule in patterns
  (`None()`). The checker rejects a bound name that collides with a local type or is imported twice
  (`E-IMPORT-CONFLICT`) and a nonexistent variant (`E-IMPORT-UNKNOWN`). Un-imported injected variants
  stay qualified-only (`E-INJECTED-VARIANT-BARE`). Example `guide/variant-imports.phg` (byte-identical
  run/runvm/PHP) + 3 parser tests + 5 checker tests. `phg format` canonicalizes a group to one import
  per line (a group has no dedicated AST node — it is N imports).

### Added — interactive debugger: `phg debug` (M-DX S5) — **M-DX COMPLETE**

An **interpreter-only** pause/step/inspect debugger with two frontends over one shared engine —
Dev-only, entirely off the correctness spine (never touches stdout / the differential).

- **Engine** (`src/debug.rs`): `Debugger` (line breakpoints + depth-aware `StepMode`
  Continue/StepInto/StepOver/StepOut), `DebugFrontend` trait, `DebugSession`. Pure + deterministic
  (unit-tested with a scripted frontend). Hooked into `exec_stmt` (a cheap `Option` check on the hot
  path; the pause is a `#[cold]` helper so the recursive frame stays small — differential unaffected).
- **REPL** (`phg debug <file>`): `step`/`next`/`stepout`/`continue`, `break`/`clear <line>`,
  `locals` (secure renderer — `Secret` redacted), `backtrace`, `quit`. UI on stderr, program output on
  stdout. Starts paused at the first statement.
- **DAP** (`phg debug --dap <file>`, `src/dap.rs`): a Debug Adapter Protocol server on stdio
  (`Content-Length`-framed JSON, same transport as the LSP) so VS Code / JetBrains can set breakpoints,
  launch, stop, inspect the stack + locals, and step. Handshake → run-to-breakpoint → `stopped` →
  `stackTrace`/`scopes`/`variables` → step/continue → `terminated`; round-trip tested.
- Interpreter-only by design (the VM has no line/local debug table; the parity spine makes an
  interpreter session faithful). The shared JSON parser (`src/lsp/json.rs`) was promoted to a
  crate-level `src/json.rs` reused by both the LSP and DAP. Walkthrough: `examples/debug/README.md`.

### Added — assertions guide + M-DX S4 scope (assertions already shipped)

`assert(cond)` / `assert(cond, msg)` were already a complete language feature (checker-validated,
`FaultMsg::Assert` on both backends, transpiled to a real PHP `throw` — never the disableable
`assert()`, always-checked). M-DX S4 formalizes and showcases them: a new `examples/guide/assertions.phg`
(byte-identical `run ≡ runvm ≡ real PHP`) + coverage-matrix entry. **The keystone holds already** —
assertions are *never stripped* in Release (that would change control flow); a profile may only make
the failure message terser. **Operand inspection on a failing assert is delivered by S3's
`--dump-on-fault`** (a failing assert is a `Signal::Runtime` fault), so no separate Dev-rich assert
message was added — avoiding a redundant, spine-risking interpreter/VM-asymmetric code path.

### Added — value-dump on fault: `phg run --dump-on-fault` (M-DX S3)

The headline diagnostic aid: on an uncaught runtime fault, print the **faulting frame's local
variables** (name → value) to stderr, after the stack trace. Opt-in and Dev-only.

- **Enablement:** `--dump-on-fault` on `phg run`/`runvm`, and only under the Dev profile — a
  `Release` `phg build` artifact never emits it (gated by `dump::should_dump` = enabled ∧ Dev; no
  environment variable can turn it on).
- **Secure + deterministic:** rendered through the S2 `inspect` renderer — `Secret<T>` locals show
  `Secret(<redacted>)` (never the plaintext), depth/element/length are capped, and locals are sorted
  by name (reproducible).
- **Side-channel only:** stderr, never stdout; nothing is transpiled — `run ≡ runvm ≡ PHP` is
  untouched (the dump-carrying `Diagnostic.dump` is a boxed, out-of-spine string).
- **Backends:** the rich named-locals dump is produced on the **interpreter** (which holds live
  named scopes); `runvm` shares the byte-identical **backtrace** but omits the locals section (the VM
  has slot-indexed locals with no name table — same interpreter-only rationale as the S5 debugger).
- Walkthrough: `examples/dump/README.md`. Tests: `dump` unit (gate + redaction + format), end-to-end
  `tests/cli.rs` (redacted locals only with the flag; VM backtrace-only; no stdout bleed).

### Added — secure value renderer (M-DX S2)

`inspect::render(&Value) -> String` — the single, safe-by-construction `Value → String` substrate the
value-dump (S3), assertion detail (S4), and debugger (S5) will share. Internal (no CLI surface yet);
lives outside the correctness spine (side-channel only, never transpiled). Three guarantees:
- **Secret redaction** — an instance of the injected `Secret<T>` wrapper renders `Secret(<redacted>)`
  without ever descending into its `value` field (mirrors the transpiler's `#[\SensitiveParameter]`
  and the type system's non-printability), including when nested inside a list/map/instance.
- **Bounded** — depth, per-collection element count, and scalar byte length are capped
  (`RenderCaps`); overflow truncates with `…`/`… (+N more)`.
- **Deterministic** — insertion-ordered `Map`/`Set` and slot-ordered instance fields; no addresses,
  `Rc` counts, or hash order — reproducible, so it is golden-testable.

### Added — build profiles: `Dev` / `Release` (M-DX S0)

A first-class `profile::Profile { Dev, Release }` — the gate every environment-sensitive,
value-exposing, or diagnostic-verbosity feature will key off. **Keystone: a profile changes
side-channels/diagnostics ONLY, never observable program output** — `run≡runvm≡real PHP` holds
identically under both (verified: a Dev and a Release `phg build` of the same program print
byte-for-byte the same output).

- **How it's chosen (entry-time, never a runtime env var):** `phg run`/`runvm`/`test` are Dev (the
  interactive tool); `phg serve` is Release unless `--dev` (its rich HTML fault page leaks
  traces/source); `phg build` is **Release by default**, `--dev` opt-in.
- **Secure by construction:** `phg build` bakes the profile into the artifact's `.phorj` container
  (the previously-unused `flags` byte, bit 0 — backward-compatible: a pre-profile artifact reads as
  Release). A shipped binary sets its profile from its own container before running, so no
  environment variable can flip a Release artifact into Dev.
- **Folded in the ad-hoc `serve --dev` switch:** `serve` now derives its dev fault-page behaviour
  from the `Profile` rather than a hand-plumbed bool. (Filled the test gap: the `dev=true` rich-page
  path was previously uncovered.)

### Fixed — diagnostics quality + three soundness holes (M-DX S1)

Front-end-only, no new `Op`/`Value`, byte-identical `run≡runvm≡real PHP` (no runtime change). Closes
the M-DX/W1 enforcement-audit gaps and hardens the type system:

- **Override return covariance (`E-OVERRIDE-SIG`)** — a return-type-incompatible override
  (`Sub.k(): string` overriding `open Base.k(): int`) used to type-check clean, then store a
  wrong-typed value on the Rust backends *and* fatal in transpiled PHP. Now rejected: an override's
  return type must be the overridden type or a subtype of it. (Parameter variance + overloaded/generic
  overrides remain documented deferrals.)
- **Duplicate enum variant (`E-DUP-VARIANT`)**, **duplicate `static` field (`E-DUP-STATIC`)**, and
  **duplicate `const` (`E-DUP-CONST`)** — each used to silently overwrite the first in a `HashMap`;
  now rejected, mirroring the existing instance-field `E-DUP-FIELD` check.
- **Uncoded diagnostics given stable codes** — "type X is already defined" → `E-DUP-TYPE`; the
  generic/collection arity errors → `E-TYPE-ARG-COUNT` (so both are `phg explain`-able and greppable).
- **24 previously-undocumented codes now self-document** via `phg explain` (the W1 audit found 14; the
  new **diagnostic-coverage ratchet** found 10 more — all four `E-TYPE-IMPORT-*`, the `E-DECL-*` pair,
  and this slice's new codes).
- **Diagnostic-coverage ratchet** (`every_emitted_diagnostic_code_has_an_explanation`) — a test scans
  non-test `src/` for every emitted `E-*`/`W-*` code and asserts each has a `phg explain` entry, so a
  new code without documentation is a CI failure. The drift-prone hardcoded "known codes" list in the
  `explain` fallback was removed in its favor.
- **Golden-diagnostic corpus** (`conformance/diagnostics/`, gated by `tests/diagnostics.rs`) — each
  case pins the *exact rendered diagnostic* (header, source line, caret, `[CODE]`, `hint:`); regenerate
  with `PHORJ_BLESS=1 cargo test --test diagnostics`.

### Changed — green threads: cooperative cutover **DONE** (M6 W4 / S4.3)

`spawn`/channels are now **genuinely cooperative**, not synchronous-degenerate. A spawned single-overload
free-function call is **deferred** (it no longer runs at `spawn`); each green task runs its own engine
inside a stackful `corosensei` coroutine (native), and a `recv` on an empty channel — or a `join` on an
unfinished task — **suspends** the task until a `send`/completion wakes it. Both backends (tree-walking
`run`, bytecode `runvm`) drive the *same* deterministic `green::sched` scheduler, so task interleaving is
**byte-identical** (`run≡runvm`). New `Op::SpawnCall(func_idx, argc)` (deferrable free-fn spawn);
`Interp` and `Vm` gained an optional coroutine-suspension handle (closure-local, no `unsafe` — the crate
stays `#![forbid(unsafe_code)]`). `spawn consume(ch); send(42)` — which the eager model faulted on — now
prints `got 42`/`done 42` on both backends. **wasm keeps the eager model** (corosensei has no native
stack to switch). Follow-ups (KNOWN_ISSUES): deferral for method/overloaded/closure spawns, cooperative
fault-trace frames, cross-task statics.

### Added — green threads: `spawn` + channels (M6 W4 / S4.3, step 2)

The concurrency **surface and value model** — uncolored cooperative concurrency: `spawn <call>` (a
contextual keyword) starts a green task and evaluates to a `Task<T>` handle; `t.join()` collects its
result; typed `Channel<T>` FIFOs (`Channel.create()`, `ch.send(v)`, `ch.recv()`). New `Value::Channel`
(shared-mutable FIFO handle) / `Value::Task`, the reserved built-in types `Channel<T>`/`Task<T>` (like
`List`/`Map`/`Set`), and five new bytecode ops (`Spawn`/`ChannelNew`/`ChannelSend`/`ChannelRecv`/`Join`).
This slice is the **synchronous-degenerate foundation**: a spawned task runs to completion at `spawn`
(byte-identical by construction — there is no scheduler to drift), so fork-join (`spawn f(); … t.join()`)
works end-to-end and a channel is filled before it is drained. The shared deterministic scheduler that
**interleaves** tasks and **suspends** a blocked `recv`/`join` (kernel `green::sched` already landed) is
the next build step. Green threads have **no PHP target** — `spawn`/channel programs are quarantined from
the PHP oracle and the transpiler emits `E-CONCURRENCY-NO-PHP` (never a misleading synchronous lowering);
`run ≡ runvm` stays fully gated. Guide demo `examples/guide/concurrency.phg`; +6 differential tests
(spawn/join, fork-join arithmetic, channel send/recv, string channel, recv-empty fault parity, `spawn`
still usable as an identifier). New diagnostics: `E-SPAWN-NOT-CALL`, `E-SPAWN-VOID`,
`E-CHANNEL-ANNOTATION`, `E-CHANNEL-NEW-ARITY`, `E-CHANNEL-NEW-TYPE`, `E-CONCURRENCY-METHOD`,
`E-CONCURRENCY-ARITY`, `E-CONCURRENCY-NO-PHP`.

### Dependencies — `corosensei` admitted (4th, feature-gated, for green-thread suspension)

`corosensei` (stackful coroutines, MIT OR Apache-2.0, miri-tested) is admitted under the dependency
policy's 4th domain (`docs/specs/2026-06-27-dependency-policy.md`): suspending a green task deep in the
interpreter/VM call stack needs hand-rolled `unsafe` stack switching that `std` lacks, and the crate
confines that `unsafe` outside phorj's `#![forbid(unsafe_code)]`. Behind the **`green`** feature
(default-on, **non-wasm only** — wasm32 has no native stack; the playground delegates to VM frame-swap).
A gating spike proves the deep-stack suspend works with **no `unsafe` in phorj's own code** (a yielder
borrowed into a lifetime-parameterized worker). The cooperative executor that uses it is the next slice.

### Added — `Core.Text.capitalize` (M4 breadth, charter-compliant)

`Core.Text.capitalize(string) -> string` uppercases the first character when it is an ASCII lowercase
letter (else unchanged) — byte-for-byte PHP `ucfirst`, ASCII-scoped like `upper`/`reverse`. Tier-1,
byte-identical `run ≡ runvm ≡ real PHP`; guide demo in `examples/guide/text.phg`, +1 unit test.

### Added — `Core.Text.lines` (M4 breadth, charter-compliant)

`Core.Text.lines(string) -> List<string>` splits on `\n` (an embedded `\r` stays in the line; an empty
string → `[""]`; a trailing `\n` → a trailing `""`) — `explode("\n", s)` semantics, byte-identical
`run ≡ runvm ≡ real PHP`. Tier-1, subject-first; guide example in `examples/guide/text.phg`, +1 unit
test. No new `Op`/`Value`.

### Added — `Core.List.chunk` (M4 breadth, charter-compliant)

`Core.List.chunk(List<T>, int) -> List<List<T>>` splits a list into consecutive groups of `size` (the
last may be shorter); an empty list yields `[]`. The first charter-era addition: subject-first, Tier-1
deterministic (byte-identity-gated guide example `examples/guide/list-breadth.phg`), and `size < 1`
faults (a programmer error, not `T?`) byte-identically on both backends. Erases to PHP `array_chunk`.
No new `Op`/`Value`.

### Added — M4 standard-library charter (governing policy)

Adopted `docs/specs/2026-06-29-m4-stdlib-charter.md`: the governing policy for every `Core.*` module
across five axes — naming (`Core.<Pascal>` / `camelCase` / `is…` predicates), subject-first argument
order (closure last), the optional-vs-fault-vs-`throws` recoverability rule, the three determinism
tiers (Tier-1 byte-identity-gated, Tier-2 representation-sensitive, Tier-3 quarantined), and the
native-vs-injected-`.phg` decision. Descriptive of the conventions already practised across the 20+
shipped modules and prescriptive for the M11 breadth push, with a quick decision tree. Doc-only.

### Added — Cross-package single inheritance + parent dispatch (M-RT S6/B1a, cross-package)

A `package Main` class can now `extends` a class declared in a library package (imported via
`import type`), inheriting its constructor and fields, overriding its `open` methods, and calling up
with both `parent.m(…)` (nearest ancestor) and the named `parent(Ancestor).m(…)` form — all resolved
across the package boundary. The loader's cross-package resolution pass now mangles the `extends` parent
name (the missing piece) and the `parent(Ancestor)` reference + arguments inside an `Expr::ParentCall`;
the transpiler emits `extends \Acme\Zoo\Animal` and `parent::m()`. Byte-identical
`run ≡ runvm ≡ real PHP 8.5` over a two-level chain (`examples/project/inherit/`, +2 project tests).
Cross-package *multiple* inheritance remains out of scope.

### Fixed — `Core.Json` in multi-package projects + cross-package map literals

A multi-package project that imports `Core.Json` now round-trips byte-identically
`run ≡ runvm ≡ real PHP`. Two PHP-emission/loader fixes: (1) the injected `Json` enum is a
`package Main` type, so in a namespaced program its variant classes live in `\Main\`; the JSON runtime
helpers (emitted in the global block) referenced them by bare name (`instanceof Obj`), so every
`instanceof` missed and stringify/parse fell through — they now reference `\Main\Obj` etc. when
namespaced. (2) The loader's cross-package resolution pass had no `Expr::Map` arm, so a qualified call
or cross-package type nested in a map literal `[k => v]` was left unresolved (`E-UNKNOWN-IDENT`); it now
descends both key and value, like a list. `run`/`runvm` were already correct — both are
PHP-emission/loader-only fixes. New example `examples/project/jsonmulti/`.

### Added — Lambdas + first-class function values in library packages (M3 S3, cross-package)

A same-package function reference inside a *library* (non-`main`) package now resolves in **every**
position: at a call site (already worked), inside a lambda body (`fn(int x) => dbl(x)`), and — the new
case — in **value position** (`var f = dbl;`, or passing `dbl` to a higher-order call). The loader's
`Expr::Ident` value-resolution arm now mangles a bare same-package function reference to its package
FQN, mirroring the call-site path; for `package Main` the mangle is a no-op, so single-file programs
stay byte-identical. Verified `run ≡ runvm ≡ real PHP 8.5` (`examples/project/funcvalues/`). Qualified
cross-package function *values* (passing `Acme.Calc.dbl` itself vs. calling it) remain deferred.

### Added — Cross-package traits (M-RT S8, cross-package)

A `trait` declared in a library package can now be composed into a class in another package. It is
imported with the terminal `import type Pkg.Path.Trait [as A];` form (a trait stays NOT a type —
`Trait x` as an annotation is still `E-USE-AS-TYPE`) and composed with `use Trait;`. No backend change
— the loader registers traits in its type symbol table and mangles both the trait declaration and the
class's `use` clause to the same FQN, so the checker's by-name trait flatten and the transpiler's
emission line up. The transpiler now also detects, buckets, and emits a `\`-mangled trait into its
package `namespace` block; the using class composes it via a fully-qualified `use \Acme\Mix\Greet`.
Method reuse, a private trait helper, and an abstract requirement satisfied by the using class all work
byte-identically `run ≡ runvm ≡ real PHP 8.5` (`examples/project/mixins/`). Lifts the prior
`package Main`-only note in `KNOWN_ISSUES.md`.

### Added — Cross-package generic library types (M-RT generics-all, cross-package)

A generic class declared in a *library* package (`Box<T>`, `Pair<A, B>`) is now a validated,
example-gated surface: it is consumed from another package via `import type Pkg.Path.Type`, its type
parameter is inferred at construction and recovered at each use site, and type arguments are invariant
across the package boundary. No new machinery — the loader leaves the type parameter untouched and
`erase_generics` removes it before any backend, so it rides the same erasure path as a `package Main`
generic class. Byte-identical `run ≡ runvm ≡ real PHP 8.5`, gated by the project-aware differential
harness (`examples/project/genericbox/`). Lifts the prior "untested" note in `KNOWN_ISSUES.md`.

### Added — LSP cross-file go-to-definition + hover

The language server (`phg lsp`) now resolves **go-to-definition and hover across the open buffer set**: a
name that resolves to neither a local nor a same-file top-level symbol is looked up in the other open
documents (a same-package sibling file), and the jump/hover targets that file. Same-file resolution
always wins; other buffers are scanned in sorted-uri order for determinism. The VSCode and JetBrains
(LSP4IJ) clients consume this transparently — no client change. The server stays off the byte-identity
spine. Cross-file *references* (which need project-aware file merging to stay scope-accurate) remain a
documented follow-up.

### Added — M-RT super/parent dispatch (B2: multiple inheritance, transpiler trait aliasing)

`parent(A).m(…)` / `parent.m(…)` now transpile correctly when the calling class has **multiple
inheritance** (or is a trait-decomposed ancestor of one). The `run`/`runvm` backends already dispatched
these (B1a's `Op::CallParent` + the MI-aware resolver); the gap was PHP emission — a multiple-inheritance
class has no native PHP parent, so `parent::m()`/`A::m()` was invalid. Byte-identical
`run ≡ runvm ≡ real PHP 8.5` (`examples/guide/parent-dispatch-mi.phg`).

- **Lowering** — a parent-method call inside an MI class (`emit_multi_class`) or a decomposed trait body
  (`emit_decomposed_class`) is rewritten to a `private` trait alias: the `use` block gains
  `T<dp>::m as private __super_<dp>_<m>;` and the call becomes `$this->__super_<dp>_<m>(…)`, where `dp`
  is the direct parent (named ancestor, or the single direct provider for the bare form). Verified
  against real PHP 8.5 (aliasing requires the aliased trait to be *directly* `use`d — which holds for a
  direct parent). A read-only AST walk (`collect_parent_method_calls`, mirroring the complete
  `rewrite_new` walker) finds every call so the `use` block declares exactly the aliases needed.
- **Scope** — direct-parent targets. A jump to a **non-direct** ancestor under MI (`parent(G).m()` where
  `G` is reached through an MI arm) is not yet lowerable (PHP can't alias a transitively-`use`d trait
  method) and is a **clean transpile error**, not invalid PHP — the `run`/`runvm` backends still handle
  it. Single-inheritance parent calls are unchanged (native `parent::`/`A::`). No backend (`run`/`runvm`)
  change; programs without MI parent calls are byte-identical.

### Added — M-RT super/parent dispatch (B1b: parent-constructor forwarding, single inheritance)

`parent.constructor(…)` / `parent(A).constructor(…)` — run the parent constructor's effect on the
**existing** instance, so a subclass that declares its own constructor can finally initialize inherited
state (closes the own-ctor-under-inheritance gap). Byte-identical `run ≡ runvm ≡ real PHP 8.5`
(`examples/guide/parent-constructor.phg`).

- **Lowering** — pure front-end *inlining* (`checker::inline_parent_ctors`, runs LAST in
  `cli::check_and_expand`): the forwarding statement is replaced by a fresh-scoped `Stmt::Block` that
  reproduces one constructor "plan entry" for the resolved parent — parameter bindings, promotions, the
  parent's own field initializers, then its body (recursively inlined for grandparent chains). The same
  lowered AST feeds every backend, so byte-identity holds by construction. **No new `Op` or `Value`.**
- **Resolution** — single inheritance: immediate `parent.constructor(…)` targets the direct parent;
  `parent(A).constructor(…)` targets a named transitive ancestor. The effect comes from the nearest
  ancestor that declares a constructor (PHP's inherited `__construct`).
- **Position** — statement-only, inside a constructor body (so every occurrence is inlined and the
  backends never see a `ParentCall{constructor}`).
- **Errors** `E-PARENT-CTOR-OUTSIDE` (not in a constructor) / `E-PARENT-CTOR-STMT` (used as a value) /
  `E-PARENT-CTOR-MI` (bare form under multiple inheritance) — plus the shared `E-PARENT-NO-PARENT` /
  `E-PARENT-NOT-ANCESTOR`. All `phg explain`-documented.
- Scope (B1b): single inheritance. Deferred: multiple-inheritance constructor forwarding (per-parent
  `parent(P).constructor(…)`) lands with B2. See `KNOWN_ISSUES.md`.

### Added — M-RT super/parent dispatch (B1a: methods, single inheritance)

`parent.m(…)` / `parent(A).m(…)` — invoke an inherited method an override shadows (or jump to a named
ancestor). Spec `docs/specs/2026-06-28-super-parent-dispatch-design.md`. Closes part of the
inheritance gap (a child override can now reuse + extend its parent's behaviour). Byte-identical
`run ≡ runvm ≡ real PHP 8.5` (`examples/guide/parent-dispatch.phg`).

- **Syntax** — `parent` is a contextual keyword, recognized only as a call head (`parent.` / `parent(`);
  immediate `parent.m(…)` (nearest declaring ancestor) and qualified `parent(A).m(…)` (a C++-style jump
  to any transitive ancestor). New `Expr::ParentCall`.
- **Resolution is lexical + single-sourced** — a new `ast::resolve_parent_method` (over `class_mro` +
  `class_method_origins` + direct parents) is shared by the checker (errors + typing), the interpreter
  (dispatch), and the compiler (bakes the target), so `run ≡ runvm` by construction. Resolution is
  relative to the class that *writes* the call (the lexical/declaring class), not the receiver's runtime
  class — so an override reaches the version it shadows.
- **Backends** — one new VM `Op::CallParent(func_idx, argc)` (non-virtual: a baked target, same frame
  layout as `CallMethod`); the interpreter threads a lexical `cur_class` through `run_call`. Transpiles
  to native PHP `parent::m(…)` (immediate) / `A::m(…)` (named ancestor). A parent-call result is a
  first-class typed value (`parent.m(…) + 1` specializes on the VM — the compiler's `ctype` resolves it
  via `method_rets`).
- **Errors** `E-PARENT-OUTSIDE-METHOD` / `-NO-PARENT` / `-NOT-ANCESTOR` / `-NO-METHOD` / `-AMBIGUOUS`
  (the last MI-only), all `phg explain`-documented.
- Scope (B1a): methods, single inheritance. Deferred: `parent.constructor(…)` (B1b — the parent ctor
  body must run on the existing instance) and multiple inheritance + the multi-of-multi trait lowering
  (B2). See `KNOWN_ISSUES.md`.

### Added — M-RT return-type overloading (Slice C1)

Free functions may now overload on **return type alone** — identical parameter signatures, differing
returns (`function read(string): int` / `function read(string): bool`). Spec
`docs/specs/2026-06-28-must-use-and-return-type-overloading-design.md`; the must-use slice (`discard` /
`E-UNUSED-VALUE`) was its enabler. **No new `Op`/`Value`** — front-end only, byte-identical
`run ≡ runvm ≡ real PHP 8.5` (`examples/guide/return-overloading.phg`).

- **`<Type>f(args)` overload selector** — a new prefix expression (`Expr::OverloadSelect`) at operand
  position naming which overload's return type to select. It is NOT a value cast (`as` is). Parses
  cleanly (a leading `<` cannot begin an operand otherwise); nested generics need no special handling
  (`>>` already lexes as two `Gt`). `discard <Type>f(…)` drops the result of a side-effecting call.
- **Resolution** (compile-time, by the checker): exact return-type match → unique assignable match →
  else `E-OVERLOAD-AMBIGUOUS-RETURN`. A selector naming no overload's return type (or on a
  non-return-overloaded callee) is `E-OVERLOAD-SELECT-UNKNOWN`; a bare return-overloaded call with no
  type context is `E-OVERLOAD-NO-CONTEXT`.
- **Mangle-before-backends** — each return-overload member's definition is renamed to a distinct name
  (`read__ret_int` / `read__ret_bool`) and the resolved call sites rewritten to match (reusing the
  span-keyed call-rewrite map applied by `rewrite_ufcs` + a new `rename_overload_defs` pass), so the
  interpreter / VM / transpiler see ordinary single-overload functions. Single-return names stay bare —
  existing programs are byte-identical.
- `E-OVERLOAD-RETURN` repurposed: it no longer means "must share a return type" but "a name mixes
  parameter- and return-type overloading" (the parameter-overload shared-return rule is kept). All four
  new codes self-document via `phg explain`.
- **C2 sink-widening** (same change): a **typed binding** (`int x = read("k")`) and a **`return`**
  (`function port(): int { return read("k"); }`) now supply the resolving type context directly — no
  selector needed in those positions. A `var x = …` inference has no context (`E-OVERLOAD-NO-CONTEXT`),
  and a declared type assignable from no overload's return is `E-OVERLOAD-AMBIGUOUS-RETURN`. The
  resolution core is shared with the selector (exact → unique-assignable → error). Scope: free
  functions; remaining sinks (typed reassignment / field write / argument-to-non-overloaded-parameter)
  still need a selector. `E-OVERLOAD-SELECT-CONFLICT` remains reserved. See `KNOWN_ISSUES.md`.

### Added — M8.5 S3: `.d.phg` declaration files + foreign-exception `catch`

The interop bridge's final slice (`docs/specs/2026-06-28-m8.5-s3-decl-files-foreign-catch-design.md`).
**No new `Op`/`Value`** — foreign symbols stay PHP-target-only (quarantined from `run ≡ runvm`), so this
is a front-end + transpiler feature; pure-Phorj spine untouched.

- **Foreign-exception `catch` (S3a)** — a `declare class` now accepts an optional `extends`/`implements`
  header. A foreign PHP exception writes `declare class DivisionByZeroError implements Error { … }` —
  `Error` is Phorj's built-in exception marker, so the class becomes catchable. It is caught by its own
  **global** PHP name (`catch (\DivisionByZeroError $e)`), NOT the `Error`→`\Exception` mapping, so an
  `\Error`-family class (a `\Throwable` that is not an `\Exception`) is caught correctly. The transpiler's
  catch-type emission is now foreign-aware (`php_catch_type` is a method consulting `foreign_classes`);
  `phg fmt` round-trips the `extends`/`implements` header. `examples/interop/exceptions.phg`.
- **`.d.phg` ambient declaration files (S3b)** — a file whose name ends `.d.phg` holds only `declare`s,
  carries **no `package`**, and is loaded ambiently into a project (the `.d.ts` analog): its presence
  under the source root makes the foreign symbols available to every file, declared once, with no
  `import`. New loader guards `E-DECL-PACKAGE` (a decl file must not declare a package) / `E-DECL-NONFOREIGN`
  (only `declare` items). A `.d.phg` is excluded from the ordinary `.phg` walk (never folder=path-validated)
  and its foreign items merge unmangled (the cross-package name-mangle pass now skips every foreign item —
  a global PHP symbol must never become a package-FQN). `examples/interop/withdecls/` (a `.d.phg` shared
  across `Main` + a library package), validated by a project-aware `tests/interop.rs` (load → refuse →
  transpile-golden). **M8.5 is now COMPLETE** (S1 functions + S2 classes + S3 decl-files & foreign catch).

### Added — M4 stdlib: `Core.List.take` / `drop`

Prefix/suffix slicing, byte-identical `run ≡ runvm ≡ real PHP 8.5`, **no new `Op`/`Value`**:
`List.take(xs, n)` (first `n`) and `List.drop(xs, n)` (skip `n`), each clamping `n` to `[0, len]`
(`n < 0 ⇒ 0`, `n > len ⇒ len`) so they never fault. Erase to `array_slice($xs, 0, max(0, $n))` /
`array_slice($xs, max(0, $n))` (the `max(0, …)` clamps a negative `n`, else `array_slice` would count
from the end). `guide/list-breadth.phg` + `conformance/collections/list-query.phg` extended.

### Changed — M-perf: FNV-hashed instance field maps

Instance field storage (`value::Instance.fields`) now uses a hand-rolled **FNV-1a** `BuildHasher`
(`value::FnvHasher` / `type FieldMap`) instead of std's DoS-resistant SipHash. Field keys are short,
source-derived identifiers (never attacker-controlled), so SipHash's keying overhead bought nothing;
FNV-1a is a few XOR/multiply per byte. **Measured** (`phg bench`, median-of-101): object-heavy workload
**VM 15.17 ms → 12.82 ms (~15.5% faster)**; the mixed `examples/bench/workload.phg` **1.60 ms → 1.48 ms
(~7%)**. Semantics are identical (same `HashMap` API; field-iteration order never reached output — it was
already `RandomState`-randomized per process, yet `run ≡ runvm ≡ PHP` held). Std-only, safe, no new
`Op`/`Value`; full PHP-8.5 oracle still byte-identical.

### Added — M4 stdlib: `Core.Text` breadth (reverse + case-insensitive)

Three ASCII-oriented `Core.Text` natives (charter Rule 5 Tier-A — each maps to a PHP **core** function
under `-n`), byte-identical `run ≡ runvm ≡ real PHP 8.5`, **no new `Op`/`Value`**:

- `Text.reverse(string) -> string` (→ `strrev`) — reverses by chars (== bytes for ASCII).
- `Text.equalsIgnoreCase(string, string) -> bool` (→ `strcasecmp(...) === 0`).
- `Text.containsIgnoreCase(string, string) -> bool` (→ `stripos(...) !== false`).

ASCII folding only (no mbstring under `php -n`); non-ASCII is a documented edge (KNOWN_ISSUES).
`guide/text.phg` extended + `conformance/stdlib/text-breadth.phg`.

### Added — editor tooling: syntax highlighting + JetBrains/PhpStorm integration

- **TextMate grammar** (`editors/vscode/syntaxes/phorj.tmLanguage.json`) — keywords, primitive +
  PascalCase types, strings with `{…}` interpolation and `\xHH`/`b"…"`/`r"…"` forms, numeric literals
  (hex/bin/oct/`_`/`1.50d`), comments, and `#[…]` attributes. Wired into the VS Code extension
  (`grammars`), which previously had only bracket config — `.phg` files are now highlighted.
- **VS Code extension v0.2.0** — the thin `phg lsp` client auto-gains the new server capabilities
  (references/rename/formatting/highlight); README + manifest refreshed.
- **JetBrains / PhpStorm** (`editors/phpstorm/`) — a no-compile path: the `editors/vscode/` directory is
  a native **TextMate Bundle** for highlighting, and **LSP4IJ** runs `phg lsp` for the full feature set.
  One server + one grammar, identical behavior across editors. A natively-compiled JetBrains plugin is a
  tracked follow-up.

### Added — LSP: references, document-highlight, rename, formatting

The `phg lsp` server gains four capabilities beyond diagnostics/hover/definition/completion/symbols —
all front-end-only, off the byte-identity spine:

- **`textDocument/references`** + **`textDocument/documentHighlight`** — every use of the symbol under
  the cursor (declaration included), via a shared **scope-accurate** `occurrences` engine: same-name
  identifiers filtered to those resolving to the *same declaration* (a shadowing local elsewhere is
  excluded), reusing the existing `resolve_decl`.
- **`textDocument/rename`** — a `WorkspaceEdit` renaming every occurrence (scope-accurate).
- **`textDocument/formatting`** — a whole-document edit from `crate::fmt::format`, so editor-format
  equals `phg fmt`; returns no edit if the buffer doesn't parse (never corrupts an in-progress file).

Advertised in `initialize`; six new LSP tests. Single-document (cross-file references are a follow-up).

### Added — public-surface file-naming rule + order-independent type resolution

Design `docs/specs/2026-06-28-public-surface-file-rule-design.md`. **No new `Op`/`Value`** (loader +
checker front-end only; the byte-identity spine is untouched).

- **Public-surface rule** (loader, project mode): a non-`main` file's public face is exactly **one
  public named type** (class/enum/interface/trait — file stem must equal it, byte-exact incl. casing)
  **or** public free functions (topic-named) — never both, never two public types. `private`/`internal`
  helper types + functions and `declare` (foreign) items ride along free; a file declaring `main` is
  exempt (programs mix freely). New codes `E-FILE-NAME` / `E-FILE-MULTI-PUBLIC` / `E-FILE-MIXED-PUBLIC`
  (+ `phg explain`). "Go packages, PSR-4 public-type files." Loose single-file + `-e`/stdin are
  `main`-only ⇒ exempt; every guide example has `main` ⇒ zero guide churn. The `examples/project/shapes`
  and `…/visibility` library packages were split to one-type-per-file (`Shape.phg`/`Rect.phg`/`Paint.phg`),
  and the `ddd` conformance project too (`Money.phg`/`Product.phg`/`OrderLine.phg`/`Order.phg`).
- **Order-independent type resolution** (checker `prebind_types` pre-pass): all top-level type names are
  registered (with generic arity) *before* any member type is resolved, so a **forward reference**
  (`function toB(): B` where `B` is declared later) and a **cross-file reference** (a sibling merged
  earlier by the loader's alphabetical sort) both resolve. This was a real limitation — it previously
  forced prelude/source ordering (the M-TIME `Duration → Date → Instant` workaround) and would have made
  the file-splitting rule painful. Duplicate + built-in-redefinition detection is preserved (now
  order-independent).
- **Fix (`phg fmt`):** the printer dropped top-level declaration visibility (`internal`/`private` on a
  free function / class / enum / interface — only `public`, the default, was correctly elided). It now
  round-trips them; regression-tested. (Found because formatting a split library file silently turned an
  `internal function` public, tripping `E-FILE-MIXED-PUBLIC`.)

### Added — M8.5 S2: foreign-PHP classes (`declare class`)

Foreign PHP **classes** — call a PHP library class (e.g. `DateTimeImmutable`, `PDO`) from Phorj,
type-checked, transpiling to idiomatic PHP. **No new `Op`/`Value`.**

- **`declare class Name { constructor(params); [static] function m(params) -> ret; [public] Type f; }`**
  — bodyless member signatures. Construction transpiles to `new \Name(...)`, an instance method to
  `$o->m(...)`, a static method to `\Name::s(...)`, a field read to `$o->f`; the class emits no PHP
  definition. The checker skips body/totality/definite-assignment for a foreign class (its bodies live
  in PHP) but registers it for member-call resolution, so `new`, method, and static calls type-check.
- Member names keep their real PHP spelling (casing-exempt); the class name stays PascalCase. `phg fmt`
  round-trips `declare class`. `examples/interop/classes.phg` (a `DateTimeImmutable` walkthrough, gated by
  `tests/interop.rs`). **M8.5 is now CORE COMPLETE** (S1 functions + S2 classes); `.d.phg` declaration
  files and foreign-exception `catch` (S3) remain deferred.

### Added — M8.5 S1: foreign-PHP interop (`declare function`)

The migration bridge — call existing PHP from Phorj, type-checked, transpiling to idiomatic PHP
(`docs/specs/2026-06-28-m8.5-interop-design.md`). `Phorj : PHP :: TypeScript : JavaScript`, and
`.d.phg : .d.ts`. **No new `Op`/`Value`.**

- **`declare function name(params) -> ret;`** — a bodyless signature for an existing PHP function
  (contextual `declare`, not a reserved word). Its name is the real PHP name (snake_case like
  `str_repeat` is allowed — the camelCase rule is waived for foreign symbols). The checker type-checks
  calls against it; it emits **no** PHP definition; a call transpiles to the global form `\name(...)`.
- **The byte-identity spine is untouched.** Foreign PHP only exists in the PHP runtime, so a program
  containing any `declare` is **PHP-target-only**: `check` and `transpile` work, but `run`/`runvm` refuse
  with one clean pre-flight gate (**`E-FOREIGN-RUNTIME`** — `phg explain` it). Such programs are
  quarantined from the `differential.rs` byte-identity oracle and validated by a new **`tests/interop.rs`**
  harness (transpile → real PHP → golden output) plus the refuse-gate assertion.
- `examples/interop/builtins.phg` (+ README, excluded from the differential glob); `phg fmt` learns the
  `declare` surface. **`declare class` and `.d.phg` files are S2/S3.**

### Added — M-TIME S3: civil (wall-time) view + ISO-8601

The human date-time view, **folded onto `Instant`** (no separate class), byte-identical
`run ≡ runvm ≡ real PHP 8.5`, **no new `Op`/`Value`**:

- `Instant.ofCivil(y, mo, d, h, mi, s)` builds an instant from broken-down UTC fields.
- `year`/`month`/`day`/`dayOfWeek`/`hour`/`minute`/`second`/`millis`/`millisOfDay` accessors (UTC).
- `toIso()` → `YYYY-MM-DDTHH:MM:SSZ` (always `Z`, second resolution). For any other layout, interpolate
  the accessors directly — Phorj has first-class string interpolation, so a printf-style pattern is
  unneeded (deferred in KNOWN_ISSUES).

`guide/datetimes.phg` + `conformance/stdlib/datetimes.phg`. **Design note:** the planned separate
`DateTime` class was dropped — the name collides with PHP's built-in `DateTime` (a `package Main` class
emits to the global PHP namespace → `Cannot redeclare class`), and `Instant` already *is* the point in
time, so the civil fields live on it. **M-TIME is now COMPLETE** (S1 instants+durations, S2 dates, S3
civil view).

### Added — M-TIME S2: `Core.Time` civil dates

`Date` — a civil calendar date (UTC, day-resolution), stored as days since 1970-01-01. Calendar math is
Howard Hinnant's days-from-civil / civil-from-days, written in **pure Phorj** in the same injected
prelude, so it is byte-identical `run ≡ runvm ≡ real PHP 8.5` by construction. **No new `Op`/`Value`.**

- `Date.of(y, m, d)` / `Date.ofEpochDay(n)`; `year`/`month`/`day`/`epochDay`.
- `addDays`/`minusDays`/`daysUntil`; `dayOfWeek()` (1=Mon … 7=Sun, ISO-8601); `isLeapYear()`.
- `isBefore`/`isAfter`/`compareTo`; `toString()` → `YYYY-MM-DD` (year zero-padded to 4).
- `Instant.toDate()` bridges an instant to its UTC civil date (floor-divides millis by a day).

`guide/dates.phg` + `conformance/stdlib/dates.phg`. **Gotcha found + worked around:** a method
return-type annotation cannot forward-reference a class declared *later* in the same compilation unit
(`E-UNKNOWN-TYPE`); the prelude is ordered `Duration` → `Date` → `Instant` so every `-> Type` refers to
an already-declared class.

### Added — M-TIME S1: `Core.Time` instants + durations

First slice of the time library (`docs/specs/2026-06-28-m-time-design.md`), byte-identical
`run ≡ runvm ≡ real PHP 8.5`, **no new `Op`/`Value`**:

- **`Instant`** — a point in time (epoch-millis, UTC): `Instant.now()` (clock seam),
  `ofEpochMillis`/`ofEpochSeconds`; `epochMillis`/`epochSeconds`, `plus`/`minus` (a `Duration`),
  `durationSince`, `isBefore`/`isAfter`/`compareTo`.
- **`Duration`** — a span: `Duration.seconds`/`minutes`/`hours`/`days`/`millis`; `toMillis`/`toSeconds`/
  `toMinutes`/`toHours`/`toDays`, `plus`/`minus`/`negate`, `isZero`/`isNegative`.
- **Architecture** — an **injected pure-Phorj prelude** (`cli::inject_time_prelude`, gated on
  `import Core.Time`): because the prelude runs through the same backends *and* transpiler as user code,
  all arithmetic is byte-identical by construction with zero hand-rolled-PHP divergence. The only native
  (`src/native/time.rs`, `Core.Time`) is the **freezable clock seam** — `Time.freeze(ms)` /
  `Time.unfreeze()` / `Time.nowMillis()`, hand-rolled identically in PHP (`__phorj_now_*`), so a frozen
  program is reproducible (the `Core.Random` determinism pattern). UTC-only (timezones are
  non-deterministic). `guide/time.phg` + `conformance/stdlib/time.phg`.

### Added — stdlib: `Core.Set` + `Core.Map` ergonomics (collection breadth complete)

Completes everyday collection breadth (List/Set/Map), byte-identical `run ≡ runvm ≡ real PHP`, no new
`Op`/`Value`:

- **`Core.Set`** += `add(s, x)` / `remove(s, x) -> Set<T>` (immutable; no-op if already present /
  absent) and `isSubset(a, b) -> bool` (union/intersection/difference already shipped).
- **`Core.Map`** += `getOr(m, k, default) -> V` (safe access — returns `default` for a missing key,
  never faults; and unlike `get`/`??` it returns a *present* key's value even when null),
  `merge(a, b) -> Map<K,V>` (a shared key takes `b`'s value at `a`'s position, `b`'s new keys append —
  ≡ PHP `array_merge` / `build_map` over `a ++ b`), and higher-order `map(m, (V)->W) -> Map<K,W>` /
  `filter(m, (V)->bool) -> Map<K,V>` over **values** (keys preserved). Each erases to a PHP array
  builtin. `examples/guide/collection-ergonomics.phg` + `conformance/collections/set-map-ergonomics.phg`.

### Added — stdlib: `Core.List` breadth (query/aggregate)

Six everyday `Core.List` ops, all byte-identical `run ≡ runvm ≡ real PHP`:

- **`unique(List<T>) -> List<T>`** — dedupe keeping first occurrence + order (value equality).
- **`min` / `max`(List<T>) -> T?`** — smallest / largest, null for an empty list. Strings order by
  **byte** (`"10" < "9"`), matching the Rust backends — *not* PHP's numeric-string juggling.
- **`find(List<T>, (T) -> bool) -> T?`** — first element satisfying the predicate, or null.
- **`any` / `all`(List<T>, (T) -> bool) -> bool`** — short-circuiting existential / universal.

`find`/`any`/`all` **short-circuit identically on every backend** (the `__phorj_find/any/all` PHP
helpers `foreach` + early-`return`), so a side-effecting predicate produces identical stdout; `unique`/
`min`/`max` get `__phorj_*` helpers too (inlining PHP `array_unique`/`min`/`max` would juggle numeric
strings). Reuses the higher-order-native + generic-call machinery — no new `Op`/`Value`.
`examples/guide/list-breadth.phg` + `conformance/collections/list-query.phg`.

### Added — M6 W3: concurrent `phg serve` (bounded thread pool)

`phg serve` now handles requests concurrently across CPU cores instead of one at a time. Each request
runs on its own worker thread with its **own `Rc` `Value` heap** — values never cross threads, so the
non-`Send` heap is no obstacle; only the immutable `ast::Program` is shared (verified `Send + Sync`).
No new `Op`, no new `Value`, the single-threaded `Rc` hot path untouched, std-only, no `unsafe`.

- **`--workers N`** sets request concurrency; default = number of CPU cores
  (`available_parallelism`); `--workers 1` is the original single-threaded server (its exact path,
  unchanged). The main thread `accept()`s and hands each connection to the pool over a **bounded
  channel** (capacity = workers) — when all workers are busy the accept loop blocks, giving natural
  backpressure (no unbounded thread spawn, no dropped connection). A worker panic is caught
  (`catch_unwind`) so one bad request never shrinks the pool.
- This **supersedes the documented "green-threads" plan** — research showed thread-per-request is
  feasible (and superior: real multi-core vs. green-threads' single core + unstable/unsafe std
  machinery). Design `docs/specs/2026-06-28-m6-w3-serve-concurrency-design.md`. Serve stays outside the
  byte-identity spine; `tests/serve.rs` gains a real-socket concurrency test (24 clients / 4 workers).

### Added — M6 W2 extensions: `#[Route]` on class methods (W2-ext complete)

`#[Route(...)]` may now annotate a **static** class method, so a class is a tidy namespace of route
handlers (the controller shape). `Http.autoRouter()` collects `#[Route]` static methods (alongside
`#[Route]` free functions) and compile-time-desugars each into a registration whose handler is a
`fn(Request req) => ClassName.method(req)` lambda — no runtime reflection. Byte-identical
run≡runvm≡real PHP.

- The attribute parser now accepts `#[…]` on class methods (a `#[…]` on a constructor/field/hook is
  `E-ATTR-TARGET`); a non-`static` `#[Route]` method is `E-ROUTE-METHOD-STATIC` (an instance
  controller has no routable receiver this slice). `phg explain E-ROUTE-METHOD-STATIC`.
- `examples/web/controller.phg` + `conformance/web/controller.phg`.

This **completes the M6 W2 extensions** milestone (middleware + groups → constraints → method
attributes). Still deferred: optional segments / wildcards, instance-controller routing, and the W3
serve/concurrency runtime.

### Added — M6 W2 extensions: regex/typed route constraints

A `{name:regex}` route pattern segment captures `name` only when the path component matches the regex,
anchored to the whole segment (`^(?:regex)$`, via `Core.Regex`). `r"/users/{id:\d+}"` matches
`/users/42` but not `/users/ada`. Precedence is **literal > constrained param > bare param**, so a
constrained route is preferred over a bare `{name}` but still loses to an exact literal. A constrained
segment whose component fails its regex makes the whole route not match (it falls through to the next).
The router prelude now imports `Core.Regex`. `examples/web/route-constraints.phg` +
`conformance/web/route-constraints.phg`, byte-identical run≡runvm≡real PHP (ASCII patterns).
**Gotcha fixed:** a constraint regex may contain braces (`\d{4}`), so the `{name:…}` inner text is
extracted by dropping only the **outer** braces (`Text.substring(seg, 1, -1)`), not by stripping every
`{`/`}`.

### Added — M6 W2 extensions: router middleware + route groups

The `Core.Http` `Router` gains a middleware pipeline and sub-router groups — pure Phorj over
first-class functions, **no new `Op`, no new `Value`**, byte-identical `run ≡ runvm ≡ real PHP`.

- **Middleware** — `router.use(mw)` where `mw : (Request, next) -> Response`. A middleware may call
  `next(req)` to continue the chain (and post-process the result) or **short-circuit** by returning a
  `Response` without calling `next` (e.g. a 401 from an auth middleware). Applied outermost-first to
  every matched handler, composed as `fn(req) => mw(req, next)` folded over the list.
- **Route groups** — `router.group(prefix, build)` runs the `(Router) -> Router` builder on a fresh
  sub-router, then merges each sub-route with `prefix` prepended and the group's own middleware
  composed around its handler. The parent's `use` middleware still applies on top.
- `Router` is now two-field (`table` + middleware); the `Http.autoRouter()` desugar and the router
  examples/conformance build it as `new Router([], [])`. `examples/web/middleware.phg` +
  `conformance/web/middleware.phg` showcase a logging + auth stack and an `/admin` group.

### Fixed

- **VM-compiler: a native-qualified call or a static-method call used as an arithmetic operand / a
  function value.** `List.length(xs) - 1` (and `Module.fn(...) <op> n`) compiled on the interpreter
  but failed on the VM (`undefined variable \`List\``); likewise a `var f = Class.staticFn(...)` whose
  result is a function then failed `f(x)` as "not a function". `ctype`'s `Call`→`Member` arm now
  resolves native-qualified and static-method calls to their return `CTy` (a new `ty_to_cty`/
  `native_ret_cty`), closing two latent `run`↔`runvm` breaks (the documented CTy-operand trap).
  Regression: `conformance/lang/native-operand.phg`.

### Added — M2.5 Phase 3a: cross-stub registry (distributed `phg build --target`)

A **distributed** (sourceless) `phg` can now `build --target <triple>` / `--all` for the Phase-2 cross
targets by downloading a prebuilt runtime stub from the release registry, verifying it, caching it, and
embedding the program — closing the Phase-2 "needs a source checkout" limitation. No signing yet
(Phase 3b); no new runtime dependency.

- **`bundle/sha256.rs`** — hand-rolled FIPS-180-4 SHA-256 (std-only, same ethos as the CRC-32),
  known-vector tested; cross-checked against the host `sha256sum` on a real binary in the tests.
- **`bundle/manifest.rs`** — the per-target sha256 manifest (tolerant line parser, `lookup`,
  `registry_base` via `Cargo.toml` `repository` + version, `PHORJ_STUB_REGISTRY`/`PHORJ_STUB_MANIFEST`
  overrides, the `phg-stub-<triple>` asset-name convention).
- **`build.rs`** — bakes `PHORJ_BAKE_STUB_MANIFEST` into the binary (empty when unset), breaking the
  stub↔manifest circularity so cross stubs have manifest-independent, stable hashes.
- **`bundle/cross.rs`** — the cache-miss path is now a 3-way branch: cache hit → local `cargo-zigbuild`
  (source checkout) → **download + sha256-verify + cache** (distributed). Verify-before-cache: a
  tampered/partial download never poisons the cache. Transport is `curl` for `http(s)` (std has no TLS;
  `PHORJ_CURL` override) and `fs::copy` for `file://`/local (the hermetic-test path).
- **`.github/workflows/stub-registry.yml`** — a 2-pass, secret-free CI workflow (build stubs env-unset
  → hash → bake manifest into the Linux primary → publish), complementing the existing `release.yml`
  human archives.
- **Tests:** `tests/registry.rs` (hermetic client: verify/cache, tamper-rejection, missing entry/asset,
  cross-implementation hash check) + a toolchain-gated `tests/build.rs` end-to-end (real musl stub →
  download → verify → embed → run, byte-identical to `runvm`). No user-visible flag change. Phase 3b
  (signing + macOS stub) deferred — see KNOWN_ISSUES.

### Added — M6 W2 `#[Route(...)]` attributes

A PHP-8-style **attribute** surface — `#[Route("GET", r"/users/{id}")]` on a handler — that
**desugars at compile time** into explicit router registration. No runtime reflection, no new `Op`,
no new `Value`; byte-identical `run ≡ runvm ≡ real PHP`.

- **New front-end surface:** the lexer gains a `#[` token; the parser accepts item-level
  `#[Name(args)]` groups on **free functions** (other targets are `E-ATTR-TARGET`); `FunctionDecl`
  carries the parsed `Attribute`s (front-end-only — no backend reads them).
- **Checker validation:** only `#[Route]` is recognized (`E-UNKNOWN-ATTRIBUTE` for any other name);
  a `Route` needs exactly two string-literal args (`E-ROUTE-ARGS`), a non-empty method + `/`-leading
  path (`E-ROUTE-SPEC`), and a one-parameter handler that returns a value (`E-ROUTE-HANDLER`). All
  five codes self-document via `phg explain`.
- **Compile-time desugar:** `Http.autoRouter()` is lowered (before the type-checker, in the injection
  chain) into `new Router([]).route(...).route(...)` — one `.route` per `#[Route]` handler, each
  referenced as a first-class function value — so every backend sees the same explicit registration.
  `examples/web/router-attrs.phg` + `conformance/web/router-attrs.phg` (golden identical to the
  explicit `router.phg` form). Patterns with `{name}` must be raw strings (`r"/users/{id}"`).

### Added — M6 W2 HTTP router + path parameters

`import Core.Http;` now also injects a **`Router`** (+ a `Route` row type): build it by chaining
`.route(method, pattern, handler)` — handlers are ordinary first-class `(Request) -> Response`
functions — then `router.handle(req)` matches and dispatches. Pure Phorj over the W1 model (no new
`Op`, no new `Value`, no socket — that is W3 `phg serve`); byte-identical `run ≡ runvm ≡ real PHP`.

- **Path parameters** — a `{name}` pattern segment captures that path component, read by the handler
  with **`req.param("name") -> string?`** (PSR-15-style request attributes, so the
  `handle(Request) -> Response` contract is unchanged — `Request` gains a 5th private `attrs` field
  carrying the captures, plus `param`/`withParams`).
- **Literal > parameter precedence** — `/users/me` (all-literal) beats `/users/{id}` regardless of
  registration order (specificity = literal-segment count; a true tie goes to the first-registered
  route). Method-sensitive; no match → a 404 response.
- A pattern containing `{…}` **must be a raw string** (`r"/users/{id}"`), otherwise the normal string
  interpolates `{id}` as a variable — documented in `examples/web/router.phg` (rewritten from the W1
  enum-tag placeholder into the real router) and pinned by `conformance/web/router.phg`.

### Added — stability & conformance (GA rock 3)

A stability story for the pre-1.0 surface: a golden-output conformance corpus, written policies, and a
deprecation mechanism.

- **Conformance corpus** (`conformance/`, gated by `tests/conformance.rs`): 32 single-feature programs
  + a flagship multi-package DDD project, each with committed golden output asserted byte-identical on
  the interpreter, the VM, **and** real PHP. Stronger than the example differential (which only checks
  the backends *agree*) — the golden pins the value, catching a regression where all backends drift
  identically. Glob-discovered (incl. project roots via `phorj.toml`). Breadth covers the full stable
  language surface: condition loops + compound-assign (`lang/loops`), `foreach … as … with i`
  (`lang/foreach`), integer ranges (`lang/ranges`), `"""` text blocks + raw strings
  (`lang/text-blocks`), `type` aliases (`lang/type-aliases`), member visibility (`types/visibility`),
  property hooks (`types/property-hooks`), and fixed-length lists `[T; N]` (`types/fixed-lists`),
  alongside the type-system, collection, stdlib, and error programs.
- **`SEMVER.md`** — the versioning contract: in `0.x` minor versions may break but each is documented
  (`### Breaking` CHANGELOG heading); at `1.0` the *stable* tier freezes under strict SemVer.
- **`STABILITY.md`** — every public construct, stdlib module, and CLI command sorted into
  stable / experimental / deprecated tiers; the conformance corpus enforces the stable tier.
- **`docs/DEPRECATION.md`** + the **`W-DEPRECATED`** lint: a deprecated stdlib symbol keeps working but
  emits a warning naming its replacement + removal version (warning channel, never gates the build),
  for ≥1 minor release before removal. Flagged via a `native::deprecation_of` side table (empty in the
  shipping build — the mechanism is ready ahead of the first real deprecation; a `#[cfg(test)]` sample
  exercises the lint). `phg explain W-DEPRECATED`.

### Added — overloaded static methods (Statics-B)

A `static` method may now be **overloaded** and called by the class name: `Color.of(int)` /
`of(int,int,int)` / `of(string)` are selected at the call site by the argument types, runtime
multiple dispatch identical to instance-method overloading. Closes the Statics-A deferral. One new
`Op::CallStaticOverload` (runtime-identical to `Op::CallOverload` — it shares the exec arm and the
`validate` bounds check; it differs only in compile-time `stack_effect`, since the compiler pushes a
dummy receiver below the args that the selected static body's arity pops). Byte-identical
run≡runvm≡real PHP.

- Checker: removed the static-call overload rejection (routes through `check_method_sigs`, the
  instance-overload path); added `E-OVERLOAD-STATIC-MIX` — every overload of one name must agree on
  `static`-ness (a mixed set has no sound call form; PHP forbids it too). Interpreter already
  selected; compiler now consults `method_overloads` at a static call site and emits
  `Op::CallStaticOverload`; transpiler emits a `static` dispatcher with `self::` branch targets.
- `examples/guide/overloaded-statics.phg` (incl. an inherited overloaded static `Swatch.of(..)`);
  checker tests; `phg explain E-OVERLOAD-STATIC-MIX`. **Still deferred:** a static on a generic class
  using the class type parameter; late static binding (`static::` / `new static()`).

### Added — `phg lsp` language server (Item D)

A Language Server over stdio so editors get live Phorj diagnostics, hover, and go-to-definition (GA
rock 2 — daily-use tooling). Design: `docs/specs/2026-06-28-lsp-design.md`. No new `Op`/`Value`; off
the byte-identity spine. Ships with a VS Code thin client (`editors/vscode/`).

- **Hover** — the declaration signature of the symbol under the cursor (top-level *or* a local/param).
- **Go-to-definition** — jump to a function / class / enum / interface / trait / type alias declaration,
  or to a local binding (parameter, `var`, `for` var, `if`-let, `catch`, destructure) in scope.
- **Completion** (v2) — top-level names, the enclosing callable's in-scope locals/params, and keywords.
- **Document symbols** (v2) — a hierarchical outline; classes/enums/interfaces/traits expand to their
  members/variants (`range` `[item..next_item)` so children nest correctly, `selectionRange` = name).
- **True end-ranges** (v2) — diagnostics, hover, and definition ranges span the whole token (re-derived
  from the buffer, since the `Diagnostic` struct is span-less), not a 1-char caret.
- Resolution lives in `src/lsp/scope.rs` (position↔offset, binding collection, enclosing-callable by
  source ordering) + `src/lsp/symbols.rs`; all front-end-only. **Deferred:** member completion
  (needs the resolved-type index) and lambda/match-pattern binders.
- **VS Code thin client** (`editors/vscode/`): registers `*.phg` + launches `phg lsp`. Generic-editor
  registration (incl. a Neovim snippet) documented in the README "Editor support" section.

- **Hand-rolled JSON-RPC in `std`** (`src/lsp/`): an LSP server is not a security-critical primitive,
  so the dependency policy excludes `tower-lsp`/`lsp-server`/`serde`. The module owns a minimal total
  JSON parser (inbound bodies), `Content-Length` framing, the server loop, and the diagnostic mapping.
- **`phg lsp`** speaks LSP on stdin/stdout: `initialize` (advertises `textDocumentSync: full`),
  `didOpen`/`didChange`/`didClose`, `shutdown`/`exit`. On open/change it runs the **same** pipeline as
  `phg check` (lex → parse → check) and pushes `publishDiagnostics`, so editor squiggles equal the CLI.
- Diagnostics map 1-based `line`/`col` → LSP 0-based ranges, error/`W-…` → severity 1/2, and carry the
  stable `code` (resolvable via `phg explain`). `tests/`-style coverage in `src/lsp/tests.rs` (10 tests:
  JSON parser, lifecycle, diagnostics, severity). **Next slice:** hover + go-to-definition (a
  position→symbol index) and a VSCode thin client.

### Added — inherited / trait static methods (Statics-A)

A `static` method is now inherited: `Child.staticFromBase(..)` resolves the declaring class's body,
and a `trait`-supplied static is callable on the using class. Closes the B0 own-class-only limitation.
No new `Op`/`Value`. Research: `docs/specs/2026-06-28-statics-research-design.md`.

- The checker propagates inherited/trait static-method *names* through `merge_inherited` + the
  trait-`use` path (mirroring `methods`), so the `static_methods` gate accepts them; the interpreter's
  `call_static_method` resolves through the shared `method_origins` table (like `call_method`); the
  compiler's `class_method_origins` already aliased the dispatch entry. Byte-identical run≡runvm≡PHP.
- `examples/guide/static-inheritance.phg`; checker tests. **Deferred:** overloaded statics (the VM has
  no static-overload dispatch set) and late static binding (`static::`/`new static()` — a deliberate
  non-feature). An *instance* method called via the class name is still `E-STATIC-CALL`.

### Added — `Secret<T>` opaque wrapper (Fork B)

A type for sensitive values (passwords, API keys, tokens). No new `Op`/`Value`/`Ty` — an injected
generic class reusing the `Box<T>` machinery. Design: `docs/specs/2026-06-28-secret-type-design.md`.

- **Loud, by construction**: a `Secret` is not a string and has no display, so
  `Console.println(secret)` / `"{secret}"` is a **compile error**; the wrapped field is `private`, so
  `.expose()` is the only read path. (Chosen over a runtime-`***`-redacting wrapper, which would need
  a new `Value` variant + a *silent* `***` — loud beats silent.)
- **`import Core.Secret;`** injects `class Secret<T> { constructor(private T value){} expose(): T }`.
  `new Secret(x)` infers `Secret<T>`.
- **`W-SECRET` lint** (non-fatal, stderr) fires when `.expose()` is a *direct* argument to a sink
  (`Console.println`/`print`, `Core.File.write`). Syntactic on the direct argument; `phg explain W-SECRET`.
- **Transpiles** to a `final class Secret` whose constructor parameter carries `#[\SensitiveParameter]`
  (PHP redacts it in stack traces — the `K-secrets-type` intent). Byte-identical run≡runvm≡real PHP.
  Showcase `examples/guide/secret.phg`.

### Added — `Core.Regex` (Fork A) + 2nd vetted dependency

A ReDoS-safe regular-expression engine. No new `Op`, no new `Value` (the compiled value reuses the
injected-type + value-as-first-arg patterns). Design: `docs/specs/2026-06-28-core-regex-design.md`.

- **Engine = the `regex` crate** — the project's **2nd** external dependency (after `argon2`). A
  RE2-style finite automaton with **guaranteed linear-time matching (ReDoS-immune by construction)**,
  unlike PHP/PCRE backtracking. The dependency policy (`docs/specs/2026-06-27-dependency-policy.md`)
  is amended: clause 1 generalizes from "crypto" to "security-critical primitive — crypto **and**
  untrusted-input parsers (regex) where `std` has none and rolling-your-own is the anti-pattern."
  Feature-gated `regex` (default on; OFF for `phorj-playground`, like `crypto`).
- **`import Core.Regex;`** → `Regex.compile(string) -> Regex` (validate once, memoized; faults on an
  invalid/unsupported pattern), `matches`/`find`(→`string?`)/`findAll`(→`List<string>`)/`findGroups`
  (→`Map<string,string>?`, named captures)/`replace`/`split`. `Regex` is a compiler-injected class
  holding the bare pattern; always Unicode (`/u`), case-sensitive.
- **Byte-identity holds on the regular subset**: the crate's no-backref/lookaround feature set is
  exactly what PHP `preg_*` matches identically; unsupported patterns are rejected at `Regex.compile`.
  Transpiles to gated `__phorj_regex_*` helpers (collision-free delimiter + `preg_*`); `run ≡ runvm ≡
  real PHP 8.5`. Showcase `examples/guide/regex.phg`.
- **Patterns use raw strings** `r"..."` — the `{n}` quantifier would otherwise collide with `{expr}`
  string interpolation, and raw strings drop `\` double-escaping.

### Added — `phg fmt` formatter (M-fmt)

A canonical-form source formatter (GA rock 2 — daily-use tooling). No new `Op`, no new `Value`.

- **Comment side-channel** — `lex_with_comments()` collects comments (which the token stream drops)
  as `Comment{span,text,kind,own_line}`; `lex()` is unchanged.
- **Full-surface, meaning-preserving printer** (`src/fmt/`) — prints from the parsed AST (not by
  re-spacing tokens), so `parse(fmt(x))` can't change meaning; exhaustive matches make it
  compiler-proven complete over every Item/Stmt/Expr/Type/Pattern. Idempotent; comments preserved.
- **`phg fmt [--check] [path… | -]`** — in-place (writes only on change), `--check` (exit 1 if any
  file would change, no writes — the CI gate), stdin (`-`), recursive dir/no-path discovery. An
  unparseable file is left untouched (exit 2). A dogfood test formats every repo example and asserts
  behavior is preserved.
- v1 is *tidy + comment-safe* (canonical indentation/spacing/blank-lines, `->`→`:`); no line-wrapping.

### Added — `phg test` runner + `Core.Test` assertions (M-Test)

A first-class testing story so Phorj can dogfood itself (GA rock 2 — daily-use tooling). No new `Op`,
no new `Value`.

- **`test "name" { … }` items** — a contextual `test` keyword (special only at item position before a
  string literal, so it stays a usable identifier). A test body is checked like a `-> void` body (no
  `this`); a `test` block in a normal build is rejected as `E-TEST-OUTSIDE-TESTS` (`phg explain`).
- **`Core.Test` assertions** — `assert(bool, string)`, `assertTrue`/`assertFalse`, `assertEquals`/
  `assertNotEquals` (value equality via the shared `==` kernel; same-type-required, generic),
  `assertNull`/`assertNotNull`, and **`assertFaults(() -> T)`** (a HigherOrder native — passes iff the
  closure faults). A failing assertion raises a fault the runner catches per-test.
- **`phg test [path…]`** — discovers `*.phg` under the project's `tests/` (or a given file/dir), loads
  each through the normal loader, validates in test mode, and runs every `test` block independently on
  the interpreter (each body is lowered into a synthetic `main` and routed through the ordinary
  check/expand/interpret pipeline — no test-specific backend path). cargo-style report; exit `0` iff all
  pass. Runnable showcase under `selftest/`.

### Added — math breadth + number formatting (M-NUM S4) — closes M-NUM

The final M-NUM slice rounds out `Core.Math`. All additive stdlib natives — **no new `Op`, no new
`Value`**:

- **Integer helpers (byte-identical regardless of float display):** `sign(int) -> int` (→ PHP `<=>`),
  `clamp(int, int, int) -> int` (→ `max(lo, min(v, hi))`, never panics when `lo > hi`),
  `gcd(int, int) -> int`. `gcd` has no PHP-core builtin (gmp is absent under `php -n`), so it erases
  to a single-sourced **`__phorj_gcd`** helper (Euclid over the magnitudes); the `i64::MIN` magnitude
  edge faults cleanly (EV-7).
- **Transcendentals:** `log`/`log10`/`exp`/`sin`/`cos`/`tan(float) -> float` (→ the same-named PHP
  libm builtins) and the constants `pi()`/`e() -> float` (→ `M_PI`/`M_E`). A non-representable result
  diverges between Rust's shortest-round-trip and PHP, so the guide exercises them at their *exact*
  (IEEE-defined) values and prints real results through `numberFormat`.
- **`numberFormat(float, int) -> string`** — non-locale `number_format`: rounded half-away-from-zero,
  grouped by threes with `,`, `.` decimal point. Erases to a single-sourced **`__phorj_number_format`**
  helper (identical string assembly to `value::number_format`), so the PHP leg never relies on PHP's
  own `number_format` (its `-0`/locale quirks). A negative `decimals` clamps to `0` on both legs.

`examples/guide/math.phg` extended; byte-identical `run ≡ runvm ≡ real PHP 8.5`. **M-NUM is now
closed** (S1 decimal core → S2 division/rounding → S3 predicates/conversions → S4 math breadth);
`BigInt` / arbitrary-precision decimal / `Money`+currency remain deferred to **M-NUM-2**.

### Added — float predicates + numeric conversions (M-NUM S3)

Rounds out the numeric surface: detect float special values and convert **explicitly** between
`int`/`float`/`decimal` (Phorj has no implicit coercion). All additive stdlib natives — **no new
`Op`, no new `Value`** (reuses the native registry, S2's `Value::Null`/optionals, and S1's
`Value::Decimal`). Every primitive is PHP **core** (available under `php -n` — no extension):

- **`Core.Math` float predicates + special values:** `isNan`/`isFinite`/`isInfinite(float) -> bool`
  (→ PHP `is_nan`/`is_finite`/`is_infinite`); `nan`/`infinity`/`negInfinity() -> float`
  (→ `NAN`/`INF`/`-INF`). The predicates return `bool`, so they are byte-identical even for a
  non-representable float operand (the divergence is in float *display*, not in a `bool`).
- **`Core.Math.intdiv(int, int) -> int`** — integer division truncating toward zero (→ PHP `intdiv`);
  single-sourced with `value::int_intdiv`. A zero divisor faults `"division by zero"` and
  `intdiv(i64::MIN, -1)` faults `"integer overflow"` — both run≡runvm (FaultKind parity), PHP `intdiv`
  throws the matching class (not a runnable example).
- **`Core.Convert` numeric conversions:** `toFloat(int) -> float` (total widening; already present),
  `toInt(float) -> int?` (truncate toward zero; **null** on NaN/±∞/out-of-i64-range — avoids PHP's
  surprising `(int)NAN == 0`), `intToDecimal(int) -> decimal` (exact, scale 0),
  `decimalToFloat(decimal) -> float` (lossy by nature), `decimalToInt(decimal) -> int?` (truncate
  toward zero; null if the integer part is out of i64 range).

The edge-safe guards are **single-sourced** in `value.rs` (`float_to_int`, `decimal_to_int` — exact
i128-carrier math, no BCMath) and mirrored by gated PHP helpers `__phorj_float_to_int` /
`__phorj_dec_to_int`, so the float→int range verdict and the decimal→int truncation agree byte-for-byte
across `run`/`runvm`/real PHP. `int` is documented as a pinned 64-bit signed integer (i64) in
`docs/INVARIANTS.md`. Byte-identical `run ≡ runvm ≡ real PHP 8.5`; `examples/guide/numeric-convert.phg`.

### Added — decimal division + rounding (M-NUM S2)

Exact, **explicitly-rounded** decimal division — the precision-safe complement to S1's `+ - *`.
Bare `decimal / decimal` (and `decimal % decimal`) is now a **compile error** (`E-DECIMAL-DIV`):
division isn't exact, so an operator would have to silently pick a scale and a rounding rule — exactly
the hidden precision loss `decimal` exists to prevent. Division goes through two natives that name
both:

- **`Decimal.div(decimal a, decimal b, int scale, RoundingMode mode) -> decimal`** — the exact
  rational `a / b`, rounded to `scale` fractional digits under `mode`.
- **`Decimal.round(decimal d, int scale, RoundingMode mode) -> decimal`** — re-scale a decimal
  (exact up-scale, rounded down-scale).
- **`RoundingMode`** — a seven-variant enum (`HalfUp`, `HalfDown`, `HalfEven` banker's, `Up`, `Down`,
  `Ceiling`, `Floor`) **injected** when a program imports `Core.Decimal` (the same compiler-injected
  enum pattern as `Core.Json`); construct a mode with `new HalfUp()`.
- **Faults:** a zero divisor → `"decimal division by zero"`; a negative `scale` →
  `"decimal scale out of range"`; any i128 overflow in the intermediate → the existing
  `"decimal overflow"`. Byte-identical run≡runvm (FaultKind parity); the PHP helper throws the same.

The rounding kernel `value::round_div(n, d, mode)` is **single-sourced** (sign-normalise so `d > 0`,
truncating quotient + dividend-signed remainder, a half-comparison via `|rem|` vs `d − |rem|` to avoid
`2*rem` overflow, the seven mode rules, all `checked_*`). It is mirrored step-for-step by gated
BCMath helpers `__phorj_dec_div`/`__phorj_dec_round` (`bcdiv`/`bcmod` truncate toward zero / take
the dividend's sign — verified identical to Rust i128 `/`/`%`), switching on the `RoundingMode` value's
PHP class and reusing S1's `__phorj_dec_check` for the i128 bounds fault. **No new `Op`, no new
`Value`** — division is a `CallNative`, `RoundingMode` rides the existing enum ops. (Transpiler-only:
the injected enum's PHP class name is mangled `RoundingMode → RoundingMode_` to dodge PHP 8.4+'s
built-in `RoundingMode` enum.) Byte-identical `run ≡ runvm ≡ real PHP 8.5`; `examples/guide/decimal-div.phg`;
`phg explain E-DECIMAL-DIV`.

### Added — the `decimal` primitive (M-NUM S1)

An exact fixed-point **`decimal`** scalar primitive for money/fixed-point math — making
float-for-currency a *compile choice*, not a silent bug. Representation is `i128` fixed-point
(`Value::Decimal { unscaled, scale }`, value = `unscaled × 10^(-scale)`), std-only and covering all
realistic money. Surface:

- **Literals `19.99d`** — a numeric literal immediately followed by `d`; the scale comes from the
  literal **text** (`1.50d` ⇒ scale 2, `1.500d` ⇒ scale 3, `100d` ⇒ scale 0). An exponent (`1e3d`)
  is rejected and an i128-overflowing literal is a compile-time error — both `E-DECIMAL-LITERAL`.
- **`Decimal.of(string) -> decimal?`** (`import Core.Decimal;`) — parse the same grammar at runtime,
  `null` on malformed/overflow (composes with `??`).
- **`+ - *`** — exact, single-sourced in `value::decimal_add/sub/mul`: add/sub align to `max` scale,
  mul sums scales; any i128 overflow (incl. alignment) is a clean `"decimal overflow"` fault. Mixed
  **`decimal ⊕ int`** (either order) widens the int to a scale-0 decimal and stays `decimal`. A
  `decimal ⊕ float` mix is rejected (`E-DECIMAL-FLOAT-MIX`) — the bug this primitive exists to
  prevent. `/` and `%` are deferred to S2 (division + rounding).
- **Comparison / equality** — numeric, **scale-insensitive** (`1.50d == 1.5d` is true; `decimal`
  compares with `decimal` or `int`).
- **Unary `-`**, scale-padded rendering (`{1999,2}` → `"19.99"`, never `-0`).

Implementation: the literal rides the constant pool (**no new `Value`-kind/`Op` for it**); the VM
gains three type-specialized ops `AddD`/`SubD`/`MulD` (the three coupled matches — `chunk.rs`
`Op`+`validate`, `vm/exec.rs`, `compiler` emit). Compiler gains `NumTy::Decimal`/`CTy::Decimal` so a
decimal-valued field/map/method-result operand specializes on the VM. Transpiles to **BCMath**
(verified available under `php -n`): a literal → a PHP string, `emit_type(decimal)` → `string`,
arithmetic → gated `__phorj_dec_add/_sub/_mul` helpers that derive operand scales at runtime, call
`bcadd`/`bcsub`/`bcmul` with the rule's scale, then bounds-check the result against i128 range and
`throw` the same fault as Rust. `Decimal.of` → a gated `__phorj_dec_of` (tier-1 PCRE). Byte-identical
`run ≡ runvm ≡ real PHP 8.5`; `examples/guide/decimals.phg`;
`phg explain E-DECIMAL-FLOAT-MIX`/`E-DECIMAL-LITERAL`.

### Added — default parameter values + `Text.parseFloat` (M4)

A PHP-familiar language feature: a trailing parameter may declare a literal **default value**
(`function f(int x, int y = 10)`), making that argument optional at the call site (`f(1)` ≡
`f(1, 10)`). **No new `Op`/`Value` and no backend change** — a call that omits trailing defaulted
arguments is rewritten to full arity (provided args + the default literals) by the existing
call-rewrite pass (`rewrite_ufcs`), so the interpreter/VM/transpiler only ever see complete calls; the
default literal is identical on all three, so `run ≡ runvm ≡ PHP` holds by construction. Rules
(checker): defaults must be **trailing** (`E-DEFAULT-PARAM-ORDER`), **literal** (`E-DEFAULT-PARAM-EXPR`),
and **type-assignable** (`E-DEFAULT-PARAM-TYPE`); **free functions only** in v1 (a method/constructor
default is `E-DEFAULT-PARAM-CONTEXT` — a documented follow-up). Natives may declare defaults via a small
`native_defaults` lookup (no churn across the ~50 registry literals). `phg explain` documents all four
codes.

The motivating native lands with it: **`Text.parseFloat(string, bool permissive = false) -> float?`** —
parse a base-10 float, or `None`. `permissive` defaults to **strict**: `[+-]?digits(.digits)?(e±digits)?`
(accepts `1`, `1.5`, `-2.5e3`; rejects `.5`, `5.`, hex, surrounding whitespace). `parseFloat(s, true)`
additionally accepts a lone leading/trailing dot (`.5`, `5.`). **Both reject `inf`/`nan`** — Rust's
`f64::from_str` accepts them but PHP can't, and the float rendering would diverge, so rejecting keeps the
spine byte-identical. Rust is the value source of truth (grammar validator + `f64::from_str`); gated
`__phorj_parse_float` PHP helper mirrors it (PCRE, tier-1). `examples/guide/default-params.phg`.

### Added — `Core.List` / `Core.Text` / `Core.Set` breadth (M4 stdlib sweep)

A breadth pass over the collection + text modules, all additive natives (no new `Op`/`Value`),
byte-identical run/runvm/real PHP 8.5, each with a guide example:

- **`Core.List`**: `slice(xs, offset, len)` (PHP `array_slice`; negatives count from the end,
  out-of-range clamps to empty — the Rust kernel replicates the normalization), `indexOf(xs, x) ->
  int?` (gated `__phorj_index_of`, mapping `array_search`'s `false` to `null`), `concat(a, b)` (PHP
  `array_merge`), `first(xs)` / `last(xs) -> T?`. Each returns a fresh list (immutable). Example
  `examples/guide/list-ops.phg`.
- **`Core.Text`**: `padLeft` / `padRight(s, width, pad)` (PHP `str_pad`), `indexOf(s, needle) -> int?`
  (gated `__phorj_text_index_of`, from `strpos`), `substring(s, start, len)` (PHP `substr`). Byte-based
  / tier-1 (no mbstring) — ASCII domain; a slice/pad that splits a multibyte char faults cleanly (EV-7)
  rather than panicking. Example `examples/guide/text-ops.phg`.
- **`Core.Set`**: `union` / `intersection` / `difference(a, b) -> Set<T>` (PHP `array_unique(array_merge)`
  / `array_intersect` / `array_diff`); the result follows the first set's order. Example
  `examples/guide/set-ops.phg`.

### Added — `Core.Map` access + functional update (M4 stdlib breadth)

`Map<K, V>` was read-only (`keys`/`values`/`has`/`size` + faulting `m[k]`); these add access and
immutable update. `get(m, k) -> V?` is a **safe** lookup — the value when present, else `null` (so a
missing key is an optional, not a fault — composes with `??`/if-let; `V` is non-optional so `null`
unambiguously means "absent"). `set(m, k, v) -> Map<K, V>` and `remove(m, k) -> Map<K, V>` return a
**new** map (Phorj maps are immutable), insertion-ordered like PHP `$m[$k] = $v` / `unset($m[$k])` —
the `set` kernel reuses `value::map_set`. `get` erases inline (`($m[$k] ?? null)`); `set`/`remove` use
gated `__phorj_map_set`/`__phorj_map_remove` helpers (PHP arrays are COW value types, so the by-value
`$m` is already a copy). Byte-identical run/runvm/real PHP; `examples/guide/map-ops.phg`. **No new
`Op`/`Value`.**

### Added — the checked `as` downcast operator (M4 casting, axis 2)

`value as Type` is a **checked** downcast: it yields `Type?` — the value itself when it really is a
`Type` at runtime, else `null` (the Kotlin/Swift `as?` model, the honest form of TS's unchecked
`<T>v` — no lying to the compiler, no later crash). It composes with `??` (`(x as Circle) ?? d`) and
if-let smart-cast (`if (var c = v as Circle) { … c.radius … }`); the scrutinee may be a class,
interface, or union value, and the target a class or interface (a primitive target like `x as int` is
rejected — that's value *conversion*, the `Core.Convert` axis — with a hint, `E-CAST-TYPE`). `value`
is evaluated **exactly once** (the example bakes a side-effecting scrutinee into its byte-identity
gate to prove it). `as` is a *contextual* word (it also separates `foreach (xs as x)` and aliases
imports); a parser restriction keeps the foreach separator from being read as a cast, with brackets as
the escape. Lowers with **no new `Op`** — reuses `Op::IsInstance` + a branch on the backends (the
`??`/`$match` scratch-slot trick, so the operand isn't re-evaluated); transpiles to a PHP arrow-fn
IIFE `(fn($x) => $x instanceof T ? $x : null)($value)`. Byte-identical run/runvm/real PHP;
`examples/guide/as-cast.phg`; `phg explain E-CAST-TYPE`. **No new `Op`/`Value`.**

### Added — `Core.Convert` value conversion (M4 casting, axis 1)

Explicit value conversion — Phorj has no implicit coercion, so you convert on purpose, and lossy
conversions are *named* (no silent `(int)`). `Convert.toString(T) -> string` (generic, reuses the
`__phorj_str` rendering — bool→`true`/`false`, float→shortest-round-trip), `toFloat(int) -> float`
(total widening), `truncate(float) -> int` (toward zero), `round(float) -> int` (half away from zero).
Because UFCS ships, `Convert.toFloat(n)` ≡ `n.toFloat()` — module + method API in one. (The type
*cast*/reinterpret is the separate `as` operator, axis 2, next slice.) Byte-identical run/runvm/real
PHP; `examples/guide/convert.phg`. **No new `Op`/`Value`.**

### Added — `Core.List.sort` / `sortWith` (M4 stdlib breadth)

Ordering for lists, mirroring PHP `sort`/`usort`. `Core.List.sort(List<T>) -> List<T>` returns a new
list in natural ascending order (the input is unchanged — Phorj lists are immutable): ints/floats
numeric, strings **lexicographic by byte** (`"10"` before `"9"`) — deliberately *not* PHP's
numeric-string-juggling `<=>`, so the PHP helper dispatches to `strcmp` for strings to match Rust's
`String` ordering. `Core.List.sortWith(List<T>, (T, T) -> int) -> List<T>` orders by a comparator
closure (higher-order, reusing the `map`/`reduce` re-entrant machinery; a comparator fault propagates
cleanly). Both stable (Rust `sort_by` ≡ PHP 8.0+ `usort`); gated `__phorj_sort`/`__phorj_sort_with`
helpers; byte-identical run/runvm/real PHP. `examples/guide/sort.phg`. **No new `Op`/`Value`.**

### Added — `Core.Text.parseInt` (the first optional-return native)

`Core.Text.parseInt(string) -> int?` — `None` when the whole string is not a valid base-10 integer
(no partial parse, no overflow clamp), unlike PHP's lenient `(int)`. Mirrors Rust's `i64::from_str`
(optional sign, base-10 digits incl. leading zeros, in `i64` range, no surrounding whitespace);
composes with `??` / `if (var n = …)`. PHP erases to a gated `__phorj_parse_int` helper whose
overflow detection matches Rust's `None` (PHP's `(int)` would silently clamp). Byte-identical
run/runvm/real PHP (incl. `+5`/`007`/overflow). `examples/guide/parse-int.phg`.

### Added — `Core.Json` (JSON parse / stringify)

A std-only, deterministic JSON module over a compiler-injected `Json` enum (`Null`/`Bool`/`Int`/
`Float`/`Str`/`Arr`/`Obj`) — expressible now that generic enums + `Map` + `List` all ship. The enum
is injected (head of `cli::check_and_expand`) only when a program `import Core.Json`s, then flows
through every backend as an ordinary enum.

- `Core.Json.parse(string) -> Json?` (None on malformed), `stringify(Json) -> string` (compact,
  matches `json_encode`), `stringifyPretty(Json) -> string` (4-space, matches `JSON_PRETTY_PRINT`).
- **PHP-faithful numbers:** `parse("42")` → `Int`, `"42.0"`/`"1e3"` → `Float` (mirrors `json_decode`;
  an `i64` overflow falls back to `Float`). Objects preserve `Map` key order; duplicate keys keep
  first position / last value (PHP assoc semantics). Strings escape to match `json_encode`'s default
  (`\/`, `\uXXXX` non-ASCII, surrogate pairs).
- **No new `Op`/`Value`:** three `Pure` natives; the one `eval` body is shared by both Rust backends,
  the PHP leg uses gated `__phorj_json_*` recursive helpers. Floats render via the positional
  shortest-round-trip form (`format!("{}")`/`__phorj_float`), so `run ≡ runvm ≡ real PHP 8.5` is
  byte-identical. `examples/guide/json.phg`.

### Added — PHP-reserved enum variant names are mangled in the transpiler

A variant named after a PHP-reserved class word (`Int`/`Float`/`Bool`/`Null`/…) now transpiles to a
mangled PHP class name (`Int` → `Int_`) at the declaration, `new`, and `instanceof` sites, instead of
emitting an invalid `final class Int`. Transpiler-only (the backends address a variant by its Phorj
name), so stdout byte-identity is untouched; reusable for any enum and load-bearing for the clean
`Core.Json` variant API. `examples/guide/enum-reserved-variants.phg`.

### Changed — `E-RESERVED-NAME` now guards the full PHP-reserved-word set (F-m)

The reserved-symbol-name check (previously `var`-only) now rejects every PHP-reserved word that is a
usable Phorj identifier but would transpile to an invalid PHP symbol — turning a latent PHP-oracle
parse error into a clean Phorj diagnostic. **Kind-aware** (empirically verified vs PHP 8.5): a
`function` is checked against the function-illegal set (`var`/`list`/`print`/`array`/`unset`/`empty`/
`eval`/`echo`/`clone`/`callable`/…), a `class`/`enum`/`interface`/`trait` additionally against the
type words (`int`/`float`/`bool`/`string`/`object`/`readonly`/…) — so a `function int()` stays legal
(legal PHP function name) while `class int {}` is rejected. All remain usable as value / parameter /
field / method names. `phg explain E-RESERVED-NAME`.

### Changed — `var` is now a contextual keyword

`var` was a hard-reserved keyword, so it could not be used as an identifier — naming a parameter,
field, or variable `var` was a parse error, and lifting PHP `$var` produced invalid Phorj. `var` is
now **contextual** (like `foreach`/`as`/`when`): it is the inference-binding keyword only at a
declaration start (`var x = …`, `var [a, b] = …`, struct destructure, `if (var x = opt)`), and an
ordinary identifier everywhere else. The change is **purely additive and backward-compatible** — every
existing program parses identically; only previously-rejected positions are now accepted.

- `var` is usable as a **variable / parameter / field / property / method** name (it maps to a legal
  PHP `$var` / `->var` / `->var()`, verified against PHP 8.5). Mutability stays the orthogonal
  `mutable` axis — `var` carries no mutability meaning.
- Naming a **free function / class / enum / interface / trait / type** `var` is rejected with the new
  **`E-RESERVED-NAME`** (PHP reserves `var` in those symbol positions — `function var(){}` / `class
  var{}` are PHP parse errors; `phg explain E-RESERVED-NAME`).
- Front-end-only (lexer keyword table + parser dispatch + one checker guard); **no new `Op`/`Value`**,
  byte-identical `run ≡ runvm ≡ real PHP 8.5`. Unblocks lifting PHP `$var` → Phorj `var` verbatim.
  `examples/guide/contextual-var.phg`.

### Added — `this`-capture in closures (Phase 1 closures slice)

A method-body lambda may now reference `this`: `function reader() -> (() -> int) { return fn() =>
this.n; }`. The receiver is captured **live** (the same instance handle), so a field write made after
the closure is built is visible when it runs. Byte-identical `run ≡ runvm ≡ real PHP 8.5`; **no new
`Op`/`Value`** — `this` rides the existing value-capture path (interpreter: a `this_capture` on the
tree closure; VM: an implicit first capture at the sub-frame's slot 0; PHP: arrow-fns auto-bind `$this`).

- The `E-LAMBDA-THIS` guard is **narrowed to field/static initializers only** — a field-default lambda
  may not capture `this` (the instance is only partially built when an initializer runs). `this`-capture
  also threads through nested lambdas and into closures passed to higher-order natives (`List.map`).
  `examples/guide/closures-this.phg`.

### Added — fixed-length lists `[T; N]` (Phase 1 types slice)

`[int; 3] rgb = [255, 128, 0];` — a `List<T>` whose length is a compile-time constant. Byte-identical
`run ≡ runvm ≡ real PHP 8.5`; **no new `Op`/`Value`** — at runtime a `[T; N]` *is* a list (erases to a
PHP array); the length is a compile-time-only guarantee.

- **Checker-only distinction:** the length is tracked, a list-literal initializer must have exactly `N`
  elements (`E-FIXEDLIST-LEN`), a *literal* index is bounds-checked at compile time (`pair[5]` on
  `[int; 2]` is `E-FIXEDLIST-BOUNDS`; a dynamic index falls back to the runtime check), and `[T; N]` is
  assignable **to** `List<T>` (a fixed list is a list) but not the reverse (a list has unknown length).
- **Element-set** `pair[i] = e` is allowed on a `mutable` fixed list (length-preserving). Erases to a
  PHP array everywhere (`emit_type` → `array`, `CTy::List` so `pair[i]` specializes as an operand).
  `examples/guide/fixed-lists.phg`. The irrefutable-destructuring payoff (`var [a, b] = pair`) arrives
  with let-destructuring (slice 5).

### Fixed — parenthesized function type in return position (Phase 1 types slice)

`function f() -> ((int) -> bool) { … }` now parses. Previously a `(` in type position was always read
as a function-type parameter list demanding a following `->`, so an explicitly parenthesized function
type in return position failed (only the parens-free right-assoc `() -> (int) -> bool` worked — both now
parse to the same type). A `(` is now disambiguated by whether a `->` follows the `)`: with `->` it's a
parameter list, without it it's a **grouped** type `(T)` ≡ `T` (Phorj has no tuples — `()`/`(A, B)`
without `->` are parse errors). Parser-only; byte-identical (`examples/guide/lambdas-pipe.phg`).

### Added — or-patterns in `match` (Phase 1 operators slice)

`match n { 1 | 2 | 3 => "low", _ => "hi" }` — group alternatives that share one arm body with `|`.
No fall-through, still exhaustive (each alternative discharges its own shape). Works for literals and
enum variants. Byte-identical `run ≡ runvm ≡ real PHP 8.5`; **no new `Op`/`Value`, no backend change**.

- **Front-end only:** the parser collects `|`-separated alternatives and **desugars** them to one arm
  per alternative (sharing the cloned body + guard), so every backend sees ordinary arms —
  exhaustiveness, duplicate-arm (`W-MATCH-UNREACHABLE`), and flow-narrowing all work unchanged.
- **Restriction:** alternatives must be **binding-free** — no `_`, no bare name, no variable-binding
  sub-pattern (`Some(_) | None()` is fine; `Some(n) | None()` is `E-OR-PATTERN-BIND`), since the shared
  body cannot know which alternative matched. Split into separate arms if you need to bind.
  `examples/guide/pattern-matching.phg`.

### Added — `**` power operator + `Math.ipow` (Phase 1 operators slice)

`2 ** 10`, `2.0 ** 3.0`, `Math.ipow(5, 2)`. The `**` operator is **type-directed** (`int ** int → int`,
`float ** float → float`), **right-associative**, and binds tighter than `* / %` — PHP-identical.
Byte-identical `run ≡ runvm ≡ real PHP 8.5`; **no new `Op`/`Value`**.

- **Lowering:** the compiler lowers `**` to an `Op::CallNative` to `Core.Math.ipow`/`pow` (resolved at
  compile time — no `import Core.Math` needed). Both the interpreter's `**` arm and the native call the
  single-sourced `value::int_pow`/`float_pow` kernels, so the two Rust backends compute and fault
  identically. The transpiler emits PHP's native `**` (compound operands parenthesized, so `-a ** 2` is
  `(-$a) ** 2` = `(-a)**2`, matching Phorj rather than PHP's default `**`-before-unary-minus).
- **Semantics:** integer power is overflow-checked; a negative exponent faults (`negative exponent`)
  rather than widening to a float — use `float ** float` for fractional powers. `Math.ipow(int, int) ->
  int` is the named, value-level twin (`Math.pow` stays the float power). `examples/guide/operators.phg`.

### Changed — mandatory `new` for construction (Feature C, breaking)

Every class instantiation and enum-variant construction now **requires** `new`: `new Counter()`,
`new Some(7)`, `new Circle(2.0)`. One uniform rule (a deliberate Phorj departure — no surface
language `new`s a sum-type variant). Byte-identical `run ≡ runvm ≡ real PHP 8.5`; **no new
`Op`/`Value`/backend change**.

- **Front-end only:** the parser wraps a construction in `Expr::New`; the checker validates it
  (`E-NEW-REQUIRED` for a bare construction, `E-NEW-ON-NONCONSTRUCT` for `new` on a free function /
  value — both `phg explain`-documented) then a new `checker::unwrap_new` pass strips `Expr::New` to
  its inner `Call` (alongside `expand_aliases`/`erase_generics`/`resolve_html`) **before any backend**,
  so construction semantics and the byte-identity spine are untouched. The project loader's
  cross-package resolution pass also descends into `Expr::New` (so `new Rect(…)` mangles to
  `new \Acme\Geometry\Rect(…)`).
- **Migration:** `phg rewrite-new <file>` — an AST-span codemod that wraps every class/variant
  construction (patterns and free-function calls are left untouched; idempotent). Applied across all
  examples, projects, and the test corpus. Match patterns (`Some(n) =>`), enum-variant *declarations*,
  and the raw `lex→parse→interpret` test path keep bare names.

### Added — runtime static field initializers (Feature B-static)

`examples/guide/static-init.phg`; byte-identical `run ≡ runvm ≡ real PHP 8.5`. No new `Op`/`Value`.

- **`static TYPE name = <expr>;`** — a static field may now carry an **arbitrary** expression (a call,
  arithmetic, a read of an earlier static), lifting PHP's constant-expression-only static-property
  restriction. Evaluated **once at program start, in declaration order, before `main`** (eager — the
  decided model; lazy + runtime config were rejected, see the master-plan Decisions Log). A literal
  static still works and stays a plain PHP `static $x = <lit>;` default.
- **Lowering:** the interpreter evaluates non-literal statics in `eval_static_inits` (after collect,
  before `main`); the compiler emits a `SetStatic` prelude at the start of `main` (literals stay seeded
  in `static_inits`, non-literals get a `Unit` placeholder); the transpiler declares a non-literal
  static without a PHP default and sets it in a generated `__phorj_init_statics()` called before
  `main()`. The static-init type-check moved to a post-collection checker pass (`E-STATIC-INIT-TYPE`),
  so an initializer may reference a function or another static; the literal-only `E-STATIC-INIT-CONST`
  is retired.
- **Deferred** (KNOWN_ISSUES): static-init mode is fixed (eager) — configurability is an M13 edition
  flag (compile-time only); a static initializer reading a *later* static, and trait static fields with
  non-literal initializers, are not guarded this slice.

### Added — expression field initializers (Feature B, instance)

`examples/guide/field-init.phg`; byte-identical `run ≡ runvm ≡ real PHP 8.5`. No new `Op`/`Value`.

- **`TYPE name = <expr>;` on an instance field** — lifts PHP's constant-expression-only property
  defaults (PHP forbids calls/`$this`/other-property reads — "Constant expression contains invalid
  operations"). Phorj allows **any** expression (calls, closures, arithmetic, `this`/sibling reads),
  evaluated **per-instance at construction in declaration order, after the promoted ctor params are
  bound and before the constructor body**.
- **Declaration-order scope** — an initializer may read `this` and any **earlier-declared** field (or
  a promoted param); a later/self reference is `E-FIELD-INIT-FORWARD-REF`. A field-default closure
  that captures `this` is rejected by the existing `E-LAMBDA-THIS` (this-capture defers to the
  closures slice); a non-capturing closure default is fine.
- **Lowering** — the shared `ast::field_initializers` (the own initializers of the class whose
  constructor PHP actually invokes — PHP doesn't auto-chain `parent::__construct`) drives all three
  backends: the interpreter sets each field after promotion, the compiler emits `SetField`, and the
  transpiler prepends `$this->f = <expr>;` to the constructor prelude (synthesizing a `__construct`
  when the class has field initializers but no constructor). New codes `E-FIELD-INIT-FORWARD-REF`,
  `E-FIELD-INIT-TYPE` (both `phg explain`-documented).
- **Deferred** (KNOWN_ISSUES): a static field still takes a literal-only initializer (Feature B-static
  lands next); inherited field initializers run via PHP's single-constructor inheritance, matching the
  Rust backends, but cross-class chaining of multiple ancestors' initializers is not synthesized.

### Added — `const` class constants (Feature A)

`examples/guide/constants.phg`; byte-identical `run ≡ runvm ≡ real PHP 8.5`. No new `Op`/`Value`.

- **`[visibility] const TYPE NAME = <literal>;`** — a compile-time, immutable, class-level constant
  with member visibility (`public` default / `private` / `protected`), accessed **class-name-only**
  (`ClassName.NAME`, never through an instance). Names are SCREAMING_SNAKE_CASE.
- **Inlined on the Rust backends, idiomatic on PHP** — the shared `ast::class_consts` table (with
  inheritance + trait consts flattened, own/nearer wins) feeds all three backends: the interpreter
  returns the literal `Value`, the compiler emits `Op::Const` (+ a `CTy` so `MAX + 1` specializes —
  the CTy-operand discipline), and the transpiler emits a PHP **typed class constant**
  (`public const int MAX = 100;`, 8.3+) accessed as `ClassName::MAX` (no `$`).
- **Inheritance** — a subclass reads an inherited constant via its own name (`Sub.MAX`), matching PHP.
- **Visibility is enforced at the access site** (the one place Phorj checks member visibility) —
  required because the transpiled PHP `private const` would otherwise diverge from the Rust backends.
- New diagnostics (all `phg explain`-documented): `E-CONST-NO-INIT`, `E-CONST-NOT-LITERAL`,
  `E-CONST-MUTABLE`, `E-CONST-INIT-TYPE`, `E-CONST-CASE`, `E-CONST-VISIBILITY`,
  `E-CONST-INSTANCE-ACCESS`, `E-CONST-REASSIGN`.

### Added — Language Evolution Phase 1 (string slice): `+` concat, `\u{}`, literal braces, raw strings

`examples/guide/strings-ext.phg`; all byte-identical `run ≡ runvm ≡ real PHP 8.5`.

- **String concatenation with `+`** — `string + string` → `string`, type-directed with **no
  coercion** (`"x" + 1` is a compile error, killing JS's `"1" + 1` footgun). Only `+` concatenates;
  `-`/`*`/`/`/`%` stay numeric. Reuses `Op::Concat(2)` on the VM (new `CTy::Str` so a string operand
  is recognized — no new `Op`); transpiles via a new `__phorj_add` runtime helper (`is_string ? . :
  +`, since PHP's `+` is numeric-only).
- **`\u{HEX}` Unicode escapes** — 1–6 hex digits naming a codepoint, expanded to UTF-8 bytes at lex
  time (independent of i18n string indexing).
- **Literal braces `\{` / `\}`** — a literal brace inside an interpolated string (`"\{a {n} b\}"` →
  `{a … b}`). The interpolation split moved into the lexer (`TokenKind::Str` now carries pre-split
  literal/interpolation segments) so a `\{` literal brace is never confused with an interpolation
  brace — a flat parser-side split couldn't tell `\{` from `\\{`.
- **Raw strings `r"…"` / `r#"…"#`** — every byte literal, no escapes, no interpolation (JSON, regex,
  templates); a Rust-style `#`-run delimiter makes embedded `"` expressible.

### Added — Language Evolution Phase 0: `void`/`Empty` + mandatory return types

The foundation slice for the language-evolution roadmap
(`docs/plans/2026-06-24-language-evolution-master.plan.md`). Two front-end-only changes, byte-identical
`run ≡ runvm ≡ real PHP 8.5`.

- **S0a — the two-type "nothing" model.** Replaced the implicit `Ty::Unit` with `void` (the common,
  *uncapturable* nothing — the implicit + side-effect return type) and `Empty` (the rare *holdable*
  nothing — a real type a caller may bind). The one widening edge `void <: Empty` keeps it ergonomic.
  New `E-VOID-CAPTURE` (binding a void value, unless annotated `Empty`). Transpiles `void` → PHP
  `: void`, `Empty` → a hint-less PHP function (capturable `null`). `examples/guide/void-empty.phg`.
- **S0b — mandatory return types.** Every named function, method (incl. `abstract` + interface
  signatures), and statement-body lambda must declare a return type (`E-MISSING-RETURN-TYPE`),
  **including `main`**. Expression-body lambdas (`fn(x) => e`) keep inferring (the `=>` form's whole
  point; PHP arrow fns carry no return type). Constructors and property hooks are exempt. A repo-wide
  codemod (`tools/return_type_codemod.py`, a balanced-paren scanner) annotated every existing function
  with `-> void`. Both new error codes self-document via `phg explain`.

## [0.5.1-alpha.1] - 2026-06-24

First tagged pre-release. Rolls up all work since the internal 0.4.0 mark: M3 + the full M-RT
rich-type system (instanceof, interfaces, Map/Set, generics-all, unions, intersections, overloading,
inheritance, traits), the three-tier error model, M5 packages + git deps, M2.5 cross-OS `phg build`,
M6 web (partial), the pattern cluster + primitives sweep, and the WASM playground. All backends remain
byte-identical (`run ≡ runvm ≡ real PHP 8.4`). Pre-release: APIs and surface may still change before 1.0.

### Added — WASM playground (DX)

A free, zero-backend browser playground (`playground/`), auto-deployed to GitHub Pages on every push
to `master` so the live site always runs the latest `phg`. Spec
`docs/specs/2026-06-24-playground-wasm-design.md`, plan `docs/plans/2026-06-24-playground-wasm.plan.md`.

- New `phorj-playground` **workspace member** (cdylib): thin `#[wasm_bindgen]` exports over plain,
  native-testable `*_json` wrappers (`check`/`run`/`runvm`/`transpile`/`explain`) that bypass
  `on_deep_stack` (no threads on wasm) and call the public pipeline directly. The core `phorj` crate
  is unchanged — still dependency-free + `#![forbid(unsafe_code)]`; `wasm-bindgen` is a wasm32-only dep
  confined to the member. New `cli::parse_program` seam for non-aborting diagnostics. 9 native tests.
- Browser frontend (CodeMirror 6 + a Web Worker with a runaway-program timeout): all three backends
  live — `run`, `runvm`, transpiled-PHP **source**, and that PHP **executed in-browser** (php-wasm,
  PHP 8.4) — with a 3-way agreement badge / diff-on-mismatch. Examples picker (from `examples/guide/`),
  shareable permalink (source in the URL hash, browser-native compression), and clickable `phg explain`
  diagnostics.
- `.github/workflows/playground.yml` builds the wasm + deploys to Pages (additive to `ci.yml`).

### Added — Pattern cluster (M-RT S5) + primitives sweep

Post-M-RT language-ergonomics, front-end-only (no new `Op`, no `Value` change), byte-identical
`run ≡ runvm ≡ real PHP 8.4`. Plan `docs/plans/2026-06-23-pattern-cluster.plan.md`.

- **Match-arm guards** (S5.1): `pat when <cond> => …` (contextual `when`); a guarded arm does not
  discharge its shape for exhaustiveness (`E-MATCH-GUARD-EXHAUST`); non-bool guard `E-GUARD-TYPE`.
- **Struct destructuring** (S5.2): `Pattern::Struct` — shorthand `Point { x, y }`, rename
  `Point { x: px }`, full nesting `Line { from: Point { x, y }, to }`; reuses `Op::IsInstance` + field
  reads. Plus **nested type patterns in variant payloads** (`W(Circle c)`); a refutable payload no
  longer falsely discharges exhaustiveness (also closed the `Some(0)`-alone gap). Codes
  `E-STRUCT-PAT-TYPE` / `E-STRUCT-FIELD-UNKNOWN` / `E-PATTERN-DUP-BIND`.
- **Flow-narrowing** (S5.3): `narrow_from_condition` — `instanceof` then/else (else narrows a union to
  its remaining members), `!`/`&&`/`||` composition, and **early-return guards** narrow the rest of a
  block. Checker-only. Plus **if-let `when` guards** (`if (var x = e when g)`), parser-desugared to a
  nested `if` (no `Stmt::If.guard` field).
- **Primitives sweep**: number-literal formats (`0xFF`/`0b1010`/`0o17`/`1_000`/`1e3`), bitwise
  `& | ^ ~ << >>` (int-only; `>>` is two adjacent `Gt`, never a token), `Console.print` (no newline),
  and a byte-safe stdlib subset (`Text.startsWith`/`endsWith`/`repeat`, `Math.round`, `List.length`).

### Changed — M-Decomp: behavior-preserving codebase decomposition

The whale source files were split into cohesion sub-modules — **zero behavior change** (the
`run ≡ runvm ≡ real PHP 8.4` byte-identity spine is the proof; 823 tests green throughout, every
wave its own commit). Plan `docs/plans/2026-06-23-decomposition-milestone.plan.md`, design
`docs/specs/2026-06-23-decomposition-milestone-design.md`, module map in `docs/ARCHITECTURE.md`.

- **Axis = hybrid by-phase** (cohesion sub-files inside one `mod`), not by-construct: the three
  coupled exhaustive `Op` matches (`vm::exec_op`, `chunk::validate`, `compiler::stack_effect`) stay
  **whole** — verified by a dummy-`Op`-variant smoke check (all three fail to compile, then reverted).
- **Mechanism:** splits live inside one module so child files see the parent struct's private
  fields/methods; moved inherent methods take `pub(super)`, **nothing crate-public widens**.
- **`checker/`** 9786→454 (mod.rs): `resolve`/`collect`/`throws`/`program`/`casing`/`stmt`/`expr`/
  `calls`/`assign`/`matches`/`common`. **`parser/`** 1934→199: `exprs`/`stmts`/`items`/`types`/
  `patterns`. **`ast/`** 1465→669: `walk`/`classes`. **`loader/`** 1220→588: `resolve`/`fs`.
  **`compiler/`** 2967→740 · **`transpile/`** 2407→355 · **`interpreter/`** 1757→612 · **`vm/`**
  915→322 (`exec`/`closure`). No source file exceeds ~1500 lines; `lexer/` and `chunk.rs` left single.
- **Tests mirror the split** as sealed child modules — **by language feature** for `checker/tests/`
  (cross-cutting integration tests through `check()`) and **by construct** for `parser/tests/`.

### Added — M-RT S8: traits (`trait` / `use`) — M-RT CLOSED

Horizontal code reuse via `trait T { … }` composed by a class with `use T;` (design
`docs/specs/2026-06-23-m-rt-s8-traits-design.md`, plan `docs/plans/2026-06-23-m-rt-s8-traits.plan.md`).
A trait is **reuse, not a type** (`use` = has-the-behavior-of, vs `extends` = is-a): a value can never
be typed as a trait and `instanceof Trait` is rejected. Trait members flatten into the using class
**before any backend** (the interpreter/VM see ordinary members); the transpiler reconstructs a native
PHP `trait` + `use`. Byte-identical `run ≡ runvm ≡ real PHP 8.4`; `examples/guide/traits.phg`.

- **Members (maximal set):** methods with any visibility (incl. `private`); `mutable` instance fields
  (set via the using class's ctor) and `static` fields (a **per-using-class copy**, PHP `use`
  semantics); a trait **constructor** (promotion + body) adopted by a using class with no ctor of its
  own; an **abstract requirement** the using class must satisfy (reuses `E-ABSTRACT-UNIMPL`); and
  **property hooks** (`get`/`set`, PHP 8.4 hooks in a trait).
- **Constructor folding:** a trait ctor folds into `ctor_plan` (the single source for all three
  backends) and **wins over an inherited parent ctor** (PHP P2). Footguns become clean ahead-of-time
  diagnostics: `E-TRAIT-CTOR-COLLISION` (two trait ctors), `W-TRAIT-CTOR-SHADOWED` (class ctor wins,
  P1), `W-TRAIT-CTOR-PARENT-SKIPPED` (parent ctor not auto-run, P2).
- **Syntax:** `use T;` is disambiguated from an S6b `use P.m` resolution clause by **dot-lookahead**
  (a `.` after the name = resolution clause). New codes `E-USE-UNKNOWN` / `E-USE-AS-TYPE`; all new
  codes self-document via `phg explain`. **No new `Op`** — traits are front-end + native PHP.
- Closes **M-RT (Rich Types)**: `instanceof` → interfaces → Map/Set → generics-all → unions →
  intersections → totality → overloading → S6 inheritance → **traits**.

### Changed — package/namespace reshape COMPLETE: PascalCase everywhere + `package Main` (slices 2b + 3)

The package model's casing reshape is finished (design `docs/specs/2026-06-20-package-namespace-reshape-design.md`).

- **`E-PKG-CASE`** — package-declaration segments, import path segments, and import `as` aliases must be
  PascalCase (`package Acme.StringUtil;`, `import Acme.StringUtil as Strutil;`), joining the existing
  `E-NAME-CASE`/`E-TYPE-CASE` casing family. This makes the source→PHP-namespace mapping 1:1 with no
  casing transform (`Acme.Convert` ⇒ `Acme\Convert`). The reserved roots `Main` and `Core` are already
  PascalCase; an empty package stays `E-NO-PACKAGE` (no double-report). `phg explain E-PKG-CASE` added.
- **Reserved entry `package main` → `package Main`** — casing-consistent (spec D2); the entry *function*
  `main()` stays camelCase (a value identifier).
- **Migration**: every example, multi-file project, vendored dependency, and test fixture moved to
  PascalCase packages/folders. Distributable coordinates (manifest `module`, `[require]` keys, vendor
  directories, lockfile `name`) stay lowercase — concept C, separate from the namespace.
- **Output-preserving** (the loader's `pascal()` already PascalCased segments for PHP), so
  `run≡runvm≡real PHP 8.4` stayed byte-identical throughout; the differential harness was the safety net.
- Earlier slices: slice 1 (manifest `module`), slice 2a (identifier casing), slice 4 (library types /
  `E-PKG-TYPE` lifted) had already landed. **The reshape is now closed.**

### Added — multiple inheritance: `extends A, B` with explicit resolution (M-RT S6b)

A class may inherit from several parents at once (`class C extends A, B`). Cross-parent method
collisions are never silent: they must be resolved explicitly, and the whole feature is byte-identical
across the interpreter, the VM, and transpiled PHP 8.4 (`examples/guide/inheritance-multi.phg`).

- **Dispatch is single-sourced** through `ast::class_method_origins` — one resolved
  `(class, name) → (declaring class, method)` table both backends consume (the interpreter looks it up;
  the compiler aliases its bytecode method-table entry to it). This replaced the prior split where the
  interpreter walked only the first-parent chain while the compiler BFS-flattened every parent — a
  latent `run`≠`runvm` divergence on any method inherited from a non-first parent.
- **Resolution clauses** in the class body: `use P.m` (pick a parent's method for the colliding name),
  `rename P.m as n` (keep both, the renamed one under a fresh name), `exclude P.m` (drop one). An
  unresolved collision is `E-MI-CONFLICT`. A **diamond** shared base auto-merges (a method reached
  identically through two arms is never a conflict).
- **`abstract` classes & methods**: an `abstract class` cannot be instantiated
  (`E-ABSTRACT-INSTANTIATE`); a concrete subclass must implement every abstract method it declares or
  inherits (`E-ABSTRACT-UNIMPL`); an abstract method is implicitly `open`; `open static` is rejected
  (`E-OPEN-STATIC`, statics aren't virtual).
- **No new `Op`, no `Value` change** — all composition, collision detection, and resolution happen in
  the checker/AST before any backend runs (the same front-end-only discipline as `erase_generics`).
- **Transpile**: PHP has no multiple inheritance, so each parent lowers to an `interface I<name>` +
  `trait T<name>`; a multi-parent class emits `class C implements I…, I… { use T…, T… { …insteadof/as… } }`
  and each decomposed ancestor also gets a concrete `class <name> implements I<name> { use T<name>; }`.
  Resolution clauses become `insteadof`/`as`; the diamond shared base auto-dedups in PHP.
- New diagnostics self-document via `phg explain`: `E-MI-CONFLICT`, `E-ABSTRACT-INSTANTIATE`,
  `E-ABSTRACT-UNIMPL`, `E-OPEN-STATIC` (plus S6a's `E-EXTEND-FINAL`/`E-OVERRIDE-FINAL`/`E-MI-CYCLE`).

### Added — method & function overloading: dynamic multiple dispatch (M-RT)

Several free functions or class methods may share a name with distinct parameter signatures. Phorj
overloading is **dynamic multiple dispatch**: the *runtime* types of the arguments select the
most-specific matching overload — identically in the interpreter, the VM, and the transpiled PHP, so
a program runs byte-identically on all three (`examples/guide/overloading.phg`). This is the
spine-safe, surprise-free realization of overloading (no Java-style static-supertype footgun) and
matches what a PHP developer hand-writes (`if (is_int($x)) … elseif (is_string($x)) …`).

- **Selection** lives in `src/dispatch.rs` (shared by both backends): a `ParamKind` runtime summary
  of each parameter type, and `select_overload` (most-specific-wins). A class subtype beats its
  supertype; primitives are disjoint. An ambiguous (cross-cutting multi-argument) or unmatched call
  is a clean, byte-identical runtime fault.
- **One new `Op::CallOverload(set_id, argc)`** for overloaded free-function calls; overloaded
  *methods* reuse `Op::CallMethod` (no second new op) via a `method_overloads` table. Both consult a
  shared `overloads` dispatch table on `BytecodeProgram`.
- **Checker** treats a name as an overload *set* (`E-OVERLOAD-RETURN` — all overloads share a return
  type; `E-OVERLOAD-DUPLICATE` — no two identical signatures; `E-OVERLOAD-GENERIC` — a generic
  declaration can't be overloaded; `E-OVERLOAD-NO-MATCH`; `E-OVERLOAD-FN-VALUE` — an overloaded
  function has no single first-class value). All self-document via `phg explain`.
- **Transpile**: each overload body emits under a mangled `<name>__ovl_<i>`; one PHP dispatcher under
  the original name selects with an `is_*`/`instanceof` chain, branches ordered most-specific-first.

Scope: free functions + class methods. **Deferred** (KNOWN_ISSUES): overloaded constructors; a union
return type; compile-time ambiguity detection (today an ambiguous call faults at runtime); generic
overloads; and two PHP-erasure limits — overloads differing only by `string`-vs-`bytes` or among
`List`/`Map`/`Set` can't be told apart in PHP (both erase to `string`/`array`), and an ambiguous call
faults in the backends while the PHP chain would take the first match (faulting input only).

### Added — error model Slice 2c: exception cause chains (M-faults)

Closes the M-faults exception tier. A conventional **`cause` field of type `Error?`** on an `Error`
subtype preserves the lower-level error that triggered a higher-level one. On transpile it is routed
into PHP's native exception chain — `parent::__construct($message, 0, $cause)` — so the generated PHP
reports an idiomatic "caused by" via `getPrevious()`, while the Phorj backends read it back as an
ordinary field. Byte-identical `run ≡ runvm ≡ real PHP` (`examples/guide/cause-chain.phg`);
**transpiler-only — no new `Op`, no backend or checker change** (a `cause` field already round-tripped
as a plain field; 2c adds the native-chain routing + a `?\Throwable` property type so the `Error` marker
is not mistaken for PHP's unrelated engine `Error` class). Recognition is gated on field name + marker
type, so a mis-typed or non-`Error` `cause` stays a plain field. The remaining interop pieces — reading
a *foreign* exception's cause via `getPrevious()` and catching PHP-thrown exceptions — fold into PHP
interop (M8.5), which does not exist yet.

### Added — error model Slice 2b: checked exceptions (`throws`/`throw`/`try`/`catch`/`finally`) (M-faults)

The enforced exception tier of the three-tier error model. Byte-identical `run ≡ runvm ≡ real PHP`
(`examples/guide/errors.phg`); **three new `Op`s** (`Throw`/`PushHandler`/`PopHandler`), each extending
the three coupled matches (`chunk.rs` validate + `vm.rs` exec_op + `compiler.rs` stack_effect) in one
change.

- **`throws E` declarations + compile-time enforcement** — a function declares the checked exceptions it
  may raise (`throws A | B`, a set). Every `throw` and every call to a throwing function must be
  *discharged*: caught by an enclosing `try`, or propagated with `?` and a matching enclosing `throws`.
  A throwable type must implement the built-in **`Error`** marker; `throws Error` is too broad
  (`E-THROWS-TOO-BROAD` — declare the specific type); `main` may not let an exception escape
  (`E-UNCAUGHT-THROW`). New codes `E-THROW-TYPE`/`E-THROW-UNDECLARED`/`E-CALL-UNHANDLED`/`E-CATCH-TYPE`
  and the `W-CATCH-UNREACHABLE` lint, all self-documenting via `phg explain`.
- **`throw e;`** unwinds to the nearest matching `catch`. **`try { } catch (T e) { } … [finally { }]`** —
  multiple sequential `catch` clauses dispatch by type, a union `catch (A | B e)` catches either, and a
  shadowed clause is a `W-CATCH-UNREACHABLE` lint. `finally` runs on *every* exit edge (normal, caught,
  re-thrown, or a `return`/`break`/`continue` escaping the block). A `Runtime` fault/panic is **not**
  catchable — it passes straight through every `catch` (panics are an uncaught-by-design tier).
- **`?`-throws propagation** — `f()?` on a throwing call propagates `f`'s exceptions to the enclosing
  `throws` (front-end-only: the checker erases the marker, the call's own throw already unwinds).
- **Native unwinding on both backends** — the interpreter uses a `Signal::Throw` (caught at the `try`
  boundary); the VM uses a handler stack (`PushHandler`/`PopHandler`) and unwinds frames + the operand
  stack to the landed handler. A `throws E` subtype transpiles to a PHP class `extends \Exception`, and
  `throw`/`try`/`catch`/`finally` transpile to the PHP constructs 1:1.

### Added — error model Slice 2a: `Result` `?` propagation + fault intrinsics (M-faults)

The first slice of the three-tier error model — the value tier and the panic tier (the enforced
`throws E` exception tier lands in 2b). Byte-identical `run ≡ runvm ≡ real PHP`
(`examples/guide/result.phg`); **no new `Op`**.

- **`?` error-propagation operator** — postfix `expr?` on a `Result<T, E>` (an enum with `Ok`/`Err`
  variants), in a let-initializer: unwraps the `Ok` payload, or **early-returns the `Err`** from the
  enclosing function (which must return the same `Result`). The lexer already munches `??`/`?.`
  separately, so a lone `?` needs no new token. Lowers via the existing `MatchTag`/`GetEnumField`/
  `Return` ops (the VM's `do_return` truncates to the frame base, so the mid-expression early-return is
  clean); transpiles to a PHP statement hoist (`$t = e; if ($t instanceof Err) return $t; $x =
  $t->value;`) since PHP can't caller-return from an expression. Restricted to a let-initializer
  (`E-PROPAGATE-POSITION`); the function must return the matching `Result` (`E-PROPAGATE-CONTEXT`/
  `E-PROPAGATE-ERR`). The `throws`-call mode is deferred to 2b.
- **Fault intrinsics** — `panic("msg")`, `todo()`, `unreachable()` (all **`never`-typed**, so they
  satisfy return-on-all-paths and complete the totality story) and `assert(cond[, "msg"])`. They reuse
  the existing `Op::Fault` (new data-carrying `FaultMsg` variants — no new `Op`); messages are
  compile-time string literals (`E-INTRINSIC-LITERAL`) single-sourced so both backends render
  identically (`FaultKind::Panic`). The names are reserved (`E-RESERVED-INTRINSIC`). Transpile to PHP
  `throw new \RuntimeException`/`\LogicException` and a ternary-`throw` for `assert`.

All five new diagnostics self-document via `phg explain`.

### Added — generic enums `enum Option<T>` / `enum Result<T, E>` (Rich Types, M-RT)

TypeScript-style type parameters on **enums**, the sum-type companion to generic classes. An enum may
declare `<T, …>` after its name; a type parameter is in scope across every variant's payload, **inferred
at the variant constructor** (`Some(7)` ⇒ `Option<int>`, `Ok(1)` ⇒ `Result<int, …>`) by the same
first-binding-wins unifier as a generic class constructor, and **recovered at every `match`** — matching
an `Option<int>` binds `Some(n)` with `n: int`. A variant that mentions no parameter (`None`) can't infer
it; annotate the binding to fix it (`Option<int> n = None();`). Byte-identical `run ≡ runvm ≡ real PHP`
(new `examples/guide/generic-enums.phg`).

Built by mirroring the shipped generic-class machinery with **zero backend changes**: `EnumDecl`/
`EnumInfo` gain a `type_params` list; `try_variant_or_class_call` infers the enum's arguments at the
variant constructor; a new `enum_subst` substitutes them at a `match`; `erase_generics` gains an
`Item::Enum` arm that rewrites a `<T>` payload to `Type::Erased` (PHP `mixed`) and clears the parameter
list before any backend. **No new `Op`, no `Value` change** — `Ty::Named` type arguments are checker-only
and the parameter list is erased pre-backend, so the byte-identity spine is safe by construction. Scope
mirrors generic classes: `package Main` only, inference-only construction, invariant, no bounds, no
generic enum methods. Reuses `E-GENERIC-PARAM`; **GENERICS-ALL now covers functions, methods, classes,
and enums.**

### Added — totality cluster (M-RT): return-on-all-paths, `never`, dead-code lints

Closed the type system's #1 soundness leak: a function whose declared return type carries a value now
must `return` (or diverge) on **every** path — falling off the end is `E-MISSING-RETURN`. Four
front-end-only sub-features, all byte-identical `run ≡ runvm ≡ real PHP` (see
`examples/guide/totality.phg`):

- **Return-on-all-paths** (`E-MISSING-RETURN`), driven by a conservative structural termination
  analysis (`return` / both-branch `if` / infinite loop / `never`-call diverge).
- **`never`** — the bottom type (`Ty::Never`): a subtype of every `T`, inhabited by nothing. A
  `-> never` function is verified to diverge (`E-NEVER-RETURN` otherwise). Transpiles to PHP 8.1
  native `never`.
- **`W-UNREACHABLE`** — a non-fatal lint for a statement after a `return`/diverging statement.
- **`W-MATCH-UNREACHABLE`** — a non-fatal lint for a `match` arm after a catch-all, or a duplicate
  literal/variant/type arm.

No new `Op`, no `Value` change: `never` erases to a PHP return hint and is otherwise checker-only; the
`E-*` errors reject before any backend runs; the `W-*` lints ride the existing warning channel (stderr,
never gating). All four codes are self-documenting via `phg explain`.

### Added — stack traces & beautiful fault reporting (error-handling slice 1)

An uncaught runtime fault now reports a **call stack** instead of a bare message — innermost frame
first, each with `function` + `line` (and `file:line` in a multi-file project), plus the source line of
the fault. Identical on both backends: the VM walks its live call frames, the interpreter keeps a
logical `trace_stack` that mirrors them, and a `run ≡ runvm` **trace-parity** test enforces byte-equal
output. The fault line is backfilled from the innermost frame, so the tree-walker now reports a line
too (the old interpreter/VM asymmetry is gone).

- **CLI:** `phg run`/`phg runvm` render the message, the offending source line, and the frame list.
- **Web:** `phg serve --dev` returns a styled HTML 500 page (fault + stack + request context, every
  value `Core.Html`-escaped). **Production returns a bare generic 500** — no trace/source/message leak.
- Front-end-only with respect to correctness: program stdout is unchanged, `FaultKind` classification
  is preserved, and the M7 PHP oracle is unaffected (traces ride on stderr). No new `Op`.
- See `examples/errors/README.md`. Catching faults (`try`/`catch` vs `Result`) is a later slice.

### Changed — `phg check` reports whole-project scope

`phg check` on a project now reports the scope it validated — e.g. *"OK — whole project type-checks
clean: 3 files, 2 packages, 5 definitions validated (every file + vendored deps)"* — making explicit
the PHP-absent superpower it already had: because the loader merges every `.phg` under the source root
(first-party **and** vendored) into one program and type-checks it before any backend runs, a broken
class or bad import in a file **no route reaches** fails up front (unlike PHP's autoload-on-demand,
where it hides until that file is interpreted). Loose mode (single file / `-e` / stdin) keeps the plain
`OK (type-checks clean)`. (Counts ride on a new `loader::LoadStats`, project mode only.)

### Added — declaration visibility (`public` / `internal` / `private`)

A three-level visibility lattice on every **top-level declaration** (class, enum, interface, free
function): `public` (default — cross-package), `internal` (this package's files only), `private`
(this `.phg` file only). Lattice `file ⊂ package ⊂ public`. A new axis distinct from member-level
`Modifier` visibility, carried as a dedicated `Visibility` enum on each declaration.

- **Parser**: an optional leading `public`/`internal`/`private` keyword before any top-level decl
  (`internal` is a new reserved keyword); explicit `public` allowed; a doubled prefix is a parse error.
- **Loader-enforced, backend-erased**: the M5 loader records each definition's `(file, package, vis)`
  in Pass 1 and applies the lattice at its three resolution chokepoints — `build_type_imports`
  (cross-package types), `resolve_type_ref` (same-package types), `resolve_call` (functions). No
  backend reads the field, so the `run ≡ runvm ≡ real PHP` byte-identity spine is safe by construction
  (PHP has no file/package-private declarations → emitted as a normal `class`/`function`).
- New codes (both with `phg explain`): `E-VIS-PRIVATE`, `E-VIS-INTERNAL`.
- New byte-identity-gated example project `examples/project/visibility/` (+ README documenting the
  two rejected cases, which can't be runnable examples).

### Added — in-place mutation (mutation milestone, M-mut.1–.7b) — feature-complete

Phorj was a pure single-assignment language (the AST had no assignment statement); the mutation
milestone adds in-place mutation **immutable-by-default, `mutable` opt-in**, with no tracing GC. The
locked spine (forced by the real-PHP oracle): `List`/`Map`/`Set`/`Bytes` are **copy-on-write value
types** (can't cycle ⇒ `Rc`/`Drop` reclaims fully); `Instance` is a **shared-mutable handle**
(PHP/Java semantics). Every slice is byte-identical `run ≡ runvm ≡ real PHP`.

- **M-mut.1** mutable locals + reassignment (`mutable` binding modifier; reuses `Op::SetLocal`).
- **M-mut.2** compound assignment + `++`/`--` + `??=` (pure parser desugar, no new `Op`).
- **M-mut.3** condition loops (`while`/`do-while`/C-`for`/while-let) + `break`/`continue` (no new `Op`).
- **M-mut.4a** `obj with { f = e }` functional update (fresh instance via `Op::MakeInstance`).
- **M-mut.5** value-type element set `xs[i] = e` / `m[k] = e` (one new `Op::SetIndex`, COW).
- **M-mut.6** shared-mutable instance fields `o.f = e` / `this.f = e` (instances are **handles**; one
  new `Op::SetField`; cycle-safe `eq_val`; **no cycle collector** — Fork-3 defer-to-process-exit).
- **M-mut.7a** `static`/`static mutable` class fields, read/written as `ClassName.field` (dot, not
  `::`); new `Op::GetStatic`/`SetStatic`; literal-const initializers seeded once at load.
- **M-mut.7b** **property hooks** `T name { get => expr; set(T v) { stmts } }` — virtual get/set; a get
  computes on read, a set intercepts a write; get-only = read-only, set-only = write-only. Lowers on
  the VM to synthetic `<Class>::<name>$get`/`$set` methods dispatched via the existing `Op::CallMethod`
  (**no new `Op`**); transpiles 1:1 to a PHP 8.4 property hook (new `examples/guide/property-hooks.phg`).
  New codes (all with `phg explain`): `E-HOOK-NO-GET`, `E-HOOK-NO-SET`, `E-HOOK-TYPE`, `E-HOOK-DUP`.

Deferred (see KNOWN_ISSUES): no cycle collector, no identity `===`, nested place-stores (`this.f[i]=e`),
and backed/static/interface/abstract property hooks.

### Added — intersection types `A & B` (Rich Types, M-RT S5)

- **Intersection types:** `A & B` is a value that satisfies *all* members at once — the narrowing dual
  of a union. Members are interfaces plus **at most one** concrete class (two distinct classes are the
  bottom type — a value has exactly one class). A value flows into `Drawable & Named` iff it implements
  both, and **inside, every member's methods are in scope** (member access searches each member, the
  one genuinely new mechanism vs. S4). Lexes a lone `&` to a new `TokenKind::Amp` (distinct from `&&`),
  which **binds tighter than `|`** (`A | B & C` ≡ `A | (B & C)`); normalized like a union
  (`Ty::intersection_of`); the assignability arms are the exact dual of S4's. **No new `Op`, no `Value`
  change** — an intersection is checker- and PHP-signature-only; the runtime value is always a concrete
  instance. Transpiles to PHP 8.1 native `A&B`. Byte-identical `run ≡ runvm ≡ real PHP`
  (new `examples/guide/intersections.phg`).
- New codes (all with `phg explain`): `E-INTERSECT-MEMBER` (a primitive/enum/optional/function member),
  `E-INTERSECT-MULTI-CLASS` (two or more concrete classes — uninhabited until S6 `extends`),
  `E-INTERSECT-ARITY` (collapses to one member), `E-INTERSECT-SIG` (two members share a method with
  conflicting signatures — no class can implement both, since Phorj has no overloading **yet**), and
  `E-INTERSECT-NO-MEMBER` (a member access resolves on no member). `instanceof` now also accepts an
  intersection-typed operand. **Deferred** (see KNOWN_ISSUES): `instanceof` against an intersection,
  optional/function members, whole-intersection optional `(A & B)?`.
- **Method overloading confirmed for M-RT** (sequenced next, right after S5): a Phorj-level feature
  lowered to a single dispatching PHP method (PHP forbids same-name redeclaration) — the
  TypeScript-over-JavaScript relationship the transpile contract is built for.

### Added — union types `A | B` + match-over-union (Rich Types, M-RT S4)

- **Union types:** `A | B | C` is a value that is *one of* several types — the open-composition
  counterpart to a closed `enum`. Members may be classes, interfaces, and primitives (`int | string`),
  and a value of any member flows into a union-typed slot (`Circle` → `Circle | Square`). A union is
  **normalized** (`Ty::union_of`: flatten nested, dedupe, canonical-sort by `Display`), so `A | B` and
  `B | A` are the same type. Lexes a lone `|` to a new `TokenKind::Bar` (distinct from `|>`/`||`);
  transpiles to PHP 8.0 native `A|B`. Byte-identical `run ≡ runvm ≡ real PHP`
  (new `examples/guide/unions.phg`).
- **match-over-union via type patterns:** `match s { Circle c => …, Square sq => … }` matches each arm
  by a runtime type test, binding the narrowed instance — **exhaustive over the union's member set**
  like an enum match. This is the one new pattern kind (`Pattern::Type`), threaded through the parser
  (disambiguated as two identifiers in pattern position — `Circle c`; a lone `Circle =>` stays a
  catch-all binding), checker (binding + narrowing + exhaustiveness), and all four backends. It reuses
  the S1 `instanceof` machinery — **no new `Op`** (the interpreter threads `class_implements`; the
  compiler emits load-path + `Op::IsInstance` + `JumpIfFalse`; the transpiler emits a PHP `instanceof`
  guard). `instanceof` narrowing now also accepts a union operand. Type patterns are top-level-only
  (nesting in a variant payload is a clean `E-MATCH-TYPE`). New codes: `E-UNION-MEMBER` (enum/optional/
  function members rejected), `E-UNION-ARITY` (a union needs ≥2 distinct members), `E-MATCH-TYPE`; all
  carry `phg explain` entries. **Deferred:** enum members in a union, intersection/negative-flow
  narrowing, common-member access on a raw union, whole-union optional `(A|B)?` (see KNOWN_ISSUES).

### Added — erased generics `<T>` on classes (Rich Types, M-RT generics-all)

- **Generic types/classes:** a class may declare type parameters after its name —
  `class Box<T> { … }`, `class Pair<A, B> { … }` — used in its field, constructor, and method
  signatures. The parameter is **inferred at construction** from the constructor arguments
  (`Box(7)` ⇒ `Box<int>`) and **recovered at every use site** (`Box(7).get()` is `int`; a method
  taking a `T` checks its argument at the instance's concrete type). Byte-identical
  `run ≡ runvm ≡ real PHP` (new `examples/guide/generic-types.phg`). This completes generics-all.
- **The TypeScript model — reified in the checker, erased in the backend.** `Ty::Named` now carries
  type arguments (`Ty::Named(String, Vec<Ty>)`): construction unifies the constructor parameters
  against the call's arguments to bind them, and member access substitutes the class's type parameters
  with the instance's arguments — full use-site precision (`string s = Box(7).get()` is a type error).
  After checking, `erase_generics` rewrites a generic class's own `<T>`-typed members (fields,
  constructor, methods) to `Type::Erased`, so the field becomes PHP `mixed` and an instance carries no
  runtime type argument (`instanceof Box<int>` ≡ `instanceof Box`). **No new `Op`, no `Value` change,
  and zero backend changes** — `resolve_cty`/`emit_type` already key a class type on its name and
  ignore arguments, so the byte-identity spine is safe by construction (a front-end-only slice). New
  diagnostic reuse: `E-GENERIC-PARAM` (a method type parameter shadowing a class one). Scope:
  `package Main` only (cross-package generic library types deferred); inference-only construction (no
  `Box<int>(7)`); invariant, no bounds, no generic enums.

### Added — cross-package types: `import type` (Rich Types, M-RT)

- **The `E-PKG-TYPE` gate is retired.** A library (non-`main`) package may now declare a
  `class`/`enum`/`interface`, and another package consumes it with the terminal
  **`import type acme.geometry.Point [as Pt];`** form (binds a bare type name; functions still use the
  Go-qualified `pkg.fn()` form; built-ins like `List` stay import-free). Nominal subtyping,
  `instanceof`, and enum `match` all work across packages. New example `examples/project/shapes/`
  (a library `class` + `interface` + `enum` consumed from `package Main`), byte-identical
  `run ≡ runvm ≡ real PHP`.
- **Mechanism — the cross-package *function* mangle/resolve pass, extended to types.** The loader
  gains a `types` symbol table (`(package, Type) ⇒ Acme\Geometry\Point`) and a per-file type-import
  map; Pass 2 rewrites every type-name position — annotations, instantiation (`Point(…)`),
  `instanceof`, enum construction/`match` (via the bare variant whose enum is mangled) — to the
  mangled FQN, mirroring `erase_generics`'s exhaustive `Type`/`Expr` walk. The checker and both
  backends see fully-resolved names (`run ≡ runvm` by construction); only the transpiler de-mangles,
  bucketing each type into its `namespace Acme\Geometry { … }` block and emitting references as
  absolute FQNs (`new \Acme\Geometry\Rect(…)`, `instanceof \Acme\Geometry\Shape`). **No new `Op`, no
  `Value` change**; a single-package program is byte-identical to the pre-lift output.
- New diagnostics: `E-TYPE-IMPORT-UNKNOWN` (no such exported type), `E-TYPE-IMPORT-CONFLICT` (two
  terminal imports bind one name — alias with `as`), `E-TYPE-IMPORT-BUILTIN` (built-ins are
  import-free), `E-TYPE-IMPORT-SHADOW` (collides with a local type or a module-import qualifier).
- Deferred: the module-qualified type form (`import acme.geometry;` → `Geometry.Point`); generic
  *types* (`Box<T>`); generic interface methods.

### Added — erased generics `<T>` on methods (Rich Types, M-RT generics-all)

- **Generic methods:** a class method may declare type parameters (`class U { function id<T>(T x) -> T
  { return x; } }`), inferred at the call site from the arguments exactly like a generic free function
  (`u.id(7)` → `int`, `u.firstOr(xs, -1)`, `u.applyTwice(5, fn(int v) => v + 1)`). The class itself is
  not generic — only the method introduces `T`. Byte-identical `run ≡ runvm ≡ real PHP` (new
  `examples/guide/generic-methods.phg`).
- **Reuses the S7a free-function machinery, zero backend changes.** The parser drops the now-vestigial
  "methods can't be generic" gate; the checker registers a method signature with its `type_params` in
  scope (so a bare `T` resolves to `Ty::Param`) and routes a generic method call through the same
  first-binding-wins `check_generic_call`/`unify`; `erase_generics` gains a class arm that rewrites
  each generic method's signature + body to `Type::Erased` (PHP `mixed`/`array`/`\Closure`) before any
  backend — so the interpreter, VM, and transpiler never see a type variable. **No new `Op`, no
  `Value` change.** Generic *interface* methods stay deferred (their signatures are built with an empty
  type-param list); generic types/classes (`Box<T>`) are the next generics-all sub-slice.

### Added — generic stdlib natives: `Core.List` & `Core.Map` query ops (Rich Types, M-RT S7b)

- **The first generic native functions**: `Core.List` `reverse(List<T>) -> List<T>` and
  `sum(List<int>) -> int`; `Core.Map` `keys(Map<K,V>) -> List<K>`, `values(Map<K,V>) -> List<V>`,
  `has(Map<K,V>, K) -> bool`, `size(Map<K,V>) -> int`. A native whose stored signature carries a
  `Ty::Param` is now checked at the call site by the **same unifier as a generic free function**
  (`check_native_call` routes through `check_generic_call` when the signature has a type parameter),
  so the parameter resolves to the concrete argument types and the result type is substituted. No new
  `Op`, no `Value` change: each erases to a PHP array builtin (`array_reverse`/`array_sum`/`array_keys`/
  `array_values`/`array_key_exists`/`count`), and the native's `Ty::Param` is registry-only — the
  compiler types a native call by expression shape (`CTy::Other`) and the transpiler emits via the
  `php` closure, so no type variable reaches a backend. Byte-identical `run ≡ runvm ≡ real PHP` (new
  `examples/guide/collections-query.phg`, oracle-gated). Caveats (KNOWN_ISSUES): `List.sum` faults on
  i64 overflow where PHP `array_sum` promotes to float; PHP coerces integer-like/bool map keys, so
  `keys`/`values` round-trip byte-identically only with plain string keys. (The higher-order
  `map`/`filter`/`reduce` build on this path in the next S7b sub-slice.)
- **`Set<T>` (`Core.Set`):** `of(List<T>) -> Set<T>` (deduplicate, insertion-ordered), `contains(Set<T>,
  T) -> bool`, `size(Set<T>) -> int`. `Value::Set` is realigned from a bare `HashSet<HKey>` to an
  insertion-ordered, `Rc`-shared `Rc<Vec<HKey>>` (the same byte-identity discipline as `Map`, risk R1),
  built only through the single `value::build_set` kernel so both backends dedup identically; `Set`
  equality is order-independent membership. Erases to a deduped PHP array (`array_values(array_unique(
  $xs, SORT_STRING))` / `in_array(_, _, true)` / `count`). Byte-identical `run ≡ runvm ≡ real PHP` (new
  `examples/guide/sets.phg`). Set union/intersection and iteration are follow-ups.
- **Higher-order `Core.List` ops (S7b-3):** `map(List<T>, (T) -> U) -> List<U>`, `filter(List<T>,
  (T) -> bool) -> List<T>`, `reduce(List<T>, U, (U, T) -> U) -> U` — the first natives that take a
  **closure** argument. A native's `eval` becomes a `NativeEval` enum: `Pure(fn(args, out))` (every
  existing native) or `HigherOrder(fn(args, invoke))`, where `invoke` is a backend-supplied
  [`ClosureInvoker`] that runs a `Value::Closure` and returns its result. The one native body drives
  **both** backends: the interpreter's invoker wraps `call_closure`; the VM gains a re-entrant
  `call_closure_value` + `run_until` that pushes the closure's frame and drives the **shared**
  `exec_op` until it returns — so a closure's result and any fault it raises are byte-identical to the
  interpreter (the parity discipline of the value kernels, extended to control flow). **No new `Op`, no
  `Value` change.** Generic over the element/result type (same call-site unifier as a generic free
  function); erase to PHP `array_map` / `array_values(array_filter(…))` / `array_reduce`. Byte-identical
  `run ≡ runvm ≡ real PHP` (new `examples/guide/higher-order.phg`, oracle-gated). This **completes
  M-RT S7b.**

### Changed — stdlib namespace is now PascalCase `Core.*` (namespace reshape)

- **The standard-library root and leaf modules are PascalCase**: `Core.Console` → **`Core.Console`**,
  and likewise `Core.Math` / `Core.Text` / `Core.File` / `Core.Bytes` / `Core.Html`. Function names stay
  camelCase (`println`, `sqrt`, `splitOnce`). `import Core.Console;` becomes `import Core.Console;` and
  the call site `Console.println(...)` becomes `Console.println(...)`. `Core` is the reserved package
  root (`E-RESERVED-PACKAGE`). This aligns the stdlib with the namespace-reshape rule that package
  *segments* are PascalCase. A repo-wide breaking codemod across every example, fixture, test program,
  and the native registry; byte-identical `run ≡ runvm ≡ real PHP` preserved (the namespace is a
  compile-time organizing layer — natives still erase to flat PHP builtins). *Consequence:* a stdlib
  qualifier (PascalCase) can no longer be shadowed by a camelCase local, so `E-SHADOW-IMPORT` now only
  bites a lowercase **user**-package leaf. (The broader reshape — `package Main` → `package Main`,
  user-package-segment casing enforcement, manifest `name`→`module` — remains pending.)

### Added — erased generics `<T>` on free functions (Rich Types milestone, M-RT S7)

- **TypeScript-style generic type parameters** on free functions: `function id<T>(T x) -> T`,
  `function firstOr<T>(List<T> xs, T fallback) -> T`, `function applyTwice<T>(T x, (T) -> T f) -> T`.
  The type parameter is **inferred at each call site** from the argument types (structural,
  first-binding-wins unification that descends into `List<T>`, `Map<K,V>`, `T?`, and function types),
  and the call's result type is the substituted return type — so `id(42)` is `int` and `id("x")` is
  `string` from one definition. Byte-identical `run ≡ runvm ≡ real PHP` (new `examples/guide/generics.phg`,
  oracle-gated).
- **Full erasure, no monomorphization, no new `Op`.** A new `Ty::Param(String)` exists *only* in a
  generic function's stored signature + body (it is opaque there — assignable only to the same
  parameter); a new post-check pass `checker::erase_generics` rewrites every type annotation that
  names a type parameter into the new `Type::Erased` and clears the parameter list **before any
  backend runs** — the same "compile-time-only, expanded out" discipline as `type` aliases and
  `html"…"`. The interpreter, VM, and transpiler never see a type variable: erased types compile to
  `CTy::Other` and emit PHP `mixed` (containers stay `array`, function values `\Closure`).
- **Scope this slice:** free functions only (`E-GENERIC-PARAM` on a type param that shadows a built-in
  or is duplicated; generic *methods* are a clean parse error; type params are PascalCase like all type
  names). Bounds, variance, generic types/classes, generic functions as first-class *values*, and an
  empty `[]` literal passed straight to a generic parameter are deferred (see KNOWN_ISSUES). This is
  the unblocker for `Set`, the generic-typed Map/Set query ops, and `core.list` — built on it next.

### Added — `Map<K, V>` foundation: literals + indexing (Rich Types milestone, M-RT S3)

- **`Map<K, V>` literals `[k => v, …]`** and **indexing `m[k]`**, byte-identical `run ≡ runvm ≡ real
  PHP` (verified; new `examples/guide/maps.phg`, oracle-gated). The map literal is distinguished from a
  list literal by the `=>` after the first element; `[]` stays the empty *list* (an empty map literal
  is deferred). Keys are the hashable subset — `int`/`bool`/`string` (`E-MAP-KEY` otherwise) — and a
  missing key is a clean, byte-identical fault (`"map key not found"`), like list out-of-range.
- **Insertion-ordered representation.** `Value::Map` is now an `Rc<Vec<(HKey, Value)>>` (not a
  `HashMap`), so map order is part of the value — keeping a future `keys()`/iteration byte-identical
  with PHP's insertion-ordered arrays. Building (first-position/last-value dedup) and lookup are
  single-sourced in `value::build_map` / `value::map_index` kernels, so the two backends agree.
- **One new `Op::MakeMap(n)`** (across the three coupled matches + `validate`); the existing
  `Op::Index` is made **runtime-polymorphic** (a `List` bounds-checks an int index; a `Map` does a key
  lookup) rather than adding a separate `IndexMap`. The compiler gains `CTy::Map(K, V)` so a map-index
  result is a first-class arithmetic operand (`m["k"] + 1` specializes on the VM — without it the VM
  would fail to compile what the interpreter accepts). Transpiles to a PHP `[k => v]` array; `$m[$k]`.
- **Scope this slice (foundation only):** `Set`, and the generic-typed query ops (`keys`/`has`/`size`/
  `contains`/iteration), are deferred to **erased generics (S7, reordered to immediately follow S3)** —
  they hit the same no-type-variable wall that defers `core.list`. New `E-MAP-KEY` in `phg explain`.

### Added — interfaces + `implements`/`extends` (Rich Types milestone, M-RT S2)

- **`interface I { method sigs }`**, **`class C implements I, J`**, and **`interface K extends I`**.
  An interface is a named contract of method signatures (no bodies). A class that `implements` an
  interface is a **nominal subtype** of it: a concrete instance flows into an interface-typed binding,
  parameter, or return, and code written against the interface works for every implementer
  (polymorphism). Interface-typed receivers resolve methods through the interface's flattened
  (`extends`-closure) signature set.
- **`instanceof` now accepts an interface** on the right (extending M-RT S1's class-only operand):
  `x instanceof SomeInterface` is true for every implementer (transitively, through interface
  `extends`), and inside `if (x instanceof I)` the operand smart-casts to `I`.
- **One shared `class_implements` table.** The transitively-flattened, sorted class→interface map is
  computed once by `ast::class_implements(program)` and consumed verbatim by the checker (subtyping +
  conformance), the interpreter, and the VM (`BytecodeProgram.class_implements`) — one algorithm, so
  the runtime `instanceof` test can never diverge across backends. **No new `Op`** (S1's
  `Op::IsInstance` gained the table lookup). Nominal subtyping threads through a new
  `Ty::assignable_with(from, to, &subtype_oracle)` (the old `Ty::assignable` is the no-subtype
  delegate), keeping the optional/function recursion in one chokepoint.
- **Transpiles to a PHP `interface` / `implements` / `extends`** — byte-identical `run ≡ runvm ≡ real
  PHP` (verified). New `examples/guide/interfaces.phg` (oracle-gated). New diagnostics
  `E-IFACE-IMPL` / `E-IFACE-UNIMPL` / `E-IFACE-SIG` / `E-IFACE-CYCLE` (+ the missing `E-INSTANCEOF-TYPE`
  explain entry, backfilled from S1) are in `phg explain`. Scope this slice: interfaces are
  `package Main`-only (`E-PKG-TYPE`), and method signatures match exactly (no variance yet).

### Added — `instanceof` type test, retiring the `is` stub (Rich Types milestone, M-RT S1)

- **`value instanceof ClassName`** is now a real runtime type test that evaluates to `bool` on
  `run`/`runvm` and transpiles to PHP `$value instanceof ClassName` — byte-identical across all three
  backends (verified against real PHP). The right operand is parsed as a class *type name* (not an
  expression), so it is a dedicated `Expr::InstanceOf` node, not a `BinaryOp`. The VM uses one new
  `Op::IsInstance(String)` (carries the class name inline, like `Op::Fault` — no name-pool entry,
  extends the three coupled `Op` matches).
- **Smart-cast narrowing:** inside `if (x instanceof C) { … }`, the checker narrows `x` to `C` in the
  then-block (reusing the if-let scope mechanism), so member access through it type-checks.
- **The value-equality `is` alias is retired.** `is` is no longer a keyword (it is now an ordinary
  identifier); the old `BinaryOp::Is` (which merely aliased `==` and the transpiler rejected) is gone.
  This closes the GA blocker where `is` parsed and type-checked but could not transpile.
- New `examples/guide/instanceof.phg` (oracle-gated). Scope notes (KNOWN_ISSUES): the operand is a
  **class** today (interface/union/intersection tests arrive with those features in later M-RT
  slices), and with no subtyping yet the test compares a concrete class to a concrete class.

### Added / Fixed — `match` transpiler completion + an Assign-position correctness fix (GA P1-b, M11)

- **Literal-pattern `match` now transpiles.** `0 => …` / `"a" => …` / `true => …` / `1.5 => …` arms
  emit a strict `=== <literal>` guard, mirroring the interpreter's exact value match. This enrolls
  `examples/guide/enums-match.phg` in the PHP oracle (previously `DEFER`'d).
- **Expression-position `match` now transpiles.** A `match` used as a sub-expression (operand, call
  argument, interpolation) lowers to an immediately-invoked PHP closure wrapping the *same* if-chain
  the statement form emits — one lowering, no divergence. Enclosing locals are captured by value via
  `use(…)` (Phorj values are immutable, so by-value is exact); `$this` auto-binds in method closures.
  New `examples/guide/match-expr.phg` (oracle-gated).
- **Fixed: `var x = match …` could throw `UnhandledMatchError` in transpiled PHP.** `emit_match`
  previously emitted independent `if`s plus an unconditional defensive `throw`; that only
  short-circuited in `return` position. In assign (var-decl-init) position the arms fell through and
  the throw ran unconditionally. The chain is now `if/elseif/else`, so exactly one arm runs and the
  throw is the terminal `else` — correct for both positions. (The `run`/`runvm` backends were always
  correct; this was a transpile-leg bug.)
- **Honesty:** KNOWN_ISSUES corrected — at P1-b the `is` operator was **value-equality (a synonym for
  `==`), not a type test**, and the transpiler rejected it. (The earlier claim that all three
  constructs "run fine, only transpile rejects" was inaccurate for `is`.) *This was superseded almost
  immediately by M-RT S1 above, which retired `is` and shipped a real `instanceof` type test.*

### Fixed — transpiled `float` now byte-identical to the Rust backends (GA P1-a)

- A finite `float` rendered through the transpiler previously diverged from `run`/`runvm`: PHP's
  default string cast uses `precision=14` and switches to scientific notation for large/small
  magnitudes (`sqrt(2.0)` → `1.4142135623731`, `1e15` → `1.0E+15`, `0.00001` → `1.0E-5`), while the
  Rust backends print the shortest round-trip, always positional. The transpiler now routes every
  float through a new **`__phorj_float`** runtime helper that reproduces Rust's `f64` Display exactly
  (shortest round-trip, positional for any magnitude, integer-valued floats drop the trailing `.0`,
  `inf`/`-inf`/`NaN` spelled the Rust way). Tier-1 PHP functions only, so it stays correct under
  `php -n`. New `examples/guide/floats.phg` round-trips irrational/large/small magnitudes through real
  PHP. The earlier KNOWN_ISSUES "exactly-representable floats only" caveat is **resolved** for all
  finite floats; the sole remaining float caveat is the fault-domain float-÷-by-zero divergence
  (PHP throws vs. Rust `inf`/`NaN`), which the differential harness excludes by design.

### Security — `phg serve` made DoS-resilient (GA blockers B3, B4 + P1-d)

- **One connection can no longer take the server down (B3).** A per-connection `recv`/`send` error
  (client reset, broken pipe, transient `accept`) previously propagated out of the accept loop and
  exited the process — an unauthenticated remote DoS. The loop now logs and skips such errors and
  continues serving; only `MAX_CONSECUTIVE_TRANSPORT_ERRORS` (64) accept errors in a row with no
  progress shuts it down (a genuinely dead listener). A per-request fault still degrades to a 500.
- **Slowloris closed with a read/write timeout (B4).** Each accepted connection now gets a
  `set_read_timeout`/`set_write_timeout` (default **30s**, configurable with `phg serve --timeout
  SECONDS`; `0` disables). A slow/idle client times out and is dropped, and the single-threaded server
  moves on to the next connection instead of being wedged indefinitely.
- **Framing is now unit-tested + a CPU-DoS fixed (P1-d).** `read_http_request` is generic over `Read`
  and covered by unit tests (Content-Length present/absent/malformed/case-insensitive, terminator &
  body split across chunks, EOF-before-headers, the 8 MiB cap), and the real-socket smoke test is
  un-`#[ignore]`d. Fixed a latent **O(n²)** re-scan of the whole buffer for the header terminator on
  every chunk (a CPU-DoS on a large no-terminator request) — it now scans only newly-arrived bytes.
- `phg serve --help` and SECURITY.md document the single-thread posture, the `127.0.0.1` default, and
  `--timeout`. All changes are in the quarantined `src/serve.rs` runtime — the `run ≡ runvm ≡ php`
  byte-identity spine is untouched.

### Security — `phg vendor` supply-chain hardening (GA blockers B1, B2)

- **Git argument-injection / arbitrary-command-execution closed.** `phg vendor` passed a
  dependency's `git` URL and `tag`/`rev` pin straight to the `git` CLI. An attacker-authored
  `phorj.toml` could therefore inject git options (a leading `-`, e.g. `--upload-pack=…`) or a
  command-executing remote helper (`ext::sh -c '…'`). The clone now uses a `--` end-of-options
  separator and `-c protocol.ext.allow=never`, and both the URL and the pin are rejected up front if
  they start with `-` or use the `ext::`/`file::` transports. The ordinary `file://` URL scheme (used
  by the offline test fixtures) is unaffected.
- **Path traversal via dependency name / `source` closed.** A `[require]` key or a `source` value was
  joined verbatim onto a filesystem path (`vendor/<name>`, `<root>/<source>`), so `"../../.."` or an
  absolute path could make `phg vendor`'s `remove_dir_all`/`rename` — or the loader's scan — operate
  outside the project tree. Both are now validated at manifest-parse time (rejecting `..` traversal,
  absolute paths, empty/`-`-leading segments, and characters outside `[A-Za-z0-9._-]`) and
  defensively re-checked at every path-join site. `source = "."` stays valid.
- Both fixes are confined to the `phg vendor` / loader supply-chain path; the `run ≡ runvm ≡
  transpiled-PHP` byte-identity spine is untouched.

### Packaging — identifier casing enforced (namespace reshape, slice 2a)

- **Identifier casing is now a hard, checked rule.** Value identifiers — functions, methods,
  parameters, fields, `var`/typed local bindings, `for`-loop variables, if-let bindings, and lambda
  parameters — must be **camelCase** (`E-NAME-CASE`); type identifiers — class names, enum names,
  enum variant names, and `type` alias names — must be **PascalCase** (`E-TYPE-CASE`). camelCase is a
  lowercase first letter with no `_` (a single lowercase word like `main` is valid); PascalCase is an
  uppercase first letter with no `_`. Each diagnostic suggests the converted form (`split_once` →
  `splitOnce`, `shape` → `Shape`) and both have `phg explain` entries.
- **The shipped stdlib public API is migrated to camelCase:** `Core.Text.split_once` → `splitOnce`,
  `Core.Html.bool_attr` → `boolAttr`, `Core.Html.void_el` → `voidEl`, `Core.Bytes.from_string` →
  `fromString`, `Core.Bytes.to_string` → `toString`. The native `eval`/PHP mappings are unchanged —
  only the call-site name.
- **Front-end-only, so byte-identity is untouched.** The casing pass lives in the checker (shared by
  all three backends) and only gates *which* programs are accepted; the AST every backend sees is
  identical, so the `run ≡ runvm ≡ transpiled-PHP` spine is unaffected. Casing applies to the original
  source identifier, so a loader-mangled cross-package name (`Acme\Util\compute`) is validated on its
  leaf (`compute`). All examples, fixtures, and inline test programs are migrated.
- This is reshape slice 2a (`docs/specs/2026-06-20-package-namespace-reshape-design.md`);
  **package-segment casing (`E-PKG-CASE`) is deferred to slice 2b.**

### Packaging — manifest distributable key renamed `name` → `module` (namespace reshape, slice 1)

- **`phorj.toml`'s top-level distributable is now `module = "vendor/package"`** (was `name`). The
  *keyword* `package` names the code unit (folder=path, `Main` entry) while `module` names the
  distributable — Go's `go.mod` split — removing the `package`-keyword vs `name = "vendor/package"`
  overload (reshape design D1). The `[require]`/`[require-dev]` dependency keys and the `phorj.lock`
  `name` field are unchanged (they are *dependency coordinates*, not the project's own identity).
  Rename-only and output-preserving: the emitted PHP namespace root (`namespace_root()`) and the
  `run≡runvm≡php` byte-identity spine are untouched. This is the first slice of the
  package/namespace reshape (`docs/specs/2026-06-20-package-namespace-reshape-design.md`); the
  example projects' `phorj.toml` files are migrated.

### Tooling — `phg check --json` (machine-readable diagnostics, LSP foothold)

- **`phg check --json`** emits the checker's diagnostics as a single-line JSON array to stdout (the
  seam `src/diagnostic.rs` always intended): each object carries `stage`/`severity`/`message`/
  `line`/`col`/`code`/`hint` (`code`/`hint` are `null` when absent), errors first then warnings.
  Exit 0 when clean (or warnings only), 1 when any error is present — but the array is always the
  output and nothing goes to stderr, so an editor/LSP can parse it unconditionally. Serializer is
  std-only (RFC-8259 escaping, no serde) on the existing `Diagnostic` type — no backend touched, no
  byte-identity surface. Plain `phg check` is unchanged.

### Core.Html — typed auto-escaping HTML (Waves 1–3: escape kernel + element builders + `html"…"` sugar)

- **Named per-tag helpers (Option 1).** A curated common HTML5 tag set — `html.div`/`html.p`/`html.a`/
  `html.ul`/`html.li`/`html.h1`–`h6`/`html.section`/`html.table`/… and the void elements
  `html.br`/`html.hr`/`html.img`/`html.input`/… — each `html.<tag>(attrs, children) -> Html` (or
  `(attrs) -> Html` for void), sugar over `el`/`void_el` with the tag baked in. Resolved the deferred
  "fn-pointer natives can't bake a tag" blocker by **monomorphizing**: two `macro_rules!` emit a
  per-tag `eval`+`php` pair with the tag literal compiled in via `concat!`, so every tag is a uniform,
  byte-identity-tested registry entry — **no new `Op`, no lexer/parser/checker/backend change** (the
  four-backend native call path is already registry-generic, like Wave 2). `examples/guide/html.phg`
  showcases them, byte-identical on `run`/`runvm`/**real PHP**.
- **Wave 3 — the `html"…"` literal sugar.** A prefixed literal `html"<h1>{name}</h1>"` (lexed by a
  dedicated `scan_html`, mirroring `b"…"`; multi-line for free, since string bodies already span
  lines) that desugars to the Wave-1/2 kernel: literal chunks → `html.raw(chunk)`, and each `{e}`
  hole is resolved **by `e`'s type** in the checker — an `Html` value embeds verbatim (no
  double-escape), a `string`/`int`/`float`/`bool` is auto-escaped via `html.text` (the safe
  default — injecting trusted markup requires writing `{html.raw(x)}` explicitly), anything else is
  `E-HTML-HOLE`. The whole literal becomes `html.concat([…])` and is **erased before any backend**
  (`checker::resolve_html`, the `expand_aliases` precedent), so there is **no new `Op`, no new
  runtime, and no new byte-identity surface** — parity is inherited from the kernel. `html"…"`
  requires `import Core.Html;` (`E-HTML-IMPORT`, robust to `import Core.Html as h;`).
  `examples/guide/html.phg` now showcases the sugar, byte-identical on `run`/`runvm`/**real PHP**.
- **Wave 2 — typed element builders.** A new distinct type `Attr` (like `Html`, erases to PHP
  `string`, non-interchangeable) plus five `Core.Html` natives compose HTML from typed fragments
  rather than hand-written markup: `attr(string, string) -> Attr` (value escaped, name trusted),
  `bool_attr(string) -> Attr` (valueless), `el(string, List<Attr>, List<Html>) -> Html`,
  `void_el(string, List<Attr>) -> Html` (self-closing), and `concat(List<Html>) -> Html`. Each
  builder's `eval` and its PHP emission are held byte-identical by a unit test (the `el`/`void_el`
  PHP uses an IIFE so the tag expression evaluates exactly once). No new `Op`; the safety wall and
  zero runtime divergence carry over from Wave 1. `examples/guide/html.phg` now also exercises the
  builders, byte-identical on `run`/`runvm`/**real PHP**.
- **Empty list literal `[]` as a call argument** now adopts its element type from the expected
  parameter type (a small, call-argument-only bit of bidirectional checking in `check_args`), so a
  zero-attribute or zero-child builder call reads naturally — `el("p", [], [text(x)])`. An empty
  `[]` in a declaration initializer or `return` still requires a non-empty literal.
- **`Html` type + `Core.Html` escape kernel (Wave 1).** The Phorj-idiomatic answer to "how do I write HTML"
  (design: `docs/specs/2026-06-19-core-html-design.md`). `Html` is a distinct checker type
  (`Ty::Html`) that erases to PHP `string` and rides `Value::Str` at runtime — but is **not
  interchangeable with `string`**, so untrusted text cannot reach rendered HTML except through
  `Core.Html.text` (auto-escape) or the audited `Core.Html.raw` (trusted markup). This makes XSS a
  *compile error*, not a runtime hazard — enforced by the type checker, zero new `Op`, zero runtime
  divergence. Boundary natives: `text(string) -> Html`, `raw(string) -> Html`, `render(Html) ->
  string`. Escaping erases to the **pinned** `htmlspecialchars($s, ENT_QUOTES, 'UTF-8')` (tier-1,
  `php -n`-safe) and is mirrored by a Rust five-char table held byte-identical by a unit test.
  `examples/guide/html.phg` runs byte-identically on `run`/`runvm`/**real PHP**. (Builders shipped in
  Wave 2 and the `html"…"` literal sugar in Wave 3, both above.)

### M9 — Engineering Hygiene (CI enforcement)

- **GitHub Actions CI (`.github/workflows/ci.yml`) — locks in M7.** A `gate` job runs the same three
  checks as the local pre-commit hook (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test`) on the toolchain pinned in `rust-toolchain.toml`, and sets `PHORJ_REQUIRE_PHP=1` (with
  `php` installed via `setup-php`) so the M7 PHP oracle in `tests/differential.rs` **fails** rather than
  skips if transpiled PHP diverges from the interpreter/VM. A `cross-build` job installs Zig +
  `cargo-zigbuild` + the four Phase-2 cross targets + `llvm-objcopy` (from `llvm-tools-preview`, via
  `PHORJ_OBJCOPY`) and runs `tests/build.rs` for real (x86_64-musl native exec + windows-gnu PE
  round-trip), plus an aarch64-gnu/musl compile smoke. This makes CONTRIBUTING.md's "CI runs the same
  gate" true (no workflow existed before).

### M7 — Correctness Closure (the third backend leg, enforced)

The transpiler→PHP backend is now inside the automated correctness loop. Previously
`tests/differential.rs` gated only `run ≡ runvm`; the transpiled PHP was never executed, so
transpiler→PHP divergences shipped silently — including inside examples advertising three-way
byte-identity.

- **PHP oracle (closes P0-ROOT).** `tests/differential.rs` gains `all_examples_transpile_and_match_php`
  and `all_example_projects_transpile_and_match_php`: every runnable example/project is transpiled,
  executed by a real `php`, and its stdout asserted byte-identical to the interpreter's (⇒ all three
  backends identical, since `run ≡ runvm` is already gated). **Fails-not-skips:** `PHORJ_REQUIRE_PHP=1`
  makes a missing `php` a test **failure** (CI mode); unset, it skips *loudly* (logged), never a silent
  green. `PHORJ_PHP=<path>` overrides the binary. Examples using a not-yet-transpiled construct are
  loudly deferred (logged `DEFER`, counted), not silently passed. The two narrow self-skipping PHP
  round-trip tests in `tests/cli.rs` (and their if-let/opt!/match-optional siblings — five in all) are
  removed, subsumed by the oracle.
- **P0-1 — integer division.** `7 / 2` now transpiles to `__phorj_div(7, 2)` (a runtime helper:
  `is_int($a)&&is_int($b) ? intdiv : /`), matching Phorj's truncate-toward-zero integer `/`. PHP's
  always-float `/` previously made `7/2` print `3.5` instead of `3`, live in `operators.phg`.
- **P0-4 — float modulo.** `5.5 % 2.0` transpiles to `__phorj_rem(…)` (`is_int…? % : fmod`), matching
  Phorj's `fmod`-style float `%`. PHP's integer `%` previously printed `1` instead of `1.5`.
- **P0-3 — bool interpolation.** An interpolated value is coerced via `__phorj_str` (`is_bool ?
  "true"/"false" : (string)$v`), mirroring `Value::as_display`. PHP's bool-in-string previously printed
  `1`/`` (empty) instead of `true`/`false`, live in `control-flow.phg`/`operators.phg`.
- **P0-2 — operand grouping.** Compound operands of unary/binary ops are now parenthesized
  (`a - (b - c)` → `$a - ($b - $c)`, `!(a && b)` → `!($a && $b)`), so PHP precedence can't
  re-associate them.
- **QW-13 — empty/reversed ranges.** Ranges transpile through `__phorj_range($a, $b, $inclusive)`,
  which yields `[]` for an empty/reversed range (PHP's bare `range()` descends). The KNOWN_ISSUES
  caveat is removed.
- **P1-#9 — large ranges fault cleanly.** A range wider than the new single-sourced
  `value::MAX_RANGE_LEN` (10M) now faults `"range too large"` (classified `FaultKind::RangeTooLarge`,
  `agree_err`-gated on both backends) instead of OOM-aborting (exit 101). Length is computed with
  `checked_sub` (EV-7). `value::build_range` single-sources the size-guarded materialization for both
  backends.

The four P0 fixes use runtime PHP helpers (mirroring Phorj's type-driven value kernels) rather than a
transpiler-side static type resolver — no duplicated operand-type inference, no inference-completeness
risk. `run ≡ runvm` was always correct; the bug class was php-leg-only.

### M3 S3 (Track A) — lambdas, first-class functions, and the pipe operator

- **Lambdas / closures.** `fn(int x) => x * 2` (expression body, return type inferred) and
  `fn(int x) -> int { … }` (statement body, explicit `-> T` required, `E-LAMBDA-THIS` if it touches
  `this`). Free enclosing locals are captured **by value** (the heap is immutable + acyclic, so no GC
  is needed). New surfaces: `Ty::Function` / `Type::Function`, `Expr::Lambda` + `LambdaBody`,
  `ast::free_vars`, `Value::Closure`, `CTy::Fn`, and two VM ops `Op::MakeClosure` / `Op::CallValue`.
- **First-class function values.** A bare named function is a value — `twice(3, dbl)` passes `dbl`
  itself; the function type is `(int) -> int`. On the VM a named-fn reference compiles to a
  zero-capture `MakeClosure`; the transpiler emits a PHP first-class callable `dbl(...)`.
- **Pipe operator `|>`.** `x |> f ≡ f(x)`, left-associative, **lowered to a plain call in the
  parser** (no new `Op`, no new backend semantics; the four dead `BinaryOp::Pipe` stubs are retired
  to `unreachable!`). `5 |> dbl |> inc` is `inc(dbl(5))`; `1 + 2 |> dbl` is `dbl(1 + 2)`.
- **Transpile targets** (Phorj : PHP :: TypeScript : JavaScript): expression lambda → arrow fn
  `fn($x) => …`; statement lambda → `function($x) use ($cap) { … }` (by-value `use`); named-fn ref →
  first-class callable; a lambda literal in call position → `(fn(…) => …)(args)`.
- All byte-identical on `run`/`runvm` and round-tripped through real PHP 8.6. Example:
  `examples/guide/lambdas-pipe.phg`. Deferred refinements (this-capture, cross-package value refs,
  block-body return inference, function-type variance, `core.list` map/filter/reduce) are recorded in
  `KNOWN_ISSUES.md`.

### M6 slices W2–W4 — routing, the serve runtime, and `phg serve`

- **W2 — static router (pure Phorj, no new feature).** A data-driven `List<Route>` table is scanned
  linearly for an exact `(method, path)` match, yielding a `Handler` enum tag dispatched by an
  exhaustive `match` to named handler functions; a method-sensitive 404 fallback. Routing is fully
  expressible with today's enums + classes + lists + `match`, so it is byte-identical on `run`/`runvm`
  and round-trips through real PHP. Example: `examples/web/router.phg`.
- **W3 — the serve runtime (`src/serve.rs`), the determinism quarantine.** The one module holding
  sockets + wall-clock non-determinism, deliberately **outside** `tests/differential.rs`. A `Transport`
  trait (`recv`/`send`) seams the loop from the world; `TcpTransport` is the real single-threaded
  socket (`Connection: close`, CRLFCRLF + `Content-Length` framing capped at 8 MiB, EV-7 no-panic).
  `serve()` routes each raw buffer through the program's single entry `respond(bytes) -> bytes`,
  degrading a request fault to a 500. **Single-threaded by force** — the `Rc`-shared heap makes runtime
  values non-`Send`, so a thread pool is impossible; true concurrency awaits M6 green-threads under the
  unchanged contract.
- **`interpreter::call_named(program, name, args)`** — invoke a named top-level function with a
  constructed argument (reuses `run_call`). The interpreter is the reference backend and `run ≡ runvm`
  guarantees the VM would agree, so a VM `call_named` (no return-value capture today) is deferred. No
  new `Op`, no new `Value` variant.
- **W4 — `phg serve <file> [--addr 127.0.0.1:8080]`.** Loads the program project-aware (like `run`),
  type-checks it, then runs the blocking HTTP serve loop on the 256 MB deep-stack worker (so the
  interpreter's `MAX_CALL_DEPTH` guard has the same headroom `run`/`runvm` rely on). Per-command
  `--help` with worked examples. Built binaries still ignore argv.
- **PHP bridge (`php -S`).** `examples/web/server.php` is a hand-written front-controller that builds a
  `Request` from PHP superglobals and calls the *transpiled* `handle(Request) -> Response` — the same
  value unit `phg serve` calls natively. The superglobal↔`Request` adapter is runtime glue, not
  transpiled (mirroring `src/serve.rs`). Documented end-to-end in `examples/web/README.md`.
- **Example** `examples/web/server.phg` — the full served app (W1 parse/serialize + W2 routing + the
  `respond` entry + `handle`); its `main()` exercises `respond` on canned `b"…"` requests so it stays
  byte-identical on `run`/`runvm` + real PHP. **Conformance** for the socket path lives in
  `tests/serve.rs` (an in-memory `FixtureTransport`, outside the byte-identity spine).

### M6 slice W1 — the HTTP handler model (`handle(Request) -> Response`, pure Phorj)

- **The portable handler contract** — `Request`/`Response` are ordinary Phorj classes and
  `parse_request(bytes) -> Request?` / `serialize_response(Response) -> bytes` are written in pure
  Phorj (PSR-7/15 shaped). Bodies are `bytes` (HTTP bodies are octets); the head is decoded ASCII for
  line/`:` splitting. Headers ride as `List<string>` raw lines with a `req.header(name) -> string?`
  linear-scan accessor (the method-call API is the public surface; a typed `Header` value arrives with
  S3). No socket yet — that is W3's `phg serve`. No new `Op`, no new `Value` variant.
- **`bytes.find(bytes, bytes) -> int?`** — first-occurrence byte search (`null` when absent, `0` for an
  empty needle, matching PHP 8 `strpos`); locates the CRLFCRLF head/body boundary. Erases to
  `(($p = strpos(…)) === false ? null : $p)`.
- **`text.split_once(string, string) -> List<string>`** — split on the first separator → `[head, tail]`
  (robustly parses `Name: value` headers whose value contains `:`). Erases to `explode($sep, $s, 2)`.
- **Example** `examples/web/handler.phg` — builds a canonical request as a `b"…"` literal, parses it,
  runs `handle`, and serializes the response (Content-Length recomputed from the body). Byte-identical
  on `run`/`runvm` + **real PHP**, auto-gated by the `examples/**/*.phg` glob.

### CLI binary renamed `phorj` → `phg`

- The CLI binary is now **`phg`** (matches the `.phg` extension; ripgrep's model — package `ripgrep`
  ships binary `rg`). All help/usage/version output, the cross-build `--bin`/artifact/cache names,
  release-asset naming, and docs use `phg`. The Cargo **package/lib name stays `phorj`**, as do
  `phorj.toml`/`phorj.lock`, the `.phorj` executable section, `PHORJ_*` env vars, and the
  `~/.cache/phorj` stub namespace.

### M6 slice W0 — the `bytes` type

- **`bytes`** — a new primitive: raw octet sequences distinct from UTF-8 `string`. `Value::Bytes`
  is `Rc`-shared (like `List`); `Ty::Bytes` is a built-in type name. No new `Op` — a `b"…"` literal
  rides the constant pool (`Op::Const`), interop rides `Op::CallNative`, `==` rides `Op::Eq`.
- **`b"…"` literals** — raw byte strings (no interpolation), escapes `\n \t \r \\ \"` plus `\xHH`
  (two hex digits → one arbitrary octet, so a literal can hold non-UTF-8 bytes).
- **`Core.Bytes`** interop module (`import Core.Bytes;`): `from_string(string) -> bytes`,
  `to_string(bytes) -> string?` (UTF-8 decode; `null` on invalid — composes with S2 `??`/if-let,
  never a fault), `len(bytes) -> int` (BYTE count, vs `Core.Text.len`'s character count),
  `concat(bytes, bytes) -> bytes`, `slice(bytes, int, int) -> bytes` (half-open, bounds-clamped —
  total, no fault).
- **Transpile** — `bytes` erases to PHP `string` (PHP strings are byte arrays); `b"…"` → a PHP
  double-quoted literal with `\xHH` preserved; the natives map to `strlen`/`mb_check_encoding`/`.`/
  `substr`. Example `examples/guide/bytes.phg` runs byte-identically on `run`/`runvm` + **real PHP**.
- First slice of the **M6 web-capabilities spike** (design-locked,
  `docs/specs/2026-06-18-m6-web-design.md`); bytes was pulled forward so HTTP bodies can be honest
  octets.

### M5 slice S3 — git dependencies + `phorj.lock` + `phg vendor` + auto-offline

- **`phg vendor`** — the only network-touching command. It clones each `[require]` git dependency
  at its pinned `tag`/`rev`, copies the dependency's source into `vendor/<vendor>/<package>/`, and
  writes `phorj.lock` pinning the **resolved commit SHA** + an FNV-1a-64 content hash. Idempotent and
  crash-safe (stages into a temp dir, swaps atomically, touches only each dependency's own subtree).
- **`phorj.lock`** (`src/lock.rs`) — a strict, deterministic TOML-subset lockfile (`[[package]]`
  blocks: `name`, `git`, `rev`, `hash`); round-trips through its own parser.
- **Auto-offline resolution** — `loader::load_project` merges vendored packages exactly like
  first-party library packages (mangle + resolve before any backend runs ⇒ `run` ≡ `runvm`
  structural; the transpiler de-mangles into `namespace …` blocks). `run`/`check`/`transpile`
  **never fetch** — they read the committed `vendor/`. New guards: `E-VENDOR-MISSING` (a `[require]`
  dep not vendored), `E-VENDOR-MAIN` (a vendored `package Main`), `E-DUP-DEF` (a duplicate
  `(package, name)` after the merge — previously a silent overwrite).
- **Example** — `examples/project/withdeps/` (a project consuming a vendored `acme/strutil` library):
  ships its committed `vendor/` + `phorj.lock`; the project-aware differential harness loads it
  offline and gates `run` ≡ `runvm`, and it round-trips through real PHP. `phg vendor` gains a
  `--help` entry, USAGE/dispatch wiring, and three `phg explain` codes.
- **Tests** — `tests/vendor.rs` drives the real `git clone`/`checkout`/`rev-parse` path against a
  `file://` local-git fixture (offline, deterministic): fetch + lock + offline byte-identical load,
  idempotent re-vendor, and `E-VENDOR-MISSING`.

### M5 slice S2d — project-aware differential harness + public multi-file example

- **First public multi-file project** — `examples/project/tempconv/` (a two-package Celsius→Fahrenheit
  converter) showcases the M5 project model end-to-end: mandatory packages + folder=path, a
  cross-package qualified call (`convert.c_to_f(0)`), import aliasing (`import acme.label as fmt;` →
  `fmt.tag(...)`), and a same-package bare call across two files. Plus `examples/project/README.md`.
- **Project-aware byte-identity gate** — `tests/differential.rs` now discovers every project root (a
  directory with a `phorj.toml`) under `examples/`, loads it through `loader::load`, and asserts
  `run` ≡ `runvm` (and that it runs). The single-file glob is made project-aware — it stops descending
  into any directory holding a `phorj.toml`, so project files are never run standalone (structural,
  name-independent; flat examples keep their `len() >= 3` floor). A project added later is auto-gated.
- **Verified** — the example runs `freezing = 32F` / `boiling = 212F` byte-identically on `run`,
  `runvm`, **and real PHP 8.6** (exact integer math, chosen so PHP's float `/` agrees).
- Docs refreshed for shipped multi-file support: `examples/README.md` (index + matrix rows; the two
  "arrives in a later slice" notes corrected) and `FEATURES.md` (Modules/packages → 🚧, git deps = S3).

### M5 slice S2c — qualified cross-package calls + namespaced PHP + import aliasing

- **Cross-package calls resolve** — `import acme.util;` then `util.compute(x)` now works across files.
  A new resolution pass in the loader (`src/loader.rs`) mangles every non-`main` definition to a
  globally-unique name (`acme.util` + `compute` ⇒ `Acme\Util\compute`; `package Main` defs stay bare),
  then rewrites call sites against each file's package + import map: same-package bare calls and
  qualified user calls become bare calls on the mangled name. Native `core.*` calls are untouched.
- **Import aliasing** — `import a.b as c;` binds the call-site leaf `c` (AST `Item::Import.alias`,
  parsed as a contextual `as` keyword so `as` stays a valid identifier). Resolves leaf collisions (O-9).
- **Namespaced PHP emission** (M5-7/M5-8) — a multi-package program transpiles to one
  `namespace Acme\Util { … }` brace-block per package + a `namespace Main { … }` block + a nameless
  `namespace { \Main\main(); }` bootstrap. Cross-package calls emit fully-qualified (`\Acme\Util\compute`);
  global-function natives gain a leading `\`. A single-package program has no mangled names and stays on
  the flat path — byte-identical to the pre-S2c output.
- **S2c scope: functions only** — a `class`/`enum` in a non-`main` (library) package is rejected
  (`E-PKG-TYPE`); cross-package type namespacing is an M5 follow-up. The S2b bare cross-package call
  interim is tightened: an unqualified cross-package call now fails on both backends.
- **Byte-identity** — resolution runs in the loader *before* any backend, so checker/interpreter/
  compiler/VM are unchanged (run==runvm is structural). Verified end-to-end: a two-file project runs
  `42` on `run`, `runvm`, **and real PHP 8.6** (`php out.php`).
- **`explain`** gains `E-PKG-TYPE` and `E-PKG-PATH` (the latter backfilled from S2b).
- 7 new tests (`tests/project.rs` qualified/alias/same-package-cross-file/unqualified-rejection/
  type-rejection/transpile-structure + a `native.rs` alias-`import_map` case). 409 total green.

### M5 slice S2b — multi-file loader + folder=path enforcement

- **Project loader** (`src/loader.rs`) — resolves an entry source to one `Unit` (a single, possibly
  multi-file-merged `Program` + the source text for diagnostics). **Project mode**: a `phorj.toml`
  found by walking up marks the root; every `.phg` under the source root is parsed, validated against
  its location (**folder = package**, Go's model — `src/acme/util/*.phg` ⇒ `package acme.util`;
  `package Main` is folder-exempt), and all items are merged into one flat program. **Loose mode** (no
  manifest above): only `package Main;` runs — a dotted library package requires a project.
- **`E-PKG-PATH`** — a file whose package does not match its directory under the source root, a dotted
  package sitting directly in the source root, or a non-`main` package living outside the source root.
- **Byte-identity preserved** — enforcement is path-aware and lives in the loader, never in the type
  checker, so `cli::cmd_run(&str)` and the differential harness are untouched. `run`/`runvm`/`check`/
  `transpile` route a `<file>` source through the loader (new `cli::run_program`/`runvm_program`/
  `check_program`/`transpile_program` consume the loaded program); `-e`, stdin, `parse`, `lex`,
  `disasm`, `bench`, and `build` keep the single-file string path. A loose single-file program through
  the loader produces identical output to the pre-S2b pipeline.
- **Flat-merge interim** — until S2c, the merged items share one flat namespace, so a cross-file call
  resolves **unqualified**; qualified cross-package calls (`util.parse(x)`) + one-brace-block-per-package
  PHP emission + import aliasing are S2c. `transpile` of a multi-*package* project therefore emits flat
  PHP for now (correct for `package Main` / single-package). Multi-file type-error diagnostics omit the
  source-line caret (no single aligned source). The `examples/project/` showcase ships at S2d.
- 12 new tests (9 `loader` unit + 3 `tests/project.rs` integration, incl. a multi-file project running
  byte-identically on both backends).

### M5 slice S2a — project manifest + source root + project detection

- **`phorj.toml` manifest** — new `src/manifest.rs` parses a minimal, std-only TOML subset into
  `Manifest { name, version, source, require, require_dev }`. The manifest speaks **Composer's
  vocabulary in an honest TOML container**: `name = "vendor/package"` (doubles as the PSR-4 namespace
  root — `acme/myapp` ⇒ `Acme\Myapp`), `[require]` / `[require-dev]` sections, dependency values as
  `{ git = "…", tag|rev = "…" }` or the `"<git-url>@<tag>"` string shorthand. Each dep self-locates
  via its git URL (no Packagist, no Composer `repositories` side-table); versions are **exact-pin
  only** — a `branch` pin, a missing/double pin, an unknown key/section, or an unquoted value are hard
  errors. A literal `composer.json` was rejected on purpose: the `composer` tool cannot process it, so
  the filename would be a false promise.
- **Project detection** — `Project::detect(path)` walks up from a source file/dir for a `phorj.toml`;
  the first one found marks the project root and resolves the source root (`root/<source>`, default
  `src`). No manifest above ⇒ `Ok(None)` (loose-script mode). Manifest presence is the sole
  project-vs-loose signal (Go's model).
- **Byte-identity preserved** — S2a is parse + represent only; nothing consumes the manifest yet, so no
  `.phg` execution path changes and `run`/`runvm` stay byte-identical. The multi-file loader +
  folder=path enforcement (S2b), qualified cross-package calls + brace-namespace PHP (S2c), and the
  `examples/project/` showcase (S2d) follow. Coverage = 18 `manifest` unit tests (the showcase example
  ships with the observable behavior at S2d).

### M5 slice S1 — package declaration (project-model foundation)

- **Mandatory `package` declaration** — every file declares its package as the first line, never
  inferred (`package app.util;`). The reserved **`package Main;`** is the runnable entry (Go's model;
  pairs with `fn main()`); `core` is reserved for the standard library. New checker codes
  `E-NO-PACKAGE` / `E-RESERVED-PACKAGE` (both `phg explain`-documented). The parser captures the
  path on `Program.package`; a `package` after any item is a parse error (it must be first).
- **Byte-identity preserved** — S1 is front-end only: the interpreter, VM, and transpiler ignore the
  package (flat PHP emission unchanged — `package Main` → no namespace), so `run`/`runvm` and the PHP
  round-trip stay byte-identical. Multi-file projects, strict folder=path, cross-package imports, and
  brace-namespace PHP emission arrive in later M5 slices
  (`docs/specs/2026-06-18-m5-project-model-design.md`).
- All 24 examples + every test program migrated to `package Main;`; the minimal program is now
  `package Main;` + `import Core.Console;` + `Console.println`. (Also fixed pre-existing Wave-1 doc
  drift: `README.md` showed `import std.io;` + bare `println`.)

### M3 slice S0 — developer experience

- **`var` local type inference** — `var x = expr;` infers the binding's type from its initializer
  (still fully static + immutable). The VM derives the local's operand type from the initializer, so
  arithmetic on a `var` still specializes (`AddI`/`AddF`); `ctype` now also resolves a `match` value.
- **`type` aliases** — `type Name = T;`, compile-time only. The checker resolves aliases (with cycle,
  built-in-shadow, and duplicate detection); a post-check pass (`checker::expand_aliases`) expands
  them out of the AST so the interpreter, VM, and transpiler all see alias-free types and the PHP
  output never mentions the alias.
- **Sharper diagnostics** — front-end (lex/parse/type) errors render the offending source line with a
  caret, attach a "did you mean `…`?" hint (nearest in-scope name, Levenshtein ≤ 2), and carry a
  stable code. `Diagnostic` gains `code`/`hint` fields + a `render` method; all construction is
  centralized through `Diagnostic::new`. Runtime-error strings are unchanged (differential parity).
- **`phg explain <CODE>`** — print the explanation for a diagnostic code (`E-UNKNOWN-IDENT`,
  `E-UNKNOWN-TYPE`, `E-INFER-NULL`, `E-ALIAS-CYCLE`).
- **Per-command help** — `phg <command> --help` / `-h` prints a description, the source/flag forms,
  and 1–2 worked examples.
- New guide example `examples/guide/inference.phg` (auto byte-identity-gated by the differential
  harness).

### M3 slice S1 — core ergonomics

- **List indexing `xs[i]`** — un-rejected in both backends (the checker already typed it), reusing the
  bounds-checked `Op::Index`. An out-of-range read is a clean `list index out of range` runtime fault,
  byte-identical across `run`/`runvm` (classified `FaultKind::IndexOob` in the differential harness).
  Transpiles to `$xs[$i]`.
- **Integer ranges `a..b` / `a..=b`** — exclusive / inclusive integer ranges, materialized to a
  `List<int>` by the one new `Op::MakeRange(bool)` (which extends the three coupled matches —
  `vm::exec_op`, `compiler::stack_effect`; `chunk::validate` needs no arm: no static index). Both
  backends build the list via Rust's native `start..end` / `start..=end` (no counter overflow), so
  `for (int i in 0..n)` works unchanged. The lexer adds `..` / `..=` (longest-match). Transpiles to PHP
  `range()`; a non-int bound is `E-RANGE-TYPE` (a `phg explain` entry).
- **Expression `if`** — `if (c) { e } else { e }` in value position (`var x = if (c) { 1 } else { 2 };`).
  Parens + a mandatory `else`; single-expression arms. Disambiguated from the statement `if` by parse
  position; lowers to the existing branch ops (no new `Op`); transpiles to a PHP ternary.
- New guide example `examples/guide/ergonomics.phg` (indexing + ranges + expression `if`),
  auto byte-identity-gated and round-tripped through real PHP.
- **S1.4 (smart-cast narrowing) deferred to S2** — it only narrows optionals (`T?`), which arrive in S2.

### M3 slice S2 — null-safety

PHP-native nullable with a compile-time non-null guarantee (TypeScript `strictNullChecks` over PHP's
nullable runtime). `T?` is the existing `null` value at runtime; the guarantee lives in the checker
(a non-optional `T` can never be `null`). All byte-identical on `run`/`runvm` and 1:1 to PHP.

- **Optionals `T?` + non-null discipline** — `Ty::Optional` + `Value::Null`; `T` auto-widens to `T?`,
  but a `T?` cannot flow into a non-optional `T` (`E-OPT-ASSIGN`), nor be used as an operand/receiver
  without unwrapping (`E-OPT-USE`).
- **`??` null-coalesce** — `a ?? b`; `?.` safe access — `opt?.member` / `opt?.method()` short-circuits
  a null receiver to `null` (PHP `?->`). Both lower to a null-test + branch, **no new `Op`**.
- **`if (var x = opt)`** — binds the non-null inner `T` (smart-cast S1.4) inside the then-block only;
  `E-IF-LET-TYPE` on a non-optional scrutinee. Transpiles to `if (($x = E) !== null) { … }`.
- **`opt!` checked force-unwrap** — `T?` → `T`, a clean `force-unwrap of null` fault on null (never a
  crash; `FaultKind::ForceUnwrap` parity). `E-OPT-UNWRAP` on a non-optional; the **`W-FORCE-UNWRAP`**
  lint flags every use. Transpiles to a once-per-file `__phorj_unwrap()` helper.
- **`match` over `T?`** — `match opt { null => …, v => … }` is exhaustive; the binding arm narrows
  `v` to the non-null inner after a `null` arm.
- **Warning channel (first lint)** — the checker now collects non-fatal warnings; `check()` returns
  them on success and the CLI renders them to stderr without gating the build.
- **No new `Op` variant** — `Op::MatchFail` was generalized to `Op::Fault(FaultMsg)` (single-sourced
  message), serving both match-exhaustiveness and `opt!`-on-null.
- New guide example `examples/guide/null-safety.phg`, auto byte-identity-gated + PHP round-tripped.

### M3 Track B Wave 1 — namespaced native foundation

- **Everything is namespaced — "nothing in the wind".** The free global `println` is retired. A
  program now `import Core.Console;` and calls `Console.println(...)`. Stdlib modules are reserved
  under the `core.*` root; the root lives in the import and the leaf qualifies the call (Go's
  `import "fmt"` → `fmt.Println`). Explicit import is required even for the stdlib.
- **`native` registry** (`src/native.rs`) — each built-in single-sources its four facets in one
  entry keyed by `(module, name)`: checker signature (`params`/`ret`), a runtime `eval` shared
  verbatim by the interpreter *and* the VM (structural parity, like the value kernels), and a PHP
  emission mapping (`Console.println` → `echo … . "\n"`). Built once via `OnceLock`.
- **`Op::Print` → `Op::CallNative(idx, argc)`** — the migrated former print op now indexes the
  registry and pushes the native's result (extends the three coupled `Op` matches + a `validate`
  bound on the native index). No separate `Const(Unit)`.
- **Import-driven resolution across all four backends** — a member call `Console.println(x)` whose
  head is an imported module qualifier dispatches to the native: the interpreter and compiler resolve
  locals-first then by leaf (they track scope); the checker and transpiler use the import map.
- **Shadowing guard** — a value binding may not shadow an imported module qualifier (`E-SHADOW-IMPORT`),
  keeping the import-map-driven transpiler consistent with the locals-first run backends.
- Migrated every `println` call site — all examples, fixtures, and inline test programs — to
  `import Core.Console;` + `Console.println`. The example differential test now also asserts each
  example *runs* (`Ok`), not merely that the backends agree (closing a vacuous-green gap).

### M3 Track B Wave 2 — stdlib breadth (`Core.Math` / `Core.Text` / `Core.File`)

- **`Core.Math`** — `sqrt`/`pow`/`floor`/`ceil` (float) and `abs`/`min`/`max` (int). Concrete-typed
  (the registry's `params`/`ret` have no type variable, so no overloading); each erases to the PHP
  builtin of the same name. `abs` faults cleanly on `i64::MIN` (EV-7).
- **`Core.Text`** — `len`/`upper`/`lower`/`trim`/`contains`/`split`/`join`/`replace`. `split` returns
  `List<string>` and `join` consumes one (the type system already carries `List<string>` end to end).
  The PHP erasures reorder args where PHP differs (`explode`/`implode` separator-first, `str_replace`
  search-first).
- **`Core.File`** — `read` (→ `string?`, `null` on any failure — composes with the S2 `??` / if-let),
  `exists`, and `write`. File *reads* stay byte-identical by reading a **committed fixture**
  (`examples/guide/fixtures/poem.txt`); `write` is a non-deterministic side effect, unit-tested but
  kept out of the byte-identity-gated example set.
- Each module ships a byte-identity-gated guide example (`examples/guide/math|text|file.phg`),
  round-tripped through real PHP. `KNOWN_ISSUES` now documents the pre-existing irrational-`float`
  precision divergence that `Core.Math` makes easy to reach (Rust shortest-round-trip vs PHP's
  default `echo` precision); examples keep to exactly-representable values.
- **Deferred:** `core.list` (needs S3 lambdas / `List<T>` generics) and `core.json` (needs a dynamic
  `Json` type) — they land once generics or S3 exist.

_Next: Track B Wave 3 (user packages: `package` decl + folder=path + PHP `namespace` emission), then
Track A (S3 lambdas/pipeline). M2.5 Phase 3 (CI stub registry; opt-in `--sign`) remains parked._

## [0.4.0] — 2026-06-17

The first fully-documented release: CLI UX, profiling, a disassembler, cross-OS standalone builds,
and a complete OSS doc set.

### Profiling & introspection

- `phg bench` now reports **memory** alongside timing: peak-RSS growth of one cold execution plus
  the process `VmHWM`/`VmRSS`, via a std-only, Linux-only `src/mem.rs` (`/proc/self/status` +
  `/proc/self/clear_refs`). Non-Linux hosts print `memory: unavailable on this platform`.
- `phg disasm <source>` — print the compiled bytecode: per-function instruction listings (index,
  source line, op, and a resolved annotation for index-carrying ops) plus the program-level
  enum/class/method descriptor tables.
- New profiling example `examples/bench/workload.phg` (CPU recursion + heap allocation) with
  `examples/bench/README.md` documenting how the time and memory numbers are collected.

### CLI UX

- `-v` / `--version` — print `phg <version>` and exit; `-h` / `--help` — full usage banner.
- Flexible program source for the run-family commands
  (`run`/`runvm`/`check`/`parse`/`lex`/`transpile`/`disasm`/`bench`): `<file>` | `-` (read from **stdin**) |
  `-e <code>` / `--eval <code>` (run **inline** source) | `--` (next arg is a path even if it starts
  with `-`).

### M2.5 Phase 2 — cross-OS standalone builds

- `phg build --target <triple>` / `--all` cross-compiles a runtime stub via
  [`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild) (zig as the linker) and embeds the
  program as a named object-file section. Targets: `x86_64-unknown-linux-musl`,
  `aarch64-unknown-linux-{gnu,musl}`, `x86_64-pc-windows-gnu`.
- `src/bundle.rs` → a `bundle/` module: CRC-guarded `container`, per-format readers `elf`/`pe`/`macho`
  (thin + fat), a magic-sniffing `section::find_section` dispatcher, and a `cross` orchestrator. The
  hand-rolled, std-only **PE/COFF**, **Mach-O 64**, and **fat/universal** readers use checked arithmetic
  (EV-7: adversarial input → `None`, never a panic) so a produced binary self-reads its own format.
- Stub cache keyed on an FNV-1a-64 of the phg binary's own bytes (a rebuilt phorj invalidates stale
  stubs, protecting the parity spine). Precise "missing rustup target" / "needs a source checkout"
  errors. apple/darwin targets are rejected with a clear message (macOS stub deferred to Phase 3; the
  Mach-O reader ships and is tested). `--sign` reserved for Phase 3.
- Cross-parity tests (toolchain-gated): `x86_64-musl` native-execution parity vs `runvm`, and a real
  windows-PE section round-trip.

### Documentation

- Full OSS project doc set: rewritten README, dual **MIT OR Apache-2.0** license, CONTRIBUTING,
  CODE_OF_CONDUCT, SECURITY, SUPPORT, GOVERNANCE, AUTHORS, ROADMAP, VISION, FEATURES, KNOWN_ISSUES,
  THIRD-PARTY-NOTICES, CITATION.cff, `.editorconfig`, and `.github/` templates.

Built standalone binaries are unchanged: they run their embedded program and ignore argv.

## [0.3.0] — 2026-06-16

First tagged POC. Usable end-to-end on `x86_64-linux-gnu`: the full M1 language on two
byte-identical backends (`run` interpreter + `runvm` bytecode VM), a Phorj→PHP transpiler, and
`phg build` producing a standalone native Linux executable. Bundles all post-M2-P3 work — the
P3.5 hardening pass, M2 P4 (classes/enums/match/methods), Wave 4 (class-aware compiler types), P5a
(`Rc`-shared heap), the full-coverage example set, and M2.5 Phase 1 (standalone build). Known v1
limits: `build` is host-only; the artifact ignores argv and always exits 0; the language has no
indexing/`Map`/`Set`/optionals/`|>`/exceptions/mutation (all M3).

### M2.5 Phase 1 — `phg build` (x86_64-linux-gnu) (2026-06-16) — **distribution**
`phg build foo.phg` produces a standalone host executable that runs `foo.phg` on the VM with no
Phorj install — by copying the running phg binary, embedding the program **source** in a
`.phorj` ELF section, and self-detecting + running that payload at startup. Same section+container
mechanism as the cross-OS end state (design §7). See
`docs/specs/2026-06-16-m2.5-phorj-build-design.md` + `docs/plans/2026-06-16-m2.5-phase1-build-linux-gnu.md`.

- **Added**
  - `src/bundle.rs` (std-only, zero new deps): a bitwise CRC-32, a versioned CRC-guarded payload
    **container** (`magic | version | header_len | kind | comp | enc | flags | len | payload_crc32 |
    header_crc32`), a hand-rolled **ELF64 section reader** (no `object`/`goblin` — it links into the
    produced binary, so it must stay zero-dep), and `embedded_source()` (graceful `None` on every
    malformed/tampered/absent input).
  - `cli::cmd_build` — validates the program (no broken binary is ever emitted), copies `current_exe`,
    and shells `llvm-objcopy --add-section .phorj=…` (override via `PHORJ_OBJCOPY`).
  - `phg build <file> [-o out]` CLI command; `main()` runs an embedded payload at startup before
    any arg parsing.
  - `tests/build.rs` — the parity spine extended to distribution: a built binary's output is
    byte-identical to `runvm`; argv is ignored (v1); ill-typed programs fail with diagnostics and
    emit no binary.
  - **Hardening (post-review):** the ELF64 reader uses fully-checked offset arithmetic — adversarial/
    malformed input returns `None`, never overflow-panics under the debug/test profile
    (regression-tested per EV-7); `phg build` rejects a dangling `-o`, an unrecognized flag, or any
    extra argument with a usage error (exit 2) instead of a silent default-named build. `docs/INVARIANTS.md`
    #1 now records the build binary as the third `cmd_runvm` parity surface.
- **Notes** (v1 limits) — host-only (`x86_64-linux-gnu`); the embedded program ignores argv and
  cannot set a custom exit code; the source is recoverable from the artifact (not obfuscated).
  Cross-targets (zig), PE/Mach-O reader arms + stub cache = Phase 2; CI stub registry + signing/
  notarization (rcodesign-from-Linux) = Phase 3.

### Examples — full-coverage showcase (2026-06-16) — **docs/tests**
A living example set covering the entire runnable language surface, plus the Phorj→PHP bridge. See
`docs/specs/2026-06-16-examples-coverage-design.md` + `docs/plans/2026-06-16-examples-coverage.md`.

- **Added**
  - Four real-world programs (`examples/realworld/{ledger,library,shop,rpg}.phg`) and six focused
    guide programs (`examples/guide/{operators,control-flow,collections,classes,enums-match,strings}.phg`),
    each exercising a different slice of the surface; an `examples/README.md` index + coverage matrix.
  - `examples/transpile/{demo.phg,demo.php,README.md}` — the Phorj→PHP transpile bridge (the only
    PHP-ecosystem path: output, not input), with a `tests/cli.rs::transpile_demo_matches_committed_php`
    snapshot test that fails on transpiler drift.
- **Changed**
  - `tests/differential.rs` now **globs `examples/**/*.phg`** instead of listing examples explicitly,
    so every current and future example is byte-identity-gated with no test edit.
- **Notes** (honest boundary, documented in `examples/README.md`)
  - Zero-payload enum variants need call form `V()` to construct **and** in a `match` pattern — a
    bare `V =>` arm is a catch-all binding (a silent logic bug both backends agree on).
  - `import` is decorative (no module resolution until M5); `null`/`T?`/`Map`/`Set`/`|>`/exceptions
    /traits/overloading remain M3+ and are deliberately absent.

### M2 P5a — `Rc`-shared heap objects (2026-06-16) — **object-path perf**
Makes compound heap objects *shared* instead of *deep-cloned*. The M1 heap is immutable + acyclic
(no reassignment, no field mutation, args evaluated before the instance exists), so `Rc` is both
sufficient and complete for reclamation — `Drop` frees everything, no cycle can leak, no tracing
collector is needed (that stays deferred to M3). See
`docs/specs/2026-06-16-m2-p5-object-model-design.md` + `docs/plans/2026-06-16-m2-p5a-rc-shared-heap.md`.

- **Changed**
  - `Value::Instance(Rc<Instance>)`, `Value::Enum(Rc<EnumVal>)`, `Value::List(Rc<Vec<Value>>)`
    (were `Box`/`Vec`). Cloning a `Value` — the `Op::GetLocal` hot path and every interpreter
    var-read — is now an O(1) refcount bump instead of a deep `HashMap`/`Vec` copy. The constructor
    now shares one `Rc` between the `this` receiver and the returned instance (no double build).
  - Three move-out sites adjusted (can't move out of an `Rc`): `vm.rs` `GetEnumField`
    (`into_iter().nth` → `.get().cloned()`), the interpreter's list `for` (iterate by ref + clone),
    and the ctor double-build (folded into one shared `Rc`). No `Op`/bytecode/AST/checker change.
- **Perf** (`phg bench`, median of 101, `fib(28)`)
  - Object-heavy VM run **1537 ms → 634 ms (2.4× faster)**; the VM's advantage over the tree-walker
    recovered from **4.73× → 9.35×**, essentially on par with the scalar baseline (10.92×) — i.e.
    the object-path penalty (deep-clone-on-load) is largely eliminated.
  - **Phase B deferred (bench-gated, not opened):** slot-indexed `Vec` field layout. With the object
    path now ~within scalar's advantage, field access (HashMap lookup) is no longer dominating, so
    there is no evidence to justify the larger interpreter-touching change.
- **Parity** — behavior-preserving refactor; the full differential suite + examples sweep stay
  byte-identical (244 tests green), clippy + fmt clean, `#![forbid(unsafe_code)]` intact.

### M2 Wave 4 — Class-aware compiler types (2026-06-16) — **closes the last `num_ty` parity gap**
Makes the compiler's operand-type inference class-aware, so the VM no longer rejects checker-valid
programs that read a field of an arbitrary instance, a method-call result, or a nested member as an
arithmetic operand. `runvm` is now a faithful drop-in across the full checker-valid surface. See
`docs/plans/2026-06-16-m2-wave4-compiler-types.md`.

- **Changed**
  - The compiler's coarse `enum TyTag { Int, Float, Other }` became `enum CTy { Int, Float,
    Class(String), Other }` — an instance now carries *which class* it is, derived structurally from
    the AST's declared `Type` annotations (`type_tag` → `resolve_cty`); the AST, the `Op` set, the
    VM, and `value.rs` are untouched.
  - `num_ty` is now the numeric projection (`as_num`) of a new recursive `ctype(&Expr)` resolver
    that walks `Ident`/`This`/`Member`/`Call` to a class-aware type. New per-program tables —
    `class_field_ctys` (class → field → type) and `method_rets` (`(class, method)` → return type) —
    plus a `cur_class` on the compiler back the `Member`/method-call/`this` resolution. The
    P4c-era `this.field`-only `num_ty` `Member` arm is subsumed by the general resolver.
- **Parity**
  - Five programs that ran on the interpreter but failed to *compile* on the VM now agree
    byte-identically (`tests/differential.rs::WAVE4_PROGRAMS`): a field of an arbitrary instance
    (`p.x + 1`), a method result (`c.get() + 1`), a nested field (`a.inner.x + 1`), a class-typed
    enum payload bound in `match` (`Some(p) => p.x + 1`), and a free function returning an instance
    (`mk().x + 1`).
  - The only remaining coarse-type note is the deliberately out-of-M1-surface `Index` (`xs[i]`
    arithmetic faults on both backends — M1 has no user indexing).

### M2 P4c — Methods + `this` on the VM (2026-06-16) — **M2 P4 complete**
Brings instance methods and `this` to the bytecode VM. With this, **`runvm` covers the full M1
language surface** and `examples/grades.phg` runs on both backends. See
`docs/plans/2026-06-16-m2-p4-classes-enums-match.md`.

- **Added**
  - `Op::CallMethod(name_idx, argc)` — runtime method dispatch off the receiver instance's class,
    via a program-level `(class, method) → function index` table; the frame opens with the
    receiver at slot 0 (`this`).
  - Methods compile to functions (receiver at slot 0, params at `1..=argc`); `this` and bare field
    reads inside a method/ctor body resolve against the receiver.
  - `examples/grades.phg` joined the differential examples sweep; `phg bench examples/grades.phg`
    runs (VM ≈3.2× the tree-walker on it).
- **Removed**
  - The last two `(M2 P4)` compile-error stubs (`Expr::This`, method calls) — `grep "M2 P4"` in
    `compiler.rs`/`vm.rs` is now clean.
- **Parity notes**
  - Method existence is checker-enforced, so the VM's method-not-found fault is a defensive
    backstop (no `agree_err` case, like P4a's exhaustiveness).
  - `num_ty` now classifies a `this.field`/bare-field arithmetic operand (via the class's field
    tags). At this commit a field read on an *arbitrary* instance was still the coarse-`TyTag` gap;
    **closed in M2 Wave 4** (see the Wave 4 entry above) by making the type class-aware (`CTy`).

### M2 P4b — Classes on the VM (2026-06-16)
Brings class construction (with constructor promotion + body side effects) and field reads to the
bytecode VM. See `docs/plans/2026-06-16-m2-p4-classes-enums-match.md`.

- **Added**
  - `Op::MakeInstance` (build a `Value::Instance` from promoted-field values) and `Op::GetField`
    (runtime field lookup, with a `no field` fault byte-identical to the interpreter).
  - A program-level `ClassDesc` table (per-class promoted-field names) and an interned
    field-name pool, both validated by `BytecodeProgram::validate`.
  - Each constructor compiles to a synthetic `<Class>::new` function: it promotes its params into
    fields via `MakeInstance`, runs the body for side effects with the instance in scope, and
    returns the instance. `ClassName(args)` resolves to a `Call` into it.
- **Object model**
  - Instances are value-native: the VM reuses the shared `Value::Instance`, clone-on-use,
    mirroring the interpreter (decision P4-1). No arena.
- **Parity notes**
  - A ctor body's `return` is discarded and the promoted instance is always returned (interpreter
    parity): the synthetic ctor redirects body `return`s to an epilogue that loads + returns the
    instance, so an early `return;` cannot change the result.
  - Reading an explicit (uninitialized) `Field` member type-checks but faults `no field` at
    runtime on **both** backends — construction populates only promoted ctor params.
- **Known limitation at this commit (coarse-type gap — since closed in M2 Wave 4)**
  - A field read used as the *direct left operand* of arithmetic (`p.x + …`) couldn't be classified
    by the compiler's coarse `TyTag`. Field reads worked everywhere else: interpolation, equality,
    call arguments, arithmetic right-operand, or bound through a typed local first. **M2 Wave 4
    closed this** by making the compiler's type class-aware (`CTy`); see the Wave 4 entry above.
  - `examples/grades.phg` still needs P4c (it calls an instance method).

### M2 P4a — Enums + `match` on the VM (2026-06-16)
Brings single-payload enums and exhaustive `match` to the bytecode VM (already in the
interpreter since M1). See `docs/plans/2026-06-16-m2-p4-classes-enums-match.md`.

- **Added**
  - `Op::MakeEnum`/`MatchTag`/`GetEnumField` (enum construction, variant tag test, payload
    extraction) + `Op::MatchFail` (checker-unreachable non-exhaustive backstop, byte-identical
    to the interpreter's fault).
  - A program-level `EnumDesc` table (the enum analogue of the constant pool), validated by
    `BytecodeProgram::validate`.
  - Compiler operand-height tracking, so a `match` used mid-expression (e.g. as a binary
    operand, or nested in another arm) spills its scrutinee to the correct stack slot.
- **Object model**
  - Enums are value-native: the VM reuses the shared `Value::Enum`, clone-on-use, mirroring the
    interpreter (decision P4-1). No arena — deferred to a bench-gated perf milestone.
- **Known limitation (pre-existing, shared by both backends)**
  - `match` cannot appear inside string interpolation — the lexer's `{…}` interpolation does not
    nest a `match`'s braces. Not a parity issue (both backends reject it identically).

### M2 P3.5 — Hardening (in progress, 2026-06-16)
Closing the parity/no-crash contract gaps before P4 widens the surface. See
`docs/plans/2026-06-16-m2-p3.5-hardening-roadmap.md`.

- **Added**
  - `phg bench <file>` — median-of-N timing of both backends, output-identity gated; measures
    the "VM faster than tree-walker" thesis (≈10× on `examples/fib.phg`) instead of asserting it.
  - `agree_err` error-parity oracle in the differential harness (faults classified by semantic
    `FaultKind`).
  - Central `src/limits.rs` (recursion/nesting caps + numeric-width policy); unified
    `diagnostic::Diagnostic` for all stages; `BytecodeProgram::validate`; `docs/INVARIANTS.md`,
    `docs/ARCHITECTURE.md`; `rust-toolchain.toml`.
- **Changed**
  - Arithmetic/comparison single-sourced into `value.rs` (both backends call the same kernels).
  - VM runtime errors now carry the source line (`Chunk.lines`).
  - Constant pool interns scalar duplicates.
  - `interpreter::Frame` → `CallScopes` (removes the name collision with `vm::Frame`); scope-verbs
    unified (`push_scope`/`pop_scope`).
  - Quality gate is now compile-time (`warnings = "deny"`, `clippy.all = "deny"`,
    `#![forbid(unsafe_code)]`) + a tracked pre-commit hook.
- **Fixed**
  - `Op::Neg` on `i64::MIN` aborted the VM (P0) — now a clean `integer overflow` fault, matching
    the interpreter.
  - Interpreter/parser/checker no longer SIGABRT on deep recursion/nesting — explicit limits fault
    cleanly.
  - Determinism: checker's non-exhaustive-`match` error sorts its missing-variant list.

## M2 — Bytecode + VM (P1–P3, 2026-06-16)
- **P1** — `Chunk` + typed `Op` enum + stack VM dispatch loop.
- **P2** — AST→bytecode compiler for the `main`-only surface + `phg runvm` + the differential
  harness (`runvm` byte-identical to `run`).
- **P3** — user function calls, clox-style call frames, recursion/mutual recursion; `examples/fib.phg`
  runs on the VM.

## M1 — Tree-walking interpreter + transpiler — 2026-06-15 (`9da6e56`)
- Full pipeline: lexer → parser → type-checker → tree-walking evaluator.
- Phorj → PHP transpiler, round-trip-verified against real PHP.
- CLI: `phg <run|check|parse|lex|transpile>`.
- Language surface: static types, immutable-by-default bindings, functions, classes + constructor
  promotion, single-payload enums + exhaustive `match`, string interpolation, `List<T>` literals,
  `for…in`, checked int/float arithmetic. 162 tests green at the tag.
