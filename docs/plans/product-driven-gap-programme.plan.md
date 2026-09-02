# Product-driven gap programme — rent-watch + twes-in

> **Rounds 1–3 of N: VERIFICATION + VISION GATING + THE ADJUDICATION BATCH.** Round 2 (§4b) probed the
> real PHP 8.5.8 gate oracle and the decision register; it **flipped two recommendations, refuted two of
> my own claims, and added one sub-question.** Round 3 (§4c) applies the developer's 2026-08-07 doctrine
> — *"all php does phorj must do and we must do it better"* — which **replaces the gating rule in §3**:
> the gap list is re-derived from PHP's own 975-function capability surface rather than from what two
> products asked for, and it turns up items **larger than anything in either requirement document**
> (dates-with-timezones, crypto beyond password hashing, charset transcoding, compression, process spawn).
> Read §4c before §3 — it supersedes §3's gating frame. No implementation, and no
> per-item spec yet — deliberately. Under Invariant 15 the dependency admissions and the surface
> shapes below are the developer's to rule, and a spec written against an unruled admission is a spec
> written twice. The NEXT round writes `docs/specs/*.md` for whatever survives the ruling.
>
> Source: two requirement documents produced independently by other CLIs against phorj at
> `1.0.0-nightly.0` / `b9e74cd` —
> `tmessaoudi-official/rent-watch:docs/PHORJ-REQUIREMENTS.md` (16.6 KB) and
> `tmessaoudi-official/twes-in:docs/PHORJ-REQUIREMENTS.md` (35.3 KB).
>
> **Both are unusually good** — they cite `file:line`, separate EXISTS from MUST-BUILD, mark questions
> as questions, and twes-in opens by refusing to be treated as a roadmap. Neither is accepted on
> trust: every claim below was re-verified against the tree on 2026-08-06 by three independent
> read-only sweeps, and **six claims did not survive**.

## Decisions Log

- [2026-08-06] AGREED: read both requirement docs, verify every claim, and gate each item on whether
  it moves phorj toward GA / 100% vision — not on a product having asked for it (developer:
  *"if it really gets us closer to our goals and ga and complete vision 100 % without any compromises"*).
