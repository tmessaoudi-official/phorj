# A8 — Over-engineering, bad decisions, YAGNI audit (2026-07-03)

> Auditor dimension: bad architectural decisions, over-engineering, unnecessary complexity,
> YAGNI violations, abstractions that don't earn their cost. Judged against the project's own
> philosophy lens: *pragmatic, legible, provably-correct upgrade of PHP; removes surprises,
> never capability* — complexity that SERVES that philosophy is not over-engineering.
> State audited: HEAD `0691228`, clean tree, 832 commits total [Verified: `git rev-list --count HEAD`].

## Headline verdict

The codebase itself is strikingly UNDER-abstracted for its size (4 traits in ~75K LOC of Rust,
all justified), and the single biggest structural cost is not any module — it is the **four
execution/conversion paths** (interpreter + VM + transpiler + lifter), each of which must
independently implement every language feature. That cost is real, measured, and *deliberately
priced in* by the invariant machinery; my verdict is that it is defensible **now** but is the
one decision whose cost grows superlinearly with the remaining roadmap (W3/W4 stdlib waves each
land 3–4 implementations per native). The documentation *process* is heavier than the code
process and is where the actual dead weight sits (≈1MB of write-once research vs 5.5K lines of
living docs).

Finding count: **9** (F1–F9). 2 significant costs, 3 watch items, 4 explicit "not
over-engineered" verdicts (asked-for calls, made honestly).

---

## F1 — Import/namespace system: 4 design passes in ~2 weeks; the churn was convergent, but the *injection prelude* mechanism is the incidental complexity a fresh design wouldn't have

**Redesign count** [Verified: specs + decision register + git log]:

1. **M5 original import/package model** (~06-18/20) — incl. `import type` for cross-package
   types (DEC-036, 06-20).
2. **Import roots / PSR-4 `[packages]`** (`docs/specs/2026-07-01-import-roots-psr4-design.md`)
   — status "designed, not implemented"; still live on MASTER-PLAN as W2-7 (line 473).
3. **"Nothing in the wind" namespace & language-surface**
   (`docs/specs/2026-07-01-no-wind-namespace-and-language-surface-design.md`) — same day,
   governing-principle lock.
4. **Unified import + injected-type discipline**
   (`docs/specs/2026-07-03-unified-import-and-injected-type-discipline.md`) — REMOVES
   `import type` entirely. `import type`'s full lifecycle: added ~06-20, deleted 07-03 —
   **a 13-day add-remove round trip** [Verified: DEC-036 in C-decisions + spec §1 + commit
   `11a6c71` + `bc523c1` removing `type_only`].

Roughly 10 import/namespace-titled commits [Verified: `git log --pretty=%s | grep -icE
"namespace|import"` = 10].

**Is the current design's rule-count worse than PHP's `use`?** Honest comparison:

- PHP: `use X`, `use function f`, `use const C`, `as` aliasing, group-use — 4-5 forms.
- Phorj end state: ONE `import` keyword (resolver classifies module-vs-type by path), `as`
  aliasing, no function import (deliberate omission). That is **simpler than PHP** at the core.
- BUT the injected-Core-type layer adds special-case rules PHP has no analog for: (a)
  single-type modules (`Json`/`Regex`/`Secret`) are bare-compliant as-is; (b) multi-type
  modules (`Http`/`Time`/`Decimal`) need leaf qualification (`Http.Router`) or a member-import;
  (c) `E-INJECTED-TYPE-BARE` + `E-INJECTED-VARIANT-BARE` enforcement; (d) preludes' own
  internal references are exempt; (e) the qualifier is Phorj-surface-only (transpiler erases
  it). Five special-case rules a user (and every future maintainer) must learn.

**The root cause is architectural, not design churn**: these rules exist ONLY because stdlib
types are *injected as AST preludes* (`inject_*_prelude` in `src/cli/mod.rs`, six of them)
rather than resolved through the normal module loader like any user package. The "collapse
pass" (`src/checker/collapse_injected.rs`), the static registry
(`checker::enforce_injected::module_of`), the two error codes, and the exemption rule are all
scaffolding compensating for that split resolution path. A fresh design would make `Core.Http`
a real (virtual) module resolved by the loader, and the entire injected-type discipline
collapses into the ordinary import rules — zero special cases.

