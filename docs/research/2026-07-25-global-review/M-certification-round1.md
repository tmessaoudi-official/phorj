# DEC-268 MAXIMAL certification — round 1, fresh-context adversarial reviewer
## Artifact: `docs/research/2026-07-25-completeness-register.md`
## Reviewer stance: evidence-based — every claim below was re-derived from the code/binaries, not read off the author's narrative.

Environment used: `target/release/phg` (`phg 1.0.0-nightly.0`), `/stack/tools/phpbrew/php/php-8.5.8/bin/php`
(`PHP 8.5.8`), `rustc 1.97.1 (8bab26f4f 2026-07-14)` matching `rust-toolchain.toml` `channel = "1.97.1"`.
Probes under `scratchpad/probe-verify/`. All repo-relative greps run from `/home/user/phorj`.

---

## Verdict per lens

| Lens | Verdict |
|---|---|
| **1 — Correctness (load-bearing claims)** | **CLEAN.** Every technical claim I independently re-derived was confirmed, several to exact line-number precision. Zero false factual claims found in the code-facing body of the register. |
| **2 — Security & safety-promises** | **FINDINGS (2, both LOW–MEDIUM).** No secret/credential leak. One understated risk (M4), one public-repo metadata nit (M6). The `no src/ modified` promise is kept. |
| **3 — Completeness & blast radius** | **FINDINGS (4).** One MEDIUM-HIGH Invariant-19 divergence (M1) that will directly mis-scope the developer's day, plus an uncorrected false claim left in the evidence base for the P0 (M2), a census-methodology gap (M3), and a self-inflicted stale label (M5). |

**Net:** the register's *facts* are sound; its *front matter and evidence hygiene* are not. Fix M1 and M2
before the developer opens it.

---

## Findings

### M1 — **MEDIUM-HIGH** · The agenda size is understated by 10 items in every summary surface, including three of the four canonical homes (Invariant-19 breach)

**Register claims** (line 5): *"Ruling rows live in the decision register (`…/C-decisions.md`, **DEC-339 … DEC-355**)"*
and (line 71) *"## 2. TOMORROW'S AGENDA — **17 rulings**"*.
`docs/plans/SLICE-STATE.md:3`: *"GLOBAL REVIEW DONE, **17 RULINGS** AWAIT THE DEVELOPER"*; `:11` *"the **17-item**
agenda (`GR-1`…`GR-17`)"*; `:13` *"**DEC-339…DEC-355**"*.
`docs/plans/MASTER-PLAN.md:34`: *"BLOCKED ON **17 DEVELOPER RULINGS**"* … *"`GR-1`…`GR-17` ⇄ **DEC-339…DEC-355**"*.

**What I observed.** The real agenda is **27 items**:
```
distinct GR ids in register: 27
GR-1 … GR-27
distinct DEC-339..365 rows in C-decisions.md: 27
```
The range `DEC-339…DEC-365` is complete, gap-free, and non-colliding (highest pre-existing id was
`DEC-338`, verified via `git show 68dca8e^:…/C-decisions.md`). Each DEC maps 1:1 to a `GR-` that exists.
The understatement accumulated because commit parts 4 (`1cc4357`, DEC-356..362), 5 (`4132139`, DEC-363/364)
and 6 (`49e1ebd`, DEC-365) touched only the register + `C-decisions.md` — `git show --stat` for parts 4/5/6
shows **no** `SLICE-STATE.md` or `MASTER-PLAN.md` in the diff.

**Why it matters.** CLAUDE.md Invariant 19 requires MASTER-PLAN + register + SLICE-STATE to be *"kept
mutually consistent in the SAME change"*, and SLICE-STATE is the designated *"read first on resume"* cursor.
A developer planning from any summary line will schedule 17 decisions and silently drop 10 — including
**GR-18** (which the register itself calls *"the single highest-value structural improvement found"*),
**GR-25** (the security item), and **GR-27**.

**Exact correction.** Replace `17` → `27` and `DEC-339…DEC-355` → `DEC-339…DEC-365` in: register line 5,
register line 71, `SLICE-STATE.md:3`, `:11`, `:13`, and `MASTER-PLAN.md:34`. Note the register also splits
the agenda across §2 (GR-1..17), §6.4 (GR-18..24), §7.3 (GR-25/26) and §8.4 (GR-27) — a single consolidated
index line would prevent recurrence, and is exactly what GR-24's one-row-per-DEC guard is meant to catch.