- [2026-08-06] AGREED: no implementation until the adjudication batch (§4) is ruled.
- [2026-08-06] AGREED — **NO SILENT OMISSION OR DROP** (developer, verbatim: *"no silent omiton or
  drop. anything that does not go with our goals needs to be asked so i decide what to do with it"*).
  An earlier draft of §3 carried a Tier C whose rows read *"Recommend: decline"* — that is me ruling
  scope, which is the forbidden move under Invariant 15 and under this directive both. **Every item
  either enters the build queue or becomes a numbered question. Nothing is closed by recommendation.**
  Tier C is now a question list, and §5's "not in the sequence" list is now "pending a ruling".
- [2026-08-07] AGREED — **THE PARITY DOCTRINE** (developer, verbatim: *"all php does phorj must do and we
  must do it better"*). This **replaces §3's gating rule**: an item no longer earns its place by closing
  MASTER-PLAN §0.3 residual, it earns it by PHP having the capability. §4c re-derives the gap list from
  PHP 8.5.8's own 975-function surface, which turns up five items larger than anything either product
  raised. Two consequences are the developer's to rule before anything is built: the doctrine collides
  head-on with Invariant 10 on timezones (Q16), and read literally it reverses a dozen ruled language
  rejections (Q23) — so its BOUNDARY must be pinned, not guessed.
- [2026-08-07] ⚠ **RETRACTION — `4dcb76d`'s `mapinsert` conclusion is WRONG and must not be relied on.**
  That commit concluded *"the baseline 1.089 is real and CONSERVATIVE"* from a ten-run campaign reading
  1.014–1.242. It contradicts three independent sources I failed to read before concluding: (1) **DEC-431.1**,
  whose title is *"the ratchet BLOCKED a push, and it is right to: `mapinsert` was never a WIN … PENDING
  RULING — push held"*, with five quiet-box runs at load 0.33–0.44 giving **0.83 / 0.81 / 0.79 / 0.81 / 0.80**
  and an interleaved pinned control proving phorj's own leg unchanged at ~7.0 ms; (2) the gate's own header
  (`scripts/microbench-gate.sh:113-119`), which states mapinsert's *"true value re-measures at 0.80-0.85 on a
  quiet box"* and calls the emitted WIN a *"fiction"*; (3) **seven fresh readings taken 2026-08-07 on a
  BYTE-IDENTICAL binary** — 0.877 / 0.829 / 0.831 / 0.848 / 0.841 at loads 0.46→2.31 and at both K=3 and
  **K=9** (raising K refutes the sampling-artifact hypothesis, since it tightens the VM-side bound and the
  value did not move). Twelve readings across two dates now cluster at **0.79–0.88**; only `4dcb76d` (`mapinsert re-measured`, itself wrong)'s
  campaign says otherwise, and that is the same commit in which four numbers were fabricated. **The
  honest position: `mapinsert` is a real LOSS at ~0.84 and the 1.089 baseline is the artifact.** Checked and
  cleared: `4dcb76d` did **not** re-emit `bench/micro-baseline.json` (last touched by `6d71227`), so no
  laundering occurred. MASTER-PLAN carries `4dcb76d` (`mapinsert re-measured`) and its claim and needs the same retraction — held for a
  ruling rather than edited unilaterally, because DEC-431.1's status is *push held*.
- [2026-09-02 17:40] AGREED — **Q23 BOUNDARY: the doctrine covers CAPABILITIES only** (stdlib,
  runtime, I/O). Ruled language rejections stand with their recorded reasons (`ini_set` DEC-409,
  `eval`, `goto`, `$$x`, dynamic properties, ambient globals); a PHP program using them lifts to a
  diagnostic, not a feature. Nothing in this batch re-opens a language ruling.
- [2026-09-02 17:40] AGREED — **Q16 TIMEZONES: tz as pinned DATA.** The IANA database ships as a
  versioned, pinned table (tz crate per DEC-247); `Instant.at(Zone.of("Europe/Paris"))` is a pure
  function of (instant, tzdata) — deterministic, byte-identical on all three legs (PHP leg emits
  `DateTimeZone`; the differential pins the tzdata version). The AMBIENT zone stays excluded;
  Invariant 10 untouched. `src/cli/preludes.rs:295`'s "timezones are non-deterministic" rationale is
  to be rewritten to "the ambient zone is non-deterministic" when the slice lands.
- [2026-09-02 17:40] AGREED — **Q3 MAIL RECEIVE: build the trio.** `Core.Net` (TcpStream, implicit TLS
  + STARTTLS over the admitted rustls) as the shared floor; `Core.Mime` (multipart, QP, base64,
  RFC 2047, RFC 2822 dates, typed `Charset`); `Core.Imap` read-only by default (EXAMINE, UID
  SEARCH/FETCH, `uidValidity`, typed error taxonomy, file-backed `.eml` transport). All three
  native-only tier 2: `E-TRANSPILE-NET`/`-MIME`?/`-IMAP` + differential quarantine + disclosure.
  IDLE/APPEND/flag writes out of scope. Narrows and closes DEC-413's deferral.
- [2026-09-02 17:40] AGREED — **Q18 CHARSETS: typed `Charset` enum + `Encoding.decode/encode`
  (`encoding_rs`, format-parsing domain; scope UTF-8/16, Latin-1/9, Windows-1252, ASCII) + a
  transpilable `String.foldAccents`** (pure table → `__phorj_fold_accents` helper, byte-identical).
  NFD/full ICU stays in DEC-271's `Core.Intl` scope.
- [2026-09-02 17:55] AGREED — **Q2 HTML5 PARSING: admit `html5ever` + `selectors` (Servo).**
  `Html.parse` (lenient, never throws on bad markup), `Html.select/selectOne` (tag/class/id/descendant/
  attribute selectors, scoped to any `Node`), `Html.text` (whitespace-normalised), `Html.attribute`
  returning `string?` (absent ≠ empty), `Html.decodeEntities` as a standalone pure function over the
  full HTML5 entity table. Transpile tier 1 via `Dom\HTMLDocument` + `querySelectorAll` (the oracle
  ships lexbor). Unblocks the `tidy` deferral; XML C14N (Q1) shares the DOM shape. New domain =
  untrusted-input parser, already admitted by name for regex.
- [2026-09-02 17:55] AGREED — **Q17 CRYPTO: AEAD + Ed25519 + HKDF via RustCrypto, misuse-resistant.**
  `Crypto.seal/open` (nonce generated and prefixed, no mode selection, typed `Key`), `Crypto.sign/verify`
  (Ed25519, detached), `Crypto.deriveKey` (HKDF). Transpile tier 1 via `sodium_crypto_aead_*` /
  `sodium_crypto_sign_*`, byte-identical on fixed vectors. X.509/CSR out of scope (own question if ever).
- [2026-09-02 17:55] AGREED — **Q19 COMPRESSION: build `Core.Compress` over `flate2`** (gzip/deflate/
  raw, decompression-bomb size cap → typed fault) **and wire the HTTP client (`Accept-Encoding: gzip,
  deflate`, transparent decode) and `phg serve` (compress when accepted).** Transpile tier 1
  (`gzencode`/`gzdecode`). Closes DEC-407's ruled-unbuilt row. zip/tar archives separate and unruled.
- [2026-09-02 17:55] AGREED — **Q21 PROCESS SPAWN: shell-free `Process.run(program, args)`** with
  captured stdout/stderr, exit code, timeout (kills the child → typed fault), env, cwd; typed
  `ProcessResult`; NO string-to-shell form exists. Transpile tier 1 via `proc_open` with an argv array.
  Examples spawn only deterministic programs (Invariant 10). Streaming pipes/PTYs out of v1 scope.

---

## 1. What the requirement docs got WRONG

Listed first because four of these are items the docs asked us to *build* that already exist, and one
of them we caused.

| Their claim | Verified reality |
|---|---|
| rent-watch **Q1: can `HttpClient` set request headers?** — *"the single most important answer on this page"*; *"if request headers cannot be set, Track 1 is blocked too"* | **YES.** `send(method, url, headerNames, headerValues, body)` — parallel lists (`src/ext/http_client/prelude.rs:100`; native `natives.rs:224-241`; header write loop `natives.rs:104-127`). Empirically verified against a loopback echo server: custom `User-Agent`, `Referer`, `Accept`, `Content-Type`, `X-Custom` all arrived on the wire. **Track 1 was never blocked.** |
| rent-watch **Q2: is the timeout configurable?** | **YES.** `HttpClient.timeout(ms)` (`prelude.rs:83,87,91`), default 30 000, applied as connect+read+write timeouts (`engine.rs:75-82`). Caveat: per-socket-op and re-applied per redirect hop, so total wall time can exceed it. |
| twes-in **④ "no CSPRNG exposed; randomness exists only in `src/ext/session/natives.rs`"** | **WRONG — `Core.Random` shipped in W3-4 (`f4c4c1d`).** `secureBytes(n)`/`secureInt(min,max)` over `/dev/urandom`, **rejection-sampled (no modulo bias)**, emitting PHP `random_bytes()`/`random_int()` (`src/native/random.rs:118-185`). It even satisfies their "no seeding API on the secure one" requirement — the seeded xorshift64 is a deliberately separate surface. The session extension is a *consumer*. |
| **BOTH: "`Core.Database` has not adopted `Closable`; `db.close()` is manual (DEC-203 a separate slice)"** | **WRONG — `class Connection implements Closable` (`src/ext/database/prelude.rs:281`), shipped 2026-07-31 as DEC-364, which explicitly CLOSED DEC-203's deferral.** ⚠ **And this one is our fault:** `examples/README.md:238` still says *"`using`/`Closable` auto-close is DEC-203, a separate language slice."* Two independent readers repeated our stale doc. Fix it regardless of everything else here. |
| twes-in **Q1: can we run `set_config(…, true)` / `current_setting()` for RLS?** — *"this is the entire tenancy design"* | **YES**, via `db.prepare("SELECT set_config('x.y', ?, true)").bind(v).query()`. `?`→`$1` translation is quote-aware (`postgres_sql.rs:172-226`). ⚠ A **literal `$1` does not work** — zero binds means zero params and a pg arity error. |
| twes-in **Q6: is `statement_timeout` settable?** | **YES** — `Connection.timeout(ms)` issues a real `SET statement_timeout` on PG (`postgres.rs:271-277`). |
| rent-watch: *"phg refuses 18 domains"* | **4.** Seven `E-TRANSPILE-*` codes have real quoted-string sites; only DB / SESSION / HTTPCLIENT / MAIL are domain refusals. UNICODE is per-function, UNCHECKED an attribute gate, VARIANT-COLLISION a naming error. |
| rent-watch: `Core.Env` | It is **`Core.Environment`**. |

**Process lesson worth keeping:** every one of these was a *negative* ("X does not exist") stated
without a control. Two independent, careful, evidence-citing readers produced six false negatives —
several because **our own docs are stale**, not because they were careless. Our doc rot is now
externally visible.

---

## 2. What we GENUINELY lack — verified absent

**Confirmed absent:** IMAP + MIME · HTML parsing / CSS selectors · XML · PDF · i18n (catalogues,
CLDR plurals, RTL) · UUID · accent folding · `RoundingMode.Unnecessary` · native `NUMERIC` / `uuid` /
`timestamptz` binding · PostgreSQL TLS · schema migrations · process spawn · HTTP-client cookies ·
HTTP response streaming · **a timed `sleep`**.

Two nuances the docs stated too strongly or too weakly:

- **`sleep`** — rent-watch says *"there is no way for a phorj program to pause."* Narrower: there is
  no **timed self-controlled** pause. Blocking I/O does pause (`Input.readLine()`), and an HTTP call
  to a black hole pauses up to `timeoutMs`. But a busy-wait burns a core, so their conclusion holds:
  **`scout run --watch` is not writable today.**
- **Savepoint observability** — filed as a *question* (twes-in Q2). It is a **blocker**. `control()`
  bypasses `with_hook` entirely (`ops.rs:356-360` vs `:320-351`), so `onQuery` never sees
  `SAVEPOINT` / `RELEASE` / `ROLLBACK TO`. The only seam is `Connection.transactionDepth()`. Their
  reproduced cross-tenant read would exist in a phorj port **with no available fix**.

### 2b. Defects found that neither document knew about

| Defect | Evidence | Why it matters |
|---|---|---|
| **HTTP client emits DUPLICATE headers** | `engine.rs:19-33` guards `Host`/`Content-Length` but unconditionally writes `Connection: close` and `Accept-Encoding: identity`. Verified on the wire: two `Connection:` and two `Accept-Encoding:` lines when the caller sets their own. | Lands squarely on the feature rent-watch depends on most. The CR/LF injection gate is careful; the duplicate case slipped through. |
| **No default `User-Agent`** | Same file — none is emitted. | Many servers 403 a UA-less client. rent-watch's 5-of-11 403 measurement may partly be *this*, not bot detection. |
| **PostgreSQL `sslmode` silently downgrades to plaintext** | `sslmode` reaches tokio-postgres, whose `Prefer` default returns a plaintext stream (`connect_tls.rs:23-25`). phorj has zero code for it — the only `sslmode` occurrence in the repo is a redaction test string. | Worse than "no TLS": write nothing → cleartext, no warning; write `sslmode=require` → a confusing connect error. **Directly contradicts our own `Core.Mail` TLS-or-refuse posture (DEC-265).** |
| **Binding a decimal to `NUMERIC` looks broken outright** | `pg_param` boxes a `String` for `Type::NUMERIC` (`postgres_sql.rs:34,40`), but `postgres-types`' `ToSql for &str` excludes `NUMERIC`. **Zero test coverage** — no `numeric` in any PG test or example. | Not merely "non-native" as the doc says. *[Suspected — source-level inference; needs a build with `database-postgres` + a live PG to confirm.]* |
| **`import Core.Bogus;` type-checks clean** | `phg check -e 'package Main; import Core.Bogus;'` → OK. | Unknown imports are silently accepted; the error surfaces later as a misleading `unknown identifier`. Bad DX, and it makes capability probing unreliable. |
| **Stock binary lacks 4 modules** | `http-client`, `mail`, `database-postgres`, `database-mysql` are all off by default. | Anyone evaluating "can phorj do X" against a stock build gets **false negatives on four modules** — plausibly a contributor to §1's errors. |
| **DEC-403 naming default: ruled, not built** | Spec says the column-naming default flipped to `Naming.SnakeToCamel`; the prelude still defaults to `Naming.Exact()` (`prelude.rs:163,289,299`). | Unrelated to these products; found in passing. Not dropped — it is **Q14**. |

---

## 3. VISION GATING — the developer's actual question

> *"if it does not affect our goals and if it really gets us closer to our goals and ga and complete
> vision 100 % without any compromises"*

The frame is `MASTER-PLAN.md` §0.3, which states the residual honestly: **the ratified projections top
out at ≈75% parity / ≈81% vision after W6 — the planned work as modeled does NOT reach 100%**, and the
residual sits in ledger items 5–7. So an item earns its place by closing residual, not by having a
consumer.

**Why only Tier C gets questions.** Tier A is work **you have already ruled** — DEC-382, DEC-271, and
two named gap-ledger rows; re-asking would be re-opening a ruled decision without new evidence, which
the protocol forbids. Tier B is not new scope at all: each row is a shipped feature that is *wrong*, and
"do not fix a bug we shipped" is not a scope option. **But their SURFACES still are yours** — a new
`RoundingMode` variant, the savepoint seam's shape, and the `NUMERIC` binding are all user-visible, so
Round 2 proposes each surface in its spec and it is ruled *there*, not assumed here. If you disagree
with any Tier A or Tier B placement, say the row number and it becomes a question too.

### Tier A — ALREADY ruled roadmap work, now with a concrete consumer. Build.

| Item | Roadmap position | Verdict |
|---|---|---|
| **XML (+ namespaces, C14N)** | **Gap-ledger item 6 — the ONE open Wave-4 fork.** DEC-382 already RULED: *"admit a vetted `quick-xml`-class crate as the 15th dependency… best parity-per-effort item left."* | **Strongest item on the page.** It is *literally* one of the three enumerated residuals to 100%. twes-in supplies the consumer and — more valuably — the **ordering constraint**: build C14N with item ⑦ in mind or XAdES later becomes a rewrite of the XML module rather than an addition. That constraint is free now and expensive retrofitted. |
| **i18n / `Core.Intl`** | Gap-ledger item 5; **icu4x already admitted (DEC-271, a policy amendment)**. | On-roadmap. twes-in adds the sharp requirement: **CLDR plural categories**, because Arabic has six and `count == 1 ? a : b` is simply wrong there. Not "full ICU MessageFormat" — just the plural rules and a fallback chain. |
| **`sleep` / timed pause** | Gap-ledger item 2, Runtime pack — *"Cli/Process enrichment"*. | On-roadmap, tiny, and it is rent-watch's **headline mode**. Best effort-to-value ratio in either document. |
| **Schema migrations** | Gap-ledger item 2, Data pack — *"migrations/ORM-lite/Serialize"*. | On-roadmap. Note twes-in's warning: a diff-generated migration **cannot know about row-level security**, so a tenant table can look finished and be completely unpoliced. Whatever we build, the gate belongs beside it. |

### Tier B — GAPS IN SHIPPED FEATURES. Build; these are parity holes, not new surface.

These do not need a vision argument. A shipped feature that is wrong or unreachable is a parity
*deduction*, and closing it raises the denominator honestly.

| Item | Why it is a hole, not a feature |
|---|---|
| **PostgreSQL TLS** | The TLS domain was admitted **2026-07-03**; rustls is already in the tree; `postgres-rustls` adds **no new trust store and no new domain**. A shipped DBAL that can only reach `localhost` is an incomplete DEC-208, and the silent-plaintext behaviour contradicts DEC-265. **No policy ruling required — only a crate row.** |
| **HTTP-client duplicate headers + no default UA** | A live bug in a shipped module (§2b). |
| **Native `NUMERIC` (+ `uuid`, `timestamptz`)** | DEC-208 shipped a DBAL whose own guidance — *"store decimal columns as TEXT"* — steers new code into a shape that cannot carry scale, cannot be summed by the server, and admits `'abc'`. That guidance is the compromise the developer said not to make. |
| **Savepoint observability seam** | DEC-340 shipped savepoint nesting; the absence of any seam makes a *correctness* bug unfixable in user code. |
| **`RoundingMode.Unnecessary`** | `Core.Decimal` charter, one variant + a fault path in `round_div`. It is how a money type says "this must be exact" — turning a silent mis-allocation into a fault at the operation. |
| **Stale/false docs** | `examples/README.md:238` (DEC-203), the `timeout()` prelude doc (SQLite-centric, wrong on PG), `E-TRANSPILE-SERVE` listed as live. **We demonstrably misled two external readers.** |

### Tier C — NOT on the roadmap today. **Nothing is declined here — each row is a question in §4.**

Per the no-silent-drop directive, this table no longer carries verdicts. It carries the *evidence* for
each item and the question number that will decide it. My lean is stated inside each §4 question as
option 1, where it can be overruled in one word.

| Item | Evidence gathered | Decided by |
|---|---|---|
| **PDF generation** | **No row anywhere** — not in MASTER-PLAN, EXTENSIONS, FEATURES, not even a deferral. **twes-in argues it down itself**: its Option A is Gotenberg-over-HTTP, which needs *nothing built* provided a binary response body works (it does — `Response.body` is `bytes`). PDF/A-3 for Factur-X is a specialist compliance target. | **Q8** |
| **XAdES / XML signatures** | Large, specialised, "use a library" — twes-in says so itself. The only hard requirement it extracts is that item 6 ship with C14N, which Q1(b) already carries. | **Q9** |
| **IMAP + MIME** | **DEC-413 DEFERRED with a stated reason**: PHP unbundled `ext/imap` to PECL in 8.4, so it is not parity-critical. rent-watch's counter is real — it blocks their whole Track 2, and the proposed scope is genuinely narrow (read-only, no IDLE/APPEND/flags). Cost: a new dependency domain **plus** MIME/charset decoding, which is squarely the *format-parsing* class the policy excludes. | **Q3** |
| **HTML parsing** | Not in the residual. But a real debt sits underneath: `MASTER-PLAN.md:108` justifies deferring `tidy` because *"the W4-10 HTML5 parser subsumes it"* — **and that parser does not exist**. A deferral leaning on unbuilt work. rent-watch needs it twice (adapters *and* email bodies). | **Q2** |
| **UUID** | No row anywhere. Cheap now that `Core.Random` exists. twes-in's warning is worth carrying if built: a v7 id is an **ordering artefact, never a secret** — its random field is incremented between same-millisecond siblings, so treating one as unguessable is a vulnerability. | **Q10** |
| **Accent folding** | rent-watch says explicitly it will build its own and is not blocked. A *transpilable* `String.foldAccents` stays in the pure tier (unlike `unicodeUpper`, which is `E-TRANSPILE-UNICODE`) and is broadly useful beyond these products. | **Q11** |
| **Client cookies / keep-alive** | Keep-alive is already gap-ledger item 4 perf debt (DEC-266). Cookies are documented as out of v1 scope. rent-watch needs cookies for exactly **one** source (AL'in). | **Q12** |
| **Response streaming** | Real — the whole body is materialised twice. No consumer beyond "a year's export". | **Q13** |
| **DEC-403 naming default: ruled, not built** | Found in passing, unrelated to either product: the spec says the column-naming default flipped to `Naming.SnakeToCamel`; the prelude still defaults to `Naming.Exact()` (`prelude.rs:163,289,299`). Fixing it is a **user-visible behaviour change to a shipped feature**. | **Q14** |

---

## 4. THE ADJUDICATION BATCH — Invariant 15, developer-only

**This is the acceleration lever.** Every large item is gated on a *policy* decision, not on
engineering. Ruling these one at a time serialises the programme behind fourteen separate
conversations; ruling them as a batch is the single biggest schedule compressor available.

Each question states its own after-state. **All twenty-three are PENDING — nothing here is ruled, and
nothing outside this list was closed by me.** Q1–Q7 are the load-bearing policy calls (a dependency
domain or a user-visible semantic hangs on each). Q8–Q14 are the smaller ones that an earlier draft
would have quietly declined; they are cheap to answer — often one word — and each names my lean first.
**Q16–Q23 come from Round 3's doctrine** and are the heaviest of the three groups: two of them (Q16
timezones, Q23 the doctrine's boundary) collide with already-ratified invariants and rulings, so they
gate the others.

**Q1 — XML crate admission: confirm DEC-382's slot, and confirm C14N is in v1 scope.**
DEC-382 already ruled "admit a `quick-xml`-class crate as the 15th dependency" (and DEC-407 then took
the 16th for `flate2`). Two sub-questions remain: (a) is `quick-xml` still the pick, or does the
namespace/C14N requirement argue for a DOM-capable crate; (b) **is C14N v1 or deferred?** twes-in's
argument for v1: without it, signatures later become a rewrite rather than an addition.
*After (a+b in v1):* the one open Wave-4 fork closes with the signature path left open.

**Q2 — HTML5 parsing: new domain, or decline?**
The policy excludes *format-parsing* crates. The counter-argument is the **regex precedent**: an
HTML5 parser over attacker-controlled markup is exactly *"an untrusted-input parser where a safe
engine cannot be built in std"* — the sub-domain the policy already admits by name. Also relevant:
our `tidy` deferral already leans on this parser existing.
*After (admit):* rent-watch's Track 1 `type: html` adapters and Track 2 email parsing both unblock,
and the `tidy` deferral stops being a promise against vapour.
*After (decline):* rent-watch is JSON-sources-only; say so plainly so they can plan.

**Q3 — IMAP: does DEC-413's deferral still hold given a concrete consumer?**
The deferral reason (PHP unbundled `ext/imap`) is about *parity*, and rent-watch's need is not a
parity need — it is "this product cannot exist without it". Scope proposed is genuinely narrow
(read-only, `SEARCH`, MIME-decoded bodies, a file-backed transport; explicitly no IDLE/APPEND/flags).
Cost is a new domain **plus** MIME/charset decoding, which is format-parsing.
*After (build):* rent-watch Track 2 becomes possible; phorj gains a receive side to `Core.Mail`.
*After (hold the deferral):* rent-watch Track 2 is dead and should be told now, not later.

**Q4 — `Core.Intl` scope: full icu4x, or CLDR plural rules + fallback chain only?**
icu4x is admitted (DEC-271) but the surface is unscoped. twes-in needs exactly two things: a locale
fallback chain, and **CLDR plural categories** (Arabic's six). It explicitly does *not* ask for ICU
MessageFormat.
*After (minimal):* a small, shippable `Core.Intl` that unblocks a real consumer.
*After (full):* a much larger slice with no consumer for most of it.

**Q5 — Is `sleep` native-only, and does `Time.freeze` suppress it?**
Invariant 14 ladder: a timed pause has a faithful PHP analog (`sleep`/`usleep`), so this is **ladder
case 1 — it transpiles**, and no `E-TRANSPILE-*` is owed. rent-watch proposes something better: with
the clock frozen, `sleep` becomes a **no-op**, so watch-loop tests do not actually wait. That pairs
elegantly with the existing test clock — but it is user-visible semantics and therefore yours.
*After (freeze suppresses):* watch loops are testable in-process; a frozen clock now changes control
flow, not just readings.
*After (independent):* simpler and more predictable; tests must inject their own pause seam.

**Q6 — Does `phorj.json` grow a runtime nested-config story, or is `Core.Json` the answer?**
`Core.Config` does **not** exist (a decision record REJECTED it: *"external-dep policy bars a YAML
crate; config-as-typed-phorj matches the roadmap"*). `#[Config]` gives compile-time typed nested
injection; `Core.Ini` is flat; `Core.Json` is the only recursive runtime tree.
*After (Json is the answer):* say so in the docs and close the question — rent-watch's Q4 preference
order already puts "use what exists" first.

**Q7 — Postgres TLS posture: match `Core.Mail`?**
No policy ruling needed for the crate. But the *posture* is a design decision: `Core.Mail` is
TLS-or-refuse (DEC-265). Should Postgres refuse plaintext by default, honour `sslmode` faithfully, or
warn?
*After (TLS-or-refuse):* consistent with our own mail decision; a plaintext DSN needs an explicit
opt-in and the silent downgrade dies.

### Q8–Q14 — the items I would otherwise have dropped

Added under the no-silent-drop directive. Each carries my lean, and each is cheap to overrule.

**Q8 — PDF generation: permanent decline, defer with a row, or build?**
No row exists anywhere today. twes-in's own preferred route (Gotenberg over HTTP) needs nothing built.
*After (decline + document the HTTP route):* one paragraph in EXTENSIONS saying PDF is an out-of-process
concern and how to reach it; the question stops being re-asked by every future reader.
*After (defer with a row):* it enters the ledger as a named non-goal with a reason, which is what
DEC-413 did for IMAP and is strictly more honest than silence.
*After (build):* a very large specialist surface (PDF/A-3, ZUGFeRD profiles) with one consumer.

**Q9 — XAdES / XML signatures: decline, or reserve a slot behind C14N?**
*After (decline, honour the ordering constraint only):* Q1(b)'s C14N leaves the door open at zero cost;
we simply never walk through it.
*After (reserve a slot):* a queued row that says "possible once C14N ships", so the XML module's shape
is reviewed against it rather than only against parity.

**Q10 — UUID: userland, stdlib row, or nothing?**
Cheap now that `Core.Random` ships. The risk if built is a *documentation* risk, not an engineering one:
someone treats a v7 id as a secret.
*After (stdlib row, v4 + v7, with the "never a secret" warning in the doc comment):* small, useful
beyond these products, and the warning is carried where it will actually be read.
*After (userland):* both products write ~20 lines each; we carry nothing.

**Q11 — `String.foldAccents`: adopt into the `Core.String` charter?**
Transpilable (stays in the pure tier), broadly useful, and rent-watch is *not* blocked on it.
*After (adopt as a charter row):* it lands whenever `Core.String` is next touched — not on a product's
account, and not as a new slice.
*After (decline):* every consumer hand-rolls a fold table, and they will disagree with each other.

**Q12 — HTTP client cookies + keep-alive: fold into the existing perf slice, or split out?**
Keep-alive is already gap-ledger item 4 (DEC-266). Cookies are documented out of v1 scope; rent-watch
needs them for exactly one source.
*After (fold both into the DEC-266 slice):* no new roadmap row, and the connection-reuse work that
cookies naturally sit beside gets done once.
*After (split cookies out now):* rent-watch's AL'in source unblocks earlier, at the cost of touching
the client twice.

**Q13 — Response streaming: KNOWN_ISSUES entry, or a slice?**
The whole body is materialised twice. Real, but no measured consumer.
*After (KNOWN_ISSUES + revisit on a measured case):* honest and cheap; Invariant 18's NO-HIDDEN-LOSS is
satisfied because the loss is *recorded*, not hidden.
*After (a slice now):* a streaming body type ripples through `Response`, the serve loop and the client.

**Q14 — DEC-403 naming default: build the ruled flip, or re-rule it?**
Off-topic for both products, surfaced because it is a **ruled-but-unbuilt user-visible default**: the
spec says `Naming.SnakeToCamel`, the code still does `Naming.Exact()`.
*After (build the flip):* code matches the ruling; existing programs relying on `Exact` change behaviour,
so it needs a CHANGELOG line and probably an example.
*After (re-rule to `Exact`):* the spec is corrected instead, and the divergence closes the other way —
also valid, and cheaper.

### Q16–Q23 — raised by the doctrine, not by either product (Round 3)

**Q16 — TIMEZONES: how does the doctrine resolve against Invariant 10?** THE question of this round.
`Core.Time` is UTC-only and the source says why: *"timezones are non-deterministic and would break the
byte-identity spine"* (`src/cli/preludes.rs:295`). PHP has full IANA tz + DST.
*After (tz as pure DATA, recommended):* admit the IANA database as a **versioned, pinned data table**
rather than a system-clock read — `Instant.at(Zone.of("Europe/Paris"))` is then a pure function of
(instant, pinned-tzdata), so it is deterministic, byte-identical, and *better than PHP*, whose result
depends on whatever tzdata the host happens to have. Invariant 10 survives intact because nothing
non-deterministic is read; what was excluded was the *ambient* timezone, which stays excluded.
*After (stay UTC-only):* Invariant 10 untouched, and phorj cannot render a local time — which no
business application can accept, so this option is honest only if paired with telling users to convert
at the edges.
*After (ambient tz like PHP):* parity, and Invariant 1 breaks — the same program prints differently on
two machines.

**Q17 — CRYPTO: how far past `hashPassword` do we go?** Today `Core.Cryptography` is argon2 only; PHP
ships `openssl` + `sodium`. This is the policy's **first admitted domain** (*"never roll your own"*), so
the crate side is uncontroversial — the *scope* is yours.
*After (AEAD + Ed25519 + HKDF, recommended):* authenticated symmetric encryption (misuse-resistant API
— nonce generated for you, no ECB, no bare CBC), detached signatures, and key derivation. Covers the
overwhelming majority of application crypto and is *better than PHP*, whose `openssl_encrypt` lets you
pick a broken mode.
*After (+X.509/CSR parsing):* much larger, and needed only for certificate tooling.
*After (hold at argon2):* phorj cannot encrypt anything at rest.

**Q18 — CHARSET TRANSCODING + folding (subsumes Q11).** `Core.Encoding` is base64/hex only; PHP has
`iconv` + `mbstring`.
*After (recommended):* `Encoding.decode(bytes, Charset.Windows1252): string` + the reverse + a
transpilable `String.foldAccents`, with a **typed `Charset` enum** rather than PHP's stringly-typed
`"WINDOWS-1252"` — a typo becomes a compile error instead of a silent mojibake, which is the "better".
Scope to the charsets that actually occur (UTF-8/16, Latin-1/9, Windows-1252, ASCII), not all of ICU.

**Q19 — COMPRESSION, and DEC-407's unbuilt admission.** `flate2` was admitted (DEC-407) and is **not in
`Cargo.toml`** [Verified]. PHP has zlib.
*After (recommended):* build gzip/deflate/raw over `flate2` as `Core.Compress`, wire it to
`Accept-Encoding` in the HTTP client and the serve loop (which today advertises `identity` only), and
close the ruled-unbuilt row. Archives (zip/tar) stay separate and unruled.

**Q20 — the two "partial" rows: WHATWG URL, and MIME-from-content.**
*After (recommended, both):* add a WHATWG normalization mode + IDN/punycode beside the existing RFC 3986
`Uri` (PHP 8.5 now ships both, so parity means both), and add content-sniffed MIME with the **security
posture stated**: for uploads, trust the content and never the extension — PHP's `fileinfo` gives you the
rope to do either, and doing only the safe one is the "better".

**Q21 — PROCESS SPAWN.** `Core.Process` is argv+env only; PHP has `proc_open`/`exec`.
*After (recommended):* a typed, **shell-free** `Process.run(program, args): ProcessResult` with explicit
argv (no string interpolation into a shell), captured stdout/stderr, exit code, and an optional timeout.
That is *better than PHP*, where `exec("… $userInput")` is the single most common RCE in the language.
Ladder case 1 (`proc_open` exists), but Invariant 10 means examples must spawn only deterministic
programs.

**Q22 — the not-on-any-roadmap block: `gd`/images, `ldap`, `soap`, `xsl`, `ftp`, `gettext`, `gmp`.** PHP
bundles all of them, so the doctrine reaches them, but they are large and several are legacy.
*After (recommended — split it):* `gmp` (arbitrary-precision **integers**, a small gap beside the shipped
`Core.Decimal`) and `gettext` (subsumed by Q4's `Core.Intl` catalogues) are **in**; `xsl` follows XML if
Q1 builds; **`gd`/`ldap`/`soap`/`ftp` are declined with a recorded reason** — SOAP and FTP are legacy
protocols PHP itself no longer promotes, LDAP is enterprise-specific, and image manipulation is a
genuinely separate discipline. Each decline gets a named ledger row, DEC-413-style, never silence.
*After (all in):* a very large programme with no consumer for most of it.

**Q23 — the doctrine's BOUNDARY: capabilities, or language features too?** Read literally, *"all php
does"* reverses ruled rejections: `ini_set` (DEC-409 — action at a distance, breaks Invariants 1 and 10),
enum inheritance (DEC-410 — rejected on soundness with seven languages agreeing), gradual typing, `eval`,
`goto`, `$$var`, `&` references, self-hosting (DEC-273).
*After (capabilities only — recommended):* the doctrine means *"every DOMAIN PHP can work in, phorj can
work in, better"*, and the ruled language-level rejections stand — they are all cases where phorj is
better *by not* having the feature, which is the same goal.
*After (literally everything):* say so and I will re-open each rejection with a spec; several are
mutually exclusive with Invariant 1, so that answer needs to say which invariant yields.

---

## 4b. ROUND 2 — what probing the ORACLE and our own register changed

Round 1's recommendations were leans. Round 2 tested them against the actual PHP 8.5.8 transpile-floor
oracle and against the decision register. **Two recommendations flipped, two of my own claims were
wrong, and one new question appeared.** Everything below is a live command's output, not recall.

### The oracle probe — `Dom\HTMLDocument`, CSS selectors and C14N are all in PHP 8.5 core

[Verified 2026-08-06, `/stack/tools/phpbrew/php/php-8.5.8/bin/php`, one program, output pasted:]

```php
$d = Dom\HTMLDocument::createFromString("<table><tr><td>Rue A<td>1200 &euro;</table>", LIBXML_NOERROR);
foreach ($d->querySelectorAll("td") as $c) echo "cell: ", $c->textContent, "\n";
$x = Dom\XMLDocument::createFromString('<a xmlns:z="urn:z"  b="2" a="1"><z:c/></a>');
echo "c14n: ", $x->documentElement->C14N(), "\n";
```
```
cell: Rue A
cell: 1200 €
c14n: <a xmlns:z="urn:z" a="1" b="2"><z:c></z:c></a>
```

Read what that output proves, precisely: the **HTML5 error-recovery algorithm ran** (unclosed `<td>`s,
an implied `<tbody>`, an entity decoded); **CSS selectors work**; and **C14N canonicalized** — it sorted
`b="2" a="1"` into `a="1" b="2"` and expanded the empty element, which is the whole of what
canonicalization means. Extension census on the same build: `dom` YES · `libxml` YES · `bcmath` YES ·
`mbstring` YES · **`intl` no** · `imap_open` no · `mailparse` no.

**So XML+C14N and HTML5+selectors are both Invariant-14 ladder case 1 — they transpile, faithfully, on
the oracle we actually gate against.** Neither owes an `E-TRANSPILE-*`, a quarantine, or a disclosure.

### What that flips

- **Q2 stops being a policy-exception argument and becomes a PARITY argument.** PHP ships a
  spec-compliant HTML5 parser and CSS selectors *in core*; phorj cannot parse HTML at all. That is
  phorj being **behind PHP** on a mainstream capability — the one thing the whole project is against.
  The policy collision also resolves better than I framed it: the exclusion list names *"JSON, TOML,
  YAML, HTTP parsing"*, and we implemented every one of those in `std` (`Core.Json`, `Core.Ini`,
  `Core.Csv`, the HTTP wire reader) — so that clause has a consistent meaning: **small, unambiguous,
  non-recovering grammars.** HTML5 is categorically different: a ~120-page *error-recovery* state
  machine with insertion modes and the adoption-agency algorithm, fed attacker-controlled markup. That
  is the **regex shape** — *"an untrusted-input parser where a safe engine cannot be built in std"* —
  which the policy admits **by name**. My Round-1 lean (decline) was wrong on the evidence.
- **Q1(b) C14N-in-v1 stops resting on twes-in's word.** `DOMNode::C14N()` is core and verified working,
  so the PHP leg of canonicalization costs nothing. C14N in v1 is now recommended on our own evidence.
- **A NEW sub-question, Q1(c) — re-opening DEC-382, with the new evidence stated.** The same test that
  admits HTML5 argues *against* an XML crate: **XML is draconian by specification** — any
  well-formedness error is fatal, there is no recovery algorithm — so XML is the *JSON* shape, not the
  *regex* shape, and clause 1 as written points at a `std` implementation. DEC-382 admitted a crate
  before that distinction was drawn. New evidence: (1) the policy's own recovering-vs-format split;
  (2) XML's non-recovery, which is a spec property, not an opinion; (3) our four in-`std` format
  parsers as precedent; (4) `DOMNode::C14N()` making the transpile leg free either way.
- **Q4 gains a hard constraint I did not have.** **`intl` is NOT compiled into the gate oracle.** Any
  transpiled output calling `MessageFormatter` would fail our own differential today. There *is*
  precedent for leaning on an optional extension — the decimal leg emits `bcadd`/`bcdiv`/`bccomp`
  (`src/transpile/runtime_php.rs:209-317`) — but bcmath is near-universal and ICU is not, and
  `src/transpile/tests.rs:327` already pins that other emitters stay free of `mb_`/`ctype_`/`iconv`.
  So the recommendation sharpens: **emit a `__phorj_plural_*` helper carrying the CLDR rules inline**
  (Invariant 16 explicitly sanctions this, and requires the trade be surfaced — it is, here), which
  keeps ladder case 1 and pushes no ICU requirement onto anyone running transpiled phorj.
- **Q5 gets cheaper than I said.** `sleep`/`usleep` are present, and `Time.freeze` **already
  transpiles** as `__phorj_now_freeze()` (`src/native/time.rs:92`) — the frozen flag is *already* on the
  PHP side. Making `sleep` consult it is a few lines in an existing helper, symmetric across all three
  legs.
- **Q3 hardens.** `imap_open` absent, `mailparse` absent: IMAP is unambiguously ladder case 2,
  native-only, owing `E-TRANSPILE-IMAP` + quarantine + disclosure. That is the honest price tag.

### Two claims of mine that Round 2 refuted

1. **`Core.Mail` is NOT "TLS-or-refuse".** DEC-265 (`C-decisions.md:1605`, shipped block `:1912`) is
   *"SMTP **requires TLS when credentials are set**"* — unauthenticated sends stay `Opportunistic` so
   local fakers work, the only escape is a loud `allowInsecureAuth = true`, and an unrecognized mode
   value **fails safe to required-TLS**. I stated it as a blanket refusal in Round 1 and built Q7's
   recommendation on that. The real precedent is *better* and transfers exactly: **a Postgres DSN
   carrying a password requires TLS; a passwordless local connection stays opportunistic.** Q7 is
   re-recommended on the corrected precedent.
2. **DEC-403 is not an unbuilt divergence — it is RULED and BUILD-QUEUED**, *"queued with
   DEC-398/399"*, and the register already specifies the migration (`examples/database/*.phg`
   re-baselined, `naming.phg` rewritten to show the default first). Calling it a defect in §2b
   overstated it. Q14 shrinks to a scheduling question.

Also worth recording, because it changes Q7's dependency framing: `postgres` is admitted with **TLS
deliberately left off** (`Cargo.toml:111-116` — *"TLS left off → no OpenSSL"*), and `rustls` +
`webpki-roots` are already in the tree for `http-client`. Adding Postgres TLS is therefore **one bridge
crate inside two already-admitted domains** (SQL + TLS) — not a new domain, and no new trust store.

And for Q6: `Core.Json` is a genuine recursive ADT — `enum Json { Null(), Bool, Int, Float, String,
Array(List<Json>), Object(Map<string, Json>) }` (`src/cli/preludes.rs:15`) — so nested runtime config is
already expressible with an exhaustive `match` today. Task **#60** (`Json.getInt`/`getString` accessors)
is a pending ergonomics ruling layered on top, **not** a blocker for Q6.

---

## 4c. ROUND 3 — the doctrine changes the GATE, so the gap list is re-derived from PHP itself

Developer directive, 2026-08-07, verbatim: *"all php does phorj must do and we must do it better"*.

**That is a different and stricter gate than §3 used.** §3 asked *"does this close MASTER-PLAN §0.3
residual?"* — a roadmap question. The doctrine asks *"does PHP do it?"* — a **parity** question, and it
does not care which product asked. So the honest response is not to re-rank §3's list: it is to
**enumerate PHP's actual capability surface and diff ours against it.** Round 3 does that, and the result
contains items **larger than anything in either requirement document**, none of which the products raised.

### The census — what the gate oracle actually ships

[Verified 2026-08-06, `php -m` and `get_defined_functions()` on `php-8.5.8`: **975 internal functions,
217 classes**, extensions —] `bcmath Core ctype date dom fileinfo filter hash iconv json lexbor libxml
mbstring pcre PDO pdo_sqlite Phar posix random Reflection session SimpleXML SPL sqlite3 standard
tokenizer uri xml xmlreader xmlwriter OPcache`.

Two of those are **new in PHP 8.5** and land squarely in gaps of ours: **`lexbor`** (the HTML5 engine
behind `Dom\HTMLDocument`) and **`uri`** (a native RFC 3986 *and* WHATWG URL API). PHP is not standing
still, and it just moved into two places we are empty.

Bundled with PHP but not compiled into *this* build (so absent from the oracle, though present in a
normal distribution — a distinction that matters for what we may emit): `curl openssl intl sodium zip
zlib gd pgsql mysqli soap xsl ftp ldap exif gettext gmp bz2 calendar pcntl xmlrpc tidy`.

### The diff — every row [Verified] against `src/` on 2026-08-06

| PHP capability (core/bundled) | phorj today | Verdict under the doctrine |
|---|---|---|
| `date` — `DateTimeImmutable`, **`DateTimeZone`/DST**, `DateInterval` (month/year-aware), `date()` patterns, `strtotime` | `Core.Time` — `Instant`/`Duration`/`Date`, **UTC-ONLY BY DESIGN** (`src/cli/preludes.rs:295`), `toIso`, `addDays`, civil conversions. Pure phorj, so byte-identical by construction | **The marquee collision — see Q16.** No timezones, no pattern formatting, no flexible parsing, no calendar-aware month arithmetic |
| `openssl` + `sodium` — AES-GCM, RSA/Ed25519, X.509, sign/verify, key derivation | `Core.Cryptography` = **`hashPassword`/`verifyPassword` only** (argon2). `Core.Hash` = `hmac`/`hkdf`/`pbkdf2`/`equals` | **Largest security-shaped gap.** No symmetric encryption, no asymmetric anything, no certificates, no signing — see Q17 |
| `iconv` + `mbstring` — charset transcoding, multibyte string ops | `Core.Encoding` = **`base64`/`hex` only**. No transcoding at all | **Gap — see Q18.** ISO-8859-1/Windows-1252 → UTF-8 is table stakes for mail, legacy CSV and scraped HTML |
| `zlib`/`bz2`/`zip`/`Phar` — compression + archives | **Nothing.** And `flate2` is **not in `Cargo.toml`** despite DEC-407 admitting it | **Gap + a ruled-unbuilt admission — see Q19** |
| `lexbor` / `Dom\HTMLDocument` — HTML5 parse + CSS selectors | `Core.Html` is **emit-only**: `text`/`raw`/`render`/`attribute`/`element`/`voidElement`/`concat`. No parser, no selectors | **Gap — Q2, now doctrine-forced** |
| `dom`/`SimpleXML`/`xmlreader`/`xmlwriter` + `C14N()` | **Nothing** | **Gap — Q1, doctrine-forced** |
| `uri` (8.5) — RFC 3986 **and** WHATWG URL | `Core.UriModule` — RFC 3986 only (`parse`/`resolve`/`with*`/`equals`) | **Partial.** No WHATWG normalization, no IDN/punycode — see Q20 |
| `fileinfo` — MIME from **content** (magic bytes) | Extension-based only (`src/serve/static_files.rs`) | **Gap — see Q20.** Extension-trust is also a security posture question for uploads |
| `proc_open`/`exec`/`pcntl` — spawn, pipes, signals to children | `Core.Process` = `arguments`/`get`/`all` — **argv + env only, no spawn** | **Gap — see Q21** |
| `intl` — collation, CLDR plurals, locale dates, transliteration | **Nothing** | Gap — Q4, and note the oracle lacks `intl`, so we cannot *emit* it (§4b) |
| `gd`/`imagick`, `ldap`, `soap`, `xsl`, `ftp`, `gettext`, `gmp` | **Nothing** | **Not previously on any roadmap — see Q22** |
| `PDO`/`sqlite3`/`pgsql`/`mysqli` | `Core.Database` (SQLite + PG + MySQL) | ✅ at parity |
| `session`, `hash`, `random`, `filter`, `SPL`, `Reflection`, `tokenizer`, `pcre`, `bcmath`, `json` | `Core.SessionModule`, `Core.Hash`, `Core.Random`, `Core.Runtime.Validate`, `Deque`/`PriorityQueue`/`List`/`Map`/`Set`, `Core.Reflect`, `Core.Regex`, `Core.Decimal`, `Core.Json` | ✅ at parity or better (typed, exhaustive-matched) |

### Three consequences of the doctrine that only the developer can rule

These are not gap rows. They are what the doctrine *implies about our own rules*, and each one collides
with something already ratified — which is exactly the Invariant 15 surface.

1. **Timezones vs Invariant 10.** `Core.Time` is UTC-only *on purpose*, and the reason is recorded in
   the source: *"timezones are non-deterministic and would break the byte-identity spine."* PHP has full
   IANA tz + DST. The doctrine says we must have it **and do it better**. Both cannot hold as written —
   Q16.
2. **"Better" implies new dependency admissions.** Doing dates-with-tz properly needs the IANA database;
   crypto needs a vetted primitive (RustCrypto/ring — *"never roll your own"* is the policy's own first
   admitted domain); compression needs `flate2`. Each is a **policy amendment**, not a crate row — Q17/Q19.
3. **Does "all php does" mean CAPABILITIES or also LANGUAGE FEATURES?** Read literally it reverses
   ruled rejections: `ini_set` (DEC-409 rejected — action at a distance, breaks Invariants 1 and 10),
   enum inheritance (DEC-410 rejected on soundness, with seven languages agreeing), gradual typing,
   `eval`, `goto`, `$$var`, `&` references, self-hosting (DEC-273). I do **not** believe you meant those,
   but the doctrine as phrased covers them, and guessing is the forbidden move — Q23.

### Round 3's effect on the existing batch

- **Q1 (XML) and Q2 (HTML5) are answered by the doctrine** — PHP does both, in core, so phorj must.
  What survives is only *how*: Q1(a) crate-vs-`std`, Q1(b) C14N-in-v1 (recommended yes, the transpile
  leg is free), Q1(c) whether DEC-382's crate slot is still the right shape.
- **Q3 (IMAP) is NOT answered by it** — PHP **unbundled** `ext/imap` to PECL in 8.4, and `imap_open` is
  absent from the oracle [Verified]. PHP core does not do IMAP, so the doctrine does not reach it.
  DEC-413's deferral survives Round 3 intact.
- **Q8 (PDF), Q9 (XAdES)** — no PHP core equivalent (`pdflib` is PECL, `xmlseclibs` is userland). Not
  doctrine-forced. Recommendations unchanged.
- **Q10 (UUID)** — no PHP core UUID either (`ext-uuid` is PECL). Not forced; recommendation unchanged.
- **Q11 (accent folding)** — PHP does it (`iconv //TRANSLIT`, `Normalizer`). **Now doctrine-forced**, and
  it folds naturally into Q18's transcoding surface rather than standing alone.
- **Q12 (cookies), Q13 (streaming)** — `curl` does both, and is bundled. **Now doctrine-forced**, which
  upgrades them from "fold into a perf slice" to real parity rows.
- **Q7 (Postgres TLS)** — `pgsql`/`pdo_pgsql` support `sslmode`. **Doctrine-forced**, and "better" is
  precisely the DEC-265 fail-safe posture, which PHP does *not* do (PHP's `Prefer` default downgrades
  silently — the identical bug we have).

---

## 4d. ROUND 4 — `mapinsert` root-caused. The loss is REAL, and the fix is a named slice.

Developer directive: *"work on the perf, don't accept it"*. So this round stops classifying the loss and
attacks it. Every number below is pasted from the run that produced it.

### 1. The loss is real, reproducible, and my earlier "it's load noise" reading was wrong

Interleaved, quiet box (**load 0.07**), php invoked with the gate's own JIT flags
(`-dopcache.enable_cli=1 -dopcache.jit_buffer_size=128M -dopcache.jit=tracing`, `jit.on === true`
probed):

| round | phorj | php+JIT | ratio |
|---|---|---|---|
| 1 | 7.29 ms | 5.86 ms | 0.803 |
| 2 | 6.83 | 5.74 | 0.842 |
| 3 | 7.17 | 5.90 | 0.823 |
| 4 | 6.92 | 5.93 | 0.856 |
| 5 | 6.92 | 5.79 | 0.837 |

**A trap worth recording:** running php WITHOUT the JIT flags gives phorj a 3.4–4.7× *win*
(phorj 6.8 ms vs php 23.1 ms). That comparison is meaningless — G-8 is against release-php **+JIT** — and
it is exactly the shape of mistake that would let someone "disprove" a real loss. Always probe
`opcache_get_status()["jit"]["on"]`.

### 2. Root cause, by controlled experiment rather than inspection

`Value::Map` is `Rc<Vec<(HKey, Value)>>` (`src/value/types.rs:147`) — an **association list**, so
`m[k] = v` is an O(n) scan. Varying ONLY the number of distinct keys, everything else identical:

| distinct keys | phorj total |
|---|---|
| 1 | 4.93 ms |
| 2 | 4.88 ms |
| 4 | 5.34 ms |
| 8 | **6.91 ms** |

So **~2.0 ms of the 6.9 ms (29%) is the linear scan**, at ~0.57 ns per comparison, against a ~4.9 ms
floor for everything else. To beat php's 5.8 ms the scan has to *go away*, not get cheaper.

### 3. Two of my own optimisation hypotheses, both REFUTED — recorded so they are not retried

- **"Give short literals the cached-hash/pointer fast path."** Wrong premise: `phstr.rs:404` asserts
  *"Literals are always heap (interned + hash-cached), even short ones"*, so these keys are already
  `Heap` and `PhStr::eq`'s `Rc::ptr_eq` fast path already fires. There was nothing to win.
- **"Stop cloning the key on every lookup/overwrite."** Real waste — `map_index`/`map_set` projected the
  index into an owned `HKey` *before* scanning, spending an `Rc` inc+dec per operation purely to compare.
  Implemented behind a new `HKey::matches_value` with a 169-pair agreement test pinning it to
  `from_value` (including the `"ab"` vs `"ab\0"` case, where the inline buffers are identical and only
  the length differs). Measured: **best-of-7 6.800 ms vs 6.83 ms before — 0.4%, inside noise. FLAT.**
  **REVERTED**, not banked: Invariant 11 wants a measured before/after, the measurement said zero, and a
  null-effect change to the single-sourced value kernel is risk without benefit.

### 4a. DEC-431.1 IS NOW RULED AND EXECUTED (2026-08-07) — `mapinsert` carried as OWED at 0.851

Developer ruled Q-A option ①. Executed as a **quiet-box re-emit** (load 0.11, inside the stricter
`MICROBENCH_EMIT_MAX_LOAD=0.7` bar; `MICROBENCH_RUNS=7`), which is the *only* sanctioned route —
`_owed` is DERIVED at `--emit`, never hand-maintained, and the file says a feature *"leaves this list by
being FIXED and re-emitted, never by being edited out."* Direct precedent: `6d71227` (`floatloop never won — the ratchet armed a lucky draw; quiet-box re-emit`) (DEC-434.1,
*"floatloop never won — the ratchet armed a lucky draw; quiet-box re-emit"*), the same class of fix.

**Result: `mapinsert` 1.089 (fictional WIN) → 0.851 OWED**, matching all thirteen independent readings
(0.79–0.88). Gate now **PASSES**: 43 WIN / 10 loss / 10 OWED / **0 blocking regressions**, all
output-identical. The push is unblocked honestly, not bypassed.

**Every row that moved is listed here, because a re-emit is exactly where laundering would hide:**

| row | before → after | assessment |
|---|---|---|
| `mapinsert` | 1.089 → **0.851 OWED** | the ruling's whole point; 13 readings agree |
| `mapget` | 1.042 → **0.953 OWED** | a SECOND row that was never a real win. Direct re-measure: 0.931/1.031/0.951/1.032/0.957 — straddles 1.0, and OWED is the conservative direction |
| `floatloop` | 0.776 OWED → **1.014 WIN** | **defensible, not laundering** — re-measured 1.059/1.028/1.028/0.859/1.013, and DEC-434.1's 0.776 predates this session's JIT work (DEC-445/446, tasks #57/#59). A real fix landed |
| `floatmul` | 0.989 OWED → **1.001 WIN** | ⚠ **NOT defensible as a WIN — see the hazard below** |
| `userhof` | (absent) → 11.458 WIN | was reported every run as *"not in baseline (new)"*; now snapshotted |
| 8 others >15% | `floatarith` −26%, `intadd` +53%, `listmap` −24%, `hofpipe` −17%, `objalloc` +17%, `methodcall` −16%, `webish` −17%, `forin` +16% | no code changed, so this is harness spread — see the hazard |

### ⚠ HAZARD RECORDED, not silently accepted: the emit arms flip-checks on coin-flip rows

`floatmul` is now baselined at **1.001** and therefore *armed* by the WIN→LOSS flip check, while five
direct readings give 0.972 / 0.998 / 0.990 / 0.955 / 1.050 — it straddles 1.0. **That is the exact
pathology that produced this session's blocker**: `mapinsert` was armed at 1.089 and then false-blocked a
docs-only push for hours. Arming a row whose true value is 1.00 ± 0.05 guarantees a future false block.

Worse, **ten rows moved >15% with no code change between two baselines both emitted on a "quiet" box** —
so the emit is not reproducible to better than roughly ±20%, and the previous baseline (`6d71227`, `floatloop never won … quiet-box re-emit`) was
itself labelled a quiet-box emit. The new baseline is therefore *more honest* about `mapinsert`/`mapget`
but is **not authoritative to better than ~±20%** on any row, and this file should not be read as claiming
otherwise.

**Proposed fix (a harness change, so PENDING a ruling — Q-C):** give the flip check a **dead band** — do
not arm a row whose emitted ratio is within ±5% of 1.0; carry it in a `_marginal` list that is reported
loudly every run but never blocks. A row leaves `_marginal` by measuring a *robust* win. This keeps the
ratchet's teeth for real regressions (a 2.4× row falling to 0.9 still blocks) while removing the class of
false block that has now cost two sessions.

### 4b. ⚠ ROUND 5 RETRACTS §4 BELOW — the fix I proposed ALREADY EXISTS, and my root cause was wrong

**Read this before §4.** §4 (written minutes earlier, commit `54c5efb` (`round 4 — mapinsert root-caused to the O(n) map scan`) concluded the cost was the O(n)
association-list scan in `map_set` and proposed adding a hash index to `Value::Map` — 100 sites, 30+ files.
**Both halves are wrong, and I would have done a large invasive change for zero gain.**

**What I failed to check:** which leg actually runs. [Verified, quiet box, load 0.07:] `phg run` **6.822 ms**
· `--no-jit` **256.9 ms** · `--tree-walker` **546.6 ms**. The JIT is **37× faster than the VM here**, so
the benchmark's hot loop *never touches `map_set`* — the association-list scan I measured and blamed is on
a path this micro does not execute. I ran that probe **after** writing the diagnosis.

**What is actually there:** `src/jit/handles/mod.rs` documents `UB_TAG_AMB` as *"the **mapinsert
vertical**: a MUTABLE `Map<string,int>` … converted from a sealed flat map by the first `m[k] = v`
(`Op::SetIndexLocal`)"*, whose record buffer is a **PACKED open-addressed bucket table**
`{canon: u64, value: i64}` at load ≤ ½, followed by `count` rank canons in **INSERTION order**. That is —
precisely — the ordered hash map §4 proposed building. It already exists, it is already O(1), and
`seal_flat_entries` builds the same structure for sealed flat maps. §4's "the machinery exists and the map
does not use it" was exactly backwards.

**The re-measured decomposition** [Verified, best-of-3 each, same box]:

| variant | time | what it isolates |
|---|---|---|
| 8 keys, inserts + periodic reset (the shipped micro) | 6.822 ms | — |
| 8 keys, no periodic reset | 6.710 ms | reset + flat→builder conversion ≈ **0.11 ms** |
| **8 keys all pre-inserted — every op a fully-inline overwrite** | **6.171 ms** | the insert/helper path ≈ **0.55 ms** |
| 1 distinct key | 4.93 ms | — |
| **php+JIT** | **5.74–5.93 ms** | — |

So: **even with every operation on the fully-inline fast path, phorj is still ~6.17 ms against php's
5.8 ms.** The insert helper and the per-cycle conversion together are only ~0.66 ms of the 6.8. And the
1-key vs 8-key gap (4.93 → 6.17 ms) persists *in the all-overwrite variant*, where no algorithmic
difference exists at all — so that scaling is **probe distance and cache locality** (eight distinct key
slots, hash/canon loads spread across lines) rather than the O(n) scan I attributed it to.

**Verdict, stated plainly: NO WIN FOUND, and no algorithmic one appears to be left.** The data structure
is already correct. The residual is ~7–17% on a ~6 ms loop against a zend hash that is genuinely
well-tuned for this exact shape, and closing it means instruction-level work on the emitted probe (canon
layout, key-slot packing, hoisting the table base out of the loop) with uncertain payoff. That is a real
perf slice with a measurement-first discipline, not something to claim in advance.

**What this round DID buy:** it stopped a 100-site invasive change to the single-sourced value kernel that
would have added a second hash index behind the one that already exists. The recurring failure is now
explicit and worth stating as a rule: **before attributing a cost to code, prove that code executes on the
measured path.** `--no-jit` is a one-line control and I ran it last instead of first.

### 4. ⚠ SUPERSEDED BY §4b — the reasoning below is retained only as the record of a wrong turn

Keep the `Vec` of entries (insertion order *is* part of the value — R1, and it is what keeps
`keys()`/iteration byte-identical with PHP), and add a **hash index** beside it: exactly zend's
`HashTable` (`arData` + hash slots), and what `indexmap` is. `PhStr` **already caches FNV-1a** on `Heap`
strings (`phstr.rs:38-42`), so the index gets literal-key hashes for free — the machinery was built for
this and is currently unused by the map.

- *Projected effect:* removing the ~2.0 ms scan puts phorj at ~4.9–5.1 ms against php's 5.8 ms →
  **ratio ≈1.14–1.18, a WIN.** ⚠ **[Inferred, NOT measured]** — it is arithmetic on the measured floor,
  and it is exactly the kind of number that must not be quoted as fact until the slice runs.
- *It fixes every map operation*, not this micro: reads, writes, `Set`, and the near-parity cluster
  DEC-431.1 lists alongside `mapinsert`.
- *Blast radius, counted:* **100 `Value::Map` sites across 30+ files**, including
  `src/jit/handles/maps_ext.rs` and `src/vm/exec.rs`. This is a real slice with a JIT surface — not a
  turn-tail edit, and not something to land unverified.

**Status: NOT accepted, NOT laundered, NOT fixed yet.** Recorded as an OWED loss with its true value
(~0.84) per DEC-365, with the fix designed and sized. DEC-431.1's *"PENDING RULING — push held"* still
governs, which is why the two doc commits sit unpushed.

---

## 5. Sequencing, if the batch rules "go"

Ordered by *residual closed per unit of risk*, not by product priority. The two products barely
overlap, so a merged product-order list would serve neither.

1. **Tier B corrections first** — they are bugs in shipped features, they are small, and two of them
   (duplicate headers, silent TLS downgrade) are actively wrong today. The stale-doc fixes cost
   minutes and stop us misleading the next reader.
2. **`sleep`** — tiny, on-roadmap, unblocks a headline product mode.
3. **Postgres TLS** — no new domain, largest deployment unlock.
4. **`RoundingMode.Unnecessary` + native `NUMERIC`** — the money foundation, and the place where
   "no compromises" has teeth.
5. **XML with C14N** — closes gap-ledger item 6.
6. Whatever Q2/Q3/Q4 rule in.

**Absent from the sequence because they are still QUESTIONS, not because they were dropped:** PDF (Q8),
XAdES (Q9), UUID (Q10), accent folding (Q11), client cookies + keep-alive (Q12), response streaming
(Q13), the DEC-403 naming flip (Q14). Each slots into the list above the moment it is ruled in — the
ordering principle (residual closed per unit of risk) does not change.

---

## 6. Standing constraints any resulting spec must satisfy

Recorded here so Round 2's specs do not have to re-derive them:

- **Invariant 14 (LADDER)** — `sleep` transpiles (PHP `sleep`/`usleep`). XML transpiles (PHP has
  `DOMDocument`/`XMLWriter`). Postgres TLS rides the existing `E-TRANSPILE-DB`. IMAP, if built, is
  **native-only tier 2** and owes `E-TRANSPILE-IMAP` + a differential quarantine + a disclosure
  paragraph. No silent downgrades.
- **Invariant 16 (META-7)** — cross-language survey before designing; a `__phorj_*` helper to keep the
  PHP leg byte-identical is an acceptable tool, but the trade is surfaced, never self-decided.
- **Invariant 9** — every shipped feature lands a runnable `examples/` entry in the same change; the
  example corpus IS the byte-identity coverage.
- **Invariant 17** — transpile AND lift AND the LSP AND both editors, same change, 100% rule.
- **Invariant 13** — new modules start split; `foo/mod.rs` + sub-files.
- **Dependency policy** — `Cargo.toml` + UNIFIED-SPEC § "External dependency policy" are the SSOT;
  any admission updates BOTH in the same change, with the domain justification and a
  THIRD-PARTY-NOTICES row.
- **The file-backed-transport house rule** — `Core.Mail` already ships `File`/`Null` transports.
  Any new I/O module (IMAP especially) ships the same, so CI never needs the network.

---

## 7. What the NEXT round produces, once §4 is ruled

One `docs/specs/<date>-<topic>.md` per admitted item, each carrying: surface (runnable phorj), the
ladder case, the backend/transpile story, the error taxonomy, the example + differential plan, the
LSP/editor row, and the dependency row if any. Mirrored into MASTER-PLAN + SLICE-STATE + the decision
register in the same change (Invariant 19).