- **Cost**: ~5 user-facing special-case rules; 1 extra compiler pass; a registry that must stay
  in sync with six preludes by hand; every new multi-type Core module (the W3/W4 waves will add
  several) extends the registry + rules. [Inferred: from spec §2 + S1 recipe step 2.]
- **Simpler alternative**: unify stdlib type resolution with the module loader (virtual Core
  modules). What breaks: nothing semantically — it's the same surface with one resolution path;
  the cost is a one-time loader refactor. [Speculative on effort size.]
- **On the churn itself**: NOT oscillation. Each pass moved monotonically toward "one import +
  nothing in the wind" — pass 4 deleted complexity (pass 1's `import type`) rather than adding
  it. Churn cost was real (18 `import type` sites migrated, ~40 .phg migrated in S2, fmt/lift
  printers touched twice) but the end state is cleaner than the start. [Verified: spec S0
  migration scope.]
- **Watch item**: W2-7 (PSR-4 roots) was designed *before* the unified-import model and says
  "breaking change to the M5 import/package model — needs a migration codemod". When it is
  implemented it MUST be re-based on the 07-03 model or it becomes redesign #5. Flag it for
  re-adjudication before build. [Verified: spec header + MASTER-PLAN W2-7.]

**Grade: [Inferred]** — pattern-based judgment on verified artifacts.

## F2 — Four execution/conversion paths: the single biggest ongoing maintenance tax; defensible today, superlinear tomorrow. Genuine verdict, no hedge.

**Measured scope** [Verified: `wc -l` per module]:

| Path | LOC | Role |
|---|---|---|
| `src/interpreter/` | 3,299 | reference oracle |
| `src/vm/` + `src/compiler/` | 2,025 + 4,341 | perf backend |
| `src/transpile/` | 6,054 | PHP bridge |
| `src/lift/` | 4,951 | PHP→Phorj (inbound, separate grammar) |
| **Total path code** | **≈20.7K** | of ~75K src total |

Plus the machinery that exists *solely to hold the paths together*: `tests/differential.rs`
(3,308 lines; 174 `.phg` example programs globbed), delivery invariants 1/2/3/6/7/8, the
MUST-CHECK rules (CTy-operand, scratch-slot), `check_and_expand_reified` threading, and the
`PHORJ_REQUIRE_PHP` oracle gate. Conservatively **~28% of the source + the entire heaviest test
harness** serve multi-backend coherence.

**Where the tax shows up**: NOT in bug churn — only 6 `fix:`-prefixed commits in 832
[Verified: `git log --pretty=%s | grep -c "^fix"` = 6]; the differential gate catches
divergence pre-commit. The tax is paid at **development time**: every stdlib native and every
language feature is implemented 3× (interp + VM/compiler + transpile mapping), sometimes 4×
(lift). And it still leaks: sibling report A5 (this session) found **3 byte-identity P0s by
fuzzing** — `String.trim` Unicode-vs-ASCII whitespace set, `String.reverse` char-vs-byte,
`String.split("")` total-vs-fatal [Verified: A5-stdlib-consistency-fuzz.md §1.1–1.3] — all of
the shape "the native's Rust implementation diverges from the PHP leg's semantics", i.e.
exactly this tax materializing despite a 174-program differential corpus.

