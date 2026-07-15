# Full Reopen Audit — 2026-07-16

> **Mandate (developer, 2026-07-16, at-desk):** full complete rich deep audit/review of everything
> done. ALL KNOWN_ISSUES and ALL decisions reopened. The bar: **phorj must be conceptually,
> theoretically, and practically better / faster / safer / more secure / more intuitive than PHP**.
> Every deviation from PHP is justified strictly or FLAGGED for the developer. Anything
> non-generic or opinionated is flagged too. Architecture bar: clean, structured, decoupled,
> no fat files, better folder structure. Phorj must also stay AHEAD of PHP (8.6 plans in scope).
>
> **Protocol (ruled via AskUserQuestion, recorded in MASTER-PLAN §0.2):** audit-first ZERO source
> changes (doc-only consolidation commits allowed) · full external PHP re-sweep incl. 8.6
> RFCs/roadmap · FULL depth on every row (~180 register+issue rows, each gets a written verdict) ·
> checkpoint triage per dimension, flags brought one-by-one · everything unified into
> MASTER-PLAN/UNIFIED-SPEC.
>
> Baseline: HEAD `6b9256ba` (== origin/master, pushed). Verdict vocabulary:
> `JUSTIFIED(why)` / `FLAGGED(F-###)` / `OBSOLETE` / `SUPERSEDED(by)`.

## Dimension cursor

