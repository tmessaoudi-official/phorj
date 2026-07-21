# A2 — `.phg` corpus: old/deprecated syntax, formatting drift, doc/example mismatches

Audit dimension 2 of 5 · from-scratch, evidence-graded · HEAD `0691228`, clean tree · 2026-07-03
Auditor scope: the whole `.phg` corpus (examples/tests/conformance/selftest), parser accept-paths for
supposedly-gone syntax, `phg format --check` drift, examples/README.md ↔ disk cross-check, invariant 9.

**Corpus inventory** [Verified: `find` counts]: 236 `.phg` total non-target = examples 174
(guide 117, realworld 4, rest project/web/interop/…) + conformance 58 + tests 2 + selftest 2.
Note: prior session memory says "121 files purged" — the corpus is 236 files; the 121 figure
referred to the subset that contained arrows, not the whole corpus [Inferred: counts above].

---

## A. The `->` seed pattern — CONFIRMED: parser silently accepts, corpus is clean ("accepted but unused")

### A1. All 6 parser accept-sites are live, at exactly the claimed lines [Verified: grep + read]

```
src/parser/types.rs:109   if self.eat(&TokenKind::FatArrow) || self.eat(&TokenKind::Arrow)   // function TYPE `(int) => R` — `->` alias
src/parser/items.rs:240   if self.eat(&TokenKind::Colon)    || self.eat(&TokenKind::Arrow)   // free-function return type
src/parser/items.rs:296   same pattern                                                        // (method decl)
src/parser/items.rs:370   same pattern                                                        // (interface/abstract member)
src/parser/items.rs:735   same pattern — comment: "A-1: `:` canonical, `->` transition alias" // (declare signature)
src/parser/exprs.rs:546   same pattern                                                        // lambda return type
src/lexer/mod.rs:1125     (b'-', Some(b'>')) => Some(TokenKind::Arrow)                        // lexer still produces the token
```

The in-code comments explicitly label this as deliberate: types.rs:108 *"`->` stays as a silent
transition alias"*; items.rs:238-239 *"`-> T` is a silent transition alias (kept until every inline
test program is migrated — `.phg` sources use `:`)"* [Verified: read those lines]. So this is a
**tracked transition state, not an accident** — but it is still a live enforcement gap: nothing
warns or errors.

### A2. Empirical proof of silent acceptance [Verified: ran it]

A test program using `->` in all three positions (free-function return, function-type param,
lambda return):

```phorj
function add(int a, int b) -> int { ... }
function apply((int) -> int f, int x) -> int { ... }
var double = function (int n) -> int { return n * 2; };
```

- `phg check` → `OK (type-checks clean)`, exit 0.
- `phg run` → correct output (`3`, `10`), exit 0.
- `phg format -` on the same file rewrites every arrow to canonical (`: int`, `(int) => int`)
  with no diagnostic [Verified: ran all three, binary `phg 1.0.0-nightly.0` built at HEAD].

### A3. The corpus is 100% clean of *syntactic* `->` [Verified: swept + filtered]

`grep -Hn -- '->'` over all 236 `.phg` files → 145 raw matching lines. After stripping string
literals and `//` comments, **0 lines** retain a `->` (all matches are prose like
`Output.printLine("{path} -> {st}")` or comments like `// 2 already present -> no-op`).
Conclusion: the corpus purge (479dee4) is genuinely complete; the parser alias is **accepted but
unused** by every committed `.phg`.

### A4. What blocks removing the alias [Verified comment / Inferred scale]

Per the parser comments, removal waits on the *embedded* Phorj programs inside Rust string
literals (`tests/*.rs` + injected preludes in `src/`). Raw `->` counts in test files (Rust's own
`->` included, so an upper bound): `tests/differential.rs` 403, `tests/project.rs` 43,
`tests/serve.rs` 24, `tests/typecheck_integration.rs` 21, others <15 [Verified: grep -c].
The prior-session estimate of ~1700 embedded arrows across src/+tests/ is consistent with this
but not independently reconfirmed here [Unverified: would need string-literal-aware parsing].
Removing the 6 accept-sites today would break the embedded-program test corpus, not the `.phg`
corpus [Inferred: from the comments + counts; matches the documented P1-remainder plan in
docs/plans/wave0-remainder.plan.md].

