# M-RT — Rich Types Milestone Plan

> TypeScript-grade type system for Phorge, mapped to PHP 8.0/8.1 natives. Built slice by slice,
> each an independent green commit with a byte-identical (`run ≡ runvm ≡ real PHP ≥8.6`) example.
> Full design: `docs/specs/2026-06-20-m-rt-rich-types-design.md`. Approved plan mirror:
> `~/.claude/plans/misty-honking-lynx.md`.

## Decisions Log

- [2026-06-20] AGREED: `is` value-equality stub is a GA blocker (parses + type-checks but
  `transpile.rs:623` rejects; `interpreter.rs:515` is a misleading `eq_val` alias). Resolve it.
- [2026-06-20] AGREED: keyword is **`instanceof`** (lowercase, PHP-style), RHS parsed as a Type.
  `is` ambiguity (reads like equality) is what caused the original stub bug — conceded over my
  initial `is`-keyword preference.
- [2026-06-20] AGREED: **maximal scope** — full TS-grade type system (interfaces, instanceof, unions,
  intersections, erased generics, inheritance, Map/Set, traits). Feasible because PHP 8.0/8.1 has
  union/intersection/interface/instanceof natively. Chosen over my "coherent cluster only" + "defer"
  recommendations after I challenged hard at each step; developer: "put a real effort here".
- [2026-06-20] AGREED: discipline guardrails — enum-vs-union coherence rule, `W-INSTANCEOF-CHAIN`
  lint, `extends` final-by-default + explicit `override`, generics fully erased (no monomorph),
  no silent Op growth.
- [2026-06-20] AGREED: build order S1 instanceof → S2 interfaces → S3 Map/Set → S4 unions →
  S5 intersections → S6 extends → S7 generics → S8 traits. Only S1+S3 add Ops.
- [2026-06-20] AGREED (pace): proceed autonomously, gate per commit; commit green self-contained
  slices (project git autonomy). Plan approved via ExitPlanMode.

## Formal Plan

See the approved plan (`~/.claude/plans/misty-honking-lynx.md`) and the design spec. Slice table:

| # | Slice | New Op? | Status |
|---|-------|---------|--------|
| S1 | `instanceof` (class-only) + smart-cast, retire `is` | `Op::IsInstance` | **DONE** (gate green: 394 lib + 10 PHP-oracle differential; clippy+fmt clean; example byte-identical run≡runvm≡PHP) |
| S2 | interfaces + `implements` (+ instanceof interface table) | no | pending |
| S3 | Map/Set values + literals + indexing | `MakeMap/MakeSet/IndexMap` | pending |
| S4 | union `A\|B` + match-over-union exhaustiveness | no | pending |
| S5 | intersection `A&B` (requires S2) | no | pending |
| S6 | `extends` (final-by-default, `override`) | no (flatten) | pending |
| S7 | erased generics `<T>` (+ unblock core.list) | no (erase) | pending |
| S8 | traits/mixins | no (flatten) | pending |

## S1 task checklist

- [ ] `token.rs` + `lexer.rs`: `instanceof` keyword
- [ ] `ast.rs`: `Expr::InstanceOf { value, type_name, span }`; remove `BinaryOp::Is`
- [ ] `parser.rs`: parse `x instanceof TypeName` (RHS = type name); remove `T::Is` op mapping
- [ ] `checker.rs`: typecheck + true-branch narrowing; remove 2 `BinaryOp::Is` arms; `E-INSTANCEOF-TYPE`
- [ ] `interpreter.rs`: eval `Expr::InstanceOf` (class-name compare); remove `BinaryOp::Is` arm
- [ ] `chunk.rs`: `Op::IsInstance(usize)` + `type_tests: Vec<String>` + validate bounds arm
- [ ] `compiler.rs`: compile `Expr::InstanceOf`; `stack_effect` arm
- [ ] `vm.rs`: `exec_op` arm
- [ ] `transpile.rs`: emit `$x instanceof Name`; remove the `is` rejection
- [ ] `examples/guide/instanceof.phg` + `examples/README.md` entry
- [ ] `KNOWN_ISSUES.md` / `FEATURES.md` / `CHANGELOG.md` updates
- [ ] gate (`cargo test` w/ `PHORGE_REQUIRE_PHP=1`, clippy, fmt) + commit
