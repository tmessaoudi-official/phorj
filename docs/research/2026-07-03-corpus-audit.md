# Corpus + language-surface audit (2026-07-03)

Deep read-only audit (5 parallel agents) of conformance/examples/docs/stdlib, triggered by the
developer's "problems are everywhere" concern. Raw per-agent reports in the session scratchpad
(`audit/raw/1..5`). **Headline: the code is clean; the problems are (1) the ruled `->` removal needs the
formatter fixed first, (2) pervasive DOC DRIFT, (3) a set of stdlib API-consistency choices.**

## A. Language-surface decisions (need developer ruling — §15; breaking OK pre-1.0)

### A1. `->` removal execution order (RULED: remove entirely)
- **BLOCKER:** `phg format` currently emits `->` for FUNCTION TYPES (rewrites `=>`→`->`) — opposite of
  the ruled `=>`. **Fix the formatter first** (emit `=>` for function types; `:` for closure + `declare`
  returns), THEN the sweep is safe.
- Scope: 177 arrows in 75 `.phg`; `phg format` fixes 160 (return types), **17 need manual edits in 9
  files** (function-type `(X)->Y`, inline-closure `function(…)->T`, `declare function …->T` in `php.d.phg`).
  Then make the parser REJECT `->` (both positions). Docs: ~5 fenced snippets + prose.

### A2. Breaking stdlib renames (pick per item)
- `Bytes.find` → **`Bytes.indexOf`** (it returns an `int?` index; `List.find` is predicate→element — a real collision).
- `Map.has` → **`Map.containsKey`** (List/Set/String all use `contains*`; `Map.has` is the odd one).
- slice third arg: `List.slice`/`String.substring` = `(start, LENGTH)` neg-from-end; `Bytes.slice` =
  `(start, END)` half-open no-neg. **Unify the convention** before more slice APIs land.

