# PHP-Fidelity & Divergence Audit

> Goal (developer, 2026-06-25): go through **every** way Phorge diverges from PHP **with no solid
> reason** and challenge each one; and ensure **both** directions (Phorge→PHP transpile AND
> PHP→Phorge lift) use **real, idiomatic language features** of the target. Flagship: the transpiler
> emits PHP *concatenation* where PHP *interpolation* exists — "this shows we don't master PHP."
> Process: detect all findings, then walk them **one-by-one via AskUserQuestion**, challenging each,
> showing a running **Finding i / N** counter.

## Philosophy anchor
Phorge : PHP :: TypeScript : JavaScript. **Familiarity-first**; PHP is the FLOOR not the ceiling;
remove *surprises* never *capability*. A divergence from BOTH PHP and TS without a capability/safety
reason is a suspected wart (often a Rust-ism leaking from the implementation language into the surface).

## Categories
- **A** — Phorge surface-syntax divergence from PHP/TS with no capability reason.
- **B** — Transpile fidelity: Phorge→PHP emits non-idiomatic PHP where a native PHP ≤8.5 feature exists.
- **C** — Lift fidelity: PHP→Phorge over-rejects or emits non-idiomatic Phorge.

## Decision states
`pending` (not yet reviewed) · `ADOPT` (agreed to change) · `KEEP` (challenged, justified, no change) ·
`DEFER` (agreed but scheduled later).

---

## Findings register

### A-1 — return type `->` vs `:`  [VERIFIED · pending]
- **Phorge now:** `function f() -> T` (Rust-style arrow). Also used for method/lambda returns and
  function types `(int) -> int`.
- **PHP / TS:** both `function f(): T` (colon). `->` is *also* PHP's member-access operator → false friend.
- **Proposed:** declared returns → `: T` (PHP+TS); function *types* → `=> T` (TS fat-arrow, since `:`
  can't express a callable type). `:` return slot is free (only other `:` use is struct-destructure
  field rename inside `{}`, a disjoint position). [Verified: parser/stmts.rs:142, patterns.rs:84]
- **Justified?** NO — pure Rust-ism, zero capability reason; transpiler already emits PHP `:`.
- **Severity:** high (every function/method).

### B-1 — string interpolation → concatenation  [VERIFIED · pending]
- **PHP emitted now:** Phorge `"Hello, {w}!"` → PHP `"Hello, " . $w . "!"` (concat); `println`'s
  newline is concatenated too (`. "\n"`). [Verified: `phg transpile`]