---

### M2 — **MEDIUM** · The P0's own evidence file still carries a retracted, FALSE claim, labelled `[Verified]`

**Register claims** — §0: *"Details: `2026-07-25-global-review/P0-block-shadow-byte-identity.md`"* (i.e. it
points the developer at that file for the single most important item), and §5: *"Two of this run's own inline
conclusions were wrong and are **corrected in place** in `K-inline-findings.md`."*

**What I observed.** `P0-block-shadow-byte-identity.md` ends with:
> *"`for (xs as int x)` is a **parse error** … So the surviving loop form is `for (item in collection)` and
> the `as` form is **retired** — this answers the developer's question #7 directly (his recollection was
> inverted). [Verified: parse error text]"*

There is **no** correction note anywhere in that file (`grep -n "CORRECT\|RETRACT\|superseded"` → only
unrelated hits). The claim is **false**, and I disproved it directly:
```
foreach (xs as int x) { … }   ->  feas 1 / feas 2 / feas 3      (WORKS)
for (int x in xs)     { … }   ->  forin 1 / forin 2 / forin 3   (WORKS)
```
`K-inline-findings.md` K-1 does correct it (*"⚠ CORRECTED … that was WRONG"*), and the register's §1 #7
verdict is right. So the correction was applied to *one* of the two files carrying the error.

**Why it matters.** The register's §1 #7 exists specifically to un-invert the developer's memory, and
**GR-5/DEC-343 is a ruling about exactly this**. Sending him to the P0 file — which he *will* read first,
since it is the "decide first" item — re-installs the inverted belief the register just corrected, with a
`[Verified]` stamp on it. §5's "corrected in place" is therefore an overclaim.

**Exact correction.** Append to the P0 report's final section: *"⚠ CORRECTED — this conclusion was WRONG.
Both loop forms are live; each keyword is locked to one separator (`for`…`in`, `foreach`…`as`). Only the
crossed forms error. See `K-inline-findings.md` K-1 and register §1 #7."* Or delete the section — it is
unrelated to the P0 anyway.

---

### M3 — **LOW–MEDIUM** · The `unwrap()` census excluded whole files, so §6.1's figure is ~30% low and ~6 sites were never read

**Register claims** (§6.1, the paragraph whose stated job is to license trust in the criticism):
*"**~20 `unwrap()` in 155k lines**"*. Source: `G-rust-quality.md:657` —
*"`unwrap()` | 217 | **≈20** outside test code [Verified: per-file census **excluding files containing
`cfg(test)`**, then read every remaining site]"*. G24 then recommends: *"**Leave the rest — they are
correct**, and 20 justified unwraps in 155k lines is …"*.

