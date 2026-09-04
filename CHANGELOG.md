# Changelog

All notable changes to Phorj. Format follows [Keep a Changelog](https://keepachangelog.com/);
the project is pre-1.0 and unpublished, so versions track milestone progress, not a release
cadence. Milestones and their status live in `docs/MILESTONES.md`.

## [Unreleased]

### Fixed

- **The surface ratchet now measures what the compiler and server actually emit.** Three blind
  spots, found the moment a new capability shipped and the numbers did not move:
  (1) it scanned the `phg explain` catalog as if explanations were emit sites, so the catalog's
  deliberate tombstones (`E-MODULE-UNAVAILABLE`, DEC-273) counted as "unasserted debt" no test could
  ever pay; (2) it recognised only the standalone `"E-FOO"` form — the loader's 25 bracketed
  `[E-FOO]` codes and the transpiler's 3 `"E-FOO: …"` prefix codes were being counted only because
  the catalog happened to name them, and vanished the instant the catalog was excluded; (3)
  `lsp_providers` matched `"…Provider"`, which cannot occur inside `INITIALIZE_RESULT` (every quote
  there is `\"`), so it was counting the provider names that TESTS quoted, not what the server
  advertised. All three fixed; the baseline is re-frozen from the honest scan. `E-VENDOR-MISSING`'s
  explain entry — which described a live guard nothing has emitted since DEC-282/316 folded it into
  `E-MODULE-NOT-FOUND` — is now a tombstone, and ADR 0005 no longer promises the dead code.


- **Every registered PHP-builtin lift is now proven to FIRE**, not merely spelled. Resolution is
  arity-gated in `lift::lifter::exprs`, and a mismatch fails SILENTLY — the call stays a bare PHP
  name while the registration still greps as handled. Three builtins were checked by name
  (`strlen`, `strtoupper`, `sqrt`); the other 70 rested on a uniqueness test that lifts nothing.
  `every_registered_builtin_lifts_end_to_end` now lifts all 74 registrations and demands two
  independent signals — the module import appears AND the call comes back qualified. The second
  signal is deliberately "qualified", not "the builtin's name is gone": 32 rows share their PHP name
  (`sqrt`, `min`, `log`, `exp`…), so a name test would have reported every one of them dead.
  Measured result: **no dead registrations**, and no optional-argument gap either — `substr($s, 1)`,
  `number_format($n)`, `round($n)`, `str_pad($s, 5)`, `array_slice($a, 1)` and ten more all resolve
  at their common shorter PHP arity.


- **The `__phorj_*` helper header is now RATCHETED against the registry**, not merely written beside
  it. `src/transpile/helper_buckets.rs` declared 68 + 105 = 173 helpers while `HELPER_BUCKETS` held
  71 + 116 = 187, with seven names absent from the `//!` lists (`cs_decode`, `cs_encode`, `cs_name`,
  `fold_accents`, `sleep`, `wordwrap`, `proc_run`) — and the doc comment on the existing test claimed
  the count "cannot drift again" while nothing checked it. It could, and had, for four months. The
  new `the_module_header_matches_the_registry` asserts THREE ways per bucket — the count declared in
  the heading, the length of the name list under it, and the registry — plus the grand total, with a
  vacuity guard so a broken slice fails loudly instead of comparing two empty sets. Sabotage-verified:
  deleting a `//!` name, decrementing a heading count and adding a bogus name each turn it red. The
  DEC-377 family reasons for the new rows are stated as that decision requires.

### Added

- **LSP signature help** (`textDocument/signatureHelp`, trigger characters `(` and `,`) — Invariant
  17's 100% RULE names it explicitly, and it was the one named capability the server did not
  advertise. Inside a call's parentheses the client shows the callee's signature with the argument
  being typed marked active: your own functions (same file, or a same-package sibling in another
  open buffer) and every `Core.*` native straight from `native::registry()`, so a new native is
  signature-helped the moment it is registered. The parameter list is sliced from the SAME
  signature text hover renders, so the two cannot disagree; the declaration's `/** … */` doc travels
  with it (DEC-419). **It works while the buffer does not parse** — inside an unclosed `(` it never
  does, so a parser-only lookup would have been silent at exactly the moment the feature is for; the
  same-file path falls back to the `function <name>(` token sequence. Which call and which argument
  is decided by a forward scan that skips strings (including the triple-quoted form) and comments,
  because a `,` or `)` inside a literal would otherwise move the hint to the wrong argument of the
  wrong call. VS Code extension `0.6.0` and the LSP4IJ guide document it in the same change
  (DEC-181); the extension itself needs no code — both clients negotiate the capability from
  `initialize`.


- **`array_slice` lifts to `List.slice`** (DEC-312's inverse). Claimed after checking the two
  directions agree on every edge PHP treats specially — negative offset, negative length, over-long
  length, offset past the end, offset before the start — against real `php-8.5.9` output: 7/7
  identical on the interpreter, the VM and the transpiled PHP. It is the ONLY uncontested core-PHP
  emit left; `count`, `array_values`, `array_merge`, `strlen` and `pow` are each already claimed by
  the dominant-idiom row, and `lift_from_builtins_are_unique` forbids a second claimant.

- **`Runtime.onShutdown(fn)` + `Runtime.isShuttingDown()`** (DEC-204; shape DEC-497, query DEC-498).
  Handlers run after `main` returns, after a FAULTING `main` (same reason a `finally` block is not
  skipped by the exception that triggered it) and on `Runtime.exit`, in registration order, with
  their output on the same stdout `main` was writing. A signal does not run them by itself — a phorj
  closure cannot run inside a signal handler, and a `Value::Closure` is not `Send`, so what a signal
  does is wake `Time.sleep` early and flip `isShuttingDown()`; a loop cooperates and the handlers run
  on the way out. A loop that never checks still hard-kills: the guarantee is cooperative, not
  pre-emptive. Transpiles to core `register_shutdown_function`; `isShuttingDown()` emits `false` on
  the PHP leg, which is correct rather than a stub. `examples/guide/on-shutdown.phg`.

- **`String.foldAccents` — accent folding for slugs and search keys** (DEC-468's second half, shape
  ruled as DEC-496). Folds all 190 accented Latin letters in U+00C0–U+017F to their ASCII base:
  `Crème Brûlée` → `Creme Brulee`, `Łódź` → `Lodz`, `Člověk` → `Clovek`. That range is exactly the
  alphabet `Core.Encoding`'s six charsets can produce, which is why the two shipped together.

  **Case is preserved and output length can differ from input.** Folding is not lowercasing, so `À`
  → `A`; and characters with no single-letter base expand — `ß` → `ss`, `æ` → `ae`, `Æ` → `AE`
  (never title-cased `Ae`), `Ĳ` → `IJ`, `þ` → `th`. A fold is therefore NOT a per-character map, and
  an index into the folded string does not point at the same place in the original. Anything outside
  the range passes through untouched: Greek and CJK are not mangled.

  The table was **generated from Unicode NFD** (decompose, drop combining marks) rather than typed,
  with the expansions stated per character because no decomposition defines them; the PHP leg emits
  `__phorj_fold_accents` built from that same table via core `strtr`, so the two legs cannot drift
  and no ini extension is involved (`iconv(…,'ASCII//TRANSLIT',…)` is both an extension and
  locale-dependent, so it is not byte-identical). `examples/guide/fold-accents.phg`.

- **Charset transcoding — `Core.Encoding.decode`/`encode` over a typed `Charset` enum** (DEC-468's
  surface, DEC-494's strategy, DEC-495's shape). Six encodings ship: UTF-8, UTF-16 in both byte
  orders, ISO-8859-1, ISO-8859-15, Windows-1252 and ASCII. Both directions return an **optional** —
  `null` when the bytes are not valid in the source charset or a character has no representation in
  the target, so nothing is replaced with U+FFFD or `?` behind the caller's back. `Charset` is an
  injected enum gated on `import Core.Encoding`, so a typo is a compile error rather than a runtime
  mojibake bug, and UTF-16 is `Utf16Le`/`Utf16Be` with no bare `Utf16`: byte order is not
  recoverable from the bytes without a BOM, so the caller states it and decode stays total.

  **It transpiles, with no ini extension.** DEC-468 had named `encoding_rs`; that was ruled out
  because the PHP leg has no legal move with it — `mb_convert_encoding` and `iconv` are shared
  extensions, absent under the oracle's `php -n` and rejected by the default-deny tier-1 guard, so
  the alternative was an `E-TRANSPILE-*` exclusion at the exact moment DEC-493 forbade parked items
  at the finish line. Instead both legs are hand-rolled and the tables in `src/charset.rs` are
  **formatted into** the emitted `__phorj_cs_decode`/`__phorj_cs_encode` helper at transpile time, so
  the native leg and the PHP leg read one source and cannot drift. No new dependency (the count
  stays 15). `examples/guide/charset.phg`; `phg run` ≡ `--tree-walker` ≡ transpiled PHP.

### Fixed

- **A lambda whose body is a void native emitted PHP that does not parse.** `Output.printLine`
  lowers to `echo`, which is a **statement** in PHP, so `function() => Output.printLine(m)` — the
  natural shape for a `Runtime.onShutdown` handler — emitted `fn() => echo "m", "\n"` and the
  transpiled file died with `syntax error, unexpected token "echo"`. The program ran correctly on
  both native backends, so only the PHP leg saw it. A statement-shaped body now falls back to the
  block-closure form (`function() use (…) { echo …; }`), which takes statements by construction and
  lists captures explicitly, since `function () use (…)` does not capture implicitly the way `fn`
  does. The arrow form is unchanged for value-returning bodies, pinned by
  `a_lambda_whose_body_is_a_statement_emits_the_block_closure_form`.

- **Request framing: a pipelined second request is answered, and the remaining RFC 9112 framing
  shapes get their ruled status (panel round 4, safety F4 P1 / F5 / F7).** `read_http_request`
  returned the whole buffer it had read, never truncated to the declared body, so a second request
  arriving in the same segment became the first request's BODY and was never answered (§9.3.2). The
  framing now returns exactly the declared body and CARRIES the bytes past it to the next read at all
  three sites (the single-threaded transport's kept-alive and fresh paths, the pool worker's loop —
  each turn sees exactly one request, so the keep-alive checks stay per-request). A header VERDICT
  (`framing_verdict`) replaces the Content-Length-only check: duplicate DIFFERING `Content-Length` →
  400; `Transfer-Encoding` beside `Content-Length` → 400; `Transfer-Encoding` alone → **501** (no
  transfer coding is implemented — §6.1, chosen over 400 deliberately); whitespace before a header's
  colon → 400 (§5.1); a declared body cut short by FIN → 400 (§6.3 ¶6, it used to be served); a body
  over the 8 MiB cap → **413** (it used to be truncated and served); a non-representable
  `Content-Length` stays 400 like `abc`. Every reject is `Connection: close` and the send error is
  logged like the sibling path. obs-fold continuations are still accepted (MAY). Red-first: a
  two-requests-in-one-write raw-socket test and one per shape; framing unit tests for the carry-over
  and every verdict; sabotages: the carry dropped (two tests red), the 501 verdict collapsed to 400.
- **Regex: syntax the native engines and PCRE read differently is rejected on BOTH constructors, a
  directly constructed `Regex` value is validated at first use, and the PCRE backtrack limit on a
  regular pattern is a loud, named PHP-leg fault (panel round 4, correctness R1–R7 / safety F1–F3).**
  A second reject scan (`ext::regex::reject::pcre_divergent`, ported to PHP as
  `__phorj_regex_pcre_divergent`) refuses class-set operators and nested classes, POSIX classes,
  `\v`/`\V`, `\<` `\>` `\b{…}`, the inline `u`/`R` flags and the PCRE-only constructs neither crate
  implements — at check time for a literal (`E-REGEX-UNSUPPORTED`, with a "rewrite portably" hint
  distinct from the linear-only hint; `validate` now returns a typed `RejectKind`) and at run time on
  every leg for a dynamic pattern; 23 constructs × 2 constructors × literal + dynamic pinned through
  real PHP, plus 10 portable controls. `new Regex(p, e)` is validated at first use by the PHP helpers
  (`__phorj_regex_validated`, memoized), so both legs fault. **Behaviour change:** the range's
  `__phorj_regex_check` turned a PCRE backtrack-limit hit into a fault; for a catastrophic pattern that
  stays inside the regular subset (`^(a+)+$`), which the native engines match in linear time on both
  constructors, that replaced a silent PHP `false` (byte-identical only for a non-matching subject) with
  a loud fault naming the PHP-leg limitation — ruled direction: loud, disclosed in KNOWN_ISSUES
  §Core.Regex, the spec rows and the example. Four sabotages red (the Rust scan, its PHP port, the
  first-use validation, the limit split).
- **Diagnostics and VM faults inside a string interpolation name the real line (W0-5 / H §5,
  INTERP-LINE-RESET; panel round 4).** The interpolation sub-tokenizer restarts at 1:1 and the parser
  re-based only each re-lexed token's `start`, so `phg check` reported `1:1` for an error inside
  `"{…}"` and the VM reported fault line 1 where the interpreter reported the true line — an
  Invariant-1 failure-behaviour divergence disclosed since W0-5 and mis-diagnosed as needing VM debug
  symbols (W5-13). `StrSeg::Interp` now carries the interpolation's own line/col and
  `segments_to_parts` re-bases `line`/`col` too (text blocks shift lines by the block's line). The
  `#[ignore]`d `interpolation_fault_line_matches_between_backends` gate is un-ignored and green (its
  expected lines were one short and corrected); `a_diagnostic_inside_an_interpolation_names_the_real_line_and_column`
  pins the front-end half and the empty `"{}"` case; a sabotage dropping the line re-base reds the gate.

### Added

- **`Regex.compileBacktracking` — the opt-in backtracking engine (DEC-461, REGEX-B; `fancy-regex`
  0.11 is the 15th vetted dependency).** PCRE-class syntax the linear engine deliberately omits —
  look-around, back-references, atomic groups, possessive quantifiers — with the same query API and the
  same `preg_*` under PHP, under a STEP BUDGET: a catastrophic pattern raises `regex step budget
  exceeded` on every leg (PHP's `PREG_BACKTRACK_LIMIT_ERROR` maps to it) instead of hanging, so ReDoS
  stays opt-in and bounded. The `Regex` value carries a second public field, `engine`. `Regex.compile`
  is untouched (linear, ReDoS-immune). Examples: `guide/regex.phg`; `phg explain E-REGEX-UNSUPPORTED`.
  Lift needs nothing (Invariant 17): the lifter has no `preg_*` model.

### Fixed

- **The regex cluster (panel C1–C3, C5, C11, F3).** (C2/C5) The linear engine now REJECTS every
  PCRE-only construct — at check time for a literal pattern (`E-REGEX-UNSUPPORTED` / `E-REGEX-INVALID`,
  the checker validating with the engine that would compile it) and at run time for a dynamic one, with
  the PHP twin `__phorj_regex_compile` porting the same reject scan — so `a++` no longer parses as
  `(a+)+` natively while PCRE reads it as possessive, and `(?=b)`/`(a)\1`/`\h`/`\R`/`\Z`/`{,n}` no
  longer fault natively while PHP says `true`. (C1) `Regex.replace`'s replacement grammar is phorj's
  own, expanded by `ext::regex::replace` and `__phorj_regex_expand` identically: `\1-`, `$$`, `$1a`
  and `${x}` agree on every leg. (C3) `__phorj_regex_delim` emits `D`, so `a$` means end of subject
  under PHP too. Every `preg_*` error is now a fault (`__phorj_regex_check`), never a silent
  `false`/`null`. (C11/F3) KNOWN_ISSUES §Core.Regex rewritten — the `\d\w\s` ASCII-vs-Unicode edge
  never existed (`u` ⇒ UCP). C4 (empty-match placement) is deferred by the REGEX-B boundary ruling. The
  PHP helpers moved to `runtime_php_regex.rs`. (Follow-up, same day: the example header and the natives
  module doc still carried the retracted `\d\w\s` claim; a check-time `E-REGEX-INVALID` case added.)
  Red-first `agree_out_php`/`agree_err_php` cases for every
  row plus the backtracking positives and the budget; four sabotages red (the `D` modifier, the PHP
  expander, the checker gate, the possessive detector).
- **A malformed `Content-Length` is a `400` and the connection closes (RFC 9112 §6.3; panel
  C10/F4).** `Content-Length: abc`, `-1` or a 24-digit value parsed to 0 through an `unwrap_or(0)`, so
  the framing read no body and the request was SERVED body-less with `200` — pinned as intended by a
  unit test. The parser now accepts `1*DIGIT` only (`Ok(0)` when absent, `Err` when malformed), and all
  three framing sites (the single-threaded transport's fresh and kept-alive paths, and the pool
  worker's loop) answer a fixed `400` with `Connection: close` and drop the socket before any handler
  runs — the request boundary is unknowable, so the connection cannot be reused. Red-first in
  `tests/serve.rs` (single-threaded and pool paths, raw sockets) and the framing unit test flipped;
  three sabotages red (each transport guard disabled, the digit check loosened).
- **Prelude-internal bindings are isolated from user imports (DEC-459 built; panel F6;
  KNOWN_ISSUES §PRELUDE-ALIAS-COLLISION).** Every `import Core.Native.X as A;` a Core prelude declares
  is rebound at injection under `A#prelude` — `#` is not an identifier character, so no user token can
  spell it — with the alias set computed over every fragment (the serve fragment calls
  `NativeHttp.registerServe` through the request fragment's import). Three user-visible defects fall
  with it: a user `import Core.Native.Http as Raw;` no longer makes the injection drop the prelude's own
  import (it compared module paths only) and fail with `E-UNKNOWN-IDENT` at prelude lines the user
  cannot open; a user alias spelled `NativeHttp` no longer captures the prelude's calls; and
  `NativeHttp` / `NativeInput` / `NativeUri` / `NativeDebug` no longer resolve in user code without an
  import ("in the wind"). The by-name containment arm in the transpile ladder is removed with its test
  flipped to `E-UNKNOWN-IDENT`. A user's own `import Core.Native.Http as NativeHttp;` (the spelling
  `E-IMPORT-NATIVE-MEMBER` recommends) keeps binding the user's qualifier; `E-UNUSED-IMPORT` is the
  loader's raw-file check and is unchanged. Injected imports are exempt from the
  alias PascalCase rule by span (`Span::is_injected`, with `INJECTED_SPAN_BASE` moved to `token`).
  Red-first in `tests/prelude_isolation.rs` (the repro runs on both backends); three sabotages red,
  including "rename the import line but not the uses", which breaks the serve prelude; the corpus
  gates stayed byte-identical and no isolated name reaches emitted PHP.
- **The two example-corpus gates account for every skip (panel K4).** `all_examples_match_between_backends`
  and `all_examples_transpile_and_match_php` had two `continue` skip arms each, no counter and a floor
  of `files.len() >= 3` — the shape that let the DEC-191 substring hole skip 201/201 examples behind a
  green suite — and the PHP gate's summary printed "218 examples gated" while 22 of those were silent
  skips (18 impure, one feature-gated file falling into an unlabelled "non-runnable" arm, three ladder
  quarantines). Both gates now tally RUN/SKIP by reason (`interp≡vm: RUN 199, SKIP 19 (…)`,
  `php-oracle: RUN 196, SKIP 22 (…)`), floor the RUN count near today's value (that pins discovery), and
  require the skipped set to equal an explicit expected list in both directions — an unexpected skip
  and a stale entry each fail with the file named. The feature-gated bucket is exempt from the exact
  match; a 6C review showed that left a wrongly-gated running example four files of silent headroom under
  the floor, so every bucket is matched now (one entry covers `http-client/fetch.phg`, feature-gated under
  the default features and impure under `--all-features`), and a build with no gated module must still
  show the bucket empty. The PHP gate gains the same feature-gate skip as its sibling, and its
  "non-runnable" arm is now a panic (the interp ≡ VM gate asserts every non-impure, non-gated example
  runs, so a file reaching it means the gates drifted). The project-corpus gate carries the same tally
  (`projects: RUN 19`, floor 18). Four sabotages each red with the file named (a widened impurity
  predicate, a widened feature-gate predicate, a raised floor, a stale expected entry).
- **The loader's expression walk is total (DEC-356 class; panel K8).** `src/loader/resolve.rs`'s
  `resolve_expr` ended in a named `leaf => leaf` that swallowed eight `Expr` variants; four were live
  defects in any LIBRARY file — a same-package call inside a tuple literal, a named-argument value or
  a pipe (both sides), and a type argument of `new List<T>()` — all surfacing on the merged unit as
  `unknown function` / `unknown type` for a program every single-file check accepts. The arm's own
  comment had recorded the same bug for `ParentCall` and `Map`, fixed one variant at a time; the walk
  now carries explicit arms for all eight (`Spawn`, `TaggedTemplate`, `Inject` resolved too;
  `OverloadSelect` is checker-constructed and kept as a visible inert arm) and ends in the
  single-sourced `expr_leaves!()` set, so the next variant with sub-expressions fails to compile
  instead of being swallowed. The file joins the DEC-356 source-scan ratchet. Red-first in
  `tests/project.rs` (all four shapes in one library file, run ≡ VM); sabotaged twice (one arm
  reverted → red; the catch-all restored → the ratchet reds).
- **A document with a `test` item is checked in test mode by `phg check`, `check --json` and the
  LSP (DEC-486; Invariant 17 `check ≡ LSP ≡ test`; panel C9/K1/K7).** Both editors squiggled
  `E-TEST-OUTSIDE-TESTS` on every `selftest/*.phg` and `phg check` rejected them, on lines `phg test`
  accepted. The flag is now derived from the document (`ast::has_test_items`) in exactly two places —
  `check_and_expand_for_check` (check) and `front_end_diagnostics_result` (LSP + `--json`) — while
  `run`/`transpile`/`build` and the bundle gate stay strict, so a release still cannot carry a test
  block; the diagnostic's wording and hint now say so. The outline lists each test (SymbolKind
  Function), so both editors' breadcrumbs reach it with no extension change. Two siblings fell with
  it: `phg check --json` called the RAW checker (an injected-prelude program `phg check` accepted came
  back as `unknown function \`Secret\``, reproduced on the release binary), and the loader's
  `resolve_item` ended in a named catch-all, so a `test` body in a library file was never mangled
  (DEC-356 class; `new Box()` → `unknown function`). Red-first in `pipeline_tests.rs`,
  `lsp/tests_test_mode.rs` and `tests/mtest.rs` (every selftest file through `phg check`, and a
  two-file project whose imported test checks but does not run); six sabotage mutations each went red.
- **The two ungated PHP-emit paths are gated (Invariant 14; panel round-3 C7/C8/F1).** The
  playground's `transpile_json` and `phg benchmark --vs-php` reached `transpile::emit` without the
  native-only ladder refusal: the playground emitted a `Core.Database`/`Core.SessionModule` program,
  EXECUTED it under php-wasm and rendered "outputs differ" where `phg transpile` says `E-TRANSPILE-DB`;
  the benchmark reported "transpile divergence" with exit 0. Both now go through the gate
  pre-expansion — `cli::transpile_source` is the single-source chokepoint (CLI + playground), and the
  benchmark gates only its PHP leg. Two new tests pin the refusals; `KNOWN_ISSUES
  §BENCHMARK-SKIPS-LADDER-GATE` is closed.
- **Two project files at the same byte offset no longer swap each other's default-filled
  arguments (P0, KNOWN_ISSUES §default_fills; panel round-3 C6).** Every checker rewrite map keys on
  `Span.start`, and every file was lexed from offset 0, so `new Box("AAA")` in `main.phg` and
  `new Box("BBB")` in a sibling file at the same offset shared one key — the later file's fill was
  spliced into both, on the interpreter, the VM AND transpiled PHP alike (all legs agreed, all wrong,
  so the byte-identity harness could not see it). The loader now gives every non-entry project file and
  every ambient `*.d.phg` its own disjoint offset window (`loader::fs::SpanWindows`, cumulative, kept
  below `INJECTED_SPAN_BASE`); the entry file stays at base 0 and `line`/`col` are untouched. Pinned by
  a three-file differential case whose expected output is stated, plus a diagnostics case proving a
  rebased file still reports its own `line:col`. The 2026-07-17 §span-collision entry (the prelude axis,
  closed 2026-07-31) is corrected to say both axes are now closed.
- **Six defects in the item-level AST walks — four crashes, an Invariant-1 spine break, and a broken
  overload dispatch — all on valid code `phg check` called clean.** DEC-356 made the
  `Expr`/`Stmt`/`Pattern` walks exhaustive and its ratchet watches the six extracted `*_walk.rs`
  files; the identical defect survived one level up, in the ITEM walks of their parent files. A
  trait's members are a full `Vec<ClassMember>` whose bodies **execute** (they flatten into the using
  class), so every walk ending in `other => other` was skipping executable code while reading as
  though it skipped a declaration. Each was verified end-to-end against the release binary with a
  class control proving the asymmetry, and each is now a red-first differential test:
  `html"…"` in a trait method and in a **field initializer** (both `unreachable!`, exit 101);
  a UFCS call in a trait method (`unknown field`); `inject<T>()` in a trait method
  (`unreachable!("inject() not expanded")`); a **generic method in a trait**, where both native legs
  printed `7` while the transpiler emitted `function echoBack(U $x): U` and PHP died with
  `TypeError: must be of type U` — three legs disagreeing, which is what Invariant 1 forbids; and a
  return-overloaded trait method, whose call site was mangled while its declaration was not
  (`unknown field read__ret_int`, with the PHP leg silently falling back to a different dispatch
  model). `item_leaves!()` now joins the three macros in `src/ast/leaves.rs` — holding `Import` and
  `TypeAlias` only, because `Interface` and `Enum` both carry `Expr` — nine item walks carry explicit
  arms, and the DEC-356 ratchet was widened to the parent files, where it immediately caught two more
  `_ => {}` collection loops. Two behaviours are deliberately unchanged and now visible as named arms
  rather than swallowed (a `#[Route]` static in a trait still does not register; a `TypeAlias` name
  still does not block a colliding variant import) — both are the developer's call. **Not closed, and
  not claimed as closed:** no item-level pass walks param defaults or attribute arguments, for any
  item form. CD-31 + addendum.

- **`phg test` ran the RAW checker, so injected-prelude programs could not be tested at all.** The
  runner called `checker::check_tests` directly — no prelude injection, no intrinsic/variant import
  resolution, no DI/Db desugar — so every symbol existing only in an injected prelude came back as the
  user's own `E-UNKNOWN-IDENT`, and the first one cascaded into three more messages that all blamed the
  user's file. Any program importing `Core.Http`, `Core.Database`, `Core.Secret`, `Core.DI` or
  `Core.Runtime.EntryKind` was untestable while `phg check` and `phg run` accepted it happily. This is
  precisely the hole DEC-252 closed for the LSP and left open here: the LSP was routed through the
  shared front end, `phg test` never was, so `phg check` ≡ LSP held while `phg check` ≢ `phg test`.

  Fixed by threading a test-mode flag through the SHARED pipeline — `checker::check_resolutions_mode`
  and `cli::check_and_expand_tests` — rather than giving test mode a pipeline of its own, which would
  have recreated the exact divergence being closed. ⚠ **Correction (same day, by milestone panel):
  the first version of this entry claimed `phg check` ≡ LSP ≡ `phg test` "now holds by construction".
  It does not.** Injected-prelude TYPES resolve under `phg test` — that part is real — but the LSP still
  calls the non-test path, so every `selftest/*.phg` squiggles `E-TEST-OUTSIDE-TESTS` in both editors;
  and `resolve_variant_imports` / `desugar_router` are item-level walks ending in a named catch-all, so
  they never descend into `Item::Test` and two of the three causes named above are still absent inside a
  test body. Both gaps are recorded in KNOWN_ISSUES §TEST-RAW-CHECKER, which is now marked PARTIALLY
  FIXED rather than fixed. Guarded by a new `tests/mtest.rs` case and by `selftest/injected_preludes.phg`, a real
  Invariant-9 surface whose every import reaches a prelude-injected type; sabotage-verified by pointing
  the runner back at the raw checker. ⚠ The first version of this entry recorded that as turning "that
  one test red and leaving the other eight green", which is impossible at HEAD and was caught by the
  panel: `the_selftest_suite_is_green` runs the whole `selftest/` directory, which now contains the new
  file, so TWO tests must go red. The sabotage was genuinely run — but before the selftest file was
  added, and the two were then described in one sentence. The check was real; the record of it was
  wrong.

  Checked rather than assumed while building it: `EntryKind` is injected only as the attribute-argument
  surface for `#[Entry(kind:)]`, so using it as a value type is `E-UNKNOWN-TYPE` under `phg test` — and
  identically under `phg check`. The agreement is the property being restored; here it simply shows up
  on a rejection rather than an acceptance.


- **The transpile ladder had a bypass, and it shipped: `Core.Native.Http.registerServe` emitted PHP
  that could not run.** `E-TRANSPILE-SERVE` was keyed on the `Http.serve` call and nothing else, so
  registering a handler through the raw twin instead — `import Core.Native.Http as NativeHttp;` then
  `NativeHttp.registerServe(cfg, h)` — walked straight past it. `phg transpile` exited 0 and emitted
  `__phorj_http_register_serve(...)`, a helper no family defines; the native legs ran clean while the
  PHP leg died with `Call to undefined function` (exit 255). That is Invariant 1's byte-identity spine
  broken by a transpile the toolchain reported as a success, and tier 2 of THE LADDER RULE
  (Invariant 14) requires a hard error at transpile time instead. The bypass was not an obscure
  corner either: `E-IMPORT-NATIVE-MEMBER` actively recommends the whole-module import spelling that
  reaches it.

  Closed the way DEC-277 closed the identical hole for the four sibling raw twins
  (`Core.Native.{Database,Session,HttpClient,Mail}`) — a module row in `reject_native_only_transpile`
  carrying the same `E-TRANSPILE-SERVE` code as the friendly spelling. The two layers now coexist:
  one keyed on the call, one on the user's import.

  ⚠ **Two corrections from the 2026-09-02 milestone panel, in opposite directions.**

  **(a) The hole was NOT closed by the import row alone.** A third spelling needed no import at all:
  the injected prelude leaks its `NativeHttp` alias into user scope, so `NativeHttp.registerServe(…)`
  type-checked clean, transpiled at exit 0, ran on both native legs and fatalled on PHP at exit 255 —
  and `phg build --php` emitted the broken helper while printing a SUCCESS banner. A call-keyed
  containment arm now refuses it (sound only pre-expansion, like the module rows). The structural cure
  is DEC-459's prelude-binding isolation; this arm becomes redundant when that lands.

  **(b) The disclosed consequence below did not actually occur.** On the un-imported spelling
  `parseQuery`/`cookiePairs` still work on all three legs, so the loss was over-stated at the same
  time the closure was over-stated. It holds only for a program that DOES import the raw module:

  **Stated consequence (for the import spelling only):** `Core.Native.Http`'s other members
  (`parseQuery`, `cookiePairs`, …) have real PHP twins and lose their transpile leg along with
  `registerServe`, exactly as the four siblings' non-placeholder members did. Nothing in the corpus imports the raw module — the only two
  hits are the injected preludes — and the friendly `Core.Http` surface, which transpiles, is the
  supported way to reach them.

  A module row is safe here **only because the gate runs pre-expansion**: `cmd_transpile` refuses the
  `lex_parse` output and `transpile_program` refuses before `check_and_expand`, so the preludes' own
  `import Core.Native.Http as NativeHttp` — and `Http.serve`'s body, which calls `registerServe` — are
  invisible to it. Applied after injection, the same row would reject every `import Core.Http;`
  program, i.e. the whole `examples/web/*` corpus the refusal exists to protect. A companion test
  pins that ordering from the outside, and a sabotage run confirms the pair goes red on the mutation
  and green on the restore.

- **The ladder gate moved out of `pipeline.rs` into `src/cli/ladder.rs`.** Adding the row above took
  `pipeline.rs` from 860 to 882 lines and the Invariant-13 size gate refused the push — "split it, do
  not grow it". The gate is a cohesive unit (one table, one walk, one job), so it became its own
  module rather than being trimmed elsewhere to make room: `pipeline.rs` dropped from 882 to 786 and
  `ladder.rs` is 110. (Both files have grown since with later commits in the same series; the point is
  the split, not a frozen line count — quoting one was a small self-inflicted staleness.) Its module doc records the property the rows depend on — that it runs
  pre-expansion — so the next person to add a row reads it before choosing a key. Pure move plus the
  doc; the three callers are repointed and no behavior changed. In passing, the doc comment that sat
  above the function actually described `transpile_program` in its first two lines and the ladder in
  the rest; each half now sits on what it documents.

- **`phg serve --help` pointed at a file that no longer exists.** Its example line named
  `examples/web/server.phg`, which S3.3d deleted when the servable examples became projects; it now
  names `examples/web/server/serve.phg`, which the suite actually serves. Found by the 2026-08-31
  milestone panel.

### Added

- **S3.5 — `phg serve` speaks HTTPS, and refuses rather than falls back (DEC-331 D7).** Inbound TLS,
  feature-gated `http-server-tls`. HTTPS enables **iff both `cert` and `key`** are set on the
  registered `ServeConfig` — no `--tls` flag, no switch; `tlsMinVersion` (`"1.2"` default, or
  `"1.3"`) is the floor. **This closes DEC-331 Slice 3** (D1/D4/D5/D6/D7 all built).

  **No new crate.** rustls 0.23 has no client/server feature split, so the outbound HTTP client's
  existing feature set already compiled `ServerConfig`/`ServerConnection`/`StreamOwned`; the second
  consumer needed neither a dependency nor a rustls feature. PEM decoding is hand-rolled in
  `src/serve/pem.rs` (~85 lines) rather than admitting a fifteenth crate to strip two marker lines.

  **The slice is mostly its refusals, deliberately.** Every failure mode here degrades identically if
  handled loosely: the port binds, requests are answered, and the traffic is in the clear with
  nothing in the response to say so. So each condition is a startup error, and each explanation says
  why the "helpful" alternative was rejected:

  * `E-SERVE-TLS-INCOMPLETE` — a lone `cert` or `key`. D7's surface text says HTTPS enables "iff BOTH
    are set", which read literally makes a half-configured server serve plaintext. That reading is
    rejected; `src/cli/serve_config_prelude.rs` had already flagged it in prose as "a security
    footgun of exactly the shape DEC-363 was written about".
  * `E-SERVE-TLS-MIN-VERSION` — `"1.1"` and friends are refused, never silently raised to 1.2. A
    floor is a security control; guessing at intent is how one ends up lower than its author thinks.
  * `E-SERVE-TLS-DISABLED` — cert+key on a build without the feature. **Enforced by the type system:**
    without `http-server-tls` the internal `TlsServer` is an *uninhabited* enum, so no refactor can
    produce one and `Conn::accept` discharges the branch with `match *never {}`.
  * `E-SERVE-TLS-CERT` — unreadable, malformed or mismatched pair, at STARTUP. A server with no usable
    identity binds its port perfectly well and then fails every handshake — a failure clients report
    rather than the server.

  Config errors outrank build errors (pinned): a lone `cert` on a feature-off build reports
  `-INCOMPLETE`, because the config is wrong however the binary was compiled.

  Two ordering facts, both load-bearing. **Stated at the wiring site and exercised END-TO-END rather
  than pinned by a unit test** — the handshake tests drive `rustls` over their own listener, so the
  `transport.rs` wiring is covered by a live `phg serve` + `curl` run on both accept paths, not by the
  suite: the handshake runs in the **worker**, never the accept loop, so a stalled client cannot serialize `accept()`; and the
  stream is wrapped only **after** blocking mode and the read/write timeouts are set on the raw
  socket — rustls fails outright on a non-blocking socket, and running the handshake through those
  same timeouts is what bounds a TLS-level slowloris.

  **Certified by execution end-to-end on BOTH accept paths** — a `--features http-server-tls` release
  binary running the README walkthrough verbatim: the pool path (8 workers) and the single-threaded
  `TcpTransport` path (`--workers 1`) each banner `listening on https://127.0.0.1:8443`, answer
  `curl --cacert … https://localhost:8443/hello` with `served over TLS: /hello`, and refuse a
  plaintext client. That run is the ONLY coverage of the `transport.rs` wiring: the handshake tests
  bind their own listener, so a 6C finding caught that every line changed there was otherwise
  exercised by nothing.

  TLS is read **directly from the config**, not through `serve::settings::resolve`: that function is
  the flag-vs-config precedence rule, and D7 rules TLS has no flag — one source, no precedence.

  Also: `src/serve/transport.rs` fell 635 → 455 lines and **left `scripts/size-baseline.txt`**, the
  wire framing having moved to `src/serve/framing.rs` (Invariant 13). v1 is terminating TLS only —
  HTTP→HTTPS redirect, HSTS, cert hot-reload and mTLS are ruled-and-deferred (KNOWN_ISSUES
  §SERVE-TLS), and cert paths resolve against the process cwd, not the site-mode app root.

- **S3.4 — the wrong verb now says so: `E-NO-ENTRY-FOR-ROLE` (DEC-331 D6, DEC-455.15).** `phg run` on
  a program whose only entry is `#[Entry(kind: EntryKind.Web)]` — and `phg serve` on a
  `kind: EntryKind.Cli` one — used to report a bare absence, in text identical to what a genuine
  library is told, though the two fixes differ completely: a library needs an entry *written*, this
  needs a different command *typed*. The new diagnostic names the missing role, the declared one, and
  the verb that works; on an interactive terminal it then offers to run that verb, **defaulting to
  NO**. A pipe or CI run gets the diagnostic, exit 1, and stdin is never read, so a script cannot hang
  on a question nobody is there to answer. The offer is withheld where it could not be taken — `-e`
  and stdin sources, and `phg serve <dir>` site mode, because `phg run` accepts neither a directory
  nor inline source. Programs with *no* entry keep `no entry point` / `E-SERVE-NO-HANDLER`, and a
  reserved kind keeps `E-ENTRY-KIND-RESERVED`: those are different diagnoses, not this one.
  `phg explain E-NO-ENTRY-FOR-ROLE`, both `--help` texts and `examples/web/README.md` updated in the
  same change. Accepting the offer runs the other verb exactly as if it had been typed: the run→serve
  direction goes through the same preamble a real `phg serve` uses, and the serve→run direction is
  decided BEFORE serve disables stdin — otherwise the switched program would have read an exhausted
  pipe instead of the terminal.

### Changed

- **`src/main.rs` shrank from 622 to 496 lines and left `scripts/size-baseline.txt`** (Invariant 13,
  the ratchet tightens): the 140-line `phg serve` argv branch — flag parsing, DEC-282 site-mode
  resolution, the process preamble — moved to a new `src/cli/serve_cli.rs`, where it sits beside the
  rest of the serve pipeline instead of inside a dispatcher. Behaviour is unchanged by the move
  (verified by diffing the extracted body against the previous `main.rs`).

- **S3.2 Part C — `Http.ServeConfig` now binds the socket; a flag that disagrees says so
  (DEC-455.14, developer-ruled).** Until this change the registered config was INERT:
  `serve_register::config()` carried `#[expect(dead_code)]` and had no caller, so
  `Http.serve(new ServeConfig(port: 3000), h)` still bound 8080.
  - **The rule: the CLI flag wins, but LOUDLY.** The config is the DEFAULT source for the four
    settings the serve loop binds — `host`+`port`, `workers`, `timeout` — so `phg serve serve.phg`
    with no flags binds what the program asked for. A flag that was PASSED and whose value DIFFERS
    wins, after one `W-SERVE-CONFIG-OVERRIDDEN` line per field on **stderr** (stdout belongs to the
    served program's `Output.*`, DEC-220). A flag that merely RESTATES the config prints nothing — a
    notice that fires when nothing changed trains the reader to ignore the one that matters.
  - **Ordering is load-bearing**: the config is readable only AFTER `web_*_factory`, whose startup
    validation run is what executes the `Web` entry and populates the global — still before any
    socket binds. Reading it earlier always sees `None`, i.e. the config silently never applies.
  - **Provenance is approximated by VALUE, and the limitation is recorded rather than hidden.** A
    constructed object carries no provenance, so a field counts as set iff it differs from D4's class
    default (`settings::class_defaults`, pinned against the prelude SOURCE by
    `class_defaults_match_the_prelude_source`). Consequence: `new ServeConfig(timeout: 0)` cannot
    express "no timeout" — `--timeout 0` can. KNOWN_ISSUES §SERVE-CONFIG-PROVENANCE; the real fix is
    a nullable D4 field set, which changes a ruled class shape and is its own Invariant 15 question.
  - **Why not read the config unconditionally:** D4 declares `timeout = 0` while `phg serve` defaults
    to 30s, so that would have SILENTLY disabled the B4 idle-socket guard for every existing server.
    The differs-from-default rule keeps `new ServeConfig()` byte-for-byte as it was.
  - **Scope is those four fields and no more**: `cert`/`key`/`tlsMinVersion` await D7 (inbound TLS is
    unbuilt — `rustls` is linked only by the outbound http-client), `maxBodySize` belongs to the wire
    parser, `serverName` has no consumer. Wiring a field whose reader does not exist would be a
    config that still does nothing.
  - **A negative config value reads as unset, fail-safe** (6C finding): `timeout: -3` differs from the
    class default so it reads as SET, and falling back to `0` there would mean *no timeout* — a typo
    silently disabling the B4 idle-socket guard. It falls back to the DEFAULT instead, with the
    already-fail-safe negative `workers` as the control. Real range validation stays OWED and is now
    written down rather than assumed.
  - Resolution is a PURE function (`src/serve/settings.rs`, `cores` injected) so it is unit-testable:
    11 tests, written RED first against a stub reproducing the ignore-the-config behaviour, and
    sabotage-verified twice (silence the notices → 1 red; invert the ruling so the config beats a
    passed flag → 2 red), both restores byte-identical. **The WIRING is pinned separately**
    (`src/cli/serve_pipeline_tests.rs`, also a 6C finding): those 11 tests all pass `cfg` explicitly,
    so they prove the rule and nothing about the chain. `prepare_serve` was split out of
    `serve_program` so the chain can be tested short of the blocking bind — hoisting the `config()`
    read above the factory build reds 2 of its 3 tests with the config silently inert. Verified
    end-to-end on a real socket both ways. `phg explain W-SERVE-CONFIG-OVERRIDDEN`, `phg serve --help` and `examples/web/README.md`
    updated in the same change (Invariant 17).
  - `src/cli/serve_pipeline.rs` split out of `pipeline.rs`: the wiring pushed that grandfathered file
    past its `scripts/size-baseline.txt` row, and a grandfathered file may only shrink (Invariant 13).

- **S3.3e — the `Http.ServeConfig` example, and LSP completion for EVERY stdlib class (DEC-455.3
  closed, DEC-455.13).** The two Invariant 9 + 17 rows S3.3 had been carrying since S3.2 Part A.
  - **`examples/web/serve_config.phg`** shows the config surface `Http.serve(cfg, handler)` receives:
    a promoted constructor whose every field is optional, so named arguments select what you set; the
    `workers = 0` AUTO and `timeout = 0` no-timeout sentinels (literal, so the class stays a
    deterministic value across all three legs); the D7 rule that HTTPS auto-enables **iff BOTH `cert`
    and `key` are set** — a lone `cert` still serves plain HTTP; and `RequestParsing.Eager`/`Lazy`.
    It deliberately does NOT call `Http.serve`: `phg transpile` refuses any file that does
    (`E-TRANSPILE-SERVE`), so this is the ONLY shape in which the config surface can face the PHP
    oracle. Byte-identical on `run`, `run --tree-walker`, `run --no-jit` and php-8.5.9.
  - **Counted, not assumed** (the DEC-191 lesson): flat corpus RUN 198 → 199, SKIP 19 → 19 — the
    example is gated, not quarantined. `scripts/surface-baseline.txt` re-emitted, examples 287 → 288.
  - **The LSP hole was wider than the row that surfaced it.** `catalog::class_members` only ever read
    the USER program, so a receiver whose declared type was ANY stdlib class — `Request`, `Response`,
    `Date`, `Instant`, `Uri`, `Session`, `ServeConfig` — completed to NOTHING. Two source comments
    called it "a documented follow-up"; neither had been measured. `src/lsp/prelude_catalog.rs` (a new
    file — the `catalog.rs` split Invariant 13 requires) answers instance members by parsing the
    `CORE_MODULES` registry's own prelude source on demand, the same mechanism `prelude_class_statics`
    already used for `Http.serve` — so a new prelude class is completable the moment it is written,
    with no LSP edit.
  - Pinned by tests: the LEAF names the class (`Http.ServeConfig cfg` ≡ bare `ServeConfig cfg`);
    `private`/`protected` and `static` members are filtered (`Request`'s `rawTarget`/`rawHeaderLines`/
    `rawBody` are private PROMOTED ctor params, i.e. real members a naive walk offers, and
    `req.parse(…)` is not a call anyone can write); and the user program is consulted FIRST, so a
    project's own `class Response` shadows the stdlib one rather than merging with it.
    Sabotage-verified twice; restores byte-identical.
  - **Editors: a verified no-op.** `editors/vscode/syntaxes/phorj.tmLanguage.json` carries no stdlib
    names (it is purely syntactic) and both editors consume the same LSP, so completion improved with
    no editor change. S3.3e introduced no new syntax.
  - **Left open and recorded, not skipped:** go-to-definition and hover on a stdlib symbol still return
    nothing — a prelude declaration has no file to open, and the three candidate answers each trade
    something real (one would land inside the user's own buffer, the §span-collision hazard).
    KNOWN_ISSUES §LSP-PRELUDE-DEFINITION.

- **The web example corpus moved to the D5 model, and the checker narrowed to match (S3.3d,
  DEC-455.12).** `(Request): Response` under `kind: EntryKind.Web` no longer type-checks:
  `E-ENTRY-SIG`, with a hint naming `Http.serve` and pointing at
  `phg explain E-SERVE-NO-HANDLER`'s before/after. S3.3c had retired that shape from the serve
  RUNTIME; the checker kept accepting it only because narrowing it before the examples moved would
  have reddened the byte-identity glob for a reason unrelated to what it gates. Both halves land
  here, together.
  - **The narrowing is in `entry_shape_matches`, NOT `entry_role`** — `desugar_config` skips config
    param-erasure whenever `entry_role(f).is_some()`, so narrowing there would erase the `Request`
    parameter and turn the diagnostic into an opaque arity complaint.
  - **Three examples became PROJECTS**: `examples/web/{core-http,json-api,server}/` each pair a
    PHP-gated `src/` (the logic, driven by a Cli entry) with a sibling `serve.phg` holding the `Web`
    entry. `phg transpile` refuses any file calling `Http.serve`, so the split is what lets the LOGIC
    keep its PHP leg. Neither differential glob collects a `serve.phg` — `collect_phg` skips any
    directory containing `src/` — so `tests/serve.rs` gained
    `every_example_serve_phg_registers_serves_and_is_transpile_quarantined`, which loads every
    shipped `serve.phg`, drives one real request through the serve loop, and asserts the transpile
    refusal. Those files were gated by NOTHING before it.
  - **`handler.phg` stayed FLAT** and simply dropped its `Web` attribute: it exists to show the wire
    format by hand, and `Http.serve` takes Core.Http's `Request`, not a hand-rolled one. The
    hand-built parse/serialize pedagogy therefore did not leave the corpus. `server/` adopted the
    stdlib types (deleting only its duplicate of what `handler.phg` teaches) because being servable
    IS its purpose. `examples/session/counter.phg` migrated in place and deliberately did NOT become
    a project: `all_example_projects_transpile_and_match_php` has no skip arm, so a project faces the
    PHP oracle unconditionally, and counter is impurity-quarantined from it.
  - **Corpus counts, measured before and after**: flat RUN 201 → 198, flat SKIP 19 → 19, projects
    15 → 18. Total gated programs unchanged at 216 — nothing left the oracle.
  - `server.php` moved into the project and was rewritten to rebuild the raw request and call the
    transpiled `respond(bytes): bytes`. Its documented `sed '$d'` recipe was WRONG for a project: the
    `\Main\main();` bootstrap is emitted inside the trailing global namespace block ahead of the
    runtime helpers, so it is not the last line and `$d` deleted a helper's closing brace. Now
    `sed '/^ *\\Main\\main();$/d'`, verified end-to-end under `php -S`.

### Fixed

- **The playground example sweep was missing `Core.FileSystemModule` (DEC-455.12).**
  `gen_examples.py`'s `SYSCALL_IMPORTS` listed `Core.File` — a different, still-live module — but not
  `Core.FileSystemModule`, so `examples/fs/*` leaked into the browser WASM build, where they would
  fault with an unknown-module error. Same shape as the `Core.Regex` omission fixed in 2026-07.
  Found only because `examples.js` had drifted out of date with the corpus and regenerating it made
  the leak visible; the regenerated file also picks up four guide examples that were simply missing.

- **Two checker tests were VACUOUS and are now real (DEC-455.12).**
  `cli_and_web_entries_may_coexist` and `well_formed_cli_and_web_kinds_are_clean` asserted
  `!has(E-ENTRY-SIG)` on programs that imported `Core.Http.Request` — but the raw checker used by
  those tests injects no preludes, so the programs failed with `E-UNKNOWN-TYPE` and never reached the
  shape gate. Their assertions held for ANY shape rule, including the one being retired. Both now
  assert the program type-checks CLEANLY first. (The asymmetry matters: a `has(...)` assertion still
  fires on such a program, because the shape gate reads type NAMES; only `!has(...)` goes hollow.)

- **`tests/serve.rs`'s module doc claimed its TCP smoke test was `#[ignore]`d.** `tcp_smoke` carries
  no such attribute and runs on every suite invocation.

- **`Http.serve(cfg, handler)` — the DEC-331 D5 web entry point (S3.3a).** A
  `#[Entry(kind: EntryKind.Web)]` function is now a closure FACTORY: it calls
  `Http.serve(cfg, handler)` with a typed `(Request) => Response`, and that handler is what runs per
  request. `Http.serve` **registers and returns** — it does not own an accept loop; `serve_program`
  drives the same loop it always has. Green on BOTH backends.
  - The `§3` inverted-loop design in the plan was DISPROVED and is superseded by `§3b`: a native
    cannot call `.serialize()` on the `Response` it gets back, and the `ClosureInvoker` does not
    outlive the native call, so a native cannot own a loop that invokes the handler. The wrapping
    therefore lives in phorj (`src/cli/http_serve_prelude.rs`), which also keeps the malformed-request
    policy (400) identical to the legacy `respond` bridge by construction.
  - Two registration slots, deliberately different kinds (`src/native/http/serve_register.rs`): the
    handler is `Rc`-bearing so it goes to a **thread-local**; the config is `Send` scalars and goes to
    a **process global**, because the parent thread needs `workers`/`host`/`port` before any worker
    exists. Nothing `Rc`-bearing crosses a thread boundary.
  - Per-request semantics are pinned on both legs by
    `captures_persist_across_requests_while_statics_reseed`: the handler's CAPTURES persist across
    requests, program STATICS re-seed. Serve is Invariant-14 quarantined, so the byte-identity
    differential cannot see this — that test is the only thing standing between the two backends and a
    silent divergence.
  - **Scope:** the registered config is stored and round-trip tested but not yet read — see S3.3c below.

- **`E-SERVE-NO-HANDLER` + `phg explain` arm — and `respond` is RETIRED (S3.3c).** The named
  `respond(bytes): bytes` serve entry, the `handle(Request): Response` entry that `import Core.Http`
  used to wrap in a synthesized `respond`, and both legacy by-name handler factories are DELETED.
  `phg serve` routes through the D5 web factories on both backends, so `Http.serve(cfg, handler)` is
  the only way a program registers a handler. `phg serve --help` and the `serve` summary line were
  rewritten in the same change.
  - **BREAKING for pre-DEC-331 serve programs.** A `respond` or `handle` entry no longer serves. The
    refusal names the migration and `phg explain E-SERVE-NO-HANDLER` carries a before/after snippet.
    Such programs still CHECK, RUN and TRANSPILE unchanged — only `phg serve` refuses them.
  - **The refusal had to be added, not just reworded.** With the by-name fallback gone, a legacy
    entry is still resolved (S3.3b kept `(Request): Response` legal for `kind: Web`) and was then
    called with no arguments — so the startup message was `` `handle` expects 1 argument(s), got 0 ``,
    an opaque arity complaint on the one diagnostic every migrating user reads exactly once. The web
    factory now refuses a parameterised web entry before calling it, keyed on ARITY.
  - The CHECKER still accepts the legacy shape for `kind: Web`; narrowing it rides with the example
    migration (S3.3d), so that this change does not take the example byte-identity glob red for a
    reason unrelated to what it gates.
  - `ServeConfig` is still stored-and-unread: making it win over `--address` requires the
    flag-vs-config conflict to hard-error rather than silently pick a winner, which is the pending
    S3.2 Part C precedence ruling. Setting `port` does not yet move the socket.

- **`E-TRANSPILE-SERVE` — the Invariant 14 tier-2 refusal, now BUILT** (it had been in the register
  and the specs for weeks with no site in `src/`). A program that CALLS `Http.serve` is refused by
  `phg transpile`: PHP is served BY a web server rather than being one, so no faithful idiomatic
  mapping exists and a silent downgrade is forbidden. Keyed on the CALL — **not** on the `Web` entry
  kind and **not** on the `Core.Http` import, both of which were checked against the corpus and would
  have refused the five shipped `examples/web/*`. The spec sentence claiming `Web` entries already hit
  it was false on both halves and is corrected in this change (DEC-455.7).

### Fixed

- **A multi-file PROJECT using an injected prelude emitted PHP that FATALLED (DEC-455.11).** The
  namespaced emit buckets injected prelude classes into `namespace Main {}` while their `__phorj_*`
  runtime helpers go into the trailing global `namespace {}` block, where they named those classes
  **unqualified** — so PHP resolved `new RequestBody(…)` against the global namespace and died with
  `Class "RequestBody" not found`. Every project touching `Core.Http`, `Core.Regex`, `Core.Decimal`
  or `Core.Session` was affected. The identical program as a FLAT single file was always correct,
  which is why no test caught it: every prelude example is a flat single file, and no example project
  imported a prelude that ships helpers. This BLOCKED DEC-331 S3.3d, whose ruled structure exists
  precisely so `src/main.phg` keeps its PHP leg.
  - Fixed **centrally**, not per family: `emit_program_namespaced` now emits `use \Main\<name>;` for
    every non-function Main-bucket name at the top of the global block — the same mechanism DEC-325
    already applies to each non-Main package block. The per-family alternative is what FAILED:
    `emit_json_helpers` had qualified ITS references with `\Main\` in an earlier pass, and that fix
    was never carried to the other four preludes.
  - **Functions are deliberately not aliased** — helper bodies call PHP builtins bare (`count`,
    `strlen`, `implode`) and a `use function \Main\count;` would hijack them. Class aliases are safe:
    the helpers spell every builtin CLASS fully qualified and the global block declares no classes.
  - Gated by a new example PROJECT, `examples/project/preludes/`, exercising three prelude families
    so the fix is proven generic rather than shaped to the one fatal first observed. Verified RED with
    that exact fatal before the fix, green after, and sabotage-verified by deleting the alias loop.


- **The surface ratchet was measuring wrong and under-protecting 169 diagnostic codes.**
  `scripts/surface-ratchet.sh` decided "is this code asserted?" with `grep --include`, which matches the
  **basename, not the path** — so its patterns (`tests/**.rs`, `*tests*.rs`, `tests.rs`) missed the
  commonest test shape in the repo: a module in a `tests/` **directory** (`src/checker/tests/mutation.rs`).
  **101 files were invisible**, and the gate reported 83/307 asserted (27%) when the truth was 252/307 (82%).
  The wrong percentage was the harmless half — the FLOOR sat at 83, so **169 codes' coverage was
  unprotected**: deleting the only test asserting `E-ASSIGN-TYPE` did not trip the gate. It now does
  (verified by sabotage: `codes_asserted = 249, floor is 250`). Same class as the DEC-191 no-op example glob.
  Two further measurement defects fixed in the same pass: the check was a SUBSTRING test, so
  `E-MISSING-RETURN` was credited as covered by a fixture rendering `E-MISSING-RETURN-TYPE` (now a
  whole-line match — and a real `conformance/diagnostics/missing-return.phg` fixture was added to earn the
  conformance count back to 25 rather than lowering the floor); and the emitted-code denominator was
  scanned over all of `src/`, so `E-MULTIPLE-MAIN` (no emit site) and `E-VARIADIC` (a test-only literal)
  counted as both emitted and asserted. Floor re-emitted at **250/305**; read against the old 83/307 as a
  measurement correction, not a coverage jump — nothing is tested that was not tested before. Real
  remaining debt is **55 codes, not 224**.

### Changed

- **A `Web` entry may now be `(): void`** (DEC-331 S3.3b). Under D5 the web handler is a closure passed to
  `Http.serve(cfg, handler)` *inside* the entry, so the entry itself is zero-arg — and with `#[Config]`
  parameters erased by the `desugar_config` pre-check before the checker runs, a config-carrying Web entry
  (`function web(Settings s): void`) arrives zero-arg and now checks clean. It previously failed
  `E-ENTRY-SIG`, which is the blocker the S3.2 notes below describe.
  **Scoped claim:** what is fixed is DEC-331 D4 §1's *entry signature/role gate*. §1 **verbatim** still does
  not check, because its body calls `Http.serve`, which does not exist until S3.3a.
  **Blast radius found and fixed in the same batch:** legalizing `(): void` for `kind: Web` broke the
  `Core.Http` respond-bridge, which resolves the web entry by its DECLARED kind and splices the name into
  `handle(req).serialize()` — so a `(): void` web entry that imported `Core.Http` got `web(req).serialize()`
  and two bogus errors (`expects 0 argument(s), found 1`, `type void has no method serialize`) reported
  against its import line. The bridge now filters on the STRUCTURAL shape (`entry_role`), which is the
  narrower question it always needed. So a config-carrying Web entry checks clean **including** with
  `import Core.Http;` — which was not true between the two commits.
  The gate is now `ast::entry_shape_matches(f, declared)` — *"is this shape legal FOR the declared role?"* —
  replacing `entry_role(f) == Some(role)`, which asked *"what role does this shape imply?"*. That was the
  right question only while DEC-191 inferred the role; S3.1 retired inference, and one shape can be legal for
  two roles. `(): void` is therefore legal for both `Cli` and `Web`, while the Cli-only shapes stay rejected
  for `Web`: `(): int` is a process exit code and `(List<string>)` is argv, and neither means anything to a
  server. The legacy `(Request): Response` web entry still checks, and retires with `respond` in S3.3c.
  `phg explain E-ENTRY-SIG` updated in the same change.
  **Not yet servable:** `phg serve` still resolves the `respond` entry, so a `(): void` Web entry checks but
  does not run until S3.3a lands `Http.serve`. No previously-working program changed behaviour — the shape
  being legalized did not compile at all before. Plan: `docs/archive/plans/2026-08-22-s3-3-http-serve.plan.md`.

### Added

- **`#[Config]` entry injection takes N typed parameters** (DEC-331 S3.2 Part B / DEC-455). An entry may
  declare several config parameters — `function main(AppConfig config, Limits limits, Map<string, string> labels)`
  — each resolved by its own TYPE, with the providers called in DECLARATION order and every unresolved type
  getting its own `E-CONFIG-MISSING`. Previously the limit was exactly one parameter. Generic config types
  work (`Map<string, string>` resolves on the type's bare head) and `examples/guide/config.phg` now ships one,
  so the shape is byte-identity-gated by the differential rather than merely asserted in a unit test.
  **S3.2 remains PARTIAL:** a `Web` entry still cannot carry config parameters (`entry_role` defines `Web` as
  exactly `(Request): Response`), so DEC-331 D4's §1 surface does not type-check yet — that gate lands with
  S3.3. Two user-visible edges are recorded as PENDING developer rulings rather than shipped silently:
  two `Map<…>` providers collide under one key (DEC-455.4), and a repeated parameter type invokes its provider
  once per parameter (DEC-455.5).

### Fixed

- **The PHP-oracle capability probe was non-deterministic — it rejected a VALID oracle ~13% of the time**
  (DEC-456, found by the certification panel). `php -m | grep -qx bcmath` under `set -o pipefail`: `grep -q`
  exits on the first match, php dies of SIGPIPE (255), and `pipefail` reports the whole pipeline as failed,
  so the probe concludes "no bcmath" about a php that has it. Measured on `php-8.5.9`: **20/150 and 8/200
  failures with `grep -qx`, 0/200 with a draining `grep -cx` count, 0/150 with no pipe at all.** Both probe
  sites now share a `_phorj_has_bcmath()` helper that drains. This is the defect the removed pin had been
  masking, and being a coin flip is why it never reproduced when chased. The pin scan now covers **every**
  tracked shell script and workflow (23 files, not 3), refuses to report a pass when it inspected zero files,
  and reports the file's real line number; `scripts/test-validate-infra.sh` gains six cases for it and is
  now RUN by pre-push — it had never been executed by any gate, which is how the check shipped scanning zero
  files in that harness's own fixture while printing a pass. The pre-commit "DOCS-ONLY" fast path is renamed
  NO-RUST and runs `validate-infra --quiet` when shell/YAML/JSON is staged: it had labelled a rewrite of the
  oracle resolver as docs.
- **The pre-push gate failed on a phantom PHP oracle after a stack patch bump.** `scripts/git-hooks/pre-push`
  kept a hardcoded `${PHORJ_PHP:-…/php-8.5.8/bin/php}` fallback — a second source of truth beside
  `scripts/toolchain.env`, which has globbed `php-8.5.*` and capability-checked `bcmath` since the
  2026-08-18 fix. When the stack moved to `php-8.5.9`, the hook handed the suite a path that does not
  exist, and a docs-only push failed with three opaque `php required (PHORJ_REQUIRE_PHP=1) but not found`
  asserts in `tests/attribute_transpile.rs` — a gate defect wearing the costume of a code defect (the
  same three tests pass 5/5 against the resolved 8.5.9 oracle). The fallback is gone: an unresolved
  oracle now exits 1 with an actionable message, and the hook echoes which php it gated against.
  `scripts/toolchain.env` additionally capability-checks an INHERITED `PHORJ_PHP` (refining DEC-331 D10d's
  "an explicit override always wins") — a stale export from a long-lived shell is warned about and
  resolved past, never trusted. `scripts/validate-infra.sh` grows a mechanical no-pinned-php-path check
  over `scripts/toolchain.env` + `scripts/git-hooks/*` (comment lines stripped, so the root-cause
  narratives that NAME the stale versions do not red-fail it), because this class has now bitten the
  gate three times and a comment has twice failed to prevent it.
- **A qualified constructor dropped its defaults and named arguments, and the VM panicked** (DEC-452).
  `new Http.Cookie("sid", "abc")` — omitting four defaulted parameters on a SHIPPED stdlib class —
  reported a bogus `expects 6 args, got 2` on the tree-walker and **panicked** the VM; the named form
  `new Http.Cookie(name: …, value: …)` panicked the VM and the transpiler. Any `new Module.Class(…)`
  was affected, so DEC-331 D4's ruled `new Http.ServeConfig(host: …, port: …)` surface could not work
  either. The two qualified-construction branches in the checker computed the DEC-297 named-arg
  normalization and the DEC-236 default-fill into a side table and never consumed it, so the rewrite
  was silently discarded and an `Expr::NamedArg` reached backends that assert it cannot exist.

### Added

- **`Http.ServeConfig` + `Http.RequestParsing`** (DEC-331 D4, S3.2 part A) — the web runtime's
  configuration contract as an immutable value with a promoted constructor: `host`, `port`, `workers`
  (0 = auto), `timeout` (0 = none), `cert?`, `key?`, `serverName?`, `maxBodySize` (8 MiB),
  `tlsMinVersion`, `requestParsing` (`Eager` default). Not yet consumed — `Http.serve` lands in S3.3.

### Added — `phg lift <dir>`: a PHP tree becomes a phorj PROJECT (DEC-439 part 1, 2026-08-05)
Developer-ruled. `phg lift <dir> -o <out>` lifts every file in ONE pass into a generated `phorj.json` +
`src/` layout mirroring the namespaces, so **cross-file references resolve** — the single fix for both
`E-MODULE-NOT-FOUND` on a lifted `import` and `E-UNKNOWN-ATTRIBUTE` on a framework attribute, which failed
for the same reason: one file cannot see its siblings. [Verified: a two-package fixture with a cross-file
`use` reports *"whole project type-checks clean: 3 files, 3 packages, 3 definitions validated"*.]

The **entry** is re-packaged as `package Main;` at `src/main.phg`. Not cosmetic: a dotted package must sit in
a matching subdirectory (`E-PKG-PATH`), so an entry left in its namespace package makes the whole project
fail to LOAD. A second script with top-level code is reported rather than silently demoted — a phorj project
has one entry per role.

Composer **vendor is REPORTED**, never synthesized: `VENDOR-REPORT.md` ranks every vendor symbol the app
references by reference count, attributed to the shipping package exactly via `installed.json`. The scan
survives a file the lifter *rejects* — a Tier-2 construct fails the whole PARSE, so the `use` block is read
off the token stream as a fallback, which is precisely where dependency information matters most.
`--vendor=stub` is accepted and **refused with its reason** (ruled, not yet built) rather than quietly
behaving like the default.

Three defects came out of a review round, each measured rather than reasoned:
- **silent data loss.** Two sources mapping to one package+stem overwrote each other while the summary said
  "lifted 2/2" — legacy PHP hits this constantly, since every namespace-less file lands in `package Main`.
  Now disambiguated by walking up the source path (`src/B/Helper.php` → `B_Helper.phg`) and reported; a phorj
  package directory may hold any number of files, so nothing is lost.
- **a symlink cycle never terminated.** A depth cap alone does not help (the cycle re-walks the subtree at
  every level, so bounded depth is still exponential — measured: killed at 30s, reporting 41 files for a
  1-file tree). Directory symlinks are skipped instead.
- **the report undercounted.** It listed "files I looked at" as "files that exist": on a Symfony-shaped tree,
  8 PHP files present and 4 examined. Files outside composer's autoload map are now named — found by
  CONTENT, since `bin/console` and Laravel's `artisan` have no extension for a filter to match.

`src/main.rs` paid for the new dispatch by collapsing twelve identical `eprintln!("{USAGE}") + exit(2)` pairs
into one `usage_exit()` and extracting `phg build`'s flag parsing to `cli::build_flags` — it is a
grandfathered size-gate breach Invariant 13 forbids growing.

### Fixed — a function keeps its IDENTITY across a call boundary; user higher-order code JITs (DEC-445, 2026-08-05)
Passing a lambda to a function used to take the caller's whole hot loop off the JIT. Both sides refused an
`Fn` argument: the analyzer's `Op::Call` signature loop and the emitter's `pop_call_args`. Analyze now
records `Kind::Fn(f)` in the call signature, so the fixpoint's `param_over` carries the callee's IDENTITY
into the callee's param slot; emit passes the runtime word through unchanged, because it is the never-read
filler `arm_call_value` already discards. Nothing is allocated, cloned or freed.

`bench/micro/userhof` goes **0.19× LOSS → 12.5× WIN** — the phorj leg 102× faster (1 195 454 357 →
11 762 269 ns) — with checksum `999978` identical on all four legs (JIT, VM, tree-walker, php). The size of
the win is DEC-441's leverage arithmetic running the right way: the loop was declining, so the fallback was
the VM at ~16× php's instruction count.

Polymorphic call sites fail **closed**, for free: `join_kind` has no `Fn` arm, so two different targets
reaching one param report "conflicting call argument kinds" and fall back to the VM; only a single target
survives. That is tested, because a miscompile would have silently called the *wrong* lambda.

Does NOT fix the fs rows (`fsforeachline` 0.30×, `fslines` 0.11×) — they reach their closure through the
NATIVE higher-order path, not `Op::Call`. The OWED list is still 7; what moved is user-written higher-order
code, which no bench measured until `userhof` shipped.

### Added — the files outside `autoload` get a ROLE, from their CONTENT (DEC-439 part 2, 2026-08-05)
Developer-ruled, closing part 1's open question. A Symfony app keeps `public/index.php`, `bin/console`,
`migrations/` and `config/*.php` outside `autoload.psr-4`; a Laravel app keeps `artisan` and `routes/web.php`
outside it. Part 1 named those files; it did nothing with them.

Discovery now reads composer's **full autoload surface** — `classmap` (a directory OR a single file), `files`,
and legacy `psr-0`, for both `autoload` and `autoload-dev`. Ignoring `classmap` was the single largest reason
app-owned code went unexamined: it is where a project declares its `migrations/` and its legacy non-PSR-4
code.

What is left over is **classified by content**, token-level at brace depth 0, into three roles:

| Shape | Role | Disposition |
|---|---|---|
| declares a class / interface / trait / enum / function | code | **LIFTED** — the app's own code however composer maps it |
| top-level `return` of DATA | configuration | reported; replacement = a `#[Config]` class (DEC-318) |
| anything else with no declarations | bootstrap | reported; replacement = `#[Entry(kind: …)]` (+ `#[Route]`) |

Never by path, and that is the ruling rather than an implementation note: a rule matching `public/index.php`,
`artisan` or `migrations/` by NAME is a list of the frameworks the lifter happens to know, and wrong for the
next one. Doctrine's `migrations/Version*.php` now **lifts** because it declares a class — and the lifter
mentions Doctrine nowhere. [Verified on a Symfony-shaped fixture: `migrations/Version20260805.php` →
`lifted/src/DoctrineMigrations/Version20260805.phg`, and the lifted project `phg check`s clean.]

Three defects the fixture found that the design round did not:
- **a returned CLOSURE is a factory, not configuration.** Symfony's `public/index.php`
  (`return function (array $context) {…}`) and a `config/*.php` file (`return [ … ]`) are BOTH a top-level
  `return`; a rule that stopped there told the developer to re-express their front controller as typed
  configuration — wrong advice, confidently given.
- **composer's `bin` key is not part of the code surface.** `autoload` says "this is my code"; `bin` says
  "this is a command". Including it bypassed classification and fed the console script to the lifter:
  `lift parse error: require is Tier-2/Tier-3` — a refusal where the right answer was "this is a bootstrap
  script, here is the entry that replaces it". `bin` is still read, so a declared executable is classified
  even when the content sniff cannot see it.
- **"no `.php` files found" was a lie for a glue-only tree.** A tree whose PHP is *entirely* bootstrap and
  configuration is a real PHP app with nothing to LIFT — a different answer from "there is no PHP here", and
  reporting them identically sends the developer looking for a file that is not missing.

PHP-ness is decided by extension **OR** content, and the OR is load-bearing both ways: `bin/console` and
`artisan` have no extension for a filter to match, while a short-tag file has no `<?php` for a content check
to find. `examples/lift/README.md` gained the directory-lift walkthrough it had been missing since part 1.

### Added — `autoload-dev` code is REPORTED, not lifted (DEC-439 part 3, 2026-08-05)
Developer-ruled, closing part 2's pending question. A Symfony app declares
`"autoload-dev": { "psr-4": { "App\\Tests\\": "tests/" } }`, so part 2 lifted `tests/PostTest.php` into a
draft whose `extends \PHPUnit\Framework\TestCase` and `assertSame` reference a framework that will never be
ported — and whose symbols then filled `VENDOR-REPORT.md` as unresolvable. phorj has `phg test`; naming that
is more useful than emitting the draft.

A fourth role, `test`, and the **one role not decided by content** — it cannot be, because a PHPUnit class
declares a class like any other, so content alone calls it application code. It comes from composer's own
`autoload-dev` declaration, checked before classification. That is still machine-readable metadata rather than
a guess at a directory named `tests/`, so the no-hardcoded-framework-paths rule is intact. The honest limit,
stated in the code: test code in a project that declares no `autoload-dev` is indistinguishable from
application code and is lifted.

**Two lists, because there are two different questions.** `autoload-dev` prefixes leave the WALK but stay in
namespace RECOGNITION: test code is the app's own even though it is not lifted, so a reference into the test
namespace is a sibling reference, not a composer dependency. The regression guard for that passed *before* the
change and would have failed had the prefixes simply been removed.

Invariant 13 paid in the same change: `discover.rs` had reached 399 lines, so the WALK mechanics moved to
`lift/project/walk.rs` along the cohesion line that matters — that module answers "what did composer DECLARE",
the new one "what does the filesystem actually HOLD", and the two have different failure modes (a wrong answer
there mis-scopes the lift; a wrong answer here fails to terminate). Verified a pure move: the only diff
against the original text is three `fn` → `pub(super) fn`. `discover.rs` is back to 295, under the soft cap.

### Added — attribute arguments are constant-FOLDED (DEC-438, 2026-08-05)
Developer-ruled, narrow by construction. `#[Tag(1 + 2 * 3, -5, 1.5 + 2.0, "a" + "b")]` now emits
`#[Tag(7, -5, 3.5, 'ab')]` instead of being refused as non-constant.

An attribute argument is compile-time metadata that is never evaluated at run time, so replacing it with its
value cannot change what a program does — which is why the fold lives in the transpile gate and not in a
checker pass. A GENERAL folder would have to answer a language question this slice deliberately avoids
(does `int x = 2147483647 + 1;` become a compile error when the fold faults?).

Two disciplines make it safe rather than clever. The arithmetic is the **single-sourced kernel**
(`crate::value::int_add`/`int_sub`/`int_mul`/`int_neg` — Invariant 4, "never re-inline them in a backend"),
and those return `Result`, so an **overflowing argument declines to fold** and falls back to the disclosure
comment — never wrapped, never promoted to a new compile error. And only exact, non-faulting operators are
folded: `+ - *` on int/int and float/float, `+` on string/string (phorj's concat), unary `-`. Division and
modulo are excluded (they fault on zero); a non-finite float result is not folded.

The biggest win was the least expected: `#[Tag(-5)]` parses as `Unary { Neg, Int(5) }`, so before the fold a
plain **negative number** — the commonest computed argument in real code — was refused. A test now asserts the
fold agrees with what the *interpreter* computes for the same expression, rather than trusting the shared
kernel by inspection. A function call argument stays disclosed, correctly: its value is not known until run
time, and that is the case PHP fatals on.

### Queued — project-aware lifting ruled but NOT built (DEC-439, 2026-08-05)
Recorded in the repo before any build (Invariant 19). `phg lift <dir>` will lift a whole tree in ONE pass into
a generated `phorj.json` + `src/` project so cross-file references resolve — the single fix for both
`E-MODULE-NOT-FOUND` on lifted imports and `E-UNKNOWN-ATTRIBUTE` on framework attributes. Composer vendor is
**detected** from `autoload.psr-4` + `installed.json` and **reported** by default (a `VENDOR-REPORT.md`
worklist); foreign `declare` stubs are opt-in behind `--vendor=stub`, because a program with foreign
declarations cannot run on either phorj engine (`E-FOREIGN-RUNTIME`) — it becomes transpile-only, which is a
deliberate trade, not a default. See DEC-439 for the full ruling.

### Added — phorj attributes are re-emitted into the transpiled PHP (DEC-437, 2026-08-05)
Developer-ruled. Attributes used to be erased entirely on the PHP leg, which was *correct* and useless:
`phorj → PHP → phorj` lost them, and PHP-side reflection could not see a transpiled program's metadata at
all. Now a USER attribute (a use of a class declared `#[Attribute]`) and the `#[Attribute]` marker itself
reach the output. The marker is not decoration — without it PHP refuses `newInstance()` with *"Attempting
to use non-attribute class"* — and with it, reflection genuinely works: a transpiled program's `Audited`
attribute constructs and reads back as `Audited=billing/2` under php-8.5.8.

Every exclusion is measured, because an emitted attribute can break byte-identity in two different ways:

- **`#[Deprecated]` is never mapped onto PHP's own `#[\Deprecated]`.** PHP 8.4's has RUNTIME behaviour —
  calling the function prints `Deprecated: Function greet() is deprecated, …` — while phorj's is
  compile-time only (DEC-417: use-site warnings come from the reference pass, at check time). Mapping them
  would make the PHP leg print a line neither phorj engine prints.
- **An attribute with a non-constant argument is not emitted.** PHP parses attribute arguments as CONSTANT
  expressions, so a function call is *"Fatal error: Constant expression contains invalid operations"* — the
  whole file dies before any output. That is reachable, not theoretical: `#[Tag(1 + 2)]` type-checks clean
  and `1 + 2` lowers to `__phorj_checked_add(1, 2)`. The omission is DISCLOSED in the PHP output
  (`// phorj: \`#[Tag(…)]\` not re-emitted — an argument has no PHP constant form`), never silent.
- Every other built-in (`#[Entry]`, `#[Route]`, `#[Config]`, `#[Injectable]`, `#[Provides]`,
  `#[Transient]`, `#[Invoke]`, `#[ToString]`, `#[UncheckedOverflow]`) is phorj compile-time machinery
  consumed by a desugar, so erasing it is correct rather than lossy. The filter is defined against the
  single `BUILTIN_ATTRIBUTE_PATHS` enumeration, so a new built-in is excluded automatically.

The gate admits literals, an enum member (`Colour.Red` → `new Colour_Red()`, admissible because PHP 8.1
allows `new` in an attribute argument — it is evaluated on reflection, not at parse time), literal
lists/maps, and named arguments (PHP 8.0 spells them identically, so nothing is reordered) — all-or-nothing
per attribute. Name resolution reuses DEC-435's canonical-path rule, so the transpiler cannot bind a name
the checker validated against a different class.

Follow-up named rather than half-built: a **constant folder** (`#[Tag(1 + 2)]` → `#[Tag(3)]`) would remove
the gate's conservatism — phorj has none today. And a pre-existing CHECKER gap surfaced: an enum member as
an attribute ARGUMENT is rejected by `phg check` (*"unknown identifier `Colour`"*), so that emission path is
currently unreachable; its rendering is pinned by a raw-emit test so the emitter is right when the gap
closes.

An **enum-valued or class-valued** attribute argument re-emits as a construction
(`#[Painted(new Colour_Red())]`), with the construction's own arguments gated recursively. The first build
matched `Colour.Red` — a bare member access, which Invariant 12 makes invalid phorj everywhere
(`E-NEW-REQUIRED`) — so that arm could never fire and every enum-valued attribute silently fell through to
"no PHP constant form". Verified end to end: PHP reflection constructs the attribute and its enum field
(`Painted c=Colour_Red`). Two further findings came out of the same investigation: a claimed CHECKER gap was
**retracted** (it was my invalid test input, not the checker — see `KNOWN_ISSUES`), and `Expr::New` turns out
to REACH the transpiler inside an attribute argument, contradicting Invariant 5 and its own doc comment,
because neither `unwrap_new` nor `qualify_variants` walks `attrs`.

A cross-package attribute is referenced by ABSOLUTE FQN (`#[\\Meta\\Audited(…)]`), reusing the same
`php_type_ref` helper `extends`/`implements` already use. The first build used the bare leaf, which inside
`namespace Main { … }` resolves to `Main\\Audited` — a class that does not exist — so the metadata would
have named nothing while looking right; only the namespaced emit path reaches it, so no single-file test
could have caught it.

Invariant 13 debt burned down rather than deferred: `transpile/classes.rs` was a grandfathered 543-line
breach the gate forbids growing, so the enum emitter moved to `transpile/enums.rs` — taking it to 448 and
letting its baseline row be **dropped**, tightening the ratchet — and pass-1 name collection moved to
`transpile/collect.rs` when `program_emit.rs` crossed 500.

### Fixed — the PHP lifter was blind to EVERY `#[…]` attribute (LIFT-ATTR / DEC-436, 2026-08-05)
A bare `#` is a line comment in PHP, and the lift lexer treated `#[Audited("billing")]` as exactly that —
so **every attribute in every lifted file was silently swallowed**. For a tool whose contract is *refuse
loudly, never guess*, that is the worst possible failure shape: the file lifted clean and quietly meant
less than the PHP did. `#[` is now its own token; a bare `#` is still a comment, and both are pinned.

**The design decision is the NAME, not the syntax.** An attribute name is a CLASS name, so it is resolved
the way PHP resolves one — the `use` map first, then the current namespace, with a leading `\` meaning the
root — and only then spelled for phorj:

- root `Attribute` / `Deprecated` → the canonical `Core.Runtime.Attribute` / `Core.Runtime.Deprecated`
  (same concept, same name; the dotted form is self-gating, so no import is synthesized);
- a class in **this file's** package (or the root) → the **bare leaf**. A single-file compile keys classes
  bare, so `#[App.Meta.Tag]` would match nothing and land on `E-ATTR-TARGET`; the bare leaf matches both
  keyings, which makes it the correct spelling rather than the lazy one;
- a class from anywhere else → the **full dotted path**. phorj matches a built-in attribute as a
  segment-boundary SUFFIX, so a Symfony `#[Route("/home")]` lifted bare would bind to phorj's own
  `Core.Http.Route` — a different class taking different arguments, checking clean and meaning something
  else. A written name longer than a canonical path can never match one, so the qualified form is
  capture-proof. This is DEC-435's bug class one layer up: that fixed the checker, this fixes the direction
  that creates the names.

`#[A, B]` groups flatten to one `#[…]` per line, and PHP 8.0 **named arguments** lift 1:1 (phorj spells
them identically — DEC-297). **Arguments are never rewritten, dropped or reordered:**
`#[Attribute(Attribute::TARGET_CLASS)]` lifts to a marker the CHECKER rejects (`E-ATTRIBUTE-ARGS` — phorj's
target restriction is not implemented yet) rather than the lifter quietly discarding the restriction, which
keeps that judgement in one place instead of duplicating it into the lifter.

Refused loudly with the position named: an attribute on a method, property, class constant, parameter, enum
or enum case (phorj allows `#[…]` on a top-level `function`/`class` only, and `#[ORM\Column]` on a property
*is* the meaning of that line); an unqualified name equal to one of phorj's eleven built-in attribute names;
and a non-ASCII class name (legal PHP, but phorj's lexer rejects it and a lex error suppresses every other
diagnostic in the file). An attribute naming a class the file does not declare — every framework attribute —
is emitted with its identity intact plus a `// CANNOT LIFT:` note saying why `phg check` will flag it.

Two collateral bugs the slice forced out: the printer emitted **function** attributes only, so class
attributes had been invisible since DEC-194 (`Printer::attrs` is now shared; the statement printers moved to
`printer/stmts.rs` because `printer/items.rs` sat at the 500-line hard cap exactly); and LIFT-NS's
unused-import probe counted a name appearing after a `.`, keeping a dead `import Attribute;` that
`phg check` accepts.

Also satisfies Invariant 17's "lift updated in the same change" for DEC-417's `#[Deprecated]`, which could
not be honoured while the lexer was unable to see `#[`. Example: `examples/lift/attributes.{php,phg}` —
byte-identical on `run` / `run --tree-walker` / php-8.5.8 and against the original PHP. Still open and
named rather than implied fixed: **project-aware lifting** (a framework attribute's class is not in the
file) and **phorj's own attribute targets** (a Doctrine entity's mappings are property-level).

### Fixed — user attributes resolve by canonical path, so a qualifier finally means something (DEC-435, 2026-08-04)
`#[ORM.Column]`, `#[Assert.Column]` and even `#[Totally.Made.Up.Column]` all bound to one `class Column`
and all type-checked clean: resolution took only the leaf and threw the qualifier away. Doctrine's
`Column` and a validator's `Column` would have silently collapsed.

BUILT-IN attributes never had this bug — `attr_path_matches` matches a written name as a
segment-boundary suffix of a fixed canonical path, which is why `#[Bogus.Entry]` was always rejected while
`#[Entry]` / `#[Runtime.Entry]` / `#[Core.Runtime.Entry]` all resolve. User attributes were the lone
outlier, so the fix deletes a special case instead of adding one, and needs no new state: class-registry
keys are already package-mangled, so `\` → `.` gives the canonical path for free.

`#[Column]`, `#[Entity.Column]` and `#[App.Entity.Column]` resolve for a `Column` in `package App.Entity`;
`#[ORM.Column]` does not — unless a package `ORM` really declares one, in which case it resolves to THAT
one and the two stay distinct (verified on a two-package project: checks clean and runs). A bare leaf that
could mean two visible packages' attributes is `E-AMBIGUOUS-ATTRIBUTE`, which is a deliberate tripwire:
it is currently unreachable because import hygiene reports `E-IMPORT-CONFLICT` / `E-IMPORT-SHADOW` first,
and is kept so resolution fails loudly rather than silently if those rules are relaxed.

### Added — named arguments in attributes (DEC-435, 2026-08-04)
`#[Route(path: "/users", method: "GET")]` was `E-NAMED-ARG-MISPLACED` even though named arguments already
worked on ordinary calls and on built-in attributes (`#[Entry(kind: …)]`). They are now normalized to
positional against the attribute class's constructor using the same helper ordinary construction uses, so
the two cannot drift, and arity plus per-argument type checks still run on the normalized list. Out-of-order
names, wrong types and misspelled names are all still caught.

### Added — the DEC-397 function-scope hoist, narrowed to what is provably sound (2026-08-04)
PHP has FUNCTION scope, phorj has BLOCK scope, so a variable first assigned inside a block was DECLARED
inside it and every later use was `E-ASSIGN-UNKNOWN` / `E-UNKNOWN-IDENT`. The declaration now moves to the
top of the function — but **only out of blocks that always execute** (the function body, a bare `{ … }`,
`if (true)` with no other arm), which is exactly the shape of DEC-397's own reproducer.

**The agreed shape was "hoist any literal first assignment", and that is unsound.** For
`if ($c) { $b = 5; } return $b + 0;`, `$c = false` prints `0` in PHP — an unassigned read is null and
`null + 0` is `0` — while a hoisted `mutable var b = 5;` prints `5`. The draft would COMPILE and be WRONG,
trading a loud error for a silent divergence: strictly worse than the bug being fixed. Faithful
reproduction needs `T? b = null` plus unwraps, and the lifter cannot infer `T` from untyped PHP locals, so
every conditional case is refused with a `// CANNOT LIFT:` note naming the variable and function instead.

Also never hoisted, each for its own reason: a parameter (already declared — a second declaration is the
`E-SHADOW-LOCAL` that DEC-397 explicitly forbids), a `foreach`/`catch` binding, a non-literal right-hand
side (moving a call out of its branch relocates a side effect), a variable read before its first
assignment, and a block-local variable. Ships `examples/lift/hoist.{php,phg}` and a `lift_roundtrip` case
— the harness that compares lifted stdout against the original PHP's on all three legs, i.e. the only one
that catches "compiles but changes the answer".

### Added — `declare(strict_types=1);` in every transpiled file, both directions (DEC-401, 2026-08-04)
The transpiler now emits `declare(strict_types=1);` as the first statement of every generated PHP file
(single-sourced as `PHP_PROLOGUE`, so the flat and namespaced emit paths cannot drift). The PHP leg now
enforces at its boundary what phorj enforces everywhere else: a host PHP caller passing `"5"` to an
emitted `function helper(int $x)` gets a `TypeError` instead of a silent coercion phorj's own checker
would never have admitted. Symmetrically (Invariant 17) the LIFTER reads `declare(strict_types=1);` and
discards it — lossless, since phorj is always strictly typed — while `strict_types=0`, `ticks` and
`encoding` are refused with a reason, because those do carry meaning phorj cannot express. This also
clears the second of the two mandatory PSR-12 prologue blockers, so a `declare` + `namespace` + `use`
file head now lifts.

### Fixed — a latent byte-identity bug that PHP's coercion had been hiding (DEC-401 fallout)
DEC-401's premise was that no existing example could change behaviour "because the checker already
guarantees the types". The differential refuted that within one run: the checker guarantees types in
*phorj* code, not in the hand-written PHP runtime helpers the emitter ships. `-tie` on a `decimal` emitted
`-("2.345")` — a decimal erases to a PHP *string*, so unary minus was PHP arithmetic and coerced it to a
float, which then reached `strpos()` inside `__phorj_dec_scale`. Coercive mode had silently stringified
that float using PHP's own formatting, a conversion the interpreter and VM never performed — so the PHP
leg was one precision difference away from disagreeing with them. Now routed through the existing exact
helper (`__phorj_dec_sub("0", $x)`: `max(scales)` plus the same i128 bounds check), verified against the
tree-walker oracle including `-0.00d` staying `0.00`. Int and decimal negation now share one dispatch
point so a future numeric kind cannot be forgotten. **`declare(strict_types=1)` turns out to be a
byte-identity smoke detector for the emitted runtime, not just host-boundary hygiene.**

### Added — the lifter accepts `namespace` and `use` (LIFT-NS, CD-30, 2026-08-04)
`namespace` and `use` sat in the lifter's `UNSUPPORTED_KW` and were HARD PARSE ERRORS, so **no**
namespaced PHP file could be lifted at all — i.e. no Symfony, Laravel or Doctrine file, regardless of
anything else. This was found while planning LIFT-ATTR and it reorders that work: **attribute lifting was
the second blocker, not the first.**

- `namespace a\b;` → `package A.B;`. Segments are PascalCase-ized because `E-PKG-CASE` is *enforced*
  (`package app.entity;` is rejected: *"package segment `app` must be PascalCase"*) and PHP does not
  guarantee PascalCase namespaces; `snake_case`/`kebab` become word boundaries (`cli_tools` → `CliTools`)
  and an already-upper segment is preserved (`ORM` stays `ORM`, never `Orm`). No namespace still yields
  `package Main;`, so every previously-liftable file keeps its package line.
- `use A\B\C [as D];` → `import A.B.C [as D];`. Phorj supports import aliases natively, so the alias the
  author wrote survives rather than being inlined at every use site. A leading `\` root marker is not part
  of the path. Only namespace segments are reshaped — the last segment is the class's own name.
- **An unreferenced `use` is dropped.** `E-UNUSED-IMPORT` is a hard error in phorj while an unused `use` is
  legal and very common in PHP, so emitting every `use` verbatim produced "a lift that fails the very check
  it should pass" — the rule `lifter/exceptions.rs` already followed for error imports. Dropping is
  semantically lossless: a `use` only creates a local alias. Usage is judged against the LIFTED text, not
  the PHP source, because a Doctrine-style `use … as ORM;` is referenced only from `#[ORM\Column]` and
  attributes are not lifted yet.
- Refused loudly WITH THE REASON rather than half-lifted: a braced `namespace A { … }` (phorj has one
  `package` per file), a second `namespace`, a `namespace` after a declaration, `use function` / `use const`
  (they import a symbol, not a type), and the grouped `use A\{B, C};` form.

**Honest scope:** this removes the FIRST of two mandatory PSR-12 prologue blockers. `declare(strict_types=1);` is still outside the Tier-1 subset, so a file carrying it (most modern framework code) still stops at the parser; and a lifted `import` cannot resolve in a flat file (`E-MODULE-NOT-FOUND`), so the `use` half needs project-aware lifting before it pays off.

Ships `examples/lift/namespaces.{php,phg}` (Invariant 9), byte-identical on interpreter, VM and
php-8.5.8, plus a `lift_roundtrip` case asserting a namespaced file's stdout matches the original PHP on
all three legs. Invariant 13: the new file-level parsing was split to `parser/file_decls.rs` rather than
pushing `parser/items.rs` past the 500-line hard cap.

### Added — attribute-name completion, and the built-in attribute set single-sourced (CD-29, 2026-08-04)
Typing `#[` offered **nothing**, uniformly, for every attribute in the language — `Entry`, `Config`,
`Route`, `Deprecated`, `Invoke`, `ToString` and the DI set were all undiscoverable from the editor, which
Invariant 17's 100% rule counts as an incomplete feature. It now offers the full built-in set plus the
buffer's own `#[Attribute]`-marked classes, in both the bare (`#[Entry`) and canonical-path
(`#[Core.Runtime.Entry`, with a replacing `textEdit` so it cannot double-insert) spellings. `[` joined `.`
as an advertised `triggerCharacter`; **both** editor integrations were updated in the same change.

The enabling refactor: the 11 built-in attributes now live as `paths::*` consts in
`ast/decls/attributes.rs`, every `is_*` recognizer is defined against its const, and
`BUILTIN_ATTRIBUTE_PATHS` lists the same consts — so completion reads the array the checker recognizes
by, and a new built-in attribute becomes completable with no LSP edit. Previously the names existed only
as literals inside the recognizers, so any LSP list would have been a second source of truth by
construction. Tests pin the checkable direction (every enumerated row is recognized, and so is its bare
leaf); the converse is documented as *not* mechanically checkable rather than implied to be proven.

Also corrected two stale editor-README claims found in the same lists, both verified against the code:
find-usages **is** project-wide (DEC-327), and `rename` **is** still single-document.

### Measured — hooking the closure path would achieve nothing today (DEC-434.2, 2026-08-01)
DEC-434's leading option carried one unmeasured assumption; measured it before anyone builds on it.
Compiled as JIT entries, a **capturing** lambda declines with `"capturing entry (deferred)"` and a
**non-capturing** one with `"entry return kind Unknown"` — so a hook on `Op::CallValue` /
`call_closure_value` would find nothing to compile. Option 1 is not the small change it read as; it needs
capturing-entry support and param-kind seeding first. Second time in one day a leading JIT candidate looked
cheap and wasn't (see DEC-431.2), so: never cost a JIT design from the outside — compile it and read the
error.

**The insight:** a closure only has known operand kinds in the context of its CALL SITE. A vertical inlines
the lambda into the caller's graph where the element type is known; a standalone entry throws that away.
So the per-native vertical strategy is **design-forced, not a stopgap** — that is the real explanation for
the scoreboard's HOF split (`listfilter` 8.0×/`listmap` 7.2× with verticals vs `forEachLine` 3.4× behind
without one), and it supersedes DEC-434's framing. Revised options in DEC-434.2; the principled fix is
kind-specialized closure entries keyed on `(closure_fn_idx, arg_kinds)`.

### Found — a CLOSURE is never JIT-compiled, however hot (DEC-434, 2026-08-01)
Took the two deepest rows, `fsforeachline` (0.293) and `fslines` (0.113). DEC-431 had shown their profiles
were 74x dominated by their own fixtures, so this measured the read alone (fixture written once by shell).

**2,806 Ir per line, and only 4.9% of it is the line scan.** 48.68% (1,366 Ir/line) is VM closure
machinery: `exec_op` 19.1%, `run_until` 10.6%, `call_closure_value` 5.4%, `Value` stack traffic 10.7%,
`do_return` 2.9%. Allocator 12.6%. **Half the cost of reading a line is calling the one-expression closure
that consumes it.**

**Root cause [Verified]:** the JIT hot hook exists at exactly ONE call site — `src/vm/exec.rs:504`, inside
the `Op::Call` arm. It is absent from `Op::CallValue` (`:972`) and from `Vm::call_closure_value` (the path
every higher-order native uses). So `List.map`/`filter`/`reduce`, `forEachLine`, and any `f()` on a
function value run their body interpreted **forever**. `PHORJ_JIT_EXPLAIN` prints nothing — not a decline,
no attempt.

This reframes the scoreboard's HOF split: `listfilter` 8.0x, `listmap` 7.2x, `mapfilter` 5.2x are fast
because they have bespoke JIT **verticals** that bypass the closure; `fsforeachline` has none and loses
3.4x. The verticals have been treating this one native at a time. Options — hook the closure paths (lifts
every HOF at once, but the JIT entry must take the captures), keep building verticals, or cut the per-call
frame cost — are a PENDING RULING in DEC-434. Nothing built: after DEC-431.2, the bar for touching the
JIT's calling convention on inference is higher than one session's remaining budget.

### Fixed — the canon registry allocated a key per map write; `mapinsert`/`mapget` are WINs (DEC-433, 2026-08-01)
First two rows off DEC-432's hunt list, and the two nobody had looked at. Both JIT cleanly
(`PHORJ_JIT_EXPLAIN` prints nothing), so this is real cost in the map path, not the DEC-431 cliff.

**Root cause [Verified by callgrind].** `UbCtx::interned` — the CANON registry, a
`HashMap<Vec<u8>, u32>` — was probed by building an OWNED copy of the key first at three sites:
`rt_u_map_builder_set` did `.to_vec()`, i.e. **one heap allocation per `m[k] = v`**, and the flat-list and
map seals used `entry(bytes.clone())`, cloning on every seal even when already registered. Since
`Vec<u8>: Borrow<[u8]>`, all three can probe by slice for free; the allocation was pure waste on every
touch after a key's first. malloc/free was ~3% of the `mapinsert` profile.

**Fixed:** probe borrowed, allocate only to insert. The two seals were the same logic written twice and
now share `canon_for`; the builder's probe+register became `canon_key_slot`. Both live in a new
`src/jit/handles/canon.rs` (63 lines). That structure was forced by the size gate and was right: the
inline version pushed grandfathered `handles/mod.rs` 2000 → 2020, and extracting left it at **1973
(−27 below baseline)** with `maps_ext.rs` at 478 (−18). The baseline row is ratcheted to 1973.

**Measured.** Ir slope (the DEC-430 instrument): `mapinsert` **90.486 → 87.219 Ir/iteration, −3.6%**,
unchanged by the refactor. Wall clock, interleaved + pinned, 9 rounds against a pre-fix binary: phorj's
leg **6.24 → 5.91 ms on minima (−5.3%)**, 6.49 → 6.03 on medians (−7.1%). Wall clock moving MORE than Ir
is the expected signature of removing an allocation — allocator cache and lock cycles that Ir
under-counts. Harness on a quiet box: `mapinsert` **1.06× (WIN)**, `mapget` **1.01× (WIN)**, with no map
bench moving backwards.

**Honesty on the flip:** the baseline had `mapinsert` at 0.813, but the *pre-fix* binary measured ~1.00 in
the same interleaved run — that row's instability is what blocked a push in DEC-431.1. So the claim is
"−5..7% on the phorj leg, measured interleaved", not "0.813 → 1.06 because of this change"; −3.6% Ir
cannot do that. Identity re-verified on all three legs.

**Not done, and it is a security trade:** `interned` still uses Rust's default SipHash (another ~2.2% of
the profile). The codebase's `FnvHasher` doc argues SipHash "buys nothing" for field-map keys because they
"come only from a program's own source" — that argument does NOT transfer, because `interned` holds
runtime map keys and `m[request.query("x")] = 1` reaches it. Surfaced for a ruling rather than self-decided.

### Added — `PHORJ_JIT_EXPLAIN=1`; the ~320x cliff's mechanism CORRECTED and my recommended fix REFUTED (DEC-431.2, 2026-08-01)
Went to build DEC-431's `throws` cliff fix. Investigated first, and the investigation killed both the
recorded mechanism and the recommended design — so nothing was built from the wrong plan.

**`PHORJ_JIT_EXPLAIN=1 phg run <file>`** now prints every hot function the JIT declined and its exact
reason; silent by default (verified both ways). This is the fix for the actual root problem: **there was no
way to ask why a function was interpreted** — the error was discarded by `.ok()` at the compile site in
`vm::exec`, and that one thrown-away value is why DEC-431 recorded a wrong mechanism and why the wrong fix
looked strongest.

**Correction 1 — the first blocker is the caller's OWN body, not transitivity.** `work` declines on
`Unsupported("unboxed Const Some(Unit)")` — the dummy receiver pushed for a prelude-CLASS static call, which
`collect_unboxed.rs:83` default-denies. Out of subset before transitivity is consulted.

**Correction 2 — supporting `Const(Unit)` alone buys NOTHING.** It appears only for prelude-class statics,
and each also declines on its own un-whitelisted `CallNative` (`FileSystem::writeText` →
`CallNative(441, 2)`). Confirmed from the other side: `String.length` is a bare `CallNative(58, 1)` with no
receiver push, which is why the infallible control compiles. `CallNative` support is a per-native whitelist
with a bespoke emit arm, and the FS ones additionally need `MakeInstance` of the typed error classes, itself
unsupported.

**Correction 3 — the fix DEC-431 recommended is REFUTED, and it is the one that matters.** "Compile the
caller and bail to the VM at that call site" would be *strictly worse than today*: code 5 does not resume,
it re-executes the whole call from `ip: 0` (`src/vm/exec.rs:556-561`), so the hot loop would run natively,
bail, and then be re-run interpreted — **paid twice**. The mechanism I cited as supporting evidence is the
mechanism that makes the design unusable. That claim was [Inferred] from "the fault-exit already bails" and
never checked against what the redo does — the same failure shape as the `opt_level=none` comment (DEC-429).

Still viable, none chosen: a VM trampoline (the only one preserving native loop execution); compiler loop
outlining (mechanising the measured 773.83 ms → 2.42 ms workaround); whitelisting the fallible natives; or a
compile-time warning. Three ratchet tests (`src/jit/tests/decline_reasons.rs`) pin both decline reasons plus
the control that the same loop compiles once no fallible call shares its function — without that third test
the first two would pass equally well if the JIT declined everything.

### Changed — STANDING RULE: nothing is put aside until it WINS; first quiet-box baseline (DEC-432, 2026-08-01)
Developer-ruled: *"until we are winning we put nothing aside."* **No loss is ever CLOSED** — not as
"documented near-parity", not as "a tie inside the noise", not as "hardware-bounded". A loss leaves the
list one way only: by becoming a WIN. Other work may proceed in between; the hunt is paused, never
abandoned. This extends DEC-365 (an *unmeasurable* loss is recorded, not passed) to say a **measured** loss
may not be retired by argument either.

**Two of the same day's calls are REVERSED and reopened.** DEC-430 closed `floatloop` as "bounded by
hardware, ~11% ceiling, stops counting as JIT-programme work"; DEC-427 closed `listcontains` as "a TIE
inside the noise". Both were self-ruled, and under this rule that was not mine to decide.

**Fix shipped — `--emit` now REFUSES a non-quiet box.** DEC-431.1's root cause was `--emit` sharing the
gating threshold (`MICROBENCH_MAX_LOAD=2.5`), which permits a box that is measurably not quiet (12 features
flagged noisy at load 2.50 vs 5 quiet). Emit gets its own `MICROBENCH_EMIT_MAX_LOAD` (default **0.7**) and
**exits 2** rather than skipping — a skipped emit exits 0 having written nothing, which reads as success and
leaves the stale baseline in place. Verified: forced past its threshold it refuses and writes no file.

**First baseline emitted on a genuinely quiet box** (load 0.08, local release php-8.5.8+JIT with the JIT
probed, post-DEC-428, output-identity gated): 52 features, 11 OWED.
**HONEST SCOREBOARD: 41 WIN / 11 LOSS, geomean 2.36x, median 2.13x.** That is LOWER than the 42/8, 2.45x,
2.30x reported earlier today, and the correction is the point — three recorded "WINs" were loaded-box
artifacts and are now OWED at their true values (`mapget` 1.004->0.958, `mapinsert` 1.012->0.813,
`floatmul` 1.002->0.981), while `floatloop` APPEARED to move the other way (0.476 -> 1.05) — **CORRECTED by DEC-434.1: that
reading was itself a lucky best-of-3 draw; floatloop is ~0.776, a real loss, and stays on the hunt list** (DEC-428
finally visible against an undistorted php leg) and `strappend` enters at 0.448.

**The hunt list, worst first:** `fslines` 0.113 · `queryparse` 0.224 · `jsonround` 0.286 ·
`fsforeachline` 0.293 · `strappend` 0.448 · `mapinsert` 0.813 · `listcontains` 0.861 · `dbwork` 0.869 ·
`deepjson` 0.884 · `mapget` 0.958 · `floatmul` 0.981. Above all of them sits DEC-431's ~320x JIT cliff,
which is not a bench row but taxes any hot loop in a `throws` function — i.e. most real code.

**Caveat, flagged not buried:** `floatloop`'s 1.05 comes from the +27%-spread bench (best-of-25 read 0.92);
its flip limit is 0.893, so the margin to a false block is ~0.03. If the ratchet trips on it with no code
change, re-measure on a quiet box before calling it a regression.

### Found — a fallible call takes the WHOLE function off the JIT (~320x); VM string append is quadratic (DEC-431, 2026-08-01)
Set out to profile the `fsforeachline` loss with DEC-430's Ir-slope method and found something much larger
on the way in. **A bench ships; both fixes are PENDING RULINGS.**

**How it surfaced.** callgrind on `fsforeachline`: 97% of the profile was `memcpy`, and **14.0 of the 14.18
BILLION** instructions were the bench's own `fixture()`, not the read under test (fixture-only measured
13,996,282,532 Ir against the full bench's 14,184,618,009 — the two reads are ~188 M). **The setup was 74x
the thing being measured.** `fixture()` builds a string in a loop and writes it, so it declares `throws` —
and that turned out to be the whole story.

**Defect A — a fallible call anywhere in a function takes the whole function off the JIT.** Same hot
integer loop (the exact `intadd` body, 5M iterations):

| shape | time |
|---|---|
| loop alone (`intadd`) | 3.43 ms |
| loop + an INFALLIBLE prelude call (`String.length`) | 1.90 ms |
| loop + a FALLIBLE one (`FileSystem.writeText(…)?`) | **773.83 ms** |
| the same, that one call hoisted to another function | **2.42 ms** |

**~320x from one line's placement**, confirmed three independent ways. Root cause [Verified]: JIT
eligibility is transitive over the `Op::Call` graph — one un-compilable callee declines the whole graph, so
the caller's hot loop is interpreted. `throws` is not exotic; the checker REQUIRES it for any fallible
call, so every function touching the filesystem/database/network/a lock has it. Silent: it type-checks,
output stays byte-identical (Invariant 1 holds — a speed cliff, not a correctness bug), nothing warns.
Workaround until ruled: keep hot loops in their own function and hoist fallible calls out.

**Defect B — `s = s + x` in a loop is O(n²) off the JIT.** `PhStr::concat` always allocates a fresh buffer
and copies both sides, and as called it cannot do better: `body = body + x` compiles to
`GetLocal(1); Const; Concat(2); SetLocal(1)`, so at `Concat` the accumulator's `Rc` is **aliased** (local
slot + stack copy) and `Rc::get_mut` can never succeed. Measured at 5k/10k/20k lines — JIT
0.66/1.18/**2.33** ms (linear) vs VM 18.1/72.2/**492.1** and tree-walker 18.2/69.1/**494.8** (quadratic).
211x apart at 20k. A and B compound: A puts the function on the VM, B makes its string building quadratic
once there.

**What ships: `bench/micro/strappend`** — the string-growing idiom vs PHP's `.=`, on the default (JIT)
path: **0.48x**, 7%/5% spread (solid). **Why the suite was blind:** `strbuild` appends in a loop too but
RESETS the accumulator at 512 bytes, so it never grows and reports a 2.06x WIN. A `fallibleloop` bench for
defect A is deliberately NOT added — its ratio would be ~0.005 and would measure a compiler limitation
rather than a feature, distorting the geomean; it is in `KNOWN_ISSUES.md` instead, stated rather than
silently omitted. Nothing re-emitted (`strappend` reports as new, non-blocking — verified).

### Added — the ratchet reports per-feature measurement spread (DEC-430.1, 2026-08-01)
Developer-ruled answer to DEC-430's question: report the spread, leave `K=3`.

`microbench.sh` already takes the K samples, so tracking the WORST alongside the best costs nothing. The
JSON gains `vm_worst_ns` / `php_worst_ns`, the table gains a `spread v/p` column, and
`microbench-gate.sh` appends `[noisy: VM spread +N%]` when the VM spread reaches `MICROBENCH_NOISE_PCT`
(default 15 — above php's observed 2-5%, below phorj's 25-40%), plus one summary line. **No verdict
changes**; the gate blocks on exactly what it blocked on before. `--emit` was verified not to leak the
new fields — the emitted baseline's key set is byte-identical to the shipped one, `_owed` included, so
DEC-365's no-laundering guarantee is untouched.

It paid off on the first real run: 51 features on a quiet box, 5 flagged — `floatarith` +29%, `floatloop`
+27%, `mapvalues` +23%, `intadd` +21%, and **`listcontains` +59%**. DEC-427 had called `listcontains` "a
TIE inside the noise" only after a separate manual investigation; the gate now says so on every push.

**The limitation, stated because the inverse reading is worse than no marker:** over K=3 the spread is a
DETECTOR, not a measurement. Three draws routinely miss the tail — `listcontains` read **+1%** in a
7-feature run and **+59%** in the full one, minutes apart on the same box. A marker means "distrust this
row"; its absence means only "these three samples agreed", never "this row is solid".

Three new gate tests (7, 7b, 9). Each guard was checked by deliberately weakening it, which found a real
gap: deleting the threshold entirely passed every pre-existing case, so a broken threshold would have
marked all 51 rows and drowned the signal silently. Case 9 exists for that and fails without it. The
field-presence `!= "null"` checks are honestly labelled defensive-not-load-bearing (bash coerces the bare
word to 0 anyway). The underlying variance remains un-root-caused and blocked on PMU access; `_owed` was
not re-emitted.

### Measured — the box's clock, `floatloop` AT the hardware floor, phorj's variance localized (DEC-430, 2026-08-01)
No code change. Task #62's variance hunt, and it produced a bigger result than the variance itself.

**`/proc/cpuinfo` understates this box's clock by 31%.** A serial integer add chain (exactly 1
cycle/iteration on any modern x86 core) measures the effective frequency at **~2.75 GHz** across six pinned
samples; `/proc/cpuinfo` and the TSC both report **2.100 GHz** — the nominal invariant-TSC rate the guest is
told, not what the core runs at. That silently breaks the arithmetic: at 2.100 GHz `floatloop` computes to
1.69 cycles/iteration, which is physically impossible for a serial FP-add chain, and that contradiction is
what opened this investigation. Any cycles/iteration on this box must use the measured clock. The probe also
showed the clock is stable to 2.6% and `/proc/stat` steal never moved (223 → 223), excluding frequency
scaling and hypervisor steal as noise sources up front.

**`floatloop`: php is sitting exactly ON the hardware dependency floor.** Best-of-25, pinned + interleaved:
php **3.603 ms = 1.98 cycles/iteration**, phorj **3.899 ms = 2.15**. The body is a serial float-dependency
chain (`x = x + 1.5` feeds the next compare) and FP-add latency on this core is 2 cycles — so php is at the
floor and phorj is 0.17 cycles above it. Combined with DEC-429 (phorj already executes 12% FEWER instructions
per iteration), the maximum recoverable on this bench is **~11%**. It is a **near-parity bounded by
hardware**, and it stops counting as JIT-programme work.

**The variance: eight hypotheses refuted, root cause blocked on hardware counters.** Reproduced at 95%
spread (php 4%), then killed: host noise (zero steal, php stable to 2-4% interleaved on the same core);
frequency (measured, and it *anti*-correlates); code placement/ASLR (`setarch --addr-no-randomize` no
better); anything per-process at all (the swing happens **within one process** — 8 consecutive calls to the
same native code at the same address, 4.75 → 7.36 ms); Cranelift compile time (a loop-containing function
compiles eagerly on call 1); silent VM fallback (`--no-jit` is 883 ms, 170×, and itself stable to 3%); the
float path (`floatmul`, a *pure* float loop, is the most stable thing measured at 2-3%); and SMT/thread
contention (no SMT on this box; `phg`'s second thread is the one that sleeps at 0 utime). The one positive
correlation: unstable loops are the short high-IPC ones (`floatloop` 2.15, `intadd` 2.25 cycles/iteration)
while the stable one has latency slack (`floatmul` ~6.9), and the absolute spread scales with iteration count
— a sustained per-iteration rate difference, not a per-call warm-up. What is left is front-end
microarchitectural state, which needs PMU counters; `perf` is absent here and PMU access is not available, so
per Rule 14 this stops at OPEN with the instrument named rather than guessing at a loop-alignment change.

**Actionable consequence — the frozen `_owed` floors for short loops are too harsh.** `microbench.sh` already
uses the right estimator (best-of-K, not a median) but `K` defaults to **3**, and best-of-3 against a 25-40%
tail lands well above the true minimum: on `floatloop`, best-of-3 ≈ 4.5-5.0 ms vs best-of-25 **3.899**. So the
recorded ratio is systematically *pessimistic* for phorj on exactly the high-variance benches — a measurement
artifact, worth stating because it cuts the opposite way from every bias guarded against so far. Raising `K`
is not self-ruled: it multiplies the time of a gate that runs on every push and moves numbers across the
whole scoreboard. Nothing was re-baselined (DEC-365).

### Measured — the sticky phi costs NOTHING; `opt_level` was never `none` (DEC-429, 2026-08-01)
No code ships. A hypothesis was built, fully tested, measured at zero, and reverted — and it took two
prior diagnoses down with it.

**The premise was a stale comment.** `emit_unboxed/mod.rs` asserted twice that *"Cranelift's baseline
`opt_level=none` does NOT DCE the loop-carried sticky phi"*. But `compile/mod.rs:185` has set
`("opt_level", "speed")` since P-2a, and `speed` removes the phi for free. That one sentence was the
stated premise of DEC-425, of DEC-428's mechanism claim, and of this whole reverted change. Both comment
sites are corrected in place with the measured numbers and a "do not restore this" note.

**The instrument changed, and that is the durable part.** Wall clock on this box cannot resolve an effect
this size: pinned, interleaved, on a settled box, phorj's `floatloop` spanned **4.03-6.68 ms** (66%,
visibly bimodal) while php spanned **3.62-4.01 ms** (11%). The verdict came instead from **callgrind
instruction counts by SLOPE** — run at two iteration counts, take `ΔIr / Δiterations` — which cancels
startup, is immune to load, and reproduces to **~0.2%** (the same binary re-measured on two occasions gave
6.9956 and 7.0106 Ir/iteration, so ±0.02 Ir/iter is the method's own floor here). From here on that is the
instrument for JIT work; wall clock only confirms a win the slope already showed.

| build | Ir / iteration |
|---|---|
| pre-DEC-428 (`8c57c79`) | 9.9981 |
| master with DEC-428 (`73d085a`) | 7.0106 |
| + the loop-scoped sticky (reverted) | 7.0003 |

**DEC-428's measurement stands; its mechanism claim was wrong.** `floatloop` 8.2 -> 5.24 ms is real and now
confirmed as 10.00 -> 7.01 Ir/iteration (-30%) — but the win came from what disappears at each newly-PROVEN
site (`sadd_overflow` + `uextend` + `bor` collapsing to one `iadd`), not from dropping the phi. **DEC-425's
"100% of the gap is the speculation STICKY" is likewise corrected**: the per-op accumulation was ~30%, the
phi 0%, and its supporting datum ("`#[UncheckedOverflow]` runs ~4.0 ms") does not reproduce — that variant
measured 6.4-7.6 ms today, consistent with the corrected model.

**The real diagnosis of `floatloop`, and it is not codegen volume:** phorj runs **7.01** Ir/iteration
against php's **8.00** — 12% FEWER instructions — and is still ~11% slower (4.03 vs 3.62 ms best-of-9).
The gap is instructions-per-cycle plus phorj's own variance, so more static proving cannot close this
shape. Its body is a serial float-dependency chain (`x = x + 1.5` feeding the next compare), leaving both
engines ~0.08 ns/iteration apart against the same latency bound — a documented near-parity rather than a
tuning debt. The measured ratio (0.90 best-of-9, 0.78 median) is better than the frozen `_owed` floor, and
is deliberately NOT re-baselined: DEC-365 forbids laundering an OWED row, and a box with a 66% spread has
no business writing a baseline.

### Changed — JIT: conditional accumulators now PROVE; `floatloop` -36% (DEC-428, 2026-08-01)
> **Mechanism corrected by DEC-429 (below/above in this file).** The -36% is real and reproduces as
> -30% Ir/iteration; it comes from the per-op `sadd_overflow`+`uextend`+`bor` sequence vanishing at each
> proven site, NOT from dropping the loop-carried phi — which is measured to cost zero, because Cranelift
> runs at `opt_level=speed` here and removes it. The paragraphs below preserve the original reasoning as
> written; read them with that correction in force.
The first step of the JIT programme, scoped to the one gap DEC-425 had already diagnosed down to a line.

`range_acc`'s body walk used to REJECT any `JumpIfFalse` that was not a loop guard — i.e. any `if`
inside a loop body. That single refusal is why a CONDITIONAL accumulator could never be proven, and one
unproven speculated op anywhere in a function forces the loop-carried sticky overflow phi that Cranelift
at `opt_level=none` will not remove. So `if (cond) { count = count + 1; }` — the commonest counting
idiom there is — taxed EVERY iteration of its loop.

The walk now models one body-level `if`: a FORWARD `JumpIfFalse` landing inside the loop opens a
conditional region (backward or escaping targets are refused); the operand stack must be EMPTY at the
branch (a statement `if`, so the two paths cannot disagree about depth at the join); only ONE region at
a time (a nested `if` is refused, not approximated); at the join every slot the region MAY have written
is widened to UNKNOWN; float arithmetic and the remaining comparisons are modelled as
pop-two-push-unknown, listed EXPLICITLY rather than swept up by a catch-all (`Neg` is deliberately
excluded — it is a speculated overflow op, not a neutral one). Accumulators keep their envelope interval
across the join, and that is earned rather than assumed: the envelope solve already takes
`min(growth.lo, 0)` / `max(growth.hi, 0)` per site, so it has ALWAYS modelled "this site may or may not
run" — which is exactly a conditional site.

**Measured**: `floatloop` **8.2 ms -> 5.24 ms (-36%)**, ratio **0.46 -> 0.71**, checksum unchanged
(500004) on the VM and the tree-walker; `intadd` (1.27x) and `fibrec` (2.00x) unmoved, so no collateral.
**Still a loss** (php 3.59 ms) — and the residual is now precise: `needs_sticky` is computed over the
WHOLE function, and `floatloop`'s `return acc + Conversion.truncate(x)` is an `AddI` *outside* the loop
body, so it stays unproven and the phi survives. One op executed ONCE taxes 5,000,000 iterations. The
fix belongs at the emitter, not the analysis (an unproven op not inside a loop can take a per-op fault
branch, free at one execution, instead of forcing the sticky) — recorded as the next step rather than
rushed into this change.

**Two of the three new guards are NOT load-bearing, and the register says so.** The join-widening IS:
deleting it fails `task9_join_widening_prevents_a_stale_then_branch_interval`, built specifically to
bite (`t` starts at 5e18 and the conditional assigns `1`, so carrying the then-branch interval past the
join would prove an elision that drops a real overflow check). The nested-region refusal and the
conditional-counter-write refusal are currently UNREACHABLE — the shapes that would reach them are
refused earlier by the single-writer counter rule — so they are kept as defensive checks and *labelled*
defensive, not presented as proven. The first attempt at those rejection tests was VACUOUS; it was
caught only by deliberately weakening each guard and re-running, which a green suite alone would have
hidden.

`range_acc.rs` was a grandfathered 762-line file and this pushed it to 829, so it split by cohesion into
`src/jit/range_acc/{mod,walk,verify}.rs` (368 / 336 / 149) — driver, one-trip body walk, one-`G`
verification attempt. Invariant 13's "split it, do not grow it", enforced by the gate rather than
remembered.

### Measured — the standing scoreboard: 42 WIN / 8 LOSS, geomean 2.45x vs PHP+JIT (DEC-427, 2026-08-01)
`dbwork` and `listcontains` were the last two losses not already blocked on a ruling. Both diagnosed,
neither worth a code change, and with them the whole board is accounted for.

**`listcontains` (0.87x) is a TIE inside the noise** — [Verified: `#[UncheckedOverflow]` makes no
difference (23.3 vs 24.0 ms), so unlike `floatloop` this is not the sticky phi; and PHP's own leg swings
21.4 → 31.5 ms across three consecutive runs on a quiet box.] It was 0.024x before the DEC-311 vertical;
the vertical did its job and the remainder is measurement error.

**`dbwork` (0.86x) terminates where everything else does** — [Verified: ~25% VM interpretation, ~16.6%
malloc/free, and `sqlite3VdbeExec` only **2.7%**.] Both legs run the same embedded SQLite, so the engine
is not the variable: the delta is phorj-level dispatch of the prepare/bind/exec chain, per row.

**Scoreboard** (quiet box, release php-8.5.8 + JIT, interleaved, pinned, output-identity gated):
**42 WIN / 8 LOSS across 50 micros, geometric mean 2.45x, median 2.30x** — phorj is ~2.4x faster than
PHP-with-JIT across the suite. 28 features win by ≥2x; 4 sit within ±10% of parity.
Biggest wins: `setunion` 50.5x, `setdifference` 33.8x, `trycatch` 27.6x, `sumby` 15.6x, `listreduce`
13.3x, `isemail` 12.1x. The 8 losses: `listcontains` 0.87x, `dbwork` 0.86x, `deepjson` 0.85x,
`floatloop` 0.46x, `fsforeachline` 0.29x, `jsonround` 0.29x, `queryparse` 0.22x, `fslines` 0.11x.

**Every remaining loss now has one of three named causes:** VM interpretation of user code (five of
eight — the JIT programme, blocked on a scope ruling); a representation/design choice (`queryparse`'s
typed bag graph, `deepjson`'s multi-pass lazy parser — both adjudicable); or noise at parity
(`listcontains`). Nothing on the board is unexplained, and nothing left would move under a tuning pass.

### Measured — `jsonround`/`deepjson` are design questions, not tuning gaps (DEC-426, 2026-08-01)
No code change. Two tuning attempts, both measured, both rejected; both benches stay OWED.

**`deepjson` (0.84x, 1038 ms vs php 869 ms): 55% of it is SKIPPING** — [Verified by callgrind:
`skip_string` 28.3%, `skip_value` 26.1%.] The lazy parser walks the document roughly THREE times per
parse — `validate_json` over the whole doc (required, since `Json.parse` must null on malformed
input), then a delimitation scan per materialized level — against PHP's single `json_decode` pass. The
memo is fine (`materialize_lazy` already caches, so the two `topString(rec0, …)` calls share one
materialization). DEC-294's lazy bet is that unread records never allocate; at 12 records a skip-scan
simply is not much cheaper than materialize-as-you-go, and we pay it three times. The structural fix —
have validation record child offsets so the root's re-scan disappears — changes the lazy
representation, so it is a DEC-294 design question.

Rejected, and recorded so they are not retried: **bulk-skipping the plain run via a slice `position`**
instead of a per-byte bounds-checked `get` [Verified: −2.2% instructions, wall clock 1034 → 1038.5 ms
— nothing; an initial 3-sample "1015" was noise a 7-sample median did not reproduce], reverted on the
same rule as `exec_hot`; and **`#[inline]` on `skip_string`** [Verified: 1105 ms — actively worse].
Neither could have helped: the document's strings are 2–8 bytes, so the cost is per-STRING call
overhead, not per-byte scanning.

**`jsonround` (0.29x) loses for a completely different reason** — [Verified: VM interpretation ~34%,
malloc 15.6%, the parser only 11.7%.] The cost is the bench's own phorj code: two nested seven-arm
exhaustive `match`es per field read, because that is how phorj gets a typed value out of the `Json`
ADT, versus PHP's `$j['id']`. Fair as an idiomatic comparison — but it names a real ergonomics gap:
phorj has no `Json.getInt(key)`/`getString(key)` accessor. Adding one would be both an API improvement
and a large perf win (a native replaces ~14 interpreted match arms per read). New user-visible stdlib
surface, so it is recorded as a PENDING question rather than self-ruled (Invariant 15).

### Measured — `floatloop` never regressed; the loss is one loop-carried sticky phi (DEC-425, 2026-08-01)
The ratchet recorded `floatloop` at 1.011 (WIN) through 2026-07-20 and it now reads 0.48 — the one
apparent WIN→LOSS flip on the board. It is not a regression. [Verified by building the exact commit
whose baseline recorded 1.011 (`b5ce34c`) and measuring it against the SAME php: phorj 9.12 ms vs php
3.95 ms — already a 0.43 LOSS. Today's master is 7.2 ms, i.e. phorj got *faster*.] The "flip" is a
baseline-ENVIRONMENT artifact: docker `php:8.5-cli` was ~2.3x slower on this loop than the local
release php. **That taints every WIN in the pre-2026-08-01 baseline** — the 2026-08-01 re-emit
supersedes it, which is why several formerly-"WIN" rows now read as losses or ties.

**100% of the gap is checked int arithmetic**, and removing it wins: [Verified: the same bench with
`#[UncheckedOverflow]` runs at ~4.0 ms against php's ~3.4–3.95 ms.] The mechanism is the one
`emit_unboxed` documents — `needs_sticky` is true when ANY reachable speculated op is unproven, and
Cranelift at `opt_level=none` will not DCE the resulting loop-carried phi. Here the hot counter IS
proven (`range_proven_ops` returns exactly `[24]`); the unproven op is `acc = acc + 1` inside a branch
that fires **7 times in 5,000,000 iterations**, and its mere reachability taxes every one of them.
[Verified: making the branch unreachable leaves the 2x unchanged — it is the phi, not the add.]

This is a general shape: **any counted loop with a conditional counter** pays it. The fix is to prove
the accumulator so no phi is emitted; `range_acc::accumulator_elision` already exists for exactly this
and [Verified by probe] declines this shape inside `verify_with_g`, whose interval walk does not model
the float ops and `CallNative` in the body. Not built — widening an overflow-elision proof is the
"ONE unsound spot" the range-analysis tests name, and it belongs to the JIT programme DEC-423 says
needs a scope ruling first.

### Fixed — `queryparse` built a fresh `ClassLayout` per instance (DEC-424, 2026-08-01)
First target off the DEC-423 loss list. DEC-338 is recorded BUILT to "flip the `queryparse` 0.10x
loss" and the sweep measured 0.13x. Worth being precise: **DEC-338 was really done** — `Request.parse`
IS nativized and the interpreter no longer walks that body. It just did not address where the time
goes, and nobody re-measured to find out.

`native::http::request::inst` called `ClassLayout::from_sorted_names` on EVERY instance. A layout is a
sorted `Vec<String>` plus a name→slot hash map and depends only on the CLASS, so one `Request.parse`
allocated a fresh string vector, sorted it, and built a fresh hash map once per bag — `Request`,
`ParamBag`, `HeaderBag`, `AttrBag`, `FileBag`, `RequestBody`, every `Cookie`. [Verified by callgrind:
malloc/free was ~38% of instructions retired, `HashMap::insert` and `Rc<ClassLayout>::drop_slow` right
behind.] Caching it per class: **1839 ms → 1177 ms (−36%)**.

Two follow-ons, one of them a lesson: the first cache used a `HashMap` whose SipHash of the class name
promptly showed up as 3% of the profile — more than the lookup it replaced, so it is a `Vec` with a
linear scan now (under a dozen classes). And `Instance::new` + a `set_field` per field takes a fresh
`RefCell` borrow per field; new `Instance::from_slots` fills the slot vector directly. Those two are
worth a further −3% of instructions and nothing measurable in wall clock — kept because they REMOVE
work rather than reorganise it, but recorded as marginal rather than sold as a win.

**Result: 680.7M → 446.6M instructions (−34%), ratio 0.13x → 0.22x.** `webish`, which also parses
requests, gains too and stays a 2.85x WIN. Still a 4.5x loss, so `queryparse` stays OWED.

**Why it still loses:** malloc/free is *still* 28.6%. PHP builds plain arrays; phorj builds a typed
object graph — a `Request` plus six bags plus every decoded string, each its own allocation. That is a
representation difference, not a tuning gap. Closing it means lazy bags (parse the query only when
`req.query` is touched) and/or arena allocation — a design change to the rich-Request model of DEC-331
slice 2, so it is adjudicable rather than self-decidable.

### Added — the G-8 ratchet is ARMED, with every loss frozen as OWED (DEC-423.1, 2026-08-01)
Developer-ruled follow-on: re-emit the baseline on the local release php **and** freeze the known
losses as OWED in the same change, so `--emit` cannot launder them.

`_owed` is **derived** at emit time from every feature that loses — never hand-maintained, so there is
no list to forget and no way to write a loss in as normal. A feature leaves it by being FIXED and
re-emitted, never by being edited out. DEC-365's no-hidden-loss rule is now structural rather than a
convention. The gate reports every owed loss on every run, **blocks if one deepens** past 25%, and says
RECOVERED (asking for a re-emit) when one flips to a win. Emitted: 51 features, 8 OWED.

**The gate now runs off-docker.** It resolves PHP as `MICROBENCH_PHP_BIN` → docker → the oracle php from
`scripts/toolchain.env` if it genuinely JITs (probed via `opcache_get_status()`, not assumed). [Verified
live: resolves php-8.5.8, reports all 8 owed losses, PASSES, 81 s at the pre-push default.] The first
attempt at this was wrong and the push it rode on caught it — the fallback was gated on the docker
BINARY being absent, but here the client is installed and only the daemon is unreachable, so the gate
was committed as "armed" while still skipping. Both conditions are now one `docker version` probe. The
seam tests covered the gate's decision logic, not its environment resolution, which is where the bug
was.

**Near-parity wobbles no longer wedge pushes.** Arming the gate immediately exposed a flaw: `mapinsert`
(baseline 1.012) tripped the flip check at 0.940 — a 7% swing on a shared box. The band is now relative
to the baseline as well as absolute; a strong WIN is unaffected. [Verified: setunion 52.5 → 0.5 still
FAILS; mapinsert 1.012 → 0.94 now warns.]

**It now actually runs in the pre-push lane, without wedging pushes.** Two more problems, both only
visible end-to-end: it skipped on load it had *caused itself* (2.78 right after `cargo build --release`,
against a 2.5 threshold), so it now WAITS for that transient load to settle (bounded, 90 s default)
before skipping; and a timing verdict could block falsely (observed: a blocking flip at load ~1.5 that
did not reproduce), so timing verdicts are now CONFIRMED by re-measuring only the flagged features
before blocking. A real regression reproduces; load noise does not. Identity breaks skip confirmation
and block on sight. If the re-measure fails, suspects are reported and not blocked (DEC-365).

**The gate has tests now — it had none.** `scripts/test-microbench-gate.sh` (pre-push, ~1s, no docker
or php) pins seven behaviours through the `MICROBENCH_GATE_JSON` seam that was built for tests and
never used — which is exactly how the gate stayed dark. Each fixture derives from the baseline so it
cannot drift. The tests were verified to FAIL against a deliberately broken gate, not merely to pass.

### Fixed — the microbench harness had been DARK, and a stale comment is why (DEC-423, 2026-08-01)
`scripts/microbench.sh` said *"the local builds are all ZTS DEBUG, JIT off, so they are NOT a valid
baseline"*. That is false for the stack's own oracle php. [Verified on php-8.5.8: `Debug Build => no`,
`Thread Safety => disabled`, OPcache present, `opcache_get_status()["jit"]["on"] === true`.] The
harness has always had a `MICROBENCH_PHP_BIN` escape hatch; nobody used it because the comment said it
was worthless. Docker is absent in the dev container, so every run skipped, the G-8 ratchet skipped on
every push, and the OWED backlog grew against infrastructure that was never missing. Comment corrected
with the one-command local recipe.

**Three things the dark gate let through**, all found in the first sweep:
- **`floatloop` flipped WIN → LOSS**: baseline 1.011, now 0.48x, reproducible across 3 runs on a quiet
  box (and the JIT is engaging — 836 ms `--no-jit` vs 8.0 ms with). Exactly the signal the ratchet
  exists to block. Not yet attributable to a phorj regression: the baseline was taken against docker
  `php:8.5-cli` and this is phpbrew php-8.5.8, so the ratios are not interchangeable.
- **`dbwork` was a PHANTOM bench** — it imported `Core.DatabaseModule.Database`/`.Statement`, an API
  that never shipped (the real one is `Core.Database.Connection`/`Row`). It could not `phg check`, so it
  aborted the whole harness run, yet `bench/micro-baseline.json` carries a `dbwork` ratio: that entry
  was fiction. Repointed at the real API; it runs on both legs now with a matching checksum and is an
  honest 0.84x LOSS.
- **`fslines`, `queryparse` and `fsforeachline` are absent from the baseline entirely**, so the ratchet
  could never have gated them. `queryparse` is the sharp one: DEC-338 is recorded BUILT to "flip the
  queryparse 0.10x loss", and it measures 0.13x today.

### Measured — the first honest G-8 scoreboard: 42 WIN / 9 LOSS
51 paired micros vs release php-8.5.8 + JIT, interleaved, both legs pinned, quiet box,
output-identity gated. Losses worst-first: `fslines` 0.10x · `queryparse` 0.13x · `fsforeachline` 0.27x
· `jsonround` 0.29x · `floatloop` 0.48x · `deepjson` 0.79x · `dbwork` 0.84x · `listcontains` 0.94x ·
`floatmul` 1.00x (tie).

For scale on the winning side: `setunion` 48.9x, `setdifference` 33.9x, `trycatch` 27.7x, `sumby`
15.6x, `listreduce` 14.2x, `isemail` 12.5x. phorj is not generally slow — it is specifically slow on
nine things, and the mandate now has a finite named target list instead of a vibe.

The G-8 ratchet is still SKIPPING, deliberately: arming it needs a baseline recorded on this php, and
`--emit` today would write floatloop's 0.48 in as the new normal — laundering the very flip the gate
exists to catch, which DEC-365 forbids.

### Fixed — the line-read benches were comparing against a HANDICAPPED PHP (2026-08-01)
`bench/micro/fslines.php` and `fsforeachline.php` folded each line with `mb_strlen`. phorj's
`String.length` is documented BYTE length, so the faithful twin is `strlen` — and `strlen` is faster.
The bench was making PHP do more work than phorj and calling the result a comparison.
[Verified, JIT on, 40k lines: `mb_strlen` 4.31 ms vs `strlen` 2.52 ms median.]

**Every line-reading loss recorded before today was understated**, on two counts at once — this, and
the baseline having been measured with PHP's JIT off. Against the ruled bar (PHP at its best, JIT on)
the honest numbers are: PHP 2.52 ms · `forEachLine` 8.59 ms (**3.41x slower**) · `lines` 22.34 ms
(**8.87x slower**). Both OWED under DEC-365. The DEC-347 "4x" and the DEC-422(a) "1.6x" are superseded.

### Changed — the higher-order native call path allocates nothing per element (2026-08-01)
`Vm::call_closure_value` cloned the closure's captures into a throwaway `Vec` on every call, and
`ClosureInvoker` took an owned `Vec` of arguments — so every list element and every file line cost two
heap allocations before any work happened. Captures now clone straight onto the operand stack, and the
invoker takes a borrowed slice (`&[Value]`), so `&[x.clone()]` is a stack temporary.

[Verified: `forEachLine` over 40k lines 9.15 -> 8.59 ms (-6%), 131M -> 116M instructions retired.]
Small on this bench, but it is the per-element path for EVERY higher-order native — `List.map`,
`filter`, `reduce`, `sortWith`, `Option.map`, the regex and test callbacks — so the whole language
collects it. The tree-walker still builds one owned `Vec` (its `call_closure` consumes one); the oracle
is not the perf target, parity is.

### Added — `FileSystem.forEachLine`, the native-driven line reader (DEC-422(a), 2026-08-01)
Reads the same lines as DEC-347's `lines(path)` under identical terminator rules, but the loop runs
INSIDE the native (and inside `fgets` on the PHP leg), so the two phorj-level virtual calls per element
disappear and the file is opened ONCE instead of re-opened and seeked per 64 KiB chunk. Built on
`NativeEval::HigherOrder` + the backend-supplied re-entrant `ClosureInvoker` — the same mechanism
`List.map` uses, so one body drives the interpreter and the VM. PHP twin `__phorj_fs_for_each_line`
(Invariant-14 ladder case 1, faithful — no quarantine); three legs byte-identical on every shape that
breaks line readers, plus a missing file.

**MEASURED, and it is still a loss.** 40k lines, same fixture and fold, output-identity gated, medians
of 5: PHP `fgets` 5.7 ms · **`forEachLine` 9.1 ms (1.6x slower)** · `lines` iterator 22.8 ms (4.0x).
So 2.5x faster than the iterator and the gap against PHP drops from 4.0x to 1.6x — recorded as an OWED
verdict per DEC-365's no-hidden-loss rule, NOT reported as a pass. The local `php` is a debug/ZTS build
with JIT off (which flatters phorj) and the official G-8 harness needs a docker daemon this container
does not have.

**Where the residual is, measured rather than guessed.** A probe build skipping only the closure
invocation: 4.4 ms without it, 13.1 ms with it — the per-line CALL FRAME is ~2/3 of the time, and the
read itself (4.4 ms) is within reach of PHP's own (2.1 ms). This reshapes DEC-422(3): a JIT vertical for
foreach-over-`Iterator` would close `lines`, but a closure invoked from inside a native is not an
iterator virtual call, so it does not touch this path.

**The trade, stated because it is not free.** `lines` stays. The closure cannot `break`, cannot `return`
from the enclosing function, and may throw only `FileSystemError`. And since phorj closures capture
locals BY VALUE, accumulating needs a field on a holder object — a `mutable int` assigned inside the
closure silently stays 0, with no error. That is closure behaviour generally (`List.map` is the same)
and already in FEATURES.md, but it is the first thing anyone writes here, so `examples/fs/foreach-line.phg`
and `tests/fs.rs` both show the working pattern.

The native keeps its two failure channels apart (`ForEachEnd::{Io, Closure}`): an I/O failure becomes a
catchable typed `FileSystemError`, while the closure's own failure propagates untouched. Collapsing them
would hand a caller's error to a `catch (FileSystemError e)` that has nothing to do with it.

### Added — `Core.ErrorModule`, phorj's standard error taxonomy (DEC-421, 2026-08-01)
Six error types every program can throw and catch — `RuntimeError`, `LogicError`, `MathError`,
`TypeMismatchError`, `InvalidValueError`, `IoError` — so code that needs a conventional error does not
have to declare its own. **FLAT on purpose:** none of the six extends another. PHP's
`Throwable`/`Error`/`Exception` split was considered and rejected — mirroring it would import a
much-criticised hierarchy into a language that deliberately lacks one, and would decide phorj's error
model as a side effect of a lift feature. Flat also means `catch` needs no subclass matching.

They are ordinary phorj classes `implements Error`: no new `Value`, no new `Ty`, no new `Op`, and they
transpile to `extends \Exception` like any other phorj error. Three legs verified byte-identical on a
throw/catch/dispatch path.

**Three of the six names avoid a collision, and that is why they read as they do.** `ArithmeticError`,
`TypeError` and `ValueError` — the obvious spellings, and the ones the ruling named — are all real PHP
**builtin classes**, so `E-RESERVED-NAME` rejects them: `class TypeError extends \Exception` would
redeclare PHP's own.

**`phg lift` maps PHP's builtin exceptions onto the set**, in both `catch` and `throw new` position,
emitting `Core.ErrorModule` plus one member import per type USED (importing all six would be
`E-UNUSED-IMPORT` — a lift failing the very check it exists to pass). A lifted
`catch (\RuntimeException $e)` now type-checks with **no hand edits**; before this it emitted valid
syntax that died on `unknown type RuntimeException`. The mapping is SEMANTIC, not hierarchical:
`InvalidArgumentException` → `InvalidValueError`, because PHP files it under `LogicException` for
hierarchy reasons but what it reports is a bad VALUE. An exception with no honest counterpart keeps its
own name and gets a `// CANNOT LIFT:` note rather than being coerced into the nearest phorj type.

`examples/lift/errors.php` + `errors.phg` are the walkthrough; the `.phg` is byte-identity-gated on all
three legs and its output matches the original PHP under php-8.5.8.

**Not included, and now recorded: `throws` inference** (`KNOWN_ISSUES.md` §LIFT-THROWS). A lifted
`throw` still needs its clause by hand — phorj has checked exceptions and PHP does not, so there is
nothing to derive one from, and a draft that CHECKS needs three draft-visible choices that are the
developer's to rule.

### Fixed — one exception walk instead of three near-identical ones (2026-08-01)
The `throw new X` arm had been added to two of the three recursive walks and, in one, to the WRONG one:
mapped exception names were reported as UNmappable, so a correctly-lifted draft carried bogus
`// CANNOT LIFT:` notes for types it had emitted properly. Replaced by a single `visit_exception_sites`
visitor (`src/lift/lifter/exceptions.rs`) that answers all three questions, making that class of
mistake unrepresentable — a new statement form is handled once, for every question at once. Also took
`lift/lifter/decls/statements.rs` from 438 to 260 lines (Invariant 13).

### Fixed — member imports were not completable, for ANY Core module (2026-08-01)
`import Core.` offered module PATHS only; a trailing `.` returned an empty list for every module.
[Verified: `import Core.ErrorModule.`, `import Core.FileSystemModule.` and `import Core.Output.` all
returned `[]`.] A **member-gated** module has no other way in — `import Core.ErrorModule;` alone leaves
its types bare (`E-INJECTED-TYPE-BARE`) — so DEC-421's taxonomy was untypeable from the editor the day
it shipped, breaking Invariant 17's 100% rule one level below the hole `withLock` fell through.

Fixed with `cli::module_catalog::core_module_members`, derived from the same two registries as
`core_module_paths` (a row's injected `bare_types` + the natives registered under that exact module
path), so a new type or native is completable the moment it is registered — no LSP edit.

### Fixed — `examples/fs/lock.phg` was racy under the concurrent test corpus (2026-07-31)
The example reset its state by DELETING its working directory, and the repo's test corpus runs every
example concurrently (`tests/format.rs` fans the corpus across cores; `tests/differential.rs` runs it
too). Since phorj has no per-process unique-path source, several copies shared one fixed temp path, so
one run could remove the counter another was tallying. **[Verified]** by hammering the old file: 16
concurrent runs produced 4 DISTINCT outputs, including real errors — `removeDirAll: Directory not empty
(os error 39)` and `appendText: … No such file or directory`. It surfaced as a single transient failure
in a full `--workspace` run.

Fixed with the example's own subject: all mutation now happens inside one `withLock(serial)`, so
concurrent runs serialize and every copy prints the same thing. No directory and no deletion — the paths
sit directly in the temp dir and state is reset by writing the counter empty under the lock. The inner
demos deliberately use a different lock file, because the OS lock is per-descriptor and a blocking
`withLock` nested on the same path would deadlock against itself. Re-verified: 3 rounds of 16 concurrent
runs, 1 distinct output each time.

Also fixed the same class of bug in `native::fs_lock`'s contention test: its scratch path is now
PID-qualified (a fixed `/tmp` path is shared state between concurrently-running test binaries), and the
`sleep(400ms)` that waited for the external holder is replaced by waiting on an observable signal the
holder creates. Under full-workspace load the holder had sometimes not acquired yet, so the try
succeeded and the assertion failed — raising the sleep would have been a bandaid over a race.

### Changed — dev builds drop DEPENDENCY debuginfo, cutting `target/debug` 24 GB → 7.4 GB (2026-07-31)
`[profile.dev.package."*"] debug = false`. Measured, not estimated: a clean full build of the workspace
with `--all-features` produced **7.4 GB** of `target/debug` against **24 GB** before, and free disk went
from 2.1 GB to 19 GB.

**Why it mattered for speed.** The container's writable allowance is finite, so builds were hitting
`No space left on device` — which forced `rm -rf target` and a cold ~10-minute rebuild on every push.
Keeping `target/` warm is what removes that, and dependency debuginfo was what filled it: nobody steps
into cranelift or rustls with a debugger.

**Phorj's OWN debuginfo is untouched** — `[profile.dev]` keeps the default `debug = 2`, so backtraces,
`gdb`/`lldb` on phorj code and every panic message keep full fidelity. The only thing given up is
variable inspection *inside third-party crates*.

Together with `cargo-nextest` now installed (the hooks already preferred it and were falling back to
`cargo test`), a WARM full-suite cycle — touch `src/lib.rs`, recompile, run all 2686 tests with
`--all-features` — is **43 s**, of which 28 s is the parallel test run.

### Fixed — a phorj function named after a PHP builtin no longer kills the PHP leg (2026-07-31, DEC-420)
`function count(int n)` passed `phg check`, ran on both Rust backends, and transpiled to
`Cannot redeclare function count()` — the PHP leg exited 255. That is exactly the DEC-213 failure mode
(`Cannot redeclare class DateTime`) with the class half fixed and the function half still open.

Developer-ruled to **MANGLE** rather than reject: no program that compiles today stops compiling, and
DEC-213 already set the precedent by mangling colliding enum VARIANTS. The emitted PHP name gains a
trailing `_` (`count` → `count_`), the same convention, so the two collision fixes read alike.

**Definition and every call site route through one `php_free_fn_name`** — mangling the definition alone
would swap one fatal for `Call to undefined function count_()`. The three sites are the definition, the
call, and the first-class-callable reference; a test asserts the call site specifically. Methods are NOT
mangled (a `count()` method is legal PHP), and a non-colliding name is untouched.

The builtin-function list lives beside the class list in `php_names.rs`, under the same DEC-213 rule:
ONE list, so the mangle set cannot drift from the emit set. It covers the always-present core rather than
the extension-gated tail — a miss is not silent, it surfaces as the same `Cannot redeclare` fatal from the
transpile→real-PHP oracle, and the fix is one row.

### Added — the PHP lifter reads `try`/`catch`/`finally` (2026-07-31, LIFT-TRY)
The lift subset had NO exception handling: the parser refused the `try` keyword and the printer listed it
as out of subset. It now handles all four shapes real PHP writes — a root-qualified type
(`catch (\RuntimeException $e)`), a UNION (`catch (A | B $e)`, every member preserved rather than
narrowed to the first), PHP 8's variable-less `catch (T)` (a binding is synthesised, since phorj's
`CatchClause` always binds), and `try`/`finally` with no catch at all. A bare `try` with neither arm is a
PHP syntax error and is reported as one instead of becoming a block.

Supporting changes: `\` is now a lift TOKEN rather than `unexpected character` — needed for qualified
catch types, and it also unblocks reading FQNs generally; and the lift printer can render a union type,
which it previously refused.

Every test asserts the lifted draft RE-PARSES, not merely that it contains the right substrings — a
plausible string that does not parse would be useless as a draft while passing a substring check.

**`throw` landed in the same session**, including the qualified `new \RuntimeException(…)` real PHP writes
— previously a LEX error, so unreadable. That also fixed an inconsistency the change exposed: `catch`
stripped PHP's root-namespace marker while `new` did not, so a lifted `throw new \RuntimeException(…)`
emitted a `\` that is not valid phorj — an unparseable draft beside a correctly-lifted catch in the same
function. Both now route through one `strip_root_ns`. PHP 8's throw-as-an-EXPRESSION stays refused, since
lifting it wrongly would move where the throw happens.

**Still deliberately out:** a lifted `try { … } finally { $h->close(); }` is NOT raised to `using` — that
is shape recognition, not printing, and guessing wrong rewrites the meaning of code the lifter does not
understand. Recorded in KNOWN_ISSUES under LIFT-USING with a test pinning the behaviour so it cannot
drift. A lifted error path also still needs a human for the exception TYPES (PHP's
`RuntimeException`/`LogicException` have no phorj counterpart); whether to map that hierarchy is a
PENDING developer question.

### Fixed — the WASM playground build was broken for six consecutive CI runs (2026-07-31)
`INJECTED_SPAN_BASE` was `1 << 32`, which is a compile ERROR on `wasm32-unknown-unknown`: `usize` is 32
bits there, so the shift overflows during const-eval — `error[E0080]: attempt to shift left by 32_i32,
which would overflow`. Introduced with DEC-364 (`using`); the playground workflow went red at that commit
and stayed red for every push after it.

**The local gate could not see it.** `cargo test`, both `clippy` passes, `cargo build --release` and
`cargo check --no-default-features` all target the 64-bit host, and the `playground` workflow was the
project's only wasm32 compile. Every local signal was green while the deploy was broken.

Two changes, so the class cannot recur:
- The base is now `1 << 28` (256 MiB) — still absurdly beyond any `.phg` source, and it fits a 32-bit
  `usize`. A `const _: () = assert!(…)` proves the whole `base + fragments * stride` range is
  representable **on the target being compiled**, which is the only place the check means anything. (The
  first draft of that assertion overflowed on its own multiplication — `checked_mul` before
  `checked_add` is what makes it correct.)
- **`scripts/wasm-check.sh`, wired into pre-push**: `cargo check` for wasm32 on both the library
  (`--no-default-features`, since `jit` is a default feature and cranelift cannot target wasm) and the
  `phorj-playground` crate in release — the exact configuration the workflow builds. A missing wasm32
  target is a LOUD skip, never a silent pass.

### Changed — `examples/fs/lines.phg` demo body extracted to a named function (2026-07-31)
`phg format` renders a closure body on ONE line, which strands the comments inside it: the numbered steps
ended up in the `catch` block, describing code that was no longer next to them. The body is now a named
`demo(path)` the `withLock` closure calls, so each comment stays with its statement.

### Added — `FileSystem.lines(path): Iterator<string>`, streaming line reads (2026-07-31, DEC-347)
O(chunk) memory instead of slurping: **[Verified] 23.7 MB peak RSS on an 84.7 MB / 1.2 M-line file,
against 322 MB for `readText` + `String.split` — 13.6x less**, and 23.7 MB is the same figure the ruling
cited for `Input.lines()`.

No file HANDLE exists. The ruling rejected a `FileHandle` type (blocked by C4: no transpiling precedent
for an opaque handle — `emit_type` would emit an unsatisfiable PHP class hint), so the iterator's whole
state is a byte OFFSET in an `int`: nothing to leak, nothing to close, no `using` required, and a later
swap to a real handle stays non-breaking because none of the mechanism is user-visible.

The native reads ~64 KiB and always stops on a LINE BOUNDARY, extending past the target rather than
truncating, so a line is never split across two reads and a single over-long line still comes back whole.
Terminators are stripped (`\n`, and a preceding `\r`, so CRLF reads like LF); a BLANK line is still a
line. `hasNext`/`next` declare `throws FileSystemError`, because a mid-iteration read failure must not
masquerade as exhaustion. Transpiles via `fgets` — Invariant-14 ladder case 1, no quarantine.

**PERF: a confirmed 4x LOSS against PHP's `fgets` loop, recorded OWED (DEC-365 NO-HIDDEN-LOSS).** The
first working version was 58x slower; two measured fixes took it to 4x (a 14x improvement) —
(1) the chunk split moved from the prelude into Rust, because `List.append` CLONES the list per call and
the prelude's per-line append was O(n²); (2) `List.length` cached in a field, since the hot path called
that native three times per line. The residual 4x is the per-line cost of a phorj-level `Iterator` (two
virtual calls per element) versus PHP's C loop, which no tuning inside this design removes. The G-8
microbench pair (`bench/micro/fslines.{phg,php}`) is added but its official number is OWED here: that
harness needs `php:8.5-cli` under docker and the docker daemon is unavailable in this container. The
numbers above are local, against a debug/ZTS PHP with JIT OFF — which flatters phorj, so the true gap is
≥4x.

### Fixed — a newline inside a string literal inside a closure was destroyed on the PHP leg (2026-07-31)
A live Invariant-1 divergence, found while building DEC-347. The transpiler emitted string literals with
RAW newlines; rendering a closure body on ONE line then turned a newline INSIDE a literal into a space.
`function(): string { return "a\nb\n"; }` printed `a\nb\n` on both Rust backends and `a b ` through PHP.
Nothing caught it because no example had put a newline-bearing literal inside a closure.

Fixed at the literal (`transpile::escapes`): control characters now emit as PHP escapes (`\n`/`\r`/`\t`,
else `\xHH`), so a literal contains no raw newline and NO downstream single-line rendering can corrupt
one — patching the closure emitter alone would not have guaranteed that. `php_escape_bytes` already did
this; the text escapers have been brought up to it. Regression test in `tests/differential.rs`, verified
to fail without the fix.

### Fixed — the tier-1 PHP-function gate scanned comment prose (2026-07-31)
`bareword_calls` skipped string bodies but not COMMENTS, so `word (` in prose was reported as a call —
`terminators (so the caller's …)` in the DEC-347 helper's own comment tripped it, as `lock (` had earlier.
This matters more since DEC-419: a user's `/** … */` doc comment is now emitted into the PHP, so a doc
mentioning `someFunction(x)` in prose would have failed the gate on the user's behalf.

### Added — doc comments cross the PHP boundary in BOTH directions (2026-07-31, DEC-419)
Both sub-questions raised with the doc-comment ruling were answered yes and are built.

`transpile` now re-emits a declaration's `/** … */` as a PHP **docblock**. Since `/** … */` IS PHPDoc
this is a re-emission, not a translation — the star column is re-added around the same body. Comments
produce no output, so the byte-identity spine is untouched.

The **lifter** now reads PHPDoc back into a phorj doc comment. A plain `/* … */` in the PHP source is
deliberately NOT lifted as documentation, mirroring PHP's own convention.

Verified as a fixed point rather than as two isolated features: PHP → phorj → PHP returns the same doc
body at both ends. That fixed point is what choosing PHPDoc's spelling over `///` was for.

The two sides key the doc differently, because they have different information: the transpiler works
from the original phorj source and keys by SPAN (`ast::item_decl_span`), while the lifter has no phorj
spans at all — it works from parsed PHP and keys by declaration NAME (`ast::item_decl_name`). Top-level
names are unique, so the name key is total. Doc comments remain non-AST on both paths.

`emit` (no source) is preserved exactly — the doc-bearing form is opt-in via `emit_with_source`, and a
test asserts the two outputs differ ONLY by the comment lines. The lifter keeps its PHPDoc in a side
channel keyed by token index rather than a new `PTok` variant: a new token would appear at any stream
position and every parser site would have to learn to skip it, with a silent failure mode.

**[Pre-existing limitation, unrelated to docs]** phorj → PHP → phorj is not generally possible:
transpiled output contains fully-qualified names (`\OverflowException` from the checked-arithmetic
helpers) and the lifter's Tier-1 lexer rejects `\`. So the doc round-trip is asserted in the
PHP → phorj → PHP direction, which the lifter's tier supports.

Invariant 13 fallout, all split rather than grown: `ast::item_meta` (new), `transpile::tests_docs`
(new), `lift::printer::{docs,setup}` (new), and a `PParser::new` constructor that removed the
duplicated state literal at both construction sites.

### Added — doc comments: `/** … */` (2026-07-31, DEC-419)
Phorj now has THREE comment forms and no others: `//`, `/* … */`, and `/** … */`. The last is a **DOC
comment** — the documentation of the declaration that follows it — and `phg lsp` renders it on hover
(markdown, under the signature) and as completion `documentation`. A plain `/* … */` above the same
declaration is deliberately NOT documentation; that distinction is the whole point.

`#` remains NOT a comment: a bare `# …` is a lex error, and `#` is only the attribute sigil `#[`. That
divergence from PHP is deliberate — accepting both would force the reader and the lexer to decide which
kind of `#` they are looking at from what follows it.

`/** … */` is PHPDoc's spelling on purpose: phorj transpiles to PHP, where that IS the docblock, so the
same bytes mean the same thing on both sides of the boundary and a lift can read them back. `///` was
rejected — no PHPDoc counterpart, so it would need translating in each direction.

The "is this a doc comment" rule is single-sourced in `token::opens_doc_comment` and shared by the
tokenizer (which picks `CommentKind::Doc`) and the LSP (which extracts the text). Two spellings would
drift invisibly — highlighted as documentation while hover showed nothing. `/**/` stays an ordinary
EMPTY block comment; `/***/` counts as a doc comment with body `*` (a corner recorded as a decision).

Doc comments are NOT AST nodes: hover already holds the buffer text and the declaration's span, so no
field is added to any declaration kind and the backends carry nothing new. Comments of every form stay
invisible to `run`, the VM and the transpiled PHP, so the byte-identity spine is untouched.

Editors: the TextMate grammar gains `comment.block.documentation.phorj`, ordered BEFORE the plain block
rule (TextMate takes the first match, so a `/\*` rule listed first would swallow `/**`). The JetBrains
path loads that same grammar file, so both editors are covered by the one change.

NOT built, and deliberately so — both raised with the ruling, both additive, neither a regression:
transpile does not yet EMIT a doc comment as a PHP docblock, and the lifter does not READ PHPDoc back.

### Added — `FileSystem.tryWithLock(path, fn)`, non-blocking advisory locking (2026-07-31, DEC-348.1)
Returns `Option<T>` — `None` when the lock is held by someone else, `Some(v)` when the closure ran and
returned `v`. Developer-ruled over the cheaper `T?`: under `T?` a busy lock and a closure that
legitimately returns null are the SAME value, and that ambiguity type-checks clean, so it is a trap
rather than a shortcut. Release is the same `using`-based guarantee as `withLock`.

Contention is deterministic to test without a second process or a sleep: the OS lock is per-file-
DESCRIPTOR, so a nested attempt opens its own descriptor and genuinely finds the lock held by its own
program. That is what `examples/fs/lock.phg` and `tests/fs.rs` assert, on all three legs.

### Fixed — prelude injection now runs to a fixed point (2026-07-31, DEC-348.1)
`cli::preludes::inject_core_modules` was a single pass over `CORE_MODULES`, so a prelude's own
`import Core.X` was honoured only when `X` sat LATER in the registry; an earlier row was dropped
SILENTLY, surfacing as `unknown identifier v` on the user's own match arm rather than a missing type.
`Core.Option` is an early row and the FS prelude imports it, which is what surfaced this. The
`ROW-ORDER CRITICAL` comments the fix invalidated are corrected rather than left in place.

### Fixed — LSP completion offered internal natives and hid prelude statics (2026-07-31, DEC-348.1)
`lsp::catalog::module_members` enumerated only `native::registry()`. Because `Core.Native.FileSystem`'s
last dotted segment collides with the friendly class name, `FileSystem.` completion advertised the
INTERNAL lock natives (`lockAcquire`/`lockRelease` — exactly the leak-prone manual API the DEC-348
ruling rejected) while offering neither `withLock` nor `tryWithLock`. `withLock` therefore shipped
invisible to the editor, breaking Invariant 17's 100% bar (DEC-417) the day it landed. Completion now
unions the module's prelude-class PUBLIC statics and excludes the `Core.Native.*` twins; `private`
statics stay hidden. Also closes the deferred "prelude-class members (Date/Uri…)" gap.

### Added — `FileSystem.withLock(path, fn)`, scoped advisory file locking (2026-07-31, DEC-348)
Whole-file advisory locking with the release guaranteed by construction: `withLock` takes the lock, runs
the closure, and releases it on every exit path including a throw. Its body IS a
`using (FileLock guard = …) { return fn()?; }` block, so the guarantee is DEC-364's — which is precisely
why DEC-348 was sequenced after it, and why the `try`/`finally` PHP helper the ruling anticipated did not
need writing (`using` already lowers to a literal `try`/`finally` on the PHP leg).
- No manual `lock`/`unlock` pair exists — the ruling rejected that shape as leak-prone. Byte-range locks
  and timeouts were rejected too (byte-range needs `fcntl`; a timeout would need a spin-sleep bandaid).
- **No new `Op` and no new `Value`:** the OS lock is held by a thread-local slab and the prelude's
  `FileLock` carries an opaque `int` ticket. Tickets start at 1 so `0` can mean *not acquired*.
- Rust and PHP contend on the SAME lock. [Verified: `/proc/locks` reports `FLOCK ADVISORY WRITE`; a Rust
  holder blocks a PHP `LOCK_EX|LOCK_NB` probe and a PHP holder blocks Rust's `try_lock`, both directions
  reproducibly; and end-to-end, with an external `flock(1)` holder BOTH the phorj run and the transpiled
  PHP run block rather than acquiring — asserted by deadline in `tests/fs.rs`, not by sleeping.]
- `tryWithLock` is **not** shipped: the native is built and tested, but its phorj-visible return type is
  user-visible surface awaiting one ruling (Invariant 15).
- `examples/fs/lock.phg` + `examples/README.md` + `FEATURES.md`; `tests/fs.rs` gains the release-on-throw
  case and the cross-process contention proof.
- **[Unverified on Windows]** — verified on Linux only. Windows is a shipped target, its lock semantics
  may be **mandatory** rather than advisory, and there is no Windows CI. Disclosed in `FEATURES.md`, the
  prelude, the example and `src/native/fs_lock.rs`, as the DEC-348 ruling requires.

### Fixed — the `Core.ClosableModule` registry row had to move after its importers (2026-07-31)
The prelude-injection fold walks `CORE_MODULES` **once** and can only inject a LATER row from an earlier
row's imports, so any prelude that imports `Closable` must be positioned before it. `Core.FileSystemModule`
became such an importer (its `FileLock` is `Closable`), which put the row out of order. This fails
QUIETLY — `Closable` is simply never injected and the importing prelude stops compiling — so the row now
documents the constraint rather than leaving the next editor to rediscover it.

### Added — `using`, the scope guard (2026-07-31, DEC-364 / DEC-364.1)
`using (T h = init) { … }` releases `h` on **every** exit path from the block: normal fall-through, a
`return`, a `break`/`continue` out of an enclosing loop, and a throw. `T` must implement
`Core.ClosableModule`'s `Closable` (`close(): void`) — checked at compile time, because nothing probes
for the method at runtime, so the requirement is what makes the emitted call total.
- **No new `Op` and no new `Value`.** All three backends run **ONE** shared lowering
  (`ast::lower_using`) to `{ T h = init; try { … } finally { h.close(); } }`, so byte-identity holds by
  construction rather than by testing, and the PHP leg is a literal `try`/`finally` with no
  `__phorj_*` helper. The declaration sits **outside** the `try`: a fault in `init` means no handle was
  acquired, so there must be nothing to release.
- `using` is **contextual** (DEC-364.1) — the tokenizer gains nothing and no identifier is reserved, so
  `int using = 1;` still compiles. The gate additionally requires the header's `Type name =` shape
  rather than just the following `(`, so `using(1);` stays a *call* to a function named `using` — the
  same discipline `at_discard` documents for itself.
- `Connection implements Closable`, so `using (Connection db = new Connection(dsn)) { … }` closes on
  every exit path. Closes the deferral both `src/ext/database/prelude.rs` and `KNOWN_ISSUES` recorded
  against DEC-203.
- Diagnostics `E-USING-NOT-CLOSABLE`, `E-USING-INFER`, `E-USING-CLOSE-THROWS` (all three explainable via
  `phg explain`). The third exists because interface conformance compares parameters and the return type
  but **not** `throws`: an implementor may declare `close(): void throws IoError`, and that call is
  synthesized into a `finally`, so it must be caught or declared — the rule DEC-257 already applies to a
  throwing iterator's `foreach`.
- `examples/guide/scope-guard.phg` (one exit path at a time + nested guards releasing inner-first),
  `examples/README.md`, `FEATURES.md`, the LSP keyword set, and the shared editor grammar (contextual,
  so `using` highlights only in the header position).
- **NOT lifted, deliberately** — the lifter has no `try`/`catch`/`finally` at all, so this is a
  lifter-wide gap rather than a `using` gap. Recorded as `KNOWN_ISSUES` §LIFT-TRY.

### Fixed — a `break` inside `try` was invisible to the totality engine (2026-07-31) — a soundness hole
`breaks_this_loop` descended into `if` and `block` but **not** `try`/`catch`/`finally` (nor a destructure
`else`), so a `break` that was a loop's only exit could not be seen. Reproduced before the fix:
`function f(): int { while (true) { try { break; } finally { … } } }` type-checked clean (`phg check`
exit 0) and then printed `unit` from a function whose declared return type is `int`, on **both** Rust
legs — an unsound *acceptance*. The predicate is now exhaustive over `Stmt`, and the file it lives in
(`checker/common_flow.rs`) says why a catch-all there is a correctness question, not a style one.

### Fixed — injected-prelude spans collided with user-file offsets (2026-07-31) — an Invariant 1 divergence
The checker records several post-check rewrites in side tables keyed on `Span.start` **alone**
(`ufcs_resolutions`, `html_resolutions`, the reflect/cast substitutions, `for_bind_resolutions`,
`for_iter_lowerings`), justified by "each call site's `(` token is at a unique byte offset". That holds
within one source string — but an injected `Core.*` prelude is a **separate** string whose offsets
restart at 0 and therefore overlap the user file's one-for-one. When a prelude call site and a user call
site landed on the same offset, the prelude's recorded rewrite was applied to the **user's** node:
`phg check` stayed clean, the tree-walker (which re-checks nothing) ran correctly, and only the VM
failed to compile.
- Reproduced and pinned down by length alone: adding one `import` line to the `Core.Database` prelude
  broke `examples/database/transaction-closure.phg` on the VM with "`transaction` is not a function,
  variant, or class" while `check` and `--tree-walker` both passed — and adding a single trailing
  **space** to that same line made it pass again.
- Fixed at the one injection chokepoint: `cli::prelude_spans::lex_parse_injected` rebases each prelude
  fragment's token offsets above `1 << 32`, with per-fragment stride, so an injected offset can never
  equal a user-file offset. `line`/`col` are untouched, so prelude diagnostics still point correctly.
- Ratchet: `injected_prelude_spans_cannot_collide_with_user_file_offsets`.

### Fixed — four more DEC-356 catch-alls the original sweep missed (2026-07-31)
`Stmt::Using` proved them live rather than theoretical: `rewrite_foreach::walk_stmts` and `::lower_stmt`
(so `materialize_for_binds` and the Iterator lowering reach inside a `using` body — Invariant 7),
`lsp::scope::collect_bindings` (the LSP saw neither the `using` binding nor anything declared inside it),
and `inline_parent_ctor::inline_stmt`. All four are now exhaustive over `Stmt`.

### Fixed — the `E-IFACE-VIS` visibility BYPASS (2026-07-30, DEC-379) — a soundness hole
A class could implement a public interface method as **`private`** and still have it reached through a
plain interface-typed receiver. The `overloads == 1` guard meant **any** second overload disabled the
check, so a throwaway `greet(int)` beside a `private greet(string)` switched it off. Reproduced before the
fix: `phg check` said OK and VM, interpreter and transpiled PHP all printed the private method's result.
- `ClassInfo::method_overload_vis` records **per-overload** visibility, index-aligned with the signature
  set, so conformance enforces the visibility of the overload that **conforms** — order-independent, and
  it inherits on both the trait-`use` and class-`extends` paths.
- The per-signature predicate was extracted from `sig_conforms` as `one_sig_conforms` and single-sourced:
  two copies could drift, and the visibility rule would then enforce against a different overload than
  the one conformance blessed.
- `KNOWN_ISSUES F-032` closes — and **two of its claims did not survive reproduction**. It rated this
  "NOT a soundness/security hole" (it was one), and said the PHP leg "fatals at the class declaration"
  (it does not: overloads emit as `m__ovl_N` **with no visibility modifier**, so PHP accepted it too —
  recorded as **CD-28**, since the transpiler still drops per-overload visibility for non-interface
  methods).

### Added — the `__phorj_*` helper classification registry, with a ratchet (2026-07-30, DEC-377)
The rule is *a helper may exist ONLY when PHP cannot do natively what phorj does*, and the audit proving
which helpers comply had been OWED. It is now `src/transpile/helper_buckets.rs`, and **bucket 3
("convenience/DRY only — must be INLINED") is EMPTY**: all 17 candidates from the earlier heuristic pass
are refuted by reading them.
- The `uri_*` trio was suspected of "reimplementing what the target already has". It **already uses** PHP
  8.5's URI extension; what it adds is the exception→sentinel bridge, which needs `try`/`catch` — and
  `try` is not an expression in PHP [verified: `$x = try {…} catch {…};` is a parse error].
- The `text_*` group was called "ASCII-oriented and inlinable". The opposite: they exist because PHP's
  calls are byte-oriented [verified: `trim()` leaves U+00A0/U+2009; `strrev("héllo")` returns mojibake].
- `__phorj_trim` **does not exist** — a phantom from prefix-matching `__phorj_trim_start`.
- **The count was wrong three times** (168 → "149 real" → **165**) and is now asserted by the ratchet, not
  claimed. A first pass here read 158, missing the by-reference `function &__phorj_x()` form.
- The ratchet fails in both directions — unclassified helper, or classified-but-deleted — verified live by
  planting one. Recording a bucket-3 entry is itself a build failure, since bucket 3 means "inline it".

### Fixed — AST rewriters are exhaustive; a catch-all was hiding a compiler PANIC (2026-07-30, DEC-356)
`rewrite_html`'s `leaf => leaf` arm swallowed `Expr::Tuple`, and `erase_tuples` runs AFTER `resolve_html`,
so `var (a, b) = (html"<p>{n}</p>", 1);` left the literal unresolved and **panicked the compiler** with
`unreachable!("html literal not resolved before compilation")`. Valid user code, hard crash. GR-18 was
rated a hygiene item; it was a live P0.
- **Every `Expr`/`Stmt`/`Pattern` total walk is now exhaustive.** Method mattered: each catch-all was
  replaced with a leaf-only or-pattern so **`rustc` enumerated the gaps** rather than trusting the spec's
  (by then decayed) table. Each of seven walkers was missing 4–6 expression-bearing forms; `Tuple` and
  `NamedArg` were missed by all of them.
- More real gaps found the same way: `rewrite_html` skipped **`Item::Test`** (which carries a statement
  body → same panic path for an `html"…"` inside a `test { … }` block); `desugar_di` skipped
  **`Stmt::Destructure`** (which bears an initializer, so `inject<T>()` there was never desugared —
  `desugar_db` walks it correctly three files away, which is what made the gap invisible); and
  `ast::walk`'s two boolean scanners answered `_ => false` for a `StrPart`.
- **Leaf sets single-sourced as macros** (`src/ast/leaves.rs`). A macro expands to an or-pattern, so
  exhaustiveness checking stays fully intact — a `fn is_leaf(&Expr) -> bool` would have reintroduced the
  catch-all by the back door. Verified by hand: adding an `Expr` variant produces `non-exhaustive
  patterns: Expr::ProbeVariant(_, _) not covered` at every fixed site.
- **A named catch-all is worse than `_`** — it compiles cleanly, reads as deliberate, and greps as
  handled. `ast::leaves::tests::no_fixed_rewriter_regrows_a_catch_all` now rejects both, and flags only
  INERT ones (a catch-all that *recurses* is total behaviour, the opposite of the bug). Written before the
  last fixes, it immediately found four more sites.
- Invariant 3's wording widened in `CLAUDE.md` + `docs/INVARIANTS.md` to cover `Expr`/`Stmt`/`Pattern`.
- **Invariant 13 net-negative:** the recursion arms are real code, so six files breached their ceilings —
  all six split by cohesion (each pass's walk trio into its own `*_walk.rs`). `rewrite_ufcs` 503→74,
  `desugar_di/walker` 782→397, `rewrite_generics` 680→252, `resolve_variant_imports` 587→168,
  `desugar_router` 577→238. **Four files fell under the hard cap and their grandfather entries were
  deleted** (67 remain, from 71).
- One exemption, recorded as **CD-27**: `rewrite_ufcs::apply_repl` dispatches on checker-CONSTRUCTED
  replacement shapes, not user AST.

### Fixed — an `html"…"` literal now counts as a use of `import Core.Html` (2026-07-30)
`var a = html"<p>{n}</p>";` under `import Core.Html;` reported `E-UNUSED-IMPORT` — *"nothing in this file
references `Html` (remove the import, or use it)"* — while **removing** the import reported
`E-HTML-IMPORT`: *"`html"…"` requires the Core.Html module"*. Two diagnostics instructing opposite
actions, and no way to write the program in that shape; the only form that compiled was an explicit
`Html a = …` annotation, which happens to spell the type name.
- Cause: the import-hygiene scan is textual and case-sensitive, so the lowercase literal prefix `html"`
  never matched the whole word `Html`. An import that GATES a literal is used by that literal.
- Keyed on the module leaf lowercased + `"`, so a future `xml"…"` / `sql"…"` sugar following the same
  convention needs no second special case. `examples/guide/html.phg` gained the `var` form, so the
  differential glob keeps it working — byte-identical on all three legs.

### Changed — `walk.rs`'s pattern-binding collector has named no-op arms (2026-07-30, DEC-356 partial)
`collect_pattern_bindings`'s `_ => {}` sat one line beneath a comment recording that this exact bug had
already fired once (a missed pattern form drops struct-bound names from `free_vars`, miscompiling a lambda
that captures one). It now lists every binding-free form **by name** — deliberately not `unreachable!()`,
since those forms are perfectly reachable and simply contribute no bindings.
- Re-measuring first showed the ruling's own prediction — *"D alone decays"* — had already come true
  **before D shipped**: **26** named catch-alls in `src/checker/`, not the 17 the spec recorded. The
  remaining 26 rewriter sites stay open with the per-walker inventory now written down.
- `walk.rs` was 812 lines (62% over the hard cap), so its inline test module split out to `walk_tests.rs`
  — reducing the debt instead of squeezing comments to hold a grandfathered ceiling.

### Fixed — every fault body is single-sourced, and the differential harness now DERIVES from it (2026-07-30, DEC-361)
A fault body is parity-affecting: Invariant 1 demands identical *failure* behaviour across `phg run`,
`--tree-walker` and the transpiled PHP. Invariant 4 already single-sourced the arithmetic bodies; every
other one had been re-typed at **38 sites**. `src/value/faults.rs` is now the one home — it re-exports the
arithmetic consts (which stay next to the kernels that raise them) and defines the rest, with the
payload-carrying bodies as functions (`panic_with`, `assert_with`, `no_field`, `no_enum_case`) so the
message *shape* cannot be re-typed either.
- The scale was the finding: `"stack overflow"` at five VM sites, two closure sites, three interpreter
  sites **and a second `pub const` in the JIT** — whose own comment said the body was "not yet
  single-sourced", i.e. the code documented the defect and shipped anyway. `FaultMsg::message()`, the
  thing three call sites already treated as the single source, was re-typing all six of its own bodies.
- **`classify` in `tests/differential.rs` now derives its needles from those consts.** It previously kept
  independent copies of all twelve bodies, so the test whose entire job is catching fault-body drift was
  the thing HIDING it — which is why the ruling rejected single-sourcing alone. Two ratchets keep it: no
  body may appear as a literal outside its definition, and no `pub const FAULT_*` may go unclassified.
- **The predicted drift had already happened, in TWO places.** The PHP leg's non-exhaustive-`match` fault
  threw a bare `\UnhandledMatchError` — whose `getMessage()` is the **empty string** — on the
  `instanceof` path, and PHP's own `"Unhandled match case true"` on the native-`match` path, against
  `"non-exhaustive match at runtime"` on both Rust legs. Fixed in both lowerings: `throw` is an
  expression in PHP 8, so a `default => throw new \UnhandledMatchError("<canonical>")` arm carries the
  right body while keeping the native `match` form. `examples/transpile/demo.php` regenerated (a one-line
  diff); all three legs still produce byte-identical stdout, since the arm is checker-unreachable.
- Classification graded up too: `NonExhaustiveMatch` is its own `FaultKind` so a future drift can't hide
  behind `Panic`, and the four arithmetic bodies that had **no arm at all** got theirs — they were falling
  through to `Other(full_string)`, which compares the VM's `at L:C:` prefix and so read a real agreement
  as a divergence.

### Fixed — `Statement` binds are now EXECUTION-scoped, and the nested-savepoint SQL is MySQL-portable (2026-07-30, DEC-351)
Two halves of one ruling. **(A) Bind lifecycle.** Binds accumulated and never reset, so reusing a prepared
statement died on the second iteration with `2 bound value(s) but 1 ? placeholder(s) in the SQL` — the exact
reuse `Core.Database` promises. `DbStmt::take_binds()` now takes them, resetting the accumulator, at all
four execution sites (`query`/`stream`/`exec`/`execReturningId`) — **before** the driver call, so a failed
execution cannot leave stale binds behind. Positional and named binds share the one accumulator and so
behave identically.
- The quadratic path went with it, **measured**: 8000 named binds through one statement **4.469s → 0.054s**,
  against the report's own re-prepare baseline of 0.059s on the same box. The reuse path now sits *at* that
  baseline — the cliff is gone, not reduced.
- This also makes params execution-scoped on BOTH legs, which collapses the case-1 step-2 PHP statement
  wrapper to `[PDOStatement, sql, params[], nextIndex]` with nothing to carry between executions.

**(B) The D5 fold-in — savepoint SQL portability.** The nested path emitted a bare `RELEASE <name>` (a
**MySQL syntax error**; the `SAVEPOINT` keyword is mandatory there) and a `;`-joined `ROLLBACK TO x;
RELEASE x` **pair** in one string (MySQL's `query_drop` runs one statement, and `DriverConn::control` is
single-statement by contract). Both were invisible because SQLite's `execute_batch`, Postgres's
`batch_execute` and PDO's `exec` all tolerate them — while the module's own `mysql.rs` already spelled the
forms correctly, so the code contradicted itself.
- Fixed by single-sourcing the vocabulary in `src/ext/database/natives/savepoint.rs` (the discipline
  Invariant 4 applies to value kernels): only the three-dialect intersection is emitted — `SAVEPOINT n`,
  `RELEASE SAVEPOINT n`, `ROLLBACK TO SAVEPOINT n`. A full unwind is genuinely two statements on all three
  backends (rolling back to a savepoint never pops it), so it is two `control` calls, and two `exec` calls
  on the PHP leg.
- A **source-scan ratchet** over every file that can emit control SQL (all of `natives/` +
  `transpile/db_php.rs`) now rejects a bare `RELEASE`, a bare `ROLLBACK TO`, and any `;`-joined pair. It was
  written first and watched fail on the unfixed tree with three findings at the exact lines.
- New coverage: nested `commit`/`rollback`/`rollbackAll` round-trips for MySQL and Postgres (env-gated on
  `PHORJ_MYSQL_TEST_DSN` / `PHORJ_PG_TEST_DSN`, skip-loud — no server is reachable in the build container,
  so that half is recorded as a stated gap, not as passed), and a PHP-leg test that reaches the
  `RELEASE SAVEPOINT` branch **no test had ever executed**: every prior case committed at depth 1, i.e. the
  real `commit()`, which is exactly why the bare spelling survived review.

### Fixed — **Invariant-1 breach**: a throwable overriding a `final` PHP method died only on the PHP leg (2026-07-30, DEC-367)
`class CustomError implements Error` defining `getMessage()` type-checked clean, ran fine on both Rust
backends, and died at PHP runtime with `Fatal error: Cannot override final method
Exception::getMessage()` — because a throwable transpiles to a class extending `Exception`, which marks
seven methods `final`. Now `E-FINAL-PARENT-METHOD` at check time, one diagnostic at the declaration.
- The seven names came from **reflection against php-8.5.8**, not memory: `getMessage`, `getCode`,
  `getFile`, `getLine`, `getTrace`, `getPrevious`, `getTraceAsString`.
- `__construct` and `__toString` are **not** final, so a throwable keeps its own constructor and
  `#[ToString]`; a class that does not implement `Error` is unaffected entirely. Both pinned by tests,
  because over-rejecting here would make `Error` subclasses unusable.
- Renaming on emission stays rejected (the ruling): it would keep the program running while silently
  diverging from the source, and break anything catching it as a PHP `Exception`.
- A `declare class` is exempt: it DESCRIBES an existing PHP class, so declaring a signature for a method
  final over there is correct (that is how `examples/interop/` binds PHP's own `DivisionByZeroError`). The
  first version of the guard over-rejected exactly that, and the pre-push gate blocked the commit before it
  landed; a regression test now pins it.
- The method counterpart of DEC-202's `E-RESERVED-NAME`, which guarded colliding class names but could not
  reach methods.

### Added — the `DatabaseResult` protocol on the PHP leg (2026-07-30, DEC-340 case-1 step 2, partial)
`__phorj_db_try` / `__phorj_db_try_unit` produce DEC-329.3's `DatabaseResult_Ok`/`_Err` — the shape the
phorj prelude matches on — with the Err payload carrying step 1's `<<Kind>>` tag, which is the join between
the two steps. Only `PDOException` is caught: a `TypeError` or a bug in an emitted expression is not a
database error, and laundering it into `DatabaseResult.Err` would let a real defect be caught as a database
problem. 4 tests against real PDO.

**Step 2 is otherwise STOPPED, and my estimate of it was wrong.** I called it "~20 emitters, mechanical".
It is not: 3 emitters are outright placeholders (`query` emits `->execute()` where it must return a list of
row handles), and most others emit the bare receiver because the Rust natives mutate a shared handle. Worse,
phorj's `Statement` accumulates binds on ONE shared handle by design (a DEC-266 allocation lever) and PDO
has no equivalent model — `bindValue` needs a 1-based index — so the PHP twin needs its own parameter
accumulator and `prepare` must return a wrapper object. That wrapper's shape governs every other emitter, so
it is a design decision, not a wrapping exercise. Recorded in the spec with a recommended shape; it wants a
ruling before it is written.

### Added — the PHP-leg SQLSTATE→kind classifier (2026-07-29, DEC-340 case-1 slice, step 1 of 3)
Developer ruled the database transaction surface should reach Ladder case 1. Step 1 is the error contract,
which is what everything else depends on: `__phorj_db_classify(PDOException)` maps a real PDO exception
onto the same 7-kind taxonomy the Rust drivers produce, tagged with the same `<<Kind>>` marker the phorj
prelude parses. Since the prelude is phorj source it already runs on the PHP leg, so this tagging is the
whole of what would make `catch (UniqueViolationError e)` work there — and what stops
`db.transaction(fn, retries)` from silently never retrying, since it retries only the transient class.
- Verified against **real PDO exceptions**, not synthesised codes — which found a second real defect:
  keying "unique" on SQLite's driver code **19** mis-classified a **NOT NULL** violation as a unique
  violation. 19 is the generic `SQLITE_CONSTRAINT`, and the extended codes the Rust driver keys on (2067
  `_UNIQUE`, 1555 `_PRIMARYKEY`) are **not exposed through PDO's `errorInfo`**, so on SQLite the message is
  the only discriminator. MySQL's 1062 genuinely is unique-specific and stays.
- An unmatched error stays **untagged** on purpose — the prelude maps that to the base `DatabaseError`,
  exactly as an unmatched Rust error does. A drift guard asserts every kind the Rust side tags is still
  reachable here, so adding one there without adding it here fails a test instead of silently degrading.
- `Core.Database` remains Ladder case 2 for now. Steps 2 and 3 — `DatabaseResult` construction across ~20
  `php:` emitters, and the `decimal` mapping (PDO yields float where phorj is exact) — are scoped in
  `docs/archive/specs/2026-07-26-transaction-depth-semantics.md`; the quarantine flips after those.

### Fixed — the savepoint helpers printed PHP-8.5 deprecation notices onto stdout (2026-07-29, DEC-340)
`SplObjectStorage::contains()` is deprecated as of PHP 8.5 — which is the transpile floor — so every depth
read emitted a notice to **stdout**. Had the database leg been made live with that in place it would have
broken byte-identity outright, in the subtlest possible way. Now `offsetExists()`.
Found by `tests/db_savepoints.rs`, a new test that runs the transpiler's own helper source under real
`php` + PDO/SQLite: nested begin/rollback composes, `unwind_to` restores a caller-owned entry depth,
`rollback_all` flattens every level, the depth counter is per-handle rather than global, and the savepoint
names still match `ops_tx.rs`. The source is read from the transpiler, so the test cannot pass against a
stale copy of the helpers. Reading the code would not have caught this.

### Security — **P1**: HTTP response splitting / request smuggling via response headers (2026-07-29, DEC-363)
`Response.withHeader` and `Cookie` interpolated both arguments straight into a header line with zero
validation, and `respond_once` returns handler bytes verbatim — so nothing downstream could re-validate.
Reproduced on a shipped `phg serve` surface before fixing: a CRLF-carrying value produced a response whose
`Content-Length: 2` still described the 2-byte body while ~30 further bytes followed — an injected header,
an early head terminator and a second body. That is a smuggling/desync primitive, not just splitting.
- **CR, LF and NUL are now rejected in any header value, and `:` in any header name**, with the wording
  ``header `X-User` contains a forbidden character`` — mirroring the request-side gate exactly.
- The **policy lives in phorj** (`HeaderSafety` in the `Core.Http` prelude), so all three legs share one
  definition of "forbidden" by construction. Verified: `run`, `run --tree-walker` and the transpiled PHP
  all fault with the identical message.
- Guarded on the **`Cookie` constructor**, so all three of its string fields and all four builders
  (`path`/`secure`/`httpOnly`/`partitioned`) are covered by one chokepoint — each re-constructs.
- Guarded at the **builder** rather than `serialize()`: both are safe, but this names the `withHeader`
  call that produced the bad value instead of surfacing at respond time.
- **The request side was widened to NUL** in the same change (it rejected CR/LF only) so the two
  directions cannot drift; PHP's own `header()` rejects NUL, and it is a header-truncation trick.
- **`HeaderSafety.isValidName` / `isValidValue` ship as public pre-checks.** A violation is a 500 by
  design (`serve/handlers.rs` degrades request faults, so this is not a DoS vector) — these let a handler
  holding user-derived input return a clean 400 instead, without making the builders throw.
- 9 tests: one per injectable surface on both Rust backends, a NUL case, a builder-smuggling case, and a
  clean-response regression guard asserting the head does not split.
- Naming deviation, recorded not silent: the ruling spelled the helpers `Http.isValidHeaderName`. That
  needs a `class Http` inside module `Core.Http`, recreating exactly the leaf-equals-type namesake that
  DEC-278's `Module` suffix existed to avoid and DEC-350 has just dissolved. They ship on `HeaderSafety`
  instead; the final spelling is the developer's call.

### Changed — **BREAKING**: `Core.DatabaseModule.Database` → `Core.Database.Connection` (2026-07-29, DEC-350)
The type is provably ONE connection — a single `Box<dyn DriverConn>` with connection-scoped
`tx_depth`/`hook`/`timeout_ms`, and no pooling anywhere — so `Connection` is what it is. 8 of 10
ecosystems call this `Connection`; `Database`/`DB` is what Go and Laravel use for the pool/manager phorj
does not have. And DEC-278's `Module` suffix existed *only* because the module leaf and the type were
namesakes, so renaming the type dissolves its rationale and the module goes bare.
- `import Core.DatabaseModule;` → `import Core.Database;`
- `Database db = new Database(dsn)` → `Connection db = new Connection(dsn)`
- Unchanged on purpose: `DatabaseError`, `DatabaseResult`, and the raw `Core.Native.Database` module.
  The error type is not the connection, and the native namespace keeps its leaf.
- Breaking rename across every DB example, test and doc — cheap now, expensive once users exist.

### Fixed — two stale surfaces the rename's own guards caught
- `src/ext/registry.rs`'s `uri` row still advertised "the deprecated `Core.Url` compat twins", which
  DEC-416 deleted. The row and the regenerated `docs/EXTENSIONS.md` no longer claim a surface that is
  gone.
- The generated `docs/EXTENSIONS.md` is regenerated, so its `database` row names `Core.Database`.

### Fixed — **P1 data loss**: transaction auto-rollback unwound only ONE level (2026-07-29, DEC-340)
`db.transaction(fn)` called rollback exactly once on the throw path, and rollback unwinds a single level.
So a `begin()` leaked anywhere inside the closure — including inside a helper it called — consumed that
one rollback and left the transaction's **own** level open with its writes live. A later unrelated
`commit()` then made them permanent, *after* the error handler had been told the transaction rolled back.
- Reproduced before fixing: a row starting at `bal = 100` read **999** immediately after the "rolled
  back" transaction and **999** again after a later commit. Both now read `100`.
- Auto-rollback unwinds to the depth recorded on **ENTRY** — "restore the depth I found" — not to 0.
  Unwinding to 0 was explicitly rejected: it would roll back a **caller-owned** outer transaction
  (`db.begin(); db.transaction(fn)` where `fn` throws), destroying work the call was never given
  authority over. Verified: with a caller-owned outer transaction, depth reads 1 before and 1 after, and
  the caller's own write survives and commits.
- Both failure paths use it — the throw path and the commit-failed path. Rollback errors are still
  discarded there so they can never mask the original throw.
- **`rollbackAll()`** added for the manual `begin`/`commit` path, where the caller does own the outermost
  level. **`transactionDepth()`** added because the depth was previously unobservable from phorj — the
  native returned it and the prelude discarded the payload, which is a large part of why this survived.
- `examples/database/transaction-closure.phg` gains the leaked-`begin` case; 4 new tests in
  `tests/database.rs` run every scenario on both backends.

### Added — the `__phorj_db_*` savepoint helpers, staged (DEC-340, PHP leg)
`src/transpile/db_php.rs`: a full savepoint helper set — per-PDO-handle depth counter in a
`SplObjectStorage` (mirroring the Rust `Rc<Cell<u32>>` sharing), with `begin`/`commit`/`rollback`
composing via `SAVEPOINT phorj_sp_N` using **the same savepoint names the Rust legs emit**. The three
`php:` emitters were repointed at it, replacing a `->beginTransaction()` mapping that could not express
phorj's nesting at all, since PDO's own `beginTransaction()` does not nest.
**It is not reachable yet, and that is not a claim it works:** `Core.DatabaseModule` is deliberately
quarantined by `E-TRANSPILE-DB` (Ladder case 2), so the emitters never run. Making the leg live means
lifting that quarantine — a case-2 → case-1 move for the whole module, which Invariants 14 and 16 leave
to the developer. Analysis and the open question are recorded in
`docs/archive/specs/2026-07-26-transaction-depth-semantics.md`.

### Fixed — **P0**: block-scope shadowing broke byte-identity on the PHP leg (2026-07-29, DEC-339)
Phorj has block scope; PHP does not. A declaration reusing the name of a live local or parameter meant
two different things on the two legs — the Rust backends made a new binding while the transpiled PHP
wrote through to the OUTER variable. Ten declaration forms could do it, and one of them, a nested `for`
reusing its counter name, silently changed the **iteration count**. A `catch` binding leaked an exception
dump, a stack trace and an absolute path into the PHP leg's output.
- Now `E-SHADOW-LOCAL`: a declaration is rejected when its name is already bound by a live local or
  parameter **in the same function** — same scope or an enclosing one. Enforced in the **checker**, the
  one pipeline every surface shares, so `run`, `--tree-walker`, `transpile`, `build --php`, the LSP, the
  formatter and the test runner all get it from one place. A transpiler-only guard would have let
  `phg run` accept a program that cannot transpile.
- The diagnostic anchors at the offending declaration and its hint names the line of the binding it
  collides with, so it is actionable rather than just a refusal.
- **The accepted half is enforced just as hard**, because this rule removes capability: sibling blocks
  reusing a name (even at a different type), sequential `for` loops reusing the counter, loop bodies
  declaring per iteration, sibling `match` arms and sibling binding-`if`s, a **lambda parameter**
  shadowing an outer local (a lambda starts a new function), nested lambda params, and a method local
  sharing a field's name — all still legal. `examples/guide/shadowing.phg` demonstrates every one and is
  byte-identical across all three legs.
- **Flow narrowing is not shadowing.** `if (x is int) { … }` installs a synthesized narrowed binding; the
  author wrote no second declaration, so it goes through a separate `declare_narrowed` path. Without
  that carve-out the rule made narrowing reject itself — caught by 8 existing tests.
- Migration cost was measured up front (DEC-412) as exactly one in-tree site, and that held:
  `examples/guide/math.phg` declared `int l1` then `float l1`. Renamed; output unchanged.
- All 23 rows of the ruled matrix are pinned by `src/checker/tests/shadowing.rs` — 14 rejected, 9
  accepted. `phg explain E-SHADOW-LOCAL` summarises both halves; the fault class is recorded in
  `examples/README.md` per Invariant 9's carve-out for non-runnable faults.

### Changed — Invariant 13 debt paid down while fixing the above
`check_lambda` extracted to `src/checker/expr/lambda.rs` (literals.rs 641 → 488) and `collect_enum` to
`src/checker/collect/enums.rs` (types_decls.rs 773 → 597), rather than growing two grandfathered files.
A lambda earning its own module is not arbitrary: it is a function boundary, which is precisely why
DEC-339 needs it.

### Added — userland `#[Deprecated(message: "…")]` (2026-07-29, DEC-417)
Mark your own API as deprecated. Provider is `Core.Runtime.Deprecated`, import-gated like
`#[Entry]`/`#[Config]`. Both halves of the ruling ship: the DECLARATION is tagged (struck through in
completion and the outline) and every USE is reported with `W-DEPRECATED` carrying your message, tagged
`DiagnosticTag.Deprecated` so editors strike the call through. It never gates — warning channel only.
- **Compile-time only.** PHP 8.5 has a native `#[\Deprecated]`, but it fires at RUNTIME onto stdout,
  which would break the byte-identity spine, so the mark is erased before every backend. Verified: the
  emitted PHP contains zero deprecation markers and all three legs match on the shipped example.
- **The mark does not spread.** A function calling a deprecated function does not become deprecated —
  matching Rust/Kotlin/Swift/C# (C# actively does the inverse). Pinned by a test.
- An overload set warns only when EVERY signature is deprecated, so a set with a live overload stays
  quiet instead of crying wolf.
- Rejected loudly, not dropped: an interpolated `message:` (no runtime exists to evaluate the holes) and
  a positional argument. Both `E-DEPRECATED-MESSAGE`, with a `phg explain` entry.
- Ships `examples/guide/deprecated.phg` + README row; the VS Code grammar already highlights it (its
  attribute rule is generic, verified rather than assumed).

### Fixed — every warning in the language said "error" (2026-07-29)
`Display for Diagnostic` hardcoded the severity word, so warnings rendered as
`warning: type error at 3:9: …`. The headline is now severity-aware and the doubled prefix is gone.
Found while building `W-DEPRECATED`, where "error" is exactly the wrong word for something that
compiles and runs fine.

### Known gap recorded — the PHP lifter is blind to all PHP 8 attributes (`KNOWN_ISSUES` LIFT-ATTR)
`src/lift/lexer.rs` treats `#` as a line comment, so `#[...]` is swallowed whole. This predates
DEC-417 and was found by actually testing the lift direction. It means Invariant 17's lift leg could
not be closed for `#[Deprecated]`; queued as its own slice rather than half-done here.

### Removed — every pre-1.0 deprecation affordance (2026-07-29, DEC-416)
Developer ruling: before the first stable release there are no users and nobody to migrate, so phorj does
not deprecate. Retiring something means changing it outright, recording the decision, teaching the
compiler **only** the new form, and updating the examples in the same change. A retired name is an
**unknown** name and gets the ordinary hard error.
- **The `Core.Url` compat twin is deleted** — `src/ext/uri/url_compat.rs` had kept the entire retired
  module registered as a parallel row-set (its own comment promised removal "after the deprecation
  window"). `import Core.Url;` is now a plain unknown-import error. Its `native::deprecation_of` rows
  went with it.
- **Three retired-but-still-explained diagnostics deleted**: `E-MULTIPLE-MAIN`, `E-DB-NAMING-NOT-CONST`
  (DEC-258), `E-TRANSPILE-FS` (DEC-313). Each arm's whole body was an announcement of its own
  retirement. The first of those was added earlier the same day and this ruling reverses it.
- **`phg vendor` is gone, not aliased** — the bespoke "retired, use add/install/update/remove" error was
  threaded through four sites including its own `phg help vendor` topic. It is now an unknown command.
- **`docs/DEPRECATION.md` kept but scoped**: a header records that its Live→Deprecated→Removed lifecycle
  applies from 1.0 onward, and that pre-1.0 follows this ruling instead.
- `W-DEPRECATED` itself **stays** — it is being repurposed as a userland facility driven by an explicit
  `#[Deprecated(message: "…")]` attribute on an author's own API, rather than by an internal stdlib
  table. The provider package is still open.
- The test asserting the old behaviour was **inverted**, not deleted:
  `retired_core_url_import_is_simply_unknown_not_deprecated` now pins that the retired import is a hard
  error and emits no `W-DEPRECATED`.

### Docs — SSOT reconciliation after Wave 0 (2026-07-29)
- **`MASTER-PLAN` and `SLICE-STATE` had diverged** on the unruled count — the roadmap said *"seven items
  stay deliberately unruled (L-19/22/25/28/31/33/86)"* while the slice cursor said four, because
  L-19/28/31 were ruled on 2026-07-29 as DEC-392/393/391. The slice cursor was right; the roadmap is
  corrected. Invariant 19 forbids exactly this fork, so it is called out rather than quietly patched.
- `SLICE-STATE` now reads **"Wave 0 is COMPLETE, next is Wave 1.1 (DEC-339)"** with a per-row evidence
  table; the stale *"NEXT: WAVE 0 … everything else is unbuilt"* header is marked superseded in place.
- **Two stale name-magic claims fixed** (the DEC-415 follow-through): `loader::fs::validate_public_surface`'s
  doc-comment said a *"non-`main` file"* and *"an entry file (declares `main`)"* — directly contradicting
  the code beneath it, which keys on `#[Entry]`; and `UNIFIED-SPEC` said *"a file declaring the entry point
  `main` is fully exempt"*. Both now say the exemption comes from the **attribute**.
- **New PENDING question recorded, not ruled**: that layout exemption tests `EntryRole::Cli` only, so a file
  whose only entry is `EntryKind.Web` is not exempt. Verified by reading the validator and its single call
  site; no shipped example trips it, so it is latent rather than a live defect. Being user-visible language
  behaviour, it is the developer's call (Invariant 15) and is queued to Wave 4.4 where DEC-345 already
  touches these validators.

### Security — re-ported the git argument/transport hardening the package manager had lost (2026-07-29, Q28 / DEC-414)
- `src/pm/fetch.rs` passed a dependency's `url` and `ref` straight to `git clone`/`git checkout`. Both
  come from a `phorj.json` spec, i.e. from whatever repository a user is asked to `phg install` — and
  git's `ext::` remote helper **runs a shell command**, so `git = "ext::sh -c 'curl … | sh'"` was
  arbitrary code execution at install time. A leading `-` on either field made it a git flag instead of
  a value (`--upload-pack=…`). The retired `phg vendor` path had all of these guards as verified
  property **P6**; the DEC-316 rewrite did not inherit them.
- Now: `validate_git_target` refuses `ext::`/`file::` (the double-colon REMOTE-HELPER forms,
  case-insensitively), a leading `-` on url or ref, and empty values — **before** any process spawns.
  `clone` passes `--` to end option parsing. Every invocation carries
  `-c protocol.ext.allow=never` as defence in depth, and the inherited `GIT_*` environment is scrubbed
  so an ambient `GIT_SSH_COMMAND`/`GIT_CONFIG_*`/`GIT_PROXY_COMMAND` cannot hijack the fetch.
- `file://` (the transport) and bare local paths keep working — they are documented as supported and a
  regression test pins six legitimate forms.
- 6 tests, and each of the five rejection tests was verified to **FAIL with the guard neutered**, so
  they detect the gap rather than merely passing. `--` is deliberately NOT added to `checkout`:
  `git checkout -- <x>` means "restore this path", so it would change the verb's meaning.

### Removed — the dead NAME-based entry resolver, and the false guarantee it underpinned (2026-07-29, DEC-415)
- Entry points are **attribute-declared only**: a free function or a static method is an entry ONLY if
  attributed `#[Entry(kind: EntryKind.…)]`. The name `main` carries no meaning (developer ruling).
- `ast::entry_point()` and `ast::entry_point_count()` — which resolved an entry by the magic names
  `main`/`handle` — are DELETED. They had **zero callers**; the `#[Entry(kind:)]` migration
  (DEC-331/DEC-337) had already replaced them with `entry_candidates`/`entry_for`.
- Their doc-comments were the source of a claim repeated in three backends — *"the checker's
  `E-MULTIPLE-MAIN` guarantees ≤1"* — that was **false**: `E-MULTIPLE-MAIN` has no emit site, and a
  program with both a top-level `main` and a class-static `main` type-checked clean. All three comments
  now cite the rule that is actually enforced.
- **The live rule is at most one entry PER KIND** (`E-DUPLICATE-ENTRY-KIND`): one `EntryKind.Cli` plus
  one `EntryKind.Web` may coexist, and `run`/`serve` each take their own — five shipped examples depend
  on that, so a flat one-entry-per-program rule would have broken them. No behaviour change here: the
  rule was already implemented and correctly named; this change removes the dead code and the stale
  claims around it.
- `phg explain E-MULTIPLE-MAIN` now says the code is retired and points at `E-DUPLICATE-ENTRY-KIND`,
  so an old build log quoting it still explains itself. The `guide/class-main.phg` header and its
  `examples/README.md` row no longer teach name-based entries.

### Added — `pre-push` documentation guards (2026-07-29, DEC-362 BUILT)
- `scripts/doc-guards.sh` — four mechanical checks against the defect class the GR-24 sweep measured as
  the project's dominant one. **G1** every `src/….rs` path named in tracked markdown exists · **G2**
  every `DEC-nnn` mentioned has a row in the decision register · **G3** a commit SHA in `docs/plans/`
  carries a ref or a subject, never bare · **G4** every diagnostic code named in the register exists in
  `src/` (the DEC-362 extension — this is the check that would have caught `E-RETIRED-FORIN`, the dead
  `E-MULTIPLE-MAIN` and Invariant 14's phantom `--sequential-concurrency`, all three found by hand).
- **G2 is HARD from day one** because it had only three violations, and they were FIXED rather than
  grandfathered: `DEC-186` (grouped member imports), `DEC-197` (bare-import leaves, superseded by
  DEC-274) and `DEC-200` (closed by DEC-202) were referenced across the docs with no register row at
  all. Rows backfilled from surviving references and labelled as reconstructions.
- **G1/G3/G4 are ratcheted** against `scripts/doc-guards-baseline.txt` (142 entries frozen: 71 dangling
  paths, 49 bare SHAs, 22 unimplemented codes), following the `size-baseline.txt` precedent — a hard
  failure on day one would have been un-landable. G4's baseline doubles as the list of diagnostic codes
  the register promises but `src/` does not implement (DEC-360's `W-UNUSED-*`, DEC-370's
  `E-TRANSPILE-PARALLEL-NO-PHP`, …). Prefix fragments and bare stems are dropped, not frozen.
- Every guard was verified to DETECT before being trusted: G1 on a planted non-existent path, G2 on a
  planted
  nonexistent DEC id, G4 on a planted register row claiming a phantom code ships. (Writing a literal
  fake id here would itself trip G2 — which is how this entry got caught on its own first push.) Restoring the tree
  returns the gate to OK.

### Fixed — `microbench-gate` blocked every push in the dev container instead of skipping (2026-07-29, DEC-365)
- `scripts/microbench-gate.sh` gated on `command -v docker`, which tests for the docker **binary**, not
  a reachable **daemon**. The remote dev container ships the client with no daemon, so the skip never
  fired, the harness ran, failed to connect, and returned setup-error **2** — which ABORTS the push.
  That is the inverse of DEC-365's rule (*an unmeasurable bench is a LOUD SKIP with an OWED verdict,
  never a block and never a pass*). Now probes the daemon with `docker version` and skips loudly,
  naming the verdict as OWED. Reproduced (exit 2) and verified fixed (exit 0 + the loud skip).
  This is what forced `--no-verify` on every push of the 2026-07-29 ruling session.

### Added — `pre-commit` docs-only fast path + a mechanically-enforced no-concurrent-commits lock (2026-07-29, DEC-378)
- **Docs-only fast path:** if nothing staged is `*.rs`, `*.phg`, `Cargo.toml` or `Cargo.lock`, the hook
  runs `cargo fmt --check` only and skips `phg format --check` (which needs a build) and the Rust test
  tier (which cannot test a markdown change). Measured motivation: the 2026-07-26 session spent ~45 min
  on twelve docs-only ruling commits and the 2026-07-29 session paid the full 1999-test tier on eight
  more. Routing verified behaviourally on four cases: `.md` alone → fast path; `.md`+`.rs`, `.md`+`.phg`,
  `.md`+`Cargo.toml` → full tier.
- **No-concurrent-commits lock:** DEC-378's second half was a *stated* rule, which cannot stop a
  backgrounded commit — and a race between two hook runs (shared `target/`, plus the `phg` binary the
  cli tests spawn) was the only test failure of the 2026-07-26 session. The hook now takes an exclusive
  `flock` and aborts with an explanation if another run holds it. Verified: a second holder is refused.

### Changed — Invariant-13 debt cleanup: 13 oversized files M-Decomp'd to <300 (2026-07-25)
- Split 13 pre-existing size-gate breaches (parser/checker/cli/ast/lift/loader/transpile — sizes 504–2066)
  into ~90 cohesive submodules, every resulting file <300 lines. Pure code movement / data reorg — no
  behavioural change; `scripts/size-baseline.txt` was NOT touched (real splits, not baseline bumps). The
  transpiler's `Transpiler` struct shrank via a `HelperGates` sub-struct (the ~65 `uses_*` gate flags →
  `gates: HelperGates` in `src/transpile/gates.rs`); byte-identity preserved (differential green). `phg
  explain` output is byte-identical (arms moved verbatim). Full ALL-FEATURES gate + `scripts/size-gate.sh`
  green. (Remaining grandfathered-at-baseline files >300 are the standing Inv-13 burn-down, not breaches.)

### Changed — nativized `Request.parse` (targets the `queryparse` perf loss) (2026-07-25, DEC-338, DEC-268-certified)
- The whole wire→`Request` parse is now one Rust native `Core.Native.Http.parseRequest(bytes): Request?`
  (`src/native/http/request.rs`) with a byte-identical PHP transpile twin `__phorj_http_parse_request`
  (`src/transpile/runtime_php_http.rs`). The `Core.Http` prelude `Request.parse` now delegates
  (`return NativeHttp.parseRequest(raw);`); its former private helpers `headerPairs`/`cookiePairs`/
  `multipartFields`/`boundaryOf` were removed. Purely internal — `Request.parse`'s signature and
  observable behaviour are unchanged (null = malformed/oversize, the eager D8a contract; never a fault),
  so byte-identity holds VM ≡ tree-walker ≡ php-8.5.8 — the 3-leg gate is the differential (`rich_request`
  tests + the `examples/web/rich_request.phg` glob), with the native's graph additionally pinned by the fast
  `parse_request_*` unit tests. Perf DIRECTION in-container: `queryparse` ~0.10× → **~0.88×** (~9× faster,
  near-parity but still <1.0× — NOT yet a WIN by WIN-OR-FLAG; whether it crosses 1.0× is certified on the
  dev-box docker harness, estimate 0.8–1.5×).
- The entry role is now a QUALIFIED, import-gated enum variant — `import Core.Runtime.EntryKind;` then
  `#[Entry(kind: EntryKind.Cli)]` / `#[Entry(kind: EntryKind.Web)]`. Previously `kind: Cli` was a bare
  magic identifier "in the wind" (no import, unresolved); it is now `E-INJECTED-VARIANT-BARE`, consistent
  with every other injected variant (`Option.Some`, …). An unimported `EntryKind.Cli` is `E-UNIMPORTED`;
  the fully-qualified `Core.Runtime.EntryKind.Cli` is self-gating (no import), mirroring `#[Core.Runtime.Entry]`.
  Reserved kinds (Desktop/Mobile/Worker/Embedded) are real variants (`E-ENTRY-KIND-RESERVED`). `EntryKind`
  is a compile-time-only marker (Inv 5) — erased before every backend, so byte-identity is unchanged.
  All ~340 shipped examples + the transpiler/lifter/formatter/LSP updated in the same change (Inv 17).

### Added — Q-A wildcard & group imports (2026-07-25, DEC-268-certified)
- `import Pkg.*;` binds every PUBLIC member of a package at once (cross-package); `import Pkg.{ A, B as C };`
  group form (DEC-186); `import Pkg.* except { X };` drops names; an explicit import wins over a wildcard.
  Pure compile-time sugar — the loader expands to sorted per-symbol imports before any backend (Inv 5,
  byte-identical). New diagnostics: `E-WILDCARD-STDLIB-ROOT`, `E-WILDCARD-EMPTY`, `E-EXCEPT-UNKNOWN`,
  `E-WILDCARD-ALIAS`, `E-IMPORT-AMBIGUOUS`, `E-IMPORT-UNKNOWN`. Example `examples/project/wildcard-imports/`.

### Added — Q-B visibility model completeness (2026-07-25, DEC-268-certified)
- Package HIERARCHY (dotted-prefix ancestor relation); top-level `internal` REDEFINED to "this package +
  descendant packages" (subtree); member `internal` added (fields/methods/consts/statics/constructor +
  constructor-promoted params), checker-enforced via the package derived from mangled names, erasing to
  PHP `public` (byte-identical). Static-field visibility (G4) confirmed enforced. Example
  `examples/project/member-internal/`. Pending dev ruling: P-Q-B-1 (overloaded interface-method vis).

### Fixed — LSP dotted import-path completion inserted a duplicated prefix
- Typing `import Core.` and accepting `Core.Output` produced `Core.Core.Output` (`.` is a client word
  boundary; items carried only a `label`). Import items now carry a `textEdit` replacing the whole typed
  path. Also fixed a latent LSP catalog bug listing non-promoted ctor params as completable members.

### Added — DEC-331 slice 2: Rich `Request` v1 (bags + files + body.json), replacing the thin Core.Http Request

The stdlib `Request` is now the PSR-7-shaped rich value: `method`/`path` (percent-decoded) plus
`query`/`headers` (case-INSENSITIVE)/`cookies` (first-`=` split)/`form` (urlencoded + multipart
fields)/`files` (`UploadedFile` w/ 256 KiB temp-spill behind deterministic handles)/`attributes`
(the ONE mutable bag — middleware scratch + route params; `Router.handle` writes them, `param()`
delegates) and `body` (`bytes()`/`text()`/memoized `json(): Json?`). Bags share
`get`/`getOrDefault`/`getAll`/`has` — repeated keys are FIRST-wins (parameter-pollution-safe).
Eager `Request.parse(bytes)` returns null on malformed/oversize (the serve bridge 400s);
`Request.fake(method, target)` + `withHeader`/`withCookie`/`withBody` rebuild through the SAME
parse from the original raw pieces (CR/LF in a header faults — no injection primitive). Wire
parsing runs in new std-only `Core.Native.Http` natives shared by both engines; the PHP leg gets
the mirrored `__phorj_http_*` helpers — byte-identical on interp, VM, and real PHP 8.5
(differential example + 3-leg conformance golden + CRLF fault-parity + native unit tests).
Deviations recorded in spec §8 (`RequestBody`/`getOrDefault` naming forced by language rules;
lazy mode + `RequestParsing` ship with slice 3's ServeConfig; body cap inert under serve;
`queryparse` bench HARD-FLAGGED ~8x loss → the flip-all-losses campaign). `Core.Json`'s registry
row moved after `Core.Http` (forward-fold transitivity); `VirtualModule.src` became multi-fragment
`srcs` (Inv-13 prelude split).

### Added — DEC-331 slice 1: `#[Invoke]` + `#[ToString]` — attribute-designated conventional methods

No magic method names — callability and stringification ride an attribute on an ordinary method.
`#[Invoke]` makes a class instance callable as `x(args)` (overloaded `#[Invoke]` methods dispatch by
arity/type; the methods stay directly callable by name). `#[ToString]` designates the ONE method a
class stringifies through — run automatically in string interpolation (`"{obj}"`) AND by
`Conversion.toString(obj)` (one stringification story). The checker records span-keyed decisions and
a new OUTERMOST pass `resolve_invoke_tostring` rewrites them to ordinary (overloaded) method calls on
the live post-fill AST — including inside field initializers — so `interpreter ≡ VM ≡ transpiled PHP`
by construction, with zero backend changes on the call paths. The transpiler emits a native
delegating PHP `__toString`; the lifter maps PHP `__toString` → `#[ToString] toString`. Guards:
`E-ATTRIBUTE-TARGET`, `E-TOSTRING-SIGNATURE`, `E-TOSTRING-DUPLICATE`, `E-INVOKE-DUPLICATE`,
`E-NO-TOSTRING`, `E-NOT-CALLABLE`, `E-INVOKE-DEFAULTS` (exact-arity resolution — default/variadic
params on `#[Invoke]` are slice-1b), all with `phg explain` entries; roles inherit with the method
(class + trait). Example `examples/guide/invoke-tostring.phg`. Deferred to slice 1b: function-type
assignability, PHP `__invoke` emission + the multi-invoke dispatch shim, lift `__invoke`.

### Added — DEC-336: extensionless `#!…phg` shebang sources light up in the editors

An executable phorj source with no extension and a `#!/usr/bin/env phg` first line (the tokenizer
already skips the shebang; `phg run ./bin/console` already works — DEC-282) is now recognized by the
editors: the VS Code extension's `phorj` language gains a `firstLine` match (`^#!.*\bphg\b`) so such
files get full language-server intelligence (the client selects by language id, not a `*.phg` glob),
a TextMate shebang highlight rule, and PhpStorm/LSP4IJ README guidance for extensionless entries.
vscode extension `0.4.0` → `0.5.0`.

### Fixed — `Reflect.className` on an enum variant: PHP leg ≠ interpreter (DEC-329.3 fallout)

`Reflect.className(variant)` returned the enum-scoped PHP class name (`Color_Green`) on the
transpiled PHP leg while the interpreter/VM return the bare variant name (`Green`) — a byte-identity
divergence introduced when DEC-329.3 made PHP variant classes enum-scoped, and surfaced by gating
against a real PHP 8.5.8 oracle. The `__phorj_class_name` helper now maps a scoped variant-class leaf
back to its bare variant (built from `variant_fields`; keys unique per the `E-TRANSPILE-VARIANT-COLLISION`
guard); regular class instances fall through `get_class` unchanged. Regression-locked by
`examples/guide/reflect.phg` (differential, 3-leg). The reflect helper was M-Decomp-moved from
`runtime_php.rs` into `runtime_tables.rs` beside the other reflect emitters (Invariant 13).

### Added — DEC-320 v1: `phg build --php` — transpile INTO a live PHP app (the TS→JS playbook)

`phg build <entry> --php` emits a `.php` SIBLING per type-declaring `.phg` (PSR-4 paths, so humans
find them where the autoloader would) plus ONE shared `_phorj/runtime.php`: the `__phorj_*` helpers
the project actually uses, the injected preludes, every free function (PHP never autoloads
functions), the runtime-static initializer (runs at include time), and a generated classmap
autoloader covering every sibling class — including an enum's several classes per file, which
plain PSR-4 cannot address. The host app's ONLY wiring is one composer `files` entry (printed as a
diff; phg never edits composer.json). Rebuilds are idempotent (content-compare skip). No `#[Entry]`
bootstrap is emitted — the host owns the lifecycle; `\Main\main()` stays a plain callable.
Host-parity gated: the split output under a composer-style host `php` is byte-identical to
`phg run`. `phg stubs` / `phg watch` are the recorded v2 slices; the `phpInterop` namespace-prefix
knob is deferred as a PENDING adjudication (v1 keeps package path = namespace).

### Fixed — DEC-329.3 (commits A + B1): variant names shared by two enums resolve to their OWNING enum

Pre-fix, every backend resolved a variant use through a bare-name map (last-declaration-wins): with
`enum A { Dup(int) }` and `enum B { Dup(string) }`, even a *qualified* `new A.Dup(7)` could
construct a value carrying ty `B` (the checker's owner pick was HashMap-iteration nondeterministic,
and `unwrap_new` erased the qualifier before any backend saw it), and `A.Dup(..)` patterns matched
`B.Dup` values by name. Now: (A) the checker resolves owners deterministically, rejects a BARE use
of a shared name with `E-VARIANT-AMBIGUOUS`, and records every resolved use in a span-keyed
side-table; (B1) the new `qualify_variants` post-check pass rewrites every construction/pattern to
its canonical enum-qualified form, and all three backends key on it — interpreter construction +
pattern-qualifier test, VM `VariantIndex` + `Op::MatchTag` now testing **(ty, variant)**, and a
transpiler qualified-construction intercept. The duck-typed `?` keeps NAME-only `Failure` matching
via the new `Op::MatchTagName` (a user Result-shaped enum beside injected `Core.Result` still
propagates correctly; the JIT declines it fail-closed when the name is shared). On the PHP leg (commit B2, the
DEC-329 ruling's deliverable) variant classes are now enum-SCOPED — `Shape.Circle` ⇒
`final class Shape_Circle extends Shape` — lifting the `E-TRANSPILE-VARIANT-COLLISION` refusal for
shared variant names entirely (it now covers only the pathological composed-name collision, e.g.
`class Shape_Circle` beside `enum Shape { Circle }`). Scoping subsumes the old reserved-word
variant mangle (`Int`→`Int_` is now `Tok_Int`); the injected enums' helper surfaces
(Json/Option/Result/FileSystemResult/Level/RoundingMode PHP helpers) reference the scoped classes;
program stdout is byte-identical (the DEC-238 debug rows render `Enum.Variant(…)` from the class
map). Duck-typed `?` covers every `Failure`-owning enum with a sorted `instanceof` chain. One-time
golden regen (`examples/transpile/demo.php`); new `examples/guide/shared-variant-names.phg` is
3-leg-gated by the differential.

### Added — DEC-302: backed enums (PHP 8.1 parity) — scalar-valued enums

`enum Suit: string { Hearts = "H", … }` / `enum Priority: int { Low = 1, … }` — an enum whose
every variant carries an `int`/`string` scalar. `s.value` reads the backing (an int-backed
`.value` is a first-class arithmetic operand); `Enum.cases()` lists every variant in declaration
order (`List<Enum>`, also valid on any plain payload-less enum); `Enum.from(x)` maps a value → its
variant (faults on no match); `Enum.tryFrom(x)` returns `Enum?` (null on no match). Representation
**B** (dev-ruled): the uniform abstract-base-class + `final class` per-variant model, extended with
a `value` property + static `cases()`/`from()`/`tryFrom()` emitted on the base — NOT a native PHP
`enum` (one representation, consistent lift, no generic special-case). The VM gains `Op::EnumValue`
+ `Op::EnumFrom` (`cases()` inlines to `MakeEnum×N + MakeList`); the from-miss fault body is
single-sourced in `value::enum_from_miss` for run≡runvm parity. PHP→Phorj lift now maps a backed
PHP enum to a backed Phorj enum. 11 new coded diagnostics validate backing/variant/value shape.
Byte-identical across interpreter, VM, and real PHP; `examples/guide/enums-backed.phg` +
differential coverage. Repr (A) native-PHP-enum path rejected (two representations, no generics).

### Added — DEC-273 wave 3: the woven four migrate; the preludes monolith keeps dissolving

`db` (the whole multi-driver tree — sqlite/mysql/postgres driver files colocated), `mail`,
`http-client`, and `session` (new default-tier `session` feature — `Core.SessionModule` +
`Core.Native.Session` are now cleanly gateable) move to `src/ext/<name>/`, and their FOUR
prelude sources (`DB_PRELUDE`, `MAIL_PRELUDE`, `HTTP_CLIENT_PRELUDE`, `SESSION_PRELUDE`) leave
`cli/preludes.rs` for colocated `prelude.rs` files (the unconditional-`#[path]` dissolution
pattern). 16 of the 23 registry rows are now physically migrated; the playground gains the
`session` feature for parity.

### Added — DEC-273 wave 2: seven more extensions migrate; the preludes monolith starts dissolving

`json`, `uri` (kernel + `Core.Native.Uri` natives + the deprecated `Core.Url` compat twins +
its PRELUDE source, all colocated), `path`, `hash`, `decimal` (the MODULE natives — the `1.50d`
primitive stays kernel), `test`, and `debug` (its DebugModule prelude colocated too; the
walk-any-value introspection SEAM stays core) move to `src/ext/<name>/` behind seven new
dep-free Default-tier features. The `DEBUG_PRELUDE`/`URI_PRELUDE` consts leave `cli/preludes.rs`
(the dissolution pattern: unconditional `#[path]` prelude modules inside the extension folders).
The playground's feature parity is RESTORED (wave 1 had silently cost it Ini/Csv/Encoding —
its `default-features = false` dependency now re-adds every dep-free Default extension).
12 of the ruled extensions are now physically migrated; `phg extensions` lists 22 rows
(2 mandatory + 16 default + 4 opt-in).

### Added — DEC-273 wave 1: the extension architecture lands (registry + 5 migrations)

The minimal-core/extension model gets its physical seam. `src/ext/registry.rs` is THE
one-row-per-extension list — the compiler's disabled-import gate, the new `phg extensions
[--docs]` listing, and the generated `docs/EXTENSIONS.md` (sync-tested, build-independent) all
read it. Importing a module whose extension is compiled out is now `E-EXTENSION-DISABLED`,
naming the extension AND the cargo flag to add (supersedes `E-MODULE-UNAVAILABLE`; the old
gated-module table is retired — a new gated module is just its extension row, and the previously
UN-gated `Core.Regex`/`Core.Cryptography`/`Core.Ini`/`Core.Csv`/`Core.Encoding` imports now fail
cleanly on reduced builds instead of cascading). FIVE extensions physically migrated to the
AMENDMENT-2 `src/ext/<name>/` layout (natives + tests colocated; regex's prelude source
colocates too): `ini` (the pilot), `crypto`, `regex`, `csv`, `encoding` — `ini`/`csv`/`encoding`
gained new default-tier cargo features; `signals` got its registry row; `green` and `db-all`
are documented non-rows (core seam / feature group). Tier heads recorded per the ruling:
`transpile`/`lift` open the MANDATORY tier (their structural move ships with a later wave).
Extensions keep the `Core.` import root — zero source churn. Also ruled in-wave: the `jit`
registry row stays (jit remains CORE-classified; the row documents its build flag for
discoverability), and `phg build` standalone artifacts — which carry and use the building phg's
JIT (measured ~110× on hot pure loops) — now honor `PHG_NO_JIT=1` as the byte-identical pure-VM
escape hatch (env, not argv: the artifact's argv belongs to the embedded program). Certified by
the full DEC-268 panel (3 evidence-based lenses; security lens closed the feature-gate-bypass question by tracing
every pipeline entry point; all correctness/completeness findings fixed in-wave).


### Changed — DEC-282: the unified, manifest-less loader ("autoload"), CLI + web (BREAKING)

`phorj.toml`, `manifest.rs`, and the network-touching `phg vendor` subcommand are RETIRED — one
loading rule everywhere, zero config. **App root** = the nearest ancestor of the entry containing
`src/` (or `vendor/`), found git-style by walking up; with neither, the entry's own directory.
**Three ordered search roots**: the entry's directory (entry-local packages, `bin/Commands/…`) →
`src/` (shared code; package names strip `src/` — `src/Model/Article.phg` ⇒ `package Model;`) →
`vendor/` (offline deps, `vendor/<Publisher>/<Name>/` folder = package; the compiler NEVER
touches the network — a package-manager extension writes the tree). First match wins;
`W-SHADOWED` names both paths when a later root also holds the package. Loading is
**import-driven and declaration-indexed** (lazy): only packages the entry's import graph reaches
are ever read — unreached or broken files are inert. Import hygiene is Go-maximal (all HARD):
`E-MODULE-NOT-FOUND` (lists the searched roots), `E-IMPORT-MAIN` (`import Main;` was silently
accepted!), `E-DUP-IMPORT`, `E-UNUSED-IMPORT` (whole-word scan; a comment mention counts — never
mis-flags). **Executable entries**: a byte-0 `#!/usr/bin/env phg` shebang is skipped and bare
`phg <existing-file> [args…]` dispatches to run — `chmod +x bin/console && ./bin/console migrate`
works, argv reaching the `#[Entry]`. **Web site mode**: `phg serve <dir>` — docroot =
`dir/public` (the ONLY web surface; static assets with a ~20-type MIME table, `ETag`/
`Last-Modified`/304, canonicalize+prefix traversal guards, `.phg` bytes never served, no
dot-files/listings; `W-PHG-IN-DOCROOT` flags stray source), entry = `dir/public/index.phg`;
`phg serve <file>` stays the handler-only dev mode. **LSP (DEC-252)**: editor diagnostics now run
the SAME loader (buffer text for the open file, sibling packages from disk) — cross-file imports
no longer squiggle. Old loose-mode's "only `package Main`" restriction is lifted; the 11
`examples/project/*` trees dropped their tomls (withdeps' vendor tree moved to the folder=package
layout).

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
it can be PROVEN gone: a new fail-closed interval pass (`src/jit/range_acc/`) analyzes a
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

## [1.0.0-nightly.0] - 2026-06-24

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