| Dim | Scope | Status |
|-----|-------|--------|
| D0 | PHP 8.4/8.5 surface re-sweep + 8.6 RFC ahead-watch vs the 824-row matrix | ✅ DONE — 8 flags, all triaged (DEC-239…244; F-003 fixed; F-008→D5) |
| D1 | Decision register full reopen (149 DEC rows) | ▶ IN PROGRESS |
| D2 | KNOWN_ISSUES full reopen (every row) | pending |
| D3 | Architecture / clean code / folder structure | pending |
| D4 | Security (every native surface vs PHP's equivalent) | pending |
| D5 | Perf-claim re-verification (WIN/HOLD/LOSS ledger) | pending |
| D6 | Docs drift + SSOT unification (runs throughout) | continuous |

## Flag ledger (grows monotonically; triage rulings recorded in C-decisions.md)

| Flag | Dim | Severity | One-liner | Ruling |
|------|-----|----------|-----------|--------|
| F-001 | D0 | HIGH | pipe `\|>` triple conflict: shipped-plain vs DEC-235-first-arg vs PHP-8.5-callable-application | **RULED DEC-239**: PHP-aligned base ratified, DEC-235 revoked, precedence fix + `%` placeholder + contextual pipe lambda queued |
| F-002 | D0 | HIGH | Core.Url = 4 helpers vs PHP 8.5 typed always-on URI parser | **RULED DEC-240**: Core.Uri (RFC 3986, typed errors, PHP-8.5 twin — transpilable) queued |
| F-003 | D0 | MED | FEATURES.md stale: "four deps" + "forbid(unsafe_code)" | **FIXED** (D6 doc commit, this session) |
| F-004 | D0 | MED | asymmetric visibility spec'd in UNIFIED-SPEC, absent in code | **RULED DEC-241**: build (sugar wave) |
| F-005 | D0 | LOW | partitioned-cookie (CHIPS) absent in Core.Session | **RULED DEC-242**: queue cookie-attr knob |
| F-006 | D0 | LOW | Core.String lacks similarity family (levenshtein/similar/soundex) | **RULED DEC-243**: levenshtein+similarText grapheme-aware; phonetics rejected |
| F-007 | D0 | MED | extension methods: PHP 8.6 drafts it — phorj should ship first | **RULED DEC-244**: early sugar-wave slot |
| F-008 | D0 | LOW | HttpClient per-request connects vs PHP persistent share handles | pending (D5) |

---

## D0 — PHP surface re-sweep + 8.6 ahead-watch

### D0 sources (fixed list, per protocol)

- php.net PHP 8.5 release page + UPGRADING/migration guide
- php.net PHP 8.4 release page + migration guide (matrix predates parts of it)
- wiki.php.net/rfc index — accepted / under-discussion / draft targeting 8.6
- SPL + function-category inventory spot-checks against the 824-row matrix

### D0.1 Source A inventory (PHP surface — external, fetched 2026-07-16)

**PHP 8.5 (released 2025-11; phorj's transpile floor):**
Syntax/lang: pipe `|>` (**callable application**: `$x |> strlen(...)` — value is the SINGLE arg;
multi-arg requires a closure) · `clone($obj, [prop => v])` clone-with (readonly wither) ·
closures + first-class callables in constant expressions · casts in constant expressions ·
static asymmetric visibility · `final` via ctor promotion · attributes on constants ·
`#[\NoDiscard]` · `#[\DelayedTargetValidation]` · `#[\Override]` on properties ·
`#[\Deprecated]` on traits/constants · fatal errors carry backtraces.
Functions/classes: `array_first()` / `array_last()` · `get_error_handler()` /
`get_exception_handler()` · `Closure::getCurrent()` (anon recursion) · `grapheme_levenshtein()` ·
**URI extension always-on** (`Uri\Rfc3986\Uri`, RFC 3986 + WHATWG) · `IntlListFormatter` ·
`Locale::addLikelySubtags()/minimizeSubtags()` · Dom `getElementsByClassName` /
`insertAdjacentHTML` / `$outerHTML` / `$children` · `FILTER_THROW_ON_FAILURE` ·
partitioned cookies (setcookie/session) · Pdo\Sqlite busy/explain/transaction-mode attrs ·
`curl_share_init_persistent()` · `mail()` returns real sendmail errors · getimagesize
HEIF/HEIC/SVG · flock on zlib streams.
Deprecations: backticks · non-canonical casts `(boolean)`… · `__sleep`/`__wakeup` ·
`case X;` semicolon · null array offset · NAN-cast + float-narrowing + non-array-destructure warnings.

**PHP 8.4 (matrix-era, re-verified):** property hooks (`get`/`set` on props) · asymmetric
visibility `private(set)` · `#[\Deprecated]` userland · Dom\HTMLDocument HTML5 parser +
querySelector · BcMath\Number (operator-overloaded arbitrary precision) · `array_find` /
`array_find_key` / `array_any` / `array_all` · **PDO driver-specific subclasses**
(Pdo\Mysql/Pgsql/Sqlite…) · `new X()->m()` no-parens chaining · lazy objects · new IR-framework
JIT · RoundingMode enum · `DateTime::createFromTimestamp/get+setMicrosecond` · mb_trim family ·
request_parse_body · fpow · grapheme_str_split · typed class constants · GMP final.

**PHP 8.6 ahead-watch (RFC index, fetched 2026-07-16):**
Implemented for 8.6: `#[\Override]` for class constants · deprecate return-from-ctor/dtor ·
**Polling API** (stream event polling) · **Debugable Enums** · `enum SortDirection` ·
doc-comments for params · `grapheme_strrev`.
PHP 9.0 pending: undefined property/variable → ERROR promotion · `${}` interpolation removal ·
no autovivification on false.
Under discussion/voting: strict namespace resolution · **pipe ASSIGNMENT operator** ·
**primary constructors** · **Duration class** · case-sensitive PHP · function autoloading v5 ·
**literal scalar types** (narrowing).
Notable drafts: **extension methods** (+ scalar extension methods) · catchable MemoryError ·
str_iter() UTF-8 iteration · **True Async**.

### D0.2 Source B inventory (phorj coverage — read from repo at `6b9256ba`)

**Core modules (36 native + prelude registry, from `src/native/` + `CORE_MODULES`):**
Bytes · Conversion · Cryptography · Csv · Db(+Sys) · Debug(+Sys) · Decimal · DI · Encoding ·
Environment · File · Fs(+Sys) · Hash · Html · Http · HttpClient(+Sys) · Ini · Json · List · Log ·
Mail(+Sys) · Map · Math · Option · Output · Path · Process · Random · Reflection · Regex · Result ·
Runtime · Secret · Session(+Sys) · Set · String · Test · Time · Url · Validation.

**Language surface (FEATURES.md, verified against code where load-bearing):** static types +
bytes/decimal/Html · generics (erased) + bounds + turbofish · unions/intersections · sealed ·
overloading · MI with explicit resolution · traits · property hooks · `with {}` functional update ·
ctor promotion + ctor defaults (DEC-236) · checked exceptions + `?` + Result · null-safety `T?` ·
match exhaustiveness · lambdas/first-class fns (by-value capture) · pipe `|>` (**shipped**, plain
application — probed live: `5 |> inc` → `6`) · `E-UNUSED-VALUE` default-on with `discard` escape ·
ranges · string interpolation `{}` · concurrency (green threads, native-only) · DI ·
casing enforcement · import discipline (nothing-in-the-wind).

**Relevant native-fn surfaces (greps above):** List = all any append chunk concat contains count
drop enumerate fill filter find first flatten indexOf isEmpty last lastIndexOf length map max min
reduce reverse slice sort sortWith sum take unique · String = 33 fns (no levenshtein/similar/soundex;
`characters` = codepoint iteration) · **Url = ONLY encodeForm/decodeForm/encodeUriComponent/
decodeUriComponent** · Map = filter get getOrDefault has isEmpty keys map merge remove set size values.

### D0.3 Delta list (each side-only item = automatic finding)

**PHP-only (gaps → flags):**
| Item | PHP | Phorj | Disposition |
|---|---|---|---|
| Typed URI parser | 8.5 always-on `Uri\Rfc3986\Uri` + WHATWG | Core.Url = 4 encode/decode fns; HttpClient parser INTERNAL | **F-002** |
| Asymmetric visibility | 8.4 `private(set)`, 8.5 static | UNIFIED-SPEC lists it in frozen surface; NO impl evidence (`private(set)` grep = 0 code hits) | **F-004** spec-vs-code |
| Partitioned cookies (CHIPS) | 8.5 setcookie/session | absent in Core.Session | **F-005** |
| levenshtein/similar/soundex (+ 8.5 grapheme_levenshtein) | yes | absent (levenshtein exists internally for did-you-mean only) | **F-006** |
| Persistent connection reuse | 8.5 curl share handles | HttpClient = per-request connect | **F-008** (perf, D5) |
| Duration/DateTime richness | 8.6 Duration RFC; DateTime mature | DEC-206 gated, unbuilt | fold into DEC-206 (D1) |
| Closure self-recursion | 8.5 `Closure::getCurrent()` | by-value capture, no self-ref idiom | minor, note |
| Lazy objects | 8.4 | none (DI covers the niche) | JUSTIFIED — DI is the phorj answer; record |
| Intl (list formatter, locales, NumberFormatter) | mature ext | no i18n domain | known matrix gap (M-gap) — unchanged |

**Phorj-only / phorj-better (AHEAD ledger — justified, recorded):**
`discard`-by-default (`E-UNUSED-VALUE`) ⊃ PHP 8.5 opt-in `#[\NoDiscard]` · `with {}` ⊃ 8.5
clone-with (immutability is default, not readonly-special-case) · ctor promotion + defaults ⊃ 8.6
primary-ctors RFC (phorj shipped first) · `decimal` primitive ⊃ 8.4 BcMath\Number (language-level,
operator-native) · Core.Db typed multi-driver + Secret + W-SQL-INJECTION ⊃ 8.4 PDO subclasses ·
List first/last/find/any/all + 15 more ⊃ 8.4/8.5 array_find/any/all/first/last · checked typed
exceptions ⊃ get/set_error_handler · compile-time undefined-var/prop errors ≡ what PHP 9.0 only
HOPES to do · green-thread concurrency ⊃ True Async (draft) · Debug.dump enum rendering ⊃ 8.6
"Debugable Enums" · sealed+exhaustive match, unions, intersections, generics: no PHP counterpart ·
8.5's deprecation list (backticks, non-canonical casts, null offsets) = things phorj never allowed.

**Both, SEMANTICS DIVERGE:**
| Item | PHP 8.5 | Phorj today | DEC-235 ruling | Disposition |
|---|---|---|---|---|
| Pipe `|>` | callable application: `$x \|> f(...)` (value = the ONE arg; `$x \|> f($a)` means *evaluate `f($a)`, then apply the result*) | `x \|> f` ≡ `f(x)` (shipped & passing) | first-arg insertion `x \|> f(a)` ≡ `f(x, a)` (unbuilt) | **F-001** — three-way conflict, re-adjudicate |

### D0.4 PHP 8.6 ahead-watch verdicts

- **Extension methods (draft + scalar variant)**: phorj's ruled-but-unbuilt sugar-pack item —
  **F-007**: ship before PHP does (ahead-of-php mandate).
- **Pipe assignment `|>=`**: fold into the F-001 ruling.
- **Literal scalar types** (narrowing): watch; phorj smart-casts already narrow on `instanceof` —
  literal-value narrowing is a possible future ahead-move, no action now.
- **Polling API**: phorj channels/green-threads answer the niche natively; no action.
- **`#[\Override]` for constants / doc-comments for params / grapheme_strrev / SortDirection**:
  minor; phorj `override` keyword is mandatory (stronger); no action.
- **PHP 9.0 error-promotions**: phorj is already there at compile time — the direction of travel
  validates phorj's core bet; record in MASTER-PLAN vision framing.

### D0 flags (→ ledger)

- **F-001 HIGH** pipe `|>` triple conflict (shipped-plain vs DEC-235-first-arg vs PHP-8.5-callable;
  FEATURES.md row says shipped while the cursor plan says "new Expr node" — plan text also stale).
- **F-002 HIGH** Core.Url is 4 helpers vs PHP 8.5 always-on typed URI parser; phorj needs typed
  `Uri` (promote HttpClient's internal parser) to match-and-beat.
- **F-003 MED** FEATURES.md stale claims: "exactly four vetted deps" (Cargo: argon2, regex, ctrlc,
  corosensei, rustls, webpki-roots, lettre, rusqlite, mysql, postgres domains), "`forbid(unsafe_code)`"
  (actual: `deny` + audited JIT island). Doc-only → D6 fix.
- **F-004 MED** asymmetric visibility: spec'd in UNIFIED-SPEC frozen surface, not found in code.
- **F-005 LOW** partitioned-cookie (CHIPS) support absent in Core.Session.
- **F-006 LOW** Core.String lacks the similarity family (levenshtein/similarText/soundex,
  grapheme-aware per W4-4).
- **F-007 MED** extension methods: PHP 8.6 drafts it; phorj should ship its ruled version first.
- **F-008 LOW** HttpClient per-request connections vs PHP persistent share handles → D5 lever.

---

## D1 — Decision register reopen

> Every row re-verdicted at full depth against the better-than-PHP bar. Vocabulary:
> JUSTIFIED(why) / FLAGGED(F-###) / OBSOLETE / SUPERSEDED(by). "probe" = re-run live this audit.

### §1 Foundational doctrine (DEC-001…016)

- DEC-001 **JUSTIFIED** — the 3-backend byte-identity spine is the project's core safety asset; re-verified daily by the differential harness (2158 tests green at baseline).
- DEC-002 **JUSTIFIED** — D-L9 (Phorj:PHP :: TS:JS) still the governing bridge contract; consistent with the ladder rule for no-analog features.
- DEC-003 **JUSTIFIED** — PHP oracle fails-not-skips; verified in the pre-push gate.
- DEC-004 **JUSTIFIED** doctrine — craftsmanship as apex filter is literally this audit's bar.
- DEC-005 **JUSTIFIED** doctrine — transpile-is-a-bridge; re-affirmed by DEC-240 (Uri twin is an EMISSION target, native impl stays the source of truth).
- DEC-006 **JUSTIFIED** — compile-time config; byte-identity argument still airtight.
- DEC-007 **JUSTIFIED** — Tier A/B determinism partition held through this run's Db/Mail/HttpClient/Fs/Session admissions (all quarantined case-by-case).
- DEC-008 **SUPERSEDED** (DEC-009) — already marked; no action.
- DEC-009 **JUSTIFIED** — per-domain vetted deps; every later admission (rusqlite/postgres/mysql/lettre/rustls) went through a recorded ruling. FEATURES.md count was stale → fixed this audit (F-003).
- DEC-010 **JUSTIFIED** standing — bounded-autonomy refinements (2026-07-15) are amendments, not conflicts.
- DEC-011 **JUSTIFIED** standing (= Invariant 9).
- DEC-012 **JUSTIFIED**, refined by the 2026-07-15 AUTO-RULED(REOPENABLE) protocol — supersession chain intact.
- DEC-013 **JUSTIFIED**; register row STALE — [Verified probe: `git remote -v` = tmessaoudi-official/phorj; cwd = /stack/projects/phorj] the "still manual" rename residue is DONE → register updated (D6).
- DEC-014 **JUSTIFIED** (`phg` binary, ripgrep model).
- DEC-015 **JUSTIFIED** — BETTER/SAME/WORSE(reject) bar = this audit's mandate, ratified again today.
- DEC-016 **OBSOLETE** — that audit completed (this register is its artifact); row's "in progress" stale → register updated (D6).

### §2 Namespace / modules / packages (DEC-020…049)

- DEC-020 **JUSTIFIED** — nothing-in-the-wind; [Verified probe: bare `panic(…)`/`assert(…)` → "fault intrinsic and needs an import"] the DEC-047 gap is CLOSED in code.
- DEC-021 **JUSTIFIED** — Go-style leaf-qualified calls; the D-L9-compatible choice.
- DEC-022 **SUPERSEDED** (DEC-034 casing, DEC-113 renames) — already marked.
- DEC-023 / DEC-024 **JUSTIFIED** — explicit stdlib imports diverge from PHP's ambient functions; justification = the anti-wind doctrine (recorded, strict).
- DEC-025 **JUSTIFIED** — mandatory `package`, Go model.
- DEC-026 / DEC-027 **JUSTIFIED** — native registry + shadow guard, structural-parity wins.
- DEC-028 **JUSTIFIED** — phorj.toml exact-pin; divergence from Composer ranges justified by determinism (DEC-010 of the module world). Transitive deps still open → tracked (FEATURES row).
- DEC-029 / DEC-030 / DEC-031 **JUSTIFIED** — loader-side enforcement/mangling; single-file PHP emission (PSR-4 autoload can't load free functions — reason still valid).
- DEC-032 **SUPERSEDED** (DEC-036) — already marked.
- DEC-033 **JUSTIFIED** — vendor-only network; `E-VENDOR-MISSING` offline discipline. Transitive deps deferred = tracked gap.
- DEC-034 / DEC-035 **JUSTIFIED** — PascalCase everywhere, hard errors; divergence from PHP's anything-goes justified: phorj ENFORCES what PSR merely suggests (better-than-PHP by construction).
- DEC-036 **SUPERSEDED** — the `import type` syntax it introduced was retired 2026-07-03 by the unified `import` (loader classifies module-vs-type); register row said plain ✅ → supersession note added (D6).
- DEC-037 **JUSTIFIED** (no wildcard import — PHP has none either); the selective-import mechanics folded into unified import.
- DEC-047 **JUSTIFIED**, register STALE — 📐 → shipped-in-substance [Verified probe above]; residual de-reservations (Core.Async naming) tracked in D2 sweep.
- DEC-048 **JUSTIFIED** as designed-deferred [Verified: no `[packages]` parsing in loader] — stays 📐, tracked.
- DEC-049 **JUSTIFIED** — keyword-vs-import 3-way rule; the Java-autoboxing rejection ages well.

### §3 Type system & generics (DEC-050…070)

- DEC-050 **JUSTIFIED** — maximal type system is phorj's core ahead-of-PHP asset; PHP 9.0's error-promotion RFCs validate the direction (D0.4).
- DEC-051 **JUSTIFIED** — `instanceof` lowercase, PHP-familiar; smart-casts beat PHP.
- DEC-052 **JUSTIFIED** — shared `class_implements` = structural parity.
- DEC-053 **JUSTIFIED** — full erasure matches the PHP-target reality; reified-in-checker keeps static safety.
- DEC-054 **JUSTIFIED** — generics-everywhere.
- DEC-055 **SUPERSEDED** (DEC-208 slice A shipped call-site turbofish `69a9151e`) — register row unmarked → supersession note added (D6).
- DEC-056 **JUSTIFIED**; its "lone `Circle =>` catch-all footgun deliberately preserved" was later CLOSED by DEC-209 (`E-MATCH-BARE-VARIANT` + `default`) — supersession-in-part noted (D6).
- DEC-057 **JUSTIFIED** + **FLAGGED(F-010)** — the recorded "revisit E-INTERSECT-SIG when overloading lands" is now DUE (overloading shipped, DEC-058/059) and nobody reopened it: intersections still require signature agreement instead of overload-resolving. Design debt, needs a ruling.
- DEC-058 / DEC-059 **JUSTIFIED** — overloading incl. return-type overloading; beyond-PHP power with compile-time ambiguity errors (PHP has neither).
- DEC-060 / DEC-061 **JUSTIFIED** — totality cluster; generic enums.
- DEC-062 **JUSTIFIED** — explicit-resolution MI; Model-3 (C3) deferral tracked.
- DEC-063 **JUSTIFIED** — final-by-default diverges from PHP open-by-default; justification recorded (consistency with immutable-by-default; Kotlin precedent; `open` is one keyword away).
- DEC-064 **JUSTIFIED** — traits with every PHP footgun promoted to compile-time diagnostics: a flagship better-than-PHP row.
- DEC-065 **JUSTIFIED**, superseded-in-part by DEC-205 — "no tracing GC (acyclic)" was amended: cycles CAN form, DEC-205 ruled the phased collector (php-style threshold first, `Weak<T>` second). Chain noted in register (D6).
- DEC-066 **JUSTIFIED** — explicit `this.field` (PHP-faithful, reader-honest).
- DEC-067 **JUSTIFIED** — compile-time visibility beats PHP's runtime-only.
- DEC-068 **JUSTIFIED** — three-tier error model; checked `throws` + Result + faults is strictly richer than PHP's untyped exceptions.
- DEC-069 / DEC-070 **JUSTIFIED**.

### §4 Language surface (DEC-080…105)

- DEC-080 / DEC-081 **JUSTIFIED** — S0 DX + null-safety suite (compile-time null safety: the single biggest better-than-PHP claim, and it holds).
- DEC-082 **JUSTIFIED**, superseded-in-part by **DEC-239** (pipe package: precedence fix + `%` + contextual lambda; capture-by-value and parser-lowering stand).
- DEC-083 **JUSTIFIED** — mandatory `new` uniformity (classes + variants).
- DEC-084 / DEC-085 **JUSTIFIED** — consts + eager statics (deterministic init order; PHP's lazy static init is the wart avoided).
- DEC-086 **JUSTIFIED** (void/empty split, reshaped by DEC-113).
- DEC-087 **JUSTIFIED** — UFCS method-first; beyond PHP, deterministic resolution recorded.
- DEC-088 **JUSTIFIED** — return-type mandate with expr-lambda inference; DEC-239's pipe-position param inference is a consistent extension of the same principle.
- DEC-089 **JUSTIFIED** batch — incl. `s[0]` deferral: PHP's byte-offset string indexing is a UTF-8 wart; `String.characters`/`substring` are the codepoint-safe answer (divergence justified, recorded).
- DEC-090 **JUSTIFIED** — ternary stays deferred; expression-`if` covers the capability and `?` already means optionals + throws-propagation (three-meanings hazard real).
- DEC-091 **JUSTIFIED** — `\{`/`\}` + raw strings.
- DEC-092 **JUSTIFIED** — no ambient superglobals EVER: categorically safer than PHP.
- DEC-093 **JUSTIFIED** — `: T` returns.
- DEC-094 **FLAGGED(F-009)** — ruled "foreach REPLACES for-in", shipped ALONGSIDE (conflict C-2 confirmed live: both run today) — and the two forms are inconsistent: `foreach (xs as x)` binds UNTYPED while `for (int x in xs)` requires the type [Verified probes]. Redundant double surface violates the one-way principle; needs a ruling.
- DEC-095 **JUSTIFIED** — type-first params (PHP-minus-sigil).
- DEC-096 **JUSTIFIED** as corrected by DEC-210 — `++`/`--` statement-only; divergence from PHP's expression form justified (sequence-point hazards; PHP's own left-to-right pinning is the confession).
- DEC-097 **JUSTIFIED** — one interpolating string + raw mode; single-quote rejection recorded (PHP's two-string-type split is the wart).
- DEC-098 / DEC-099 **JUSTIFIED**.
- DEC-100 **JUSTIFIED** — contextual `var` (the recorded same-day reversal is the register working as intended).
- DEC-101 **JUSTIFIED** — default params; DEC-236 extended to ctors consistently.
- DEC-102 **JUSTIFIED** — `length`(ordered)/`size`(keyed) semantic split beats PHP's count-everything; hard rename, no aliases.
- DEC-103 **JUSTIFIED** — both entry-point forms (developer overrule recorded with reasons).
- DEC-104 **JUSTIFIED** — checked `as` casts beat PHP's silent coercions.
- DEC-105 **JUSTIFIED** — B1 iteration protocol (but see F-009 for the surface duality).

## D2 — KNOWN_ISSUES reopen

<!-- one verdict line per issue row -->

## D3 — Architecture

## D4 — Security

## D5 — Perf ledger

## D6 — Docs unification log
