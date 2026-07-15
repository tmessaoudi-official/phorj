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
| D1 | Decision register full reopen (155+ rows incl. conflicts/autonomous lists) | ✅ DONE — 5 flags triaged (DEC-245…250); 7 conflicts closed; 12 register-stale rows queued for D6 |
| D2 | KNOWN_ISSUES full reopen (every row) | ✅ DONE — 17 stale rows→D6; 8 flags ruled (DEC-251…258) + DEC-259 perf doctrine + standing rules (META-7, check≡LSP, transpile/lift-always-current) |
| D3 | Architecture / clean code / folder structure | ▶ IN PROGRESS |
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
| F-009 | D1 | HIGH | for-in/foreach duality + binding inconsistency (conflict C-2) | **RULED DEC-248**: full PHP loop alignment — typed foreach + k=>v; for-in retires; codemod |
| F-010 | D1 | MED | E-INTERSECT-SIG overloading revisit 3 weeks overdue | **RULED DEC-245**: overload-set resolution on intersections |
| F-011 | D1 | MED | clippy::pedantic ruled (DEC-176) but never enabled | **RULED DEC-246**: build pedantic slice |
| F-012 | D1 | HIGH | Core.Time has NO DateTime/Duration/tz — biggest stdlib gap vs PHP | **RULED DEC-247**: Core.DateTime now, high priority |
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
- DEC-106 **JUSTIFIED** — dogfood fixes.
- DEC-107 **JUSTIFIED**, register STALE — the ADD half (method references as typed closures) is SHIPPED [Verified probe: `5 |> a.plus` → 15 works], the REJECT half (string-instantiate) stands justified (un-typeable). 📐 → ✅ noted (D6).

### §5 Naming (DEC-110…114)

- DEC-110 / DEC-111 / DEC-112 **JUSTIFIED** — camelCase stdlib; PHP-reserved variant mangling transpiler-side; forced Channel.create rename.
- DEC-113 **JUSTIFIED** — the naming overhaul (clarity/no-shortcut) is a systematic better-than-PHP readability position; "unpushed" note stale (pushed long since) → D6.
- DEC-114 **SUPERSEDED**→✅ chain already correct.

### §6 Runtime / VM / perf (DEC-120…129)