**Severity**: by design/tracked, but any doc claiming "`->` is gone" is overclaiming while these
sites exist. See C below — several docs still *teach* `->`.

---

## B. `import type` — parser correctly REJECTS it, but four docs still teach it as current syntax

### B1. Parser rejection [Verified: ran it]

Copied `examples/project/shapes/` to scratch, changed one line to `import type Acme.Geometry.Rect;`:

```
parse error at 11:8: expected a module path segment, found TypeKw
import type Acme.Geometry.Rect;
       ^        (exit 1)
```

So unlike `->`, `import type` is NOT a silently-accepted ghost — it is a hard error. The problem
is inverted: **documentation still presents a syntax that now fails to parse.**

### B2. Doc sites teaching `import type` as CURRENT syntax [Verified: grep + read]

| Site | Text |
|---|---|
| `examples/project/README.md:88` | "a `class`/`enum`/`interface` in a library package is consumed cross-package via `import type Pkg.Path.TypeName;` (see `shapes/`)" — **wrong**; shapes/ itself now uses plain `import` |
| `examples/README.md:154` | `project/shapes/` index row: "consumed from `package Main` via `import type Acme.Geometry.Rect;`" — **contradicts the file it describes** (shapes/src/main.phg:11 is `import Acme.Geometry.Rect;`) |
| `FEATURES.md:46` | feature row titled "Cross-package types — `import type Pkg.Path.Type [as A]`" marked ✅ — the ✅'d syntax no longer parses |
| `docs/INVARIANTS.md:133` | "User / library types — `import type Pkg.Path.Name [as Alias];`" — an INVARIANTS doc stating a dead form |

(`docs/HISTORY.md:74` also mentions it, but as chronological narrative — acceptable.)

### B3. Stale `import type` comments inside `.phg` files [Verified: grep; statements are clean]

No `.phg` file contains an `import type` *statement* (`grep -E '^\s*import\s+type\s'` → none).
Three files still reference it in comments:

- `examples/project/shapes/src/Acme/Geometry/Shape.phg:3` — "EXPORTS types for other packages to `import type`"
- `examples/project/visibility/src/main.phg:5` — "this cross-package `import type` is allowed"
- `examples/project/inherit/src/Acme/Zoo/Animal.phg:4` — "(`import type`), override its `open` methods…"

Cross-ref: this is the concrete file:line list behind the P4/P5 "`import type` ghosts" item in the
cleanup program.

---

## C. Old `->` syntax still taught in documentation (would parse via alias today, breaks when alias is removed)

### C1. Runnable Phorj code blocks using `-> ret` [Verified: read the lines]

- `examples/dump/README.md:26` `function compute(int n) -> int {` and `:33` `function main() -> void {`
- `examples/lift/README.md:42` `function greet(string name) -> string {`, `:48` `public open function next() -> int {`, `:53` `function main() -> void {`

These are full code listings a reader would copy. They parse today only because of the A1 alias;
they contradict the canonical style and will hard-break when the alias is removed.

### C2. `declare` signatures documented with `->` while the corpus uses `:` [Verified]

- `examples/interop/README.md:20` `declare function strtoupper(string) -> string;` and `:27`
  `declare function name(params) -> ret;`
- Actual corpus form: `examples/interop/builtins.phg:17` and `withdecls/src/php.d.phg:10` both read
  `declare function str_repeat(string s, int times): string;` — README and corpus disagree.

### C3. API pseudo-signature notation `f(args) -> T` throughout the doc set [Verified: grep counts]

