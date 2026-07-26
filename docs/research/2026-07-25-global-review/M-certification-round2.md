# DEC-268 MAXIMAL certification — ROUND 2 (fresh-context adversarial reviewer)

Artifact under review: `docs/research/2026-07-25-completeness-register.md` + the round-1 fix commit
`e0fab96` and everything it touched. Repo state at review: `HEAD = e0fab96`, `git status --porcelain`
empty (clean tree). All evidence below re-derived from the code/files, not from the commit message.

---

## Verdict per lens

| Lens | Verdict | Findings |
|---|---|---|
| **1 — correctness + regression** | **FINDINGS** | N3, N4, N5, N6, N8 |
| **2 — security + safety-promises** | **FINDINGS** | N2 (highest consequence this round), N6, M6-residue |
| **3 — completeness + blast-radius** | **FINDINGS** | N1, N3, N7, M6-residue |

**ROUND 2 IS NOT CLEAN.** Two-consecutive-clean counter stays at **0**.

No round-1 finding was *reversed* — every fix moved in the right direction. But **five of seven fixes
(M1, M3, M4, M5, M6) were applied to one surface and left an identical or contradicting claim standing on
another**, which is the same single-surface-edit failure mode M1 itself was raised for. One of the
survivors (N2) is the P1 security item still labelled "small" in the canonical ruling row.

---

## Part A — fix verification