- **PHP idiomatic:** `"Hello, {$w}!"` (PHP supports `{$var}` interpolation natively; works under `php -n`).
- **Reducible?** Yes — emit a PHP interpolated string from a Phorge interpolated string literal.
- **Severity:** high (every interpolated string — extremely visible, the developer's flagship).

<!-- discovery agents (A/B/C sweeps) will append A-2.., B-2.., C-1.. here -->

### B-2 — `Console.println` newline concatenated  [VERIFIED · pending]
- **PHP now:** `echo "Hello, Phorge!" . "\n";` (newline appended via concat). [src/native/mod.rs ~254]
- **PHP idiomatic:** `echo "Hello, Phorge!\n";` (embed `\n`). Cleanest fix rides on B-1 (build one
  interpolated/embedded string).
- **Reducible?** Yes. **Severity:** high visibility (every `println`), but cosmetic (output identical).

### B-9 — redundant `$` escaping in string *literals*  [VERIFIED · pending · LOW]
- **PHP now:** literal parts run through `php_escape` which escapes `$` even when no interpolation
  follows. [src/transpile/expr.rs ~347]
- **Idiomatic:** could skip `$`-escaping where no valid identifier/`{` follows. Output is *correct*,
  just over-escaped — no behavior change. **Severity:** low (cosmetic, regression-safe as-is).

### B-10 — ~~`intdiv`/`fmod` missing `\` in namespaced code~~  [REJECTED · false positive]
- Claimed critical bug. **Verified false:** PHP auto-falls-back unqualified *function* calls to the
  global namespace, so bare `intdiv`/`fmod` in `namespace Acme\Util` resolve correctly (`php -n` →
  `intdiv(9,5)=1`). Confirmed by `tempconv`'s green namespaced integer-division oracle. No action.

> Agent B also confirmed already-idiomatic / irreducible (no action): native `match`/ternary/`clone`/
> `?? throw` (Track 1), `array_values(array_filter)` (key reindex), `__phorge_float` (Ryū, irreducible),
> `__phorge_range` (PHP `range()` descends — semantic mismatch), `array_reduce` arg-order, `\xHH` bytes.

### A-6 — `in` iteration keyword (false friend)  [pending · HIGH]
- **Phorge now:** `for (string w in list)` (iterates values). [examples/*.phg]
- **PHP:** `foreach ($xs as $x)` (`as`). **TS:** `for (const x of arr)` (`of`). **JS `for…in` iterates
  KEYS** → Phorge's `in` is a genuine false friend for the TS/JS-familiar audience.
- **Justified?** NO — naming choice, no capability reason. Candidates: `as` (PHP) or `of` (TS).
- **Severity:** high (every loop).

### A-3 — parameter syntax: type-first, no sigil  [pending · MEDIUM]
- **Phorge now:** `function f(int age, string name)`. **PHP:** `(int $age, string $name)`.
  **TS:** `(age: int, name: string)`.
- **Justified?** PARTIAL — type-first + no sigil is unambiguous and Go/Rust-like, but matches neither
  model language's *order*. Dropping `$` aligns with TS (A-4, considered justified). Debatable: order.
- **Severity:** medium (every signature). *Note: deeply coupled to A-1/A-4 — decide as a set.*

### A-7 — interpolation delimiter `{w}` vs `{$w}`/`${w}`  [pending · LOW]
- **Phorge now:** `"hello {w}"`. **PHP:** `"hello {$w}"` / `"$w"`. **TS:** `` `hello ${w}` ``.
- **Justified?** PARTIAL — `{w}` is clean (sigil-free, consistent with A-4) but matches neither exactly.
  Relevant to the B-1 transpile fix (the source delimiter maps to PHP `{$w}`).
- **Severity:** low (dialect choice; consistent internally).

### A-46 — `++`/`--` statement-only (no expression form)  [pending · LOW]
- **Phorge now:** `x++;` legal as a statement; `y = x++` rejected. PHP/TS allow both.
- **Justified?** YES (removes side-effect-in-expression footgun) — but it *is* a divergence; confirm KEEP.
- **Severity:** low.

### A-50 — single string form (all interpolate)  [pending · LOW]
- **Phorge now:** only `"…"` (always interpolation-capable); no `'…'` literal form. PHP uses `'…'` for
  non-interpolated. **Justified?** PARTIAL (grammar simplicity) — confirm KEEP or add `'…'`.
- **Severity:** low.

> Agent A confirmed ~50 surface forms as **justified or identical** to modern PHP 8.x / TS (no action,
> but the developer may still challenge any): `$`-removal (A-4), `+` string-concat type-directed (A-5),
> `T?` suffix optional (A-8), match-no-parens (A-9), `fn(…)=>` lambda (A-10), function-type `(int)->int`
> (A-11 — note: rides on A-1's `=>` decision), `List<T>`/`Map`/`Set` generics (A-12), `constructor`
> (A-13), `implements`/`extends` (A-14), payload enums (A-15), visibility + promotion (A-16/A-17),
> `mutable` (A-18), `open`/final-default (A-19), `package`/`import` Go-modules (A-20/A-21),
> `Module.fn()` calls (A-21), ranges `..`/`..=` (A-22), `??`/`?.` (A-23), `opt!` (A-24), if-let (A-25),
> match guards (A-26), `&&`/`||`-only (A-27), bitwise (A-28), `**` (A-29), pipe `|>` (A-30),
> typed catch (A-31), `throws` (A-32), `var` inference (A-34), list/struct destructuring (A-35/A-36),
> unions/intersections (A-39/A-40), `instanceof` (A-41), bare `this` (A-42), contextual kw (A-43),
> erased generics (A-44), `??=` (A-45), `+=` family (A-47), `[…]`/`[k=>v]` literals (A-49), raw/html/byte
> string literals (A-51/A-52/A-53), `void`/`Empty`/`never` (A-54/A-55/A-56), static/override (A-59/A-60).

### C-1 — lift over-rejects string interpolation  [pending · HIGH]
- **Lift now:** rejects PHP `"Hello, $name"` / `"{$name}"` as `"string interpolation is Tier-2"`
  [parser.rs:881] — even though **Phorge supports interpolation** `"{name}"`. The lift lexer already
  detects it (`InterpStr`); only the parser gates it. Mirror of B-1 on the ↑ side.
- **Target:** map PHP interpolation → Phorge `"{name}"`. **Severity:** high (very common PHP).

### C-45 — lift emits untyped return silently  [pending · MEDIUM]
- **Lift now:** a PHP fn with no `: T` → Phorge fn with `ret: None` [lifter.rs:125]; parses but FAILS
  Phorge's checker downstream (Tier-1 requires explicit returns). Silent invalid output.
- **Target:** reject loudly (Tier-2) or warn. **Severity:** medium (produces non-compiling Phorge).

### C-46 — lift doesn't handle `instanceof`  [pending · MEDIUM]
- **Lift now:** PHP `$x instanceof C` not lexed/lifted — Phorge SUPPORTS `instanceof` (M-RT S1).
  Straightforward Tier-1 add. **Severity:** medium (feature gap).

### C-47 — lift doesn't handle bitwise operators  [pending · MEDIUM]
- **Lift now:** `& | ^ ~ << >>` rejected as unsupported chars [lexer.rs:349] — Phorge SUPPORTS them
  (primitives sweep). Tier-1 add (lexer+parser+lifter). **Severity:** medium (feature gap).

### C-5/C-6 — printer over-parenthesizes / unary as pseudo-call  [pending · LOW]
- **Lift now:** prints `("Hello, " + name)` (full parens) and `!(cond)` (unary as `op(x)`)
  [printer.rs:472-478]. Correct + re-parse-safe, but non-idiomatic; this is the visible lifted output.
- **Target:** precedence-aware printing (minimal parens), prefix unary `!cond`. **Severity:** low.

### C-38 — ~~multiple `implements` silently fails~~  [REJECTED · false positive]
- Agent had stale tier info. **Verified false:** `class C implements A, B` runs → "12" on run/runvm
  (M-RT S2/S6). No action.

> Agent C confirmed CORRECT rejections / mappings (no action): elvis `?:` (truthiness ≠ `??` null —
> a *good* rejection), foreach/array (need inference), non-literal match arms, enum methods, backed
> enums, default params, instance-field defaults, assign-as-subexpr, casts, array-append, include/
> require, bare `clone`, try/catch (Tier-2), declare/global/goto; and faithful mappings for `?->`→`?.`,
> `?T`→`T?`, `__construct`→`constructor`, promotion, `readonly`→immutable, `===`→`==`, `$this`→`this`,
> `::`→`.`, `echo`→`Console.print`+auto-import, top-level→`main()`, visibility/abstract/static.

---

## Review queue (the curated, decision-worthy findings)
14 findings worth a per-item ADOPT/KEEP/DEFER decision (the ~50 justified-A + ~40 correct-C items and
the 2 rejected B-10/C-38 are NOT in the queue — challenge any from the appendices if desired).

| # | ID | Finding | Sev | Theme |
|---|----|---------|-----|-------|
| 1 | A-1 | return `->` → `:` (+ fn-types `=>`) | HIGH | syntax |
| 2 | A-6 | `in` iteration keyword (false friend) → `as`/`of` | HIGH | syntax |
| 3 | A-3 | param type-first / no sigil | MED | syntax |
| 4 | B-1 | transpile interpolation → concat | HIGH | interpolation |
| 5 | B-2 | transpile `println` newline concatenated | HIGH-vis | interpolation |
| 6 | C-1 | lift over-rejects interpolation | HIGH | interpolation |
| 7 | A-7 | interpolation delimiter `{w}` vs `{$w}`/`${w}` | LOW | interpolation |
| 8 | C-46 | lift: add `instanceof` | MED | lift gap |
| 9 | C-47 | lift: add bitwise ops | MED | lift gap |
| 10 | C-45 | lift: untyped return emitted silently | MED | lift gap |
| 11 | C-5/6 | lift printer over-parenthesizes | LOW | lift output |
| 12 | A-46 | `++`/`--` statement-only | LOW | syntax |
| 13 | A-50 | single string form (no `'…'`) | LOW | syntax |
| 14 | B-9 | transpile over-escapes `$` in literals | LOW | transpile |

---

## Progress
- Discovered: **3 sweeps complete** (A/B/C). Verified findings: **14** queued + **2 rejected** (B-10, C-38).
- Reviewed with developer: **0 / 14**.
- Decisions: ADOPT · KEEP · DEFER per finding (logged below as we go).

## Decisions Log
<!-- per-finding decisions appended here as the walkthrough proceeds -->
- [2026-06-25] AGREED: run a full PHP-fidelity audit; walk findings one-by-one via AskUserQuestion with
  a running counter. Two categories of divergence (syntax) + fidelity (transpile/lift).
- [2026-06-25] **A-1 = ADOPT.** Declared returns → `: T` (PHP+TS); function *types* → `(int) => int`
  (TS fat-arrow); `->` is **fully retired**. Typed lambdas become TS-identical: expr body
  `fn(int x): string => "{x}"`, block body `fn(int x): string { … }`. Clarified vs developer's riders:
  (a) lambda body is introduced by `=>`/`{}`, never `->`; (b) mandatory-return is ALREADY enforced
  (`E-MISSING-RETURN`, totality cluster) and is INDEPENDENT of this syntactic change. Milestone-sized
  breaking codemod (all .phg/tests/docs/lift-printer/playground). Status: decided, not yet implemented.
- [2026-06-25] **A-6 = ADOPT (iteration redesign).** Replace `for (x in coll)` with PHP-identical
  **`foreach (coll as BINDING)`** (collection-first); free `for` for C-style `for(;;)` only. ONE keyword
  (`as`) — `of`/`in` rejected as synonyms ("different not better"). Four binding forms (additive, one
  keyword): value `as T v`; key/value-as-index `as K k => V v` (unifies map-key + list-index +
  set/range-position, beats PHP's arrays-only); element destructure `as Point { x, y }` / `as [int a,
  int b]` (PHP can't); ranges `0..n as int i`. **Optional position counter** `… with int i` — loop-owned,
  read-only, 0-based; useful mainly alongside a map/set key binding (lists already index via `k =>`).
  Challenge logged: an auto-counter solves *position* counting only, NOT *conditional* counting
  (count-matches still needs a normal `mutable int n`); custom start/step = YAGNI (use a range).
  Status: decided (counter shape pending final confirm), not yet implemented.
- [2026-06-25] **A-6 counter = ADOPT `with int i`.** Optional trailing clause `foreach (… with int i)`;
  `i` is loop-owned, read-only, 0-based position. `with` chosen over `;` (avoids the `for(;;)` visual
  collision; consistent with `clone … with`). Conditional counting still uses a normal `mutable int n`.

- [2026-06-25] **A-3 = KEEP.** Type-first params `(int name)` stay: it's PHP-minus-sigil (PHP is
  type-first), internally consistent with Phorge locals (`string x`) and fields (`int x`), and pairs
  with the PHP-style `:` return (`(int name): string` ≡ PHP without `$`). Rejected full TS name-first
  flip (`name: int`) — bigger codemod, trades PHP-closeness; the modern-consensus readability argument
  was weighed and lost to PHP-familiarity + zero churn.

- [2026-06-25] **B-1 = ADOPT per-hole.** Transpiler emits native PHP `"{$…}"` interpolation for every
  *variable-rooted* hole (var / `->prop` / `[idx]` / `->method()` chains), concatenating only holes PHP
  can't interpolate (operators, free-fn calls). **Requirement (developer):** the hole-kind
  classification must be EXHAUSTIVE and byte-identity-gated — every `Expr` shape explicitly bucketed
  interpolatable-vs-concat, with differential tests covering each (a misclassification = wrong PHP).

- [2026-06-25] **B-2 = ADOPT `echo X, "\n"`.** `println(x)` → `echo <expr>, "\n";` (comma-list, one
  universal rule, byte-identical output); `print(x)` → `echo <expr>;`. **`printf` rejected** for println:
  literal `%` would be misread as a format spec (corruption risk), more verbose, not more readable —
  `printf` is reserved for a possible future `Console.printf`/format-string feature only.

- [2026-06-25] **C-1 = ADOPT (faithful subset).** Lift maps PHP interpolation → Phorge `"{…}"` for
  var/`->prop`/`->method()`/`[index]` chains; keeps the loud Tier-2 rejection for the risky tail
  (legacy `"${name}"` — deprecated PHP 8.2 — and complex holes). Honors lift's never-guess contract;
  round-trip-gated per the B-1 coverage discipline. ("Try everything" rejected: a silent wrong guess is
  worse than a loud rejection for a migration tool.)

- [2026-06-25] **A-7 = KEEP `{w}`.** Sigil-free delimiter stays — coherent with no-sigil-everywhere
  (A-4). Escaping cost (`\{` for a literal brace) is mitigated: `\{` works (verified) and brace-heavy
  literals use raw `r"…"` (verified, no interpolation). `${w}` (TS, less escaping) and `{$w}` (PHP)
  both rejected — reintroduce/retain the `$` sigil. Zero churn (already shipped). Three tools: `{w}`
  hole · `\{` literal · `r"…"` raw.

- [2026-06-25] **C-46 = ADOPT.** Lift translates PHP `$x instanceof C` → Phorge's existing
  `x instanceof C` (M-RT S1); dynamic-RHS `instanceof $var` rejected loudly. (Clarified: Phorge already
  HAS `instanceof`; this is purely lift coverage.)
- [2026-06-25] **A-61 (new) = KEEP `instanceof`.** Rename to camelCase `instanceOf` REJECTED — every
  reference language (PHP/JS/TS/Java) uses lowercase `instanceof`, and all Phorge keywords are
  lowercase; `instanceOf` would diverge from the universal convention and be the lone camelCase keyword.

- [2026-06-25] **C-47 = ADOPT.** Add bitwise `& | ^ ~ << >>` to lift (lexer + parser at PHP precedence
  + 1:1 mapping to Phorge's existing bitwise ops).

- [2026-06-25] **C-45 = ADOPT (void-or-reject).** Lift: PHP fn with no return hint + no value-returning
  `return` → emit `: void` (provable from body, not a guess); has a value `return` but no type → reject
  loudly (Tier-2). Fixes today's silent non-compiling output.

- [2026-06-25] **C-5/6 = ADOPT.** Lift printer goes precedence-aware: minimal parens (only where the
  child precedence requires) + prefix unary `!cond`/`-x`, replacing full parenthesization / `op(x)`
  pseudo-calls. Guarded by the printer's round-trip idempotency test. Output-quality only.

- [2026-06-25] **A-46 = ALLOW expression form** (developer overruled my KEEP rec after a full
  explanation of the pre/post + eval-order + hidden-mutation hazards). Obligations: implement BOTH pre
  `++i` (new value) and post `i++` (old value) in interpreter+VM+transpiler **byte-identically**;
  **pin evaluation order to PHP's left-to-right** so `f(i++,i++)` agrees run≡runvm≡PHP (differential-
  gated); transpiles 1:1 to PHP `++$i`/`$i++`. **Sweetener (recommended):** optional `W-SEQUENCE-
  MUTATION` lint flagging the same var mutated twice in one expression — keeps the capability, warns on
  the footgun ("better not just different"). Status: decided; lint pending objection.

- [2026-06-25] **A-50 = KEEP two modes.** `"…"` (interpolates; brace-free = plain literal, zero
  escaping) + `r"…"` (raw). PHP's `'…'` rejected — redundant (`"…"` covers brace-free literals) and
  its semi-raw escape semantics are a PHP wart. Verified: multiline works today via `\n` escapes AND
  literal source-spanning `"…"`; `r"…"` spans raw (≈ PHP heredoc/nowdoc, no new syntax needed).
- [2026-06-26] **A-62 (new) = ADOPT `"""…"""` text blocks.** Triple-quote auto-dedent blocks (Java/
  Swift/C#/Kotlin/Ruby/Nix consensus): opening `"""`+newline; closing `"""` indentation = strip
  baseline; strip min-common leading whitespace of non-blank lines; **trailing whitespace stripped per
  line** (Java); interpolates `{…}`; compile-time transform (transpiler emits dedented bytes as a plain
  PHP string → byte-identical). **Purely additive** — `"…"` unchanged (all `\n`/spanning/`r"…"` behavior
  preserved, verified). Keep-indent: relative always preserved; absolute via plain `"…"` or column-0
  closing. Optional `r"""…"""` (raw+dedent). Beats PHP heredoc (cleaner) + TS (can't dedent).

- [2026-06-26] **B-9 = ADOPT minimal `$` escaping.** Transpiler escapes `$` only where PHP would
  interpolate (before `[A-Za-z_]` or `{`); `$5`/trailing-`$` left bare. Cleaner PHP AND the exact rule
  B-1's interpolated output requires — implement together.

## REVIEW COMPLETE — 16 / 16

### Decision summary  (12 ADOPT · 4 KEEP · 2 rejected = 16 + 2)
**ADOPT (12):** A-1 (`:` returns + `=>` fn-types, `->` retired) · A-6 (`foreach…as` + 4 forms +
`with int i`) · A-46 (expression `++`/`--` + `W-SEQUENCE-MUTATION` lint) · A-62 (`"""…"""` text blocks) ·
B-1 (per-hole PHP interpolation) · B-2 (`echo X, "\n"`) · B-9 (minimal `$` escaping) · C-1 (lift
interpolation, faithful subset) · C-5/6 (lift printer minimal parens) · C-45 (lift void-or-reject) ·
C-46 (lift `instanceof`) · C-47 (lift bitwise).
**KEEP (4):** A-3 (type-first params) · A-7 (`{w}` delimiter) · A-50 (two string modes) ·
A-61 (`instanceof` lowercase).
**REJECTED false-positive (2):** B-10 (intdiv namespace) · C-38 (multi-implements).

### Implementation streams (decided, NOT yet implemented — each its own gated slice/milestone)
- **Stream 1 — Surface-syntax reshape (BIG breaking codemod):** A-1, A-6, A-46, A-62. Touches
  lexer/parser/checker/interpreter/VM/transpiler/lift-printer + a global rewrite of every `.phg`,
  inline test program, doc, example, and the playground default. Highest risk; byte-identity-gated;
  needs its own design spec + codemod tooling. **Mandatory-return already exists** (totality cluster) —
  not part of this.
- **Stream 2 — Transpile interpolation fidelity:** B-1 + B-2 + B-9 (coupled). ✅ **IMPLEMENTED
  2026-06-26.** `emit_string` rewritten to emit native PHP `{$…}` for `$`-rooted Str/Int holes
  (guards: `is_php_interp_chain` + emitted-`$`-rooted + brace-free), concat fallback otherwise;
  `println`→`echo X, "\n"`; `php_escape_interp` minimal `$` escaping. 871 lib + 109 PHP-8.5 oracle
  byte-identical, clippy+fmt clean.
- **Stream 3 — Lift enhancements:** C-1, C-5/6, C-45, C-46, C-47. Mostly independent (lift-printer also
  changes in Stream 1 for the new syntax). Medium. **In progress:**
  - ✅ **C-45 IMPLEMENTED 2026-06-26** (`lift_ret` + `body_has_value_return` in lifter.rs): a PHP fn/
    method with no return hint → `void` when the body never returns a value (provable), else loud
    Tier-2 reject. Replaces the old silent non-compiling `ret: None`. Lift tests green.
  - ✅ **C-46 IMPLEMENTED 2026-06-26**: lift PHP `value instanceof ClassName` → Phorge `instanceof`
    (M-RT S1). New `PhpExpr::InstanceOf`; handled at the postfix level (non-associative); dynamic
    `instanceof $var` rejected loudly. Printer + Phorge backend already had `instanceof`.
  - ✅ **C-47 IMPLEMENTED 2026-06-26**: lift bitwise `& | ^ ~ << >>`. New `PhpBinOp::{BitAnd,BitOr,
    BitXor,Shl,Shr}` + `PhpUnOp::BitNot` + lexer tokens (`Amp/Bar/Caret/Tilde/Shl/Shr`); `infix_op`
    renumbered to the full PHP-8 table (bitwise/shift levels inserted, prior ops keep relative order);
    1:1 lifter mapping. The lift printer already covered all of these.
  - ⏳ Remaining: C-1 (interpolation), C-5/6 (printer minimal-parens).

STATUS: Designed — not yet implemented. Say go to plan/build a stream.
