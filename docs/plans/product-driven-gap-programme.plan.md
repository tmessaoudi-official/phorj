# Product-driven gap programme — rent-watch + twes-in

> **Round 1 of N: VERIFICATION + VISION GATING + THE ADJUDICATION BATCH.** No implementation, and no
> per-item spec yet — deliberately. Under Invariant 15 the dependency admissions and the surface
> shapes below are the developer's to rule, and a spec written against an unruled admission is a spec
> written twice. Round 2 writes `docs/specs/*.md` for whatever survives the ruling.
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

Each question states its own after-state. **All fourteen are PENDING — nothing here is ruled, and
nothing outside this list was closed by me.** Q1–Q7 are the load-bearing policy calls (a dependency
domain or a user-visible semantic hangs on each). Q8–Q14 are the smaller ones that an earlier draft
would have quietly declined; they are cheap to answer — often one word — and each names my lean first.

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

## 7. What Round 2 produces, once §4 is ruled

One `docs/specs/<date>-<topic>.md` per admitted item, each carrying: surface (runnable phorj), the
ladder case, the backend/transpile story, the error taxonomy, the example + differential plan, the
LSP/editor row, and the dependency row if any. Mirrored into MASTER-PLAN + SLICE-STATE + the decision
register in the same change (Invariant 19).