`examples/README.md` alone has **41** `->` occurrences (e.g. line 70 `parse(string) -> Json?`,
line 144 `(Request) -> Response` — the latter describes a *function type*, whose canonical form is
`(Request) => Response`). Also: `examples/process/README.md:10-12`, `examples/random/README.md:9-12`,
`examples/web/README.md:3,10,12`, `examples/errors/README.md:42`, `examples/build/README.md:9`
(shell comment — harmless), `examples/project/README.md:24` (tree-diagram annotation). FEATURES.md: 1.
Severity: informal notation, but it is exactly the notation the language just purged — the index
doc teaches non-canonical arrows 41 times.

---

## D. `phg format --check` drift — 2 files, both in `selftest/`, plus an enforcement-gap root cause

### D1. Drift [Verified: ran `phg format --check` on all 236 files]

- **`selftest/arithmetic.phg`** — would reformat: formatter inserts a blank line after
  `package Main;` (file currently has `package Main;` immediately followed by `import Core.Test;`).
- **`selftest/faults.phg`** — identical drift (blank line after line 1).
- All other 234 files (examples 174, conformance 58, tests 2) pass `--check` clean.

### D2. Why the green gate didn't catch it [Verified: read tests/fmt.rs:112-167]

`tests/fmt.rs::every_repo_phg_formats_idempotently_and_safely` collects `examples/` **and**
`selftest/` (fmt.rs:120-121) but asserts only (a) formats without error, (b) **idempotent**,
(c) meaning-preserving. It never asserts `fmt(src) == src` — i.e. the corpus test does NOT enforce
that committed files are already formatted. So `selftest/` can drift indefinitely while the gate
stays green. The Phase-1 reformat (479dee4) covered examples/tests/conformance; `selftest/` was
missed [Inferred: drift exists only there + commit scope].

**Suggested fixes** (either): run `phg format selftest/` (2-line diff), and/or strengthen the
corpus test (or a pre-commit step) to assert already-formatted for tracked `.phg`.

---

## E. examples/README.md ↔ disk cross-check (bidirectional)

Inventories: disk = 174 `.phg` under `examples/` [Verified: find]; README side = the main
`examples/README.md` index (197 unique backtick path references) + 13 per-directory READMEs
(`bench, build, cli, debug, dump, errors, interop, lift, process, project, random, transpile, web`)
[Verified: ls + extraction]. `guide/` and `realworld/` have no sub-README and are indexed in the
main README.

### E1. README→disk direction: clean [Verified]

The single candidate ("`return-overloading.phg` referenced but missing") was an extraction
artifact: `examples/guide/return-overloading.phg` exists; README:101 mentions it prefix-free in
prose. Every README-referenced example file exists on disk.

### E2. Disk→README direction: 5 project dirs documented NOWHERE [Verified: exact greps in both the main README and examples/project/README.md all return 0]

- `examples/project/funcvalues/` (2 files)
- `examples/project/genericbox/` (3 files)
- `examples/project/jsonmulti/` (2 files)
- `examples/project/mixins/` (3 files)
- `examples/project/inherit/` (2 files)

`project/README.md` walks through `tempconv/` in depth and name-drops `shapes/` and `withdeps/`;
`project/visibility/` is indexed in the main README. The five above are runnable, differential-gated
examples with zero documentation entry — an invariant-9 (examples/README entry) violation for
whichever features they shipped with.

### E3. Minor index omission [Verified]

`examples/web/json-api.phg` is absent from the main README's web table (which lists the other
9 web files) — it IS documented in `examples/web/README.md`. Inconsistent indexing rather than a
true gap. All other "missing" singles are covered by their sub-README: `random/dice.phg` (3 mentions),
`process/args-env.phg` (2), `interop/exceptions.phg` (1), `interop/withdecls` (2), `lift/sample.phg` (4).

---

## F. Invariant 9 — recently-shipped features vs examples

- **W3-4 crypto**: `examples/guide/hashing.phg` + `examples/guide/crypto-mac.phg` exist; main README
  has rows mentioning hmac/hkdf/pbkdf2 [Verified: find + grep, 1 hit each].