| Fix | Status | Evidence |
|---|---|---|
| **M1** agenda size | **LANDED-INCOMPLETE (minor)** | Core fix correct. Register head `:5` = "**DEC-339 … DEC-365** — 27 rows"; §2 heading `:71` = "**27 rulings** (`GR-1`…`GR-27` ⇄ DEC-339…DEC-365)"; index `:73-76`; `SLICE-STATE.md:3,11-13`; `MASTER-PLAN.md:34` — all say 27 / DEC-339…DEC-365 [Verified: read each line]. Independent recount: **27** distinct `GR-` ids in the register (`GR-1`…`GR-27`, gap-free) and **exactly 27** table rows matching `^\| DEC-3(39\|4x\|5x\|6[0-5]) \|` in `C-decisions.md`; **zero** `DEC-366+` anywhere in `docs/` → range is gap-free and non-colliding [Verified: scripted counts]. **Index pointers all correct** [Verified: §2's `### GR-n` headings run `:82`→`:283` = GR-1…GR-17; §6.4 spans `:404-440` and contains GR-18 `:406`, GR-19 `:417`, GR-20 `:420`, GR-21 `:425`, GR-22 `:428`, GR-23 `:432`, GR-24 `:435`; §7.3 `:539-559` contains GR-25 `:541`, GR-26 `:553`; §8.4 `:630` = GR-27]. `C-decisions.md:3378`'s "seventeen open adjudications" is **not** stale — it is a correctly-scoped per-batch heading (DEC-356…362, DEC-363/364, DEC-365 have their own headings at `:3423`, `:3454`, `:3479`). **Residue:** two summary surfaces still carry the 17-scoped framing — `register:325` *"Work that is unambiguous once **GR-1…GR-17** are ruled"* (§4 now under-scopes the agenda) and `SLICE-STATE.md:43` *"explains 6 of the **17** items"*. See also N9 (cosmetic double blank line at `register:78-79` from the insert). |
| **M2** retracted claim | **LANDED-CORRECT** | `P0-block-shadow-byte-identity.md:105-118` carries a blockquoted `⚠ CORRECTED — THE CONCLUSION BELOW WAS **WRONG**. Do not act on it.` and the old text is struck through and stamped `**[RETRACTED …]**` at `:120-122`; the `[Verified: parse error text]` stamp is gone. **I re-ran all four forms and the retraction is exactly right, verbatim** [Verified: `phg run` on 4 probes, `probe-cert2/{forin,foreach_as,foreach_in,for_as}.phg`, both engines]: `for (int x in xs)` → prints `1\n2\n3`; `foreach (xs as int x)` → prints `1\n2\n3`; `foreach (int x in xs)` → *"parse error at 8:16: expected 'as' after the foreach iterable"*; `for (xs as int x)` → *"parse error at 8:14: expected 'in' in for-loop header"*. Supporting claims also confirmed: `E-RETIRED-FORIN` → **0** occurrences in `src/`; census **87 `for…in` vs 8 `foreach…as`** in `examples/**/*.phg` — **exact** (93 `for (` lines, 6 of them C-style → 87 with `in`; 8 `foreach`). |
| **M3** unwrap census | **LANDED-INCOMPLETE** | The two required surfaces landed: `register:365` = "**26 production `unwrap()`** in 154,817 lines across 566 files"; `G-rust-quality.md:657` table cell = "⚠ **CORRECTED to 26**"; the "leave the rest" assurance at `:784-787` now discloses non-coverage. **I re-derived the count and 26 is right** [Verified: scripted brace-matched `#[cfg(test)]`-block exclusion over all 566 `src/**/*.rs`, then hand-filtered test files by path (`/tests/`, `*_tests.rs`) and dropped the doc-comment hit at `jit/mod.rs:59` → **exactly 26 occurrences** across 17 files: `ast/class_hierarchy.rs:263`, `bundle/sha256.rs:29,30`, `checker/collect/interfaces.rs:122`, `checker/collect/types_decls.rs:222,744`, `compiler/stmt/core.rs:80,91`, `ext/database/natives/mysql_sql.rs:234`, `ext/json/natives.rs:58`, `jit/range_acc.rs:108(×2),280,281,305,306`, `lift/lifter/exprs.rs:179`, `native/random.rs:147`, `parser/items/types/members.rs:122`, `pm/manifest.rs:196`, `pm/resolve.rs:93`, `tokenizer/ident.rs:10`, `tokenizer/scan.rs:139,174`, `tokenizer/strings.rs:551`, `transpile/classes.rs:48`]. `566` files and `154,817` lines also **exact** [Verified: `find src -name '*.rs' \| wc -l` = 566; `cat`-all `\| wc -l` = 154817]. **But** → N3 (three residual `~20` claims in the same file, one still `[Verified]`-stamped) and N4 ("Six … never read" is wrong; it is five). |
| **M4** GR-25 severity | **LANDED-INCOMPLETE** | Severity landed in the register: `:541` now reads "`P1` SECURITY, treat as a top-10 item, not a small one" with the mechanism. **The mechanism is fully verified — and I proved it empirically, which is stronger than the file claims.** [Verified] `withHeader` interpolates `"{name}: {value}"` with zero validation (`src/cli/http_prelude.rs:71-73`); `withCookie` interpolates `"Set-Cookie: {line}"` (`:74-77`); `serialize()` CRLF-joins via `String.join(this.headerLines, nl)` where `nl` = `b"\x0d\x0a"` (`:91-99`); `respond_once` returns handler bytes verbatim — `b.as_ref().clone()` (`src/serve/handlers.rs:189,200-205`). **Live probe** (`probe-cert2/split.phg`, `phg run`): `Response.text(200,"ok").withHeader("X-User", "x\r\nX-Injected: yes\r\n\r\n<html>pwned</html>")` serializes to a head that terminates early and injects a full extra header **and a second body** — `X-User: x<CRLF>X-Injected: yes<CRLF><CRLF><html>pwned</html><CRLF><CRLF>ok`. No error, no validation. Sibling guard cited **correctly**: `src/ext/http_client/natives.rs:112-118` rejects `\r`, `\n` (and `:` in names) with *"header `{n}` contains a forbidden character"*, pinned by `src/ext/http_client/tests.rs:449-467 header_injection_is_rejected_at_the_gate`, which does feed `"a\r\nHost: evil"` as the header **value** — so the "request side is the template" claim holds. **But** → N5 (the `http_prelude.rs:52` citation is wrong — `:52` is `class Response {`) and **N2** (the canonical `DEC-363` row still says "small", no P1). |
| **M5** stale no-ruling entry | **LANDED-INCOMPLETE** | Removed from `SLICE-STATE.md:59-61` ✅ [Verified: read the block — now "Grammar fix + gate (GR-3) · stale-label fixes … · UFCS diagnostic span … · the block-scoping differential example"], and the underlying fix is real (`CLAUDE.md:8-14` now says "**14 vetted, feature-gated crates**", committed in `b3e635e`). **But the identical stale item survives as `register:326-330` §4 item 2**, stated in the present tense as fact — see **N1**. Re-checked the other bullets against reality, per instruction: (1) grammar bug **still present** — `editors/vscode/syntaxes/phorj.tmLanguage.json:34` `"begin": "\\b(b\|r)?\""` ✅; (3) wildcard spec header **still stale** — `docs/specs/2026-07-24-wildcard-imports.md:1,3` say "NOT YET BUILT"/"NOT BUILT" while `register:41`§1 #5 verdicts it "BUILT + CERTIFIED" ✅, and the "LSP AUTOCOMPLETE — DONE + COMPREHENSIVE" claim exists ✅ but at `SLICE-STATE.md:1083`, not the cited `:1022` (→ N8); (4) bad UFCS span **reproduced** ✅ (see "Claims re-derived"); (5) block-scoping example absence — not re-tested, round 1 certified it with a positive/negative-validated detector. |
| **M6** corporate filenames | **LANDED-INCOMPLETE** | `register:277-278` is genericized ✅ ("three corporate service `.env` files plus desktop-automation drivers"). **But `docs/research/2026-07-25-global-review/J-claude-bundle.md:157` — committed in the same review set, in this **public** repo — still names three corporate service `.env` files verbatim**, inside the very cell arguing that committing corporate MCP configs to a public repo is an information-exposure risk. `M-certification-round1.md:178` re-quotes all three as well. Low absolute risk (bare filenames, no contents), but the fix demonstrably did not cover the blast radius it was raised about. |
| **M7** sample-dependent counts | **LANDED-CORRECT** | `register:389` now reads "`phg disassemble` **is unstable across runs (≥5 distinct outputs; 5–6 observed per 20-run batch)**" ✅ — a distribution, not a property. I7's "3 different answers across 20 runs" was intentionally left (round 1 itself noted it is stable because only 3 candidates exist) — correct call. Two notes, neither a defect: the commit message claims "two … rephrased" when only one was; and `I-gaps-enforcement.md:111,649` still say "5 distinct outputs from 12 runs" — acceptable, because that phrasing states its own sample size. |

---

## Part B — new findings

### N1 — MEDIUM · M5's stale entry survives in the register, which then contradicts itself
`register:326-330` (§4 "READY FOR AUTONOMOUS EXECUTION", item 2) still says:
> *"**`CLAUDE.md:9` dependency correction** — it claims "**four** vetted, feature-gated exceptions … actual is **14** … Docs-only."*

This is a **present-tense false claim**: `CLAUDE.md:8-14` already reads *"the core pipeline stays std-only, and **14 vetted, feature-gated crates** are admitted"* [Verified: read `CLAUDE.md:1-14`; landed in `b3e635e` "part 3"]. §4 is the *canonical analysis home* that `SLICE-STATE` points to, so removing the item from the cursor while leaving it in the register inverted the fix's direction of travel. **The register also already contradicts itself**: `register:519` states *"(Two of these — the CLAUDE.md count and the KNOWN_ISSUES heading — were fixed tonight.)"* Two adjacent sections of the same file now disagree about whether this work is pending.
**Fix:** delete `register:326-330` and renumber §4 (5 items → 4).

### N2 — MEDIUM · the P1 security item is still labelled "small" in the canonical ruling row
`C-decisions.md:3461`:
> `| DEC-363 | GR-25 | The Response-side outbound sink has **no CRLF guard** (header-injection shape) | Add the guard — small, security-relevant | **PENDING** |`

M4 upgraded GR-25 to **P1 SECURITY, top-10, "not a small one"** in `register:541` and `SLICE-STATE.md:12` ("GR-25 P1 security"), but the row the developer actually rules from was not touched — it still carries the retracted "small" and, unlike its siblings (`DEC-339` = "**P0**", `DEC-340` = "**P1 data loss**", which do carry severity in-row), no severity tag at all. **This is the exact failure mode M4 existed to prevent, reproduced on a different surface**: a developer working down `C-decisions.md` sees "small" and deprioritises a verified, empirically reproduced HTTP response-splitting path on a shipped `phg serve` (see M4 row for the live probe). Highest-consequence finding of round 2.
**Fix:** tag the `DEC-363` row `**P1 security**` and drop "small" from the Recommended cell.

### N3 — MEDIUM-LOW · three residual `~20 unwrap()` claims survive in the file M3 corrected, one still `[Verified]`
`G-rust-quality.md` was corrected in two places and left uncorrected in three others [Verified: `grep -n '~20\|≈20'`]:
- `:664` (the prose directly under the corrected table) — *"and **~20** production `unwrap()` in 155k lines is exceptional discipline"*
- `:753` (the **G24 section heading**) — *"### G24 — **~20** production `unwrap()`, ~0 with a justification comment…"*
- `:754` — *"**[Verified: read every one of the ~20 sites]**"* — this is the precise false stamp M3 was raised about: by M3's own correction, 5 production sites were never read, so a `[Verified]` claim of having read every site is still standing three lines under the correction that refutes it.
A reader who lands on `:753` (a heading — a normal entry point) gets the retracted figure with no correction in view.

### N4 — LOW-MED · "Six were therefore never read" is wrong; it is five, and the sibling note says five
`G-rust-quality.md:657` says *"**Six** were therefore never read:"* and then enumerates **five** sites (`pm/resolve.rs:93`, `pm/manifest.rs:196`, `bundle/sha256.rs:29`, `:30`, `native/random.rs:147`). The sibling correction at `:786` says *"the **5** sites listed in the corrected census row above"*. The two halves of the same fix disagree.
**Five is correct** [Verified: of the 17 files holding the 26 production unwraps, exactly **4** contain `cfg(test)` — `bundle/sha256.rs`, `native/random.rs`, `pm/manifest.rs`, `pm/resolve.rs` — and they hold exactly **5** production `.unwrap()` occurrences; so the old whole-file-exclusion census covered 21 sites, consistent with its "≈20"]. An off-by-one inside a correction whose whole subject was a miscount.

### N5 — LOW · wrong file:line citation introduced by the M4 edit
`register:542` cites *"`src/cli/http_prelude.rs:52` `Response.withHeader(name, value)`"*. `:52` is `class Response {`; `withHeader` is at **`:71-73`** and `withCookie` at **`:74-77`** [Verified: `sed -n '40,95p'` + `grep -n 'withHeader\|withCookie'`]. Off by 19 lines on the load-bearing citation of a P1 security finding.

### N6 — LOW · GR-3's evidence row renders garbled in BOTH surfaces the developer rules from
An unescaped `|` inside the quoted regex `(b|r)` splits the GFM table cell (a code span does **not** protect `|` — only `\|` does), so the row has one column too many and its trailing content shifts/truncates when rendered:
- `register:52` — §1 verdict #6, header has 5 pipes, this row has 6
- `C-decisions.md:3396` — the **`DEC-341` / GR-3 ruling row itself**, header 6, row 7
[Verified: escape-aware pipe-count checker over the register, both cursors and all 13 review reports]. Pre-existing (not introduced by `e0fab96`), but it degrades exactly the two rendered surfaces GR-3 is decided from. The same sweep found 9 further mismatched rows outside this review's scope, worth one mechanical pass: `C-decisions.md:152,211,212,213,214,217`, `H-docs-consistency.md:954`, `I-gaps-enforcement.md:86`, `L-onhold-inventory.md:116`.
**Fix:** escape as `(b\|r)`.

### N7 — MEDIUM-LOW · Invariant-19 content duplication across the three homes (N2 is its realised symptom)
`register:6` claims *"One canonical home each — Invariant 19, no duplicated content"* and `SLICE-STATE.md:13` / `C-decisions.md:3386` claim the decision register holds *"identity + status only; analysis lives in the register"*. Two of the new rows break that:
- `C-decisions.md:3432` (`DEC-356`) carries full analysis + evidence (37 `Expr` variants, 13 rewriters, 17 catch-alls, `ast/walk.rs:748`) plus a three-step ranked recommendation — duplicating `register:406-416`.
- `C-decisions.md:3482` (`DEC-365`) carries **the same five measured ratios** as `register:606` *and* `SLICE-STATE.md:49-51` (`floatloop` 1.011→0.803, `dbwork` 1.004→0.960, `floatmul` 1.002→0.980, `mapget` 1.152→0.996, `setcontains` 1.129→0.954) [Verified: `grep -n '1.004\|0.960\|1.011\|0.803'` across all three files].
Three copies of the same analysis is three places to drift — and **N2 is that drift, already realised** on `DEC-363`. Either honour "identity + status only" and trim the rows, or drop the no-duplication claim; the current state asserts a discipline the artifacts do not keep.

### N8 — LOW · stale line pointers into `SLICE-STATE.md`
`register:333` and `SLICE-STATE.md:33` both cite *"SLICE-STATE's *"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"* claim"*. The claim is real but now lives at **`:1083`** [Verified: `grep -n 'AUTOCOMPLETE'`]. It *was* at `:1022` at commit `25053be` and has drifted 61 lines as parts 4/5/6 and the M1 fix prepended blocks [Verified: replayed `git show <sha>:docs/plans/SLICE-STATE.md | grep -n` across the last 8 commits touching the file — 954 → 980 → 1007 → 1022 → 32 → 33]. This is the doc-rot class `GR-24`/`DEC-362` proposes guards for; worth noting that it is *self-inflicted by the review's own edits* and will keep drifting, so line-anchored citations into `SLICE-STATE.md` should be replaced by heading anchors or quoted subjects (which is what `GR-24`'s third guard already recommends for SHAs).