**What I observed.** The methodology drops an entire production file whenever it happens to carry an inline
`#[cfg(test)] mod tests`. Counting production code properly — excluding test *files* and `cfg(test)`
*blocks* but keeping production code in files that also contain tests:
```
DEFINITIVE production .unwrap() count = 26
```
Six of those 26 sit in files the G census skipped wholesale and therefore **never read**:
`src/pm/resolve.rs:93`, `src/pm/manifest.rs:196`, `src/bundle/sha256.rs:29`, `:30`,
`src/native/random.rs:147` (`draw.try_into().unwrap()` — in the RNG path).
Corroborating figures I did confirm: `src/` is **566** `.rs` files and **154,817** lines (register: "566
files", "155k lines" — both exact).

**Why it matters.** Low decision impact (no GR rests on it), but §6.1 is explicitly the trust-licensing
section, and G24's *"read every remaining site … leave the rest, they are correct"* is an assurance about a
set that provably omits 6 members.

**Exact correction.** §6.1: *"~20"* → *"26 production `unwrap()`"*. G24: re-scope the census to exclude
`cfg(test)` *blocks* rather than whole files, and read the 5 unreviewed sites above before repeating
"leave the rest".

---

### M4 — **LOW–MEDIUM (understated risk)** · GR-25's response-splitting exposure is real, verified, and ranked 25th of 27 as "small"

**Register claims** (§7.3): *"**GR-25 (DEC-363) — Response-side CRLF guard.** The outbound sink is unguarded
(header-injection shape). Recommended: guard it; **small** and security-relevant."* No `P` severity is
assigned, and it is placed 25th of 27 — below items tagged P1/P2.

**What I observed — the mechanism, confirmed end-to-end.** `src/cli/http_prelude.rs:52` `class Response`:
```
function withHeader(string name, string value): Response {
    return new Response(this.status, this.body, List.concat(this.headerLines, ["{name}: {value}"]));
}
function withCookie(Cookie c): Response {  … ["Set-Cookie: {line}"] … }
function serialize(): bytes { … string userHeaders = String.join(this.headerLines, nl); … }
```
`name`/`value` are interpolated into a header line with **zero validation**, then CRLF-joined into the
response head. `src/serve/handlers.rs:189` `respond_once` returns the handler's `Value::Bytes` **verbatim**
(`b.as_ref().clone()`) — there is no Rust-side response serializer that could re-validate. So the entire
response head is opaque user-assembled bytes. `grep` for any CRLF validation across `src/` returns nothing
on this path.

**The asymmetry is the aggravating fact.** The sibling *request* path already implements exactly this guard:
`src/ext/http_client/natives.rs:116` → `"Core.HttpClientModule: header \`{n}\` contains a forbidden character"`,
pinned by `src/ext/http_client/tests.rs:450 header_injection_is_rejected_at_the_gate` (which feeds
`"a\r\nHost: evil"` and asserts rejection). The fix is a copy of code already in-tree.

**Why it matters.** This is textbook HTTP response splitting, reachable from ordinary handler code, on a
shipped `phg serve`, with `withCookie` (commonly user-derived values) flowing through the same sink. Calling
it "small" and ranking it 25th risks it being deferred past cosmetic items.

**Exact correction.** Tag GR-25 **P1**, state the mechanism (`withHeader`/`withCookie` → `serialize()`),
cite the existing request-side guard as the template, and promote it into the top-10 alongside the other P1s.

---

### M5 — **LOW** · SLICE-STATE still lists a fix that this same night's commit already applied

**Register/cursor claims.** `SLICE-STATE.md` §*"Needs NO ruling — safe to execute autonomously"* lists:
*"`CLAUDE.md:9` dependency correction (**says "four" vetted exceptions; actual is 14 optional deps…**)"*.

**What I observed.** That correction was applied in commit `b3e635e` (part 3) the same night. `CLAUDE.md`
now reads *"**14 vetted, feature-gated crates**"* and enumerates them. I verified the count independently:
`grep -cE "^[a-z][a-z0-9_-]* = .*optional = true" Cargo.toml` → **14**, and the enumerated set matches
exactly (argon2, unicode-segmentation, rustls, webpki-roots, corosensei, lettre, rusqlite, postgres, mysql,
cranelift, cranelift-jit, cranelift-module, regex, ctrlc). The register's §6.5 *does* record it as
"Already fixed tonight" — so the register and the cursor now contradict each other.

**Why it matters.** Mild irony worth naming: the pass whose headline finding is *"40 stale status labels"*
created a fresh one. It would cost the developer a few minutes re-doing a done fix.

**Exact correction.** Delete that bullet from `SLICE-STATE.md`'s no-ruling list (the register §6.5 entry is
the canonical record). Same sweep should re-check the other bullets in that list against part-3's diff.

---

### M6 — **INFO** · The register names corporate MCP config filenames in a public repo, using "public repo" as the reason to exclude them

**Register claims** (GR-16): *"**Hard OUT regardless:** all 57 `mcp/**` files — corporate tooling artifacts
(`jira.env`, `confluence.env`, `gitlab.env`) with zero relevance, and `phorj` is a **public** repo."*