**Verdict**: Keeping all four is **the right call for THIS project** — because byte-identity
with real PHP is the product's core claim, not a nice-to-have. The interpreter (oracle) and
transpiler (the claim's other half) are non-negotiable. The two honest pressure points:

1. **The VM is the one path that is optional in principle.** Its justification is perf
   (invariant 11's benchmark discipline exists because of it). It adds the compiler (4.3K),
   the VM (2K), 73×3 exhaustive match arms, and two whole classes of MUST-CHECK bug (CTy
   operands, scratch slots). If perf-vs-interpreter numbers ever stop justifying it, it is the
   path to cut — but cutting a working, gate-protected backend now would be churn, not
   simplification. Keep, but treat every VM-only bug class as evidence to weigh at GA.
   [Speculative: design judgment.]
2. **The lifter is fine because it's quarantined**: it is inbound-only, best-effort by design
   (Tier-3 loud annotations, DEC-166), and does NOT participate in the byte-identity spine —
   so its incompleteness doesn't multiply. Its 5K lines are a bounded, separable cost.
   [Inferred: from DEC-166 + differential harness not gating lift.]

The A5 finding pattern suggests the cheap marginal investment is not fewer backends but
**making the PHP-semantics contract explicit per native** (the fuzz harness A5 used, promoted
into the gate) — that attacks the actual leak (native-vs-PHP drift) without an architecture
change. [Inferred.]

**Grade: [Verified] for the measured costs; [Inferred] for the verdict.**

## F3 — Premature abstraction: NOT FOUND. The codebase is trait-minimal and every trait earns its keep. (Explicit negative finding.)

Exactly **4 traits** in all of `src/` [Verified: grep `pub trait` across src, 4 hits]:

| Trait | Impls | Justification | Call |
|---|---|---|---|
| `Transport` (`src/serve.rs:59`) | 3 — real `TcpTransport` + `FixtureTransport`/`ScriptedTransport` in `tests/serve.rs` [Verified: grep] | deterministic port-free serve tests | **Earns it** |
| `DebugFrontend` (`src/debug.rs:62`) | 3 — REPL, DAP, scripted-test | 3 genuine frontends | **Earns it** |
| `Task` (`src/green/exec.rs:64`) | 3 — coroutine + 2 test mocks | scheduler seam | **Earns it** |
| `Suspend` (`src/green/exec.rs:40`) | **1** — `YielderSuspend` (`src/green/coro.rs:45`) | keeps `corosensei` types out of engine signatures (dep-policy quarantine) | **Borderline-fine** — see note |

Note on `Suspend`: its doc comment claims "the wasm build supplies a frame-swap implementor" —
**no such implementor exists anywhere in the repo** [Verified: grep across src/, tests/,
playground/ → 1 impl]. So the polymorphism justification is currently aspirational. The
*dependency-isolation* justification (engines holding `&dyn Suspend` compile without the
coroutine crate — directly serving the feature-gated dep policy) is real and sufficient on its
own, but the comment should be corrected to claim what is true. Cost of keeping: one vtable
indirection at suspension points — negligible. **Not over-engineering; fix the comment.**

Everything else in the codebase uses concrete types, per-pass free functions, and enums —
i.e. the *opposite* failure mode was avoided. **Grade: [Verified].**

## F4 — Op enum at 73 variants: proportionate, and the wildcard-free triple-match makes growth self-pricing. (Explicit negative finding.)

73 variants confirmed [Verified: `awk '/pub enum Op {/,/^}/' src/chunk.rs | grep -cE '^    [A-Z]'`
= 73]. Reference points: Lua ≈40 opcodes, CPython ≈120, JVM ≈200. Phorj's surface — classes,
payload enums, pattern matching, closures/lambda tables, COW index-assign, null-safety operator
family, string interpolation, green-thread traps — sits naturally in the 60–90 band. Two
observations that convert this from "judgment call" to "priced":

- Each new Op costs exactly **3 exhaustive match arms** (`vm::exec_op`, `chunk::validate`,
  `compiler::stack_effect`) with no `_` arm anywhere (invariant 3) — so every addition is a
  visible, reviewed, same-commit cost, not silent accretion.
- The specialization variants that do exist (e.g. `Op::SetIndexLocal` for in-place COW
  index-assign) were added with measured before/after benchmarks per invariant 11 — the
  anti-YAGNI discipline was actually followed [Inferred: memory + invariant 11; not re-measured
  in this audit].

The VM is NOT tracking interpreter complexity 1:1 — sugar is expanded out before both backends
(invariant 5), so the Op set tracks the *desugared core*, which is the correct altitude.
**Grade: [Inferred].**

## F5 — Documentation PROCESS: the living set is lean; the research corpus is the over-engineered layer (≈1MB write-once, read-rarely, growing ~100–500K per audit cycle)

**Measured** [Verified: `wc -l` / `du`]:

- **Living docs — healthy and small**: FEATURES.md 89 lines, INVARIANTS.md 143,
  ARCHITECTURE.md 109, HISTORY.md 96, MASTER-PLAN.md 1,296 (99K), KNOWN_ISSUES 1,133,
  CHANGELOG 2,719. Total ≈5.5K lines. For a language project this is *frugal*.
- **Research corpus — the heavy layer**: full-audit 12 files/500K + roadmap-completeness
  20 files/408K + wave3-4-drafts 64K + this audit's raw dir ≈100K → **≈1.07MB**, i.e. ~⅔ of
  the entire docs tree is frozen research output. 18 frozen specs (≈230K) sit between.

**Is the ceremony (decision register, gap matrix, % ledger, phase trackers) over-engineered for
one developer?** My call: **no for the register, yes for the accumulation.** The 141-row
decision register demonstrably pays: the SUPERSEDED table records ~30 reversals (mostly the
one deliberate DEC-113 naming overhaul), and in an LLM-driven single-dev workflow where every
session is stateless, the register is what prevents re-litigating them — it is load-bearing
infrastructure, not ceremony. Same for the % ledger (it anchors the GA claim). What does NOT
pay is keeping every raw research file at full size forever: the full-audit and
roadmap-completeness raw dirs are superseded by their own syntheses (MASTER-PLAN is the
declared SSOT) yet remain the bulk of the tree, and each new audit adds another raw layer.

**Concrete recommendation feeding the unification goal**: keep MASTER-PLAN + the decision
register + INVARIANTS as the living spine (already the plan); after each audit's findings are
merged into the SSOT, **archive or delete the raw dirs** (they're in git history regardless).
The 18 frozen specs should be squeezed the same way the P4/P5 directive already prescribes:
specs whose content is fully absorbed into MASTER-PLAN/FEATURES become pointers or get pruned.
**Grade: [Verified] for sizes; [Speculative] for the process opinion.**

## F6 — Env-var/flag proliferation: clean. 12 `PHORJ_*` vars, all with live owners; one minor two-flags-one-axis wrinkle. (Explicit negative finding.)

Full inventory [Verified: grep across src/tests/scripts/.github]:

| Flag | Owner | Status |
|---|---|---|
| `PHORJ_PHP`, `PHORJ_REQUIRE_PHP`, `PHORJ_SKIP_PHP` | oracle gate control | live (SKIP added 07-03 for the split pre-commit/pre-push gate) |
| `PHORJ_BLESS` | snapshot re-blessing | live, standard pattern |
| `PHORJ_STUB_MANIFEST`, `PHORJ_STUB_REGISTRY`, `PHORJ_BAKE_STUB_MANIFEST` | bundle/stub registry | live (`src/bundle/`, `src/manifest.rs`) |
| `PHORJ_OBJCOPY`, `PHORJ_CURL` | cross-bundle tool overrides (`src/bundle/cross.rs:295,312`) | live, legitimate seams |
| `PHORJ_IT_PRESENT`, `PHORJ_IT_DEFINITELY_UNSET_XYZ`, `PHORJ_TEST_ENV_DEFINITELY_UNSET_XYZ` | test fixtures | live, test-only |

No dead flags found. One wrinkle: `PHORJ_SKIP_PHP` + `PHORJ_REQUIRE_PHP` are two booleans
encoding one three-state axis (skip / optional / require), leaving one contradictory
combination (both set) whose precedence is implicit in code. Cosmetic; worth a one-line doc
note, not a refactor. **Grade: [Verified].**

## F7 — Crypto surface split (Cryptography / Hash / Random): NOT over-engineering — it mirrors PHP's own layering, which is the project's stated design compass. Call made, as asked.

The lead: "three modules — poor discoverability as a whole; should it have been ONE
`Core.Crypto`?" Re-verified: the split is real — `src/native/crypto.rs` (=`Core.Cryptography`,
argon2 password hash/verify), `src/native/hash.rs` (digests/HMAC/HKDF/PBKDF2), and
`src/native/random.rs` (CSPRNG `secureBytes`/`secureInt`), used in
`examples/web/password-verify.phg` + `examples/guide/crypto-mac.phg` [Verified: file list +
grep]. **Verdict: keep the split.** Three reasons:

1. **It is exactly PHP's shape**: `password_hash()` / `hash_*()+hash_hmac()` / `random_bytes()`
   are three separate PHP API families with no umbrella "crypto" module. Familiarity-first is
   the philosophy's first commandment; merging them would *create* a surprise for the PHP
   developer, violating "removes surprises".
2. **Different audiences and stakes**: password storage (never reach for a bare digest),
   data integrity (MAC/KDF), and randomness are distinct decision domains; one grab-bag module
   invites the classic `md5`-for-passwords misuse the split structurally discourages.
3. The discoverability cost is real but is a **docs problem**, not an architecture problem —
   one cross-referencing paragraph in FEATURES.md ("choosing between Cryptography / Hash /
   Random") closes it at zero code cost. Recommend that paragraph in the P4/P5 doc pass.

**Grade: [Inferred]** — grounded in the verified PHP API mapping + philosophy doc.

## F8 — Watch item: `src/checker/` at 21K lines is the gravitational center of the codebase

Not an over-engineering finding per se (no unjustified abstraction found inside it this pass),
but a proportionality flag: the checker is **2.2× the entire native-spine execution code**
(interp+vm+compiler ≈9.7K) and growing with every discipline pass (collapse_injected,
enforce_injected, enforce_member_vis, alias/generics erasure…). Each "nothing in the wind"-style
rule lands here as another pass over the AST. The per-pass style is legible (matches the
codebase convention) but the S1 recipe itself already notes three near-identical type-walkers
(`expand_aliases` / `erase_generics` / `this`) that could be one `rewrite_type_names(prog,
lookup)` [Verified: spec S1 step 2's own DRY note]. That's the file-size-rule (invariant 13)
pressure point of the next quarter. Recommendation: when the NEXT type-walking pass is added,
take the consolidation — not before. **Grade: [Inferred].**

## F9 — Decision-quality signal from the register: reversals are developer-driven design taste, not architecture thrash (context for all findings above)

Reading all ~30 SUPERSEDED rows [Verified: C-decisions.md §SUPERSEDED]: the overwhelming
majority are surface/naming reversals consolidated in ONE deliberate overhaul (DEC-113:
`fn`→`function`, `Ok/Err`→`Success/Failure`, `->`→`: T`, CLI verb spellings…), plus a handful
of genuine architecture supersessions that each *simplified* (OS-thread pool → green threads;
mark-sweep GC criterion → Rc/COW permanently mooting tracing GC; zero-dep absolutism → 4-dep
vetted policy). Two same-day reversals (DEC-090 ternary, DEC-100 `var`) show decisions being
re-examined within hours — cheap, healthy churn. The one hygiene defect: **C-3** — the 06-26
"NO regex/TLS/serde, LOCKED" framing doc was never updated after the dependency policy
superseded it and is now actively false (4 deps exist; rusqlite+rustls approved 07-03). That is
the doc-drift class the P4/P5 pass already targets; add the native-modules-research plan to its
kill list. **Grade: [Verified].**

---

## Priority order (this dimension only)

1. **F1** — decide the virtual-Core-module unification question BEFORE the W3/W4 waves add
   more injected multi-type modules (each one extends the special-case registry).
2. **F2** — promote A5's native-vs-PHP fuzz probes into the standing gate (cheapest attack on
   the real byte-identity leak).
3. **F5** — fold into P4/P5: archive superseded raw research dirs; add C-3's stale framing doc
   to the kill list; add F7's crypto cross-reference paragraph.
4. **F1-watch** — re-adjudicate W2-7 (PSR-4) against the 07-03 import model before building it.
5. **F3-minor** — correct the `Suspend` doc comment's aspirational wasm claim.
