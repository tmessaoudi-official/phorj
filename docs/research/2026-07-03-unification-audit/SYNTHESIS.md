# Unification Audit — Stage B Synthesis (2026-07-03)

> Merged, deduped, severity-ranked synthesis of the 10 parallel Stage-A audit reports
> (A1–A10, `raw/`). HEAD `0691228`, clean tree. Severity taxonomy: Rule 6 (P0 blocks
> correctness/security · P1 high-impact quality · P2 minor · P3 stylistic/optional).
> Evidence grades preserved from source reports (Rule 18: Verified / Inferred / Unverified /
> Speculative). This document feeds the Stage-C ask-human decision gate; raw reports remain
> the full-detail backup.

## Executive summary

The single most important finding of the whole audit is a **cluster of four undisclosed
byte-identity (Invariant 1) violations** found by fuzzing — `String.trim` (Unicode-vs-PHP
whitespace set), `String.reverse` (char-vs-byte), `String.split("")` (total-vs-fatal), and
`Hash.pbkdf2` iteration truncation (which is simultaneously a silent crypto-weakening bug) —
each needing a per-case "which side wins" ruling from the developer (Bucket 2). Their **root
cause is structural**: four independently-maintained execution paths (interpreter / VM /
transpiler / lifter, ~28% of src + the heaviest harness) drift at exactly the native-vs-PHP
seam, which is why A8 recommends promoting A5's fuzz probes into the standing gate rather than
cutting backends. The biggest just-do-it win is the **test-gate latency complex**: the
"cmd_run global lock" in project memory is not a Mutex — it is a per-call 256 MiB thread spawn
plus two monolithic corpus tests (200 s + 155 s) that serialize nextest to ~2.5× on 8 cores;
a persistent deep-stack worker + test sharding + the already-installed-but-unwired mold linker
plausibly halve the 113 s commit gate (Bucket 1). On the doc side, the shipping docs contain
**five P0-grade false-claim families** — "zero external deps", `import type` taught as current
syntax, four dead CLI verbs (declared *stable* in STABILITY.md), the nonexistent
`E-TRANSPILE-CONCURRENCY` code, and 🔲-markers on shipped features that make the GA % figure
untrustworthy — all resolvable by Stage D with no code changes (Bucket 3). Finally, the
deprecated-`->` problem surfaces in two places that must be fixed together: the parser still
silently accepts it at 6 sites AND the CLI's own `--help` examples still teach it — the
reject-timing decision is Bucket 2; the watch list (Bucket 4) is headed by the four-backend
tax itself and the injected-prelude registry growth that W3/W4 will compound if not redesigned
first.

**Finding counts**: Bucket 1 = 19 · Bucket 2 = 13 · Bucket 3 = 14 · Bucket 4 = 15 (61 total;
cross-referenced items counted once, in their primary bucket).

## Cross-reference merges performed (read before the buckets)

1. **"cmd_run global lock" = test-gate serialization** — A6 F5 (no Mutex exists; the
   serializer is `on_deep_stack`'s per-call 256 MiB thread spawn, measured ~97 µs/call,
   8-way scaling 4.8× vs 7.2×) and A9 §3/§4 (pre-commit gate 112.8 s of which 111 s is
   nextest at ~2.5× CPU utilization on 8 cores) are the SAME issue from two angles, compounded
   by A6 F6 (two monolithic corpus tests, 200.5 s + 154.7 s, that nextest cannot parallelize).
   Merged → **B1-1**. *Project memory ("cmd_run has a global lock") should be corrected.*
2. **pbkdf2 truncation joins the byte-identity cluster** — A7 M-1 is a security finding AND a
   byte-identity break of the same family as A5's three fuzz P0s (native computes with the
   wrapped u32; the PHP leg receives the untruncated i64). Merged → **B2-1**, cross-tagged
   security.
3. **A8 F2 is the root cause of B2-1** — four independently-maintained execution paths drift;
   A5's P0s and A7's M-1 are that tax materializing despite a 174-program differential corpus.
   Stated as a causal chain in B2-1; the mitigation (promote fuzz probes into the gate) is
   **B1-13**; the standing watch on the architecture itself is **B4-1**.
4. **`->` is ONE deprecated-syntax problem, two surfaces** — A2 §A (parser silently accepts at
   6 sites + lexer; corpus clean) and A10 F2 (the CLI's own `--help` examples and serve prose
   use `->`), plus the doc corpus that still teaches it (A2 §C, A4 A9). Merged → **B2-3**.
5. **Conflict resolved: `playground/web/examples.js` is NOT stale** — A9 §11 inferred
   staleness from a mid-audit diff; A10 F14 ran `gen_examples.py` and got byte-identical
   output [Verified beats Inferred; A9's observed diff was A10's own regen, restored after].
   Only the CI staleness-check idea survives → B4-15.
