# M — DEC-268 MAXIMAL certification, ROUND 4 (fresh-context adversarial reviewer)

**Artifact under review:** `docs/research/2026-07-25-completeness-register.md` (27-item agenda
`GR-1`…`GR-27` ⇄ `DEC-339`…`DEC-365`), its 16 supporting reports, and both cursors.
**HEAD at review time:** `d137ae5` (round-3 fixes O1–O8). Prior rounds: R1 → M1–M7 (`e0fab96`),
R2 → N1–N9 (`cfb42f0`), R3 → O1–O8 (`d137ae5`). Clean counter entering round 4: **0**.

Everything below was re-derived from the files and from live probes. No claim is taken from a commit
message. Probe scripts: `scratchpad/probe-cert4/{hygiene.py,emph.py,tables.py,stream.phg,peak.sh,lk.rs}`.

---

## Verdict per lens (CLEAN / FINDINGS)

| Lens | Verdict | Summary |
|---|---|---|
| **1 — correctness + regression** | **FINDINGS** (Q2 P2, Q4 P2) | `d137ae5` introduced content damage: a scripted global string replace corrupted **five** sentences (including retro-editing round 2's own finding N8 into a non-statement), and a mid-quotation annotation in `C-stdlib-input-fs-clone.md:917` left an unclosed `[` **and** now attributes the OLD four-crate list to the NEW "14 vetted, feature-gated crates" wording — a factually inverted sentence that did not exist before this commit. |
| **2 — security + safety-promises** | **FINDINGS** (Q6 P3) — *no exposure* | Exposure surface is **clean**: zero tokens (`ghp_`/`glpat-`/`xox*`/`sk-`/`Bearer`), zero corporate identifiers (`internal-portal`, `cloud-acme`, `AWSELBAuthSessionCookie`, `source_machine_hash`, `e7cde47e`) anywhere under `docs/`; all 81 `acme` hits are phorj's own generic `Acme.Strutil` example vendor package. The finding is promise-accuracy only: `M-certification-round3.md:42`'s `[Verified]` claim of *"**Zero** occurrences of `jira.env`/`confluence.env`/`gitlab.env` anywhere under `docs/`"* is **self-falsifying** — that sentence is now the only occurrence of each. |
| **3 — completeness + blast-radius** | **FINDINGS** (Q1 P2, Q3 P2, Q5 P2, Q7 P3) | The R3-diagnosed pattern **recurred for a fifth consecutive round**, and this time on a *hand-delivered list*: R3 named four surfaces for O1 by exact line and **two were not touched** (`L-onhold-inventory.md:570` row 12, `H-docs-consistency.md:1543` row 7), while `d137ae5`'s message asserts *"All such rows now carry a superseded-FIXED stamp"*. O2 was fixed as a literal-string replace, so ~9 sibling dead `SLICE-STATE.md` anchors with the **identical +61 drift** survive — one of them in the *same sentence* as an O2 fix. |

**ROUND 4 IS NOT FULLY CLEAN. The two-consecutive-clean counter stays at 0.**

Nothing found is a **research error** and **no ruling's substance changes** — the 27 rulings remain safe to
take. Three previously-unverified register claims were re-derived from scratch and all three reproduced
(one of them *more* strongly than stated). The defects are documentation-integrity defects in the decision
artifact, two of which (Q2, Q4) are regressions *created* by the round-3 commit.

---

## Part A — fix verification (one row per O1..O8)

| Fix | Status | Evidence |
|---|---|---|
| **O1** — CLAUDE.md dep fix nowhere presented as pending | **PARTIAL — 2 of 4 named surfaces missed** | Underlying fix real: `CLAUDE.md:8-16` reads *"the core pipeline stays std-only, and **14 vetted, feature-gated crates** are admitted"* and adds a never-restate-a-count rule [Verified: read `CLAUDE.md:1-20`]. **14 is the correct number** [Verified: `Cargo.toml` declares exactly 14 optional crates — `argon2`, `unicode-segmentation`, `rustls`, `webpki-roots`, `corosensei`, `lettre`, `rusqlite`, `postgres`, `mysql`, `cranelift`, `cranelift-jit`, `cranelift-module`, `regex`, `ctrlc` at `:130`–`:197`]. Stamped ✓: `L-onhold-inventory.md:128` (`L-71`), `H-docs-consistency.md:133` (`H3`). **NOT stamped ✗: `L-onhold-inventory.md:570` (40-stale-label table, row 12 — still contrasts *"four"* vs *"Eleven domains"* with a live `[Verified]` and no note) and `H-docs-consistency.md:1543` (top-10 table, row 7 — still an open **P1**)**; both were named verbatim by R3 (`M-certification-round3.md:70`, `:241`). Also `L-71`'s row still **ends** with *"Classifier-blocked for Claude → present the exact diff for manual application"* **after** the words "do NOT action it" — the actionable hand-back O1 was raised for is the last thing the reader sees. → **Q1** |
| **O2** — zero dead `:1022` anchors; claim cited by subject | **APPLIED but DAMAGING and incomplete** | Live dead anchors: **0**. The 4 residual `:1022` strings are all *quotations of the defect* inside `M-certification-round2.md`/`round3.md` (legitimate historical record) [Verified: `grep -rn 'SLICE-STATE.md:1022\|SLICE-STATE:1022' docs/` → 4 hits, all in M-cert reports]. The claim genuinely lives at **`:1083`** [Verified: `grep -n AUTOCOMPLETE docs/plans/SLICE-STATE.md` → 33, 1083, 1226]. **But** the fix was a literal global replace of the string `SLICE-STATE.md:1022`, which corrupted five sentences (→ **Q2**) and left ~9 sibling dead anchors carrying the identical +61 drift (→ **Q3**). |
| **O3** — `register:6` qualified + overlap disclosed | **CLEAN** | `register:6-7`: *"One canonical home for each *analysis* — Invariant 19. (The `SLICE-STATE` cursor deliberately carries a short summary of §8's live push-blocker so a fresh context sees it immediately; that is the one intentional overlap.)"* [Verified: read `register:1-20`]. The disclosure is accurate — `SLICE-STATE.md:47-56` is indeed a condensed restatement of `register` §8.1–§8.4 [Verified: read both]. |
| **O4** — `.env` count identical on both surfaces | **CLEAN — and the author's number is the correct one** | `register:278` and `J-claude-bundle.md:157` both say **four** [Verified: read both]. **Independently counted: exactly 4** — `mcp/<mcp-client-3>/{confluence,gitlab,jira,trivy}.env` [Verified: `find <bundle>/global/mcp -name '*.env'` → 4 files, no symlinks]. **Adjudication: the author is right, R3's suggested "three" was wrong.** (Separate defect in the same sentence → **Q5**.) |
| **O5** — `Content-Length: 2` smuggling shape recorded on GR-25 | **APPLIED on the register; not carried to the canonical DEC row** | `register:553-555`: *"The head also still carries `Content-Length: 2` while ~30 further bytes follow in the same response, so the primitive is a request-smuggling/desync shape as well, not only response splitting."* ✓ [Verified: read `:544-560`]. `C-decisions.md:3461` (`DEC-363`) still characterises it as *"HTTP **response splitting**"* only — the canonical ruling row the developer rules from does not carry the upgraded shape. → **Q7a** (P3) |
| **O6** — count phrasing de-scoped | **APPLIED; introduces a new small arithmetic slip** | `register:498`: *"~46 need a ruling (the 24 presented above; GR-25…GR-27 follow in §7.3/§8.4, + ~22 smaller)"* ✓ de-scoped [Verified: read `:495-500`]. But the enumeration now reads 24 + 3 + ~22 = **~49** against a headline of **~46** — the pre-fix text (*"the 24 above + ~22 smaller"*) summed exactly. Mitigating: §7's category breakdown was never additive anyway (46+30+17+22+40 = 155 ≫ 95, categories overlap and this is not disclosed). → **Q7b** (P3) |
| **O7** — italic at the old `:554` closed | **CLEAN** | Register whole-file emphasis parity (fences and code spans excluded): `**` = 678 (even), single `*` = 170 (even) [Verified: `probe-cert4/emph.py`]. `register:560` now ends `…the aggravating fact.)*` ✓. |
| **O8** — `Core.Validation` labelled a THIRD divergence | **CLEAN** | `register:530-533`: *"…and note it is a **THIRD live Invariant-1 (byte-identity) divergence**, alongside §0's block-shadow P0 and §6.2's `I8` hook divergence. §0 and §6.2 should not be read as implying there are only two."* ✓ Both cited corroborators check out: `KNOWN_ISSUES.md:335` carries the `VALIDATION-regex-trailing-newline` section calling it *"A interp ≡ VM≡PHP byte-identity divergence (Invariant 1)"*, and `L-onhold-inventory.md:138` (`L-82`) labels it *"**real 3-leg divergence**"* [Verified: read all three]. *(Pre-existing P3 nit, not an O8 defect: `L-onhold-inventory.md:455` and `:629` call the same thing a **"two-leg"** divergence, contradicting `:138`.)* |

**Damage sweep on `d137ae5` (mechanical):** all 8 touched files have **0 literal CR bytes** and an **even
number of code-fence lines** [Verified: `probe-cert4/hygiene.py`]. Escape-aware GFM cell counts: **0**
anomalies in 6 of 8 files; the 2 hits are both **pre-existing and untouched by `d137ae5`** —
`H-docs-consistency.md:954` genuinely does over-split (a quoted nested table row with unescaped pipes,
8 cells in a 5-cell block, P3 rendering defect), and `L-onhold-inventory.md:590` is a **detector artifact**
(a backslash inside the code span `` `App\` `` fools my escape-aware splitter; the raw row has the correct
3 pipes) [Verified: `probe-cert4/tables.py` + `awk 'NR==590'`]. `H-docs-consistency.md`'s odd `**` count is
also pre-existing (1257 at `cfb42f0`, 1259 at `d137ae5` — the O1 edit added a balanced pair) and traces to a
code span opened on a prior line at `:711`, i.e. another detector artifact.

---

## Part B — new findings

### Q1 — **P2** · O1 was applied to 2 of the 4 surfaces round 3 named *by exact line*, and the commit asserts otherwise

`M-certification-round3.md:241` reads: *"**O1** — stamp `L-onhold-inventory.md:128` (`L-71`) and `:570`
row 12, and `H-docs-consistency.md:133` (`H3`) + `:1543` row 7, as **FIXED (`b3e635e`)**."* Applied:
`:128` ✓, `:133` ✓. **Not applied:**

- `L-onhold-inventory.md:570` — `| 12 | **`CLAUDE.md:8-9` "four vetted, feature-gated exceptions"** | four | **Eleven domains** in `Cargo.toml` | [Verified: `Cargo.toml:113-180`] |` — unchanged, no supersession note, live `[Verified]` stamp that no longer reproduces. This row is **inside the 14-recorded-as-DONE-but-NOT table** that `register:502-524` (§7.1) presents as *"the single largest waste surface found"* and *"exactly … your decision time"*.
- `H-docs-consistency.md:1543` — `| 7 | **H3/H20** "four vetted deps" — actually **14 declared / 9 default-on**; the cited spec is stale too | **P1** | Opening paragraph of the file every session reads …` — still an open **P1** in the top-10 table, two lines below a row whose own detail section (`:133`) now carries the FIXED stamp.

`d137ae5`'s message states *"**All** such rows now carry a superseded-FIXED stamp naming b3e635e"* and
*"every fix was applied by grepping the corrected FACT across all 19 surfaces in one pass, not row by
row"*. Neither is true for O1. [Verified: `git show d137ae5 -- <both files>` → hunks at `@@ -130`, `@@ -61`,
`@@ -125`, `@@ -271`, `@@ -560`; `:570` and `:1543` are in no hunk.]

**This is the fifth consecutive round of the same defect** (`M1 → N1 → O1 → Q1`), and it is now strictly
worse than round 3's version: R3 handed over a four-item list of exact line numbers and half of it was
skipped. The failure is no longer "the reviewer didn't point at the sibling surface" — it is "the fix was
not checked against the list it was given". **Fix:** stamp `L:570` row 12 and `H:1543` row 7, and move
`L-71`'s *"Classifier-blocked → present the exact diff for manual application"* clause **before** the
FIXED stamp or delete it (a hand-back for committed work is the exact hazard O1 was raised for).

### Q2 — **P2** · the O2 fix was a scripted literal replace and corrupted five sentences, one of which retro-edits round 2's own finding into a non-statement

`d137ae5` replaced the literal `SLICE-STATE.md:1022` / `SLICE-STATE:1022` with the phrase
`SLICE-STATE's *"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"* claim` everywhere it appeared, including
inside sentences whose grammar depended on the replaced text being a *line reference*. Result
[all Verified: read each line]:

1. **`M-certification-round2.md:84`** — now: *"`register:333` and `SLICE-STATE.md:33` both cite **"SLICE-STATE's *"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"* claim"**. The claim is real but now lives at `:1083`."* Round 2's finding **N8 was that those two sites cite a stale line number**; the record now says they cite the quoted subject — i.e. that they already do the thing N8 asked for — while the rest of the paragraph still explains the 61-line drift. **A closed round's audit record has been rewritten into a self-defeating paragraph.** Under DEC-268 these reports *are* the certification evidence trail; retro-editing them removes the ability to audit the chain. It is also nested-italic-inside-italic, so it renders wrong.
2. **`M-certification-round2.md:140`** — *"de-line-anchor the SLICE-STATE's *"…"* claim citations (→ `:1083` or a heading anchor)"* — the remediation instruction no longer names what is to be de-anchored.
3. **`L-onhold-inventory.md:274`** — *"SLICE-STATE's *"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"* claim **claims** *"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"*, which is measurably false…"* — "claim claims" plus the assertion duplicated verbatim.
4. **`L-onhold-inventory.md:563`** (40-stale-label table, row 5) — cell 1 is now *"**SLICE-STATE's *"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"* claim — "LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"**"*, duplicating the string and dropping the file locator entirely; cell 2 then repeats it a third time. (Cell **count** is fine — no table breakage.)
5. **`B-lsp-editors.md:610`** — *"Note SLICE-STATE's *"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"* claim **asserts** **"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"**"* — same duplication.
6. **`B-lsp-editors.md:697`** — the replacement was applied inside a **code span**, producing the pseudo-path `` `docs/plans/SLICE-STATE (cited by subject, not line — the anchor drifts)` `` — prose rendered as a file path, with `.md` dropped. Any future markdown reference-checker (the very guard `GR-24`/`DEC-362` proposes) will report it as a dangling path.

**Fix:** re-word each of the six sites by hand. Sites 1–2 should be **reverted to their round-2 wording** —
a prior round's report is a historical record and must not be edited to match a later fix.

### Q3 — **P2** · the dead-anchor *class* was not swept: ~9 sibling `SLICE-STATE.md` anchors survive with the identical +61 drift, one in the same sentence as an O2 fix

`register:328-331` correctly generalises the lesson (*"this file's own edits have already drifted that
anchor from `:1022` to `:1083`, which is exactly the doc-rot GR-24's third guard addresses"*), but the fix
was applied to the string, not the class. Verified dead anchors still live in the review corpus:

| Citation | Cited for | Actual location | Line `:N` now holds |
|---|---|---|---|
| `B-lsp-editors.md:698`, `:600`, `L-onhold-inventory.md:544` → `SLICE-STATE.md:1015` | "LSP find-usages project-wide" queue entry | **`:1076`** | microbench/JIT prose (`MICROBENCH_DOCKER_BOTH`) |
| `B-lsp-editors.md:142`, `:598` → `SLICE-STATE.md:1016-1017` | "prelude-class members / cached index / inferred receivers" | **`:1077-1078`** | `src/lsp/refs.rs` M-Decomp note |
| `L-onhold-inventory.md:532` → `SLICE-STATE.md:1013`, `:1116`, `:1129` | `lift_from` (DEC-312) listed REMAINING | **`:1074`, `:1177`, `:1190`** | `--no-jit` interpreter-campaign prose |
| `L-onhold-inventory.md:547` → `SLICE-STATE.md:1114`, `:1127`, `:1009` | DEC-313 transpile FS emitter | **`:1175`, `:1188`, `:1070`** | LSP4IJ / scheduling-point prose |

[Verified: `sed -n '<N>p'` and `<N+61>p` on `docs/plans/SLICE-STATE.md` for each; `grep -n find-usages`
→ `1076`.] `B-lsp-editors.md:697-698` is the sharpest case — **the O2 replacement and a surviving dead
`:1015` are in the same sentence, two lines apart**. And `L:532`/`L:544`/`L:547` are rows of the
**40-stale-label table**, i.e. the very table §7.1 tells the developer to work first; a reader who follows
`:1013` lands on unrelated text and cannot confirm the stale label.

**Fix:** de-line-anchor every `SLICE-STATE.md:<N>` citation in the 2026-07-25 review corpus (cite by
quoted subject or heading), or re-derive them all in one pass — the anchors will drift again on the next
edit to the cursor, so subject-citation is the only stable form.

### Q4 — **P2** · the `C-stdlib-input-fs-clone.md:917` annotation was inserted **mid-quotation**, leaving an unclosed bracket and a factually inverted sentence

Current text at `:917-918` [Verified: `sed -n '915,922p'`]:

> **Stale-doc flag (P3):** `CLAUDE.md:8-9` claims *"four vetted, feature-gated exceptions" [⚠ SUPERSEDED — FIXED in `b3e635e`, now "14 vetted, feature-gated crates — `argon2`,
> `regex`, `ctrlc`, `corosensei`"*. `Cargo.toml:127-180` declares **eleven** domains …

Three defects in one insertion, none of which existed at `cfb42f0`:

1. The `[⚠ SUPERSEDED …` bracket is **never closed** — no `]` anywhere in the sentence.
2. The list `argon2, regex, ctrlc, corosensei` is the **OLD four-crate list**, and the insertion moved the
   `now "14 vetted, feature-gated crates — ` text *in front of it*, so the sentence now asserts that the
   14 admitted crates **are** those four. That is the exact understatement `UNIFIED-SPEC.md:871-877` warns
   *"must not be repeated"*, re-created by the fix meant to retire it.
3. The italic span `*"…"*` now opens before the annotation and closes after the old list, so the rendered
   output wraps the annotation inside the quotation attributed to `CLAUDE.md`.

Related: R3 named **both** `C-stdlib-input-fs-clone.md:421` **and** `:917`. Only `:917` was touched, so the
same file now carries a SUPERSEDED flag at `:917` and an un-annotated live *"Side note (P3, tangential)"*
making the identical stale claim at `:421-424`. (R3 did say the `C-` flags *"can carry a one-line note or
be left"*, so leaving both would have been defensible — annotating one and not the other is not.)

**Fix:** rewrite `:917` as a clean two-sentence flag (original quote intact, then a separate
`⚠ SUPERSEDED — fixed in b3e635e; CLAUDE.md now says "14 vetted, feature-gated crates".`), and either
annotate `:421` the same way or drop the `:917` annotation.

### Q5 — **P2** · `"57 mcp/** files"` does not reproduce (48), and `J`'s per-bucket tally does not reconcile to its own `≈ 199 ✅ every file has an explicit verdict`

`register:278` (`GR-16`): *"**Hard OUT regardless:** all 57 `mcp/**` files"*; `J-claude-bundle.md:157`
repeats *"`mcp/**` (57 files …)"*; `:159` closes with *"**File tally check:** 48 skills + 23 hooks + 4 refs
+ 31 bin + 3 project-template + 57 mcp + settings.template + … ≈ **199 files** ✅ every file has an
explicit verdict."* Independently counted on the audited bundle (`claude-setup-global-20260722-103235`,
the same one `J:7` names) [Verified: `find`, no symlinks anywhere in the tree]:

| Bucket | J states | Counted | Note |
|---|---|---|---|
| **`mcp/**`** | **57** | **48** files (61 incl. 13 subdirs) | neither 48 nor 61 is 57 |
| `hooks` | 23 | **39** files (20 top-level entries) | neither counting method gives 23 |
| `bin` | 31 | **34** files (8 top-level entries) | neither gives 31 |
| `skills` | 48 | 48 | ✓ |
| `refs` | 4 | 4 | ✓ |
| `projects/.claude-template` | 3 | 3 | ✓ |
| **bundle total** | **≈199** | **199** | **✓ [Verified]** |

The **grand total of 199 is exactly right**, and every bucket has *some* verdict in the table — so the
audit's substance ("no silent omits") is not disproved. But the arithmetic the `✅` rests on sums to
**188**, not 199, with the stated buckets; and `register:278` hands the developer *"all 57 `mcp/**` files"*
as the hard-OUT scope of a **security** decision. `MANIFEST.json` carries no per-bucket counts, so the
numbers were hand-derived and three of them are wrong. **Fix:** recount the buckets, or drop the
per-bucket arithmetic and keep only the verified `199`.

*(Confirmed clean in passing: the bundle's own scrub is intact — `<mcp-client-N>` directory placeholders,
`acme_trivy_mcp` package name, `MANIFEST.scrubbed_placeholders` with 10 entries — and `J:157`'s promise
that the corporate filenames are *"deliberately not"* stated holds on the `J` surface itself.)*

### Q6 — **P3** · `M-certification-round3.md:42`'s zero-occurrence `[Verified]` claim is self-falsifying

`:42` reads: *"**Zero** occurrences of `jira.env`/`confluence.env`/`gitlab.env` anywhere under `docs/`
[Verified: `grep -rn` over the whole tree → empty]."* Re-derived now: `grep -rniF` returns **1 hit each,
and it is that line**. The check was true when run and the act of recording it made it false — but as
committed the `[Verified]` stamp does not reproduce, which is the one thing every stamp in this corpus is
supposed to guarantee. **No security consequence** (these are generic SaaS product names; the sensitive
tokens — `ACME_INTERNAL_PORTAL_COOKIE`, `internal-portal.cloud-acme.fr`, `AWSELBAuthSessionCookie`,
`CONFLUENCE_PERSONAL_TOKEN` — have **zero** hits under `docs/`). **Fix:** restate as *"zero occurrences
outside this line"*, or drop the three filenames from the sentence.

### Q7 — **P3** · two small carry-overs from the O5 / O6 fixes

- **Q7a** — O5's severity upgrade (`Content-Length: 2` ⇒ request-smuggling/desync, not only response splitting) is on `register:553-555` but **not** on `C-decisions.md:3461`, the `DEC-363` row the developer actually rules from. Under `d137ae5`'s own stated method ("grep the corrected fact across register + both cursors + `C-decisions.md` + all reports"), the canonical ruling row was the one surface that mattered most.
- **Q7b** — O6's rewording makes `register:498` enumerate 24 + 3 + ~22 = **~49** under a **~46** headline (see the O8/O6 rows above). Also worth one line of disclosure: §7's five categories (46 / ~30 / 17 / ~22 / 40) sum to 155 against **95** deduplicated items, i.e. they overlap heavily and the file never says so.

---

## Claims I independently re-derived

Three register claims that **no prior round tested**, chosen from the untested set, plus the mechanical
recount. All reproduced.

### 1. `§1 #4` / `GR-9` — *"`Input.lines()` already streams — 88 MB / 2 M lines in **23.7 MB peak RSS**"* → **[Verified — and stronger than stated]**

Source-level: `src/native/input.rs:132` uses `std::io::stdin().read_line(&mut line)` per pull, never a
`read_to_end` ✓. Live measurement (VM backend, `target/release/phg run`, peak `VmHWM` polled from
`/proc/<pid>/status`, `probe-cert4/{stream.phg,peak.sh}`):

| stdin | lines counted | peak RSS |
|---|---|---|
| empty | 0 | **23,664 KB** |
| 27.9 MB | 500,000 | **23,712 KB** |
| **112.9 MB** | **2,000,000** | **23,716 KB** |

Peak RSS is flat to within **52 KB across a 113 MB input range** — the figure is the binary's own baseline,
so `Input.lines()` is O(1) in input size. `C-stdlib-input-fs-clone.md:191`'s exact `23,712 KB` reproduced
to the kilobyte at a *different* input size. The register's *"~4× smaller than the file itself"* framing
understates it: the correct statement is that streaming costs **essentially nothing above process
baseline**. GR-9's premise holds.

### 2. `§1 #9` / `GR-12` — *"the `Database` object is provably ONE connection, not a pool or façade"* → **[Verified]**

`src/ext/database/natives/handles.rs:88-103`: `struct DbConn` holds exactly one
`driver: Rc<RefCell<Option<Box<dyn DriverConn>>>>` plus three connection-scoped shared cells
(`tx_depth: Rc<Cell<u32>>`, `hook: Rc<RefCell<Option<Value>>>`, `timeout_ms: Rc<Cell<i64>>`); its own doc
comment says *"all bindings name the same connection"*. `grep -rniw pool src/ext/database/` → **0 hits**
(the 66 repo-wide `pool` hits are all `src/serve/`'s OS-thread worker pool, a different subsystem).
`kind()` returns the literal `"db-connection"`. The rename premise is sound. **Bonus corroboration for
`GR-2`:** the same doc comment states *"an inner rollback never aborts the outer"*, which is precisely the
mechanism behind the auto-rollback data-persistence bug `GR-2` asks about — the two rulings are consistent.

### 3. `§1 #12` / `GR-10` — *"`std::fs::File::{lock, try_lock, unlock}` compile on the pinned rustc, and Rust std locks and PHP `flock()` block each other bidirectionally"* → **[Verified]**

Toolchain pinned to `1.97.1` (`rust-toolchain.toml`); `rustc --version` → `1.97.1 (8bab26f4f 2026-07-14)`.
`probe-cert4/lk.rs` using all three methods compiles clean with `rustc -O --edition 2021` (exit 0) — no
crate admitted, so **the dependency-policy blocker is indeed FALSE**. (Note for whoever builds GR-10:
the stabilised signature is `try_lock() -> Result<(), std::fs::TryLockError>`, **not** `Result<bool, _>`.)
Bidirectional interop against `/stack/tools/phpbrew/php/php-8.5.8/bin/php`:

| Scenario | Result |
|---|---|
| Rust holds `f.lock()` → PHP `flock(LOCK_EX\|LOCK_NB)` | `PHP try flock: WOULDBLOCK (wouldblock=1)` |
| PHP holds `flock(LOCK_EX)` → Rust `f.try_lock()` | `RUST try_lock: WOULDBLOCK (held elsewhere)` |
| control, nobody holding | both acquire ✓ |

Ladder **case 1** confirmed with the same strength the register claims. (`GR-10`'s Windows caveat remains
`[Unverified]` — see "Could not verify".)

### 4. Agenda recount from scratch → **fully CLEAN**

[Verified: `probe-cert4` python pass over `C-decisions.md` + the register.]
- **27** distinct `GR-` ids in the register: `GR-1`…`GR-27`, gap-free.
- **27** rows in `C-decisions.md` matching `^| DEC-3nn | GR-n |`, range `DEC-339`–`DEC-365`, **gap-free, zero duplicate ids**.
- **Mapping `DEC-(338+n) ⇄ GR-n` holds for all 27** — zero violations.
- **All 27 rows carry `PENDING`** — Invariant 15 honoured, matching `SLICE-STATE.md:14`'s claim.
- Every `GR-n` is defined in exactly the section §2's FULL AGENDA INDEX claims: `GR-1`…`GR-17` at register lines 82–283 (inside §2, which spans 72–298) · `GR-18`…`GR-24` at 407–436 (inside §6.4, 405–441) · `GR-25`/`GR-26` at 544/561 (inside §7.3, 542–567) · `GR-27` at 638 (§8.4). **No item is orphaned or double-homed.**
- Both cursors agree: `MASTER-PLAN.md:34` (*"BLOCKED ON 27 DEVELOPER RULINGS … `GR-1`…`GR-27` ⇄ DEC-339…DEC-365, 27 rows"*) and `SLICE-STATE.md:10-14` (same, with the identical §-split).
- `register:15`'s *"13 per-topic reports"* is right: `A`–`L` (12) + `P0-block-shadow…` (1) = 13, with the 3 `M-certification-round*` files on top = the 16 files on disk.
- *(Prompt cross-check: the grammar-leakage figure in the register is **81/383**, consistently at `:53`, `:106`, `:108`, `:535`. The **87** figure belongs to `GR-17` (`->` occurrences in `.phg`). Both are internally consistent — no defect.)*

---

## Could not verify

- **`GR-10` Windows lock semantics** — no Windows runner in this container. The register already grades this `[Unverified]` and flags it as must-be-surfaced-in-the-ruling; that grading is correct and I add nothing.
- **`GR-13`'s ~75× `bindNamed` quadratic figure and the DB auto-rollback persistence bug** — both need a live SQLite round-trip through `phg`; I chose the three probes above instead and did not run these. The *structural* premise behind the rollback bug is corroborated at the source level (`handles.rs:84-87`: *"`commit`/`rollback` `RELEASE`/`ROLLBACK TO` the innermost level … an inner rollback never aborts the outer"*), which is exactly the shape `GR-2` describes, but the 4.469 s / 0.059 s timings and the data-persistence reproduction are **[Unverified] by me**.
- **`§6.1`'s "87/383"-adjacent grammar and `unwrap()` census figures, the `1:9` span bug, the CRLF exploit, the block-shadow P0** — deliberately not re-run; prior rounds verified them and the prompt scoped them out.
- **`--all-features` correctness gate** — not run (no `src/` change in the reviewed series; disk at 87% / 5.2 GB free, and the round's mandate forbids cargo builds). The register's §5 already discloses this.
- **Whether `L-onhold-inventory.md`'s snapshot line counts should be refreshed** — `:590-592` cites `C-decisions.md` at "3376 lines" (now **3490**) and `SLICE-STATE.md` at "2308 lines" (now **2369**). Pre-existing snapshot drift from the review's own edits, same class as Q3; not counted as a round-4 finding but it will keep growing.

---

## Bottom line

**Round 4: NOT FULLY CLEAN — 7 findings (Q1–Q7: four P2, three P3). Two-consecutive-clean counter
remains 0.** Two findings (**Q2**, **Q4**) are *regressions introduced by `d137ae5` itself*, both caused by
applying a documentation fix with a scripted literal replace instead of reading each insertion point —
the mirror image of round 3's diagnosis, which was "don't fix only where the reviewer points". The
correct method is neither: **grep the fact to find every surface, then edit each one by hand and re-read
the resulting sentence.**

Two findings (**Q1**, **Q3**) are the *sixth* appearance of the incomplete-blast-radius pattern
(`M1 → N1 → O1 → Q1`; `N8 → O2 → Q3`), and Q1 is the most serious version yet because round 3 supplied an
explicit four-line list and two entries were skipped while the commit message claimed completion.

**Nothing found changes any ruling.** The 27-item agenda is structurally sound and verified gap-free; all
three previously-untested register claims I probed reproduced, one of them more strongly than written.
The register is safe to rule from — but `L-onhold-inventory.md:570`, `H-docs-consistency.md:1543` and
`C-stdlib-input-fs-clone.md:917` will actively mislead a developer working the §7.1 stale-label pass, and
should be fixed before that pass is run.
