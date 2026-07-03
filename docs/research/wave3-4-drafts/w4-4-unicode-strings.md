# W4-4 · M-text: Unicode-correct strings — Design Doc

> Status: DESIGN-NEEDED (research draft, read-only). LENS = **MANDATORY-better** (DEF-016, §12
> RULED: adopt Unicode-correct-by-default; bytes explicit). This doc grounds the design in the
> actual codebase + real PHP 8.5 behaviour and hands the developer the two genuine forks.

All PHP outputs below are **[Verified]** — run against
`/stack/tools/phpbrew/php/php-8.5.7/bin/php` (the gate floor). Rust `char`/`str` behaviours are
**[Inferred: Rust std docs]** (cargo is out of scope for this read-only pass).

---

## 0. The single most important correction to the brief

The task brief assumes Unicode ops "must transpile to `mb_*`/`grapheme_*`". **The project has
already rejected that framing.** The oracle in `tests/differential.rs` runs the transpiled PHP
under **`php -n`** (no `php.ini`) — `mbstring` and `intl` are ini-loaded shared modules on the CI
build and are **stripped by `-n`** (memory `transpile-no-ini-extensions`; the mbstring trap
already bit `core.bytes.toString` once and BCMath bit `decimal`). The M9 **locked policy** is:
*core stdlib = tier-1 (always-compiled) extensions only* → PCRE (`preg_*`), Core/standard.

MASTER-PLAN §W4-4 HOW states this as the doc's *hard constraint* verbatim: "core stdlib must map
to tier-1 PCRE/`preg_*` with `u` or hand-rolled UTF-8 helpers". So `mb_*` is not the mapping — it
is (a) the naive default the plan rejects and (b) a possible **explicit-load** option (§6, fork B)
that collides with the M9 policy and must be adjudicated, not assumed.

Two facts from the plan are therefore **settled, not open**, and this doc treats them as decided:
- **Unit = code points** for length/index/slice (explicit grapheme API layered on top). WHAT says
  so, and it is *forced*: codepoint ops are tier-1-achievable AND match Rust `char` (see §1/§2).
- **Tier-1 mapping**, not `mb_*`, for everything with a tier-1 path.

The genuinely-open questions (§6) are narrower: **case folding** and **grapheme strategy**.

---

## 1. The core question — what IS a Phorj `string`?

**Model (recommended, plan-aligned): a `string` is a sequence of Unicode scalar values (code
points) over a guaranteed-valid UTF-8 buffer.** `Value::Str(String)` is already a Rust `String`
(UTF-8-valid by construction, `src/value.rs:123`). Nothing in the representation changes — only the
*unit of measurement* exposed by the natives changes from **bytes → code points**.

| Operation | Today (byte, DEF-016 flaw) | W4-4 (code-point-correct) |
|---|---|---|
| `length` | `s.len()` bytes → `"héllo"`=6 | `s.chars().count()` → `"héllo"`=5 |
| indexing `s[i]` / `charAt` | not yet exposed (deferred here) | i-th code point |
| `substring(s,start,len)` | byte slice, faults on mid-char split | code-point slice (never splits) |
| `indexOf`/`lastIndexOf` | byte offset | code-point offset |
| iteration `for (c in s)` | Rust `chars()` (already code points) — but PHP `str_split` is byte-wise | code points, both legs |
| comparison `==`, `<=>` | byte-lexicographic (`a==b`, `value.rs:520`) | **unchanged — keep byte/scalar ordering** |
| `count(s,sub)` | `substr_count` (byte) | code-point-count of matches |

**Consequences / deliberate non-changes:**
- **Comparison stays code-point/byte-lexicographic.** UTF-8 byte order == code-point order, so
  `==`/`<=>` need no change and stay byte-identical. Unicode *collation* (locale-aware ordering) is
  explicitly NOT in scope — it is neither tier-1 nor deterministic. [Inferred: UTF-8 invariant.]
