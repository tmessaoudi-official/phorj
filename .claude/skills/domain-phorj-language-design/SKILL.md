---
name: domain-phorj-language-design
description: Use when a phorj task adds, changes or rules on the language surface - new syntax, a checker rule, an E- or W- diagnostic, import/package/namespace resolution, UFCS, generics, enum/match, tuple, visibility, overload, Any/Object, deprecation, a UNIFIED-SPEC section, a DEC ruling or a PENDING Invariant-15 question; paths src/checker/**, src/parser/**, src/ast/**, src/loader/**, docs/specs/**. How an expert works here - rulings, rejected ideas, traps, evidence.
---

Review date: 2026-10-08 22:37   Validation mode: advisory   Core: .claude/rules/expertise-core.md
Delta over CLAUDE.md: Invariants 5, 12, 15 and 16 and the SSOT quartet are NOT repeated here (CLAUDE.md § Delivery invariants).

## Roles and mental models
- **Language designer under a PHP floor**: PHP is the floor, not the identity or the ceiling; the keep-or-change filter is craftsmanship (SOLID, known patterns), not familiarity; new power must COEXIST with old strengths (multiple inheritance AND traits, overloading AND nullable/variadic); refuse to lie (byte-identity, no panics, no silent coercion).  [Source: memory/philosophy-of-phorge.md; memory/transpile-is-a-bridge-not-a-runtime.md]
- **Adjudicator's clerk, not the judge**: when "more correct" and "PHP-familiar" diverge, recommend the more correct one; the developer repeatedly picks the thorough option, so present the real-cost option with equal weight and a failing current-syntax program.  [Source: memory/search-specs-before-design-questions.md; memory/unification-audit-2026-07-03.md]
- **Front-end owner**: a language feature is front-end work first (parse, check, rewrite, erase) and backend work only when it cannot be erased; prefer a field on the AST node to carry a checker fact to the compiler over a new `compile()` parameter.  [Source: memory/ast-field-carries-checker-fact-to-compiler.md]

## Standards, rulings, norms
| Rule | Applies when | Source | Last checked |
|---|---|---|---|
| Pre-1.0 no deprecation lifecycle (DEC-416): change outright, only the new form parses; the post-1.0 lifecycle in the same file is dormant | retiring or renaming anything | [Source: docs/DEPRECATION.md:3-13] | 2026-10-08 |
| Keyword-vs-import 3 tiers (T-12): built-in types never imported (single list `is_builtin_type_name`), user/library types via `import Pkg.Path.Name [as Alias];` (no wildcard there), stdlib functions `import Core.X;` then `X.fn()` | any new type name, attribute or verb | [Source: docs/INVARIANTS.md:179-199; src/checker/common.rs:195] | 2026-10-08 |
| One `import` keyword (`import type` removed); the resolver classifies by what the PATH resolves to (module, type, else `E-IMPORT-UNKNOWN`); injected Core types are leaf-qualified unless member-imported | import resolution | [Source: docs/specs/UNIFIED-SPEC.md:455-491] | 2026-10-08 |
| Functions are NOT bare-importable; no associated functions; UFCS reaches the CURRENT package's free functions only (DEC-527) | call resolution, UFCS | [Source: docs/specs/UNIFIED-SPEC.md:493-506] | 2026-10-08 |
| Visibility: `private` = this file (top level) or this class (member); `internal` = package plus DESCENDANT packages; `protected internal`/`private protected` rejected | visibility changes | [Source: docs/specs/UNIFIED-SPEC.md:883-897] | 2026-10-08 |
| Block-scope rule (DEC-339): a name already bound by a live local or parameter in the same function is `E-SHADOW-LOCAL`; fields never conflict; a lambda starts a new function | any new declaration form | [Source: docs/specs/UNIFIED-SPEC.md:974-1000; src/checker/plumbing.rs:235] | 2026-10-08 |
| `using (T h = expr) { }` is the only scope guard (`Closable`), contextual keyword, lowers to try/finally (no new Op); `defer` stays rejected | resource cleanup | [Source: docs/specs/UNIFIED-SPEC.md:1305-1342] | 2026-10-08 |
| Ordering (DEC-512): tuples of int/float order lexicographically; `List<T>` is NOT orderable because PHP array `<=>` is count-first | comparison operators | [Source: docs/specs/UNIFIED-SPEC.md:1169-1190] | 2026-10-08 |
| Generics erased, not monomorphized; type safety is a front-end guarantee | generic features | [Source: docs/adr/0002-erasure-not-monomorphization.md:16-22] | 2026-10-08 |
| Language config and feature flags are COMPILE-TIME and baked identically into every backend and the emitted PHP; a runtime knob (env, ini) is rejected because transpiled PHP has no Phorj runtime | any "make it configurable" idea | [Source: memory/config-must-be-compile-time.md] | not re-run |
| Whole-project validation: the loader merges every project file into one AST before checking, so a broken class in an unreferenced sibling fails `phg check` | diagnosing "unrelated" errors | [Source: memory/error-handling-and-whole-project-validation.md] | not re-run |
| Ruled, NOT built: DEC-357 rejects a write to a captured enclosing local (today it silently does nothing on all three legs); DEC-335 `Any`/`Object` tops | before citing them as behaviour | [Source: docs/specs/2026-07-26-capture-write-rejection.md:3; docs/research/full-audit/raw/C-decisions.md:3199-3201; Observed: no DEC-357 in src, 2026-10-08] | 2026-10-08 |

## Tools of the trade
- `phg check <file>` (type errors), `phg explain <CODE>` (every emitted code has an entry; ratchet `every_emitted_diagnostic_code_has_an_explanation`), `phg format --check`.  [Source: README.md:138; src/cli/tests/explain_ratchet.rs:48]
- The decision register (DEC rows with supersession chains) and the rejection catalogue are the search targets BEFORE any design question.  [Source: docs/research/full-audit/raw/C-decisions.md; docs/specs/UNIFIED-SPEC.md:1894-1921]
- Leaf sets as or-pattern macros keep `rustc` exhaustiveness on AST walks (CLAUDE.md § Delivery invariants, Invariant 3).  [Source: src/ast/leaves.rs]

## What "good" looks like
- A new rule ships with a checker negative per rule (its own `E-` code plus an explain entry), a positive differential example, and the formatter printing it back idempotently; the transpile, lift and LSP surfaces follow (CLAUDE.md § Delivery invariants, Invariant 17).  [Source: docs/MILESTONES.md:128-129]
- A question to the developer embeds the failing current-syntax program in the question text and the after-state inside each option; in autonomous mode it becomes a `PENDING (Invariant 15)` register row.  [Source: .claude/skills/phg-ask-human/SKILL.md:54-62]

## Classic failure modes (with detection)
- **Member visibility enforced on some access sites only**: there are six external access surfaces (field read, field write, clone-with, let-destructuring, match struct-pattern, method call); three were found only by a blast-radius pass. All must route through `enforce_member_vis`, or private access runs on the VM and throws in real PHP. Detect: a private-access differential program per surface.  [Source: memory/member-visibility-six-access-sites.md; src/checker/calls/visibility.rs:75]
- **Bare zero-payload variant arm**: `Yes =>` binds a catch-all and matches everything; both backends agree on the wrong output so the differential is blind (now `E-MATCH-BARE-VARIANT`). Detect: read the output, not only the agreement.  [Source: memory/zero-payload-variant-call-form.md; src/cli/explain/match_overloads.rs:72]
- **Contextual keyword collisions**: `as` has three roles (foreach separator, import alias, checked cast); folding the cast into binary parsing broke foreach. Detect: run every existing use of the keyword after a grammar change.  [Source: memory/checked-as-cast-and-contextual-as.md]
- **Generic-subject native leaf collision**: a native whose first parameter is a bare type parameter matches every receiver under UFCS (`List.repeat` collided with `Text.repeat`, renamed `fill`). Detect: the differential example, then grep the leaf name across Core modules.  [Source: memory/ufcs-generic-subject-leaf-collision.md]
- **Smart-cast narrowing drops the binding's facts**: narrowing is a shadow declaration in a child scope and must inherit the outer binding's mutability and declared type (DEC-537).  [Source: memory/wrapper-variants-and-scoped-registries.md]
- **Name-keyed checker lookups outside `Main`**: function tables are keyed by loader-mangled names (`Pkg\name`), so a bare-name lookup works in `Main` and misses elsewhere. Detect: a two-package decoy test.  [Source: memory/mangled-key-lookups-and-functiondecl-fields.md]
- **Formatter-hostile examples**: `phg format` expands grouped imports one per line and strips a redundant `public` on free functions, so a committed example holding `import X.{a,b};` fails the idempotency sweep. Run `phg format` on every new `.phg`.  [Source: memory/dec-196-cleanup-complete.md; memory/formatter-idempotency-new-phg.md]
- **Reading a section literally under a pending banner** (core section 0 item 4): e.g. DEC-207 `::` for class-level access and DEC-214 `new List<T>()` supersede older prose.  [Source: docs/specs/UNIFIED-SPEC.md:28-34]

## Evidence surfaces (what counts as proof here)
- A checker negative test per new code plus `phg explain <CODE>`; a differential example under `examples/` (single-file glob or a project with a manifest); `conformance/` goldens pin the VALUE, which is stronger than agreement alone.  [Source: memory/rock3-conformance-stability.md; tests/differential.rs:2226]
- A design question is evidenced by the register row it creates (DEC-n, or PENDING) in the same change.  [Source: CLAUDE.md:363-365]

## Reviewer lenses
1. **Self-ruling** - did the change decide a user-visible language question alone, or re-open a rejected idea?  2. **Wind** - can any new symbol be used without an import?  3. **PHP semantics drift** - do shadowing, `==`, array `<=>`, by-value capture or reserved names differ on the PHP leg?  4. **Every access surface** - is the rule enforced at all sites of its class (visibility's six, UFCS receivers, both packages)?  [Source: .claude/agents/safety-promises-reviewer.md:70-72; memory/member-visibility-six-access-sites.md]

## Vocabulary
| Term | Meaning |
|---|---|
| DEC-n / CD-n | developer ruling row in the register / completeness-defect row (CD-27, -31, -32 are exhaustiveness exemptions) |
| RULED / QUEUED / BUILT / PENDING | decided / decided not built / shipped / asked and unanswered |
| nothing in the wind | every name traceable to an import or an on-screen declaration |
| injected prelude | stdlib types written in Phorj and injected by `cli::inject_*_prelude` on import (Json, Time, Http, Secret, Closable) |
| T-n vs Invariant N | technical invariant in INVARIANTS.md vs delivery invariant in CLAUDE.md |
| EV-6 / EV-7 | float display rule / no crash on input |
| D-L2, D-L9 | no `any` escape hatch; "Phorj : PHP :: TypeScript : JavaScript" |
| UFCS | method-first call sugar `s.trim()` over free functions |
[Source: CLAUDE.md:356-365; docs/specs/UNIFIED-SPEC.md:13-18; docs/INVARIANTS.md:102-110; docs/adr/0002-erasure-not-monomorphization.md:6-12]

## Canonical sources
- docs/specs/UNIFIED-SPEC.md (language SSOT; pending banners at the top), docs/INVARIANTS.md (T-12), docs/DEPRECATION.md, docs/adr/README.md (ADR process: immutable once Accepted, reversal = a new superseding ADR), docs/research/full-audit/raw/C-decisions.md (register), FEATURES.md, KNOWN_ISSUES.md. Read 2026-10-08.
