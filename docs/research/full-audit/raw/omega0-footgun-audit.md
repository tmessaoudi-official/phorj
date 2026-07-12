# Ω-0 — Footgun audit: all 49 GAP-by-design rows, one by one

> Session 5, 2026-07-12. Governing rule (developer ruling C, 2026-07-11): *do everything PHP
> does, better; take NONE of PHP's weaknesses.* Per row: a genuine CAPABILITY hiding behind the
> footgun → route it to its better/safer Phorj cover (schedule the wave); a PURE footgun →
> confirm excluded with the rationale. Verdicts here feed row detail into Ω-1…Ω-7.
> Row sources: `M-gap-matrix.md` (24 SYN + 24 FN + 1 RT = 49), names from `D-php-surface.md`.

## Verdict key

- **EXCLUDED ✓** — pure footgun; stays GAP-by-design, rationale confirmed.
- **COVERED-BETTER ✓** — the capability already exists in a safer Phorj form; row stays GD
  (the *PHP mechanism* is what's excluded), nothing to schedule.
- **CAPABILITY → Ω-n** — a genuine capability residue exists; scheduled to the named wave in
  its safer form (the footgun mechanism itself stays excluded).

## SYN rows (24)

| Row | PHP surface | Verdict | Reasoning / routing |
|---|---|---|---|
| SYN-013 | `eval()` | **EXCLUDED ✓** | The RCE-class root. Closed no-`eval` language is a foundational spine (E-importer-stageC); every legitimate use (plugins, config DSLs) has a static answer (first-class fns, match, compile-time config). No residue. |
| SYN-015 | backtick `` `cmd` `` operator | **CAPABILITY → Ω-2** | The OPERATOR is the injection footgun (shell-string interpolation). The capability — running external programs — is genuine and routes to Ω-2 `Core.Process` (typed arg-VECTOR API, no shell string, explicit env/cwd; the FN-PROC exec/system/proc_open GU rows land there). |
| SYN-019 | `$$x` variable variables | **EXCLUDED ✓** | Pure dynamic-scope footgun. Dynamic key→value is a `Map`. |
| SYN-021 | `new $cls(…)` dynamic class names | **CAPABILITY → Ω-4/Ω-7** | Stringly construction is the footgun. The capability (polymorphic factories, framework wiring) routes to first-class fn values + `match` today, and the typed reverse-discovery primitive `subjectsWith<Attr>()` (DI v2, Ω-4; framework stack, Ω-7). `E-NEW-REQUIRED` stands. |
| SYN-022 | `global $x` | **EXCLUDED ✓** | DEF-010. Shared state = class `static` fields + DI (both shipped). |
| SYN-023 | function-`static $x` persistent locals | **COVERED-BETTER ✓** | Hidden per-function state is the footgun; class `static` fields give the same persistence, visible in a type. |
| SYN-025 | `define()`/`defined()`/`constant()` runtime constants | **EXCLUDED ✓** | DEF-019. Config-must-be-compile-time tenet; `Environment` reads cover runtime inputs. |
| SYN-059 | `@` error suppression | **EXCLUDED ✓** | DEF-007. Expected failures are `T?`/`Result` + `try/catch` — visible in the type, not silently eaten. |
| SYN-065 | references `$b =& $a` (all forms) | **EXCLUDED ✓** | DEF-006 class. Out-params → multiple returns/records; aliased mutation → the exact class the JIT's ownership discipline banks on being absent. |
| SYN-073 | `switch` (loose `==`, fall-through) | **COVERED-BETTER ✓** | DEF-027. Exhaustive `match` with typed-strict compare is the strictly-better cover (shipped). |
| SYN-076 | `goto` | **EXCLUDED ✓** | Pure. (If labeled break/continue ever proves needed it is an Ω-4 nicety, not this row.) |
| SYN-093 | by-ref params / by-ref returns | **EXCLUDED ✓** | Same DEF-006 class as SYN-065. |
| SYN-097 | `Closure::bind/bindTo/call` | **EXCLUDED ✓** | `this`-rebinding = dynamic scope. First-class fn values + `fromCallable`-equivalent named-fn refs cover the legitimate residue (shipped). |
| SYN-126 | `__destruct()` | **CAPABILITY → Ω-4 (design Q)** | Nondeterministic finalization under Rc is the footgun (DEF-035). The residue — deterministic scope-exit resource cleanup (files, locks, connections) — is REAL and currently uncovered: **schedule a scope-guard construct (`using`/`defer`) as an Ω-4 design question** (ask-human at build time; ADJUDICATION rule). Recorded in KNOWN_ISSUES §PENDING. |
| SYN-127 | `__call` | **EXCLUDED ✓** | Magic dynamic dispatch. Proxies/delegation route to traits (§7 fork) + DI decorators — static forms. |
| SYN-128 | `__callStatic` | **EXCLUDED ✓** | Same. |
| SYN-129 | `__get` | **COVERED-BETTER ✓** | Property hooks (shipped, M-mut.7b) — computed reads, statically declared. |
| SYN-130 | `__set` | **COVERED-BETTER ✓** | `set` hooks (shipped). |
| SYN-131 | `__isset` | **EXCLUDED ✓** | isset/empty conflation class (DEF-014); optionals make it meaningless. |
| SYN-132 | `__unset` | **EXCLUDED ✓** | Same; `Map.remove` exists for the collection case [Verified: `src/native/map.rs:241`]. |
| SYN-135 | `__serialize` | **CAPABILITY → Ω-7 (candidate)** | DEF-013 RCE class — magic-method serialization stays out. The residue (durable typed object persistence) is genuine: `Core.Json` is the data path today; a **schema-checked typed serde derive** (zero code execution on decode) is an Ω-7 beyond-PHP candidate. |
| SYN-136 | `__unserialize` | **EXCLUDED ✓** | The dangerous half; subsumed by the SYN-135 routing. |
| SYN-153 | `set_error_handler`/`set_exception_handler` | **CAPABILITY → Ω-2 (partial)** | Global MUTABLE handler registry is the footgun. The residue — centralized error REPORTING — routes to Ω-2 structured logging (G-log) + `serve`'s error rendering + top-level catch. No global mutable hooks. |
| SYN-160 | `isset()`/`empty()`/`unset()` | **COVERED-BETTER ✓** | DEF-014. Optionals + explicit `isEmpty` + `Map.remove`. |

## FN rows (24)

| Rows | PHP surface | Verdict | Reasoning / routing |
|---|---|---|---|
| FN-STR ×3 | `setlocale`/`nl_langinfo`/`strcoll` | **CAPABILITY → Ω-5** | The footgun is PROCESS-GLOBAL locale state (setlocale mutates every subsequent call's behavior). The capability (locale-aware collation/formatting) is genuine → Ω-5 intl with EXPLICIT locale-instance APIs (`new Collator(locale)` — instances tenet), behind the ICU extension story (below). Global-state form stays excluded. |
| FN-ARR ×2 | `compact`/`extract` | **EXCLUDED ✓** | The `$$x` class in function form — scope injection. Records/maps cover. |
| FN-DATE ×1 | mutable `DateTime` | **COVERED-BETTER ✓** | DEF-011. `Core.Time` is immutable-only (shipped). |
| FN-DATE ×1 | `strtotime` DWIM parsing | **CAPABILITY → Ω-5** | Silent wrong parses ("next Tuesday", locale-dependent) = the footgun. The residue — parsing dates from strings — routes to Ω-5 explicit-FORMAT parse APIs (typed `Date.parse(s, format) -> T?`). DWIM linguistics stay excluded. |
| FN-INTL ×2 | `Collator`, `Transliterator` | **CAPABILITY → Ω-5 (behind adjudication)** | Capability genuine; blocker is the ICU data dependency vs the std-only policy. Ω-5's charter already flags "intl needs an ICU extension story" — a §15 fork to SURFACE (feature-gated vetted dep, the argon2/regex precedent — RECOMMENDED — vs pure-Rust `icu4x` vs stay-excluded). Instance-model APIs either way. |
| FN-REFL ×2 | `ReflectionMethod/Function::invoke` | **EXCLUDED ✓** | Stringly dynamic dispatch. The framework capability routes to DI v2 + `subjectsWith<Attr>()` typed reverse discovery (Ω-4/Ω-7, already scheduled). Read-only introspection stays. |
| FN-PROC ×6 | pcntl fork/signal family | **CAPABILITY → Ω-2 (one slice)** | `fork()` in a managed runtime + global signal handlers = the footgun class; concurrency is already covered-better (spawn/Channel/Task). ONE genuine residue: **graceful-shutdown hooks for long-lived processes** (`serve` handles SIGINT internally via the vetted `ctrlc` dep) — schedule a typed `Runtime.onShutdown`-shaped surface as an Ω-2 design question alongside `Core.Process`. Raw pcntl stays excluded. |
| FN-VAR ×3 | `serialize`/`unserialize`/`get_defined_vars` | **EXCLUDED ✓** | serialize/unserialize = DEF-013 (routing per SYN-135); `get_defined_vars` = scope introspection (statically meaningless). |
| FN-FUNC ×1 | `func_get_args` | **CAPABILITY → Ω-4 (already scheduled)** | Static signatures cover; the residue (variable arity) is TOP-20 #7 named-args/variadics/spread — already Ω-4. |
| FN-MISC ×2 | `ini_get`/`ini_set` family | **EXCLUDED ✓** | Compile-time-config tenet (SYN-025's function form). |
| FN-MISC ×1 | FFI | **EXCLUDED ✓ (policy IS the cover)** | Arbitrary native loading voids every memory-safety guarantee — the RT-015 story below. |

## RT row (1)

| Row | Surface | Verdict | Reasoning |
|---|---|---|---|
| RT-015 | dynamic extension model (.so plugins, FFI) | **EXCLUDED ✓ (policy IS the cover)** | Loading arbitrary native code is the one mechanism that voids `#![deny(unsafe_code)]`, byte-identity, and the determinism spine simultaneously. The BETTER form already exists and has precedent: the **vetted feature-gated Rust dependency policy** (argon2 / regex / ctrlc / corosensei, rusqlite approved for Ω-1; the Ω-5 ICU story rides the same lane) — extensions enter through audited compile-time features, never runtime loading. |

## Tally + routed work

**49/49 audited. Zero rows flip to COVERED today** (every verdict either confirms the exclusion
or routes a capability to a wave where its safer form was already, or is now, scheduled):

- **Ω-2**: `Core.Process` typed subprocess (absorbs SYN-015 + FN-PROC exec-family GU rows);
  structured logging absorbs SYN-153's reporting residue; graceful-shutdown surface (design Q).
- **Ω-4**: scope-guard construct `using`/`defer` (NEW design Q from SYN-126 — the one genuinely
  uncovered residue found by this audit); variadics (already #7); DI v2 reverse discovery.
- **Ω-5**: explicit-locale intl instances + explicit-format date parse; the ICU extension-story
  fork to SURFACE (feature-gated vetted dep RECOMMENDED).
- **Ω-7**: schema-checked typed serialization derive (candidate, from SYN-135).

**Grade:** row verdicts [Verified: each against its M-gap-matrix note + D-php-surface
definition; `Map.remove` existence checked in source]; wave routings [Inferred: consistent with
the Ω-1…Ω-7 charters already ratified in MASTER-PLAN]; the two NEW design questions are
recorded in KNOWN_ISSUES §PENDING per the ADJUDICATION rule (surface, don't self-rule).