- **A `string` is guaranteed valid UTF-8.** Invalid byte sequences live in `bytes`, never `string`
  (§4). This is what makes "code point" well-defined and lets the buffer stay a Rust `String`.
- **Grapheme clusters are NOT the default unit** — see §2/§6. `"e\u{0301}"` (e + combining acute)
  is **2 code points** [Verified: `preg_match_all("/./us")` = 2] but **1 grapheme**. Default ops
  count code points; grapheme awareness is an explicit, opt-in API.

---

## 2. Byte-identity tension (the hard part) — the tier-1 mapping table

Invariant 1 (byte-identity spine: `run ≡ runvm ≡ transpiled PHP`) must hold for every op. The Rust
legs use `char`-based iteration; the PHP leg must produce identical bytes **under `php -n`** — so
the mapping targets PCRE `/u`, not `mb_*`. Verified equivalences against PHP 8.5:

| Phorj op | Rust backend | PHP leg (tier-1, `-n`-safe) | Byte-identical? |
|---|---|---|---|
| `length` | `s.chars().count()` | `preg_match_all('/./us', $s)` | ✅ [Verified: `"héllo"`→5 both; == `mb_strlen`] |
| `substring(s,a,n)` | char slice | `preg_split('//u',$s,-1,PREG_SPLIT_NO_EMPTY)` + `array_slice` + `implode` | ✅ [Verified: `(1,3)`→"éll"] |
| `indexOf(s,sub)` | char offset of match | `preg_split('//u')` → `array_search`, or byte `strpos`→count code points before | ✅ [Verified: `l`@2 both tier-1 and `mb_strpos`] |
| `charAt` / `s[i]` | `s.chars().nth(i)` | `preg_split('//u')[i]` (or PCRE `\X`-free single-cp match) | ✅ [Inferred: same split] |
| `reverse` | `s.chars().rev()` | `implode('',array_reverse(preg_split('//u',…)))` — **changes** from `strrev` | ✅ code-point reverse |
| `count(s,sub)` | count non-overlapping | `substr_count` (byte, but substring match is byte-position-invariant for valid UTF-8) | ✅ result unchanged |
| `contains/startsWith/endsWith/split/splitOnce/replace/join/trim*/repeat` | as today | **unchanged** — substring/position-membership ops give the same answer for valid UTF-8 regardless of unit | ✅ no semantic change |
| **`uppercase`/`lowercase`/`capitalize`/`equalsIgnoreCase`/`containsIgnoreCase`** | Rust `to_uppercase`/`to_lowercase` (full Unicode) | **NO tier-1 equivalent** — `strtoupper` is ASCII-only | ⚠️ **THE break — see below** |
| pad family (`padLeft`/`padRight`) | byte `str_pad` today | code-point width → hand-rolled or `str_pad` on cp-count | needs cp-width rewrite |

**Where PHP `mb_*` semantics differ from Rust `char`/`str` — the risk, verified:**

1. **Case folding is the sharpest, sneakiest divergence.** It *looks* trivial and *silently*
   diverges. Three-way split, all [Verified] on PHP 8.5:
   - tier-1 `strtoupper("straße")` → **"STRAßE"** (ASCII-only, ß untouched)
   - `mb_strtoupper("straße")` → **"STRASSE"** (ß→SS, full Unicode)
   - Rust `"straße".to_uppercase()` → **"STRASSE"** [Inferred: std full case mapping]

   So Rust matches `mb_*`, **not** tier-1. To make Phorj case ops Unicode-correct, the Rust leg is
   already correct, but the **PHP leg cannot stay tier-1** — it must call `mb_strtoupper`, which
   requires the explicit-extension-load escape hatch (§6 fork B) and breaks the M9 tier-1 policy.
   And even with `mb_*`, **residual divergence remains** (Rust and PHP-ICU may use different
   Unicode versions on rare newly-assigned characters → parity breaks silently).
   - Greek final sigma: `mb_strtolower("ΟΔΟΣ")` → "οδος" (final ς) [Verified]. Rust **`str`**-level
     `to_lowercase` special-cases final sigma too (matches); Rust **`char`**-level does not — so
     the impl MUST use `str::to_lowercase`, never per-char, or it diverges. [Inferred: std docs.]
   - Ligature `mb_strtoupper("ﬁ")` → "FI" [Verified]; Rust agrees. Turkic-i: both locale-independent
     (`mb_strtolower("I")`→"i") [Verified] — no locale surprise as long as neither uses a locale.

   → **Case folding is the primary quarantine candidate (LADDER rung 2) if the developer holds the
   tier-1-only line.** It is the "biggest byte-identity risk" for the summary.