**What I observed.** The security scan is otherwise **clean** — I grepped the whole committed
`docs/research/2026-07-25-global-review/` tree plus the register for tokens (`ghp_`, `glpat-`, `xox[baprs]-`,
`sk-…`), `Authorization:`/`Bearer`, `api_key`/`secret`/`passwd`, PEM private-key headers, internal
hostnames (`*.grdf|engie|intra|corp|local|internal`), and RFC1918 IPs. **Zero real hits** — every match was
benign (`Secret<T>` is a phorj type; `private function secret` is a visibility probe; `SECRET_PRELUDE` is a
Rust const). No credential, no `.env` content, no hostname, no corporate identifier was committed.
`J-claude-bundle.md:157` gives all 57 `mcp/**` files an explicit HARD-OUT verdict with the security
rationale — the exclusion decision itself is correct and well-reasoned.

The residual is metadata only: the public repo now states that the developer's environment carries
Jira/Confluence/GitLab/Trivy MCP configs. Sensitivity is very low (ubiquitous tooling) and it is arguably
necessary to justify the exclusion. Flagged for awareness, not as a defect.

**Exact correction (optional).** If the developer wants zero corporate footprint: replace the three
filenames with *"three corporate service `.env` files"*. No other change needed.

---

### M7 — **INFO / nit** · Two sample-dependent counts are stated as fixed facts

**Register claims** (§6.2 I1): *"`phg disassemble` yields **5 distinct outputs in 12 runs**"*.
**What I observed** (20 runs): **6** distinct orderings, then **5** on a second independent 20-run batch.
The non-determinism is fully real and reproducible; the *count* is a draw from a distribution, not a
property. Same shape applies to I7's "3 different answers across 20 runs" (I measured 3 in 25 — stable
because there are only 3 candidates). Cosmetic; suggest phrasing as *"≥5 distinct outputs (6 observed in
20 runs)"*.

---

### M8 — **INFO (process)** · The artifact was a moving target during certification

At review start the register was 555 lines with a 26-item agenda (`DEC-339…DEC-364`). Commit `49e1ebd`
(*"part 6"*) landed mid-review, adding §8 and GR-27/DEC-365 → **632 lines, 27 items**. My findings are
against the 632-line `HEAD` state (working tree clean, `git status --porcelain` empty). Noting it because
M1's severity is a function of this growth: each new part widened the gap against the "17" summary lines.

---

## Claims I independently CONFIRMED

Named with what I actually ran — a clean lens is only meaningful if the checks are visible.

**§0 / GR-1 — the P0 (fully confirmed, including the absence claim)**
1. Divergence reproduced on all three legs: `phg run` → `out=1`, `phg run --tree-walker` → `out=1`,
   `phg transpile | php-8.5.8` → **`out=2`**. Emitted PHP inspected: `$a = 1; if (true) { $a = 2; … }` —
   same `$a`, no new scope. Root cause exactly as stated.
2. **All six shapes** verified individually, each on all three legs: bare block, `if`, `for`, `while`,
   **parameter shadow** (`outer v=7` vs `outer v=42`), **3-deep** (`d3=3,d2=2,d1=1` vs `d3=3,d2=3,d1=3`).
3. **Sibling control confirmed negative:** two sequential non-shadowing blocks agree on all three legs
   (`b1=1|b2=s`) — so the claim is correctly scoped to *live outer* shadowing.
4. **The absence claim tested properly, not grepped.** I wrote a comment/string-stripping, brace-depth
   scope tracker, **validated it against 6 known positives and 1 known negative** (first two versions had
   false negatives and were discarded — a zero result from an unvalidated detector is worthless). Result:
   **0 nested shadows in 266 `examples/**/*.phg`, and 0 across all 383 repo `.phg` files.**
5. `tests/differential.rs` coverage confirmed: recursive `collect_phg(Path::new("examples"))` at `:1818`
   (interp≡VM glob) and `:3050` (PHP-oracle glob), with documented quarantines (`interop/`, `database/`,
   `mail/`, `process/`). The register's "globs `examples/**/*.phg`" is accurate modulo those carve-outs.

**§1 finding #7 / GR-5 — loop forms**
6. `for (int x in xs)` works; `foreach (xs as int x)` works.
7. Both crossed forms error precisely: `foreach (int x in xs)` → *"expected 'as' after the foreach
   iterable"*; `for (xs as int x)` → *"expected 'in' in for-loop header"*.
