# PHP-parity readiness — the wave after Slice 3

> The body of work ruled on 2026-09-02: **phorj must be able to do everything PHP does, better** —
> more secure, more object-oriented, faster, with more sugar — measured against three real PHP
> applications, a cross-language survey and the current PHP ecosystem, and built in the order
> harness-trust → readiness wave → perf roadmap. Nothing here is a port of any of those apps
> (developer ruling: the developer implements them later; phorj's job is to be READY).
>
> SSOT discipline (Invariant 19): rulings live in the Decisions Logs (this file, the consolidation
> plan, the gap-programme plan) and the register; MASTER-PLAN + SLICE-STATE mirror the cursor. The
> six raw inventories this plan was derived from are session artefacts
> (`scratchpad/raw/{issues,perf,scout-needs,twesin-needs,invoiceninja-needs,cross-language-delta,
> php-ecosystem-delta}.md`) — their CONTENT is folded into the tables below; the files are not the record.

## Decisions Log

- [2026-09-02 16:40] AGREED: no scout port; phorj readiness is the goal; all three streams authorised
  (readiness wave / open-bug drain / DEC-333 perf). *(Recorded in the consolidation plan; pointer.)*
- [2026-09-02 17:05] AGREED: REGEX option B; ORDER = harness trust → readiness wave → perf roadmap;
  perf internals are Claude's, `Json.getInt`-style surface is asked; panel re-run NOW. *(Pointer.)*
- [2026-09-02 17:40–17:55] AGREED (gap-programme Q23/Q16/Q3/Q18/Q2/Q17/Q19/Q21): doctrine =
  capabilities only; tz as pinned data; `Core.Net`+`Core.Mime`+read-only `Core.Imap`; typed `Charset`
  + `foldAccents`; `html5ever`+`selectors`; AEAD+Ed25519+HKDF; `Core.Compress` wired into HTTP;
  shell-free `Process.run`. *(Recorded in the gap-programme plan; pointer.)*
- [2026-09-02 18:15] AGREED (Part 4): DEC-455.5 memoize per entry; DEC-455.6 decline candidacy when no
  provider resolves; `ServeConfig` fields nullable; `decimal` bound/fetched as TEXT on the PHP leg.
  STANDING: LSP + both editors + transpile/lift are first-class per slice; cross-language scan;
  PHP-ecosystem scan. *(Pointers.)*
- [2026-09-02 19:40] RECORDED (not ruled): the consolidated delta below — every NEW row is an
  Invariant-15 question until it carries an AGREED line here.
- [2026-09-02 20:05] AGREED — **FRAMEWORK TIER lives in Core stdlib, staged**: first validation
  attributes (checked against the field type), CSRF, rate limiting, signed URLs, RFC 7807 problem
  details, OpenAPI generated from real types; then `Core.Queue` (jobs, retries, idempotency keys,
  scheduler with overlap/one-server locks) and a query-builder ORM over `Core.Sql` as their own
  slices. Storage/cache (A27) remain questions. Collapses A21/A23/A24 (+A22's seam is designed inside
  the ORM slice, tenancy-first).
- [2026-09-02 20:05] AGREED — **GENERATORS (X1)**: lazy adapters (`map/filter/take/zip/…`) land NOW on
  the shipped `Iterator<T>` as interface methods, transpiled via a `__phorj_iter_*` helper class
  (byte-identical, the `FileSystem.lines` precedent); `yield` stays queued at W4-2 with its
  byte-identity-vs-PHP-`Generator` proof obligation; UNIFIED-SPEC:1633's rejection is NARROWED to
  "lazy sequences that cannot transpile" (fibers stay rejected). Spec amendment in the adapters slice.
- [2026-09-02 20:05] AGREED — **XML (X6, widens DEC-382/Q1)**: one `Core.Xml` domain = DOM + XPath +
  C14N + XSD validation + XMLDSig (enveloped; RSA/ECDSA via the Q17 crypto primitives); XAdES profiles
  as a follow-on. The schema-capable crate admission is decided inside the slice. Transpile tier 1.
- [2026-09-02 20:05] AGREED — **GATE RULE (X9, amends DEC-268)**: two-consecutive-clean is replaced by
  **fix-then-verify**: fix every P0/P1 in step 1, freeze, run ONE panel round; CLEAN closes the
  milestone gate; P2/P3 residue is tracked, not blocking. DEC row to record the amendment.

## 1. Sources and their yardsticks

| source | size | what it demands of a runtime |
|---|---|---|
| scout (`/stack/projects/scout`) | 117 files / 27 303 lines / 1 272 tests | CLI watcher: PCRE2 regex, hand-rolled IMAP+SMTP over TLS sockets, MIME, HTML5 parse + selectors, SQLite, `--watch` sleep + signals, tz, `.env` |
| twes-in (`/stack/projects/twes-in/api`) | 133 files / 18 492 lines / 712 tests | invoicing API: exact decimal with 8 rounding modes, Postgres RLS tenancy through a DB **statement middleware seam**, gapless counters, attribute-driven REST + validation, RFC 7807, XLIFF i18n, UUIDv7, FrankenPHP binary |
| Invoice Ninja (`/stack/projects/invoiceninja/api`) | 2 522 files / 567k lines / 598 tests / 310 packages | Laravel SaaS: ORM+migrations, queues/scheduler, 10 mail transports, HTML→PDF via Chromium, sandboxed Twig, e-invoicing XML + XMLDSig, 27 payment drivers, OAuth/TOTP/WebAuthn, S3/Redis, CSV/XLSX/ZIP, 45 locales |
| cross-language survey | 24 NEW items vs `F-cross-language.md` + Appendix A | see §4 |
| PHP ecosystem (2026-09-02, fetched) | TypePHP v0.6.8, Manticore, elephc, PHP 8.5/8.6 RFCs, tooling | see §5 |

## 2. Delta A — capabilities PHP apps need that phorj lacks

Status key: **RULED-TODAY** (build queued by a 2026-09-02 ruling) · **QUEUED** (pre-existing row) ·
**NEW** (no row anywhere — an open question) · **PARTIAL** (exists, gap named).

| # | capability | needed by | status | row / note |
|---|---|---|---|---|
| A1 | PCRE-class regex (look-around, backrefs, branch reset, possessive) + compile-time literal validation on the transpile leg | scout core + `sources.json`; IN 276 uses | RULED-TODAY | REGEX-B; panel C1–C5/C11 fold in |
| A2 | HTML5 parse + CSS selectors + entity decode | scout, IN (Purify) | RULED-TODAY | Q2 |
| A3 | `Core.Net` TCP+TLS, `Core.Mime`, read-only `Core.Imap` | scout | RULED-TODAY | Q3 |
| A4 | tz-aware time (pinned IANA data), format patterns, RFC 2822 parse, `+1 month` arithmetic | all three | RULED-TODAY (tz) / QUEUED W4-5 (patterns) | Q16, DEC-247 |
| A5 | charset transcoding + `foldAccents` | scout, IN | RULED-TODAY | Q18 |
| A6 | AEAD / Ed25519 / HKDF | IN (37 encrypt sites), twes-in | RULED-TODAY | Q17 |
| A7 | gzip/deflate + HTTP wiring | IN, scout | RULED-TODAY | Q19 |
| A8 | shell-free process spawn | IN (Chromium PDF), scout | RULED-TODAY | Q21 |
| A9 | `sleep`, shutdown signal handler | scout `--watch` | QUEUED | W4-11, DEC-204 |
| A10 | XML DOM + XPath + **XSD validation + XMLDSig/XAdES signing** + C14N | IN e-invoicing, twes-in W4/5 | QUEUED (DOM, DEC-382) / NEW (XSD, XMLDSig) | Q1 scope must widen |
| A11 | Intl: CLDR plurals, locale number/currency/date formatting, collation, XLIFF catalogues, per-recipient locale | twes-in (3 locales), IN (45) | QUEUED (DEC-271, Q4 unscoped) | Q4 |
| A12 | money: BigInt/BigDecimal, currency-scale registry, largest-remainder allocation, `RoundingMode.Unnecessary`, scale guard, negative-zero normalisation, NUMERIC(27,12) round-trip | twes-in (440-line `Decimal.php`), IN (`BcMath.php`) | QUEUED W4-13 / PARTIAL (`decimal` i128: verify max scale vs (27,12)) | brick/money shape |
| A13 | UUIDv7 / ULID | twes-in, IN (hashids) | QUEUED Q10 | |
| A14 | PDF: HTML→PDF (via A8 + Chromium), merge/stamp, QR / Swiss-QR / barcodes, image resize | IN (~300 KB layout engine) | QUEUED Q8 (PDF) / NEW (QR, images) | Q22 `gd` |
| A15 | `.env` loading (parsed, validated, real env wins) + `putenv` | scout, IN, twes-in | NEW | `Core.Environment` has `get`/`all` only |
| A16 | JSON: typed error on malformed input (not `null`), list-vs-object distinction, `decodeInto<T>`, decimal-preserving numbers, streaming parse | all three (IN `json-machine`, twes-in money-as-string) | NEW / PARTIAL (`parseLines` exists) | ecosystem #17 |
| A17 | injectable `HttpClient` interface (fakeable in tests), cookie jar, per-host redirect policy, response-header callback | scout tests, IN 16 SDKs | NEW / Q12 pending | `HttpClient` is a concrete class |
| A18 | structured log fields (typed, PSR-3+) | IN (Monolog/GELF/Sentry) | NEW — **regression vs PSR-3** | cross-lang #4 |
| A19 | string builder (O(1) amortised append) | IN, scout | NEW — **regression vs PHP** | ties to `strappend` 0.44× |
| A20 | typed `Path` + `resolveWithin` traversal guard | IN storage, serve | RULED (App. B design) unbuilt | cross-lang #8 |
| A21 | **`Core.Queue`**: jobs, retries, at-least-once + idempotency keys, batches, scheduler with `withoutOverlapping`/`onOneServer`, distributed lock | IN (113 jobs, 30 scheduled), twes-in messenger | NEW — question | ecosystem #16 |
| A22 | **DB statement middleware seam**: intercept prepare/exec/query SQL text; transaction-local `set_config` + read-back; connection-release hook (`DISCARD ALL`); savepoint-rollback observability | twes-in RLS tenancy (HARD, security) | NEW — question | `Core.Database` has no interceptor |
| A23 | **ORM tier** over `Core.Sql`: relations, casts, soft-deletes, scopes, observers, composite keys, migrations tool, `lockForUpdate`, multi-DB sharding | IN (~60 % framework demand) | NEW — question (DBAL only today) | |
| A24 | **web-framework tier**: attribute-driven resources, declarative validation attributes, auth guards (session/token/OAuth), CSRF, rate limiting, signed URLs, RFC 7807, content negotiation, **OpenAPI generated from types** | twes-in (55 `#[Assert]`), IN (423 FormRequests) | NEW — question; partial (`Router`, middleware, `Validate`, `HeaderSafety` exist) | twes-in reads docblock generics at runtime to build OpenAPI — phorj's compiler can do it exactly |
| A25 | sandboxed user-authored template language + HTML sanitizer | IN (Twig sandbox, Purify) | NEW — question | ecosystem #18 |
| A26 | OAuth2 client flows, TOTP, WebAuthn (CBOR/COSE) | IN | NEW — question | |
| A27 | object storage (S3-class), Redis-class cache + locks, shared typed KV across workers | IN | NEW — question | ecosystem #19 |
| A28 | XLSX read/write, ZIP archives | IN import/export, backups | NEW — question (Q19 excluded archives) | |
| A29 | events/listeners/observers as stdlib | IN (271) | NEW — low (app pattern) | |
| A30 | gapless counter via `INSERT … ON CONFLICT … RETURNING` | twes-in | PARTIAL — verify `RETURNING` hydration on all three drivers | |
| A31 | persistent-worker global-state isolation (Octane leak class) | IN, twes-in gate | phorj by design (no ambient globals) — VERIFY with a serve test | "better" claim needs evidence |

## 3. Delta B — PHP flaws the three codebases work around, and phorj's answer

| flaw (evidence) | phorj today | gap |
|---|---|---|
| float money (`Number.php:29`, `Money::of` widening trap) | `decimal` exact, checked | A12 scale limits; decimal over JSON (A16) |
| ORM magic untyped — ~50 `@method` lines, phpstan L5 + baseline | compiler owns types | no ORM (A23) |
| `strict_types` per file; union coercion prefers `int` | always strict, no coercion | — |
| int silently → float past 2^63 | checked overflow, fault | — |
| bcmath truncates at scale; absurd scale allocates | `decimal` fixed scale | verify `MAX_SCALE`-class guard exists |
| negative zero two spellings | ? | VERIFY `-0.000d == 0d` |
| ambient time/globals banned by a token gate (twes-in) | no ambient globals; `Time.freeze` | — (this is the design) |
| PDO returns strings, `false` as failure | typed hydration, typed errors | — |
| `json_decode` silent `null`; `array_is_list` ×13/×241 | `Json.parse -> Json?` null | A16 |
| mutable `DateTime` — 104 `->copy()` | `Instant` immutable | — |
| at-least-once redelivery disabled (`retry_after` 2.8 y) | no queue | A21 — design idempotency in |
| `catch (\Throwable)` catch-alls; two deletion axes in one condition | checked `throws`, sealed enums | — |
| `set_time_limit` ×20, `memory_limit` as health metric | ? | VERIFY per-request budget in `phg serve` |
| dynamic properties `#[\AllowDynamicProperties]` | rejected | — |
| `strtotime` guessing on CSV dates | no `strtotime` (rejected) | A4 explicit parse formats |
| translator singleton "not octane safe" | worker model | A31 verify |

## 4. Delta C — cross-language (24 NEW; 4 are regressions vs PHP)

Regressions vs PHP first: **string builder** (A19), **structured log fields** (A18), **test fixtures
`setup`/teardown** (PHPUnit has them), **dependency audit** (`composer audit` exists). Then, ranked by
the reviewer: code coverage `phg test --coverage`; lazy iterator adapters (`map/filter/take` on
`Iterator<T>` — sits on the generators-vs-`UNIFIED-SPEC:1613` "lazy sequences rejected" contradiction,
which must be ruled before building); `E-IMPORT-CYCLE`; interface default methods; parametrized tests
`test "…" for (…)`; `Path.resolveWithin`; re-exports (`export`); `SortedMap/SortedSet`; `phg audit` /
`phg sbom`; error `.context()`; `phg test --filter` + `#[Tag]`; `LruCache`; reproducible builds; in-source
`bench "…" {}`; `char` type; named-field tuples; `#[Inline]`; (low) mutation testing, multimap, bitset,
sized ints — **sized ints recommended DECLINE** (no `u64` PHP mapping).
PARTIAL worth finishing: variadics on methods/lambdas (`E-VARIADIC-UNSUPPORTED` — a PHP regression);
`Core.Path` value type; named tuples; native turbofish; the adopted-never-scheduled pattern batch
(range / `@` / list patterns, XL-030/031/032). `while (var x = …) when …` SHIPS but is missing from
`FEATURES.md`.

## 5. Delta D — PHP ecosystem (fetched 2026-09-02)

- **TypePHP** (github.com/swoole/typephp, v0.6.8 2026-08-31, GPL-3): PHP → C++17 → native, keeps the
  Zend engine linked through **phpx** (their C++ Zend-API bridge — "phpx" is NOT a compiler). Subset:
  `main()` + declaration-only globals; adds `bigInt/decimal/bigFloat`, typed `std::vector/map`
  containers, derive-style `#[Getter]/#[With]/#[Printer]/#[Arrayable]`; **no type checker, no
  generics**; claims 6.5–10× vs interpreted PHP. **Manticore** (LLVM, reifies `@template`, 1.5–44×,
  2.6 ms cold start) and **elephc** (Rust backend, iOS targets) are the other live competitors.
- What phorj must learn / beat: (1) **source protection** — `phg build` embeds recoverable source
  (UNIFIED-SPEC:2286); ship the reserved `payload_kind=1` bytecode before AOT; (2) **monomorphized
  generics** as an explicit `--native` milestone; (3) W4-13 BigInt/Money as tier-1 (TypePHP's "cannot
  be run by php"); (4) unboxed `List<int>` + closure JIT — the typed-container 10× phorj cannot answer
  today (DEC-434); (5) run php-src `bench.php`/`micro_bench.php` as macro benches for an
  apples-to-apples claim; (6) the compat bar: "lift a real Composer project to green".
- PHP 8.5/8.6 features phorj lacks: `#[\NoDiscard]` (→ `E-DISCARDED-RESULT`, on by default for
  `Result`); general partial application `f(1, ?)` (8.6, implemented 33-0; phorj has `%` only inside
  `|>`); `Io\Poll` (fold into `Core.Net`); closures in const exprs (W4 queued); WHATWG `Url` variant;
  `grapheme_levenshtein`; `Time\Duration` tier-1 twin once the floor is 8.6.
- Tooling delta: architecture tests (Deptrac/Pest) → `phorj.json "architecture"` checked by `phg check`;
  mutation testing + coverage; data providers; `phg audit`/`--patch-only`; Psalm's `file`/`ssrf` taint
  sinks → typed `Path` + `HttpClient(Uri)` newtypes; expression-form `is` with bindings (pattern-matching
  RFC); `value class` (records RFC — nominal, not structural); HTML sanitizer; `#[Derive(...)]`.
- Already better than the ecosystem (with the URL that proves it): checked generics (PHP declined even
  erased ones 7-19, php.watch/rfcs/bound_erased_generic_types); uncolored `spawn` (True Async RFC
  cancelled); compile-checked `using` (Context Managers still in discussion); subtree `internal`;
  secure cookie/session defaults (PHP reaches them in 8.6).

## 5b. Sourcing and grades

Every row in §2–§5 is **agent-read from the three codebases, the survey and fetched pages, not
re-probed by the parent** unless marked. Parent-verified on the shipped binary or by grep, 2026-09-02:
regex look-around spine break (run/transpile/php); `String.lowerCase` byte-preserving; `Core.Environment`
= `get`/`all` only (A15); no HTML entity decoder; `phg build` embeds recoverable source
(UNIFIED-SPEC ~2286); `%` placeholder exists only inside `|>` (`parser/mod.rs:59`); **CSRF: zero hits
in `src/`** (A24); `decimal` = i128 fixed-point, `u8` scale (`src/value/decimal.rs`); lazy-sequences
rejection at UNIFIED-SPEC:1633 vs generators queued at MASTER-PLAN Ω-4 #8 / W4-2. Everything else is
[Inferred: agent evidence cited in the raw files] until its slice re-probes it.

## 5c. CONFLICTS with what the plan or code already says (the "different from plan" answer)

| # | conflict | what the evidence says | disposition |
|---|---|---|---|
| X1 | **Generators**: UNIFIED-SPEC:1633 REJECTS "lazy sequences/fibers (fight the eager-array transpile target)"; MASTER-PLAN Ω-4 #8 and W4-2 QUEUE `yield` + generators (XL, DESIGN, must prove byte-identity vs PHP generators) | two SSOTs disagree; blocks cross-lang lazy adapters (§4) | **ruling needed** |
| X2 | **Money representation**: twes-in measured and REJECTED scaled-integer money — but its stated reason is PHP-specific (`int` silently becomes float past 9.22e18, so NUMERIC(19,4) at 1e-4 needs 1e19). phorj's `decimal` is i128: 38 digits, so NUMERIC(27,12) (27 digits) fits with 11 to spare | conflict examined, **does not transfer**; what DOES transfer: bcmath-style truncation must be a fault, absurd scale must be refused, `-0.000d` must normalise — verify all three in the A12 slice | no ruling; verify |
| X3 | **Intl on ICU**: DEC-271 plans `Core.Intl` on icu4x; twes-in keeps its currency-scale registry deliberately OFF ICU because a library upgrade must not move a currency's scale | argues for a pinned, versioned currency table (like tz data, Q16) inside the money slice rather than ICU's CLDR currency data | **ruling needed** (small) |
| X4 | **Gapless numbering**: twes-in rejected Postgres `SEQUENCE` (non-transactional, burns numbers) for upsert+`RETURNING`; no phorj row mentions the pattern | not a conflict with a ruling; a missing capability check (A30) | verify `RETURNING` hydration on all three drivers |
| X5 | **Composer interop**: TypePHP runs existing Composer packages through its Zend bridge; MASTER-PLAN Appendix A.2 REJECTED FFI (`.d.phg` declarations are the seam) and the lifter produces a draft | a competitor's compat story challenges a ruled rejection | **surfaced, not ruled** — next batch |
| X6 | **XML scope**: DEC-382/Q1 ruled a `quick-xml`-class DOM + C14N question; both e-invoicing codebases need XSD validation AND XMLDSig/XAdES signing, which Q1 does not cover | scope widening, not contradiction | **ruling needed** with Q1/Q9 |
| X7 | **`\d\w\s` doc**: KNOWN_ISSUES:2484 says `\d` is ASCII-only in transpiled PCRE; the helper appends `u`, all legs agree (panel C11/F3b) | code right, doc wrong | docs pass |
| X8 | **Named arguments**: UNIFIED-SPEC:1209 "Phorj has no named arguments" — false for user functions/constructors (probed), true for natives only | doc wrong | docs pass |
| X9 | **Certification**: DEC-268 wants two consecutive clean panel rounds; the 2026-08-19 economize ruling wants one panel per milestone; round 3 returned 35 findings | decides whether a round 4 is owed after step 1 | **ruling needed** |

## 5d. Where phorj is WORSE than PHP today (consolidated)

Regressions a PHP developer would hit on day one, in one place: (1) `s = s + x` is O(n²) off the JIT
and there is no string builder; (2) `Core.Log` takes `(string)` only — no structured fields (PSR-3
has context arrays); (3) `phg test` has no fixtures/setup-teardown, no `--filter`, no data providers;
(4) no `phg audit` (Composer has `audit`); (5) variadics on methods/lambdas are `E-VARIADIC-UNSUPPORTED`;
(6) `Json.parse` returns `null` with no message (PHP has `JSON_THROW_ON_ERROR`) and cannot tell a list
from an object; (7) no `.env` loader; (8) `HttpClient` is a concrete class — not fakeable, no cookies,
no gzip (gzip ruled); (9) `phg --help` omits four verbs; (10) regex: look-around/backrefs (ruled B),
possessive/`\h`/`\R`/`\Z`, replacement syntax, `$` before final newline all diverge from PCRE
(panel C1–C5); (11) no time zones (ruled), no format patterns, no RFC 2822 parse; (12) no `sleep`, no
signal handler, no spawn (ruled), no IMAP/MIME/HTML parse (ruled). Items marked "ruled" have a build
queued by today's rulings; the rest are wave items or step-1 fixes.

## 6. Order (ruled) and the questions queue

1. **Harness trust** — panel round 3 disposition (consolidation plan § "Panel round 3").
2. **Readiness wave**, leverage order: REGEX-B → `sleep` + DEC-204 → tz (Q16) → `.env`/process/stderr
   (A15, Q21) → JSON (A16) → HTML (Q2) + XML (Q1, widened) → `Core.Net`/`Mime`/`Imap` (Q3) → HTTP
   client (A17, Q19) → charset (Q18) → crypto (Q17) → money (A12) → Intl (Q4). Each: example, LSP +
   both editors, transpile AND lift, flip-or-flag bench.
3. **Perf roadmap** (DEC-333) with TypePHP's benches as macro twins.

**Questions queue (Invariant 15, ≤4 per ask):** A21 queue · A22 DB seam · A23 ORM tier · A24 framework
tier · A25 templates/sanitizer · A26 auth · A27 storage/cache · A28 XLSX/ZIP · A10 XSD+XMLDSig widening ·
A14 QR/images · Q4 Intl scope · source-protection payload · generic bounds · PFA · `#[NoDiscard]` ·
`value class` · lazy adapters vs `UNIFIED-SPEC:1613` · sized-ints decline · coverage/mutation ·
`Json.getInt` surface (perf split).