2. **Grapheme clustering has no tier-1 PHP path at all.** `grapheme_*` is `intl` (ini-loaded, `-n`
   strips it). Achieving grapheme correctness requires either explicit `intl` load or a hand-rolled
   UAX-29 break-table on both legs (large, Unicode-version-sensitive). This is the biggest
   *difficulty* — but it is **contained**: grapheme is opt-in, not the default unit, so it does not
   threaten default-string byte-identity. Ships late (S3).

---

## 3. Migration / blast-radius

**Smaller than "changes the whole string type" implies.** The `Value::Str` representation does not
change; only ~10–15 of the 35 `Core.String` natives change *semantics*, and shipped examples are
**ASCII-only by policy** (KNOWN_ISSUES "Core.String breadth (M4) — ASCII only") so example churn is
minimal — the work is *new non-ASCII conformance tests* + native rewrites, not rewriting the corpus.

**Ops that change (offset/length/width-returning):** `length`, `substring`, `indexOf`,
`lastIndexOf`, `count`, `reverse`, `padLeft`/`padRight`, plus the case family (`uppercase`,
`lowercase`, `capitalize`, `equalsIgnoreCase`, `containsIgnoreCase`). ≈ 12 of 35.

**Ops that do NOT change** (position/membership/substring — byte-position-invariant for valid
UTF-8): `contains`, `startsWith`, `endsWith`, `split`, `splitOnce`, `lines`, `replace`, `join`,
`trim`/`trimStart`/`trimEnd`, `repeat`, `isEmpty`, `parseInt`/`parseBool`/`parseFloat`. These keep
their current `strpos`/`str_contains`/`explode`/`str_replace` erasures untouched.

**Each changed native touches all three legs together** (per invariants 1–4): interpreter
(`src/native/text.rs` `eval`), VM (same native path — natives are backend-shared), and the `php:`
closure in the registry entry. Plus: expose string indexing `s[i]`/`charAt`/`substring` (perimeter
deferral from DEC-089 "`s[0]` → defer M-text") — new AST/checker/compiler surface, not just natives.
`chr`/`ord` land as a new `Core.Codepoint` module (none exists today — verified no `chr`/`ord`).

**Codemod:** DEF-016 note (KNOWN_ISSUES) says byte-length-dependent user code is expected rare; a
codemod flags `.length` used as a byte count. Low volume given ASCII-only examples.

**Tests:** `src/native/text_tests.rs` (15K) currently asserts byte semantics in places — those
assertions flip. `tests/differential.rs` gains a non-ASCII vector set.

---

## 4. bytes vs string — the split already exists

The type-level split is **already implemented** and is the right boundary; W4-4 sharpens its
meaning rather than inventing it:

- **`string`** = *text*: guaranteed-valid UTF-8, measured in code points (post-W4-4). `Value::Str`.
- **`bytes`** = *raw*: arbitrary octets, measured in bytes, may be invalid UTF-8. `Value::Bytes`
  (`Core.Bytes`, `src/native/bytes.rs`). `Bytes.length` is **byte count** (`strlen`) by design.