- DEC-120 / DEC-121 / DEC-122 **JUSTIFIED** — bench-gated evolution recorded honestly (slot-indexed shipped only when evidence arrived — the discipline working).
- DEC-123 **JUSTIFIED**, superseded-in-part by DEC-205 (cycle collector phased in — the "acyclic" premise fell to closures/self-reference; chain noted).
- DEC-124 **JUSTIFIED** — the 3-match Op discipline (= Invariant 3).
- DEC-125 / DEC-126 / DEC-127 **JUSTIFIED** — higher-order natives; insertion-ordered maps (PHP-faithful, and PHP's own model); COW O(1) index-assign.
- DEC-128 **JUSTIFIED** — W2 deferral was evidence-based; the perf-gate shipped instead. Still 📐, tracked.
- DEC-129 **JUSTIFIED** — profiles as side-channels; the byte-identity keystone held.

### §7 Concurrency (DEC-130…135)

- DEC-130 **JUSTIFIED**, conflict C-6 **RESOLVED** — [Verified: `src/serve/handlers.rs` builds one program *per worker thread*, values never cross threads] the OS-thread serve pool does NOT violate the !Send single-heap doctrine; each worker owns its heap. Conflict row closable (D6).
- DEC-131 **JUSTIFIED** — admission lattice; shared-state threads remain HARD NO; parallel/reactive 📐 tracked (DEC-135).
- DEC-132 **JUSTIFIED** — uniform stackful coroutines both backends; corosensei admission recorded.
- DEC-133 **JUSTIFIED**, refined by DEC-225 (PHP 8.1 Fibers ruled a faithful transpile candidate — spike queued; quarantine stands until proven).
- DEC-134 **OBSOLETE** (interim step) → superseded by the A1 cutover, chain correct.
- DEC-135 **JUSTIFIED** — parallelism on hold with the models table recorded; actor-model lean noted, no silent commitment.

### §8 Web / stdlib / natives (DEC-140…156)

- DEC-140 / DEC-141 / DEC-142 **JUSTIFIED** — value-level `handle(Request)→Response` (the PSR-7 insight, minus PHP's mutable-stream warts); one public API; bytes primitive.
- DEC-143 **JUSTIFIED** — the URL/network deferral resolved correctly over time: HttpClient (DEC-231) + Core.Uri (DEC-240) now close it with determinism preserved.
- DEC-144 **OBSOLETE** (interim subset; both deferred modules long since shipped).
- DEC-145 **JUSTIFIED** — Json Int/Float split is PHP-faithful AND type-honest.
- DEC-146 **JUSTIFIED** — strcmp sort, never numeric-string juggling: a recorded divergence that is precisely a PHP-wart removal.
- DEC-147 / DEC-148 **JUSTIFIED** — decimal primitive with exact-or-fault division and always-fault div-by-zero (IEEE inf/NaN removed): categorically safer than PHP floats + BCMath strings.
- DEC-149 **JUSTIFIED** — NaN/Inf as functions; `toInt → int?` fixes the `(int)` quirk.
- DEC-150 **JUSTIFIED** — pure PRNG with hand-rolled PHP twin (byte-identity over convenience — the doctrine at its best).
- DEC-151 **JUSTIFIED** — Argon2id via vetted dep; note phorj's default (Argon2id) is stronger than PHP's password_hash default (bcrypt) — AHEAD ledger.
- DEC-152 / DEC-153 / DEC-154 **JUSTIFIED** — Http API shape; charter-first ordering; Router+attributes.
- DEC-155 **JUSTIFIED** — identical traces, prod-bare-500 (leak-safe by default; PHP needs display_errors discipline).
- DEC-156 **JUSTIFIED** — manual timing quarantined.

### §9 Tooling / build / interop (DEC-160…176)

- DEC-160 / DEC-161 / DEC-162 **JUSTIFIED** — source-embedding build; one timing surface; helper-based transpile.
- DEC-163 **JUSTIFIED** — 8.5 floor (now also the enabler of DEC-240's native-Uri twin and DEC-239's native pipe emission).
- DEC-164 / DEC-165 **JUSTIFIED**.
- DEC-166 / DEC-167 **JUSTIFIED** — staged lift with loud Tier-3 disclosure; "silent wrong guess worse than loud rejection" is the audit's own creed.
- DEC-168 / DEC-169 / DEC-170 / DEC-171 / DEC-172 **JUSTIFIED** (3b signing parked, tracked).
- DEC-173 **JUSTIFIED** — M-Decomp hybrid model; note D3 will re-measure fat files against it.
- DEC-174 **JUSTIFIED** standing — never-push held all run.
- DEC-175 **OBSOLETE** — that ordering completed/was overtaken; roadmap authority = MASTER-PLAN (already the SSOT).
- DEC-176 **FLAGGED(F-011)** — developer ruled "blanket `clippy::pedantic`, fix ALL" (overriding selective-lints); Cargo.toml today says `[lints.clippy] all = "deny"` — pedantic was never turned on [Verified: Cargo.toml:182-183]. Ruled-but-unbuilt quality gate, register row still says "in progress". ALSO found: the Cargo.toml comment block repeats the stale "four vetted crates + forbid(unsafe_code)" claims (F-003's source-side twin) — queued with the build slices since comments live in source files (zero-source-change discipline).

### §10 Parity SSOT summary row

- **JUSTIFIED** — the 290/187/81 adopt/defer/reject triage stands; spot-checked reject bucket (single-quotes, `<=>`, `.` concat, `switch`, superglobals, `eval`, variable-variables, runtime magic, `@`, loose `==`) — every rejection is a wart-removal with a recorded reason, none regressed by later work. The philosophy-recalibration correction (DEC-004) remains the governing lens.

### §11 Fork-backlog pass (DEC-177…181)

- DEC-177 **JUSTIFIED** — trait+MI duality (mirrors PHP's own trait duality, statically checked).
- DEC-178 / DEC-179 **JUSTIFIED**, register STALE — Waves A/C shipped (memory + MASTER-PLAN record Waves A/B/C DONE); 📐 → ✅ (D6).
- DEC-180 **JUSTIFIED**, register STALE — Wave B (error-model ergonomics + native fault reclassification) shipped; 📐 → ✅ (D6). No-catchable-faults stands: bugs stay bugs.
- DEC-181 **JUSTIFIED** — LSP-first symmetric; full-native phase still 📐, tracked; both-editors-same-change DoD standing.
- DEC-182 **JUSTIFIED**, register STALE — Core.Result/Core.Option ARE in CORE_MODULES today [Verified: registry grep]; 📐 → ✅ (D6). The T?-vs-Option distinct-roles ruling ages well (no implicit coercion = no Scala-style ambiguity).
- DEC-184 **JUSTIFIED**, register STALE — is/instanceof full symmetry SHIPPED [Verified probe: both `v is int` and `v instanceof int` narrow and run]; 📐 → ✅ (D6). The TIMTOWTDI challenge was heard and overruled with reasons — properly recorded.
- DEC-183 **JUSTIFIED** — flat `T?` match exhaustiveness shipped; the recorded caveat is CONFIRMED STILL OPEN [Verified probe: `match c { Red() => … }` over `Color?` → "variant pattern requires an enum scrutinee"] — Optional<enum> still needs smart-cast/`_`; follow-up queued (checkpoint list).

### §2026-07-12 batch (DEC-201…206 + META-1…3)

- DEC-201 **SUPERSEDED** (DEC-214) — chain recorded inline; correct.
- DEC-202 **JUSTIFIED**, shipped — `E-RESERVED-NAME` over invisible mangling: loud beats silent, and PHP interop names stay honest.
- DEC-203 **JUSTIFIED**, ruled-UNBUILT [Verified probe: `using (var x = …)` = parse error] — tracked; `Closable` + PHP try/finally twin keeps byte-identity. Queue standing.
- DEC-204 **JUSTIFIED**, ruled-UNBUILT [Verified: no `onShutdown` native] — rides with Ω-2 Core.Process work; tracked.
- DEC-205 **JUSTIFIED**, ruled-UNBUILT (phased collector + `Weak<T>`) — serve-can-never-leak is a safety commitment; queue standing. Amends DEC-065/123 as noted.
- DEC-206 **JUSTIFIED** ruling, but the probe found the REAL state: `DateTime` does not exist AT ALL (no type, member-import or not) — the gate ruling was about a hypothetical. **FLAGGED(F-012)**: Core.Time has no DateTime/Duration/timezone surface while PHP's is among its most mature APIs (8.4 added microsecond/timestamp ctors; 8.6 discusses Duration). Biggest remaining stdlib gap vs PHP.
- META-1 **OBSOLETE-COMPLETED** — sqlbuild went all the way; the run-end full reopen = THIS audit (executing now).
- META-2 / META-3 **JUSTIFIED** — executed as ruled.

### §2026-07-13 batch (DEC-207…219, META-4…6)

- DEC-207 **JUSTIFIED** — `::` static-access separator; part-1 shipped, **part-2 (E-SEP-MISMATCH enforcement + codemod) still pending** — tracked. ⚠ interaction noted for the DEC-234 build: qualified member-errors (`catch (Db.Timeout e)`) must pick `.` vs `::` consistently with part-2's rule.
- DEC-208 **JUSTIFIED** — the enhanced-PDO primitive over a baked-in builder; the whole Db execution stack shipped this run (drivers, hydration, streaming, transactions). Sub-rulings (error-mechanism prelude-wrapper, strict name-mapping, both bind styles) all JUSTIFIED and shipped. **PENDING retained: the retry SURFACE (`db.transactionRetry(fn, retries)`) awaits the developer's name/shape confirmation** → checkpoint.
- DEC-209 **JUSTIFIED**, shipped — bare-PascalCase rejection verified live this audit (closed C-10 and the #1 autonomous footgun).
- DEC-210 **JUSTIFIED** — statement-only `++`/`--` ratified; register corrected.
- DEC-211 **JUSTIFIED**, shipped — generic bounds, both halves sound.
- DEC-212 **JUSTIFIED** — tagged templates part-1 shipped; part-2 (html → first-party library on the primitive) pends DEC-218 execution; tracked.
- DEC-213 **JUSTIFIED**, shipped — single-sourced builtin-class list fixed a real spine break.
- DEC-214 **JUSTIFIED**, shipped (both parts) — `E-EMPTY-LITERAL` + `new List<T>()`; supersedes DEC-201 cleanly. Note: `new Set<T>()` still deferred (needs an Op) — tracked residue.
- DEC-215 **JUSTIFIED** — DI stays compile-time, L1/L2 refactor scheduled Ω-4/Ω-7; the DEC-208-consistency argument holds.
- DEC-216 **JUSTIFIED**, ruled-UNEXECUTED — vendor/manifest split to a companion tool not yet extracted [Verified: `phg vendor` still in FEATURES]; tracked build item.
- DEC-217 **JUSTIFIED** — `phg test` stays built-in (Rust/Go precedent; byte-identity culture).
- DEC-218 **JUSTIFIED**, ruled-UNEXECUTED — web-spine externalization to userland libs awaits the DEC-216 vendor path; tracked. The Http-primitive OOP note stands.
- DEC-219 **JUSTIFIED**, deferred — static overload resolution with the soundness carve-out recorded; a META-6 zero-cost item awaiting its slot.
- META-4 **JUSTIFIED** — two-SSOT consolidation (this audit lives by it).
- META-5 **OBSOLETE** (advisor situation changed with Fable; certification still disclosed per-session).
- META-6 **JUSTIFIED** — rich-core / zero-cost-sugar / no-bloat: the externalize audit and every ruling since applies it.

### §2026-07-13/14 follow-through (DEC-220…222)

- DEC-220 (+S3) **JUSTIFIED**, fully shipped — named sinks killed the context-magical Output redirect; `Output.capture` as an import-gated primitive (option d) over the leaking prelude wrapper honored nothing-in-the-wind (the reverted-then-reshipped record is exemplary). The ob_start-dangling PHP edge is disclosed in KNOWN_ISSUES.
- DEC-221 **JUSTIFIED**, shipped — throwing constructors (`new Db(dsn)` fail-fast ≡ PHP `new PDO`).
- DEC-222 **JUSTIFIED**, shipped — throwing-closure function types with the sound fewer-throws-substitutable variance; unblocked the transaction closure form.

### §2026-07-15/16 batches (DEC-223…238) — ratified this run, re-verdicted per DEC-237's reopen clause

- DEC-223 **JUSTIFIED** — Core.Mail per locked spec, shipped + tested (6 tests, injection-safe Address, DKIM).
- DEC-224 **JUSTIFIED** — Mongo shape ruled (sync driver, postgres precedent), build deferred; tracked.
- DEC-225 **JUSTIFIED** — concurrency PHP-leg hard error stands; Fibers spike queued (the one 3-leg exception remains loud + disclosed).
- DEC-226 **JUSTIFIED** — unchecked-overflow transpile exclusion stands; pack/unpack emulation rejected-with-reason.
- DEC-227 **JUSTIFIED** — `db` default feature + E-MODULE-UNAVAILABLE + E-TRANSPILE-DB (100-line error wall → one actionable code).
- DEC-228 **JUSTIFIED** — streaming (laziness proven by test); the rewrite_html/Expr::New P0 found during it is pinned by conformance.
- DEC-229 **JUSTIFIED** — MySQL driver + PG arrays; killed two silent-failure paths (withPassword no-op; mysql:// falling to SQLite).
- DEC-230 **JUSTIFIED** — mail deviations recorded honestly (ctor-default gap since CLOSED by DEC-236; the withAuth/at factories now thin aliases).
- DEC-231 **JUSTIFIED** — HttpClient with security gates (CR/LF, userinfo, 64MB cap); F-008 (connection reuse) tracked to D5.
- DEC-232 **JUSTIFIED** — Core.Fs typed taxonomy; Fs-prefixing lesson recorded; Core.File deprecation question queued (D2).
- DEC-233 **JUSTIFIED** — Core.Session secure-by-default; F-005 (partitioned) now ruled DEC-242.
- DEC-234 **JUSTIFIED** ruling, build queued — with the DEC-207-part-2 separator interaction noted above.
- DEC-235 **REVOKED** → DEC-239 (this audit).
- DEC-236 **JUSTIFIED**, shipped — ctor defaults with conformance golden.
- DEC-237 **OBSOLETE-EXECUTED** — the wholesale ratification's reopen clause is THIS audit.
- DEC-238 **JUSTIFIED**, shipped — Dumped<T> one-function dump; three-backend byte-identity pinned; erased-shape disclosure stands.
- DEC-239…244 — born of this audit, not re-reopened (they ARE the reopen).

### CONFLICTS table re-verdicts (C-1…C-10)

- C-1 (D-L3 vs MI) **OBSOLETE** — the contradicting spec files no longer exist (docs/specs = 4 files post-consolidation; D-L3 survives only in audit raws as history). Close.
- C-2 (foreach vs for-in) **→ F-009**, being triaged this checkpoint.
- C-3 (zero-dep locked framing) **OBSOLETE** — framing doc consolidated away; DEC-009 chain is the record. Close.
- C-4 (Text→String shadowing rationale) **RESOLVED-JUSTIFIED** — PascalCase `String` cannot shadow primitive `string` (case-distinct, checker-enforced); the original concern is structurally moot. Close with this note as the missing "conscious dismissal".
- C-5 (ternary same-day records) **OBSOLETE** — perimeter spec deleted in consolidation; DEFERRED verdict verified live (parse error). Close.
- C-6 (serve OS-thread pool) **RESOLVED** — per-worker heap isolation verified (src/serve/handlers.rs); superseded by green threads anyway. Close.
- C-7 (CLI verb doc drift) **RESOLVED** — CLAUDE.md/docs use `phg benchmark`/`format` today. Close.
- C-8 (E-INTERSECT-SIG revisit) **→ F-010**, being triaged this checkpoint.
- C-9 (intrinsics in the wind) **RESOLVED** — [Verified probe: bare `panic`/`assert` → import-required error]. Close.
- C-10 (bare `V =>` catch-all footgun) **RESOLVED** — [Verified probe: `Circle =>` in a union match → parse error "`Circle` is a PascalCase name used as a bare pattern binding — it would silently catch every value"] — the #1 AUTONOMOUS-HIGH-IMPACT item is CLOSED by the DEC-209 guard, which covers class patterns too. Close.

### AUTONOMOUS-HIGH-IMPACT list re-verdicts

1 (catch-all footgun) **RESOLVED** — see C-10 probe. 2 (foreach drift) **→ F-009**. 3 (totality
contours) **JUSTIFIED-BY-USE** — shipped, differential-pinned, no complaints in 3 weeks of dogfood.
4 (pattern-cluster syntax sweep) **JUSTIFIED-BY-USE**, formally ratifiable at run-end reopen.
5 (no-turbofish) **SUPERSEDED** — DEC-208-A shipped turbofish. 6 (UFCS breadth) **JUSTIFIED-BY-USE**
— `E-UFCS-AMBIGUOUS` guard held (forced exactly one rename). 7–8 (debugger UX, dogfood grammar)
**JUSTIFIED-BY-USE**. 9 (invariance retrofit) **JUSTIFIED** — a soundness fix; breaking-toward-correct
is the right direction. 10 (COW SetIndexLocal) **JUSTIFIED** — aliasing contract documented in
INVARIANTS. Footer note on DEC-096's unbuilt `W-SEQUENCE-MUTATION`: **OBSOLETE** — DEC-210's
statement-only correction removes the hazard the lint targeted.

## D2 — KNOWN_ISSUES reopen

> Every section re-verdicted. The file's discipline is exemplary — everything disclosed with
> reasons — but it carries HEAVY staleness: 17 entries superseded by later shipped work. Verdicts:
> the overwhelming majority of deferrals are **JUSTIFIED** (clean rejection + recorded reason +
> tracked follow-up — the exact pattern the mandate demands); listed below are only the STALE rows,
> the new FLAGS, and notable justified-divergence confirmations.

### Stale rows (all → D6 doc-fix batch)

1. Db hydration "contextual inference, NOT turbofish" → turbofish shipped (fable spine 1) and WINS over annotation.
2. transaction-retry "PENDING… developer to confirm" → ruled DEC-249 (method defaults; `transactionRetry` retires).
3. Reserved top-level names "OPEN, deferred (DEC-200)" → DEC-202 shipped the builtin-class guard.
4. `Output.capture` "a lambda cannot declare throws" (twice) → DEC-222 shipped throwing lambdas — the disclosure's premise changed; the ob_start-dangling divergence is now reachable via lambdas too (quarantine stance unchanged, text must update).
5. "MySQL/MariaDB (slice J) is not built" → DEC-229 shipped it.
6. Default params "free functions only; ctor = E-DEFAULT-PARAM-CONTEXT" → DEC-236 (ctors) + DEC-249 (methods ruled).
7. "No cycle collector… lands only if a need appears" → DEC-205 ruled the phased collector.
8. "No empty Map/Set literal" (dogfood + Maps sections) + "safe has/get awaits generics" + "Set itself still pending" → DEC-214 `new Map<K,V>()`; Map.get/getOrDefault/has and Core.Set shipped.
9. Error model 2b "a lambda cannot declare throws yet" + "method `throws` not discharged at call site" → DEC-222 + method-throws discharge shipped (the whole Db surface runs on it).
10. "core.list map/filter/reduce not yet available" + "Still pending on this path: the higher-order …" → shipped (S7b-3).
11. "No bounds" (generics ×2) → DEC-211 shipped `<T: Interface>`.
12. Interop internal contradiction: §M8.5-deferrals says ".d.phg + foreign-exception catch NOT yet implemented" while §Interop-header says "S3 ships" — the older text is the stale one.
13. Router contradiction: "Attributes are free-function-only" vs the adjacent "#[Route] on static methods works".
14. Intersection corners "no overloading YET… revisited after" → overloading shipped; revisit ruled DEC-245.
15. Bare-`DateTime`-not-gated entry "adjudicate before the DB/HTTP waves" → ruled DEC-206; and DEC-247 now builds the real DateTime.
16. Core.Time "UTC-only, no timezones (would break byte-identity)" → superseded-in-part by DEC-247 (timezones ruled IN); **build-note: the DateTime spec must pin a bundled tzdb version** (webpki-roots precedent) so the PHP twin and natives agree deterministically.
17. Fable-triage section: DEC-231's queued error-namespace ruling → ruled DEC-234 at the desk (entry can note it).

### New flags (checkpoint triage)

- **F-013 (MED)** — nullable unions inexpressible: `(A|B)?` and `A | B | null` are both rejected while **PHP expresses `A|B|null` natively**. A PHP-can/phorj-can't type shape.
- **F-014 (HIGH)** — the PHP-enforcement-ahead class (latent transpile-fatal, same shape the return-covariance fix killed): (a) override **parameter** compatibility unchecked; (b) `private`/`protected` **static field** external read unenforced; (c) member access through an **intersection-typed** receiver unenforced. In each, phorj compiles what PHP fatals on → the PHP leg can fatal where run/runvm succeed.
- **F-015 (HIGH UX)** — LSP diagnostics run the RAW checker (no prelude injection, no intrinsic-import resolution): every injected-type program shows spurious squiggles in editors while the CLI is clean. The editor lies about the flagship types (Option/Result/Json/Router/Db…).
- **F-016 (MED, design)** — no by-ref/`inout` params: PHP's `quicksort(array &$arr)` class of in-place cross-call mutation is inexpressible (the one benchforge port blocker). Needs an explicit ruling: reject-with-reason (functional idiom + shared-mutable instances cover it) or build `inout`.
- **F-017 (MED, correctness-audit)** — the fault-parity **exit-status sweep was never executed**: the recorded "REAL hazard" (a phorj-faulting native lowering to a PHP builtin that RETURNS instead of throwing → PHP silently succeeds) has an audit recipe written down and not run.
- **F-018 (MED)** — W4-4 Unicode: `String.length` is BYTE length (= PHP `strlen`, not the ruled codepoints-default), `upper/lowerCase` + `equalsIgnoreCase` are ASCII-fold only; ruled direction exists, unbuilt, unscheduled.
- **F-019 (MED, design)** — Iterator protocol: `DbStream`/`RowStream` cannot be looped (`foreach` over a stream needs the protocol DEC-228 queued); shape adjudication owed (now in DEC-248's foreach world).
- **F-020 (LOW, design)** — Db column-naming strategy (snake_case DB ↔ camelCase phorj) slice B2 awaits its surface ruling.
- Plus carried process items: I/O-native perf-gate carve-out confirmation · cargo-fuzz dep admission (parser/lift unwrap audit) · `var/phorj-app/` keep-or-delete · prelude-parse-failure loud assert (queued as a no-design P1 build item) · live legs (Mailpit/MySQL round-trips, developer-run) · serve keep-alive absence → D5 lever list.

### Notable JUSTIFIED confirmations (spot record)

`finally` cannot return a value (PHP allows it — known footgun, removed with reason) · panics
uncatchable (bugs ≠ recoverable) · strict `string as bool` (never PHP truthiness) · `List.sum`
faults on overflow (PHP silently widens to float) · no LSB `static::` (explicit override pattern,
recorded) · sealed whole-program model · regex non-regular subset rejected (ReDoS immunity) ·
Secret non-printable by type (stronger than PHP's `#[\SensitiveParameter]`) · float div-by-zero
faults (no IEEE inf leak) · `new` on package-Main fns colliding with PHP builtins documented ·
fmt guarantees meaning-preservation with tracked cosmetic gaps · parked-perf section is honest
(ceiling spike refuted its own red flag; loss baselines recorded, not hidden).

## D3 — Architecture

> Repo shape at baseline: 351 Rust files, 124,035 lines. The M-Decomp hybrid discipline is
> visibly applied (checker/vm/jit/format all sub-moduled; the `*_tests.rs` sibling convention is
> uniform across native/) — the architecture is fundamentally healthy. Findings below.

### D3.1 File-size cap violations [Verified: `wc -l` sweep this audit]

**Hard cap (1000) — 10 files:** jit/analyze.rs **2957** · checker/desugar_db.rs **2703** ·
native/db/mod.rs **2267** · jit/handles.rs **2104** · jit/emit_unboxed/mod.rs **1952** ·
jit/tests/verticals.rs **1847** · cli/preludes.rs **1750** (grew +390 during the office arc — every
new Core module inflates it) · cli/explain.rs **1727** (grows with every diagnostic) ·
transpile/runtime_php.rs **1116** (grows with every PHP twin) · jit/emit_unboxed/verticals.rs
**1025**. **Soft cap (800):** vm/exec.rs 983 · native/mail.rs 936.
Note: KNOWN_ISSUES' fmt/printer.rs 1680 entry is STALE — the printer was split (printer/exprs.rs
728 today) → D6. Structural observation: preludes/explain/runtime_php are **growth-coupled** files
(every feature adds to all three) — their split should be BY MODULE/TOPIC so future features add
files, not lines (kills the regrowth class, not just today's numbers).

### D3.2 Folder-structure findings (src/ root has 19 loose files)

- **F-021**: cohesion groupings proposed — (a) `manifest.rs + lock.rs + vendor.rs` → `src/package/`
  (pre-stages the DEC-216 companion-tool extraction — the boundary becomes one directory move);
  (b) `dap.rs + debug.rs + dump.rs + inspect.rs + profile.rs + mem.rs` → `src/devtools/` (the
  debugger/introspection family); (c) `token.rs` → `src/tokenizer/token.rs` (it IS the tokenizer's
  vocabulary). Root keeps the genuinely-core singletons (types, diagnostic, dispatch, phstr,
  limits, php_names, json).

### D3.3 Domain-coupling findings (the non-generic/opinionated lens)

- checker/desugar_db.rs (2703) + desugar_router.rs (536) + desugar_di/ (1292) = **~4500 lines of
  APPLICATION-DOMAIN compiler passes inside the checker** — the exact category DEC-215/the
  externalize audit ruled must become one generic L1 attribute-reflection primitive + L2 consumers
  (scheduled Ω-4/Ω-7). D3 confirms the finding and its ruled fix; the open question is only TIMING
  (the biggest single non-JIT compiler file is a domain pass).
- JIT = 5 of the 10 hard-cap violations — all spine-sensitive, each split needs a FRESH context
  (the standing M-Decomp rule).

### D3.4 Structural positives (recorded so they never regress silently)

Single `check_and_expand` chokepoint (sugar never reaches backends) · value kernels single-sourced ·
Op three-match discipline wildcard-free · native registry one-keyed · `# Sources:` header convention ·
CORE_MODULES registry (1 row per module) · Transport seam quarantining sockets · per-worker heap
isolation in serve · the *_tests.rs sibling convention.

## D4 — Security

## D5 — Perf ledger

## D6 — Docs unification log

### D2 checkpoint outcome

All 8 flags + 3 process items RULED: F-013→DEC-253 (nullable unions, both spellings) ·
F-014→DEC-251 (3 PHP-enforcement-ahead checks, HIGH) · F-015→DEC-252 (LSP injection fix, HIGH +
check≡LSP standing rule) · F-016→DEC-254 (slice 1b + `ref` copy-out params + mutability triad) ·
F-017→DEC-255 (fault-parity exit-status sweep, HIGH) · F-018→DEC-256 (W4-4 Unicode FULL package
now: codepoint length + Unicode case + grapheme family) · F-019→DEC-257 (Iterator interface) ·
F-020→DEC-258 (opt-in column naming) · perf doctrine→DEC-259 (bench everything with a PHP
equivalent + real-app macros; var/phorj-app is the instrument) · cargo-fuzz admitted (dev-only) ·
prelude-parse loud-assert queued (no-design P1). Standing rules recorded: META-7 (cross-language
scan; byte-identity-is-a-tool, always asked) · check≡LSP · transpile/lift always-current.