6. **Ghost E-codes triangulated** — A4 B6 described `E-STATIC-INIT-CONST` /
   `E-TYPE-IMPORT-BUILTIN/SHADOW` as "emitted but not explainable"; A1 #12/#13 and A3 F5
   verified they exist **only as comments/doc-strings** (no raise sites). The deeper grep
   wins: they are comment-only ghosts, not emission gaps → B1-17.

---

## BUCKET 1 — No design decision needed: just do it

| # | Sev | Finding | Location | Sources | Grade |
|---|-----|---------|----------|---------|-------|
| B1-1 | **P1** | **Test-gate serialization** (merge #1): replace per-call 256 MiB `on_deep_stack` thread spawn with a persistent deep-stack worker, and shard the two monolithic corpus tests (`fmt::every_repo_phg_formats_idempotently_and_safely` 200.5 s; `runtime::shipped_manual_example_runs_on_both_backends` 154.7 s) into per-directory `#[test]`s. Expected: 111 s commit gate → ~50–60 s | `src/cli/mod.rs:300-308`; `src/fmt/tests.rs:26-27` | A6 F5/F6/F10, A9 §3/§4 | Verified (measured: microbench + nextest timings); win estimate Inferred |
| B1-2 | **P1** | **mold linker installed but not wired** — `/bin/mold` exists, no `.cargo/config.toml` anywhere; 2-minute gitignored config, saves 30–70% of link time on every rebuild + 23 test-binary relinks | repo root (file absent) | A9 §2 | Verified (absence); win Inferred |
| B1-3 | **P1** | **Fix broken `--help` examples**: `run`/`runvm`/`disassemble` examples fail verbatim (missing `package Main; import Core.Output;`) — the first thing the help teaches produces two errors; convert their `-> void` to canonical `: void` at the same time (canonical form already ratified; syntax half cross-refs B2-3) | `src/cli/mod.rs:77,85,129` (+ `:52,201,638`, `src/main.rs:301` arrow prose) | A10 F1/F2 | Verified (ran verbatim) |
| B1-4 | **P1** | Add `lsp` + `debug` to the long `--help` (they're in the terse usage line — the two help surfaces disagree) and give each a per-command help | `src/main.rs:73-75,244,256` | A10 F3, A4 B1/B2 | Verified |
| B1-5 | P2 | Add **6 golden diagnostic corpus cases** for verified-working siblings: `E-METHOD-VISIBILITY`, `E-CTOR-VISIBILITY`, `E-INJECTED-TYPE-BARE`, `E-DUP-FIELD`, `E-NEW-ON-NONCONSTRUCT`, one `protected` variant (all render correctly today; `PHORJ_BLESS=1` makes it cheap). Add a **reverse ratchet** test (every explained code has ≥1 emission site) | `conformance/diagnostics/`; `tests/diagnostics.rs` | A3 F2/F3/F4/F7, rec 2–3 | Verified (live probes p1–p11) |
| B1-6 | P2 | Add the mirrored **static-FIELD-via-instance** diagnostic: `a.s` on a static field yields a generic code-less "type `A` has no field `s`" while the method sibling got dedicated `E-STATIC-VIA-INSTANCE` + hint in W0-3 — mirror the shipped pattern + corpus case (message wording follows the already-ruled method precedent) | `src/checker/calls.rs:1709` | A3 F1 | Verified (probe p5) |
| B1-7 | P2 | **17 emitted E-codes with zero test coverage** — add one triggering test each, at minimum the non-quarantined ones (hooks family ×4, E-UFCS-AMBIGUOUS, E-VARIANT-QUALIFIER, E-PARENT-AMBIGUOUS, E-DECIMAL-LITERAL, E-OVERLOAD-FN-VALUE, E-NEW-ON-NONCONSTRUCT) | table in A3 F6 | A3 F6 | Verified (scripted cross-reference) |
| B1-8 | P2 | Format `selftest/{arithmetic,faults}.phg` (missed by the Phase-1 reformat) AND strengthen the fmt corpus test to assert `fmt(src) == src` (it currently checks only idempotency, so tracked files can drift forever) | `selftest/*.phg`; `tests/fmt.rs:112-167` | A2 §D, A10 F9 | Verified (ran `--check`) |
| B1-9 | P2 | Attach `[E-…]` codes to the most common diagnostics (arg-type, arity, `expected int found string`) — `check --json` currently emits `"code":null` for them, and fix the unknown-MEMBER misreport (`String.lenght` → "unknown identifier 'String'" points at the wrong thing) | checker arg/arity paths; `E-UNKNOWN-IDENT` site | A10 F4, A5 §5.3/§5.5 | Verified (probes) |
| B1-10 | P2 | `phg benchmark --vs-php` hardening: run php with `-n` (or detect+warn on Xdebug/DEBUG builds) — on this box the PHP leg silently aborts past 512-deep recursion and the printed comparisons fold in a debug+Xdebug interpreter | `src/cli/bench.rs` | A6 F9 | Verified (observed abort) |
| B1-11 | P2 | CI fixes: (a) cargo-zigbuild installed from source before the cache step every run — use `taiki-e/install-action`/binstall; (b) gate job uses `cargo test` not nextest; (c) `oracle-nightly` runs on every push (no cron) — ~25% of per-push runner minutes; (d) playground.yml has no rust-cache | `.github/workflows/ci.yml:66,139-159`; `playground.yml` | A9 §10 | Verified (read workflows) |
| B1-12 | P2 | Install `wasm-pack` locally — playground Rust-side changes are currently untestable end-to-end locally | dev machine | A9 §9 | Verified (absent) |
| B1-13 | P2 | **Promote A5's native-vs-PHP fuzz probes into the standing differential gate** (adversarial inputs: non-ASCII whitespace, multibyte strings, empty separators) — the cheapest attack on the byte-identity leak class (companion to B2-1; each B2-1 fix must land its case same-commit) | `tests/differential.rs` | A8 F2 rec, A5 §7 | Verified probes exist; promotion Inferred-value |
| B1-14 | P2 | **Disclosures to ADD** (known-but-undisclosed): no-sandbox/full-user-authority trust model paragraph (FEATURES); `Math.pow(0.0, neg)` PHP-deprecation note; `List.append` O(n²) + the `List.fill`+index-set fast path (guidance currently exists only as a Rust doc comment); `String.length` byte semantics before W4-4 reopens it; widen W5-13's scope (span reset also mis-anchors CHECKER diagnostics in interpolation); `PHORJ_SKIP_PHP`+`PHORJ_REQUIRE_PHP` both-set precedence line | FEATURES.md / KNOWN_ISSUES.md | A7 L-3, A5 §1.4/§5.4/§8, A6 F3, A8 F6 | Verified (each gap grep/probe-confirmed) |
| B1-15 | P3 | `php_escape` used in single-quoted PHP context (latent injection landmine, no current path — identifiers are quote-free): rename → `php_escape_dq` / add `php_escape_sq` and switch the two call sites | `src/transpile/program.rs:1380,1392`; `transpile/mod.rs:835-839` | A7 L-4 | Verified (escaper mismatch); unexploitability Inferred |
| B1-16 | P3 | Use `subtle::ConstantTimeEq` for `Hash.equals` (already in the dep tree via argon2; guarantees constant time across compiler versions) + install/CI `cargo-audit` so the vetted-4 pins get RustSec-checked | `src/native/hash.rs:468-477`; CI | A7 L-1, P13 caveat | Verified |
| B1-17 | P3 | **Ghost/stale diagnostic-code cleanup** (merge #6): drop never-raised explain entries `E-OVERLOAD-SELECT-CONFLICT` + `E-PKG-TYPE`; fix comment-only ghosts `E-TYPE-IMPORT-BUILTIN/SHADOW` (`loader/mod.rs:617`, `resolve.rs:590`), the `E-STATIC-INIT-CONST` claim (`value.rs:596`), and the self-contradicting `casing.rs:398` comment | `src/cli/explain.rs:645`; as listed | A3 F5, A1 #11–13, A4 B6/B7 | Verified (greps) |
| B1-18 | P3 | Correct the `Suspend` trait doc comment (claims a wasm frame-swap implementor that doesn't exist; the dep-isolation justification is the true one) | `src/green/exec.rs:40` | A8 F3 | Verified |
| B1-19 | P3 | Delete stray `target/` scratch dirs (`s2c_php_check/`, `s2d_php_check/`, empty `tmp/`) — they also pollute `phg format --check .` at repo root (2 false positives) | `target/` | A9 §7, A10 F8 | Verified |

## BUCKET 2 — Needs a design/direction decision from the developer

*Each item lists concrete options with a recommendation — this bucket feeds the AskUserQuestion
batch. Per the ADJUDICATION RULE, nothing here is ruled; recommendations are inputs.*

### B2-1 [**P0**] Byte-identity violation cluster — 4 cases, each needs a "which side wins" ruling
**Sources**: A5 §1.1–1.3 (Verified-run: both legs executed, divergent output), A7 M-1 (Verified:
source read) · **Cross-tag: security (pbkdf2)** · **Root cause (merge #3)**: A8 F2 — four
independently-maintained execution paths; these are the drift tax materializing despite the
174-program differential corpus. None is disclosed anywhere. Every fix lands its adversarial
differential case same-commit (B1-13).

| Case | Divergence | Options | Recommendation |
|------|-----------|---------|----------------|
| **a. `String.trim`/`trimStart`/`trimEnd`** (`src/native/text.rs:25,405,414,459`) | Rust strips all Unicode whitespace; PHP `trim()` strips only `" \t\n\r\0\x0B"` — probe `"\u{00A0}x"` → 5 1 vs 5 5 | (1) restrict Rust to the PHP charset (precedent: `Core.Ini` hand-rolls exactly this set); (2) emit a PHP helper matching Unicode | **Option 1** — PHP-familiarity philosophy + existing Ini precedent |
| **b. `String.reverse`** (`text.rs:40,477`) | Rust char-wise (`lëon`); PHP `strrev` byte-wise (mangles UTF-8) | (1) char-wise both via emitted mb-safe PHP helper; (2) byte-wise both (PHP-faithful but mangles multibyte natively) | **Option 1** — "removes surprises, never capability"; byte-mangling is the surprise |
| **c. `String.split`/`splitOnce` empty separator** (`text.rs:535`) | Rust total (`["","a","b",""]`, exit 0); PHP `ValueError` fatal (exit 255) | (1) fault on empty separator on BOTH legs (precedent: `String.count` already faults on empty needle); (2) emit a PHP polyfill making it total | **Option 1** — matches the module's own precedent |
| **d. `Hash.pbkdf2` iteration truncation** (`src/native/hash.rs:516-518`) — **security** | `i64 as u32` wraps: 2^32 iterations → effectively 1 round natively, while the PHP emission gets the untruncated i64 → different derived keys | (1) guard `iters <= u32::MAX`, fault above (one line + test + differential case); (2) widen to u64 internally | **Option 1** — smallest fix, no legitimate use case above u32::MAX |

### B2-2 [P1] Injected-prelude architecture — decide BEFORE the W3/W4 waves
**Source**: A8 F1 [Inferred on verified artifacts] · The ~5 user-facing special-case rules
(`E-INJECTED-TYPE-BARE`, leaf qualification, prelude exemption, collapse pass, hand-synced
registry) exist ONLY because stdlib types are injected as AST preludes (`inject_*_prelude` ×6,
`src/cli/mod.rs`; `src/checker/{collapse_injected,enforce_injected}.rs`) instead of resolving
through the module loader. Every new multi-type Core module (W3-1 DB, W3-2 HTTP…) extends the
registry and the rules.
- **Options**: (1) unify stdlib type resolution with the loader (virtual Core modules) now,
  before W3-1/W3-2 land — the injected-type discipline collapses into ordinary import rules;
  (2) keep the mechanism, keep extending the registry per wave; (3) defer the call until after
  W3, accepting registry growth.
- **Recommendation**: **Option 1, timed before W3-1/W3-2** — the S1/S2 surface is preserved
  (same syntax, one resolution path); the cost is a one-time loader refactor [Speculative on
  effort]; the alternative compounds with every wave.

### B2-3 [P1] `->` retirement: parser-reject timing + sequencing (merge #4 — ONE problem, two surfaces)
**Sources**: A2 §A (Verified empirically: `->` in all three positions checks + runs silently at
HEAD; 6 accept-sites `src/parser/types.rs:109`, `items.rs:240/296/370/735`, `exprs.rs:546` +
lexer `src/lexer/mod.rs:1125`; corpus 100% clean → accepted-but-unused), A10 F2 (Verified: the
CLI's own `--help` examples and serve prose teach `->`), A2 §C + A4 A9 (docs still teach it:
5 runnable code-block lines in dump/lift READMEs, 2 wrong `declare` signatures in interop
README, 41 pseudo-sig arrows in examples/README.md, KNOWN_ISSUES arrow prose ×10). Removal is
blocked by ~1700 embedded arrows in Rust-string test programs (tracked P1-remainder;
bulk-sed corrupts function-type arrows — per-site fixes required).
- **Options**: (1) docs/help pass FIRST (safe, no regex traps — B1-3 + Bucket 3 items), then
  flip the parser-reject and fix the ~200 gate-surfaced embedded sites individually (the
  plan-documented correct approach); (2) flip the reject now and do docs after; (3) interim
  deprecation warning at the 6 sites before the hard reject.
- **Recommendation**: **Option 1** — matches `wave0-remainder.plan.md` ("P4+P5 docs = safe
  first step; P1-remainder flip = the risky one") and guarantees the help/docs never teach a
  syntax the parser rejects.

### B2-4 [P1] Empty-Map literal hole — no way to write an empty Map at all
**Source**: A5 §3.8 [Verified-run: both probes] · `Map<string,int> e = [];` → "cannot infer
element type" even with the declared type present; `[=>]` → parse error. Only workaround is
`Map.remove(["k"=>v], "k")`.
- **Options**: (1) extend the expected-type rule that already admits empty `[]` for List to
  Map positions; (2) add a `[=>]` empty-map literal; (3) add `Map.of()`/`Map.empty()`
  constructors (pairs with the missing `Map.entries`/`fromList`, B2-11).
- **Recommendation**: **Option 1** (+ optionally 3) — zero new syntax, uses machinery that
  already exists for List.

### B2-5 [P1] `Core.Hash` output-type mismatch: `hmac` → hex string, `hkdf`/`pbkdf2` → bytes
**Source**: A5 §2.11 [Verified-read: hash.rs:539-597] · `Hash.equals(bytes,bytes)` cannot take
`hmac`'s own output; composing MAC into KDF needs `Encoding.hexDecode`.
- **Options**: (1) add `hmacBytes` (keep `hmac` hex for PHP `hash_hmac` familiarity); (2)
  change `hmac` to return bytes + let users `Encoding.hexEncode` (breaking); (3) document the
  asymmetry.
- **Recommendation**: **Option 1** — non-breaking, PHP-familiar default preserved, closes the
  composition trap.

### B2-6 [P1] `Math.clamp(min > max)` returns silent nonsense
**Source**: A5 §4 [Verified-run: returns `10` (the min), no fault — every neighbouring
precondition violation faults] ·
- **Options**: (1) fault on min > max (consistent with `chunk`/`fill`/`repeat`; Rust's own
  `clamp` panics here); (2) document the total behaviour.
- **Recommendation**: **Option 1** — consistency with the module's own validation philosophy.

### B2-7 [P1] ReDoS asymmetry on the PHP leg: disclosure vs mitigation
**Source**: A7 M-2 [Inferred: both invocation paths read; PCRE backtracking is standard
documented behavior] · Natively-linear patterns (`(a+)+$`) catastrophically backtrack under
transpiled `preg_*` — a program safe under `phg run` is DoS-able once deployed as PHP with
attacker-controlled subjects. The bridge is exactly the deployment path.
- **Options**: (1) KNOWN_ISSUES/LADDER-style disclosure paragraph now (cheap); (2) disclosure
  + a transpile-time pattern-complexity lint later; (3) full mitigation (reject risky patterns
  at transpile).
- **Recommendation**: **Option 1 now, 2 as a tracked follow-up** — proportionate to a
  bridge-only surface, honest per the LADDER disclosure discipline.

### B2-8 [P2] Naming-unification batch: execute the approved trio + adjudicate A5's additions
**Sources**: A5 §2 [Verified-read, probes for slice semantics], A8 F7 · Approved-but-unshipped:
`Bytes.find→indexOf`, `Map.has→containsKey`, slice unification. A5 adds to the same
discussion: `List.slice`(offset,LENGTH,neg-ok) vs `String.substring`(start,LENGTH,neg-ok) vs
`Bytes.slice`(start,END-exclusive,clamp) — three conventions, two names; `length` vs `size`
(List/String/Bytes vs Map/Set); `List.count`(predicate) vs `String.count`(needle);
`Core.Conversion`'s three naming families (`xToY` / bare `toX` / `asX` type-assertions) +
`Conversion.round` duplicating `Math.round`; `intBetween` vs `secureInt` ranged-pair naming;
`nowMilliseconds` vs `monotonicNanos` unit suffixes. Crypto 3-module split: **keep** (A8 F7 —
mirrors PHP's own API families) + one cross-referencing docs paragraph.
- **Options**: (1) execute only the approved trio now, adjudicate the additions as a batch
  question; (2) fold everything into one rename wave; (3) trio + defer the rest.
- **Recommendation**: **Option 1** — the trio is ruled; the additions are new user-visible
  surface decisions that need per-item AskUserQuestion treatment (§15).

### B2-9 [P2] Fault-message canonicalization (stale prefixes + 4 competing formats)
**Source**: A5 §5.1/5.2 [Verified-run + read] · User-visible faults still say `Text.repeat…` /
`Convert.…` / `Validate.…` (pre-rename module names) and `Bytes.from_string` (snake_case that
never existed); four format shapes coexist (`Mod.fn: msg`, `Mod.fn msg`, `msg in Mod.fn`,
bare `msg`). These are parity-affecting fault strings (Invariant 4) — ~40 strings, one sweep,
but the canonical format is a user-visible decision.
- **Options**: (1) canonicalize on `Module.function: message` and sweep; (2) module-less terse
  faults everywhere; (3) fix only the stale module names, leave formats mixed.
- **Recommendation**: **Option 1** — before more natives land; differential gate required.

### B2-10 [P2] VM string performance direction — VM is 1.53× SLOWER than the interpreter on string concat
**Source**: A6 F1/F7 [Verified: measured, `phg benchmark` median-of-101] · `Value::Str(String)`
is the only non-Rc compound variant; `Op::GetLocal` deep-clones on every read
(`src/value.rs:124`, `src/vm/exec.rs:149-153`). Inverts the documented "VM is the fast
backend" contract on a whole workload class.
- **Options**: (1) `Rc<String>`/`Rc<str>` + COW `make_mut` path for mutating natives; (2)
  targeted fix (stop cloning on the concat operand path only); (3) accept + document the
  inversion.
- **Recommendation**: **Option 1** — parity-affecting surface: full differential gate +
  before/after `phg benchmark` per Invariant 11.

### B2-11 [P3] P3-additive stdlib batch: confirm scope extension
**Source**: A5 §3 [Verified-read: all absent from the 236-native registry] · Approved+unshipped:
`String.startsWith/endsWithIgnoreCase`, `replaceFirst`, `Set.isSuperset/symmetricDifference/isDisjoint/map/filter`,
`Math` Float variants. A5's NEW candidates for the same batch: `Bytes.isEmpty` (only sequence
type without it), `Map.entries`+`Map.of`/`fromList` (Map has no constructor or list
round-trip), document-level `Csv.parse` (currently single-row only — undocumented scope
surprise), `Math.ceil/floor→float` vs `round→int` return-type note.
- **Recommendation**: present the new candidates as one batch AskUserQuestion; ship approved
  items regardless.

### B2-12 [P3] Import-redesign guide example — pending dev question
**Source**: A2 §F [Inferred: no example/README row found] · S0–S2 shipped a language-discipline
change (qualified `Http.Router`, `#[Http.Route]`, `E-INJECTED-TYPE-BARE`) with no dedicated
guide example — whether a discipline change "ships an example" (Invariant 9) is flagged, not
ruled. **Recommendation**: yes, one small guide example + README row (cheap, closes the gap).

### B2-13 [P3] `playground/src/lib.rs` lacks `#![forbid(unsafe_code)]`
**Source**: A1 §3/#15 [Verified: grep] · Letter-of-invariant compliant (INVARIANTS #10 names
the phorj crate roots) but the guarantee doesn't extend to the workspace member by attribute;
wasm-bindgen compatibility under `forbid` is [Speculative]. **Options**: (1) add it and see if
it compiles; (2) rule the exemption + record it in INVARIANTS. **Recommendation**: try (1);
fall back to (2) with the reason recorded.

## BUCKET 3 — Doc-only fixes (no code change; resolved by Stage D consolidation)

| # | Sev | Finding | Sites | Sources | Grade |
|---|-----|---------|-------|---------|-------|
| B3-1 | **P0** | **"Zero external dependencies / std-only" is FALSE** — 4 default deps exist (argon2, regex, ctrlc, corosensei); README:311 even links THIRD-PARTY-NOTICES while denying deps. Include the stale `Cargo.toml:83-85` "only external crate" comment and the C-3 framing doc ("NO regex/TLS, LOCKED" — now doubly false with rusqlite+rustls approved) | README.md:5-6,89,311-312; FEATURES.md:84; VISION.md:59; CONTRIBUTING.md:15; Cargo.toml:83-85; native-modules framing doc | A4 A1, A1 #14, A8 F9 | Verified |
| B3-2 | **P0** | **`import type` taught as CURRENT syntax while it hard-fails to parse** — incl. INVARIANTS (a read-before-backend-work doc) and STABILITY listing it *stable*; + 3 stale `.phg` comments + ~14 stale src doc-comments | FEATURES.md:46; STABILITY.md:20; docs/INVARIANTS.md:120,133; KNOWN_ISSUES.md:211,254,333-335,615; docs/MILESTONES.md:74; examples/README.md:154; examples/project/README.md:88; Shape.phg:3, visibility/main.phg:5, Animal.phg:4; src sites per A4 A2 | A4 A2, A2 §B | Verified (empirical parse rejection) |
| B3-3 | **P0** | **Dead CLI verbs (`fmt`/`lex`/`disasm`/`bench`) taught as real — STABILITY declares them *stable*; all 3 editor READMEs instruct `phg fmt`** (the most user-facing pages shipped); README self-contradicts (`fmt` at :136, `format` at :203-209) | README.md:128-136; FEATURES.md:67; STABILITY.md:69; GA-CHECKLIST.md:16,33,42,44; ROADMAP.md:111,147; CONTRIBUTING.md:66; ARCHITECTURE.md:54; MILESTONES.md:19,109,112,126; examples/README.md:136; editors/{vscode,phpstorm,.}/README.md; workload.phg + playground/web/examples.js:10 | A4 A4, A10 F5/F13 | Verified (ran each verb) |
| B3-4 | **P0** | **`E-TRANSPILE-CONCURRENCY` does not exist** (actual code: `E-CONCURRENCY-NO-PHP`); `E-RETIRED-SYNTAX` must be labeled *planned*, not existing | README.md:277-279; MASTER-PLAN.md:84,1160,1271 (correct name at KNOWN_ISSUES.md:757) | A4 A5 | Verified (grep both codes) |
| B3-5 | **P0** | **FEATURES 🔲/🚧 on SHIPPED features + GA % from a false premise**: traits 🔲 (construct type-checks clean), concurrency 🔲, lift 🔲, "Editor/LSP, formatter 🔲" (six lines after marking formatter ✅ in the same file), Set-algebra 🚧 (shipped), M5 🚧 vs MILESTONES ✅; GA-CHECKLIST:16 "Missing: an LSP" is false → the ≈57% GA figure is computed from a stale premise | FEATURES.md:38,53,54,55,67,74-80; docs/GA-CHECKLIST.md:16 | A4 A6/A7, A10 F6 | Verified (binary probes + grep) |
| B3-6 | P1 | **16 doc-vs-doc contradiction pairs** (traits, concurrency, M5, LSP, formatter, lift, serve, HTML tier, GC wording, zero-deps, `fn`, fn-type arrow, fmt/format, M7/M8 numbering collision, missing decimal row, E-TRANSPILE name) — resolve once at merge | table in A4 §C | A4 §C | Verified per-row (see table) |
| B3-7 | P1 | **Percentage model**: ≈58%/≈60% is a *stale lower bound* (S0–S2, W3-4 crypto, NDJSON, INI shipped since the 07-02 compute) — row-level FN re-score needed at merge; the GA-CHECKLIST ≈57% is a different model with a broken rock-2 input (B3-5) | MASTER-PLAN.md:60-63,1119-1121; M-gap-matrix §4 (model itself Verified-sound) | A4 §D | Verified (model arithmetic) / Inferred (staleness direction) |
| B3-8 | P1 | **Undocumented shipped features**: W3-4 crypto stdlib has no FEATURES row; S1/S2 import discipline has no user-facing doc (spec only); decimal missing from the FEATURES language table; `ctrlc`/`corosensei` never named even where deps are admitted (STABILITY:64 names only 2 of 4); playground-member deps (serde_json, wasm-bindgen) absent from the dependency-policy spec | FEATURES.md; STABILITY.md:64; docs/specs/2026-06-27-dependency-policy.md | A4 B3/B4/B5, C15; A1 §4 | Verified |
| B3-9 | P2 | **5 `examples/project/` dirs documented NOWHERE** (funcvalues, genericbox, jsonmulti, mixins, inherit — runnable, differential-gated, zero index entry = Invariant-9 gap) + `web/json-api.phg` missing from the main index | examples/README.md; examples/project/README.md | A2 §E2/E3 | Verified (bidirectional inventory) |
| B3-10 | P2 | Count/status staleness in plans: MASTER-PLAN line-counts (KNOWN_ISSUES 1125→1133, differential.rs 2966→3308, explain "270"→200, "~22 modules"→26); W0R "S1 uncommitted" (committed `cd29f3c`); W3-4 shipped but unmarked in MP; W2-4 stale vs Phase-1 progress | MASTER-PLAN.md:232,264,319,426-429,633-646; wave0-remainder.plan.md:112,145; GA-CHECKLIST.md:18 | A4 A10/A11/A12 | Verified |
| B3-11 | P2 | KNOWN_ISSUES stale content: `\Main\Obj` (renamed `Object`); L338-341 "not yet implemented" list contradicted by the same file (exceptions/traits/accessors/match shipped); `->` prose as canonical presentation ×10 (fold into B2-3's doc pass) | KNOWN_ISSUES.md:150,338-341,678 + arrow lines | A4 A9 | Verified/Inferred per item |
| B3-12 | P2 | **Archive the superseded research corpus** — ≈1.07 MB (~⅔ of the docs tree) of write-once raw dirs already superseded by their syntheses (MASTER-PLAN is SSOT); treat `roadmap-completeness/raw/` (20 files) as historical only — its claims ("no LSP, no formatter, no decimal…") are all refuted at HEAD; prune absorbed specs to pointers per the P4/P5 directive | docs/research/full-audit/raw/, roadmap-completeness/raw/, wave3-4-drafts/ | A8 F5, A4 F3 | Verified (sizes) / Speculative (process opinion) |
| B3-13 | P3 | ARCHITECTURE.md:88 embedded verification command now false ("grep 'trait ' = 0" — 3 non-test traits exist; the substantive no-Backend-trait claim still true); statics research-spec header stale (Area A shipped) | docs/ARCHITECTURE.md:88; docs/specs/2026-06-28-statics-research-design.md | A4 A8, A3 §4 | Verified |
| B3-14 | P3 | Undocumented flags to add to help: `--dump-on-fault` (run/runvm), `benchmark --json`, `build --dev`/`--sign`(stub); `phg --help` prose prints `->` twice (folds into B2-3) | src/main.rs:137-420 vs help corpus | A10 F10, A4 §E | Verified |

## BUCKET 4 — Watch / defer (worth knowing, not urgent)

| # | Sev | Finding | Sources | Grade |
|---|-----|---------|---------|-------|
| B4-1 | P2 | **Four-backend maintenance tax** (root cause of B2-1, merge #3): ~20.7K LOC of path code + ~28% of src serving multi-backend coherence; verdict = right call for THIS project (byte-identity IS the product), but the cost grows superlinearly with W3/W4 (3–4 implementations per native). Watch: re-weigh the VM's perf justification at GA; the cheap mitigation is B1-13, not fewer backends | A8 F2 | Verified (costs) / Inferred (verdict) |
| B4-2 | P2 | DAP transport write errors silently swallowed (`let _ = write!`) + malformed `Content-Length` handled non-loudly in both framers (LSP desyncs the stream) | A1 #1/#2 (`src/dap.rs:50-51,90,93`; `src/lsp/mod.rs:663-669`) | Verified |
| B4-3 | P2 | Zero direct tests for two pure-logic modules: `src/dispatch.rs` (overload selection), `src/json.rs` (DAP/LSP framing) | A1 #6 | Verified |
| B4-4 | P2 | File-size rule adopted but unenforced: 12 files > 1000 production lines, `scripts/size-gate.sh` not built, no exemption register — tracked as W1-6, not silent drift | A1 §5/#7 | Verified |
| B4-5 | P2 | **W2-7 (PSR-4 import roots) was designed BEFORE the unified-import model** — must be re-based/re-adjudicated before build or it becomes import redesign #5 | A8 F1-watch | Verified (spec header) |
| B4-6 | P2 | `src/checker/` at 21K lines is the codebase's gravitational center (2.2× the execution spine); three near-identical type-walkers already noted in the S1 spec — take the consolidation when the NEXT type-walking pass is added, not before | A8 F8 | Inferred |
| B4-7 | P3 | VM per-dispatch String clones (`CallMethod` ×2, `MakeEnum`, `MatchTag`, `MakeInstance`, String-pair dispatch keys; `GetField`'s inline-cache technique would apply) — needs measured before/after per Invariant 11; VM still 17× ahead where it matters | A6 F2, A1 #3-5, #8-9 | Verified (reads) |
| B4-8 | P3 | Monolithic 84 kLoC crate → 50.3 s release rebuilds; crate split is a Large refactor — flag only. Incremental dev builds measured healthy (4.5–7.6 s), so not a current pain | A6 F8, A9 §1 | Verified (measured) / Speculative (split) |
| B4-9 | P3 | `List.append` refcount-1 fast path (signature change or owned-arg path needed); the user-guidance half is B1-14 | A6 F3 | Verified (measured O(n²)) |
| B4-10 | P3 | DX feature requests: debugger REPL `print <var>`/eval (locals-only today); did-you-mean suggestions (member typos, dead CLI verbs, unknown flags — bad invocations never echo the offending token); missing-file error leaks `(os error 2)`; `phg format` exclude mechanism (beyond B1-19's stray-dir deletion); `phg fmt` alias for muscle memory | A10 F7/F11/F12/F13/F8, A9 §8 | Verified |
| B4-11 | P3 | Dev-loop nice-to-haves: green-tree stamp to skip redundant pre-push after a green commit; `debug = "line-tables-only"` (target/debug is 22 GB); periodic target sweep | A9 §5/§7 | Speculative |
| B4-12 | P3 | Hand-rolled SHA-256 now underpins keyed crypto (HMAC/HKDF/PBKDF2) — tension with the charter's "checksums, not a MAC facility" + the never-roll-your-own clause that admitted argon2; mitigated by RFC KATs + PHP-oracle byte-identity + no data-dependent branches. Revisit if/when RustCrypto enters the tree anyway (e.g. with rustls) | A7 L-2 | Verified (reads) |
| B4-13 | P3 | Unbounded self-DoS allocations (`secureBytes(n)` exabytes, regex pattern cache no eviction) — self-inflicted class, no resource-quota model by design; listed for completeness | A7 L-5 | Verified |
| B4-14 | P3 | Two production `Result::expect` on scheduler invariants (`interpreter/coop.rs:103`, `green/exec.rs:131`) — internal-invariant panics, not user-input-reachable | A1 #10 | Verified |
| B4-15 | P3 | `examples.js` CI staleness check (`gen_examples.py && git diff --exit-code`) — the artifact is currently NOT stale (merge #5: A10's byte-identical regen refutes A9's inference), but nothing local gates future drift | A9 §11, A10 F14 | Verified (current state) / Speculative (check) |

## Positive attestations worth preserving (do not re-litigate)

- `src/` structurally clean: 0 stubs/TODOs/dead-code allows; Op coupling 73/73/73 wildcard-free;
  `forbid(unsafe_code)` on both crate roots; value kernels single-sourced; deps exactly per
  policy; all prior P0/P1s touching src verified FIXED (A1 §8 — Verified).
- All 9 golden diagnostic cases byte-identical, zero drift (A3 §1); `phg explain` covers 100%
  of greppable emitted codes (A10 F3b); zero Rust panics across every error probe (A10).
- Security posture deliberately strong: 13 positive attestations incl. Argon2id/OWASP defaults,
  fail-loud CSPRNG with rejection sampling, no shell in any exec path, hardened `phg vendor`,
  injection-safe PHP emission, bounded serve (A7 — Verified). 0 High findings.
- No premature abstraction: 4 traits in 75K LOC, each earning its keep; Op enum proportionate;
  env-flag inventory clean; import churn was convergent, not oscillation (A8 F3/F4/F6/F9).
- Corpus: 100% clean of syntactic `->` and `import type` statements; naming rules fully clean;
  formatter idempotent + convergent; single-test loop 0.26 s (A2 §A/G, A10 §4, A9 §1).

## Certification note

Stage-A auditors disclosed self-graded three-lens checks where `advisor()` was unavailable
(A1, A2, A10 explicitly). This synthesis performed no new auditing: severities and grades are
carried from the source reports; only bucket placement, merging, and ordering are Stage-B
judgment. Conflicts between reports were resolved in favor of the higher evidence grade
(documented in the merge section).