**The encode/decode boundary is the only crossing** and it is fallible one way:
- `Bytes.fromString(s)` → infallible (valid UTF-8 → bytes; PHP erasure is identity).
- `Bytes.toString(b)` → **fallible**: validates UTF-8 via tier-1 PCRE `preg_match('//u',$b)===1`
  (NOT `mb_check_encoding` — that was the original mbstring trap; `src/native/bytes.rs:96`), returns
  `string?`. This is exactly the model to keep: **byte semantics remain explicitly available via
  `bytes`; `string` is never byte-indexed.** Users who need byte length ask `bytes`, not `string`.

W4-4 adds: `Core.Codepoint` (chr/ord over scalar values), and the charset-conversion contract
(ICONV rows) lives at this boundary (`bytes` ⇄ `string` under a named encoding) — staged S2/S3,
and any non-UTF-8 charset conversion is itself a tier-1/extension question (likely LADDER).

---

## 5. Phasing

- **S1 — code-point core (ships first; the DEF-016 fix).** Flip the offset/length ops to code
  points on all three legs using the **tier-1 PCRE mapping** (§2 table): `length`, `substring`,
  `indexOf`, `lastIndexOf`, `count`, `reverse`, pad family. Expose `s[i]`/`charAt`. Add
  `Core.Codepoint` chr/ord. This is fully tier-1, fully byte-identical, **no fork needed** — closes
  the headline `"héllo".length == 5` acceptance. Case ops explicitly *left ASCII* in S1 (documented).
- **S2 — case folding (fork-gated).** Resolve §6 fork B first. Either (a) quarantine Unicode case
  (keep ASCII `strtoupper`, add `Text.upperUnicode` native-only w/ `E-TRANSPILE-*`), or (b)
  explicit `mb_*` load + CI wiring + residual-divergence conformance vectors.
- **S3 — grapheme + normalization + charset breadth.** Opt-in grapheme API (`Text.graphemes`,
  grapheme-aware length), NFC/NFD normalization, ICONV charset rows. Each is its own extension/
  hand-roll decision; grapheme almost certainly LADDER rung-2 native-only unless `intl` is adopted.

---

## 6. Open questions for the developer (§15 adjudication — recommended answers)

Per invariant 15, these are the developer's to rule interactively; each ships with a minimal failing
program in the question text. Recommendations below, recommended-first with the why.

**Q1 — Unit for `length`/indexing: code point vs grapheme?**
*Recommend: **code point** (default), with an explicit opt-in grapheme API (S3).* Why: (a) the plan
already recommends it; (b) code points are tier-1-achievable AND byte-identical [Verified §2]; (c)
graphemes need `intl`/hand-rolled UAX-29 (no tier-1 path, Unicode-version-fragile) — making them the
default would either break byte-identity or force a heavy dependency for the common case. `"👨‍👩‍👧".length`
= several code points (honest, matches `mb_strlen`) vs 1 grapheme (nicer but non-tier-1). Ship
code points; let `Text.graphemeLength` answer the grapheme question explicitly.

**Q2 — Unicode case folding: quarantine (tier-1-only) vs explicit `mb_*` load? (THE hard fork.)**
Minimal failing program: `Text.uppercase("straße")` — Rust yields `"STRASSE"`, tier-1 PHP yields
`"STRAßE"` [Verified] → byte-identity breaks TODAY if we make Rust Unicode-correct.
- *Option A (recommend, leans to the M9 policy): **quarantine Unicode case** (LADDER rung 2).* Keep
  `uppercase`/`lowercase` **ASCII** (tier-1 `strtoupper`, byte-identical, unchanged). Add explicit
  `Text.uppercaseUnicode`/`lowercaseUnicode` as **native-only** with `E-TRANSPILE-CASE-UNICODE` hard
  error + differential quarantine + disclosure paragraph. Rationale: preserves M9 tier-1-only, keeps
  the spine intact, makes the PHP-can't-do-this-purely fact explicit rather than silently downgraded
  (rung-3 FORBIDDEN).