### N9 — NIT · cosmetic artifact of the M1 insert
`register:78-79` — the FULL AGENDA INDEX insert left a double blank line before "Each item: …". Renders identically; noted only for completeness.

### Damage check on the round-1 edits themselves — CLEAN
- **Code fences balanced** in every edited file [Verified: `grep -c '^```'` → register 8, `G-rust-quality.md` 16, `P0-…md` 4, `SLICE-STATE.md` 0, `MASTER-PLAN.md` 0 — all even].
- **Zero literal CR bytes** anywhere in the register or the whole review directory [Verified: `grep -rlc $'\r'` → no matches], so the M4 edit's `"a\r\nHost: evil"` survived as the intended two-character escape sequence, not a real carriage return.
- **Strikethrough correctly paired** in `P0-…md` (`~~` opens `:120`, closes `:122`, one paragraph → valid GFM).
- **No orphaned/duplicated text** at any edit boundary [Verified: read `register:69-84`, `:275-280`, `:363-367`, `:538-556`; `G-rust-quality.md:653-666`, `:778-790`; `P0-…md:98-123`; `SLICE-STATE.md:1-16`, `:56-62`; `MASTER-PLAN.md:34`].
- Every table in the register still parses to 3 tables with one mismatching row, and that row is pre-existing (N6), not from `e0fab96`.

---

## Claims I independently re-derived

| Claim | Source | My result |
|---|---|---|
| Four loop-form results (2 live, 2 crossed = parse errors), verbatim error texts | `P0-…md:107-111` (M2) | **CONFIRMED exactly**, both engines |
| `E-RETIRED-FORIN` absent from `src/` | `P0-…md:114`, §3 | **CONFIRMED — 0 occurrences** |
| **Loop census 87 `for…in` vs 8 `foreach…as`** *(chosen: §1 #7 / K-1)* | `register:389`-adjacent, `K-inline-findings.md:31` | **CONFIRMED exactly** — 93 `for (` lines in `examples/**/*.phg`, 6 C-style, 87 with `in`; 8 `foreach` |
| **26 production `unwrap()`** | `register:365`, `G:657` (M3) | **CONFIRMED exactly** (26 occurrences / 17 files; full list in the M3 row) |
| **566 files / 154,817 lines** | `register:365` | **CONFIRMED exactly** (566 / 154817) |
| **GR-8: 2223 qualified sites, 1231 `Output.printLine` (55.4%)** *(chosen: §2 GR-8)* | `register:165+`, `C:3401` | **CORROBORATED** — `Output.printLine(` = **1231 exact**; my qualified-site regex gives **2231** vs 2223 (+0.36%, methodology variance). Ratio 55.2–55.4% either way |
| **§7.1's "40 stale labels"** *(chosen: §7.1)* | `register:501-521` | **ARITHMETIC CONFIRMED** — 26 "OPEN but BUILT" + 14 "DONE but NOT" = 40, and the "(two were fixed tonight)" disclosure is accurate (`CLAUDE.md` count verified fixed) — which is what exposes N1 |
| HTTP response splitting is reachable from handler code | `register:541-548` (M4) | **CONFIRMED — and reproduced live**, stronger than the file claims (full extra header **and** injected second body, no error) |
| `respond_once` returns handler bytes verbatim | `register:545`, `handlers.rs:189` | **CONFIRMED** — `b.as_ref().clone()` at `:205` |
| Request-side CRLF guard + its test are the claimed template | `register:547-549` | **CONFIRMED** — `natives.rs:112-118`, `tests.rs:449-467`, feeds `"a\r\nHost: evil"` |
| **UFCS bad span `1:9`** *(chosen: §4 item 4 / K-7)* | `register:334`, `K-…:179-191` | **CONFIRMED — and the trigger identified.** My plain probe gives correct spans (`8:9`, `9:19`); moving the call **inside a string interpolation** (`"len={b.length()}"`) reproduces `type error at **1:9**` pointing at `package Main;`. This upgrades K-7's *"likely the same class as the K-5 interpolation skew"* from `[Inferred]` to `[Verified]`, and the register's §4 item 4 should state the trigger so the fix is scoped correctly |
| TextMate grammar bug still present | `register:52`, §2 GR-3 | **CONFIRMED** — `phorj.tmLanguage.json:34` |
| Wildcard spec header stale | `register:333`, §7.1 | **CONFIRMED** — `2026-07-24-wildcard-imports.md:1,3` say NOT BUILT; §1 #5 verdicts BUILT + CERTIFIED |
| `CLAUDE.md` dependency count fixed | commit `b3e635e` | **CONFIRMED** — `CLAUDE.md:8-14` says 14 → which is what makes `register:326-330` false (N1) |

## Could not verify

- **Nothing blocked on access or tooling.** No `cargo` build was run (disk constraint, per instruction); all Rust claims were verified by reading source plus running the **existing** `target/release/phg`, which is sufficient for every claim examined.
- **§4 item 5** (no `examples/**` program shadows an outer local) was not re-tested — round 1 verified it with a detector validated on 6 positives and 1 negative; re-running it would add nothing.
- **Round-1 Lens 1's 53 confirmed items** were not re-verified wholesale; I re-derived a chosen subset (table above). I did not find a single case where a round-1 `[Verified]` claim failed to reproduce.
- **`I-gaps-enforcement.md`'s disassemble instability** was accepted from round 1's own 2×20-run measurement rather than re-measured (round 1 already superseded the register's figure with a wider sample; a third sample adds no information).

---

## Recommended disposition

All nine findings are documentation/framing defects in a decision artifact — **none is a research error, and
none changes any ruling's substance**. N2 is the one that could change a developer's *priority* and should be
fixed before the agenda is worked. Suggested single pass, all mechanical:

1. **N2** — tag `C-decisions.md:3461` `**P1 security**`, drop "small". *(do first)*
2. **N1** — delete `register:326-330`, renumber §4.
3. **N3/N4** — fix `G-rust-quality.md:664`, `:753`, `:754` (`~20` → 26; drop the false "read every one" stamp) and `:657` ("Six" → "Five").
4. **N5** — `register:542` `http_prelude.rs:52` → `:71-77`.
5. **N6** — escape `(b\|r)` at `register:52` and `C-decisions.md:3396` (+ the 9 out-of-scope rows if a sweep is cheap).
6. **M1 residue** — `register:325` `GR-1…GR-17` → `GR-1…GR-27`; `SLICE-STATE.md:43` "6 of the 17 items" → "6 of the items".
7. **M6 residue** — genericise `J-claude-bundle.md:157`.
8. **N7** — decide: trim the analysis out of `DEC-356`/`DEC-365`, or drop the "no duplicated content" claim.
9. **N8/N9** — de-line-anchor the SLICE-STATE's *"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"* claim citations (→ `:1083` or a heading anchor); drop the extra blank line.
10. **Bonus (not a finding)** — add the interpolation trigger to `register:334`, since I proved it.

Round 3 should re-verify these and then, if clean, count as clean **round 1 of 2**.