### A3. Additive stdlib gaps (pick which to add now)
- `Math.abs/min/max/clamp/sign` are **Int-only** — no Float variants (float code can't get abs/clamp).
- `String`: has `containsIgnoreCase`/`equalsIgnoreCase` but no `startsWith/endsWithIgnoreCase`; `replace`
  is replace-all with no `replaceFirst`.
- `Set`: `isSubset` without `isSuperset`; no `symmetricDifference`/`isDisjoint`; no `map`/`filter`.
- Lower: `List.prepend/takeWhile/sortBy`, `Map.mapKeys/entries`, `Bytes.isEmpty`,
  `Random.floatBetween/choice/shuffle`; `List.count` name ambiguity vs `length`; `Conversion.round` dup of `Math.round`.
- (Ruled OUT as deliberate: `length` vs `size` = DEC-102 D; `uppercase`/`lowercase` + module renames = DEC-113.)

### A4. Traits — resolve the 3-way contradiction
FEATURES.md `🔲 future` vs KNOWN_ISSUES "S8 shipped" vs MASTER-PLAN §7-OPEN. A user runs
`guide/traits.phg` successfully while FEATURES says traits don't exist. Decide: shipped (✅) or the
§7-OPEN adjudication genuinely keeps it pending?

## B. Documentation drift (mostly mechanical corrections of FALSE claims)

- **B1 (HIGH). "zero external crates / std-only / no third-party deps" is FALSE** — README:5-6,89,311;
  FEATURES:84. Four vetted deps are on by DEFAULT (`default=["crypto","regex","signals","green"]` →
  argon2, regex, ctrlc, corosensei). Also README's "sole external dependency argon2" (examples/README:150).
- **B2 (HIGH). `import type` documented as current syntax** (removed in S0 — parse error today):
  FEATURES:46, INVARIANTS:133, examples/README:154 (contradicts the actual `import`-using example),
  STABILITY:20, KNOWN_ISSUES ×5. (CHANGELOG/HISTORY historical uses: leave.)
- **B3 (HIGH). Dead CLI verbs in README table:** `lex`→`tokenize`, `disasm`→`disassemble`,
  `bench`→`benchmark`, `fmt`→`format` (short forms removed). README:128/130/131/136; FEATURES:67;
  CONTRIBUTING:66; ARCHITECTURE:54; MILESTONES/GA-CHECKLIST.
- **B4 (HIGH). README:279 cites non-existent `E-TRANSPILE-CONCURRENCY`** — real code `E-CONCURRENCY-NO-PHP`.
- **B5 (MED). FEATURES status column behind reality:** concurrency (L55), `lift` (L79), LSP/formatter
  (L80, contradicts L74's ✅) marked `🔲` but shipped; `phg debug` has NO row.
- **B6 (MED). Whole security stdlib undocumented in FEATURES/README:** Core.Hash (incl. W3-4
  hmac/hkdf/pbkdf2/equals), Core.Random CSPRNG, Core.Crypto argon2, Core.Regex.
- **B7 (MED). S1/S2 import redesign undocumented:** member-imports, `E-INJECTED-TYPE-BARE`, qualified
  `new Http.Router()` / `#[Http.Route]`. FEATURES/README still tell the old `import type` story.
- **B8 (MED). STABILITY.md lists removed keywords as stable:** `import type` (L20), `fn` lambdas (L22).
- **B9 (MED). KNOWN_ISSUES.md "not yet implemented" tail (L338-340) lists SHIPPED features** as missing:
  exceptions, method/function overloading, traits, property accessors, const/final. Only
  operator-overloading, sized-integers, statement-position `match` are genuinely pending.
- LOW: `phg serve` doc entry `handle(Request)->Response` vs actual `respond(bytes)->bytes`; `phg lsp`
  missing from `--help`; docs route to `ROADMAP.md` not the SSOT `MASTER-PLAN.md`; HISTORY.md tail stub.

## C. Examples / conformance (mechanical + additive)

- **C1 (P1, invariant 9).** 5 single-file examples with no `examples/README.md` row
  (`interop/exceptions`, `lift/sample`, `process/args-env`, `random/dice`, `web/json-api`); 5 project
  examples documented nowhere (`funcvalues`, `genericbox`, `inherit`, `jsonmulti`, `mixins`).
- **C2 (P2).** 3 subtrees absent from the top-level README index: `debug/`, `lift/`, `random/`.
- **C3 (P2).** Diagnostics corpus thin (9 cases). **19 E-codes have no by-name coverage anywhere**,
  clustering on no-PHP-analog surfaces (concurrency/channels 5, property hooks 4, spawn 2, overloading 2,
  misc 6) → add `conformance/diagnostics/*.phg`+`.expected` pairs.
- **C4 (P2).** 4 E-codes emitted in `src/` but absent from `phg explain`: `E-STATIC-INIT-CONST`,
  `E-TYPE-IMPORT-BUILTIN`, `E-TYPE-IMPORT-SHADOW`, `E-FIELD-INIT` (last: verify not a prefix). NB the two
  `E-TYPE-IMPORT-*` are import-redesign leftovers — likely dead now (S0 re-homed to `E-IMPORT-*`).

## D. Tooling (deferred to AFTER corpus clean, per decision)
- Fix `phg format` function-type output (A1 blocker — do this FIRST, it's also correctness).
- Editor/LSP refresh: VSCode `tmLanguage`/snippets + PhpStorm + LSP completion/hover for new natives
  (W3-4 crypto), injected-type discipline (S2), `:`/`=>` syntax, dead-verb removal.

## Non-issues (verified clean)
Code has zero stubs (no `todo!()`/`unimplemented!()`; 46 guarded `unreachable!()`). Naming migration
complete (no real `Str/Obj/Arr`/`println`/`fn`). `conformance/diagnostics/*` is an INTENTIONAL must-fail
corpus (not bugs). All 29 `Core.*` modules have an example. Headline README snippets run correctly.