8. `grep -rn "E-RETIRED-FORIN" src/` → **0**. Repo-wide it appears in **docs only** (6 files, all plans/
   research). Ruled-but-never-built confirmed.

**§1 finding #15 / GR-11 — `p with { }`**
9. Works on all three legs (`p.x=1 q.x=99`). **Shallow confirmed**: mutating `q.inner.n = 77` also reads
   `p.inner.n=77` — the inner object is shared.
10. Transpiles to **bare `clone($p)`** (line 10 of emitted PHP). Safe pre-8.5 too, since `clone($p)` parses
    as the operator applied to a parenthesized expr.
11. Rejected spellings confirmed: `clone p` → parse error; `p.clone()` → *"type `Point` has no method `clone`"*.
12. **Formatter double space confirmed** via `cat -A`: `mutable Point q = p with {  };`.
13. **Lift refusal confirmed (H9 / Invariant-17 gap):** `phg lift` on *the transpiler's own output* →
    `lift parse error: \`clone\` is Tier-2/Tier-3`.

**§1 finding #14 / GR-6 — `main` reservation**
14. `type_bodies.rs:347` citation is **exact** — that line is
    `let is_entry_main = f.name == "main" && (self.cur_class.is_none() || self.in_static_method);`.
    No `#[Entry]` consultation anywhere in the predicate.
15. A library `function main(string s): string` with **no** `#[Entry]` is rejected: `[E-MAIN-SIGNATURE]`.
16. `#[Entry] function startHere()` with **no** `main` at all runs fine — the attribute does free the name.
17. `E-MULTIPLE-MAIN` has **no emission site** in `src/` (only `cli/explain/members_destructure.rs:94-95`,
    four doc comments, and one *negative* test assertion), yet `phg explain E-MULTIPLE-MAIN` prints a full
    teaching entry. "Dead code still taught" confirmed.

**§6.2 I8 — the SECOND exception to Invariant 1 (the strongest claim; fully reproduced)**
18. Reproduced exactly: VM → `runtime error at 9`, underlining `get => this.p;`, **4099** stderr lines,
    frames `C::p$get line 9`. Tree-walker → `runtime error at 17`, underlining `int v = c.p;`, **4** stderr
    lines, frame `main line 17`. Line **9 vs 17**, **4099 vs 4** — matches the claim digit for digit.
19. **Isolation control confirmed** — and this required care. My first control used
    `Output.printLine("d={deep(1)}")`, which diverged (`main line 1` vs `line 7`) — but that is the
    *documented* interpolation carve-out (`INVARIANTS.md` §7), not a new one. Re-run with the call in a
    plain statement (`int d = deep(1);`): **byte-identical stderr and stdout on both engines.** So the
    divergence genuinely is hook-specific.
20. Framing confirmed by source: `INVARIANTS.md:12-15` defines `agree_err` as matching on the fault **body
    substring**, and §7 names the interpolation skew as *"the one exception"*. I8 is outside it. The
    "second exception" characterisation is correct.

**§6.2 I1 / I7 — determinism (≥15 runs each, as instructed)**
21. **I1, 20 runs:** 6 distinct `CallOverload` id orderings (5/4/4/3/2/2); a second 20-run batch gave 5
    distinct. Non-determinism confirmed.
22. **I1 blast-radius bound confirmed:** 20 `phg run` invocations → **20/20** identical (`ai bs gi`);
    20 `phg transpile` → **20/20** identical md5. The spine is genuinely intact.
23. **I7, 25 runs:** `car` ×11, `cot` ×8, `cut` ×6 — **3 distinct answers**, confirmed.
24. **I7 root cause confirmed by reading the source**, and the citation `plumbing.rs:160-167` is **exact**:
    `nearest_name` spans those lines, `min_by_key` over a `Vec` built by `in_scope_names` (`:144-156`,
    also exact) via `extend(scope.keys())` / `extend(self.funcs.keys())` / `extend(info.fields.keys())` —
    all `HashMap` keys.

**§6.1 positive attestations**
25. **Zero `unsafe` outside `src/jit/`** — 0 real occurrences; the single grep hit is a *test function name*
    (`green/spike.rs:36 fn coroutine_suspends_from_deep_recursion_without_unsafe`).
