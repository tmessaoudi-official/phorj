# DEC-268 MAXIMAL certification — ROUND 3 (fresh-context adversarial reviewer)

Artifact under review: `docs/research/2026-07-25-completeness-register.md` + the round-2 fix commit
`cfb42f0` and every surface it touched. Repo state: `HEAD = cfb42f0`, `git status --porcelain` empty
(clean tree) [Verified: `git log --oneline -6`, `git status --porcelain`]. All evidence below was
re-derived from the files and from live `phg`/`php` runs — nothing was accepted from a commit message
or from rounds 1/2.

---

## Verdict per lens

| Lens | Verdict | Findings |
|---|---|---|
| **1 — correctness + regression** | **FINDINGS** | O2 (wrong citation on a live cursor), O4 (numeric contradiction introduced by the fix) |
| **2 — security + safety-promises** | **CLEAN** | none — N2 landed on every surface; the live probe reproduced verbatim; zero corporate filenames or secrets under `docs/` |
| **3 — completeness + blast-radius** | **FINDINGS** | O1 (the N1 fix's blast radius), O2, O3 (Invariant-19 claim still unqualified) |

**ROUND 3 IS NOT CLEAN.** Two-consecutive-clean counter stays at **0**.

Seven of the nine round-2 findings landed correctly and I re-derived their evidence independently.
**Two landed on the surface round 2 named and were not carried to a sibling surface that still
contradicts them** — the same surface-local failure mode round 2 was itself raised for, now in its
third consecutive occurrence (M1→N1→O1, N8→O2). Nothing found is a research error, and **no ruling's
substance changes**; the register remains safe to base the 27 decisions on once these are applied.

---

## Part A — fix verification

| Fix | Status | Evidence |
|---|---|---|
| **N1** stale `CLAUDE.md` dep item | **LANDED-INCOMPLETE** | Register side is **correct**: `register:322-335` — §4 now has exactly **4** items (1 grammar, 2 stale-labels, 3 diagnostic span, 4 block-scoping example), correctly renumbered, and closes with *"(The `CLAUDE.md` dependency correction that used to head this list was **applied** in part 3 — see §6.5.)"* [Verified: read `:322-335`]. §6.5 `:443` and §7.1 `:518-519` both still record it as done ✓. The underlying fix is real: `CLAUDE.md:8-16` reads *"the core pipeline stays std-only, and **14 vetted, feature-gated crates** are admitted"* and even adds a never-restate-a-count rule [Verified: read `CLAUDE.md:1-18`; `git log -- CLAUDE.md` → `b3e635e`]. **But four other surfaces still present it as pending — see O1.** |
| **N2** `DEC-363` severity | **LANDED-CORRECT** | `C-decisions.md:3461` now leads **`**P1 SECURITY**`**, states the mechanism (`withHeader`/`withCookie` unvalidated → CRLF-joined → `respond_once` verbatim ⇒ response splitting), records *"reproduced live (an injected header **and** a second body, no error)"*, recommends copying the sibling request-side guard, and points at `register §7.3` [Verified: read `:3461`]. **"small" is gone from every live claim surface**: the only remaining occurrences repo-wide are `register:541` (*"not a small one"*), `register:554` (an explicit *"Initially filed as 'small' … corrected"* history note) and the round-1/2 reports quoting the defect [Verified: `grep -rn "small"` filtered to GR-25/DEC-363/CRLF context]. **I reproduced the vulnerability myself** — see "Claims I independently re-derived"; the register's live-probe sentence is accurate **verbatim**, and if anything understated (see O5). |
| **N3/N4** unwrap census | **LANDED-CORRECT** | Zero residual `~20`/`≈20` unwrap claims outside the certification reports that quote them as the finding [Verified: `grep -rn '~20\|≈20'` over the review dir + register → only `M-certification-round1/2.md`]. `G-rust-quality.md:657` now says *"**Five** were therefore never read"* and lists five ✓. `:664` says *"26 production `unwrap()`"* ✓. The `G24` heading `:753` now says *"26 production `unwrap()` (⚠ this section originally said ~20 — CORRECTED…)"* and its stamp `:754` reads *"[Verified for 21 of 26 sites — … **5 production sites were never read** (`pm/resolve.rs:93`, `pm/manifest.rs:196`, `bundle/sha256.rs:29`, `:30`, `native/random.rs:147`). This stamp does NOT cover those five.]"* — the false blanket `[Verified]` is gone and the non-coverage is disclosed ✓. **I re-derived 26 independently** (below). |
| **N5** `http_prelude` citations | **LANDED-CORRECT** | `register:542-544` cites `:71-73` (withHeader), `:74-77` (withCookie), `:91-99` (serialize). All three exact [Verified: `grep -n` → `71: function withHeader`, `74: function withCookie`, `91: function serialize`; body read — `withHeader` closes at `:73`, `withCookie` at `:77`, and `serialize`'s CRLF join `String.join(this.headerLines, nl)` is at `:97` with the `head` interpolation at `:98` and the return at `:99`, so `:91-99` covers the whole mechanism]. |
| **N6** unescaped pipe | **LANDED-CORRECT** | Both required surfaces escaped: `register:52` and `C-decisions.md:3396` now carry `` `"\\b(b\|r)?\""` `` [Verified: `git show cfb42f0` diff + escape-aware recount]. My **escape-aware cell counter** (strips code fences, pairs each row against its own header, treats `\|` as literal) over the register, both cursors and all 15 review reports finds **exactly 9** mismatched rows — **`register:52` and `C-decisions.md:3396` are no longer among them** — and the 9 are line-for-line the out-of-scope set round 2 listed: `C-decisions.md:152,211,212,213,214,217`, `H-docs-consistency.md:954`, `I-gaps-enforcement.md:86`, `L-onhold-inventory.md:116`. Count confirmed at 9, unchanged, as expected. |
| **N7** Invariant-19 duplication | **LANDED-PARTIAL** | Row trimming landed: `C-decisions.md:3432` (`DEC-356`) is now identity + status + a short recommendation + *"**Analysis + the three ranked options: register §6.4**"*, and `:3482` (`DEC-365`) is identity + status + *"**Measured ratios + full analysis: register §8**"* [Verified: read both]. **Ratio homes: 2, down from 3** — `register:588,611` and `SLICE-STATE.md:49-51`; `C-decisions.md` no longer carries any of the five [Verified: `grep -rln` on all eight numbers `1.011/0.803/1.004/0.960/1.002/0.980/1.152/0.996/1.129/0.954` → register + SLICE-STATE only, plus `M-certification-round2.md` quoting them as evidence and one pre-existing unrelated `1.152` in `full-audit/raw/M-gap-matrix.md`]. So the commit's "two places rather than three" claim is accurate. **But the honest answer to "does any analysis still live in two homes?" is YES** — see O3. |
| **N8** stale `SLICE-STATE:1022` anchors | **LANDED-INCOMPLETE** | Register side fixed: `register:328-330` now cites the claim by quoted subject with an explicit drift note ✓. **`SLICE-STATE.md:33` — which round 2 named by exact line — was NOT fixed**, and `:61` carries the same anchor. See **O2**. |
| **N9** double blank line | **LANDED-CORRECT** | Zero double blank lines anywhere in the register [Verified: `awk` consecutive-blank scan → no output]. |
| **M1 residue** (27-scoping) | **LANDED-CORRECT** | `register:325` now reads *"unambiguous once `GR-1`…`GR-27` are ruled"* ✓; `SLICE-STATE.md:43` now reads *"explains 6 of the **27** items"* ✓. Cursors consistent: `SLICE-STATE.md:3` ("**27 RULINGS**"), `:11-12` (the 27-item split), `:14` ("DEC-339…DEC-365 (27 rows)"), `MASTER-PLAN.md:34` ("**27 DEVELOPER RULINGS**", "27 rows") [Verified: read each]. One residual 24-scoped phrase at `register:497` — **O6**, a nit, positionally accurate. |
| **M6 residue** (corporate filenames) | **LANDED-CORRECT (with a new nit)** | **Zero** occurrences of `jira.env`/`confluence.env`/`gitlab.env` anywhere under `docs/` [Verified: `grep -rn` over the whole tree → empty]. `J-claude-bundle.md:157` is genericised ✓. Introduced a three-vs-four count contradiction — **O4**. |

---

## Part B — new findings

### O1 — **P2** · the applied `CLAUDE.md` dependency fix is still presented as PENDING on four other surfaces, one of which the register itself designates as the inventory's canonical detail table

`CLAUDE.md` is fixed (14 crates, `b3e635e`) [Verified: read `CLAUDE.md:8-16`]. The register removed the
item from §4 and records it done in §6.5/§7.1. But:

- **`L-onhold-inventory.md:128` (`L-71`)** — *"**`CLAUDE.md` understates the dependency set: it claims
  "four vetted, feature-gated exceptions"** … | **STILL OPEN** [Verified: `CLAUDE.md:8-9` vs
  `Cargo.toml:113-180`] … | Fix in the next CLAUDE.md touch. **Classifier-blocked for Claude → present
  the exact diff for manual application**"*. This is a present-tense pending row carrying a
  `[Verified]` stamp that no longer reproduces, **plus an actionable hand-back instruction asking the
  developer to do work already committed.** `register:494` explicitly routes the developer here:
  *"Full table in `…/L-onhold-inventory.md` (95 rows, each with citations, a reality check, and a
  recommendation)"* — so the stale claim survives on the exact surface the developer is told to run the
  inventory from.
- **`L-onhold-inventory.md:570`** (the 40-stale-label table, row 12) — still contrasts *"four"* against
  *"Eleven domains"*, with no correction note.
- **`H-docs-consistency.md:133`** — *"### H3 — `CLAUDE.md:9` says the core has **four** external deps;
  there are **14** [P1]"*, and **`:1543`** row 7 re-lists it as an open **P1**.
- **`C-stdlib-input-fs-clone.md:421`, `:917`** — stale-doc flags (P3) stating the same.

Round 2 counted `register:326-330` a finding on precisely this basis ("§4 is the canonical analysis
home"), and round 2's own fix set genericised a *report* (`J-claude-bundle.md:157`), so the reports are
in scope. **Fix:** stamp `L-71`, `L:570` row 12 and `H3` as **FIXED (`b3e635e`)**; the `C-` flags are
P3 snapshots and can carry a one-line note or be left.

### O2 — **P2** · `SLICE-STATE.md` still cites its own line `:1022` twice, and `:1022` now holds unrelated text

Round 2's N8 named *"`register:333` **and `SLICE-STATE.md:33`** both cite `SLICE-STATE.md:1022`"*. Only
the register half was fixed. Still standing:

- **`SLICE-STATE.md:33`** — *"**`SLICE-STATE.md:1022` "LSP AUTOCOMPLETE — DONE + COMPREHENSIVE" is
  measurably FALSE for UFCS**"*
- **`SLICE-STATE.md:61`** — *"stale-label fixes (a spec header says "NOT BUILT" about a certified
  feature; `SLICE-STATE:1022`)"*

Both are wrong today: the claim lives at **`:1083`**, and **`:1022` is unrelated text** —
*"ex-`architecture-decomp.plan.md` folded into MASTER-PLAN.) Full report + root-cause +"*
[Verified: `grep -n AUTOCOMPLETE docs/plans/SLICE-STATE.md` → `33`, `1083`, `1226`; `sed -n '1022p'`].
A cursor citing a wrong line **inside itself** is worse than the register doing it: a developer
following it lands on unrelated prose and may conclude the finding was already cleaned up. Aggravating:
`register:328-330` now *documents* this exact drift (`:1022`→`:1083`) while the cursor it describes
still carries the dead anchor. Also present in reports (lower stakes, snapshot-shaped):
`L-onhold-inventory.md:64,274,563`, `B-lsp-editors.md:610,697`.
**Fix:** de-line-anchor `SLICE-STATE.md:33` and `:61` to the quoted subject, exactly as `register:328`
now does and as `GR-24`'s third guard recommends.

### O3 — **P3** · `register:6`'s "no duplicated content" claim is still unqualified while §8's analysis lives in two homes

`register:6`: *"One canonical home each — Invariant 19, no duplicated content."* Round 2's N7 offered
two resolutions (trim the rows **or** drop the claim). The rows were trimmed, but
`SLICE-STATE.md:47-56` still reproduces **register §8's analysis substantively whole**: all five
measured ratios, the `"cpuset … discarded"` root cause, the "drifted down in lockstep / a real
regression would not" corroboration, the not-bypassed disclosure, **and** the `queryparse` 0.146-vs-0.88×
contradiction [Verified: read `SLICE-STATE.md:47-56` against `register:582-631`]. **Ratio home count: 2**
(register + cursor). Defensible as a cursor recording a live blocker — but then `register:6` should say
so (e.g. "one canonical home for each *analysis*; the cursor carries a summary") rather than assert
"no duplicated content" flatly. It is the register's own opening credibility claim.

### O4 — **P3** · the round-2 genericisation introduced a three-vs-four `.env` contradiction

- `register:277`: *"all 57 `mcp/**` files — **three** corporate service `.env` files plus
  desktop-automation drivers"*
- `J-claude-bundle.md:157`: *"**four** corporate service client configs + their `.env` files"* and
  *"contains corporate-tooling artifacts — **four** service `.env` files"*

[Verified: read both lines.] Pre-edit the two were not in conflict (the register named three filenames;
J said "Jira/Confluence/GitLab/Trivy **client configs** + `.env` files" — four configs, unstated .env
count). Genericising both independently turned an implicit distinction into an explicit numeric
contradiction. Zero decision impact (both surfaces say HARD OUT under every option), but it is the same
one-surface-at-a-time edit pattern, now self-inflicted by the fix for it.
**Fix:** pick one number (the round-1 evidence supports **three** `.env` files among four service configs)
and state it identically in both.

### O5 — **P3** · the reproduced exploit is *understated*, not overstated (safety-promise direction: favourable)

My probe (below) shows the serialized head carries **`Content-Length: 2`** while ~30 further bytes follow
inside the same response — so the primitive is not only response **splitting** but a
request-smuggling/desync shape as well. The register describes only "an injected header and a second
body". Not a defect (no false claim); recorded because a reviewer should say when an artifact errs
toward caution, and because it slightly strengthens the P1 case in `DEC-363`.

### O6 — **P3 (nit)** · residual 24-scoped framing at `register:497`

*"**Counts:** **95** deduplicated items — ~46 need a ruling (**the 24 above** + ~22 smaller)"*. The
agenda is 27. This is *positionally* accurate (GR-1…GR-24 appear above `:497`; GR-25/26 land in §7.3
just below and GR-27 in §8.4), and the arithmetic 24+22≈46 is internally consistent because GR-25/26
were promoted **out of** the "~22 smaller" bucket. Flagged only because it is the last survivor of the
17/24-scoped framing family round 2 cleaned at `:325`, and a reader who has internalised "27" may pause.
**Fix (optional):** *"the 24 presented above (GR-25…GR-27 follow) + ~22 smaller"*.

### O7 — **P3 (nit)** · unclosed italic at `register:554`

`*(Initially filed as "small" and ranked 25th — corrected … the aggravating fact.)` opens an emphasis
span that never closes, so GFM renders a literal `*` [Verified: emphasis-parity scan over the register
with code spans and `**` stripped — `:554` is the only genuinely unpaired single `*`; the other odd-count
lines (`11/12`, `329/330`, `623/624`, `637/638`) are legitimate two-line spans]. **Pre-existing from
`e0fab96`, not introduced by `cfb42f0`** [Verified: `git show e0fab96:… | grep -n "Initially filed"` →
present at `:551`].

### O8 — **P3** · a third live Invariant-1 divergence is not labelled as one in the register

I reproduced `Core.Validation`'s trailing-`\n` divergence end-to-end (below) — a genuine three-leg
byte-identity break. `register:530` lists it as *"the cheapest real bug in the whole sweep"* but does not
label it an Invariant-1 / byte-identity break, while `register:21` frames the block-shadow bug as
*"**A** P0 byte-identity break"* and `register:385` calls `I8` *"**A SECOND** exception to Invariant 1,
which claims exactly one."* A reader can reasonably conclude there are exactly two. Mitigated:
`L-onhold-inventory.md:151` (`L-82`) does label it *"(**real 3-leg divergence**)"* and cites
`Invariant 1`, and `KNOWN_ISSUES.md:335-345` records it. **Fix (one clause):** note in `register:530`
that it is a third live Invariant-1 divergence, one line to fix.

### Damage check on the `cfb42f0` edits — **CLEAN**

- **Code fences balanced** in all 20 files touched or adjacent [Verified: `grep -c '^```'` → register 8,
  SLICE-STATE 0, MASTER-PLAN 0, C-decisions 0, and A…P0 reports 52/32/34/24/12/0/16/16/24/0/4/0/8/0/4/0 —
  every count even].
- **Zero literal CR bytes** across the register, both cursors, `C-decisions.md` and the whole review
  directory [Verified: `grep -rlU $'\r'` → no matches], so the `"a\r\nHost: evil"` and
  `"x\r\nX-Injected…"` strings survived as two-character escape sequences, not real carriage returns.
- **The two index-sliced rows sit correctly inside their tables and swallowed nothing.**
  `C-decisions.md:3431` header → `:3432` `DEC-356` → `:3433` **`DEC-357` intact and complete**
  (*"A lambda's write to a by-value-captured variable is silently lost …"* + full recommended cell)
  → `:3434` `DEC-358` → … `:3437` `DEC-362`. `:3480` header → `:3482` `DEC-365` → the
  **`**Collateral finding (perf certification …)**` paragraph fully intact** at `:3484-3491`, including
  the `queryparse = 0.146` figure, the DEC-338 ~0.88× contrast, the two-readings disclosure and the
  "WIN stays un-certified" conclusion [Verified: read `:3425-3436` and `:3477-3492`].
- **Cell counts correct on the rebuilt rows** — the escape-aware counter reports **no** mismatch anywhere
  in `C-decisions.md:3390-3495`; the only mismatches in that file are the six pre-existing ones at
  `:152,211,212,213,214,217`.
- **Agenda recount from scratch, gap-free and non-colliding** [Verified: scripted]: **27** distinct `GR-`
  ids in the register = exactly `GR-1`…`GR-27`; **27** `| DEC-nnn | GR-n |` rows in `C-decisions.md` =
  exactly `DEC-339`…`DEC-365`, no duplicates, and the mapping `DEC-(338+n) ⇄ GR-n` holds for **all 27**;
  **zero** `DEC-366+` anywhere in `docs/` except the round-2 report's own prose. Section coverage matches
  §2's FULL AGENDA INDEX: `### GR-n` headings = 1…17 (§2), bulleted items = 18…26 (§6.4 = 18-24,
  §7.3 = 25-26), and GR-27 under the `### 8.4 — GR-27 (DEC-365)` heading — 27/27 presented, none dropped.