- *Option B: **explicit `mb_*` load** (BCMath precedent).* `php -n -d extension=mbstring` + CI
  `extensions: mbstring`, make `uppercase` full-Unicode on all legs. This **conflicts with the M9
  locked policy** ("core stdlib = tier-1 only") and carries **residual risk**: Rust vs PHP-ICU may
  disagree on rare characters across Unicode versions → silent parity breaks needing a pinned
  conformance vector set. Only choose if Unicode case in *core* `String` is judged essential.
- Recommendation: **A** unless the developer wants Unicode case in the core default — then B with a
  pinned Unicode-version conformance suite.

**Q3 — grapheme strategy: `intl` load vs hand-rolled UAX-29 vs native-only quarantine?**
*Recommend: native-only (LADDER rung 2) in S3*, opt-in `Text.graphemes*`; revisit `intl` only if
grapheme demand is real. Why: no tier-1 path; hand-rolling UAX-29 is large and version-sensitive.

**Q4 — `reverse` semantics change (byte→code point).** `Text.reverse("héllo")` today byte-reverses
(PHP `strrev`, would corrupt the é); code-point reverse is correct. *Recommend: change to code-point
reverse (S1).* It's already listed as a changed op; low risk (examples ASCII).

**Q5 — charset conversion (ICONV rows) at the bytes⇄string boundary.** *Recommend: defer to S3,
treat non-UTF-8 charsets as an extension/LADDER question (`iconv`/`mbstring` are non-tier-1).* UTF-8
validation stays the only always-present crossing (already shipped).

---

## 7. Acceptance / tests

- **Headline (S1):** `"héllo".length == 5` byte-identical on all three legs (run ≡ runvm ≡ PHP).
  [Verified the PHP leg computes 5 via tier-1 `preg_match_all('/./us')`.]
- **Non-ASCII conformance vector set**, each asserted equal across interpreter, VM, and transpiled
  PHP under the real gate `PHORJ_PHP=/stack/tools/phpbrew/php/php-8.5.7/bin/php PHORJ_REQUIRE_PHP=1`:
  - length/substring/indexOf/charAt on: `"héllo"`, `"naïve"`, `"Ωμέγα"`, a 4-byte emoji `"😀"`,
    combining sequence `"e\u{0301}"` (assert **2** code points — proves cp≠grapheme).
  - `substring` never faults on a multibyte boundary (the current EV-7 fault disappears).
  - `reverse` code-point-correct on `"héllo"` → `"olléh"`.
- **Case (S2, per fork):** if Option A — a differential-quarantine test that `Text.uppercaseUnicode`
  hard-errors on transpile (`E-TRANSPILE-*`) and the ASCII `uppercase` stays byte-identical; if
  Option B — a pinned conformance suite covering ß→SS, Greek final sigma, ligature fi, asserting
  Rust `str::to_uppercase/to_lowercase` == `mb_strtoupper/mb_strtolower` on PHP 8.5, with any
  divergence explicitly enumerated. [Verified today: ß→SS and final-sigma agree between Rust `str`
  and `mb_*`; `char`-level would break final sigma — the impl must use `str`-level.]
- **bytes boundary:** `Bytes.toString` of invalid UTF-8 returns `null` (tier-1 `preg_match('//u')`),
  byte-identical.
- **Examples:** ship a runnable `examples/` program exercising non-ASCII length/substring/charAt
  (invariant 9), + `examples/README.md` entry; KNOWN_ISSUES DEF-016 note closed; the "Core.String
  breadth — ASCII only" section rewritten to describe the code-point contract + the case quarantine.
- **CI:** if Option B, add `-d extension=mbstring` probe to `tests/differential.rs::php_n_args`
  (twin of the BCMath handling) + `extensions: mbstring` to both `setup-php` steps.

---

## Summary of forks the developer owns

1. **Q2 case folding** — quarantine (recommend A, holds M9 tier-1) vs explicit `mb_*` load (B).
2. **Q3 grapheme** — native-only quarantine (recommend) vs `intl` adoption.

Everything else (code-point default, tier-1 PCRE mapping, bytes/string split, S1 scope) is settled
by the plan + verified byte-identical here.