26. **Zero `todo!`/`unimplemented!`** — 0 repo-wide in `src/`.
27. **Zero production `panic!`** — all 375 occurrences are inside `#[cfg(test)]` blocks or test files;
    brace-matched analysis gives **0** in production code.
28. **All three Invariant-3 matches wildcard-free** — `src/compiler/emit.rs`: no `_ =>` at all;
    `src/chunk/validate.rs`: the only hit is a *comment* at `:41` documenting the closed `_ => None` gap;
    `src/vm/exec.rs`: `_ =>` at `:847`, `:893`, `:986` are on `Value`/receiver-kind/closure matches, **not**
    on `Op`. Exactly as claimed.
29. `src/` is **566** `.rs` files, **154,817** lines — both figures exact.

**§7.1 stale labels — 6 spot-checks (3 each direction), all confirmed**
*Recorded OPEN but actually BUILT:*
30. **Tuples (DEC-288)** — `var (id, name) = labelled(7);` → `8 many`. Built.
31. **HOF `List.map/filter/reduce`** — `List.map`/`filter`/`reduce` with `function(int x) => …` → `sum=18`. Built.
32. **P-Q-A-5 file-size debt** — `bash scripts/size-gate.sh` → `[size-gate] grandfathered=78 **fails=0**
    warns=118 stale=6 … OK`. The `fails=0` claim is literally true.
*Recorded DONE but NOT:*
33. **W2-4 `->` retirement** — `function twice(int n) -> int` *and* `function main() -> void` both still
    parse and run (`42`). Unbuilt, confirmed.
34. **Wildcard spec header self-contradiction** — `docs/archive/specs/2026-07-24-wildcard-imports.md:1` and `:3` say
    *"NOT YET BUILT" / "BUILD-READY, NOT BUILT"* while `:228` says `## ✅ Q-A DONE (2026-07-25 — DEC-268
    certified)`. Exactly the contradiction claimed.