- **No truncated / duplicated / orphaned text at any edit boundary** [Verified: read `register:45-55`,
  `:69-84`, `:275-280`, `:320-336`, `:536-562`; `G-rust-quality.md:653-668`, `:750-790`;
  `J-claude-bundle.md:153-160`; `SLICE-STATE.md:28-64`; `C-decisions.md:3390-3400`, `:3425-3436`,
  `:3455-3465`, `:3477-3492`]. One stylistic wrinkle, **pre-existing from `e0fab96` and already accepted
  by round 2**: `G-rust-quality.md:786-788` splices the correction parenthetical mid-sentence
  (*"…in 155k lines is ⚠ **(CORRECTED: …)** not a debt."*) — grammatically it still completes; no fact
  is wrong. Not counted as a finding.

---

## Claims I independently re-derived

| Claim | Source | My result |
|---|---|---|
| **§0's P0 block-shadow break, all three legs** *(chosen: the single most load-bearing claim in the artifact; neither prior round re-ran the PHP leg)* | `register:26-37`, `MASTER-PLAN.md:34` | **CONFIRMED EXACTLY.** `probe-cert3/shadow.phg` — `phg run` → `in=2 / out=1`; `phg run --tree-walker` → `in=2 / out=1`; `phg transpile` + `/stack/tools/phpbrew/php/php-8.5.8/bin/php` → `in=2 / **out=2**`. The P0 is real, reproducible, and correctly described |
| **HTTP response splitting, live** *(N2's newly-added register sentence)* | `register:549-550`, `C-decisions.md:3461` | **CONFIRMED VERBATIM — and understated.** `probe-cert3/split.phg`, `phg run`: head = `HTTP/1.1 200 OK⏎Content-Length: 2⏎Content-Type: text/plain⏎X-User: x⏎X-Injected: yes⏎⏎<html>pwned</html>⏎⏎ok` (all `⏎` = real CRLF). An injected header **and** a second body, no error, no validation — plus a `Content-Length: 2` mismatch (→ O5) |
| **26 production `unwrap()` / 566 files / 154,817 lines**, and **exactly 5 sites in `cfg(test)`-containing files** | `register:365-366`, `G:657`, `G:754` | **CONFIRMED EXACTLY.** Scripted brace-matched `#[cfg(test)]`-**block** exclusion over all 566 `src/**/*.rs`, dropping path-shaped test files: raw 45 → minus 19 in `src/ext/database/natives/tests_more.rs` (a test file my path filter missed) → **26**, across the **same 17 files and same line numbers** round 2 listed. Of those 17, exactly **4** contain `#[cfg(test)]` (`bundle/sha256.rs`, `native/random.rs`, `pm/manifest.rs`, `pm/resolve.rs`) holding exactly **5** occurrences (`sha256.rs:29,30`, `random.rs:147`, `manifest.rs:196`, `resolve.rs:93`) — so **"Five", not "Six", is right**, and the disclosed 5 are the correct 5. `find src -name '*.rs' \| wc -l` = **566**; total lines = **154817** |
| **§7.2 #1 / `L-82` — `Core.Validation` trailing-`\n` divergence and the "one-line `/D` fix"** *(chosen: ranked #1 in the autonomous hand-back batch, never verified by rounds 1-2)* | `register:529-530`, `L:151` | **CONFIRMED, both halves.** `Validation.isAlpha("abc\n")` → `phg run` **false**, `phg run --tree-walker` **false**, transpiled PHP under 8.5.8 → **true**; emitted `preg_match('/^[A-Za-z]+$/', "abc\n")`. Source confirms **exactly five** pre-`/D` predicates — `isInt`, `isNumber`, `isAlpha`, `isAlnum`, `isHex` (`src/native/validate.rs:261-278`) — while every later predicate already carries `/D`, with an in-file comment stating `D` *"kills the trailing-`\n` divergence the pre-D validators above still carry"*. So "five emitters, one flag each" is exact |
| **§1 #5 / `GR-3` — stdlib wildcard imports are parser-rejected** *(chosen: the wildcard capability matrix, unverified by rounds 1-2)* | `register:51` | **CONFIRMED.** `import Core.Text.*;` → *"parse error at 5:19: wildcard import of the standard-library module `Core.Text` … **is not yet supported** — import its members explicitly"*; same for `Core.String.*`; and `import Core.*;` gives a distinct, correct rejection (*"it would bind the entire standard library"*). Parser-level, wording as quoted. (Incidentally corroborates `GR-20`: neither error carries a diagnostic code) |
| **§1 #6 / `GR-3` — the TextMate `\b`-before-optional-group root cause, and its denominators** *(chosen: the grammar figure, unverified by rounds 1-2)* | `register:52`, `C-decisions.md:3396` | **ROOT CAUSE CONFIRMED, figures partly.** `editors/vscode/syntaxes/phorj.tmLanguage.json:34` is exactly `"begin": "\\b(b\|r)?\""` ✓. Re-running that pattern against representative lines: on `string s = "abc";` (quotes at 11, 15) it matches **only at 15** — the **closing** quote; on `Output.printLine("hi");` (quotes at 19, 22) only at **22**. On `b"…"` / `r"…"` it correctly matches the opening. So *"every plain string starts at its CLOSING quote"* is exactly right. Denominators exact: **383** `.phg` files repo-wide (excl. `target/`), **266** under `examples/` ✓. The `81/383` and `188/266` leakage counts are **[Unverified]** — they need `vscode-textmate` (absent; no `node_modules`, and installing is out of bounds on disk/network grounds) |
| **Escape-aware table integrity across all 19 markdown surfaces** | round 2's N6 sweep | **CONFIRMED — exactly 9 mismatched rows, the expected out-of-scope set, and GR-3's two rows are fixed** (full list in the N6 row above) |
| **Agenda size / gap-freeness / 1:1 mapping** | `register:5,71-76`, `C-decisions.md`, both cursors | **CONFIRMED — 27/27** (full detail in the damage-check section) |
| **`CLAUDE.md` dependency count is fixed** | `register:443`, round 2's claim | **CONFIRMED** — `CLAUDE.md:8-16` says 14 with a re-derive-from-`Cargo.toml` rule → which is what makes O1's four surviving surfaces false |

---

## Could not verify

- **The `81/383` and `188/266` TextMate leakage counts** — require a real Oniguruma/`vscode-textmate`
  tokenizer; `node` exists (`/opt/node22/bin/node`) but `vscode-textmate` is not installed and installing
  it would need network + disk, both out of bounds. The **root cause** and both **denominators** are
  independently verified (above), so the direction and mechanism are certain; only the exact leakage
  counts rest on the B-report's own measurement. **[Unverified]**
- **`GR-13`'s `~75×` `bindNamed` quadratic figure** — my probe used the wrong API spelling
  (`Database.open` → *"expected a field or method name after '.', found Open"*), and chasing the correct
  `Core.Database` surface plus an 8000-row timing run was not a good use of the remaining budget against
  a claim that changes no verdict. Round 1 accepted it from `D-database.md`'s own measurement
  (`4 000→1.135 s`, `8 000→4.469 s` vs `0.049/0.059 s`). **[Unverified]** — chosen deliberately; the other
  three Part-B probes were higher value.
- **No `cargo` build was run** (disk constraint, per instruction). Every Rust claim was verified by
  reading source plus the existing `target/release/phg`, which sufficed for all four live probes.
- **`register:497`'s `~46 need a ruling` / `~22 smaller`** — the `~` figures are the L report's own
  approximations over 95 rows; I confirmed the internal arithmetic (24+22≈46) and the 95-row scope but did
  not recount `decision-needed` rows one by one. **[Unverified]**, and immaterial to O6's point.
- **Round-1/2 confirmed-item sets were not re-verified wholesale.** I re-derived the chosen subset above
  and **did not find a single case where a prior `[Verified]` claim failed to reproduce** — every number I
  re-checked (26 / 566 / 154,817 / 5 / 383 / 266 / 27 / 9) came back exact.

---

## Recommended disposition

Round 3's findings are, again, **documentation/framing defects in a decision artifact — no research
error, no ruling change**. The register is safe to base all 27 decisions on. But **round 3 is NOT clean**,
so the two-consecutive-clean counter stays at **0**.

Do first (the two P2s, both blast-radius carry-overs):

1. **O1** — stamp `L-onhold-inventory.md:128` (`L-71`) and `:570` row 12, and `H-docs-consistency.md:133`
   (`H3`) + `:1543` row 7, as **FIXED (`b3e635e`)**. This is the one that could waste developer time on
   completed work.
2. **O2** — de-line-anchor `SLICE-STATE.md:33` and `:61` (drop `:1022`, cite the quoted subject).

Then, in the same pass (all P3, all one line each): **O3** qualify `register:6`'s no-duplication claim (or
trim `SLICE-STATE:47-56` to a pointer) · **O4** make the `.env` count agree (three) in `register:277` and
`J-claude-bundle.md:157` · **O6** *"the 24 presented above (GR-25…GR-27 follow)"* · **O7** close the
italic at `register:554` · **O8** note in `register:530` that `L-82` is a third live Invariant-1
divergence. **O5** needs no edit.

**Structural note for whoever applies this.** Three rounds have now produced the same defect shape:
`M1→N1→O1` and `N8→O2` are each a fix applied where the reviewer pointed, not where the *claim* lives.
Round 4 will keep finding these one at a time unless the fix method changes. Concretely: for every
finding, `grep` the corrected **fact** (not the cited line) across `register` + both cursors +
`C-decisions.md` + **all 15 reports**, and fix every hit in one commit. Two of round 3's own P3s (O4) were
*created* by fixing two surfaces independently, which is the same failure with the sign flipped.

Round 4 should re-verify O1-O8 and, if clean, count as clean **round 1 of 2**.