- **Core.Random secureBytes/secureInt**: no runnable example — correct per invariant 9's
  fault/non-deterministic carve-out (CSPRNG output can't be byte-identity-gated); both are
  README-mentioned [Verified: grep, 1 hit each].
- **NDJSON / INI**: `examples/guide/ndjson.phg` + `examples/guide/ini.phg` exist; README rows present
  (`parseLines`, `Core.Ini` each 1 hit) [Verified].
- **Import redesign S0–S2** (unified import, injected-type qualification, `E-INJECTED-TYPE-BARE`):
  the corpus was migrated (19 files) and `guide/json.phg`'s README row documents
  `E-INJECTED-VARIANT-BARE`, but there is **no dedicated guide example** demonstrating the new
  discipline (qualified `Http.Router`, `#[Http.Route]`, what `E-INJECTED-TYPE-BARE` rejects), and no
  README row for the TYPE-level rule [Inferred: `find examples -iname '*import*'` → none; grep for
  E-INJECTED-TYPE in examples/README.md → none]. Judgment call whether a language-discipline change
  "ships an example" — flagged for the developer, not ruled (ADJUDICATION RULE).

---

## G. Naming-rule sweep (CLAUDE.md rule 12 / naming-overhaul spec) — clean

- `fn` as a keyword: **0** occurrences in the corpus [Verified: grep].
- Lowercase `class`/`enum`/`interface` names: **0** [Verified: grep].
- snake_case function names: only `declare function str_repeat(...)` in
  `interop/builtins.phg:17` + `withdecls/src/php.d.phg:10` — foreign-PHP declares that MUST carry
  the PHP name; intentional, not a violation [Verified: read].
- Old `Obj`/`Arr`/`Str` injected-variant names: **0** in injected-enum positions. The `Str`/`Int`/
  `Float` variants in `examples/guide/enum-reserved-variants.phg:14` are USER-defined enum variants —
  the file's entire purpose is demonstrating PHP-reserved-word variant mangling (`Int` → `Int_`);
  intentional [Verified: read the file header].
- `import type` statements: **0** (see B3 for residual comments) [Verified: grep].

---

## Summary table

| # | Finding | Where | Grade | Severity |
|---|---|---|---|---|
| A | `->` alias: 6 parser sites + lexer accept it silently; corpus 100% clean → accepted-but-unused; removal blocked by embedded Rust-string programs (tracked P1-remainder) | src/parser/{types.rs:109, items.rs:240/296/370/735, exprs.rs:546}, src/lexer/mod.rs:1125 | Verified (empirical: check/run exit 0) | P1 (tracked, but docs overclaim while it exists) |
| B | `import type` now a hard parse error, yet 4 docs teach it as current + 3 stale `.phg` comments | project/README.md:88, examples/README.md:154, FEATURES.md:46, docs/INVARIANTS.md:133; Shape.phg:3, visibility/main.phg:5, Animal.phg:4 | Verified (empirical rejection) | P1 (docs actively wrong) |
| C | Old `->` still taught in docs: 5 runnable code-block lines (dump, lift READMEs), 2 wrong declare-signatures (interop README), 41 pseudo-sig arrows in the main index | see C1–C3 | Verified | P2 |
| D | fmt drift in `selftest/{arithmetic,faults}.phg` (missing blank after `package`); root cause: fmt corpus test checks idempotency, never formatted-ness | selftest/*.phg; tests/fmt.rs:117-167 | Verified | P2 (2-line fix + gate gap) |
| E | 5 `examples/project/` dirs (funcvalues, genericbox, jsonmulti, mixins, inherit) documented nowhere; `web/json-api.phg` missing from main index only | examples/project/, examples/README.md | Verified | P2 |
| F | Recent features (crypto, NDJSON, INI) all have examples+README rows; import-redesign discipline has no dedicated example/row | examples/guide/ | Verified / Inferred (last item) | P3 (last item = PENDING dev question) |
| G | Naming rules: corpus fully clean (declares + reserved-variant demo are intentional) | — | Verified | none |

Certification note: this subagent has no `advisor()` tool; per the Phase 3C/6C carve-out the
three-lens check (completeness / adversarial / blast-radius) was self-graded — disclosed here, and
certification can be re-run by the orchestrating session.