35. **`E-RETIRED-FORIN` absent** and **`E-MULTIPLE-MAIN` explained-but-never-emitted** — both confirmed
    above (#8, #17).
36. I also stress-tested the **DEC-247 DateTime** claim rather than accepting it: `Core.DateTime`,
    `Core.Date`, `Core.Instant`, `Core.Time` all *resolve* as imports, which looked at first like a
    contradiction. It is not — `L-onhold-inventory.md:93` scopes the claim precisely to *"`Core.DateTime`
    **with a vendored-IANA tz crate**"* (named zones + DST), and there is no tz crate among the 14 optional
    deps and no `Core.DateTime` module; what exists is a tz-less `Core.Time` (`Date`/`Instant`/`Duration`,
    reached as `Time.Date` or member-imported). The L report is correctly scoped; the register's compressed
    phrasing is a fair summary. **Not a finding.**

**§1 finding #1 / GR-7 — package validation follows the import graph**
37. Reproduced exactly, and it is a striking result: `package Foo.Bar;` in the source root with only
    `Core.*` imports → **runs fine** (`loose pkg ok`). The *same* package declaration plus **one user
    import** → `package \`Foo.Bar\` cannot sit directly in the source root … [E-PKG-PATH]`.
38. Mechanism confirmed at the cited lines: `src/loader/entry.rs` `load_unified_src` returns early at
    `if queue.is_empty() && collect_unified_decls(&roots)?.is_empty()` — **before** any validator. The
    citation `entry.rs:53-66` matches the fast-path block precisely.

**§6.2 G26 — fault-string drift is invisible, not merely untested**
39. Confirmed by reading `tests/differential.rs:129 fn classify`: it re-types every canonical fault body as
    its own literal (`err.contains("integer overflow")`, `"division by zero"`, `"modulo by zero"`,
    `"stack overflow"`, `"list index out of range"`, `"force-unwrap of null"`, …) instead of deriving from
    the canonical `FaultMsg` consts. Drift between a backend and the canonical const cannot fail this test.

**GR-10 — file locking (verified with a control, not just a compile)**
40. **`std::fs::File::{lock, try_lock, unlock}` are stable on the pinned toolchain** — a direct
    `rustc -O` compile (no cargo, no target dir) of a probe using all three succeeded on
    `rustc 1.97.1`, which matches `rust-toolchain.toml`. No crate needs admitting: confirmed.
41. **Rust std locks and PHP `flock()` block each other bidirectionally** — proven with a negative control
    first: on an unlocked file both `try_lock` and `flock(LOCK_EX|LOCK_NB)` **succeed** (so the probes are
    meaningful). Then: Rust holds `lock()` → PHP `flock(LOCK_EX|LOCK_NB)` **fails** (`wouldblock=1`);
    PHP holds `flock(LOCK_EX)` → Rust `try_lock()` **fails** (`WouldBlock`). Ladder-case-1 claim confirmed
    with unusually strong evidence, as the register says.
42. **The Windows caveat is honestly labelled.** §7.3/GR-10 states Windows lock semantics *"may be
    mandatory rather than advisory"* and that any cross-platform guarantee is currently `[Unverified]`,
    with no Windows CI. Correct labelling — I cannot verify it either (no Windows runner).

**The five docs FIXES applied tonight — each verified to do what its commit claims, with no new error**
43. `docs/INVARIANTS.md` §1 identifiers all real: `cmd_run_exit` → `src/cli/pipeline.rs:439 pub fn
    cmd_run_exit`; `built_binary_matches_vm` → `tests/build.rs:204`; `cross_musl_binary_matches_vm` →
    `tests/build.rs:140`. The corruption described in `b3e635e` (`cli::cmd_the VM leg` etc.) is gone, and
    the section now correctly says the hook dispatches `cmd_run_exit`, matching `src/main.rs:32`
    (`match cli::cmd_run_exit(&src)` — verified at that exact line).
44. All four corrected module paths exist **and** the line numbers land on the right items:
    `src/vm/exec.rs:9` → `fn exec_op`; `src/chunk/validate.rs:21` → `pub fn validate`;
    `src/compiler/emit.rs:75` → `fn stack_effect`; `src/value/arith.rs` exists.
45. **`14` is the right dependency count** — `grep -cE "^[a-z…] = .*optional = true" Cargo.toml` → **14**,
    and the enumerated set in CLAUDE.md matches the file exactly. Only `[dependencies]` and one
    `[target.'cfg(not(wasm32))'.dependencies]` table exist; no optional deps hide elsewhere.
46. **Orphaned-SHA edits are correct and well-reasoned** — both cursors previously pinned `6e0c58a` /
    `dee608e` (orphaned by a history re-sign) and *named different tips*; they now name ref + subject
    instead of a bare SHA. Verified against the `b3e635e` diff.
47. **README fix verified by execution, not reading.** Hero snippet extracted verbatim from the fence at
    line 20 → runs, `rc=0`, `area = 12.56636 / area = 12`, and is **byte-identical on all three legs**
    (vm/tw/php). `phg run examples/hello.phg` → `Hello, Phorj!`. Quickstart one-liner A
    (`echo … | phg run -`) → `3`. Quickstart one-liner B (`phg run -e …`) → `inline!`. Both match the
    documented output exactly. Bonus: the `test` fence at line 185 also runs
    (`1 passed, 0 failed, 1 tests in 1 files`).

**Other verified structural claims**
48. **`no src/ modified` promise kept** — `git diff --stat b30d9b5^..HEAD -- src/ tests/ Cargo.toml` is
    **empty**. The full 21-file diff is docs-only. §6.6's rationale for changing no code is therefore
    truthful, and §5's "the `--all-features` gate was not re-run (no source changed)" is a sound inference.
49. **DEC range integrity** — 27 rows, `DEC-339…DEC-365` gap-free, no collision with pre-existing ids
    (max was `DEC-338`), each mapping 1:1 to an existing `GR-`.
50. **All 15 developer findings are answered in §1**, none left blank or evasive. #8 is labelled
    *"AMBIGUOUS — 5 distinct features"* and routed to GR-14 for disambiguation — that is an honest answer
    to a genuinely ambiguous question, not a dodge. The scorecard's arithmetic checks out
    (6+4+3+1+1 = 15).
51. **Stdlib-wildcard gap (finding #5) confirmed** — `import Core.String.*;` → `[E-WILDCARD-STDLIB-ROOT]`
    *"wildcard import of the standard-library module … is not yet supported"*. Matches the claim.
52. **Evidence grading is generally disciplined.** §8.3 is a model: it states both numbers as `[Verified:
    read from the harness output]` and explicitly downgrades the *interpretation* to `[Inferred]` because
    the two workloads were not diffed. §5's honest-limits list (403'd vendor docs, `[Inferred-strong]`
    MySQL grammar claim, `[Unverified]` Windows locks, gate not re-run) matches what I could and could not
    confirm. I found **no** case of `[Inferred]` being presented as `[Verified]`.

**Bonus observation (not a register defect)**
53. The `E-MISSING-RETURN-TYPE` hint teaches the syntax GR-17 proposes to retire:
    *"add `-> void` for a side-effecting function"*. Any W2-4 execution must update that hint text too —
    worth adding to GR-17's checklist, which currently lists `.phg` sites, `.rs` fixtures and the
    comment/prose arrows but not diagnostic hint strings.

---

## Claims I could NOT verify (and why)

| Claim | Why not | Register's own labelling |
|---|---|---|
| Windows advisory-vs-mandatory lock semantics (GR-10) | No Windows runner in this container | Correctly `[Unverified]`, with "no Windows CI" stated |
| Cross-language DB naming survey ("8 of 10 ecosystems call this `Connection`") (§1 #9 / GR-12) | Vendor doc fetches return HTTP 403; I did not re-attempt network | Correctly `[Unverified]` in §5 |
| MySQL/Postgres nested-savepoint grammar portability (D5 / GR-13) | Needs live MySQL + Postgres servers; `PHORJ_*_TEST_DSN` absent | Correctly `[Inferred-strong]`, resting on the module's internal self-contradiction |
| The `--all-features` correctness gate | Explicitly out of scope (no cargo builds — disk) | §5 discloses it was not re-run; I independently confirmed the premise (`src/` untouched) |
| The 81/383 → 0/383 TextMate leakage measurement (GR-3) | Requires a `vscode-textmate` harness not present | Stated as measured; the 383 total matches my `.phg` file count exactly, which is a weak corroboration only |
| 2223 / 1231 / 55.4% UFCS corpus counts (GR-8) | Not re-derived — regex-sensitive, and no GR-8 option turns on the exact figure | Stated without derivation; low risk |
| "87 `for…in` vs 8 `foreach…as`" census (GR-5) | My crude regex gave **84** vs **8** — within regex-precision noise of 87, and the 8 matches exactly | The 87:8 *ratio* argument is unaffected |

---

## Bottom line

**Is the register safe to base 27 binding decisions on? YES — after applying M1 and M2.**

Every load-bearing technical claim I attacked independently survived, and several survived at
line-number precision (`type_bodies.rs:347`, `plumbing.rs:160-167`, `loader/entry.rs:53-66`,
`vm/exec.rs:9`, `chunk/validate.rs:21`, `compiler/emit.rs:75`, `main.rs:32`). The P0 reproduced on all
three legs across all six shapes with a correctly-negative sibling control; its absence claim held under a
*validated* detector; I8 reproduced digit-for-digit with a correct isolation control; the locking claims
held under bidirectional testing with a negative control; the README fixes execute. I found **no** false
factual claim capable of producing a wrong ruling, and **no** case of an `[Inferred]` claim dressed as
`[Verified]`.

The defects are in framing and evidence hygiene, not facts: the agenda is **27** items, not the **17**
that three of the four canonical homes advertise (**M1**, the one that will actually cost him decisions);
the P0's own evidence file still carries a retracted false claim under a `[Verified]` stamp, contradicting
the register's §1 #7 and pre-loading GR-5 wrongly (**M2**); the `unwrap()` census understates by ~30% and
left 6 sites unread (**M3**); GR-25 is a verified response-splitting exposure ranked 25th as "small"
(**M4**); and one already-applied fix is still queued as outstanding (**M5**).

**Round 1 = FINDINGS. Not clean.** Per the DEC-268 ladder the clean counter stays at 0; fix M1/M2 (and
preferably M3–M5), then re-round. None of the findings requires re-doing research — all five are edits to
summary lines, one appended correction note, one severity tag, and one deleted bullet.
