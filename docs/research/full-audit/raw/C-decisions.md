# Agent C — Full Decision Register

> Harvested 2026-07-02 from: 66 `docs/plans/*.md` (Decisions Log sections + inline markers), 81
> `docs/specs/*.md` (Decision/D-x/LOCKED markers), `/stack/projects/phorj/CLAUDE.md`, and the 100-file
> memory dir (`~/.claude/projects/-stack-projects-phorj/memory/`). Duplicated records (plan+spec+CLAUDE.md)
> are merged into one row with the primary source cited. The 555-row parity triage
> (`docs/specs/2026-06-21-php-parity-and-beyond.md`) is summarized by category (§ Parity SSOT), with only
> contested/major rows pulled out individually.
>
> **Mode legend:** ASKED = developer explicitly chose (AskUserQuestion / plan approval / recorded
> "developer chose/overruled"). AUTONOMOUS = decided in a `_AUTONOMOUS_3C` / bypass-sentinel session
> without a per-decision ask (incl. "locked at implementation" entries inside autonomous slices).
> RATIFIED = made autonomously, later reviewed & confirmed by the developer (counted with ASKED in
> totals, flagged separately). **Shipped:** ✅ in code · 📐 designed-only · ⬆ superseded · ◐ partial.

---

## 1. Foundational doctrine & process

| ID | Date | Decision | Alternatives rejected | Source | Mode | Shipped |
|----|------|----------|----------------------|--------|------|---------|
| DEC-001 | 06-15 | Three-backend model: tree-walking interpreter + bytecode VM + Phorj→PHP transpiler, gated by a byte-identity differential spine (`run ≡ runvm`) | single backend | specs/2026-06-15-phorj-language-design.md, m2-bytecode-vm-design.md | ASKED | ✅ |
| DEC-002 | 06-17/18 | Transpile contract **D-L9**: Phorj : PHP :: TypeScript : JavaScript — every feature maps to idiomatic PHP; PHP-absent features compile-time-only + erased | features w/o a PHP target | specs/2026-06-17-m3-language-roadmap-design.md | ASKED | ✅ |
| DEC-003 | 06-19 | M7 PHP oracle in the loop: transpiled PHP executed under real `php` must match interpreter stdout; `PHORJ_REQUIRE_PHP=1` fails-not-skips | skip-when-missing | plans/2026-06-19-m7-correctness-closure.plan.md; memory php-leg-outside-correctness-loop | ASKED | ✅ |
| DEC-004 | 06-21 | **Philosophy locked**: craftsmanship (SOLID/patterns/best practice) is the APEX filter — not familiarity, not purism; PHP is the floor, never the ceiling; additive power, never remove capability | familiarity-first; PL-theory purism (both explicitly corrected) | memory philosophy-of-phorge; parity SSOT §1 | ASKED (dev corrected Claude twice) | ✅ doctrine |
| DEC-005 | 06-27 | **Transpile is a bridge, not a runtime**: every feature/native implemented natively on Rust backends; PHP emission is a peer target, never the source of truth; never delegate a capability to PHP | PHP-only implementations (Claude proposed twice, rejected) | memory transpile-is-a-bridge-not-a-runtime | ASKED (hard feedback) | ✅ doctrine |
| DEC-006 | 06-24 | **Language config must be compile-time** (phorj.toml `[language]` / editions → M13); runtime knobs (env/.ini) architecturally rejected — transpiled PHP runs with no Phorj runtime, would silently break byte-identity in prod | runtime env/.ini flag | plans/2026-06-24-language-evolution-master.plan.md; memory config-must-be-compile-time | ASKED | ✅ doctrine (M13 📐) |
| DEC-007 | 06-26 | **Determinism Partition**: every capability is Tier A (pure/deterministic → byte-identity-gated) or Tier B (impure → quarantined via Transport model, fixture-tested); admission decided CASE-BY-CASE, no blanket Tier-B charter | blanket Tier-B charter (dev's own round-1 lean, withdrawn post-challenge) | plans/2026-06-26-native-modules-research.plan.md + extended-scope.plan.md | ASKED | ✅ |
| DEC-008 | 06-26 | Zero-dependency std-only core ("NO TLS, NO regex, NO http/serde crates") as locked framing | — | plans/2026-06-26-native-modules-research.plan.md | ASKED | ⬆ superseded by DEC-009 (see CONFLICTS C-3) |
| DEC-009 | 06-27→29 | **Dependency policy**: narrowly-scoped vetted external deps admitted per-domain — argon2 (crypto), regex (ReDoS-immune matching), ctrlc (signals), corosensei (coroutines); all optional/feature-gated, playground stays dep-free | hand-rolled crypto/regex/unsafe; general-purpose deps (tokio et al. stay disallowed) | specs/2026-06-27-dependency-policy.md; Cargo.toml comments | ASKED (each dep individually authorized) | ✅ |
| DEC-010 | 06-21 | **Autonomy contract**: TOTAL autonomy incl. big architecture, stop+ask only on genuine craftsmanship forks; auto-commit green slices; NEVER push | per-slice checkpoints | memory ga-direction-and-autonomy | ASKED | ✅ standing |
| DEC-011 | 06-17 | **Examples ship with features** (standing rule): every feature lands with a byte-identity-gated `examples/` program + README entry in the same change | retroactive examples | memory examples-ship-with-features; CLAUDE.md | ASKED | ✅ standing |
| DEC-012 | 06-25 | Overnight-session fork protocol: genuine forks logged with provisional call + `⏳ AWAITING CONFIRMATION`, never decided silently; walked next morning | silent autonomous decisions | plans/2026-06-25-overnight-autonomous-session.plan.md | ASKED | ✅ process |
| DEC-013 | 06-28 | **Rename Phorge → Phorj** (reads "forge"; Phorge = active Phabricator fork, SEO/legal collision); `phg` binary + `.phg` extension kept | Clarus/Hone/Hearth/… shortlist; fire-theme names (all collide); keep Phorge | memory name-collision-rename-decision | ASKED | ✅ code (`297229f`); GitHub repo rename + dir `mv` still manual |
| DEC-014 | 06-18 | CLI binary renamed `phorj` → `phg` (ripgrep model: package/lib/env-vars stay `phorj`… then-`phorge`) | — | CLAUDE.md (`70ea75d`); memory binary-renamed-to-phg | ASKED | ✅ |
| DEC-015 | 06-18 | Quality bar for every mapped PHP feature: BETTER / SAME+syntax / SAME / WORSE(reject) — never worse than PHP | — | plans/2026-06-18-m8-php-import-design.md | ASKED | ✅ doctrine |
| DEC-016 | 07-01 | Full-audit shape: audit-first NO code; every recorded decision adjudicated interactively (AskUserQuestion, batches of 4); "100% of the language" = everything ever mentioned, no cutline; CLAUDE.md full rewrite rules-only | — | plans/2026-07-01-full-audit-and-master-plan.plan.md | ASKED | in progress (this register) |

## 2. Namespace / module / package system

| ID | Date | Decision | Alternatives rejected | Source | Mode | Shipped |
|----|------|----------|----------------------|--------|------|---------|
| DEC-020 | 06-18 | **"Nothing in the wind"** — everything namespaced by default, no free-floating globals | globals-by-default | specs/2026-06-18-m3-namespace-system-design.md; memory namespace-system-decisions | ASKED | ✅ (intrinsics gap being closed, DEC-047) |
| DEC-021 | 06-18 | **Go-style module-qualified calls** (leaf-qualified: root in the import, leaf at the call site) | Java `System.out.println` object-path (no idiomatic PHP target, breaks D-L9); 3-segment full paths | same | ASKED | ✅ |
| DEC-022 | 06-18 | Reserved `core.` stdlib root; jargon-free leaves `console` (not io), `file` (not fs), `text` (not string — "avoids shadowing the `string` type") | io/fs/string names | same | ASKED | ⬆ leaves renamed twice (DEC-034, DEC-113; see CONFLICTS C-4) |
| DEC-023 | 06-18 | Bare global `println` **RETIRED**; `println` requires `import core.console;` | prelude/auto-import | same | ASKED | ✅ (name now `Output.printLine`) |
| DEC-024 | 06-18 | Explicit import required even for stdlib | prelude imports | same | ASKED | ✅ |
| DEC-025 | 06-18 | User code **mandatorily packaged**, `package` never inferred — even `-e`/stdin one-liners write `package Main;`; reserved `package Main` = runnable entry (Go model) | inferred packages; PHP/TS optional namespacing | plans/2026-06-18-m5-modules-packages.md | ASKED | ✅ |
| DEC-026 | 06-18 | Native registry keyed by `(module, name)`; one `Op::CallNative(idx, argc)`; `Op::Print` retired; shared `eval` = structural parity (one impl, two callers) | per-native Ops; two print mechanisms | plans/2026-06-18-trackB-stdlib-io-imports.md | ASKED | ✅ |
| DEC-027 | 06-18 | `E-SHADOW-IMPORT`: a value binding may not shadow an imported qualifier (keeps locals-first run-backends and import-map transpiler consistent) | — | same; memory namespace-system-decisions | AUTONOMOUS (impl detail) | ✅ |
| DEC-028 | 06-18 | Manifest = **Composer vocabulary in an honest TOML** (`phorj.toml`, `[require]`, `vendor/package` names); literal `composer.json` REJECTED (a file the composer tool can't process is a false promise); **exact-pin only**, no `^`/`~` ranges | composer.json; version ranges + resolver | plans/2026-06-18-m5-modules-packages.md | ASKED (dev's own kill-shot) | ✅ |
| DEC-029 | 06-18 | Directory=package, strict folder=path (`E-PKG-PATH`), enforcement path-aware in the **loader**, never in `check()`; flat AST merge | enforcement in checker | same | ASKED | ✅ |
| DEC-030 | 06-18 | Cross-package resolution = **loader-side name-mangling to PHP FQNs** before any backend (backends consume rewritten AST unchanged → run≡runvm structural) | backend-aware resolution | same (S2c) | ASKED | ✅ |
| DEC-031 | 06-18 | PHP emission = **single-file brace-namespace blocks** + `\Main\main()` bootstrap | PSR-4 dir tree + Composer autoload (can't autoload free functions; Phorj is function-heavy) | specs/2026-06-18-m5-project-model-design.md; selective-type-import spec | ASKED | ✅ |
| DEC-032 | 06-18 | Library packages export **functions only** (`E-PKG-TYPE`) — interim scope | — | plans (S2c) | ASKED | ⬆ lifted by DEC-036 (planned supersession) |
| DEC-033 | 06-18 | M5 S3: git deps + `phorj.lock` (SHA pin + FNV-1a-64 tree hash) + `phg vendor` = the ONLY network-touching command; run/check/transpile offline-only (`E-VENDOR-MISSING`); guards `E-DUP-DEF`, `E-VENDOR-MAIN` | live fetch on run | plans/2026-06-18-m5-modules-packages.md | ASKED (design 3C-converged) | ✅ (transitive deps deferred) |
| DEC-034 | 06-20 | Stdlib root + leaves become **PascalCase** (`Core.Console`, `Core.Text`…; fn names stay camelCase) | lowercase `core.*` | plans/2026-06-20-m-rt-rich-types.plan.md ("even native core should be PascalCase") | ASKED | ✅ (`c4479d6`) |
| DEC-035 | 06-20 | **Casing is a HARD ERROR for all**: package/folder segments PascalCase (`E-PKG-CASE`), types PascalCase, fns/vars camelCase; no `W-CASE` lint fallback; manifest key `name` → `module`; PascalCase enforced incl. vendor (PHP deps case-mapped at importer boundary) | warn-only lint | plans/2026-06-20-post-wave3-four-tracks.plan.md; parity write-back | ASKED | ✅ (`15a5745`+) |
| DEC-036 | 06-20 | E-PKG-TYPE **lifted**: library packages may declare class/enum/interface, consumed via terminal **`import type Pkg.Path.Type [as A];`**; all three kinds in one commit; codes `E-TYPE-IMPORT-*` | classes-first phasing; module-qualified `Geometry.Point` form (deferred) | specs/2026-06-20-epkgtype-lift-crosspackage-types-design.md | ASKED ("all three at once") | ✅ |
| DEC-037 | 06-20 | Selective type import applies to user/library types ONLY; built-ins stay import-free; **no wildcard** (PHP has no `use A\*`) | `import Core.List.List` | specs/2026-06-20-selective-type-import-design.md | ASKED | ✅ |
| DEC-047 | 07-01 | **No-wind closure** (design-locked, NOT implemented): fault intrinsics `panic/todo/unreachable/assert` move behind mandatory `import Core;`, called `Core.assert(...)` etc. (`E-UNIMPORTED`); deep imports `import Core.A.B.C` any depth binding bare leaf AND parent-qualified; aliasing extended to stdlib+deep; de-reserve `Attr`→Core.Html, `Error`→Core.Error, `Channel`/`Task`→**`Core.Async`** (dev rejected "Concurrent" as misnomer — tasks are cooperative, never parallel) | keep intrinsics in the wind; `Core.Concurrent` | specs/2026-07-01-no-wind-namespace-and-language-surface-design.md | ASKED | 📐 ⊳ intrinsic-imports leg superseded by DEC-196 Q3 (two-mode `Core.Assert`/`Core.Abort`, shipped 07-05); deep imports / aliasing / de-reservations still open (C-9) |
| DEC-048 | 07-01 | Import roots: PSR-4-style optional `[packages]` map in manifest; default root `src/` folder=path; first-party bare; `vendor:` prefix for deps | — | specs/2026-07-01-import-roots-psr4-design.md | ASKED | 📐 (spec committed `8fc85f2`) |
| DEC-049 | 07-01 | **Keyword-vs-import 3-way rule**: built-in types (`int float string bool bytes decimal void never`, `List Map Set`, `T?`, fn types, ranges) are keywords NEVER imported; user/library types `import type`; stdlib functions `import Core.X` | force-import of primitives; `Integer`/`Float` wrapper objects (Java-autoboxing anti-pattern) | plans/2026-07-01-m-dogfood-benchmark-marathon.plan.md | ASKED (rejected 2 proposals) | ✅ documented (INVARIANTS) |
| DEC-285 | 07-18 | **Built-in attributes resolve in EVERY "nothing in the wind" import form** (developer-raised: `#[Core.Runtime.Entry]` errored `E-UNKNOWN-ATTRIBUTE` but should work). Recognition of the 7 built-ins (`Entry`/`Route`/`UncheckedOverflow`/`Attribute`-marker/DI `Injectable`/`Provides`/`Transient`) now suffix-matches the canonical dotted path via `ast::attr_path_matches` — so bare leaf (after member-import), any partial qualifier, AND the full canonical path all resolve; import-gating of the bare/partial forms stays with `enforce_injected` (dotted = self-gating), so the discipline is unchanged. Entry single-sourced through `is_entry_attr`→`is_entry`, Route centralized into `is_route` (3 sites). Byte-identical (verified run≡runvm≡php-8.5.8 on the qualified form). **Preferred surface stays bare-after-import** (all examples use it; FEATURES.md notes both resolve) | recognize only bare + one partial (the pre-existing gap); make bare self-gating (would break "nothing in the wind") | developer session directive 2026-07-18; tests/attribute_paths.rs | ASKED | ✅ |

## 3. Type system (M-RT) & generics

| ID | Date | Decision | Alternatives rejected | Source | Mode | Shipped |
|----|------|----------|----------------------|--------|------|---------|
| DEC-050 | 06-20 | M-RT scope = **maximal TS-grade type system** (interfaces, instanceof, unions, intersections, erased generics, inheritance, Map/Set, traits) | "coherent cluster only"; defer (Claude's recs, overruled — "put a real effort here") | plans/2026-06-20-m-rt-rich-types.plan.md | ASKED | ✅ (M-RT CLOSED 06-23) |
| DEC-051 | 06-20 | Keyword **`instanceof`** (lowercase, PHP-style); the broken `is` value-equality stub replaced by a real type test with smart-cast narrowing; `is` no longer a keyword | keep `is` as type-test keyword; Claude's dissent to RETIRE `is` entirely (recorded non-binding; dev chose Option 1) | same + plans/2026-06-20-post-wave3-four-tracks.plan.md | ASKED | ✅ |
| DEC-052 | 06-20 | Interfaces: nominal subtyping via one shared `ast::class_implements` consumed by checker+interpreter+VM; `package Main`-only that slice; exact sig match | per-backend duplication | m-rt plan (S2 design) | AUTONOMOUS (impl, inside approved slice) | ✅ |
| DEC-053 | 06-20 | Generics = **fully erased** (no monomorphization), reified-in-checker; call-site first-binding-wins `unify`; `Type::Erased` rewritten pre-backend at the `check_and_expand` chokepoint | monomorphization | m-rt plan (S7a) | ASKED (approach) / AUTONOMOUS (details) | ✅ |
| DEC-054 | 06-20 | **Generics reach = ALL** — free fns + methods + classes + (later) enums | free-functions-only | m-rt plan ("I want generics all options") | ASKED | ✅ |
| DEC-055 | 06-20 | Generic classes: inference-only construction (`Box(7)`, no `Box<int>(7)` turbofish), invariant, no bounds | explicit type-arg syntax | specs/2026-06-20-generic-types-classes-design.md | AUTONOMOUS | ✅ |
| DEC-056 | 06-20 | S4 unions: **D1 primitive members allowed** (`int\|string`); **D2 one big S4** (unions + match-over-union together); **D3 fully autonomous**; `Pattern::Type` reuses `Op::IsInstance`; lone `Circle =>` stays a catch-all binding (footgun deliberately preserved) | enum members (deferred); S4a-only split | specs/2026-06-20-s4-union-types-design.md | ASKED (D1–D3) / AUTONOMOUS (details incl. footgun) | ✅ |
| DEC-057 | 06-21 | S5 intersections: **D1 = ≤1 concrete class + N interfaces** (dev overruled Claude's interface-only rec — correctly); `E-INTERSECT-MULTI-CLASS` for ≥2 classes; **D2 = require-agreement `E-INTERSECT-SIG`** (revisit when overloading lands) | interface-only members; first-member-wins conflict rule | m-rt plan; specs/2026-06-20-s5-intersection-types-design.md | ASKED (2 challenge rounds) | ✅ ⊳ D2 revisit DONE — DEC-245 (overload-set resolution, BUILT 2026-07-16); C-8 closed |
| DEC-058 | 06-21/22 | **Method overloading confirmed** (dev explicitly rejected "stay PHP-aligned / don't add it": "this language should be equal or better than PHP"); lowers to ONE dispatching PHP method; compile-time unambiguous, most-specific-wins, `T?`≠`T` | no overloading (PHP parity) | m-rt plan; memory ga-direction-and-autonomy | ASKED | ✅ |
| DEC-059 | 06-28 | **Return-type overloading**: overloads may differ only in return type; resolved from a SHALLOW/direct sink set; `<type>f(...)` selector (distinct from `as` cast); `E-OVERLOAD-AMBIGUOUS-RETURN`/`-SELECT-CONFLICT`/`-NO-CONTEXT`; dev conceded `discard <int>f()` valid | — | plans/2026-06-28-ga-marathon-super-overloading.plan.md | ASKED | ✅ |
| DEC-060 | 06-22 | **Totality cluster**: return-on-all-paths `E-MISSING-RETURN` + `never` bottom type + `W-UNREACHABLE` + `W-MATCH-UNREACHABLE`, all front-end-only, sequenced FIRST in M-RT (before overloading) | — | specs/2026-06-22-totality-cluster-design.md; parity triage | ASKED (ordering) / AUTONOMOUS (execution) | ✅ |
| DEC-061 | 06-22 | Generic enums `enum Option<T>` / `Result<T,E>` mirroring Box machinery, zero backend change | — | plans/2026-06-22-generic-enums.plan.md | AUTONOMOUS | ✅ |
| DEC-062 | 06-22 | **S6 = multiple inheritance, Model 1 explicit-resolution** (`class C extends A, B`; cross-parent collision = compile error unless resolved); Model 3 (C3 + cooperative super) deferred to post-S8 gated milestone; `super`/`parent` under multiple parents = clean error `E-MI-SUPER-AMBIGUOUS` (forward-compat reservation) | single-`extends`-only + traits framing (dev rejected twice); C3 linearization now | specs/2026-06-22-s6-multiple-inheritance-design.md | ASKED | ✅ (reverses D-L3 — see CONFLICTS C-1) |
| DEC-063 | 06-22 | **Final-by-default + `open`** (Kotlin model); `final` keyword retired as redundant — internal consistency with immutable-by-default beats PHP-familiar open-by-default | PHP open-by-default | same spec | ASKED | ✅ |
| DEC-064 | 06-23 | **S8 traits**: reuse-only NOT a type (`use T`; `instanceof T` rejected); members carry visibility+mutability; **maximal D4** (ctors, static state, hooks, const, abstract requirements — all supported); every PHP-fatal/silent trait footgun becomes an ahead-of-time diagnostic (D5); trait-ctor shadowing warnings D6/D8 | trait-as-type; minimal trait subset | specs/2026-06-23-m-rt-s8-traits-design.md (D1–D8) | ASKED (challenge round + PHP 8.4 evidence) | ✅ |
| DEC-065 | 06-21 | **Mutation model**: immutable-by-default, keyword **`mutable`** (not `mut`); 4 orthogonal axes mutable/const/static/open; `final`/`readonly` eliminated as value modifiers; value/handle split — List/Map/Set/Bytes = deep-frozen COW values, Instance = shared-mutable handle; **no tracing GC** (Rc/Drop suffices; acyclic) | `mut`; readonly modifiers; tracing GC | specs/2026-06-21-mutation-milestone-design.md; memory mutation-milestone, ga-direction-and-autonomy | ASKED (Claude challenged, dev agreed) | ✅ |
| DEC-066 | 06-27 | **`this.field` everywhere** — bare field access is `E-BARE-FIELD` (BREAKING, PHP-faithful); `E-STATIC-THIS` in statics | implicit field resolution | memory decision-review-and-9-fixes (`53dc203`) | ASKED (decisions-review) | ✅ |
| DEC-067 | 06-21 | Visibility: public/private/protected enforced in the checker across six access surfaces; parity hole closed later with `E-FIELD-VISIBILITY`/`E-METHOD-VISIBILITY` | runtime-only enforcement | specs/2026-06-21-visibility-modifiers-design.md; plans/2026-06-25-full-bidirectional-php-support.plan.md | ASKED | ✅ |
| DEC-068 | 06-22 | **Error model = three tiers**: enforced typed `throws E` (PHP-familiar default; specific type required) + `Result<T,E>` value surface + unchecked faults/panics for bugs; `try/catch` discharges `throws` + PHP-interop bridge | Result-first-only (Claude's rec); Java checked-everything | parity SSOT §2.1; plans/2026-06-21-roadmap-completeness-review.plan.md | ASKED (dev extended the rec; reconciled via challenge) | ✅ (slice 2 closed; 3 new Ops) |
| DEC-069 | 06-28 | `super`/`parent` dispatch via `Op::CallParent`; must-use returns + `discard` contextual keyword (`E-UNUSED-VALUE`) | `void f()` C-style discard | plans/2026-06-28-ga-marathon-super-overloading.plan.md | ASKED (order + scope Option 1) | ✅ |
| DEC-070 | 06-29 | Soundness Batch B: same-head generic types made truly **invariant** at assignment (`Box<string>` rejected where `Box<int>` expected) — closing a known M-RT gap | — | CLAUDE.md; memory m-rt-progress | AUTONOMOUS (marathon) | ✅ |

## 4. Language surface & syntax (evolution decisions)

| ID | Date | Decision | Alternatives rejected | Source | Mode | Shipped |
|----|------|----------|----------------------|--------|------|---------|
| DEC-080 | 06-17 | S0 DX: `var` local inference; `type` aliases expanded out pre-backend; diagnostics with codes + `phg explain` | — | specs/2026-06-17-m3-slice1-s0-s1-s2-design.md | ASKED | ✅ |
| DEC-081 | 06-17 | S2 null-safety suite: `T?` optionals w/ compile-time non-null guarantee; `??`; `?.`; if-let + smart-cast; `opt!` + `W-FORCE-UNWRAP`; warning channel = **stderr, non-fatal, all commands** | separate `Ty::Null` variant | plans/2026-06-17-m3-s2-null-safety.md | ASKED (channel via AskUserQuestion) | ✅ |
| DEC-082 | 06-18 | S3 lambdas: expr-body infers, block-body explicit return; capture by value; pipe `x \|> f ≡ f(x)` lowered in parser (no new Op) | — | specs/2026-06-18-m3-s3-lambdas-pipe-design.md | ASKED | ✅ (later `fn`→`function`, DEC-113) |
| DEC-083 | 06-24 | **Mandatory `new` EVERYWHERE** — classes AND enum variants (`new Some(7)`); one-rule uniformity | `new` for classes only (Claude's rec, overruled) | specs/2026-06-24-mandatory-new-design.md | ASKED | ✅ (`5fb1259`) |
| DEC-084 | 06-24 | `const` class constants: literal-only v1, inherited, inlined on Rust backends → PHP typed const; SCREAMING_SNAKE; const-of-const + interface constants deferred | — | plans/2026-06-24-new-const-fieldinit.plan.md | ASKED (accepted all recs) | ✅ (`c6b1ac2`) |
| DEC-085 | 06-24 | Expression field initializers (instance + static); statics **EAGER once at program start, declaration order, before main**; may read `this` + earlier siblings (forward-ref = error); lazy `??=`-on-first-access rejected; runtime config rejected (→ DEC-006) | lazy init; runtime knob | plans/2026-06-24-language-evolution-master.plan.md; specs/2026-06-24-member-initializers-design.md | ASKED | ✅ |
| DEC-086 | 06-24 | No-value types: `void` (uncapturable) + `Empty` (holdable), `void <: Empty` | single unit type | language-evolution master plan | ASKED | ✅ then reshaped (`Empty`→`empty`, DEC-113) |
| DEC-087 | 06-24 | **UFCS general, method-first** (method → user free fn → any *imported* native by first-param unify) | rigid type→module map | plan + overnight fork F-001 | ASKED (adopt) / RATIFIED (mechanism F-001) | ✅ |
| DEC-088 | 06-24 | Return-type mandate: named fns + methods + statement-body lambdas annotated; **expression-body lambdas keep inferring** (dev's "Option 2?" instinct challenged and reversed — `=>` can't fall off the end) | annotate everything | language-evolution master plan | ASKED | ✅ |
| DEC-089 | 06-24 | Perimeter verdicts: string `+` ✅; `**` + `Math.ipow` both ✅; or-patterns instead of `switch` (reject); `s[0]` → defer M-text; single-quotes ❌; `<=>` ❌; PHP `.` concat ❌; tuples defer (classes now); let-destructuring full + `else`; fixed-length `[T; N]` adopt; `\u{}` pull forward; this-capture build; decimal/BigInt → M-NUM | — | specs/2026-06-24-language-ergonomics-perimeter-design.md | ASKED (item-by-item) | ✅ mostly (`[T;N]` see plan) |
| DEC-090 | 06-24 | **Ternary `? :` DEFERRED, not rejected** — postfix-`?` collision + third meaning of `?`; expression-`if` already covers the capability | adding it now (the same-day perimeter record said "✅ add" — superseded within the day; see CONFLICTS C-5) | language-evolution master plan | ASKED | not shipped [Verified: `? :` is a parse error in current `phg`] |
| DEC-091 | 06-24 | Literal braces: BOTH `\{`/`\}` escapes AND raw strings `r"…"`/`r#"…"#` (lexer-side interpolation split) | parser-side split (can't distinguish `\{`) | introspection-strings-process design | ASKED | ✅ |
| DEC-092 | 06-24 | Reflection: full name-level read-only introspection now (typeName/className/hierarchy/member names); dynamic-dispatch + attribute reflection rejected; **no ambient superglobals ever** (env/args → M-Batteries; request → M6 typed Request; `$_REQUEST` rejected) | deferred reflection; ambient superglobals | specs/2026-06-24-introspection-strings-process-design.md | ASKED (challenge upheld) | ✅ |
| DEC-093 | 06-25 | **A-1: `: T` return syntax; `->` fully retired**; typed lambdas TS-identical (`fn(int x): string => …`) | keep `->` | plans/2026-06-25-php-fidelity-and-divergence-audit.plan.md | ASKED | ✅ |
| DEC-094 | 06-25 | **A-6: `foreach (coll as BINDING)` adopted to REPLACE `for (x in coll)`**; one keyword `as`; 4 binding forms; optional `with int i` counter; `of`/`in` rejected as synonyms | keep `for in`; `of` keyword | same plan | ASKED | ◐ shipped **alongside** for-in, not replacing ⊳ C-2 closed by DEC-343 (2026-07-26): keep both is now the RULED state |
| DEC-095 | 06-25 | **A-3: type-first params KEEP** (`(int name)` = PHP-minus-sigil) | TS name-first `name: int` | same plan | ASKED | ✅ |
| DEC-096 | 06-25 | **A-46: `++`/`--` allowed as EXPRESSIONS** (dev overruled Claude's statement-only KEEP after full hazard briefing); eval order pinned to PHP left-to-right; `W-SEQUENCE-MUTATION` lint sweetener | statement-only | same plan; specs/2026-06-26-m3-stream1-syntax-reshape-design.md | ASKED (overruled) | ✅ *(CORRECTED per DEC-210, 2026-07-13: shipped design is STATEMENT-ONLY — `++`/`--` are NOT expressions and the `W-SEQUENCE-MUTATION` lint was never built; verified `x=i++`/`a[i++]=i++` are parse errors. The overrule to expr-form was itself reversed/never-built; ✅ tracks the statement-only outcome.)* |
| DEC-097 | 06-25/26 | Strings: two modes `"…"` (interpolating) + `r"…"` (raw); PHP `'…'` rejected; **A-62 `"""…"""` auto-dedent text blocks adopted** (Java-style trailing-strip, interpolating, purely additive); `{w}` interpolation delimiter KEEP (A-7; `${w}`/`{$w}` rejected — reintroduce the sigil) | single quotes; `${}` | same plan | ASKED | ✅ |
| DEC-098 | 06-25 | **A-61: `instanceof` stays lowercase** — universal cross-language convention beats camelCase-consistency | `instanceOf` | same plan | ASKED | ✅ |
| DEC-099 | 06-25 | Transpile fidelity: B-1 per-hole native PHP `"{$…}"` interpolation with EXHAUSTIVE hole-kind classification (dev requirement); B-2 `println` → `echo X, "\n"` (`printf` rejected — literal `%` corruption risk); B-9 minimal `$` escaping | printf; blanket concat | same plan | ASKED | ✅ |
| DEC-100 | 06-26 | **Keep `var`, make it CONTEXTUAL** — all four declaration forms stay; the real bug was hard-reservation, not the spelling (supersedes the same-day "retire `var`" agreement after research on Hack/Haxe + philosophy re-read) | retire `var`; `let`=immutable; keyless synthesis; Go `:=` | plans/2026-06-26-retire-var-declaration-reshape.plan.md (two logs) | ASKED (reversal recorded) | ✅ |
| DEC-101 | 06-26 | Default parameters: `param: T = <literal>`, trailing-only, literal-only, front-end call-fill (no backend change) | — | plans/2026-06-26-default-parameters.plan.md | ASKED | ✅ |
| DEC-102 | 06-26 | Idea-backlog batch 1: no top-level execution in project files (A); optional `main(args: List<string>): int`, no `argc` (B); `handle(Request) -> Response` reserved web entry (C); **`length` for ordered / `size` for keyed** collections, hard rename no alias (D) | PHP-style top-level code; `argc` | plans/2026-06-26-developer-idea-backlog.plan.md | ASKED | ✅ |
| DEC-103 | 06-27 | **Class entry points: BOTH forms allowed** — top-level `main`/`handle` OR `static` class method (dev overruled Claude's "top-level only, Java-ism" challenge); `E-MULTIPLE-MAIN` on ambiguity | top-level only | specs/2026-06-27-class-entry-points-design.md | ASKED (overruled) | ✅ |
| DEC-104 | 06-27 | `as` operator → checked cast to primitives (`value as Type` ⇒ `Type?`); `as` contextual; casting system = mix (Core.Convert + `as` + UFCS), TS `<X>` assertion axis separated from value conversion | C-style `(int)x` cast (the PHP surprise) | plans/2026-06-26-m4-stdlib-breadth.plan.md; memory as-primitives-and-crypto-session | ASKED (spec-first) | ✅ |
| DEC-105 | 06-30 | B1 iteration protocol: for-in over string/Map (two-binding) + `List.enumerate`; `zip` deferred to B3 | — | memory session-naming-and-b1 | AUTONOMOUS (within approved marathon) | ✅ |
| DEC-106 | 07-01 | Dogfood W0/W2: empty-list literal init; comma-throws; nested-quote interpolation; list upcast | — | memory marathon-m-dogfood | AUTONOMOUS | ✅ |
| DEC-107 | 07-01 | **Q1 dynamic dispatch: NO string-instantiate/string-call primitive** (un-typeable/un-erasable); ADD method-references-as-values (`obj.method` → typed closure) + typed-registry guide | PHP `new $class`/`$obj->$m()` | no-wind spec §context; four-lane plan | ASKED | 📐 |
| DEC-288b | 07-18 | **Tuple REPRESENTATION = compile-time sugar ERASED to List** (dev-ruled follow-up): `(a,b)` desugars to a `List`, `(A,B)` type-checked then erased, destructuring → indexing — Invariant-5 "expand out before backends" (like generics/type-aliases). NO value-model / backend / kernel change → targeted-validatable on the current box. Trade-off (accepted): a tuple is NOT a distinct runtime type — it prints/behaves as a list. Multi-slice: parser (type + literal + destructuring-pattern positions) → checker (tuple types + destructuring bind) → desugar chokepoint (`cli::check_and_expand`) → then zip/partition/Map.entries on top | real `Value::Tuple` (spine-critical, needs the gate-heavy full-validation this box SIGKILLs); defer | dev AskUserQuestion 2026-07-18 | ASKED | 📐 (ruled, in progress) |
| DEC-288 | 07-18 | **Built-in tuple type `(A, B[, …])`** (dev-ruled, parity push) — a lightweight structural tuple + destructuring (`for ((int i, string s) in ps)`, `(a, b) = pair`), transpiling to PHP 2+-element arrays. Unblocks `List.zip -> List<(A,B)>`, `List.partition -> (List<T>, List<T>)`, `Map.entries -> List<(K,V)>`, and general multi-value return. FOUNDATIONAL language feature: parser + type-system + destructuring patterns + all 3 backends + transpile — the biggest slice of this batch; spine-critical, advisor-certified, likely multi-slice | `Core.Pair<A,B>` std class (clunkier, no destructuring); reuse `List<List<T>>` (untyped, same-type only); skip zip/partition | dev AskUserQuestion 2026-07-18 | ASKED | 📐 (ruled, not built) |

## 5. Naming & renames

| ID | Date | Decision | Alternatives rejected | Source | Mode | Shipped |
|----|------|----------|----------------------|--------|------|---------|
| DEC-110 | 06-20 | Stdlib API camelCase fn names (`split_once`→`splitOnce` etc.) with the casing hard-error slice | snake_case | post-wave3 plan | ASKED | ✅ |
| DEC-111 | 06-26 | Core.Json enum variants PHP-reserved-name mangling in transpiler only (`Int`→`Int_`…), API stays clean | `J`-prefixed API | plans/2026-06-26-autonomous-backlog.plan.md | ASKED | ✅ |
| DEC-112 | 06-29 | `Channel.new()` → `Channel.create()` (`new` became a keyword token); `Task`/`Channel` reserved forcing example `class Task`→`Parcel` rename | — | big-marathon plan | AUTONOMOUS (forced) | ✅ |
| DEC-113 | 06-30 | **Full naming overhaul (clarity / no-shortcut)**: lambda `fn`→**`function`**; `Empty`→lowercase **`empty`** (union-able; `void` rejected in unions → `E-VOID-IN-UNION`); Result `Ok`/`Err`→**`Success`/`Failure`**; `recv`→`receive`; CLI `fmt`→`format`, `bench`→`benchmark`, `disasm`→`disassemble`, `lex`→`tokenize`; packages `Console`→**`Output`**, `Text`→**`String`**, `Validate`→`Validation`, `Convert`→`Conversion`, `Reflect`→`Reflection`, `Crypto`→`Cryptography`; new `Core.Environment`; ~20 native renames (println→printLine, upper→uppercase, div→divide, args→arguments, next→nextInt, millis→milliseconds, url-encode family…); KEPT: math notation, acronyms, `of` factories, Task/Channel (Thread & Observable rejected) | Thread/Observable; Unit; Console/Out | specs/2026-06-30-naming-overhaul-design.md; memory naming-overhaul-decisions | ASKED (exhaustive review) | ✅ (unpushed); Lane-1 leftovers done 07-01 |
| DEC-114 | 06-28 | Name **Phorj** locked (see DEC-013) — this row records that the *prior* 06-21 decision was "keep Phorge for now, rename before GA" (superseded) | rename immediately (06-21); Phurnace | memory name-collision-rename-decision | ASKED | ⬆→✅ |
| DEC-284 | 07-17 | **Extension names track their real module name** (DEC-273 hygiene): Cargo feature + registry `name`/`feature` `crypto`→**`cryptography`** (module was already `Core.Cryptography` since DEC-113), `db`→**`database`** (module `Core.DatabaseModule`), `db-postgres`→**`database-postgres`**, `db-mysql`→**`database-mysql`**, `db-all`→**`database-all`**. Atomic flip of every `cfg(feature=…)` (the `unexpected_cfgs` deny-lint guarantees no silent compile-out); registry rows, summaries, generated `docs/EXTENSIONS.md`, and the SPEC/FEATURES flag references all updated in the same change | keep short flag names (mismatch module) | developer directive ("extensions names needs to reflect their real module name") | ASKED | ✅ |

## 6. Runtime, VM, performance

| ID | Date | Decision | Alternatives rejected | Source | Mode | Shipped |
|----|------|----------|----------------------|--------|------|---------|
| DEC-120 | 06-16 | P4 object model **A — value-native** (reuse shared `Value::Instance`/`Enum`, clone-on-use); arena/handle deferred bench-gated | arena/handle model | plans/2026-06-16-m2-p4-classes-enums-match.md | ASKED | ✅ then evolved (P5a) |
| DEC-121 | 06-16 | P5a: `Rc`-share Instance/Enum/List (2.4×); **Phase B slot-indexed layout bench-gated, unopened**; slab-arena rejected (no locality evidence) | slab arena | plans/2026-06-16-m2-p5a-rc-shared-heap.md | ASKED | ✅ (slot-indexed later shipped in 06-28 marathon when evidence arrived) |
| DEC-122 | 06-16 | Wave 4 before P5 (correctness gap outranks bench-gated perf); class-aware `CTy` derived structurally from AST annotations | threading checker `Ty` into compiler | plans/2026-06-16-m2-wave4-compiler-types.md | ASKED | ✅ |
| DEC-123 | 06-17 | No tracing GC in M2 — Rc/Drop reclaims the immutable+acyclic heap fully; tracing deferred to a mutation milestone (then permanently mooted by COW value semantics) | mark-sweep GC (original M2 criterion, revised) | CLAUDE.md; memory mutation-milestone | ASKED | ✅ |
| DEC-124 | 06-18 | `Op` discipline: any new Op extends exactly three coupled matches (`exec_op`/`validate`/`stack_effect`) same commit; "no new Op" default for front-end features | — | docs/INVARIANTS.md; memory op-variant-match-coupling | ASKED (standing) | ✅ |
| DEC-125 | 06-20 | Higher-order natives = **`NativeEval` enum (Pure \| HigherOrder)** + backend-supplied closure invoker; VM gains re-entrant `run_until`/`call_closure_value`; no new Op | backend intrinsics; dedicated Ops | m-rt plan; memory higher-order-natives-reentrant-vm | ASKED | ✅ (later + `Reflective`) |
| DEC-126 | 06-20 | S3 Maps: insertion-ordered `Rc<Vec<(HKey,Value)>>`; `Op::MakeMap` + runtime-polymorphic `Op::Index` (no `IndexMap`); Set folded into generics slice (not shipped thin) | HashMap rep; separate IndexMap op; thin Set now | m-rt plan | ASKED (full gates for this slice) | ✅ |
| DEC-127 | 06-29/07-01 | Perf wins: FNV-1a string hashing; slot-indexed fields S1a/S1b + VM inline cache; COW index-assign in place (`Op::SetIndexLocal`, O(n²)→O(1)); reified-operand side-table | — | memory m4-text-and-mperf-fnv, marathon-perf-mustuse-superparent, cow-index-assign-inplace | AUTONOMOUS (marathons) | ✅ |
| DEC-128 | 07-01 | M-perf W2 (Rc-share `Value::Str`) DEFERRED — 164 call sites, ROI not demonstrated; CI perf-regression gate shipped instead (`scripts/perf-gate.sh`, ratio + best-of-N) | do the Str sharing now | memory session-2026-07-01-lane1-perfgate | AUTONOMOUS | ✅ gate / 📐 W2 |
| DEC-129 | 07-01 | M-DX build profiles Dev/Release **side-channels only** — byte-identical run≡runvm≡PHP preserved (the "keystone"); interpreter-only debugger (REPL + DAP) | profile-dependent semantics | plans/2026-07-01-m-dx-error-experience.plan.md; memory m-dx-error-experience | ASKED (milestone) / AUTONOMOUS (slices) | ✅ (unpushed) |
| DEC-286 | 07-18 | **`EnumVal.payload` inline (`Payload { Zero, One(Value), Many(Vec) }`)** — every 0/1-payload enum node (all Json variants, Option/Result, the common user variant) now stores its payload INLINE, paying no per-node heap `Vec`; only 2+-field variants keep a `Vec`. Byte-identical (2279 tests + differential + php-8.5.8 oracle + all-micro output-identity); microbench-gate PASS, no WIN→LOSS flip, `enum`/`match` benches improved. A broad alloc reduction across the whole value model, single-sourced via `Payload::as_slice`. **PENDING (developer review — the "what's blocking jsonround" answer):** jsonround stays **0.29× (LOSS)** — VM 507ms vs C-`json` 145ms, a 3.4× gap. TWO byte-identical levers tried (DEC byte-cursor parse + this inline-payload) bought only ~3% because ~65% of the ~20 allocs/iter is the `Rc<EnumVal>` BOX ITSELF (one per node), the boxed-enum value model PHP's C zval-array beats structurally. Flipping jsonround needs a **value-model rebuild (arena / lazy-materialize Json nodes)** — a spine-deep architectural change to the user-visible, pattern-matched `Json` enum, possibly still short of C; **Invariant-15 developer decision, NOT autonomously attempted.** The two byte-identical wins are banked regardless ⊳ SUPERSEDED by DEC-380 (2026-07-26): CHASE THE WIN — research slice queued | keep the per-node `Vec` (wasteful); autonomously attempt the arena rebuild (Invariant-15 violation + spine risk) | developer all-night session 2026-07-18; measured `phg benchmark`/microbench pinned | AUTONOMOUS (byte-identical perf) + PENDING (arena = ASK) |

## 7. Concurrency (M6 W4 / green threads)

| ID | Date | Decision | Alternatives rejected | Source | Mode | Shipped |
|----|------|----------|----------------------|--------|------|---------|
| DEC-130 | 06-18 | **Single-threaded FORCED** by the Rc-shared heap (`Value` is `!Send`); OS-thread pools "off the table"; real concurrency = green threads later | OS threads | specs/2026-06-18-m6-web-design.md | ASKED (design-locked) | ✅ then ◐ (W3 shipped an OS-thread serve pool — see CONFLICTS C-6) |
| DEC-131 | 06-26 | Concurrency admission: cooperative async + pure data-parallelism + reactive over deterministic sources + a Tier-B live escape; **shared-state OS threads = HARD NO**; suspension-free subset first (D-Async-1) | shared-memory threading | native-modules-extended-scope plan | ASKED | ◐ (green threads shipped; parallel/reactive 📐) |
| DEC-132 | 06-29 | Green threads = **uniform stackful coroutines on BOTH backends + single-sourced deterministic scheduler kernel** (`green::sched`); dev chose Option A over Claude's VM-frame-swap simplification; corosensei admitted (4th dep) after a no-unsafe spike; wasm keeps eager (corosensei won't compile there) | VM frame-swap hybrid (B); OS-thread-per-task (Value !Send); literal "1+3" mix (rejected incoherent) | specs/2026-06-29-m6-w4-green-threads-design.md §4; big-marathon plan | ASKED | ✅ (A1 cutover complete, unpushed) |
| DEC-133 | 06-29 | Concurrency **quarantined from the PHP oracle** (`E-CONCURRENCY-NO-PHP` + harness skip) — transpile→sync-PHP rejected as spine-breaking; the spawn/channels example ships with no PHP equivalent (accepted exception to the 3-leg rule) | sync-PHP emission | big-marathon plan | ASKED | ✅ |
| DEC-134 | 06-29 | Interim step 2 shipped **synchronous-degenerate** (spawn eager, recv-on-empty faults) with 5 new Ops; developer then demanded the real cooperative cutover FIRST (litmus: `spawn consume(ch); send(42)` must not fault) | leaving eager semantics | big-marathon plan; memory session-playground-fix-and-cutover-foundation | ASKED | ⬆→✅ (A1 cutover) |
| DEC-135 | 07-01 | Real parallelism **ON HOLD** — models table (async-reactor / actor / data-parallel / shared-memory) recorded; actor model = best structural fit; deep M-Parallel plan delegated | committing to a model now | no-wind spec §5 | ASKED | 📐 |

## 8. Web (M6), stdlib & natives

| ID | Date | Decision | Alternatives rejected | Source | Mode | Shipped |
|----|------|----------|----------------------|--------|------|---------|
| DEC-140 | 06-18 | Portable web unit = **`handle(Request) -> Response` at the VALUE level** (PSR-7/15 insight); socket/superglobal bridge is runtime glue, never transpiled 1:1 | raw-bytes handler | m6-web-capabilities-research plan (3C 8/8) | ASKED | ✅ |
| DEC-141 | 06-18 | **Shape A** (pure-Phorj Request/Response classes) is the ONE public API; native header map = later invisible optimization | Shape B native map as 2nd API ("do both" resolved to one) | same + specs/2026-06-18-m6-w1-handler-design.md | ASKED | ✅ |
| DEC-142 | 06-18 | `bytes` primitive pulled forward as its own W0 slice; Transport trait seam quarantines the socket (`src/serve.rs`, tested outside differential.rs) | UTF-8-text-only v1 | m6 research plan §11 | ASKED | ✅ |
| DEC-143 | 06-18 | **URL/network deferred to M6** — determinism (not the dependency) gates examples; rich std-only stdlib NOW (L-2) | HTTP client via crate now | specs/2026-06-18-m3-next-intuitive-features-and-io-design.md (L-2) | ASKED (heard full challenge) | ✅ |
| DEC-144 | 06-18 | Wave-2 buildable subset only (`core.math`/`text`/`file`); `core.list`+`core.json` DEFERRED until generics/lambdas exist | force-typing with concrete sigs | trackB plan | ASKED | ✅ (both later shipped) |
| DEC-145 | 06-26 | Core.Json: number model `Int(int) + Float(float)` (PHP-faithful); `stringify` + `stringifyPretty` both; sealed `Json` ADT + explicit `mixed` escape hatch | `Num(float)`; J-prefix API | autonomous-backlog plan; ga-direction memory | ASKED | ✅ |
| DEC-146 | 06-26 | M4 sort API = `sort` + `sortWith` (mirrors PHP sort/usort); strings compare via strcmp (byte-lexicographic) never PHP numeric-string juggling; stable; returns NEW list | locale/numeric-string compare | m4-stdlib-breadth plan | ASKED | ✅ |
| DEC-147 | 06-26 | **M-NUM decimal**: primitive `decimal` (i128 fixed-point `{unscaled, scale}`); literal `1.50d`; transpile target **BCMath** (corrects the SSOT's brick/math — composer pkg can't load under `php -n`); bare `decimal/decimal` = `E-DECIMAL-DIV` → `Decimal.div(a,b,scale,mode)`; 7-mode RoundingMode; overflow = clean fault; arbitrary precision → M-NUM-2 | stdlib class; brick/math; silent division | m-num plan + specs (LOCKED) | ASKED | ✅ (later refined DEC-148) |
| DEC-148 | 06-27 | Decimal refinements from decisions-review: `%` exact remainder (dev caught mis-lumping with `/`); bare `/` = exact-or-fault (`FaultKind::DecimalInexact`); division by zero ALWAYS faults (incl. float — IEEE inf/NaN removed); numberFormat digit-string rounding | keeping E-DECIMAL-DIV for both ops; IEEE semantics | memory decision-review-and-9-fixes | ASKED | ✅ (pushed) |
| DEC-149 | 06-26 | NaN/Infinity are `Core.Math` **functions**, not keywords/literals; `Convert.toInt(float) -> int?` null on NaN/Inf/overflow (fixes PHP `(int)` quirk); conversions live in `Core.Convert` | keywords; PHP cast semantics | m-num plan (S3) | ASKED | ✅ |
| DEC-150 | 06-26/27 | `Core.Random` = seeded sub-2^63 shift-add PRNG, **pure:true** — transpiler hand-rolls identical xorshift in PHP (masked `>>`), never `mt_rand`; byte-identical across 3 legs | mt_rand mapping; quarantine | extended-scope plan (D-PRNG); decision-review memory | ASKED | ✅ |
| DEC-151 | 06-27 | **Core.Crypto = Argon2id via the first external dependency** (RustCrypto, audited) — rolling your own is the security anti-pattern; PHP-only delegation rejected (DEC-005) | hand-rolled; PHP-delegated | memory as-primitives-and-crypto-session; Cargo.toml | ASKED | ✅ |
| DEC-152 | 06-27 | Core.Http API = **Option 1: static/instance methods on injected types** (`Request.parse`, `resp.serialize()`, `Response.text`) — namespace-clean, pure Phorj | free functions; native impl | big-chunk plan | ASKED | ✅ |
| DEC-153 | 06-27 | M4 **stdlib charter FIRST** before any new stdlib surface (naming/shape conventions govern all future modules) — reorder over M-Test-first | mint modules then charter | ga-sequence plan | ASKED | ✅ |
| DEC-154 | 06-28 | Router: `Core.Http` Router + `#[Route]` attributes + middleware | — | specs/2026-06-28-m6-w2-router-attributes-design.md | ASKED | ✅ |
| DEC-155 | 06-21 | Stack traces identical across backends (interpreter gains a logical call-stack mirroring VM frames); traces on stderr only (FaultKind spine untouched); CLI + dev-mode web error page; **prod = bare 500, never leaks trace/source** | VM-only traces | error-handling plan | ASKED | ✅ |
| DEC-156 | 07-01 | Manual benchmarking (`Core.Runtime.memoryBytes`/Stopwatch) legal but **quarantined from the byte-identity example set** (`pure:false` model) | blocking manual timing entirely | m-dogfood plan | ASKED | ✅ |
| DEC-289 | 07-18 | **`List.groupBy(List<T>, (T) -> K) -> Map<K, List<T>>`** (dev-ruled, parity push) — the universal groupBy shape (Kotlin/Swift/LINQ): `[1,2,3,4].groupBy(n => n%2)` = `{1:[1,3], 0:[2,4]}`. First-seen key order; higher-order-native recipe + gated `__phorj_group_by` helper. Self-certifiable (follows the established recipe) | fold-into-Map manually (skip) | dev AskUserQuestion 2026-07-18 | ASKED | 📐 (ruled, next up) |
| DEC-290 | 07-18 | **Native `Core.DateTime` → PHP `DateTimeImmutable`** (dev-ruled, TOP-20 parity) — construct/parse/format/add-subtract/compare. Parse/format/arithmetic/compare are DETERMINISTIC → byte-identity-gated (DateTimeImmutable + a fixed format map); only `now()` is impure (freezable clock, like Core.Time → quarantined). Supersedes DEC-206's bare-name gate direction (calendar IS built, not lib). Larger slice (new native module) | userland calendar lib on Core.Time; defer | dev AskUserQuestion 2026-07-18 | ASKED | 📐 (ruled) |
| DEC-291 | 07-18 | **`Core.Fs` breadth** (dev-ruled, TOP-20 #5) — add copy/move(rename)/delete(unlink)/mkdir/exists/isFile/isDir/size/mtime/glob/tempfile mapping to the PHP builtins; impure + spine-quarantined like the existing Core.Fs. Mostly mechanical (impure-native recipe); self-certifiable | minimal set only; defer | dev AskUserQuestion 2026-07-18 | ASKED | 📐 (ruled) |

## 9. Tooling, build, distribution, interop

| ID | Date | Decision | Alternatives rejected | Source | Mode | Shipped |
|----|------|----------|----------------------|--------|------|---------|
| DEC-160 | 06-16 | `phg build`: embed program SOURCE in a versioned CRC-guarded `.phorj` ELF/PE/Mach-O section; stub = the running phg binary; cross-compile via cargo-zigbuild; apple `--target` rejected (Phase 3) | bytecode embedding | m2.5 plans/specs | ASKED | ✅ (vendor-merge + Phase 3 deferred) |
| DEC-161 | 06-17 | Profiling lives in `bench` (one timing surface); memory measured COLD (warm glibc reads ~0); `phg disasm` ships | separate `--profile` flag | v0.4.0 plan | ASKED | ✅ |
| DEC-162 | 06-19 | GA road M7→M12; keep 3-backend model + Op descriptor table; **shared-IR rewrite deferred**; M7 correctness closure non-negotiably first; runtime PHP helpers (`__phorj_div`/`__phorj_rem`/`__phorj_str`) over a static transpiler type resolver | shared IR now; transpiler-side type resolver | ga-roadmap plan | ASKED | ✅ |
| DEC-163 | 06-23/24 | PHP transpile floor raised 8.4 → **8.5**; CI pins 8.5 + non-gating 8.6-dev canary; version *targeting* (`--php-target`) = separate post-S8 milestone | float to 8.6 | memory php-transpile-floor-84; traits spec version note | ASKED | ✅ |
| DEC-164 | 06-24 | **WASM playground**: Cargo workspace, isolated `playground/` crate (core stays dep-free); full 3-way with php-wasm from day one; CodeMirror 6; GitHub Pages | core-crate wasm deps | playground plan | ASKED | ✅ |
| DEC-165 | 06-25 | Transpile modernization Track 1 before M-Lift (native match/ternary/clone/`??throw` emission; `OpKind` operand resolver; `__phorj_float` Ryū helper irreducible) | lift first | transpile-modernization plan | ASKED | ✅ |
| DEC-166 | 06-25 | **M-Lift (PHP→Phorj)**: staged Tier-1 → Tier-2 (round-trip-gated) → **Tier-3 best-effort with loud `// LIFTED TIER-3 (unsafe — verify)`**; hard-untranslatable core (`eval`, `$$x`, runtime magic, dynamic class names) always `// CANNOT LIFT`, never guesses | demo-only reach; original blanket "refuse Tier-3" (superseded for the attemptable subset) | full-bidirectional + m-lift plans | ASKED | ✅ |
| DEC-167 | 06-25 | Lift verdicts: C-1 interpolation faithful-subset (silent wrong guess worse than loud rejection); C-45 void-or-reject; C-5/6 precedence-aware printer; C-46/47 instanceof + bitwise coverage | "try everything" | php-fidelity plan | ASKED | ✅ |
| DEC-168 | 06-27+ | LSP: ONE server reusing the checker `Diagnostic` surface + thin VSCode/JetBrains clients; cross-file support in the 06-28 marathon | per-editor logic | memory ide-tooling-extensions | ASKED | ✅ |
| DEC-169 | 06-27/28 | M-Test: `phg test` + `Core.Test` + `test"…"{}` blocks; seeded Faker; Reflect-based auto-mocker (full suite chosen) | minimal assertions-only | extended-scope plan; memory m-test-milestone | ASKED | ✅ |
| DEC-170 | 06-28 | `phg fmt`: canonical-form, comment-preserving (side-channel), meaning-preserving printer | reflow/opinionated formatter | memory phg-fmt-milestone | ASKED | ✅ (CLI now `format`) |
| DEC-171 | 06-28 | M8.5 interop: `declare` blocks / `.d.phg`-style typed PHP bindings | — | memory m8.5-interop-declare | ASKED | ✅ |
| DEC-172 | 06-27 | M2.5 Phase 3a stub registry: SHA-256 + manifest + verify-before-cache; 3b (`--sign`) deferred/parked | — | memory m25-phase3a-stub-registry | ASKED | ✅ 3a / 📐 3b |
| DEC-173 | 06-23 | M-Decomp: whale files split into `foo/mod.rs` cohesion clusters, byte-identity-gated; **HYBRID by-phase backbone + selective thin-dispatcher** (pure by-construct rejected) | by-construct split | specs/2026-06-23-decomposition-milestone-design.md | ASKED | ✅ |
| DEC-174 | 06-27 | `git push` NEVER autonomous (standing, survives every bypass); `git add`/`commit` autonomous when green (project override of global Rule 10, authorized 06-16) | — | CLAUDE.md | ASKED | ✅ standing |
| DEC-175 | 07-01 | Post-M-DX order: **Naming → M-perf → VM-debug-symbols → Stdlib-breadth**; + 5 folded ADD candidates (`phg repl`, `phg doc`, parser multi-error recovery, A2 generators, opportunistic wins) | other orders | four-lane plan | ASKED | in progress |
| DEC-176 | 07-01 | Post-dogfood: clarity workstream = **blanket `clippy::pedantic`, fix ALL** (dev overrode "selective lints only" rec) | selective lints | post-dogfood plan | ASKED (overruled) | in progress |
| DEC-287 | 07-18 | **dbwork perf arc → AT PARITY (0.64×→~0.98× vs C PDO-sqlite)** + two OPERATIONAL notes for review (dev-requested "log inconvenient things"). PERF: 3 byte-identical levers — `prepare_cached` (rusqlite LRU stmt cache, 0.64→0.85; PDO doesn't cache), chainable `bind` returns `this` not `new Statement` (0.85→~0.95), `DbStmt.sql` String→PhStr (0.95→~0.98). Residual <1% = the per-op catchable-`DatabaseError` enum (semantically required — NOT a lever). Per the MATCH-not-beat-on-C mandate this is success; NOT claimed a >1.0 WIN (reads 0.96–0.98 under load; microbench baseline stays 0.63 until a quiet-box `--emit` re-baseline, OWED). OPS-1: **heavy full-tree cargo runs (`nextest --all-features`, `clippy --all-targets`) get SIGKILLed on this box** (load ~8, 2 terminal deaths); worked around with targeted `-E 'binary(...)'` tests + `NEXTEST_TEST_THREADS=4` + `clippy --lib` = [[heavy-cargo-runs-killed-on-this-box]]. OPS-2 (⚠ VALIDATION SCOPE): the 3 dbwork commits (`a90c4f8c`/`80e5d9b3`/`e8dd5dd3`) were validated by TARGETED db tests + the pre-commit fast tier only — the full `--all-features` suite + the two heavy pre-push sweeps (incl. `shipped_manual_example_runs_on_both_backends`, which runs `examples/database/*` on both backends) have NOT run on final HEAD since gate4 (predates all three). Isolated db-gated code, low risk, but the dev's first `pre-push` is the first FULL validation | chase the last 2% (noise + required semantic); claim a flipped WIN on loaded reads | dev all-night session 2026-07-18; advisor-certified | AUTONOMOUS (byte-identical perf) + OPS notes for review |
| DEC-292 | 07-18 | **dbwork FLIPPED to WIN** (extends DEC-287: 0.95× LOSS → ~1.0–1.06× WIN vs C PDO+SQLite; commit `a13d845f`). Two further Phorj-side dispatch levers, both user-invisible (DatabaseResult is a prelude-local carrier; DB is native-only, spine-quarantined): (1) **cached unit success carrier** — bind/bindNamed/bindList/begin/commit/rollback/timeout/onQuery discard the Ok payload (`Ok(_) => this` / `Database.ok()`), so they return one thread-local `DatabaseResult.Ok(null)` (Rc bump via `wrap_unit`) instead of allocating; the bind inners return `Value::Null` not a handle clone (bind runs ~40k×/run). (2) **`with_hook` fast-path** — skip the two `Instant::now()` clock reads + take/restore plumbing when no `onQuery` hook AND no timeout (the common case, ~20k execs/queries/run). Measured docker php:8.5+JIT interleaved core-pinned K=9: 2/3 runs WIN (1.02/1.06×), one 0.95 VM-side noise spike. 160 db/hook/timeout/transaction tests green; `db_runtime_round_trip` updated (chain via same handle, not the native return). ✅ **VERDICT CONFIRMED — [Verified: quiet-box (load 1–3) K=9 × 3 runs → 1.00× / 1.04× / 1.06×, all WIN, 2026-07-18]**. The earlier 0.95 was a load-contaminated VM spike; on a quiet box dbwork is a stable ~1.04× median WIN vs C PDO+SQLite. `--emit` baseline refresh still owed (needs a quiet-box window). The code wins (fewer allocs + clock reads) are instruction-count real | leave at parity; chase the prepare Statement-instance alloc (diminishing) | dev all-night session 2026-07-18; advisor-certified structural read | AUTONOMOUS (byte-identical perf) |
| DEC-293 | 07-18 | **jsonround within-representation perf wins** (0.28× → 0.32×; commit `b4cd85a6`; kept a LOSS by the structural floor — see DEC-294). Byte-identical: (1) **intern the immutable scalar Json nodes** — `Json.Null`, `Json.Bool(true/false)`, small `Json.Int(n)` in [-16,256] are cached thread-locals; `parse` clones a cached node (Rc bump) not a fresh `Rc<EnumVal>` (the Json ADT is immutable — match/encode/eq_val read ty+variant+payload content, never node identity). (2) **alloc-free encode** — `write!` ints/floats straight into the buffer (no throwaway `to_string()`/`format!`); stringify output pre-sized. Byte-identity verified (jsonround checksum + examples/guide/json.phg md5 identical run vs run --tree-walker) | change the Json value representation to win (that's DEC-294) | dev all-night session 2026-07-18 | AUTONOMOUS (byte-identical perf) |
| DEC-294 | 07-18 | **jsonround is a STRUCTURAL loss AND the lazy/compact-Json flip is GREENLIT** (dev ruled BOTH options via AskUserQuestion: "document + do more effort to flip correctly"). ARITHMETIC (why optimization alone cannot flip it): Phorj's `Json` is a typed ADT — one `Rc<EnumVal>` per JSON node; PHP's `json_decode($doc,true)` yields a bare nested array with ZERO per-node objects. Measured parse ALONE = 205ms (allocation-bound: proven proportional to node count — 1-node doc 74ms, 9-node 205ms) which already EXCEEDS PHP's entire round-trip (153ms), so even zeroing match+build+stringify leaves parse > PHP. THE FLIP PATH = a **lazy/compact Json value** that materializes ADT nodes only when a `match` deconstructs them (parse yields a cheap compact form; unread fields — `name`/`tags`/`price` in jsonround — never allocate). SCOPE (mapped, spine-sensitive — a FRESH-context slice per the standing rule, NOT a session-tail rush; byte-identity is invariant #1): a lazy node must materialize-on-deconstruct at ~15 runtime touch-points — `match_pattern` (interpreter/kernels.rs:171), VM `MatchTag`+`GetEnumField`+`MakeEnum` (vm/exec.rs:614/621/603), `encode`/`encode_pretty` (ext/json), `eq_val` (value/core_impl.rs:112), and the inspect/reflect/dispatch/type_name readers; a new `Value` variant would ripple every `Value` match. `Map.get`/indexing is transparent (clones the child out, never inspects) so a lazy Object's Map children flow through untouched — materialization pressure is only at the deconstruction sites. ⚠ **IMPLEMENTATION-LEVEL FINDING (2026-07-18, ruled "start now" → explored → reverted)**: laziness does NOT help jsonround's *doc shape*, on three grounds discovered by building the design out: (1) **null-on-malformed semantics** force `Json.parse` to still VALIDATE the whole doc upfront (a byte-identical alloc-free skip-scanner — parsing isn't saved, only allocation deferred), which is itself a doubled-parser byte-identity risk; (2) **materializing a container re-scans its bytes** (double-scan of the accessed part) and allocates a Map + key strings + **one `Rc` lazy-child placeholder per element** (same alloc count as eager children) — so only DEEP UNREAD subtrees are saved; (3) jsonround's doc is **shallow** (`{id,qty,name,tags,price}`) and reads 2 of 5 top fields, so placeholder allocs + double-scan roughly offset the deferred `name`/`tags`/`price` savings (net ~27% fewer allocs at best, likely eaten by the re-scan). Lazy-Json wins for **deep/wide docs with few fields read**, NOT this bench's shape. The exploratory `Value::JsonLazy` variant (ripple gauged: only 8 compile sites + ~6 silent materialize sites — smaller than feared) was REVERTED (tree green). CONCLUSION: **jsonround stays a documented FLAG** (DEC-293 within-rep wins are the ceiling for this shape); lazy-Json remains a valid FUTURE feature for real deep-JSON workloads (macro benches), just not a jsonround flip. NEXT (if pursued): a deep/wide-doc macro bench to justify it, built fresh under DEC-268 | (A) accept documented FLAG [now RECOMMENDED — shape analysis]; build lazy-Json anyway for deep-doc workloads [valid, separate justification] | ✅ **BUILT (2026-07-18, dev re-ruled "build lazy-Json for a NEW deep-doc bench")**: shipped `Value::JsonLazy(Rc<LazyJson{src,start,cached}>)` + `bench/micro/deepjson.{phg,php}` (a paginated user-list response, 12 six-field records, handler reads ~4 fields). `Json.parse` now validates the whole doc (alloc-free `validate_json` skip-scan — null-on-malformed preserved) then returns a top lazy node; nodes materialize ONE LEVEL on deconstruction (memoized via `OnceCell` shared through the `Rc`; VM `MatchTag`+`GetEnumField` reloads hit the cache). Deconstruction sites handled: interp `match_pattern`, VM `MatchTag`/`GetEnumField`, `encode`/`encode_pretty`, `eq_val`, `type_name`, `reflect` kind/typeName, `inspect`, `Debug.dump`, native param dispatch. Gated on `json` (value-level `materialize_if_lazy` shim = identity when off). Byte-identity VERIFIED (jsonround 7800000, deepjson 1300000, examples/guide/json.phg md5 — all identical run/tree-walker/PHP); full `--all-features` 2279 green; `--no-default-features`+clippy clean; corpus guard `lazy_matches_eager_on_corpus` (18 valid deep-materialize-equal + 24 adversarial acceptance-equal) protects the `.expect` against skip-vs-build divergence. Commits `1e5d1498`+`50df9333`. PERF [Verified: quiet-box (load 1–3) K=9 × 3 → deepjson 0.94×/0.94×/0.96× stable, 2026-07-18]: eager 0.57× → lazy **~0.95×** (VM 1362→~820ms, ~40% faster) — a large, STABLE improvement that **does NOT flip** (confirmed on a quiet box, not noise). Honest verdict: lazy-Json **MATCHES-not-beats** C-native `json_decode` on this deep/wide shape (within ~5%, while producing a richer typed ADT) — a legitimate outcome under the perf mandate. Residual PhStr-copy lever taken (DEC-294 residual, `3cd28e4c`, <1%). jsonround unchanged 0.30× (shape-immune, stays FLAG). `--emit` baseline (dbwork WIN + deepjson row) still owed a quiet-box window. KNOWN RESIDUAL (look here first if the idle-box run lands <1.0): `Json.parse` now does one `Rc::from(s.as_str())` full-doc copy per call (the `Rc<str>` backing the lazy nodes) — cheap vs the tree it replaces, but an allocation proportional to doc size the eager path didn't pay; reusing the input `PhStr`'s own `Rc` (instead of copying) would shave it | (A) accept jsonround FLAG; build lazy for deep-doc [DONE] | dev AskUserQuestion 2026-07-18 (both rounds) + advisor 6C-certified | ✅ BUILT (deepjson improved; idle-box flip-confirm owed) · jsonround 📐 FLAG |
| DEC-295 | 07-18 | **`Regex.replaceCallback` callback receives a typed `Match` object** (dev-ruled, "conceptually better than PHP") — `Match.full() -> string` (whole match) + `Match.group(name) -> string?` (named capture, `null` if absent). Beats PHP's untyped `$matches` array on two axes: (1) a typo in a group name can't silently yield `""`; (2) a non-participating named group returns `null` **consistently on all 3 backends** — the PHP twin uses `preg_replace_callback` + `PREG_UNMATCHED_AS_NULL` so PCRE's `""`-fill divergence (see KNOWN_ISSUES, inherited by findGroups/findAllGroups) is fixed BY DESIGN for this API. Higher-order native (re-entrant VM per [[higher-order-natives-reentrant-vm]]) + a new compiler-injected `Match` type + PHP twin class. Spine-sensitive (higher-order + new injected type) — build carefully under DEC-268 | callback gets full-match string only; callback gets `Map<string,string>` (inherits divergence) — both rejected | dev AskUserQuestion 2026-07-18 | ASKED | ✅ **BUILT (2026-07-18)**: injected `RegexMatch` prelude class (`full()`/`group(name)->string?` via `Map.get`; type name RULED `RegexMatch` since `Match` is a PHP-8 keyword — reserved-name gap fixed `3da89d12`) + `NativeEval::HigherOrder(regex_replace_callback)` (captures_iter, native-built RegexMatch instance, byte-offset splice) + PHP twin `preg_replace_callback` w/ `PREG_UNMATCHED_AS_NULL`+null-filter. Byte-identical run≡runvm≡php incl. the discriminating non-participating-group case (`group("a")==null` all 3 legs — FIXES the findGroups/findAllGroups divergence by design). First native-built instance whose methods dispatch on both backends (validated). |
| DEC-296 | 07-18 | **`Regex.quoteMeta(string) -> string` uses `regex::escape` as the oracle** (dev-ruled) — the `regex` crate's own `escape()` (guaranteed-correct for the engine) is the interpreter/VM impl; the PHP twin `__phorj_regex_quote_meta` reproduces its EXACT meta-set char-for-char (byte-identity via a mirrored helper, Invariant 16) — NOT PHP's `preg_quote` (different set). Small additive native (Pure) | explicit shared minimal meta-set both sides (rejected — diverges from regex::escape output) | dev AskUserQuestion 2026-07-18 | ASKED | ✅ BUILT (this slice) |
| DEC-297 | 07-18 | **Named arguments — call syntax `f(name: value)`** (dev-ruled, PHP-8.0-aligned). Colon spelling (not `name = value`); transpiles 1:1 to PHP 8.0 named args → best lifter fidelity, unambiguous at call sites. Supersedes the old `partitioned = true` builder workaround. Interacts with default params (fill by name). Slice #3 static core. | `name = value` (rejected — reads as assignment, needs transpile rewrite) | dev AskUserQuestion 2026-07-18 | ASKED | ✅ **BUILT FULL SCOPE (2026-07-19, free fns + constructors + methods incl. static)**: `Expr::NamedArg` variant (mirrors `Expr::Tuple`, erased before backends, Inv-5); parser detects `IDENT :` at arg-start; `FnSig.param_names` + `ClassInfo.ctor_param_names` + `MethodSig` 5-tuple carry names; shared `normalize_named_args` front-normalizes named→positional+defaults, recorded as REPLACE fill via `pending_named`+`default_fills` (post-resolution→overload-safe); formatter emits `name:`; byte-identical run≡runvm≡php (3 differential tests + example). 8 rejects (all unhandled paths): unknown/dup/positional-after-named/missing/misplaced + E-NAMED-ARG-UNSUPPORTED for native/generic/overloaded/variadic-combo/no-names(iface-typed). Committed free-fn `89526a84`; ctor+method next commit | DEC-298 | 07-18 | **Variadics — `function f(int ...nums)` collects into `List<int>`** (dev-ruled). `...` prefix (PHP-aligned); the collected param is a typed `List<T>` (reuses the mature List API). Call: `f(1,2,3)`. Slice #3 static core. | dedicated native varargs type (rejected — less reuse) | dev AskUserQuestion 2026-07-18 | ASKED | ✅ **BUILT v1 (2026-07-18, free functions only — like defaults)**: `...`→`TokenKind::DotDotDot` (lexer); `Param.variadic`; sig effective-type `List<T>` + `FnSig.variadic` via single-sourced `effective_param_ty`; call-collection in the SHARED `check_args_defaulted` chokepoint (REPLACE fill via `pending_variadic`+`default_fills`, post-resolution → overload-safe); AST decl rewrite `T ...name`→`List<T> name` (`desugar_variadic_params`, Inv-5) so backends see `f([1,2,3])`+`array $nums`; formatter emits `...`. Validation: last-only (`E-VARIADIC-NOT-LAST`) + no-default (`E-VARIADIC-DEFAULT`). Method/lambda variadic REJECTED (`E-VARIADIC-UNSUPPORTED`) via shared `reject_nonfree_variadic` (the ≥3-site trap: lambda slipped once, fixed). Byte-identical run≡runvm≡php (differential + example); 2229 green; clippy both legs. Approach-B (advisor-ruled: name-based pre-check desugar breaks on return-overloads). Methods/lambdas = follow-on (needs `Ty::Function` variadic flag). |
| DEC-299 | 07-18 | **Spread `f(...x)` — three forms, split into a static CORE + a runtime leg** (dev-ruled, "I want option 2, made robust"). CORE (static, slice #3): (a) **List→positional** — `f(...list)` splats a `List<T>` into positional args, element type+arity checked at compile time; (b) **Map-LITERAL→named** — `f(...["k": v, ...])` DESUGARS at compile time to named args (fully static, heterogeneous values fine — it's literally `f(k: v)`). RUNTIME LEG (leg 2, likely a follow-on slice): (c) **runtime union-typed Map→named** — a `Map<string, U>` (U a union) spreads into named params when each targeted param's type is a member of U (checker enforces statically — no unrelated types); at runtime each value is narrowed to its param's type and key presence validated, both via a typed **E-SPREAD-ARG** fault; byte-identical PHP leg (same fault conditions). Robustness rationale (dev-explored): unions fix REPRESENTATION (heterogeneous map expressible + type-sound to hold) but NOT the key→member or key-presence gaps, which are irreducibly runtime for a runtime map — contained to typed faults, never silent bad values (as robust as a static language allows). Intersections don't apply (red herring). ⚠ leg (c) depends on `Map<K, union>` ergonomics being solid — confirm before building it. | homogeneous-only runtime map (rejected — less flexible); full PHP-dynamic w/ `mixed` (rejected — un-static); literal-only (rejected — dev wants runtime maps) | dev AskUserQuestion 2026-07-18 (3 rounds) | ASKED | ⏸️ **AUTO-DEFERRED (2026-07-19 overnight, fork rule)**: spread's real value is the RUNTIME case (`f(...computedList)`); the literal-only static core (`f(...[1,2,3])`≡`f(1,2,3)`) is near-zero-value sugar not worth an `Expr::Spread` variant alone. Runtime spread needs a runtime-VARIABLE call arity, which collides with the VM's STATIC `stack_effect` model (Invariant #3) → requires a new runtime-args call convention + a VM-Op DESIGN FORK. Per the overnight rule (don't rush a spine+fork change unattended), DEFERRED to an attended slice; overnight hours pivoted to Wave B (the % mover). REVIEW: decide the VM runtime-args convention with the dev, then build spread full (List→positional + Map-literal→named static + runtime union-Map→named w/ E-SPREAD-ARG). |
| DEC-300 | 07-19 | **`Core.Deque<T>` — generic double-ended queue as a pure-Phorj prelude class over `List<T>`** (overnight AUTO, fork rule; collections breadth). Chosen OVER a faithful map of PHP `SplDoublyLinkedList`/`SplStack`/`SplQueue`. Two "better than PHP" departures: (1) `pop*`/`peek*` return `T?` (null on empty) — a normal control-flow condition modelled as a value to unwrap, not the `RuntimeException` the `Spl*` classes throw (safer, more OOP, impossible to forget to guard); (2) ONE uniform generic type replaces three overlapping `Spl*` classes. Implemented in Phorj over `Core.List` (append/concat/slice/index) with a `mutable List<T>` field, so byte-identical run≡runvm≡php BY CONSTRUCTION — no native, no ladder quarantine (verified: the prelude-class compiler path is identical to a user class; direct 3-leg probe + differential `deque_double_ended` + `collections-deque.phg`). Constructed from an initial backing list (`new Deque(new List<int>())` empty; `new Deque([1,2,3])` seeded) per the mandatory-`new`+inferred-`T` rule (a no-arg ctor couldn't infer `T`). API: pushBack/pushFront/popBack/popFront/peekBack/peekFront/size/isEmpty/toList. Registered as `CORE_MODULES` row `Core.Deque` (position not load-bearing — only imports the native `Core.List`). Lift N/A (no distinct PHP construct maps to a Deque; PHP array code already lifts generically). ⚠ PERF (WIN-OR-FLAG, FLAGGED not degraded): front ops (`pushFront`/`popFront`) are O(n) over the backing list — the idiomatic-PHP transpile target is a plain array (`array_unshift`/`array_shift`, also O(n)), so array-op PARITY holds vs real PHP code; the `Spl*` linked list (O(1) ends) is NOT the honest baseline (rarely used). A ring-buffer NATIVE could later win the ends — tracked, not silently downgraded. | faithful `Spl*` map w/ throwing pops (rejected — models normal condition as exception); native ring-buffer now (deferred — perf-only, no correctness need) | overnight AUTO (fork rule, Invariant-15 suspended for the night) | AUTO | ✅ **BUILT (2026-07-19)**: `DEQUE_PRELUDE` + `Core.Deque` CORE_MODULES row (`src/cli/preludes.rs`); example + README + differential; 2249 green; clippy both legs; release built. REVIEW: confirm the T?-on-empty vs throwing-pop choice + whether a ring-buffer native is wanted. |
| DEC-301 | 07-19 | **`Core.PriorityQueue<T>` — generic max-priority queue as a pure-Phorj prelude class over two parallel `List`s** (overnight AUTO, fork rule; collections breadth, twin of DEC-300). Explicit integer priorities; `extractMax` removes+returns the highest-priority element (PHP `SplPriorityQueue` shape). SAME two "better than PHP" departures as Deque: `extractMax`/`peekMax` return `T?` (null on empty, not a thrown `RuntimeException`); uniform typed API. Implemented over TWO index-aligned lists (`values` + `priorities`) rather than a `List<(T,int)>` of tuples — keeps `T` free of any tuple-key-coercion concern (cf. DEC-288 Map bool-key gotcha) and leans only on the mature `Core.List` kernel. Constructed from an initial value list (`new PriorityQueue(new List<int>())` empty) for `T` inference; seed values enter at priority 0 (`List.fill(0, len)` — a defined behaviour, never a misaligned pair). Ties → FIRST element scanned at max priority (deterministic; PHP is unspecified on ties → stricter, not weaker). ⚠ PERF (WIN-OR-FLAG, FLAGGED): `insert` O(1), `extractMax`/`peekMax` O(n) linear scan; idiomatic-PHP transpile target is arrays with the same scan, so array-op parity holds (the `Spl*` heap is not the honest baseline). Heap-backed native could later win extract — tracked, not degraded. ⚠ PROCESS: the first probe was byte-identical run≡php but SEMANTICALLY WRONG (`List.fill` arg order is `(value,count)`, I wrote `(len,0)`→empty priorities→misalign); caught ONLY by a seeded-tie assertion checking the expected VALUE, not backend agreement — reinforces "byte-identity ≠ correct; assert semantics." | tuple-backed `List<(T,int)>` (rejected — needless tuple-key exposure); throwing `Spl*` map (rejected — models normal condition as exception); binary-heap native now (deferred — perf-only) | overnight AUTO (fork rule, Invariant-15 suspended for the night) | AUTO | ✅ **BUILT (2026-07-19)**: `PRIORITY_QUEUE_PRELUDE` + `Core.PriorityQueue` CORE_MODULES row; example + README + differential `priority_queue_max_first`; green; clippy both legs; release built. REVIEW: confirm T?-on-empty + priority-model (explicit int vs Comparable-T) + whether a heap native is wanted. |
| DEC-302 | 07-19 | **Backed enums + `cases()` — scalar-valued enums (PHP 8.1 backed-enum parity)** (overnight AUTO-RULED w/ build-map; NOT built — Large spine+representation fork, deferred to fresh context per the advisor + the spine→FRESH-context standing rule). **Current-syntax failing program:** `enum Suit: string { Hearts = "H", Spades = "S" }` → parse error (no backing type after the name; no `= value` on variants); `Suit.cases()` / `s.value` / `Suit.from("H")` → E-UNKNOWN. **The fork (dev owns — Invariant 15):** phorj enums transpile to an abstract-class + `final class Variant extends Base` hierarchy (VERIFIED this pass: `abstract class Json {}` + `final class JNull extends Json {}`), NOT PHP-native enums. So a backed enum can EITHER (A) switch to a PHP-native `enum Suit: string { case Hearts = 'H'; }` transpile path — most faithful PHP, free native `::cases()`/`::from()`/`->value`, but a SECOND enum representation diverging from the uniform class model, and impossible for GENERIC enums (PHP native enums can't be generic) so it'd be a payload-less-only special case; OR (B) keep the uniform abstract-class model and EMIT the surface — a `value` const per variant + static `cases()`/`from()`/`tryFrom()` methods on the base class. **RECOMMENDED = (B)** [rationale: one enum representation, not two; consistent lift; generic + backed enums share machinery; the PHP output is a plain class with the same methods, byte-identical by the usual class-lowering — META-7 "byte-identity is a tool" via emitted methods, not a native-enum special case]. `cases()` also generalizes to ANY all-payload-less enum (not just backed ones), first-inserted order. **Syntax (recommended):** `enum Suit: string { Hearts = "H", Spades = "S" }` (colon backing type like PHP; `= literal` per variant); accessor `s.value`; statics `Suit.cases(): List<Suit>`, `Suit.from(v): Suit` (fault on miss), `Suit.tryFrom(v): Suit?` (null on miss). **BUILD-MAP (increments, each green-committable):** (1) parser — optional `: BackingType` after enum name (before `{`; coexist with generics? backed enums are payload-less so DISALLOW generics+backing together) + `= literal` per variant → `EnumDecl.backing_type: Option<Type>`, `EnumVariant.backing_value: Option<Expr>` (`src/ast/decls.rs`, `src/parser/items/types.rs`); reject partial/dup/typed-mismatch backing values in checker (`src/checker/collect/types_decls.rs`). (2) checker — resolve `EnumName.cases()`→`List<EnumName>`, `.from`/`.tryFrom`, member `.value`; reject `.value`/`from` on a non-backed enum, reject payload-carrying variant in a backed enum. (3) interpreter — enumerate variants for `cases()`, carry backing value on `EnumVal`, `from`/`tryFrom` scan (`src/interpreter/call.rs`, `src/value.rs`). (4) VM — same, reified (Invariant 6/7 — `cases()` result + `.value` as operands need CTy). (5) transpile — emit `value` const + static `cases()`/`from()`/`tryFrom()` on the base abstract class (`src/transpile/classes.rs`). (6) lift — PHP native backed enum → phorj backed enum. (7) example `examples/guide/enums-backed.phg` + README + differential (all 3 legs incl. a `.value + 1`-shaped CTy case per Invariant 7) + `cases()` on a plain payload-less enum too. **Parity effect:** FN-SPL/SYN + closes a real PHP-8.1 gap; est. +1–2pp. | (A) PHP-native-enum transpile path (rejected as default — two representations, no generics, lift split); throwing-only `from` w/o `tryFrom` (rejected — needs the safe variant) | overnight AUTO (fork rule; Invariant-15 dev-review REQUIRED on the (A)/(B) representation choice) | **RULED (dev AskUserQuestion 2026-07-19: repr (B) — class model + emitted methods)** | ✅ **BUILT (2026-07-19, 6 green increments)** — repr (B) shipped end-to-end. Checker: `EnumInfo.backing`, 11 coded diagnostics (all with explain.rs entries), types `s.value`/`cases()`/`from`/`tryFrom`. Interpreter + VM: `enum_backing`/`EnumDesc.backing` (single-sourced via `const_literal`); VM adds `Op::EnumValue` + `Op::EnumFrom` (all 3 exhaustive matches extended, Invariant 3), `cases()` inlines to `MakeEnum×N + MakeList`; CTy arms for `.value` + `from/tryFrom/cases` (Invariant 7). from-miss fault single-sourced in `value::enum_from_miss` (Invariant 4). Transpile: `value` property + static `cases()`/`from()`/`tryFrom()` on the base class; `Enum.method()` → `Enum::method()`. Lift: PHP backed enum → phorj backed enum. Example `examples/guide/enums-backed.phg` + README + 3 differential tests (incl. `Priority.from(9).value + 1` CTy case + from-miss `agree_err`). run≡runvm≡PHP verified byte-identical. **AUTO decisions taken (dev reviews):** (a) CTy-operand differential uses the approved surface `Priority.from(9).value + 1` (bare `Enum.Variant.value` has no resolution path and isn't approved surface — verified by probe); (b) `cases()` allowed on any all-payload-less enum (per build-map) incl. non-backed; (c) `cases`/`from`/`tryFrom` reserved as variant names across ALL enums (`E-ENUM-RESERVED-VARIANT`) so interception is unambiguous; (d) `.value` on a plain enum = `E-ENUM-NOT-BACKED`. Repr (A) native-PHP-enum path rejected (two representations, no generics, split lifter). |
| DEC-304 | 07-19 | **`Map.containsValue(m, v)` — value-side membership** (overnight AUTO, safe stdlib gap). `Map.has` tests KEYS (`array_key_exists`); there was no VALUE-membership query. Adds `containsValue` = structural `eq_val` over the values, erasing to strict `in_array(needle, map, true)` (scans values, ignores keys) — byte-identical run≡runvm≡php for scalar/nested-collection values; the class-instance identity-vs-structural caveat is `List.contains`'s (documented). Pure native, no spine, no fork. | — | overnight AUTO (safe gap) | AUTO | ✅ **BUILT (2026-07-19)**: `map_contains_value` (`src/native/map.rs`) + map-ops.phg + README + differential `map_contains_value`; green. |
| DEC-305 | 07-19 | **`List.product(List<int>) -> int`** (overnight AUTO, safe stdlib gap) — the multiplicative companion to `List.sum` (empty → 1, PHP `array_product([])`). Checked overflow (faults, doesn't wrap — the SAME `List.sum` caveat: PHP `array_product` promotes to float on overflow; examples stay in i64 range, KNOWN_ISSUES). Pure native, no spine, no fork. | — | overnight AUTO (safe gap) | AUTO | ✅ **BUILT (2026-07-19)**: `list_product` + list-breadth.phg + README + differential `list_product`; green. |
| DEC-306 | 07-19 | **`Set.isSuperset(a, b) -> bool`** (overnight AUTO, safe stdlib gap) — a ⊇ b, the symmetric partner of the existing `isSubset` (`a.isSuperset(b)` ≡ `b.isSubset(a)`). Mirrors `set_is_subset` with args swapped; erases to `count(array_diff(b, a)) === 0`. Pure native, no spine, no fork. | — | overnight AUTO (safe gap) | AUTO | ✅ **BUILT (2026-07-19)**: `set_is_superset` + set-ops.phg + README + differential `set_is_superset`; green. |
| DEC-307 | 07-19 | **`List.none(List<T>, (T) -> bool) -> bool`** (overnight AUTO, safe stdlib gap) — the third of the any/all/none trio (`none` ≡ `!any`): true iff no element satisfies the predicate. Higher-order, mirrors `list_any` (short-circuits at the first match); gated `__phorj_none`. Pure, no spine, no fork. | — | overnight AUTO (safe gap) | AUTO | ✅ **BUILT (2026-07-19)**: `list_none` + `uses_list_none`/`__phorj_none` + list-breadth.phg + README + differential `list_none`; green. |
| DEC-308 | 07-19 | **`List.sortDescending(List<T>) -> List<T>`** (overnight AUTO, safe stdlib gap) — the descending companion to `sort` (natural/byte order, reversed). Defined SORT-then-REVERSE (not a reversed comparator) so it is byte-identical to `array_reverse(__phorj_sort($xs))` including equal-element order; reuses the `__phorj_sort` helper (same `uses_list_sort` flag). Pure, no spine, no fork. | reversed-comparator (rejected — differs from array_reverse on equal elements) | overnight AUTO (safe gap) | AUTO | ✅ **BUILT (2026-07-19)**: `list_sort_descending` + sort.phg + README + differential `list_sort_descending`; green. |
| DEC-309 | 07-19 | **jsonround flip via ARENA-allocated Json — prototype + measure** (dev AskUserQuestion 2026-07-19, "why can't we beat php on jsonround"). Blocker [Verified: DEC-294 arithmetic]: phorj's typed `Json` ADT allocates one `Rc<EnumVal>` PER NODE; php `json_decode($doc,true)` yields a bare nested array = ~1 allocation total. Parse ALONE (205ms, allocation-bound) already exceeds php's whole round-trip (153ms), so it's lost at parse. DEC-293 (within-rep opt) + DEC-294 (lazy) don't flip jsonround's SHALLOW shape. UNEXPLORED lever = **arena/bump-allocate all typed nodes per parse** (all nodes in one block → ~1 alloc, KEEPS the typed `match` surface — unlike abandoning the ADT for a bare array). Challenge: node lifetime/sharing (an extracted sub-node must not dangle → keep the arena alive behind an `Rc`, cousin of lazy-Json's `Rc<str>`, or copy-out-on-extract). Dev ruled: **PROTOTYPE + MEASURE** (empirical answer to "can we beat php"), NOT accept-the-flag and NOT abandon-the-typed-ADT. | (B) abandon typed Json for a php-style array [rejected — loses typed match/exhaustiveness]; (C) accept the FLAG [rejected — dev wants the empirical test] | dev AskUserQuestion 2026-07-19 | RULED | ✅ **RESOLVED — NO-WIN (2026-07-19)**: fresh-context worktree subagent ran a phase-split + eager-routing proxy (bounded, did NOT build the full `Value::JsonArena` — judged not worth the blast radius given negative bounding evidence). NO-WIN on three legs: (a) parse is ALREADY lazy/near-zero-alloc post-DEC-294 (phase-split min-of-9: parse 171ms is the SMALLEST phase, rebuild+stringify 200ms the largest — an arena targets the cheapest phase); (b) deepjson eager-build shows +60% regression = INTRINSIC materialization work an arena can't recover (it makes each alloc cheaper, but alloc is a minority of even the eager cost); (c) blast radius enormous (new `Value::JsonArena` threading dozens of wildcard-free exhaustive matches per Invariant 3 + VM ops + encode/eq/hash/display) for at best single-digit-% jsonround. jsonround residual loss stays a dev-accepted structural FLAG (DEC-294). Nothing built/committed; worktree pristine. Method note: measurable now via per-core `mpstat` (not load-avg). |
| DEC-311 | 07-19 | **JIT vertical for `Core.Map.has` — FLIP the perf loss (campaign #1 of maphas→setcontains→listcontains→…)** (dev-ruled 2026-07-19: flip the native-call-in-loop losses via per-op JIT verticals; fresh-context subagent + main-session independent gate/certify). Mirrors the winning `mapget` vertical: recognizes `Map.has(m,key)` as `CallNative(id,2)` in a JIT-eligible hot loop and emits the inline packed-bucket probe returning a Bool (present?), with the one inversion — a miss is a clean `false` (code 0), NOT a fault. New `extern "C" rt_u_map_has` (one `unsafe { &mut *ctx }` deref, same contract as `rt_u_map_get`, confined to `src/jit/`); no new `Op` (pure vertical over existing `CallNative`). **RESULT [Verified — main-session independent]: 0.03× loss → 1.50× WIN vs php:8.5-cli+JIT** (checksum-gated); VM→JIT 51.4×; hits>0 proven; 4-way byte-identical (JIT≡VM≡interp≡php, present+absent keys); full --all-features gate 2306 green; clippy both legs + fmt clean. | (a) accelerate ALL key/value map types — DEFERRED; (b) full VM-native-overhead reduction — doesn't flip (dispatch inherently slower than inline) | dev AskUserQuestion 2026-07-19 | **AUTO — 2 coverage forks for dev review**: FORK-A the vertical seals `Map<string,int>` (StrIntMap) ONLY (identical to mapget's proven subset); all other key/value maps fall back to the VM (correct, unaccelerated). FORK-C mutable-builder (AMB) maps + `has` are VM-fallback (`code:5` redo) — accelerating them earns nothing on the bench and adds audited-island risk, deferred. Both are perf-COVERAGE boundaries (not language semantics), byte-identical either way. | ✅ **BUILT + COMMITTED (2026-07-19)** — 6 files, additive; `src/jit/{handles,analyze,compile,emit_unboxed/{mod,verticals},tests/verticals}.rs`. |
| DEC-310 | 07-19 | **`Core.Validation` ctype character-class predicates** (autonomous, safe stdlib gap — the §4.12 re-tally flagged FN-CTYPE's GU validators as a cheap, high-value win). Added `isLower`/`isUpper`/`isWhitespace`/`isPunctuation`/`isControl`/`isVisible` (printable non-space)/`isPrintable` (`string -> bool`), mirroring PHP `ctype_lower/upper/space/punct/cntrl/graph/print`. Each Rust predicate = `!empty && all(is_ascii_X)`; transpile emits `preg_match` over the SAME explicit `\xNN` char class WITH the `D` (dollar-end-only) flag. ⚠ NOT `ctype_*`: the ctype extension is SHARED in CI (absent under the hermetic `php -n` oracle — the `transpiled_examples_use_only_tier1_php_functions` guard caught the ctype_* attempt, exactly as it caught the historical `ctype_digit` leak); PCRE is always compiled. The `D` flag makes `$` match only the absolute end, so these do NOT accept a trailing `\n` — MORE correct than the pre-D existing 5. `is_whitespace` uses `[\x09-\x0D\x20]` (the ctype_space set incl. 0x0B, which std `is_ascii_whitespace` omits). Pure, no new `Op`/`Value`, no `CTy` (bool result). run≡runvm≡php-8.5.8 byte-identical + unit tests (byte-boundary + empty) + example. | (a) `ctype_*` transpile — REJECTED (shared extension, hermetic-oracle guard fatal); (b) `preg_match` WITHOUT `D` like the existing 5 — REJECTED (carries the trailing-`\n` divergence; the existing `isAlpha`/`isAlnum`/`isHex`/`isInt`/`isNumber` still have it latent — flagged KNOWN_ISSUES for a follow-up `D` fix) | overnight AUTO (fork rule) | **AUTO — NAMING for dev review**: chose descriptive names over PHP's abbreviations (`isWhitespace` not isSpace, `isPunctuation` not isPunct, `isControl` not isCntrl, `isVisible` for ctype_graph, `isPrintable` for ctype_print); dev may rename | ✅ **BUILT (2026-07-19)** — `src/native/validate.rs` + registry + `validate_tests.rs` + `examples/guide/validate.phg` + README; full `--all-features` gate green. |

## 10. Parity SSOT (2026-06-21/22) — verdict summary

**One-shot 20-track (A–S+V), 41-agent review → 555 deduplicated candidates: 290 adopt / 187 defer / 81
reject.** SSOT: `docs/specs/2026-06-21-php-parity-and-beyond.md`. Verdict vocabulary: kind
port/new/map/omit × rec adopt/defer/reject. Category sections: 2.1 error handling/totality · 2.2 OO &
types · 2.3 pattern matching · 2.4 call convention/operators/syntax · 2.5 semantics/numerics · 2.6
mutation/build/packages · 2.7 stdlib & batteries · 2.8 concurrency/web/security · 2.9 tooling/testing/DX ·
2.10 performance · 2.11 interop & migration · 2.12 docs/governance/competitive.

Developer-locked batch decisions from the triage (already itemized above where major): three-tier error
model (DEC-068); totality-before-overloading reorder (DEC-060); nine new milestones approved (M4, M-NUM,
M-TIME, M-text, M-Test, M-perf, M-Batteries, M8.5, M13); full ROADMAP/MILESTONES write-back; PascalCase
incl. vendor (DEC-035). Representative REJECT bucket (81): single-quote strings, `<=>`, `.` concat,
`switch`, ambient superglobals, `eval`, variable-variables, runtime magic methods (`__get`/`__set`/`__call`),
loose `==` semantics, `@` suppression, PL-theory items that don't earn their surprise budget (typestate,
refinement types, comptime macros noted as vanity for this language's thesis). An earlier version of the
review had a ~56-item purist reject bucket that the developer **corrected** (philosophy recalibration —
see DEC-004); verdicts were re-graded under the craftsmanship-apex lens.

---

## 11. 2026-07-04 fork-backlog adjudication pass (DEC-177…181, all ASKED interactively)

Cleared the entire open-fork backlog so the feature marathon runs without stalls; each ruled via
AskUserQuestion with a verified failing/working program. Full narrative in MASTER-PLAN §13.1.

| ID | Date | Decision | Alternatives rejected | Source | Mode | Shipped |
|----|------|----------|----------------------|--------|------|---------|
| DEC-177 | 07-04 | **`trait` BLESSED alongside MI** — `trait` is fully wired (run≡runvm≡PHP `trait`/`use`, verified end-to-end); both `trait` AND multiple-inheritance are first-class (mirrors PHP's duality). Closes §7-OPEN | reject keyword (SUBSUMED-BY-MI); trait-as-MI-sugar | MASTER-PLAN §7-CLOSED, §13.1 | ASKED | ✅ (already wired; docs pending) |
| DEC-178 | 07-04 | **W3-5 mixed-type-args blocker RESOLVED** via option A (expected-type threading into list-literal call args), built in Wave A; `String.format` args use a CLOSED scalar form, not open `Any`. Folds in UA-1.6 (Set/Map literals — same mechanism) | verbose-now `List<union>` local; W4-1 variadics first | MASTER-PLAN §6 W3-5, §13.1 | ASKED | 📐 (Wave A/C) |
| DEC-179 | 07-04 | **Type-System Completion programme (Wave A)** — usable union-element collections + primitive `match` type-patterns + primitive exhaustiveness + `is` flow-narrowing + is-refinement + **W5-3 sealed hierarchies** (exhaustive class unions too) + faithful transpile. Largest scope ("no half solutions"); reuses M-RT S4 engine | primitives-only (no sealed); collections+match-only phase-1 | MASTER-PLAN §2.7 Wave A, §13.1 | ASKED | 📐 |
| DEC-180 | 07-04 | **Error model — HONOR the ratified 3-tier.** "Which error" solved by `Result<T,ErrorEnum>` + exhaustive variant match + typed try/catch (shipped). Complete Result/throws ergonomics + **audit/reclassify faulting natives** (normal-input → Result/throws/`T?`); faults stay uncatchable (bugs). NO catchable faults | reopen keystone → catchable fault subset; both | MASTER-PLAN §2.7 Wave B, §13.1 | ASKED (dev probed twice, reconsidered) | 📐 (Wave B) |
| DEC-182 | 07-04 | **Canonical `Core.Result<T,E>` + `Core.Option<T>` — injected, explicitly-imported** (were user-defined per-file = "in the wind"). Same pattern as injected `Json` (prelude gated on import + `module_of` registry). `Option<T>` vs built-in `T?`: DISTINCT roles, explicit convert (`Option.ofNullable`/`.toNullable`), NO implicit coercion — `T?` = lightweight/stdlib default, `Option` = opt-in rich monadic. `Error` marker stays built-in; `E` = user enums | Option replaces T? in stdlib; implicit T?↔Option coercion; keep user-defined | MASTER-PLAN §2.7 Wave B, §13.1 | ASKED (dev challenged; reconsidered) | 📐 (Wave B) |
| DEC-181 | 07-04 | **Editors — LSP-first symmetric, then full-native.** VSCode itself is LSP-first (all smarts via `phg lsp`). LSP-first both editors + thin native shells now (run/debug/test+DAP), THEN full native (rich VSCode ext + native IntelliJ/PSI plugin) as follow-on. **STANDING DoD: every feature → both editors same-change** | build native now (unverifiable here); LSP-only forever | MASTER-PLAN §2.7, §13.1 | ASKED | 📐 (native phase) |
| DEC-184 | 07-04 | **Type-test operator = FULL SYMMETRY `is` + `instanceof`** (Wave A slice 3). Both operators test/narrow over primitives AND classes, interchangeably: `x is int` ≡ `x instanceof int`, `x is Circle` ≡ `x instanceof Circle`. Both flow-narrow in `if` branches. Developer chose full symmetry OVER the recommended `is`-universal-/-`instanceof`-class-only split (challenged on TIMTOWTDI + `instanceof int` having no PHP precedent; ruled symmetry anyway). Supersedes UNIFIED-SPEC's deferred `is`=identity (identity → named stdlib form later if needed). Discriminable set = match's (int/float/string/bool/null; decimal/bytes/html/attr erase → rejected); same `string`-over-erased-union byte-identity guard | is-universal + instanceof-class-only (recommended, declined); is=identity (spec, superseded) | MASTER-PLAN §0/§13.2 Wave A slice 3 | ASKED (dev challenged, ruled symmetry) | ✅ (shipped — `src/parser/exprs/climb.rs:132-160`, `is` ≡ `instanceof` incl. `x is null`) |
| DEC-183 | 07-04 | **Flat wildcard-free `match` over `T?` is exhaustive** — `Optional<T>` treated as `T \| null` for match totality: member arms + a `null` arm discharge it, no `_` needed (`int?`, `Circle?`, `(A\|B)?`). Completion of slice-1 (null already discriminable); byte-identity holds (`is_int`/`is_null`/`=== null`, pattern-driven). Bounded caveat: `Optional<enum>` (`Color?`) still needs `_` until enum-variant coverage is threaded through `?` (separate follow-up). Surfaced PENDING by Wave A slice 2, ruled Option A | keep requiring `_`/smart-cast (Option B) | MASTER-PLAN §0/§13.2 Wave A slice 2b | ASKED (dev asked for recommendation, then ruled A) | ✅ (slice 2b) |

### Backfilled pointer rows (⊳ added 2026-07-28, consistency audit — closes C-005: 13 DEC ids were cited across the repo with no register row)

> Pointer rows: id, date, one-line subject, and WHERE the full ruling text lives. DEC-190's ruling
> existed nowhere else, so it is copied IN FULL here rather than pointed at.

| ID | Date | Decision | Alternatives rejected | Source | Mode | Shipped |
|----|------|----------|----------------------|--------|------|---------|
| DEC-185 | 07-04 | **Full `Core.Result` combinator set (Wave B B-2b)**: `map`/`mapErr`/`andThen`/`orElse`/`getOrElse`/`toOption`/`isSuccess`/`isFailure`, UFCS-reached; `filter` intentionally absent (no error to synthesize on `false`). Full ruling: MASTER-PLAN §13.2 decisions log [2026-07-04] | pick-a-subset | MASTER-PLAN §13.2 log; CHANGELOG (B-2b); `examples/README.md` `guide/result-combinators.phg` row | ASKED | ✅ SHIPPED (`src/native/result.rs`, gated `__phorj_result_*` PHP helpers) |
| DEC-187 | 07-04 | **Full width-aware `phg format` wrapping** — EXPAND-ONLY ruled, then **AMENDED same-day to WIDTH-CANONICAL (Rule 2 only)**; Rule 1 "preserve author breaks" dropped. Full ruling + amendment: MASTER-PLAN §13.2 log [2026-07-04] | split the rules across slices | MASTER-PLAN §13.2 log; `examples/format/README.md` ("the width-canonical formatter (DEC-187)") | ASKED | ✅ (shipped as `phg format`) |
| DEC-188 | 07-04 | **TS utility types stay REJECTED; use interface segregation** (compose UP with multi-`extends`; ADR escape hatch only if a real case can't be segregated). Full ruling: MASTER-PLAN §13.1.1 | admit `Exclude`/`Partial`/`Omit` | MASTER-PLAN §13.1.1 | ASKED | ✅ (decision-only; no build) |
| DEC-189 | 07-04 | **stdlib/framework = a sequenced per-component DESIGN PROGRAMME** (Symfony-component/PSR selection principle; each component earns its place via §15 ruling + §14 ladder before building). Full ruling: MASTER-PLAN §13.1.1 | build-the-breadth-at-once | MASTER-PLAN §13.1.1 | ASKED | 📐 (standing programme framing) |
| DEC-190 | 07-04 | **FULL RULING (copied from MASTER-PLAN §13.1.1 — it exists nowhere else): Core is extensible: all Core CLASSES `open`, all Core methods overridable.** (Developer chose "all Core internals open," NOT a whole-language flip — USER code KEEPS final/closed-by-default + the `open`/`open function` opt-in.) `class MyRequest extends Request { … }` + method override works on any Core class. Made SAFE by the mandatory `override` marker (DEC-192). Call up with `parent.method(…)` / `parent(Ancestor).method(…)`. Enum customization stays "redeclare same-name enum to shadow" (ships). **CORRECTION recorded:** `Core.Result.Success` is an enum VARIANT, not a class — you never "extend a variant"; enums are closed data types (shadow to customize). BREAKING-ish: mark Core classes `open` | whole-language open-by-default flip | MASTER-PLAN §13.1.1 | ASKED | 📐 (not built) |
| DEC-192 | 07-04 | **Mandatory `override function` keyword**: overriding without it = `E-MISSING-OVERRIDE`; marking a non-override = `E-NOT-AN-OVERRIDE`; parent opts in (`open function`), child confirms (C#/Kotlin/Swift model) — what makes DEC-190's all-open Core safe. BREAKING. Full ruling: MASTER-PLAN §13.1.1 | marker-optional | MASTER-PLAN §13.1.1 | ASKED | 📐 (not built — no `override` in `src/parser/` as of 2026-07-28) |
| DEC-193 | 07-04 | **Example-coverage audit = its own slice, LATER (after Wave B)**: enumerate every keyword + feature vs `examples/` + playground `gen_examples`; include HTML/templating showcases. Full ruling: MASTER-PLAN §13.1.1 | interrupt the marathon now | MASTER-PLAN §13.1.1 | ASKED | 📐 (queued) |
| DEC-194 | 07-04 | **User-defined attributes (PHP `#[Attribute]` style)**: an attribute IS a class marked `#[Attribute]`, compile-time-const args, read via `Core.Reflect`; byte-identical attribute READING is the design crux (own §15 + ladder slice under DEC-189). Full ruling: MASTER-PLAN §13.1.1 | string/config metadata only | MASTER-PLAN §13.1.1; M-gap-matrix SYN-118 | ASKED | ◐ PARTIAL (declarable + applyable with checked args on classes + free functions, git `bf05648`/`451fb89`; reflection reading not yet — M-gap-matrix SYN-118 verdict P) |
| DEC-195 | 07-05 | **Guard-helper for the 3 "divergences" — RULED, then the PREMISE was RETRACTED same day; RE-DECIDED: DROP entirely** (all 3 fault in PHP too — behaviourally consistent; helpers were cosmetic). Full record: MASTER-PLAN §13.2 log [2026-07-05] | keep the helpers (cosmetic) | MASTER-PLAN §13.2 log; `docs/research/b2d-rich-error-audit.md` | ASKED (re-decided on the corrected basis) | ✅ (dropped by ruling — nothing built, deliberately) |
| DEC-196 | 07-05 | **Examples/conformance audit decisions Q1–Q4**; Q3 = the TWO-MODE intrinsic-import model (`Core.Assert` = { `assert` }, `Core.Abort` = { `panic`,`todo`,`unreachable` }: whole-module→qualified, member→bare, else `E-UNIMPORTED`). Full ruling: MASTER-PLAN §13.2 log [2026-07-05] | single-`import Core;` (the DEC-047 model); bare-always | MASTER-PLAN §13.2 log; `docs/research/2026-07-05-examples-conformance-audit.md` | ASKED | ✅ COMPLETE (Q1+Q2+Q3+Q4 shipped 07-05; `src/checker/intrinsic_imports.rs`) |
| DEC-198 | 07-03/04 | **`String.format` spec = `{}`-style grammar** shared with W5-1 interpolation specifiers (`%`-style rejected at the time) | `%` sprintf (initially rejected) | MASTER-PLAN §6 W3-5 | ASKED | ⊘ SUPERSEDED by DEC-199 (`{}`-for-format dropped) |
| DEC-199 | 07-05 | **`String.format` = PHP-style `%` sprintf, SUPERSEDES DEC-198**: a runtime spec can't be statically checked in any syntax, `%` is collision-free with `{expr}` interpolation and transpiles to literal `sprintf`; rendered STRICTLY (wrong type = clean fault, not coercion). Full reasoning chain: MASTER-PLAN §6 W3-5 | keep `{}` | MASTER-PLAN §6 W3-5; KNOWN_ISSUES §`String.format` | ASKED | ✅ (slices 1+2+3a+3b+3c+4a+4b shipped — Wave C conversion set complete) |
| DEC-303 | 07-19 | **`String.chunk`** — codepoint-based chunking, `__phorj_str_chunk` PHP helper | byte-based chunking | SLICE-STATE 2026-07-19 overnight block (commits `bb39af6f` + `73f31189`) | AUTO (overnight fork rule) | ✅ BUILT (2026-07-19) |

---

## CONFLICTS (contradictory records — adjudicate)

| # | Conflict | Trace | Status |
|---|----------|-------|--------|
| **C-1** | **D-L3 (06-18) REJECTED multiple inheritance** ("realized as traits/mixins + interfaces") — yet **S6 shipped real MI** (`class C extends A, B`) *and* S8 shipped traits. | Traced: 06-18 D-L3 reject (next-intuitive-features spec) → 06-21 dev: "multi-inheritance wanted, real game changer, WITHOUT removing traits" (ga-direction memory) → 06-22 dev rejected the single+traits framing **twice**, demanded research → S6 Model-1 explicit-resolution MI ASKED + shipped; Model-3 C3 deferred. So: a legitimate developer reversal, properly recorded each step — but **D-L3's text was never amended**, so the two specs still contradict. | Developer-driven supersession; needs doc reconciliation, not re-adjudication. |
| **C-2** | **A-6 (06-25) adopted `foreach (coll as …)` to REPLACE `for (x in coll)`** ("free `for` for C-style only") — but commit `0747385` (06-26) shipped foreach **"alongside the typed `for (T x in xs)` form"**; examples still use for-in everywhere; FEATURES.md lists `for … in` ✅ with no replacement note; B1 (06-30) *extended* for-in (string/Map two-binding). | The decided replacement was silently softened into an addition during an autonomous slice. Either the decision or the implementation is wrong. [Verified: both forms parse today.] | **Closed by DEC-343 (2026-07-26): keep both** (DEC-248 superseded on this point; cross-form migration hints queued). |
| **C-3** | **Zero-dep locked framing (06-26): "NO TLS, NO regex, NO http/serde crates, `[dependencies]` empty, verified"** — days later `regex` admitted as dep #2, plus argon2/ctrlc/corosensei (4 deps total). | Each dep individually developer-authorized under the 06-27 dependency policy; but the 06-26 "LOCKED FRAMING" text (native-modules-research plan) explicitly names regex as forbidden and was never updated. | Superseded-in-practice; framing doc stale. |
| **C-4** | **`text` leaf chosen 06-18 explicitly "not `string` (avoids shadowing the `string` type)"** — naming overhaul (06-30) renamed `Core.Text` → **`Core.String`**. | The original rationale (shadowing) is mooted by PascalCase (`String` ≠ `string`), but no record shows the old rationale being revisited when the rename was made. | Likely fine; confirm the shadowing concern was consciously dismissed. |
| **C-5** | **Ternary: two same-day records disagree (06-24)** — perimeter spec says "ternary ✅ add"; master plan says "DEFERRED, not rejected" (postfix-`?` collision + expression-if coverage). | [Verified: `? :` is a parse error in the current binary → DEFERRED won.] The perimeter spec was never corrected. | Resolved in practice; fix the stale record. |
| **C-6** | **M6 design (06-18): OS-thread pools "off the table"** (Rc heap) — yet **M6 W3 shipped an OS-thread pool for `phg serve`** (memory: m6-w3-serve-concurrency), later superseded by green threads. | The W3 pool isolated per-connection state so it didn't share `Value`s, but it contradicts the design's blanket statement; superseded anyway by DEC-132. | Historical; no action beyond doc note. |
| **C-7** | **CLAUDE.md/docs still document `phg bench`, `phg disasm`, `phg fmt`** while DEC-113 renamed the CLI verbs to `benchmark`/`disassemble`/`format`/`tokenize`. | Doc drift from the unpushed naming overhaul; e.g. project CLAUDE.md instructs `phg bench <file>`. | Doc reconciliation task. |
| **C-8** | **`E-INTERSECT-SIG` (require-agreement) was decided with "revisited when overloading lands"** — overloading landed (param + return-type); no record shows the revisit happening. | m-rt plan D2 note vs overloading completion. | **Closed by DEC-245 (overload-set resolution, BUILT 2026-07-16)** — the scheduled D2 revisit happened. |
| **C-9** | **"Nothing in the wind" (06-18) vs shipped import-free intrinsics** — `panic`/`todo`/`unreachable`/`assert` shipped usable with no import, violating the standing principle for weeks. | Caught by the developer 07-01; fix designed (DEC-047: `import Core;`) but NOT implemented. | **Partially resolved:** intrinsic imports shipped via DEC-196 Q3 (two-mode `Core.Assert`/`Core.Abort`, 2026-07-05), NOT the DEC-047 single-`import Core;` model; DEC-047's remaining sub-items (deep imports, aliasing, de-reservations) stay open. |
| **C-10** | **Zero-payload enum-variant construction guidance is stale in older records** — pre-06-24 docs/memory said "construct with `V()`"; mandatory-`new` (DEC-083) made it `new V()`, while *match patterns* still use bare call form `V()` (bare `V =>` remains a silent catch-all footgun, deliberately preserved in DEC-056). | memory zero-payload-variant-call-form (already corrected 07-01) + S4 footgun preservation. | Mostly reconciled; the `V =>` catch-all footgun itself may deserve re-adjudication (it was preserved autonomously). |

## SUPERSEDED (decision → what replaced it)

| Original | Superseded by | When/Who |
|----------|--------------|----------|
| D-L3 reject MI → traits at S5 (06-18) | S6 explicit-resolution MI **and** S8 traits both shipped (DEC-062/064) | 06-21/22, developer (twice rejected the old framing) |
| lowercase `core.console` etc. (06-18) | PascalCase `Core.Console` (DEC-034) | 06-20, developer |
| `console` leaf (06-18) | `Core.Output` (DEC-113) | 06-30, developer |
| `text` leaf (06-18) | `Core.String` (DEC-113) | 06-30, developer |
| `fn` lambda keyword (S3, 06-18) + A-1 typed-lambda `fn(int x): string` (06-25) | lambda keyword `function` (DEC-113) | 06-30, developer |
| `->` return syntax (M1) | `: T` returns, `->` retired (DEC-093) | 06-25, developer |
| `Ok`/`Err` Result variants (error-model slice 2) | `Success`/`Failure` (DEC-113) | 06-30, developer |
| `Empty` PascalCase unit type (06-24) | lowercase `empty` keyword + `E-VOID-IN-UNION` (DEC-113) | 06-30, developer |
| `recv` (green threads) | `receive` (DEC-113) | 06-30, developer |
| CLI `fmt`/`bench`/`disasm`/`lex` | `format`/`benchmark`/`disassemble`/`tokenize` (DEC-113) | 06-30, developer |
| "retire `var`" agreement (06-26 AM) | keep `var`, contextual (DEC-100) | 06-26 same day, developer after research |
| `is` value-equality stub (M1) | real `instanceof`; `is` de-keyworded (DEC-051) | 06-20, developer (over Claude's retire-it dissent) |
| Bare construction `V()` / `Name()` | mandatory `new` everywhere (DEC-083) | 06-24, developer |
| E-PKG-TYPE functions-only libraries (06-18) | cross-package types + `import type` (DEC-036) | 06-20, developer (planned lift) |
| manifest key `name` (06-18) | `module` (DEC-035 slice 1, `ce588e3`) | 06-20, developer |
| `package main` lowercase (M5 S1) | `package Main` PascalCase reshape | 06-23, developer |
| PHP floor 8.4 | floor 8.5 + 8.6-dev canary (DEC-163) | 06-24, developer |
| "keep Phorge, rename pre-GA" (06-21) | rename NOW → Phorj (DEC-013) | 06-28, developer |
| Zero-dep absolute (06-26) | 4-dep vetted policy (DEC-009) | 06-27/29, developer per-dep |
| Tier-3 lift = refuse (M-Lift tier table) | Tier-3 best-effort + loud annotation (DEC-166) | 06-25, developer ("Option 1 and 3") |
| W3 OS-thread serve pool | green-thread runtime (DEC-132) | 06-29, developer |
| spawn-eager synchronous-degenerate (step 2) | cooperative cutover A1 (DEC-134) | 06-29/30, developer demanded litmus |
| `Value::Set` as `HashSet<HKey>` (S7b-2 initial) | insertion-ordered `Rc<Vec<HKey>>` | 06-20, autonomous realignment |
| `Channel.new()` | `Channel.create()` (DEC-112) | 06-29, forced by `new` keyword |
| `Op::MatchFail` | generalized `Op::Fault(FaultMsg)` | 06-17, agreed in-slice |
| M2 "mark-sweep GC" success criterion | Rc/Drop + COW; tracing GC permanently mooted (DEC-123/065) | 06-17→06-21, developer |
| Reflect/Convert/Validate/Crypto package names | Reflection/Conversion/Validation/Cryptography (DEC-113) | 06-30, developer |
| `Bytes.len`/`Text.len` | `.length` hard rename (DEC-102 D) | 06-26, developer |
| php-parity-review (narrow Track A/B) | 20-track roadmap-completeness review (§10) | 06-21, developer |
| flat 2-level imports only | deep imports + dual call form (DEC-047) | 07-01, developer — 📐 not implemented |
| `Attr`/`Error`/`Channel`/`Task` reserved built-ins | de-reserved → importable Core modules (DEC-047) | 07-01, developer — 📐 not implemented |

## AUTONOMOUS-HIGH-IMPACT (adjudicate first)

Ranked by user-visible blast radius (syntax/keywords/semantics). All were made in `_AUTONOMOUS_3C` /
bypass-sentinel sessions without a per-decision ask; some sit inside developer-approved *milestones* but
the specific user-visible call was Claude's.

1. **DEC-056(d) — the `Circle =>` catch-all footgun deliberately preserved** (S4, autonomous D3): a bare
   PascalCase ident in a match arm is a *binding*, silently catching everything; the type-pattern needs
   two idents. This is the same trap that already bit zero-payload enum variants. A one-line warning
   (`W-BINDING-SHADOWS-TYPE`) was possible and was not chosen. **Highest silent-bug surface.**
2. **DEC-094 execution drift — foreach shipped "alongside" instead of "replacing" for-in** (C-2): the
   language now permanently carries TWO iteration statements; every doc/example choice compounds it.
   Decided-ASKED, drifted-AUTONOMOUS.
3. **Totality cluster semantics (DEC-060)**: `E-MISSING-RETURN` hard error + `never` type + the exact
   divergence rules (`while(true)` with no `break` counts, etc.) — a breaking-ish soundness gate whose
   precise contours (what counts as terminating) were fixed autonomously.
4. **Pattern-cluster surface (06-23, fully autonomous)**: `when` guard keyword (contextual), struct
   destructuring forms (shorthand/rename/nesting), number-literal grammar (`0x`/`0b`/`0o`/`_`/`1e3`),
   bitwise operator set incl. `>>` lexed as two `Gt` — all permanent user-facing syntax chosen in one
   autonomous sweep.
5. **S7a generics details (autonomous)**: PascalCase-only type params (`E-TYPE-CASE`), first-binding-wins
   (non-backtracking) inference, inference-only construction with **no turbofish** (`Box<int>(7)` illegal
   forever unless revisited) — the no-explicit-type-arg call syntax is a notable permanent gap vs TS.
6. **Overnight F-001/F-003 (RATIFIED next morning but shipped first)**: UFCS resolves *any imported
   native by first-param unify* — including number receivers (`n.abs()`) — a broad implicit-resolution
   surface; ambiguity = `E-UFCS-AMBIGUOUS`, which later forced a native rename (`repeat`→`fill`).
7. **M-DX debugger surface (07-01, autonomous slices)**: `phg debug` REPL command set + DAP protocol
   choices + `--dump-on-fault` format — developer-facing tool UX fixed without a surface review.
8. **Dogfood W0/W2 grammar patches (07-01, autonomous)**: empty-list literal init rule, comma-throws,
   nested-quote interpolation semantics — small but permanent grammar decisions.
9. **DEC-070 invariance retrofit (autonomous)**: same-head generic assignability tightened (programs
   that previously type-checked now rejected) — a breaking soundness fix applied without an ask.
10. **DEC-127 `Op::SetIndexLocal` + COW in-place mutation model** (autonomous): observable only via
    performance, but it created a new Op + a subtle aliasing contract (`make_mut` at refcount 1) that
    future features must honor.

**Notable ASKED-but-thin decisions worth re-surfacing during adjudication** (recorded as developer
choices but decided rapidly inside marathons): DEC-133 (concurrency permanently outside the PHP oracle —
the single standing exception to the 3-leg identity claim); DEC-083 (`new` on enum variants — no other
language does this; dev overruled the rec); DEC-096 (`++`/`--` as expressions — overruled after hazard
briefing; `W-SEQUENCE-MUTATION` lint status unverified); DEC-057 D2 revisit (C-8).

---

*Register totals: 147 primary rows (DEC-001…DEC-182 numbering with gaps; +6 in the 2026-07-04 fork
adjudication §11) + 555 triage rows summarized
by category (§10). Mode split over primary rows: ASKED ≈ 108 (incl. 2 RATIFIED overnight forks),
AUTONOMOUS ≈ 25, UNCLEAR = 0 — ⊳ CORRECTED 2026-07-02 (row-by-row verification): every primary row
carries an explicit Mode; the original "UNCLEAR ≈ 8 (early-M1/M2, no mode note)" was an arithmetic
residual, not located rows. The 5 mixed-mode rows (DEC-053/056/060/087/129 — ASKED approach /
AUTONOMOUS details) are the only ambiguity, and ALL FIVE were re-adjudicated in the 2026-07-02
rulings (MASTER-PLAN §12). 10 conflicts,
33 supersessions traced.*

⊳ CORRECTED 2026-07-28 (consistency audit): the register now holds **239 `DEC-`/`META-` table rows
through DEC-388** (re-derived: `grep -cE '^\| ?(DEC|META)-'` = 239, including the 13 backfilled
pointer rows added the same day; the many bullet-format rulings in the dated sections are
additional), **≥40 traced supersessions** (31 SUPERSEDED-table rows + the inline `⊳` row
annotations); conflicts **C-2 (closed by DEC-343)** and **C-8 (closed by DEC-245)** are closed —
8 of the original 10 remain open. The "MASTER-PLAN §12" pointer above is now **Appendix B**
(MASTER-PLAN renumbering).


---

## 2026-07-12 adjudication batch (Fable run, session 6 — developer via AskUserQuestion, all Mode: ASKED)

Per the developer's standing instruction this batch records EVERY ruling **with the alternatives
considered and why they lost**. All six pending forks + three run-level meta-rulings cleared in
one sitting (failing programs + after-state previews were embedded in each dialog).

- **DEC-201 — empty collection literals: BOTH contextual typing AND explicit constructors.** *(SUPERSEDED by DEC-214, 2026-07-13 — empty collections now use `new List<T>()`/`new Map<K,V>()`; `[]`/`{}` contextual typing and `List.empty`/`Map.empty` removed; `[1,2,3]` kept. `List.empty` bypassed mandatory-`new` and the contextual typing was "type-from-later-use" inference the developer ruled out.)*
  `List<int> xs = [];` adopts the annotated type in declarations/assignments/call-args/returns,
  AND `List.empty<T>()` / `Map.empty<K,V>()` ship for expression positions with no context.
  *Alternatives:* contextual-only (loses the no-context expression case), constructors-only
  (verbose; the annotation is right there). Both was chosen for completeness.
- **DEC-202 (closes DEC-200) — PHP-reserved top-level type names: REJECT with `E-RESERVED-NAME`.** *(SHIPPED 2026-07-13: `is_php_builtin_class_name` in checker/common.rs — ~100 always-loaded Core/SPL/date/json names, case-insensitive, class-position kinds only; foreign `declare class` binds are EXEMPT by design — they bind to the builtin, nothing redeclares; free functions stay legal (separate PHP namespace); tests in checker/tests/casing.rs + `phg explain E-RESERVED-NAME` updated.)*
  Extend `is_php_reserved_symbol_name` with the full keyword set (derived empirically vs php-8.5.8)
  + the PHP builtin-class core (`Exception`/`Error`/`Closure`/…). *Alternatives:* invisible mangle
  (like enum variants — rejected: silently renames a USER-chosen top-level symbol, surprising on
  PHP interop/debugging); hybrid reject-keywords/mangle-builtins (rejected: two rules where one
  suffices). Legibility + no-surprises won.
- **DEC-203 — scope guard: `using (h = expr) { … }` block** (C#-style; closes at block exit on
  every path incl. throw; the type implements a `Closable` contract; transpiles to PHP
  try/finally). *Alternatives:* Go-style `defer` (rejected: LIFO order + capture timing = new
  footgun surface with no PHP analog); both (rejected: two mechanisms, more spec surface — can be
  revisited if `using` proves insufficient).
- **DEC-204 — graceful shutdown: typed `Runtime.onShutdown(fn)`** (single registration point,
  SIGINT/SIGTERM before exit; vetted `ctrlc` already in-tree; lands with Ω-2 `Core.Process`;
  pairs with DEC-203 for resource cleanup). *Alternatives:* serve-only hook (rejected: CLI worker
  loops still die cold); stay excluded (rejected: kills the Ω-1 web-spine durability story).
- **DEC-205 — Rc cycle leak: BOTH, PHASED — PHP-style threshold cycle collector first (safety:
  `serve` can never leak; semantically invisible, exact PHP engine parity), `Weak<T>` second**
  (zero-overhead idiom for graph back-edges; transpiles 1:1 to PHP `WeakReference` (7.4+), so
  byte-identity holds). Ruled after a perf re-ask: collector ≈ zero steady-state cost
  (root-buffering on decrement + threshold passes), Weak = fastest but not a safety net alone.
  *Alternatives:* collector-only (graph-heavy code pays avoidable passes); Weak-only (a forgotten
  weak edge still leaks in serve — burden on the user).
- **DEC-206 — bare `DateTime`: GATE IT** (`E-INJECTED-TYPE-BARE`, same hint as its Core.Time
  siblings — closes the UA-L2 nothing-in-the-wind inconsistency; the fix for affected code is one
  member-import line). *Alternatives:* un-gate the siblings (repeals nothing-in-the-wind for the
  module); leave-and-document (permanent wart against the #1 recurring design rule).
  ⊳ Direction superseded by DEC-386/L-85 → DEC-353 consistency (un-gate bare `DateTime`).

**Run-level meta-rulings (same sitting):**
- **META-1 — sqlbuild bar: go ALL THE WAY (L2a str-ACL builder → L2b field-transfer → L3
  refcounted JIT handles) until ≥ 1.0× vs php, BEFORE Ω-wave work**; at run end ALL known issues
  and design decisions are reopened for a full re-discussion; every decision records its
  alternatives (this format). *Alternatives:* flag after L2a/L2b (deferred perf debt); flag now
  (fastest breadth) — both rejected for the perf mandate.
- **META-2 — L3 representation constraint: IN-ISLAND, ZERO-DEP** — refcounts live as arena
  bookkeeping inside `src/jit/handles.rs` (the existing audited unsafe island; a parallel
  per-slot count array). Ruled after a dep re-ask: no crate does arena-word refcounting
  (thin-Rc crates target the VM-side Value layer = parked V3b, not L3). *Alternatives:*
  pre-approve `triomphe` for V3b too (broader than needed); decide-per-design (more asks).
- **META-3 — wave order confirmed as written:** Ω-1 Core.Db → HTTP → sessions, then Ω-2…Ω-9
  in sequence. *Alternatives:* language-surface-first, web-spine-depth-first — both declined.

---

## 2026-07-13 language-reconsideration batch (Opus run — developer via AskUserQuestion, all Mode: ASKED)

Developer-initiated "rethink anything opinionated that should not be in the language," apex filter
= CRAFTSMANSHIP (SOLID / design patterns / best practice), NOT familiarity or minimalism. Each
ruling had a failing/before program + per-option previews embedded in its dialog. Session
certification ran **self-graded** (advisor inactive: advisor==main==Opus 4.8). All items below are
**RULED, build-pending** unless marked SHIPPED. Full research: `scratchpad/verify-*.md`,
`raw-static-access.md`, `raw-core-vs-library.md`, `raw-opinionated-sweep.md`.

- **DEC-207 — static/class-level access separator: adopt `::`.** Class/type-level access uses `::`
  (static methods, static fields/consts, enum-variant construct + match, `parent`); instance access
  stays `.`/`?.` (→PHP `->`/`?->`); module functions stay `.` (a module is a namespace, not a class;
  →PHP free function). Makes static-vs-instance visible at the call site (legibility = a craftsmanship
  axis) and PHP↔Phorj round-trip lossless (transpiler already emits `Counter::make()`/`parent::`; the
  lifter today FLATTENS PHP `::` and `->` both into `.`). Does NOT change checker resolution (stays
  name-based). *Alternatives:* `::` for ALL non-instance incl module fns (rejected — conflates namespace
  with class; dishonest about what a module is); keep unified `.` (rejected — static/instance invisible,
  lossy round-trip). **Partially supersedes the naming-overhaul "unified `.`".**
  *(CODEMOD SCOPE CORRECTION 2026-07-13: NOT ~182 files — module functions like `Output.printLine` STAY
  `.` (R1); the codemod is class-static/const/enum-variant/`parent` accesses — larger than "moderate"
  because enum variants (Result/Option/Json) are pervasive, but NOT the 962 module-fn occurrences.)
  **PART-1 SHIPPED 2026-07-13 (additive — the earlier "no sound partial" fear was WRONG for DEC-207): the
  `::` CAPABILITY.** `TokenKind::ColonColon` + tokenizer two-char rule; `enum MemberSep { Dot, ColonColon }`
  + `sep` field on `Expr::Member` (~36-site ripple, all `Dot`); parser accepts `::` in the postfix member
  loop, `new Enum::Variant`, match patterns, and `parent::`; both printers (format + lift) render `::`;
  lifter maps PHP `::`↔`->` faithfully. **Additive — `.` still works everywhere**; example
  `guide/colon-colon-access.phg` (`MathUtil::square()`/`Counter::start()` static via `::`, `c.add()`
  instance via `.`), byte-identical run/runvm/php, canonical formatting, transpiles to PHP `::`. No new
  `Op`. **PART-2 (enforcement + codemod):** add `E-SEP-MISMATCH` (require `::` for class-static/const/
  enum-variant/parent, `.` for instance/module) at the checker resolution sites; add a `sep` marker to
  `Pattern::Variant` + `ParentCall` so match-patterns and `parent::` also RENDER `::` (part-1 renders
  those back to `.`); then codemod all class-level `.`→`::` across preludes + examples + fixtures
  (enforcement errors pinpoint every site) — the large-but-mechanical migration. FULL IMPL MAP (verified/built in the attempt): (1) token — add
  `TokenKind::ColonColon` (`token.rs`) + a `(b':', Some(b':')) => ColonColon` arm in the tokenizer
  two-char dispatch (`tokenizer/mod.rs:~340`). (2) AST — add `enum MemberSep { Dot, ColonColon }` +
  `sep: MemberSep` field on `Expr::Member` (`ast/exprs.rs`); ~36 sites ripple (26 construction → `Dot`,
  10 match → `sep: _`); a subagent did this once cleanly. NB the ~9 rewrite passes that rebuild `Member`
  clobber `sep`→`Dot` but that's HARMLESS — `sep` only matters pre-rewrite (formatter reads the raw
  parser AST; checker enforces during type-check; backends ignore it). (3) Parser — postfix loop accepts
  `Dot|QuestionDot|ColonColon`, sets `sep` (`parser/exprs/climb.rs`, done); STILL TODO: enum-variant
  construct (`new Enum::Variant` — the `new` dotted chain) + match patterns (`Enum::Variant` in
  `parser/patterns.rs`) + `parent::` (parse_parent_call). (4) CHECKER ENFORCEMENT (the semantic core) —
  at each Member resolution (`calls/core.rs::check_call`, `calls/methods.rs::check_member`, enum-variant
  + const + parent sites), after the existing name-based kind resolution, require `sep==ColonColon` for
  class-static/const/enum-variant/parent and `sep==Dot` for instance/module, else `E-SEP-MISMATCH`.
  (5) Formatter — render `::`/`.`/`?.` from `sep` (`format/printer/exprs.rs` Member arm). (6) Lifter —
  PHP `::` → `ColonColon`, `->`/`.` → `Dot` (`lift/lifter/exprs.rs`, currently flattens both to `.`).
  (7) Codemod all class-level accesses in examples/conformance/tests → `::` + fixtures. (8) Gate. Steps
  (1)-(3) mechanical; (4) is the real work but comparatively mechanical (kind already known at each site).)*
- **DEC-208 — DB: drop the query builder from the language; ship an enhanced-PDO primitive.** The SQL
  query builder leaves the language AND is NOT a first-party library (any builder = 100% userland).
  Phorj instead provides an **enhanced PDO-style DB primitive** (better than PHP's PDO — surface TBD
  in a follow-up design round: typed, Result-returning, prepared-statement-first, no silent coercion).
  **Strict import discipline reaffirmed: always `import` required, nothing inferred, nothing in the
  wind.** *Alternatives:* seam — move the web spine (Sql/Db/HTTP/Router/Sessions/Template/Dotenv) to
  first-party bundled libraries via the existing `phorj.toml`/`phg vendor` path (RECOMMENDED by the
  analysis but OVERRULED — dev wants the low-level primitive, not a curated builder); keep in Core
  (rejected — heavier than PHP's floor, couples app concerns to the language). **Supersedes the shipped
  Core.Sql DBAL slices + the DEC-era Core.Sql design.**
  - **SURFACE RULED 2026-07-13 (ASKED, two AskUserQuestion rounds).** The enhanced-PDO primitive =
    **shape 1 + shape 3 combined** — a strongly-typed PDO with generics. `import Core.Db` required.
    - **Connection:** `Db db = new Db("sqlite:app.db")` (DSN string; mandatory `new`).
    - **Prepared-first:** `Statement s = db.prepare(sql)`; every path goes through a `Statement`.
    - **Both bind styles** (chosen): positional `s.bind(v)` (`?` placeholders, left-to-right) AND named
      `s.bindNamed("name", v)` (`:name`); mutually exclusive per statement; binds are chainable and typed.
    - **Dynamic path (shape 1, KEPT):** `Rows rows = s.query()` → `for (Row r in rows) { r.getInt("c");
      r.getString("name"); ... }` — typed accessors, no silent coercion; for ad-hoc/aggregate SQL where no
      result class exists (`COUNT(*)`, exploratory).
    - **Typed-generic path (shape 3):** `List<T> = s.queryInto<T>()` and `T? = s.queryOneInto<T>()`
      (0 rows → `null`, 1 → the object, >1 → `DbError`). Row→object mapping is **by field NAME, STRICT**
      (chosen): every public field of `T` must have a same-named result column; a type mismatch OR a SQL
      NULL into a non-optional field → `DbError`; extra columns ignored; declare `int? age` to admit NULL.
    - **Writes:** `int n = s.exec()` → affected-row count.
    - **Errors:** a checked `DbError` (thrown, never PDO's silent `false`/`null`); propagated with `?` like
      any checked fault. Enhancements over PDO: strong typing + generics + strict mapping + no silent
      coercion + checked errors + mandatory prepared statements.
    - **LADDER (invariant 14):** transpile leg is **faithful (case 1)** — maps to PHP PDO
      (`new PDO(dsn)`, `prepare`/`bindValue`/`execute`/`fetch*`, object hydration). Native leg executes
      over `rusqlite` (`db` feature, already vetted — UNIFIED-SPEC §External dependency policy Q1/Q2).
    - **SPINE TREATMENT:** per the adopted plan (UNIFIED-SPEC P2/Tier-B) DB *execution* is **Tier-3
      fixture-tested, NOT in the example-glob byte-identity spine** (live I/O can't be trivially
      byte-identical rusqlite-vs-PDO); the surface's *parse/check/transpile-shape* stays spine-tested.
    - **Alternatives (this round):** shape 1 only (PDO-faithful, no generics — rejected, dev wants
      generics); shape 2 one-shot `db.query(sql, binds)` (rejected — no Statement reuse); mapping by
      constructor-order (rejected — order-fragile); lenient mapping (rejected — silent-default footgun);
      positional-only / named-only binds (rejected — dev chose both); `queryOneInto():T`-throws or
      no-single-helper (rejected — dev chose `T?`); generics-only, drop dynamic path (rejected — dev keeps
      both). *Build is multi-slice; slice plan in MASTER-PLAN §0.1.*
  - **ERROR-MECHANISM RULED 2026-07-13 (ASKED) = Option A: prelude-wrapper over result-returning
    natives.** Blocker found while building: phorj's native ABI has no throws channel — a native's
    `Err(String)` is a HARD, uncatchable fault (only `Op::Throw` from phorj-source is catchable), so
    routing `db.prepare(...)` to a plain `CallNative` cannot express the ruled catchable `throws DbError`
    (Q6). Ruling: the `Db`/`Statement`/`Row` surface methods are **phorj-source prelude methods declared
    `throws DbError`**; the Rust natives (src/native/db.rs) **return a result-encoding value (ok | error),
    never fault**; the prelude inspects it and `throw`s a real `DbError` (the same catchable mechanism
    Core.Sql's `?`-throws used). No native-ABI change; spine-safe; reuses proven machinery. *Implication
    for the build:* commit 2's natives must be reworked from `Err(String)`-on-SQL-error to returning a
    result value; the S1 surface (commit 3) becomes prelude classes wrapping the opaque native handle
    rather than pure built-in-class recognition. *S2 caveat noted:* the type-directed `queryInto<T>`
    hydration still needs a native returning the same result-encoding convention. *Alternatives:* B —
    extend the native ABI with a `throws` channel (cleaner call sites, benefits all future throwing
    natives, but a cross-cutting spine-adjacent change — REJECTED as too big for the need now); C —
    DbError as a hard uncatchable fault (REJECTED — reverses Q6, un-PDO-like, no in-language recovery).
  - **SLICE C SHIPPED (2026-07-14) — transactions & correctness (partial), one PENDING adjudication.**
    Shipped on the SQLite driver, designed for later drivers: manual PDO-faithful `db.begin()`/`commit()`/
    `rollback()` (savepoint-aware — a nested `begin()` opens `SAVEPOINT phorj_sp_<depth>`, depth tracked
    in `src/native/db.rs` shared across handles, so transactional helpers compose) + `db.rollbackQuiet()`
    (never-throwing, for the `finally` auto-rollback idiom) + a typed `DbError` taxonomy (`open class
    DbError` + `UniqueViolation`/`ConstraintViolation`/`ConnectionError`/`SerializationFailure`/`Timeout`/
    `SyntaxError` — mapped from SQLite extended result codes at the native boundary, classified at the
    single `DbError.fail` throw-helper so every method incl. the S2 `queryInto` helpers auto-upgrades to
    the precise catchable type) + deterministic idempotent `db.close()` (`Option`-wrapped connection;
    later use → `ConnectionError`). Files: `src/native/db.rs`, `DB_PRELUDE` in `src/cli/preludes.rs`,
    `tests/database.rs` (9 native unit + 6 phorj fixtures), `examples/database/transactions.phg`. `run ≡ runvm`;
    spine-quarantined (impure); nothing-in-the-wind (subtypes member-gated in `bare_types`).
    - **PENDING (Invariant 15) — the closure form `db.transaction(() => { … })` + closure `retry`.**
      ⊳ SHIPPED 2026-07-14 — unblocked by DEC-222 throwing-closure function types (see the DEC-249
      block); label flipped 2026-07-28, consistency audit. NOT a
      scope choice: a phorj **lambda cannot declare or propagate a checked exception** (`Type::Function`
      has no `throws` clause in the parser/AST; `cur_throws` is empty in a lambda body), so a closure that
      does real DB work cannot carry `throws DbError` nor surface a *catchable typed* error to the wrapper
      for auto-rollback/transient-retry. Minimal failing program in `KNOWN_ISSUES.md`. Enabling it needs
      **throwing-closure function types** (`() => T throws E`) — a cross-cutting user-visible language
      change (affects ALL higher-order code) = the developer's ruling. *Deferred, not blocked:* `using`/
      `Closable` auto-close (DEC-203 — separate language slice; `close()` ships) and isolation levels
      (SQLite has ~one; meaningful once Postgres lands — kept out to keep the overload set arity-clean).
- **DEC-209 — match legibility: reject bare PascalCase arms; `default` is the catch-all; `_` = ignore-only.**
  A lone PascalCase ident arm (`Circle =>`) currently becomes a SILENT catch-all binding — verified
  live: `match(s){Circle=>"c"}` returns "c" for a `Square` (byte-identity holds across all 4 backends,
  so a legibility/refuse-to-lie footgun, not a spine break). Reject it with `E-MATCH-BARE-VARIANT`
  (hint the 3 intents). The standalone catch-all keyword becomes **`default`** (PHP-match aligned), NOT
  `_`; `_` survives ONLY as an ignore-placeholder (type-test `Square _`, unused bindings). *Alternatives:*
  warn-only (rejected — ignored warnings still ship wrong-but-passing programs); keep silent (rejected);
  full `Shape.Circle` qualification (rejected — breaks idiomatic bare `Circle() =>`); remove `_` entirely
  (rejected — forces named-but-unused bindings); keep both `_` and `default` as catch-all (rejected — TIMTOWTDI).
  Closes DEC-056d. *(SHIPPED 2026-07-13: parser `parse_arm_pattern` (`default`→Wildcard catch-all;
  standalone `_`→`E-MATCH-BARE-VARIANT`) + bare-PascalCase rejection in `parse_pattern`; formatter + lift
  printer render a top-level catch-all Wildcard as `default`; `phg explain E-MATCH-BARE-VARIANT`; nullary
  variant matches now require `Name()` (bare `Red`→`Red()`); codemod of all `_ =>` + bare-variant arms
  across examples/conformance/bench/tests; new parser tests; full oracle gate 1974 green.)*
- **DEC-210 — `++`/`--` ratified STATEMENT-ONLY; register corrected.** The code is already statement-only
  (`parser/stmts.rs`, desugar `x=x+1`; `x=i++`/`a[i++]=i++` are parse errors) — the craftsmanship-correct
  design with no sequence-point footgun expressible. The register's DEC-096 row wrongly marked the
  expression-form + a `W-SEQUENCE-MUTATION` lint as shipped; both were OVERRULED 2026-06-25 and never
  built. Ruling: affirm statement-only, mark DEC-096 superseded/never-built. No code change. *Alternatives:*
  build expr-form + the lint (rejected — reintroduces the eliminated footgun). Corrects/supersedes DEC-096.
- **DEC-211 — generic type bounds: add `T: Interface`/trait.** A type param may be bounded to an
  interface/trait, enforced at BOTH the definition site (body limited to the bound's members) and
  instantiation (the type arg must implement it); erased to PHP interface calls. Bare `<T>` stays legal.
  Closes the "maximal generics" hole (`function max<T: Comparable>(a:T,b:T):T` is unwritable today —
  `a>b` on bare `T` is rejected). Reuses the existing interface/trait conformance table. *Alternatives:*
  stay bound-less (rejected — `max`/`sort` unwritable); hardcode magic `Comparable`/`Numeric` (rejected —
  the one-domain-hardcode anti-pattern this sweep removes elsewhere). (Doc fix: UNIFIED-SPEC:104 says
  "monomorphized"; impl is ERASURE everywhere else. Memory index "trait CLOSED" is wrong — DEC-177 blessed traits.)
  *(SHIPPED 2026-07-13 — full + sound. Both halves built: (a) def-site — a bounded `Ty::Param(T)`'s
  member access resolves against its bound interface (`check_method_call` remap) + a bounded `T` is
  `ty_assignable` to its bound (so `a.cmp(b)` with `b: T` type-checks); (b) instantiation — after θ binds
  `T:=X` in `check_generic_call`, `X` must implement the bound or `E-BOUND-NOT-SATISFIED`. Bounds
  threaded via `active_type_param_bounds`/`cur_class_type_param_bounds` (checker context) + `FnSig`; the
  formatter renders `<T: Bound>` (`type_params_body`); pre-check rewrite passes (rewrite_alias/
  collapse_injected) PRESERVE bounds (the key bug: they'd dropped them to `Vec::new()` before the check).
  Example `guide/generic-bounds.phg`, checker test `generic_bound_enforced_at_definition_and_instantiation`,
  `phg explain E-BOUND-NOT-SATISFIED`. Full oracle 1976 green; clippy both + fmt clean; byte-identical.
  The earlier "no committable partial" was right — so it was built whole, not partial. FULL IMPL
  MAP (verified sites): (1) AST — add `type_param_bounds: Vec<(String,String)>` to FunctionDecl/ClassDecl/
  EnumDecl (`ast/decls.rs`); ~31 construction sites need the field (parser sites use the parsed value, all
  backend/erasure/rebuild/test sites `Vec::new()`). (2) Parser — `parse_type_params` (`parser/types.rs`)
  returns `(Vec<String>, Vec<(String,String)>)`, parsing an optional `: Interface` per param; its 4
  callers destructure. (3) Checker context — add `active_type_param_bounds` + `cur_class_type_param_bounds`
  to the Checker (`checker/mod.rs:453/457`), set/clear ALONGSIDE `active_type_params` (in
  `program/type_bodies.rs` method/ctor/hook sites + `check_function` for free fns). (4) DEF-SITE — in
  `check_method_call` (`calls/methods.rs:6`), just before `match base`, remap a `Ty::Param(p)` that has an
  active bound `B` to `Ty::Named(B, vec![])` so the existing interface-method-resolution arm types the
  call against the bound (one clean remap). (5) INSTANTIATION (soundness-critical) — in the generic-call
  unify path (`check_generic_call`/`unify`+θ), after θ binds `T:=X`, check `X` implements each bounded
  `T`'s interface via `ast::class_implements` (`ast/class_hierarchy.rs:17`); else `E-BOUND-NOT-SATISFIED`.
  (6) Erased before backends. (7) Tests: `max<T:Comparable>` body `a.cmp(b)` type-checks; `max<Socket>`
  rejected; bare `<T>` still legal; example. Steps (1)-(3) mechanical (~40 min, a subagent did (1) once);
  (4)+(5) are the real type-system work.)*
- **DEC-212 — domain literals: generalize `html"…"` to a tagged-template primitive.** The language gains
  ONE general tagged-template mechanism (a user-definable interpolation handler returning a typed
  newtype); `html` becomes a first-party library on it, keeping the EXACT escaping kernel
  (`htmlspecialchars(ENT_QUOTES,'UTF-8')`), the erased `Html`/`Attr` newtypes, and byte-identity. No more
  hardcoded domain literals in the lexer. Consistent with DEC-208 (domains live as libraries; the language
  provides the primitive) + nothing-in-the-wind (import-gated). *Alternatives:* keep hardcoded `html`,
  add no more (rejected — a permanent lexer special-case that doesn't generalize).
  **SURFACE RULED 2026-07-13 (developer via AskUserQuestion): BOTH modes.** Any `tag"…literal{hole}…"`
  (an ident directly before `"`) is a tagged template; the checker resolves `tag` and picks the desugar:
  (1) **protocol mode** — `tag` provides `raw`/`text`/`concat` (+ a typed newtype) → desugars EXACTLY like
  html today (`tag.concat([tag.raw(lit), tag.text(hole), …])`, escape-by-default kernel); html becomes one
  such tag, kernel unchanged. (2) **function mode** — `tag` is a function `(List<string> literals,
  List<H> holes) -> R` → desugars to `tag([lits], [holes])` (JS-style; the handler owns escaping). Part-1 =
  the general primitive with both modes, html re-expressed as a protocol tag (still built-in, additive);
  part-2 = migrate `html` to a first-party library once the library-delivery path lands (DEC-218).
  *(PART-1 SHIPPED 2026-07-13: any `ident"…"` is a tagged template (`TokenKind::TaggedTemplate` + lexer
  ident-glued-to-`"` rule; `Expr::TaggedTemplate`; `html` kept on its own `Expr::Html` path unchanged).
  `check_tagged_template` (checker/expr/literals.rs) resolves the tag: FUNCTION mode when it names a
  non-overloaded free function → `tag([lits],[holes])`; PROTOCOL mode when it names a type/module with
  raw/text/concat → `tag.concat([tag.raw(lit), tag.text(hole),…])`; else `E-UNKNOWN-TAG`. The desugar is
  stored + applied by `resolve_html` (erased before backends). Formatter/lift render `tag"…"`. Example
  `guide/tagged-templates.phg` (both modes), checker test `tagged_template_unknown_tag_rejected`,
  `phg explain E-UNKNOWN-TAG`. Full oracle 1978 green; clippy both + fmt clean; byte-identical; no new Op.
  PART-2 remains: migrate `html` off its special path onto this primitive as a first-party library (DEC-218).)*
- **DEC-213 — PHP-name collision: fix the live byte-identity bug; keep the reject/mangle axis.**
  BUG (G-1 spine break, verified): the enum-variant mangle list (~17 engine-core names,
  `transpile/names.rs`) is a strict SUBSET of the DEC-202 reject list (~100 preloaded builtins,
  `checker/common.rs`), so a variant named `DateTime`/`RuntimeException`/`ArrayObject` runs (exit 0) but
  its transpiled PHP throws `Cannot redeclare class DateTime` — masked only because no example uses one.
  Fix: feed BOTH the reject and the mangle from ONE shared builtin-class constant. The reject-vs-mangle
  AXIS is principled and KEPT (human-chosen API name = loud `E-RESERVED-NAME`; impl-detail variant =
  silent mangle). *Alternatives:* emission-side isolation / always-namespaced output (would drop both the
  reject and the mangle so a phorj programmer may name a class `Exception` — truest to "bridge not soul",
  but a spine-level full byte-identity re-baseline of every single-package example; DECLINED for now,
  not scheduled); unify toward one policy all-reject/all-mangle (rejected — worse both ways). This is a
  correctness fix, implemented independent of the surface rulings. *(SHIPPED `b8dd069`: `src/php_names.rs`
  single-sources the builtin-class list; `checker/common.rs` re-exports it, `transpile/names.rs` group-3
  calls it; differential example `transpile/enum_variant_builtin_names.phg`; full oracle gate 1973 green.)*
- **DEC-214 — empty collections via `new List<T>()` / `new Map<K,V>()`; SUPERSEDES DEC-201.** Empty
  collections are CONSTRUCTED with mandatory `new` (`new List<int>()`, `new Map<string,int>()`); the
  empty-literal contextual typing (`var xs = [];` inferred from later use) AND the `List.empty<T>()` /
  `Map.empty<K,V>()` static factories are both REMOVED. Non-empty literals `[1,2,3]` stay (element type
  is locally obvious, not "in the wind"). Local scalar inference (`var n = 42`) stays. Rationale:
  `List.empty<T>()` bypassed the mandatory-`new` tenet, and empty-literal "type from later use" is exactly
  the inference the developer's "nothing inferred" rules out. *Alternatives:* all collections via `new`
  incl. `[1,2,3]` → `new List<int>(1,2,3)`, remove bracket literals entirely (rejected — loses ergonomic
  literals where the type is self-evident); keep DEC-201 (rejected — retains the `new`-bypass factory +
  the type-from-later-use inference). **Supersedes DEC-201.** *(PART-1 SHIPPED 2026-07-13: the
  `new List<T>()` / `new Map<K,V>()` CAPABILITY — `Expr::NewColl` + `CollKind`, parser reuses
  `parse_type` for the generic head, checker `check_new_coll` self-types via `resolve_type`, all 3
  backends build an empty collection (transpile→`[]`), formatter/lift render, parser test + example
  `guide/empty-collections.phg`; PURELY ADDITIVE — `[]` still works. Full oracle 1975 green. `Set`
  deferred (no empty-set VM op → would need a new `Op`). **PART-2 PENDING** *(⊳ shipped 2026-07-14 —
  see the PART-2 SHIPPED entry below)*: remove the empty-`[]`
  contextual typing (calls/args.rs `check_arg` + `thread_literal_expected` empty-list path + decl/return
  threading) so bare `[]` errors "use `new List<T>()`", then codemod every empty-`[]` across the repo —
  a DEC-209-sized churn; separate slice, fresh context. **RE-SEQUENCED (2026-07-13, evidence-based):
  the 3-edit checker removal was ATTEMPTED and REVERTED — measured blast radius = 9 differential
  examples + 7 checker/JIT tests, and critically the empty-`[]` sites are DOMINATED by (a) the WEB
  examples (router/middleware/controller/route-constraints/router-attrs) that DEC-218 EXTERNALIZES and
  (b) the Core.Sql PRELUDE (the sqlbuild/union-dyn JIT tests broke — the prelude uses empty `[]`) that
  DEC-208 EXTERNALIZES. Doing part-2 now = prelude surgery + double-churn on code about to leave the
  language. CORRECT ORDER: DEC-208 (Sql prelude → userland) + DEC-218 (web spine → userland) FIRST,
  THEN part-2 codemods only the small remaining general-purpose empty-`[]` set. Part-2 depends on
  DEC-208/DEC-218.)* **PART-2 SHIPPED (2026-07-14, developer override of the resequencing — done
  now, accepting the web/Sql-prelude double-churn):** a bare empty `[]` is rejected everywhere with
  `E-EMPTY-LITERAL` ("an empty collection needs its type") — one `err_empty_literal` helper wired to
  the three typing sites (`check_list`, `thread_literal_expected` for decl/return, `check_arg` for
  call args; the former bidirectional empty-`[]`→`List<T>` arg case is gone). No `List.empty`/`Map.empty`
  factory ever existed (nothing to remove). `desugar_router`'s synthesized `new Router([], [])` now
  emits `Expr::NewColl` with the ctor's exact `List<Route>` / `List<mw>` types. Codemod (mechanical,
  by class): HTTP prelude 3 sites; `examples/**` 9 sites (web + guide); `conformance/web/**` 12 sites;
  Rust `.phg` fixtures 8 sites (differential + checker/JIT tests); the gitignored `var/phorj-app`
  bench 2 sites. `phg explain E-EMPTY-LITERAL` added. The lifter still emits `[]` for an untyped PHP
  `[]` (no type context in PHP source) — noted in KNOWN_ISSUES, not gate-exercised.*
- **DEC-215 — DI stays compile-time; L1/L2 refactor affirmed, scheduled Ω-4/Ω-7.** DI v1 is a 1292-LOC
  bespoke COMPILER pass (`desugar_di/`, pre-check, `Expr::Inject`) — the same "app framework privileged
  into the compiler" category as the ejected SQL builder (DEC-208). The spec's own ruling stands: build a
  generic L1 attribute-reflection primitive (compile-time attribute enumeration + `subjectsWith<Attr>()`
  discovery) and rewrite DI as an L2 consumer (routing/ORM/validation ride the same L1). DI MUST remain
  compile-time — a pure-runtime `.phg` DI library is infeasible (`inject<T>()` is type-directed and PHP
  erases types → byte-identity break). Execute at the SCHEDULED wave (Ω-4/Ω-7); DI v1 stays as-is until
  then (green, contained). *Alternatives:* pull the L1/L2 refactor forward now (rejected — reorders ahead
  of priorities, ~1300 LOC, buys nothing while DI v1 works); keep DI compiler-baked permanently (rejected
  — contradicts the spec's L1/L2 ruling + the DEC-208 principle).

**Session meta-rulings (2026-07-13):**
- **META-4 — unify ALL plans/specs into the two SSOTs** (developer, mid-session): `MASTER-PLAN.md`
  (roadmap) + `UNIFIED-SPEC.md` (surface) + this register (decisions). No standalone plan/spec files;
  the language-reconsideration working plan is folded into MASTER-PLAN and retired.
- **META-5 — session certification is self-graded + disclosed** (advisor inactive: advisor==main==Opus 4.8).
- **META-6 — GOVERNING PHILOSOPHY (developer, 2026-07-13): rich core, zero-cost safe sugar, no bloat.**
  The language is RICH — it does everything PHP does, **better / faster / safer / more secure** — plus
  **safe sugar that must NOT affect performance** (zero-cost or it doesn't ship). It is deliberately
  **NOT bloated**: anything that should be a library IS a library, never baked into the language. Every
  feature is adjudicated through the **"in-language vs externalize" lens** — IN if it is a core
  capability that beats PHP or is zero-cost safe sugar; OUT (library / separate tool) if it is an
  application-domain or packaging concern. Applications so far: DEC-208 (SQL builder → userland),
  DEC-215 (DI → L1/L2 library), DEC-216 (package management → separate). Refines the craftsmanship apex
  filter (`memory/philosophy-of-phorge.md`). **Next design activity: a systematic feature-by-feature
  in-language-vs-externalize audit of the current surface.**
- **DEC-216 — PENDING (developer lean, 2026-07-13): package management is SEPARATE from the language.**
  ⊳ RESOLVED by DEC-316 (2026-07-20): built as `phg` subcommands — softens the "companion tool" lean, approved at plan-exit.
  `phg vendor` + `phorj.toml` should likely leave the language — "the language does not need to handle
  package management; it needs to be separate." Ladder to adjudicate (present with previews, recommended
  first): (1) **remove entirely** — no dependency mechanism in `phg` at all; (2) **dumb `vendor/`
  consumption** — `phg` still resolves imports from a pre-populated `vendor/` dir (offline), but the
  fetch command + manifest leave `phg` to an external companion tool; (3) **external tool owns
  everything** (manifest + fetch + vendor); `phg` is package-agnostic. Impacts `examples/project/withdeps`
  + `src/loader/` + `src/manifest.rs`. Blocks nothing; adjudicate after DEC-214.
- **IN-LANGUAGE-vs-EXTERNALIZE AUDIT (2026-07-13, 4-agent sweep — full doc `docs/research/2026-07-13-externalize-audit.md`).**
  Applied META-6 to the whole surface. KEEP-CORE: stdlib primitives + native-backed app primitives
  (Crypto/File/Path/Process/Env/Reflection/Runtime/Url/Secret/Db/Csv/Ini) + language capabilities +
  zero-cost sugar + the language toolchain (transpile/format/test/…). EXTERNALIZE candidates (ranked):
  package-mgmt (DEC-216), Http (→primitive+userland), DI (DEC-215), **desugar_router (NEW — a 489-LOC
  web-framework compiler pass, peer to DI; same DEC-215 L1/L2 treatment)**, serve, lift, lsp, Time
  (calendar→lib, keep clock), Validation, html (DEC-212), Dotenv/Event/Cli/Log/Uuid/Sessions/Serde/
  Template (→userland), debug/DAP. New PENDING adjudications surfaced:
  - **DEC-217 — PENDING: Test framework in-language or userland?** Genuine tie — PHPUnit is PHP
    *userland* (externalize) vs Rust/Go ship a *built-in* runner (keep). Surface with both precedents.
  - **DEC-218 — PENDING: externalize DELIVERY destination** — userland (DEC-208 style) vs first-party
    bundled lib (DEC-212 style). **Must be ruled WITH DEC-216** (if packaging is removed, a "userland"
    web spine has no distribution path).
  - **DEC-219 — PENDING: overloading dispatch** — resolve statically where arg types are known
    (zero-cost) vs current runtime multiple-dispatch (per-call cost); a META-6 zero-cost-sugar tension.
  Suggested ruling order: DEC-216+DEC-218 together → DEC-215 family (DI + desugar_router) → per-module
  moves (Http/Time/Validation) → DEC-217 → DEC-219. Every move a tracked, tested, register-recorded slice.

**Audit adjudications RULED (2026-07-13 batch 2, developer via AskUserQuestion):**
- **DEC-216 — RULED: SPLIT.** phg KEEPS import/module resolution + offline `vendor/` consumption (it is
  the language's import system); `phg vendor` fetch + `phorj.toml` + lock MOVE to a separate companion
  tool (rustc/cargo, go/`go mod` model). The language stays package-agnostic (no network, no manifest);
  userland libs still work (the tool populates `vendor/`, phg consumes it offline). *Alternatives:* remove
  entirely (kills third-party libs); keep in phg (the rejected status quo). Impacts src/manifest.rs +
  src/lock.rs + the vendor subcommand (extract to the tool) — loader's resolution stays.
- **DEC-218 — RULED: userland libraries + Core primitives** (consistent with DEC-208). Externalized web
  spine (Http/router/sessions, Dotenv/Event/Cli/Log/Uuid/Serde, Template, SQL builder) ships as USERLAND
  libraries via the DEC-216 vendor path; Core keeps only the thin primitive each rides. **Http-primitive
  note (developer):** the Core HTTP primitive must expose HTTP verbs (GET/POST/HEAD/…) + request
  bodies/file uploads in a **clean, well-organized OOP** way (not a flat function bag). *Alternatives:*
  first-party bundled libs (curated but phg-adjacent); keep-in-Core (bloat).
- **DEC-217 — RULED: keep `phg test` built-in** (Rust/Go toolchain precedent; phg's byte-identity
  discipline is testing-centric — a first-class runner is core identity, not bloat). *Alternative:*
  userland test lib (PHPUnit precedent) — declined.
- **DEC-219 — RULED: static overload resolution** — the checker picks the overload at compile time when
  argument types are statically known (zero-cost direct call); runtime multiple-dispatch remains ONLY for
  genuinely union-typed args. A META-6 zero-cost win, no surface change. *Alternative:* always-runtime
  dispatch (per-call cost) — declined. ⚠ soundness: subtype refinement can make runtime dispatch ≠ static
  selection (`f(Animal)`+`f(Dog)`, arg static `Animal` holding a `Dog`) — the sound subset is where no
  runtime refinement can change the selection (safe approx: primitive/leaf arg types). Deferred (low
  priority vs the DB/output work).
- **DEC-222 — RULED (autonomous, parallel to DEC-221): throwing-closure function types.** A function
  TYPE and a lambda literal could not carry a checked exception, so a closure that did `x?` / `throw new
  E(...)` hit `E-THROW-UNDECLARED` (a lambda body was always checked with an EMPTY `cur_throws`), and a
  call of a function VALUE discharged nothing — blocking the closure form `db.transaction(() => {…})`.
  Ruling: **add a `throws` component to the function type `(A) => B throws E` and to the lambda literal
  `(x): T throws E => …`**, the exact parallel of DEC-221 (throwing constructors) for callables. A lambda
  DECLARES its throws (explicit clause — no inference, matching named functions/ctors which declare not
  infer); its body is checked with those throws in `cur_throws`; a call of a `throws E` function value
  routes E through `route_call_throw` so the caller must handle/propagate (`E-CALL-UNHANDLED`). *Variance
  (the sound rule chosen):* a function that throws FEWER exceptions is substitutable where one throwing
  MORE is expected — `from ⊑ to` iff params/ret match (exact, spec A6) AND every exception in `from`'s
  throws is `<:` some member of `to`'s throws (using the nominal subtype oracle). So a plain `() => T`
  (throws nothing) passes where `() => T throws E` is expected; the reverse is rejected. *Alternatives:*
  contextual/expected-type throws inference for a clause-less lambda (rejected — the expected-type
  threading is not wired into `check_args`, and inference of a throws set from a body is a larger,
  riskier feature; explicit declaration is the DEC-221-parallel, lower-risk path); no variance / exact
  throws match (rejected — a non-throwing lambda then could not pass where a throwing type is expected,
  the required capability). *Scope note:* throws on a bare function-TYPE annotation are resolved but not
  Error-validated (validation happens at the lambda DEFINITION site, `check_lambda`, like a fn/ctor decl).
  Discharge covers both callable-value paths — a function-typed LOCAL/PARAM `f(x)` (`calls/core.rs:26`)
  and a general callee expression `(expr)(x)` (`calls/core.rs` `other` arm). A function-typed FIELD call
  (`this.op(x)`) is not a reachable path — phorj already rejects it as `no method` before throws is
  considered — so no discharge site is needed there. Checker/parser-only — no runtime change (the throw
  is the existing `Op::Throw`), so byte-identical (`run ≡ runvm ≡ php`).
- **DEC-208 slice C closure form — SHIPPED (2026-07-14, unblocked by DEC-222).** The closure form
  `db.transaction(function(): T throws DbError { … })` + retry, previously BLOCKED (KNOWN_ISSUES) on the
  lambda-can't-throw limitation DEC-222 fixed. Built: a `HigherOrder` native `DbSys.transaction(handle,
  fn)` — one attempt: BEGIN, invoke the closure re-entrantly, COMMIT on `Ok` (return the closure's
  value), ROLLBACK + re-propagate the ORIGINAL thrown value on the invoker's `Err`. Throw preservation
  is the load-bearing part: a closure throw arrives as `Err(THROW_SENTINEL)` with the thrown value in the
  backend's `pending_throw`, and `rollback_inner` runs pure `rusqlite` (never re-enters the backend), so
  `pending_throw` survives and returning the same `Err` unchanged lets the backend rebuild the exact
  typed `DbError` — the caller catches the original, not a generic error. A nested `db.transaction` is a
  SAVEPOINT (reuses the slice-C `tx_depth`). The manual `begin`/`commit`/`rollback`/`rollbackQuiet` stay
  (developer ruled BOTH). Retry loop lives in the PRELUDE (`db.transactionRetry`) because only phorj
  source can `catch` the TYPED `SerializationFailure` (`pending_throw` is invisible to a native).
  - **PENDING adjudication (Invariant 15) — retry SURFACE.**
    ⊳ RESOLVED by DEC-249: the retry surface is `db.transaction(fn, int retries = 0)` (method default
    params built); `transactionRetry` RETIRED — label flipped 2026-07-28, consistency audit.
    The spec (§5) illustrates one method
    `db.transaction(retries: N, fn)`, but the language supports NEITHER named args, NOR method default
    params, NOR generic-method overloading — three independent walls that make a single generic
    `transaction` carrying an optional `retries` impossible. Realized as a distinct
    `db.transactionRetry(fn, retries)` (retries trailing, positional). *Alternatives (all unbuildable):*
    (a) `transaction(fn, retries = 0)` — `E-DEFAULT-PARAM-CONTEXT` (methods can't default); (b)
    `transaction(fn)` + `transaction(fn, retries)` overload — `E-OVERLOAD-GENERIC` (generic methods can't
    overload); (c) `transaction(retries: N, fn)` — no named args. Developer to confirm the final
    name/shape. Isolation-arg retry (`db.transaction(Isolation.Serializable, fn)`) rides with the
    deferred isolation slice. Example `examples/database/transaction-closure.phg`; `tests/database.rs`; both backends.
- **DEC-221 — RULED (ASKED 2026-07-13): throwing constructors.** phorj constructors could not declare
  `throws` (a `constructor(...) throws E` was a parse error; a throwing call in a ctor body had no
  `?`/try escape), which forced DEC-208's fail-able open into a static factory `Db.connect(dsn)` —
  deviating from the ruled `new Db(dsn)`. Ruling: **make constructors able to declare + propagate
  `throws`** so `new Db(dsn) throws DbError` works, exactly as ruled and exactly like PHP's `new PDO`
  (fail-fast + PHP-faithful + enriches ALL fallible construction, not just Db). *Alternatives:* keep the
  `Db.connect` factory (rejected — permanent deviation from the ruling + PHP; the "named constructor"
  idiom is clean but not what was ruled); lazy-open to preserve `new Db` syntactically (rejected —
  fail-LATE, a bad DSN constructs "fine" and errors on first use, disconnecting error from cause).
  **Impl:** (1) AST — add `throws: Vec<Type>` to `ClassMember::Constructor` (`ast/decls.rs:189`; ~60
  match/construct sites, most use `..`). (2) Parser — parse an optional `throws` clause (reuse
  `parse_throws_clause`, `parser/types.rs:31`) between `)` and `{` at BOTH ctor parser sites
  (`parser/items/types.rs:318`, `parser/items/decls.rs:423`). (3) Checker — store the ctor throws on the
  class's ctor signature (`collect/types_decls.rs` ctor build); check the ctor BODY with those throws in
  context (so its throwing calls discharge, like `check_function`); at `check_new` (`expr/core.rs:252`)
  route the ctor's throws to `route_call_throw` so `new X(...)` is a throwing expression the caller must
  handle/propagate. (4) Formatter — render `throws` on ctors. (5) Then convert DB_PRELUDE `Db.connect`
  back to `constructor(string dsn) throws DbError { this.raw = match(...){...} }` + example `new Db(dsn)`.
- **DEC-220 — RULED (ASKED 2026-07-13): unified output/log/response system (Output/Log/Response), 3 named
  sinks + opt-in capture.** Prompted by a real bug the dev hit: `Output.print*` in a `phg serve` handler
  goes to the SERVER LOG (stderr), not the browser (`serve/handlers.rs:182`) — a context-magical redirect
  (stdout in CLI, stderr-log in serve). The challenge (accepted): the fix is EXPLICIT NAMED sinks, not
  making `Output` more ambient. Ruling — three context-independent sinks:
  (1) **`Output.*` → process STDOUT, always** (CLI). The serve-only Output→stderr redirect is REMOVED.
  (2) **`Log.debug/info/warn/error(msg)` → structured, leveled STDERR** — first-class server/app logging
  (beats PHP `error_log`). New `Core.Log` module.
  (3) **`Response.html/text/json/bytes(..).status(n).withHeader(k,v).withCookie(..)` → the browser** — a
  typed builder; headers-before-body enforced structurally (PHP's "headers already sent" impossible).
  PLUS **`Response.capture(() => { Output.printLine(..) })`** — opt-in PHP-like echo-into-body within an
  EXPLICIT scope (no ambient state; combines the "explicit builder" + "capture block" options).
  Ties into DEC-218 (Core.Http/Response + Log as thin Core primitives; richer helpers userland). Byte-id:
  Log→stderr is invisible to the stdout differential; Response is a value (the portable `handle(Request):
  Response` unit). *Alternatives:* ambient echo (Output writes to the response in a handler — REJECTED,
  implicit ambient sink + PHP header/buffer footguns); leaner 3-sinks WITHOUT capture (REJECTED — dev
  wants the opt-in ergonomic); keep the current serve Output→stderr magic (REJECTED — the reported bug).
  *Build (fresh context, multi-slice):* S1 `Core.Log` (leveled natives → stderr; additive, self-contained)
  · S2 `Response` builders (`.html/.text/.json/.status/.withHeader/.withCookie`) replacing raw
  `new Response(status,bytes,headers)` + remove the serve Output→stderr redirect (Output stays stdout) ·
  S3 `Response.capture(fn)` opt-in buffering. Each = Invariant-9 example + gate.
  *STATUS:* S1 SHIPPED (`Core.Log`). **S2 SHIPPED** (2026-07-14): `Response.html/json` + immutable
  `.status(n)`/`.withHeader(k,v)`/`.withCookie(k,v)` in `HTTP_PRELUDE`; serve `respond_once` now sends a
  handler's captured stdout to the server's real STDOUT (was stderr); example
  `examples/web/response-builders.phg` byte-identical `run`≡`runvm`≡php-8.5.8; full gate green.
  **S3 SHIPPED (2026-07-14): `Output.capture(fn): string`, an import-gated primitive (option (d) — the
  ruled `Response.capture` prelude wrapper was dropped, it had no leak-free path). Detail in DEC-220-S3 below.
  DEC-220 now fully shipped (S1+S2+S3).**
- **DEC-220-S3 — SHIPPED (2026-07-14, option (d) ruled by the dev): `Output.capture(() -> void) -> string`,
  an explicit IMPORT-GATED capture primitive — no leak.** The ruled `Response.capture` PRELUDE wrapper was
  DROPPED because its only path to the native (`import Core.Output` inside `HTTP_PRELUDE`) leaked `Output.*`
  into every `import Core.Http.Response` program (the "nothing in the wind" violation recorded below). The
  shipped surface is the primitive `Output.capture(fn)` reachable ONLY via the user's own `import Core.Output;`
  (the same import `Output.printLine` already needs); the ruled `Response.capture` shape is expressed by
  WRAPPING it — `Response.html(Output.capture(() => { … }))`. No prelude / `CORE_MODULES` code changed, so
  `Output`'s reachability is byte-for-byte identical to HEAD; a leak-probe test (`checker::tests::output_capture`)
  proves both legs: `Output.capture` resolves under `import Core.Output`, and a program importing ONLY
  `Core.Http.Response` still gets `E-UNKNOWN-IDENT` for bare `Output`. *Deviation from the ruled surface,
  noted per the ruling:* the capture entry point is `Output.capture(fn): string` + a manual `Response.html(...)`
  wrap, NOT a `Response.capture(fn): Response` prelude method — because that wrapper had no leak-free path.
  *Implementation (all sites, as the prior proof predicted):* new `NativeEval::Capturing` variant +
  `CapturingInvoker` type (`native/mod.rs`); `output_capture` native (`Core.Output.capture`, `pure:true`,
  params `[() -> void]`, ret `string`); interpreter arm (`interpreter/call.rs`) + VM arm (`vm/exec.rs`), both
  doing `out.split_off(start)` in the backend invoker (the one spot holding both `out` and the closure runner);
  transpile gated helper `__phorj_capture($fn){ ob_start(); $fn(); return ob_get_clean(); }`
  (`transpile/{mod,call,runtime_php}.rs`) + `ob_start`/`ob_get_clean` added to `TIER1_PHP`
  (`tests/differential.rs`); example `examples/web/response-capture.phg` (byte-identical `run`≡`runvm`≡php-8.5.8,
  formatter-idempotent) + `examples/README.md` row. The gated byte-identity claim covers the happy path only
  (a printing, returning closure). A LAMBDA cannot introduce a mid-capture throw (a lambda literal can't declare
  `throws` — parse error — and a throwing lambda body is `E-THROW-UNDECLARED`, both verified), but a NAMED
  throwing function CAN be passed by reference (`Output.capture(boomer)`, verified type-checks). On such a throw
  `run`≡`runvm` still holds on every path (both backends leave the partial output in `out` and never `split_off`
  on a fault; the interpreter/VM throw-sentinel handling is kept for parity with the higher-order path); the PHP
  leg leaves `ob_start` dangling until script-end auto-flush — byte-matches in the simple propagate-and-catch
  case (verified) but not guaranteed for nested shapes, so this path is kept out of the byte-identity example set
  and recorded in `KNOWN_ISSUES.md` (like the non-finite `sprintf` divergence). Full gate green: build + clippy (default /
  `--no-default-features` / `--features db`, warnings deny) + `fmt --check` + `PHORJ_REQUIRE_PHP=1 nextest
  --features jit` (1993 passed). *DEC-220 now fully shipped (S1+S2+S3).*
- **DEC-220-S3 — [SUPERSEDED by the SHIPPED entry above] PENDING (autonomous, 2026-07-14): `Response.capture`
  forces a new ambient name via the
  prelude.** A working, byte-identical (`run`≡`runvm`≡php-8.5.8) implementation was built and then
  REVERTED (not shipped) because it violates the hard "nothing in the wind" rule. Mechanism: `Response`
  lives in `HTTP_PRELUDE`; for its static `Response.capture` to call the capture native it must resolve
  `Output.capture`, and phorj has NO fully-qualified `Core.Output.capture(...)` call form (that parses as
  `unknown identifier Core`) — the only way is `import Core.Output;` in the prelude. But prelude top-level
  imports MERGE into user scope (a pre-existing behavior: `import Core.Http` already makes
  `Bytes`/`String`/`List`/`Regex` resolvable without the user importing them), so adding `import Core.Output`
  makes `Output.*` resolvable in ANY program that does `import Core.Http.Response` alone. Embedded evidence
  (the leak, minimal): a program with `import Core.Http.Response;` + `Output.printLine("x")` in `main`
  type-checks and runs (Output resolves) ONLY when the prelude imports Core.Output; with zero imports
  `Output` is correctly `unknown identifier`. *Options for the developer:* (a) ACCEPT the leak as consistent
  with the existing 4-module prelude-transitive-import behavior (batteries-included facade); (b) REJECT it;
  (c) the real fix — scope prelude imports so they do NOT merge into user scope (also removes the 4
  pre-existing leaks, but changes shipped behavior → riskier); (d) sanction `Output.capture(() -> void) ->
  string` as an explicit, import-gated PRIMITIVE (user writes `import Core.Output;` themselves → no leak) and
  drop the prelude `Response.capture` wrapper (deviates from the ruled surface). *Implementation that was
  proven (ready to re-apply once ruled):* new `NativeEval::Capturing` variant + `CapturingInvoker` type
  (`native/mod.rs`); `output_capture` native (`Core.Output.capture`, `pure:true` — byte-identical like
  `List.map`); interpreter arm (`interpreter/call.rs`, mirrors the HigherOrder throw structure) + VM arm
  (`vm/exec.rs`), both doing `out.split_off(start)` to divert the closure's output; transpile gated helper
  `__phorj_capture($fn){ ob_start(); $fn(); return ob_get_clean(); }` (`transpile/{mod,call,runtime_php}.rs`)
  + `ob_start`/`ob_get_clean` added to `TIER1_PHP` in `tests/differential.rs`; prelude static
  `Response.capture((() -> void) render): Response { return Response.html(Output.capture(render)); }`; example
  `examples/web/response-capture.phg`. Recommended: (d) if a capture surface is wanted now without the
  architectural change, else (a) to ship `Response.capture` as ruled.

## 2026-07-15 mailer + quarantine-reopen batch (Opus run — developer via AskUserQuestion; DEC-223 RULED build-pending, DEC-224/225/226 REOPENED-PENDING for the Fable handover)

Developer idea: "we need a native mailer too." Full research/brainstorm ran (twin-of-Core.Db
architecture). Self-graded certification (advisor==main==Opus). The mailer is RULED and locked to a
spec (`docs/specs/archive/2026-07-15-core-mail.md`); build handed to Fable. Alongside it, the developer asked
for the full non-transpilable inventory and chose to REOPEN three native-only rulings — recorded here
as PENDING (NOT re-ruled this session, per the developer's "just note all of this and hand to Fable").

- **DEC-223 — native mailer `Core.Mail` (RULED, build-pending; full spec `docs/specs/archive/2026-07-15-core-mail.md`).**
  ⊳ BUILT since — `src/ext/mail/{tests,handles,natives,mime}.rs` + `lettre` in the registry (label flipped 2026-07-28, consistency audit).
  A native email primitive, architecturally a **twin of Core.Db** (DEC-208): native-only, spine-quarantined
  (`pure:false` natives → `uses_impure_native` excludes it from `differential.rs`), tested against the
  stack's **Mailpit** faker + deterministic `file`/`null` transports. **LADDER (invariant 14) = case 2,
  native-only:** transpile is a HARD ERROR `E-TRANSPILE-MAIL` — PHP's stdlib `mail()` has no SMTP auth,
  no TLS, and is header-injection-prone, so there is no faithful safe PHP map and any attempt (e.g.
  text-only→`mail()`) would silently drop auth/TLS/attachments (a rule-14-forbidden downgrade). Mailer
  joins the `E-TRANSPILE-*` list (concurrency/unchecked/mongo). **Transports** (behind a `MailTransport`
  trait, mirroring the Db driver trait): **SMTP with OPTIONAL auth** (Mailpit/MailHog fakers accept
  no-credential connections) · **sendmail** (local MTA) · **file** (`.eml` → dir, deterministic offline
  tests) · **null** (dry-run/discard). **Composition** = full rich surface: `new Email()` builder
  (`from`/`to`/`cc`/`bcc`/`replyTo`/`subject`/`text`/`html`), `.html(body)` **auto-derives a plaintext
  alternative** (`multipart/alternative`), `.attachInline(cid, img)` inline CID images, `.attach(file)`
  attachments; typed injection-safe `Address` (no raw-header injection possible — the #1 PHP `mail()`
  footgun), TLS-by-default, credential **`Secret`** (the same Secret from Core.Db driver slice G),
  RFC-correct MIME. **Typed `MailError` taxonomy** (ConnectionFailed / AuthFailed / RecipientRejected /
  TlsError / …), shaped like `DbError`, via the same prelude-wrapper `MailResult<T>` Ok|Err mechanism.
  **Dependency amendment — ADMIT `lettre`** (feature `mail`, non-default, non-wasm): the mature de-facto
  standard, RFC-correct MIME/multipart, SMTP auth, STARTTLS/implicit TLS via already-admitted **rustls**,
  optional **DKIM** signing, and crucially a **blocking `SmtpTransport`** so it stays **no-tokio**.
  `lettre = { version="0.11", default-features=false, features=["smtp-transport","rustls-tls","builder","dkim"] }`.
  *Alternatives:* `mail-send`+`mail-builder` (Stalwart) — modern, extremely RFC-correct, DKIM built-in,
  but **tokio-async** → pulls tokio, violates the no-tokio policy (rejected for that reason); hand-roll
  SMTP+MIME over std+rustls (rejected — large RFC/MIME/encoding bug surface lettre already gets right);
  transpile trivial text emails to PHP `mail()` (rejected — silent downgrade, rule 14).
- **DEC-224 — REOPENED (PENDING, for Fable): MongoDB.** Developer chose to reopen MongoDB rather than
  leave it a deferred future LADDER item. Current status: NOT built; documented `E-TRANSPILE-MONGO`
  candidate (non-SQL, no PDO analog, async-driver problem). To decide with Fable: native-only driver
  shape (twin-of-Db, spine-quarantined, `E-TRANSPILE-MONGO`) vs continue deferring. NOT re-ruled this
  session.
- **DEC-225 — REOPENED (PENDING, for Fable): concurrency PHP leg.** Developer chose to reopen whether
  `spawn`/channels (green threads, DEC-133) should attempt any PHP mapping. Current status: no PHP leg
  (`E-CONCURRENCY-NO-PHP` hard error; no opt-in flag exists — DEC-369 deleted the phantom
  `--sequential-concurrency` from the rule). ⚠ Any PHP mapping serializes the
  program silently — a rule-14 downgrade risk to weigh. NOT re-ruled this session.
- **DEC-226 — REOPENED (PENDING, for Fable): `#[UncheckedOverflow]` transpile.** Developer chose to
  reopen whether unchecked wrapping arithmetic should try a PHP map. Current status: hard error
  `E-TRANSPILE-UNCHECKED` (PHP overflows int→float — no faithful wrapping-int mapping exists). NOT
  re-ruled this session.

## 2026-07-15 fable overnight run — AUTO-RULED batch (bounded autonomy, developer-approved protocol; every entry REOPENABLE, mirrored in KNOWN_ISSUES §"Fable overnight run — morning triage")

- **DEC-227 — AUTO-RULED (REOPENABLE): `db` becomes a DEFAULT cargo feature + clean feature-gating
  errors.** Found by the run's first review probe: the stock binary (default features) could not run
  ANY `Core.Db` program — `import Core.Db` produced a ~100-line wall of prelude-internal
  `E-UNKNOWN-IDENT` errors (the prelude classes reference `DbSys` natives that don't exist in a
  db-less build). Risk example: `phg run app.phg` on the shipped binary, where `app.phg` is the
  documented `examples/database/basic.phg` — unusable with an incomprehensible error. RULED: (1) `db` joins
  the default feature set (PHP ships PDO by default; a batteries-included DBAL absent from the stock
  binary contradicts the 2026-07-11 vision ruling); (2) importing a feature-gated Core module on a
  build without that feature = ONE clean `E-MODULE-UNAVAILABLE` diagnostic (registry
  `GATED_CORE_MODULES`, preludes.rs); (3) transpiling a `Core.Db` program = clean `E-TRANSPILE-DB`
  ladder error on BOTH transpile entries (rule-14 leg 2 — was the same unknown-ident wall).
  *Alternatives:* keep `db` opt-in with only the clean errors (rejected: parity mandate — PHP's PDO
  is default); silently strip Db calls on transpile (FORBIDDEN, rule 14 leg 3). Build-time cost of
  bundled SQLite accepted (one-time, cached). `db-postgres` stays opt-in (network dep).

- **DEC-228 — AUTO-RULED (REOPENABLE): Db streaming surface (item H) = `RowStream` + generic
  `DbStream<T>` with hydrate-on-pull closure; cursor materializes today (disclosed).** Surface:
  `stmt.stream(): RowStream` (`next(): Row?`, null = end) and `stmt.streamInto<T>(): DbStream<T>`
  (`next(): T?`, LAZY — hydration runs per pulled row via a DEC-222 throwing closure synthesized by
  `desugar_db` from the same `build_class` machinery as `queryInto`; turbofish + contextual sinks;
  naming strategies apply). Risk example: `var s = stmt.streamInto<User>(); User? first = s.next();`
  — only the first row is ever hydrated (a later broken row throws NOTHING unless pulled; proven by
  `db_stream_into_hydrates_lazily_early_exit_skips_bad_rows`). *Disclosed limit:* both drivers
  materialize the result set at `stream()` (rusqlite/postgres iterators borrow their statement —
  self-referential lifetime, unavailable under `#![deny(unsafe_code)]`); the surface contract is
  delivery + lazy hydration, drivers upgrade underneath. *Alternatives:* self_cell/ouroboros dep for
  true incremental stepping (rejected: new unvetted dep for an internal perf property); thread+channel
  per cursor (rejected: heavyweight, Connection not Sync); defer streaming entirely (rejected: queue
  item H, unblocks the one-Iterator-protocol seed). NOT ruled: for-in over streams (the Data-pillar
  Iterator-protocol slice — a REAL adjudication for the developer, queued).
- **BUG FIX (en route, rule 14): `rewrite_html` walker-totality — `Expr::New` was un-walked.** Every
  span-keyed checker rewrite (throws-`?` erasure, `html"…"` holes, tagged templates) SKIPPED anything
  nested in `new C(args)`: first live trigger = a throwing lambda with `?` in ctor args (the DbStream
  hydration closure) — checker accepted it, VM rejected it as Result-mode `?`, interpreter faulted at
  runtime ("`?` requires a Result value"). Sibling walkers audited (rewrite_ufcs / desugar_router /
  resolve_variant_imports / intrinsic_imports all have New arms — rewrite_html was the sole hole).
  Pinned by `conformance/errors/lambda-in-ctor.phg` on all three backends.

- **DEC-229 — AUTO-RULED (REOPENABLE): `mysql` crate admission (10th external-dependency domain) +
  the slice-J MySQL/MariaDB driver + slice-K Postgres array mapping.** The 2026-07-03 amendment
  already RULED the three-driver SQL DBAL (SQLite + Postgres + MySQL sync) — this realizes the
  remaining admission: `mysql` v28 under `minimal-rust` (pure-Rust wire protocol, no libmysqlclient,
  no TLS/compression/chrono extras; `unsafe` internal to the dep — the rusqlite/postgres criterion),
  feature `db-mysql` (non-default, non-wasm, implies `db`; `db-all` extended). Driver divergences
  handled explicitly (Invariant 14): no RETURNING (id via `last_insert_id`, SQLite-shaped) ·
  standalone SAVEPOINT rejected under autocommit (BEGIN at depth 0, Postgres-shaped) ·
  `max_execution_time` ms with MariaDB `max_statement_time` seconds fallback · DECIMAL→exact-text ·
  TEXT-vs-BINARY blob split on BINARY_FLAG · temporal steering to CAST(col AS CHAR). En route:
  `redact_dsn_password` hoisted to db/mod.rs (shared) and `Db.withPassword` now injects into
  mysql/mariadb DSNs (was a SILENT NO-OP on non-postgres URL DSNs — a slice-G footgun killed).
  Slice K: Postgres bool/int/float/text ARRAY columns → `Value::List` + STRICT typed accessors
  `Row.get{Int,String,Float,Bool}List[OrNull]` + `List<scalar>` hydration fields/queryScalar sinks
  route there via accessor_for. *Alternatives:* `mysql_async` (rejected: tokio at the API);
  `diesel`/`sqlx` (rejected: whole-framework deps vs a driver); defer J (rejected: ruled driver set,
  README already promises it). Risk example: `new Db("mysql://app@db:3306/prod")` previously fell
  through to the SQLITE FILE PATH driver (opening a local file literally named the DSN!) — now a
  clean feature-gated ConnectionError or the real driver.

- **DEC-230 — AUTO-RULED (REOPENABLE): Core.Mail surface realizations where the locked spec exceeded
  the language.** (1) `new SmtpConfig(host, port, user, Secret pw)` → static factory
  `SmtpConfig.withAuth(host, port, user, Secret)` and `new SendmailTransport()` path override →
  `SendmailTransport.at(path)`: phorj has NO constructor default params (probe: parse error at
  `constructor(... string user = "")`) and no ctor overloading — LANGUAGE GAP flagged in
  KNOWN_ISSUES for the sugar wave (functions have defaults; ctors don't — an inconsistency).
  (2) Taxonomy subtypes `Timeout`/`Io` realized as `MailTimeout`/`MailIo` — bare `Timeout` already
  belongs to Core.Db's injected taxonomy and two injected classes may not collide (risk example:
  `import Core.Db; import Core.Mail;` in one program — both preludes inject). (3) `Address.of(email)`
  static = the display-name-less form. (4) SMTP TLS = STARTTLS-opportunistic default (fakers work,
  TLS used when offered); implicit-TLS config knob QUEUED (real adjudication: config surface shape).
  *Alternatives:* per-field builder on SmtpConfig (more chatty); a Db-style union ctor arg for auth
  (rejected — a Secret-bearing variant reads worse). En route: `all_examples_transpile_and_match_php`
  gained the generic `E-TRANSPILE-*` ladder-skip arm; the differential run≡runvm glob gained the
  feature-gated-module skip via the new `phorj::cli::unavailable_gated_modules()` seam.

- **DEC-224 — AUTO-RULED (REOPENABLE): MongoDB = admission SHAPE ruled, build DEFERRED behind the
  value-ordered packs.** Ruled shape (so the reopen is a decision, not a re-deferral): the official
  `mongodb` crate's SYNC API is the admissible candidate — its blocking wrapper over an internal
  tokio runtime is EXACTLY the postgres-crate precedent the dependency policy already admits ("the
  crate's async usage is its internal impl detail; the phorj-facing API stays sync"); surface =
  twin-of-Db document store (`Core.Mongo`: typed `MongoError` taxonomy, Secret credentials,
  `findInto<T>` hydration reusing the desugar machinery); LADDER case 2 native-only
  (`E-TRANSPILE-MONGO` — no PDO analog). Build deferred tonight because: heavyweight dep tree (full
  tokio) for a niche driver, no in-tree faker to gate against (Mailpit/SQLite-style), and the
  value-ordered mandate puts web/data-pillar packs ahead. Risk example: none live — no program can
  reach Mongo today; the DEFER costs only absence, never wrongness. *Alternatives:* build tonight
  (rejected: value order); reject permanently (rejected: developer explicitly reopened toward
  having it); hand-rolled wire protocol (rejected: enormous, the lettre-hand-roll argument).
- **DEC-225 — AUTO-RULED (REOPENABLE): concurrency PHP leg stays E-CONCURRENCY-NO-PHP; PHP FIBERS
  recorded as the ruled faithful-candidate upgrade path.** Any eager serialization mapping silently
  reorders interleaved effects (rule-14 downgrade — confirmed FORBIDDEN). NEW in this ruling: PHP
  8.1 Fibers are cooperative single-threaded coroutines — the SAME concurrency model as phorj's
  corosensei green threads — so a transpile emitting a deterministic round-robin Fiber scheduler
  (mirroring `green::sched`'s order exactly) is a PLAUSIBLE byte-identical mapping, the first
  candidate that does not downgrade semantics. Queued as its own future slice: spike = 3 programs
  (spawn/join, channel ping-pong, select) hand-mapped to Fibers, byte-compared before any emitter
  work. Until that spike proves order-identity, the hard error stands (never silently). Risk
  example: `spawn a(); spawn b();` with interleaved prints — eager mapping prints a-then-b where
  the VM prints the interleaving; Fibers with a mirrored scheduler print the interleaving.
- **DEC-226 — AUTO-RULED (REOPENABLE): `#[UncheckedOverflow]` transpile stays E-TRANSPILE-UNCHECKED;
  the pack/unpack emulation is REJECTED-WITH-REASON.** PHP can emulate 64-bit wrapping arithmetic
  (`unpack('q', pack('q', ...))` pairs, or GMP mod-2^64), but every emulation is SLOWER than PHP's
  native checked-ish arithmetic — and `#[UncheckedOverflow]`'s ONLY purpose is speed (the 2× intadd
  win). A transpile that silently turns a perf opt-in into a perf LOSS is a semantic-adjacent
  downgrade of intent; the honest artifact is the existing hard error steering to the checked
  default (which transpiles faithfully) or `Math.tryAdd/trySub/tryMul`. Risk example: a hot loop
  annotated for the VM's 2× win transpiles to PHP running ~5× SLOWER than un-annotated — the user
  reads "it transpiled" as "it's fine". *Alternatives:* GMP emulation (correct, slowest, adds a PHP
  extension requirement — violates transpile-no-ini-extensions); 32-bit-halves manual wrap (subtle,
  still slow); silently emit checked semantics (rule-14 leg 3, FORBIDDEN).

- **DEC-231 — AUTO-RULED (REOPENABLE): `Core.HttpClient` shipped (W3-2, TOP-20 #2 blocker) — sync
  HTTP/1.1 over std TcpStream + rustls (the TLS domain admitted 2026-07-03 EXPLICITLY for this),
  webpki-roots trust anchors; feature `http-client`, non-default, native-only
  (`E-TRANSPILE-HTTPCLIENT` — curl-mapping recorded as a possible future lift).** Surface: separate
  `Core.HttpClient` module (Symfony-component decomposition — the server-side `Core.Http` keeps
  Request/Response/Router; alternatives: nest under Core.Http (no nested-module precedent), one
  merged module (couples client to server)). Instance `HttpClient` with chainable timeout/redirects;
  get/post/put/delete + general send; typed `HttpResponse`; v1 scope excludes HTTP/2, pooling,
  proxies, cookies (documented). SECURITY beyond PHP curl: 64 MB response cap, CR/LF
  header-injection gate, URL-userinfo rejection (credential smuggling), explicit timeouts always
  on. Taxonomy names prefixed (`HttpTimeout`/`HttpTlsError`/`HttpConnectionFailed`) because bare
  names are TAKEN by Core.Db/Core.Mail — which surfaced a real design smell: INJECTED-CLASS DEDUP
  ACROSS PRELUDES = cross-module name capture (if two preludes declare `TlsError`, the second
  silently reuses the first's class, breaking catch semantics). Recorded in KNOWN_ISSUES as a
  QUEUED ADJUDICATION: per-module error namespacing (e.g. `Db.Timeout` member-error syntax) vs the
  prefix convention. Risk example: `import Core.Mail; import Core.HttpClient;` — a TLS failure in
  the HTTP client caught by `catch (TlsError e)` would land in a MAIL-taxonomy class. En route: the
  sweep-batch-1 quarantine substring hole FIXED generically (`Core.XSys` impure natives now
  quarantine programs importing the `Core.X` prelude twin).

- **DEC-232 — AUTO-RULED (REOPENABLE): `Core.Fs` shipped (W3, TOP-20 #5 blocker) — the TYPED
  filesystem module (std-only, always compiled, no feature gate).** Files + directories + sorted
  listings + recursive walk + tempDir; every failure a catchable `FsError` subtype classified from
  the OS error kind (FsNotFound/FsPermissionDenied/FsAlreadyExists/FsNotADirectory/FsIsADirectory/FsDirNotEmpty/
  FsIo); `removeDirAll` is the separate LOUD recursive delete refusing `/`, `.`, `..`. Determinism:
  `listDir`/`walk` are SORTED (Invariant 10 — OS directory order never leaks). Purely ADDITIVE next
  to the older `Core.File` (whose write/delete failures are uncatchable hard faults and whose read
  maps all failures to null — found by the spine-7 sweep); Core.File's deprecation/migration is a
  QUEUED developer adjudication (changing its error contract is user-visible — never self-ruled).
  Transpile = `E-TRANSPILE-FS` FOR NOW (PHP has faithful filesystem functions; the typed-error PHP
  emitter is a recorded future lift — refusing beats silent divergence). Risk example:
  `Fs.writeText("/etc/hosts", …)` under a normal user → catchable `PermissionDenied` with the path
  in the message; the same through `Core.File.write` → an UNCATCHABLE fault killing the program.
  LIVE LESSON folded in: the taxonomy is Fs-PREFIXED (`FsNotFound`, …) — the first draft claimed the
  bare name `NotFound` as an injected type and instantly CAPTURED `examples/web/server.phg`'s own
  `NotFound` class (E-INJECTED-TYPE-BARE on the user's own type) — the strongest evidence yet for the
  queued cross-prelude/user-space error-namespace adjudication (DEC-231 note).
  *Alternatives:* enrich Core.File in place (rejected: changes its shipped error contract);
  instance-based `new Fs(root)` sandbox (deferred: a chroot-style scoped-FS instance is a genuinely
  good SECURITY idea — queued as a v2 adjudication); feature-gating (rejected: std-only, no dep).

- **DEC-233 — AUTO-RULED (REOPENABLE): `Core.Session` shipped (W3, TOP-20 #3 blocker) — HTTP
  sessions over the Core.Http value types, std-only (no dep, no feature gate).** In-process
  `Mutex<HashMap>` store (String values → Send+Sync across `--workers` threads; structured data via
  Core.Json — PHP's serialized $_SESSION does the same), 128-bit /dev/urandom ids, idle-TTL expiry
  (default 1800 s, touch-on-access, lazy+opportunistic sweep — the gc_maxlifetime shape without a
  GC thread), `regenerate()` fixation defense, cookie defaults `HttpOnly; SameSite=Lax; Path=/`
  (PHP needs ini opt-ins), expired/unknown ids silently replaced with FRESH EMPTY sessions (never
  resurrected, never an error). THROW-FREE surface (store ops are total — no taxonomy needed).
  Native-only for now (`E-TRANSPILE-SESSION`; a session_start() mapping is the recorded lift).
  Risk example: attacker plants `phorjsid=X` pre-login (fixation); after `s.regenerate()` on login
  X is dead — with PHP the developer must know to call session_regenerate_id(true).
  *Alternatives:* store as prelude-visible SessionStore contract with swappable backends (QUEUED
  layered-openness v2 — file/redis-style backends; v1 in-memory matches phg serve's single-process
  model); Value-typed session data (rejected v1: Rc values cannot cross worker threads); cookie
  attributes configurable (queued with the v2 config surface — `; Secure` documented as manual).
  GOTCHA recorded: `open` is a phorj KEYWORD (open classes) — a native named `open` is unparseable
  at the call site (SessionSys.open → renamed `acquire`); prelude parse failures are SILENT
  (inject_core_modules skips unparseable preludes — a debug trap worth a loud assert someday).

## 2026-07-16 office batch (developer via AskUserQuestion — the run's queued adjudications RULED)

- **DEC-234 — RULED: error-class namespacing = MEMBER-ERROR SYNTAX** (`catch (Db.Timeout e)` /
  `throw new Mail.TlsError(...)` — qualified error types per module, no global bare-name claims).
  Developer note: `import Core.Db.Timeout as DbTimeout;` remains the local-shorthand escape hatch
  (the DEC-186 alias machinery) — confirmed as part of the design. Migration: current names stay as
  deprecated aliases during the transition. *Alternatives (offered): bless the prefix convention
  (rejected — ergonomics); collision = compile error (rejected — fixes the bug, not the design).*
  Implementation = a checker/parser slice (qualified names in catch/throw/extends positions), queued.
  **BUILT (2026-07-16 fable):** the qualified-member collapse now routes through the UA-L2
  `module_of` registry (the old hardcoded table predating UA-L2 knew only Http/Time/Decimal), so
  EVERY injected module's member types are qualifiable in every TYPE position — `catch
  (Uri.UriMalformed e)`, `catch (Db.Timeout e)`, `throws Mail.TlsError`, annotations — and
  `new Qual.Member(…)` construction works even when the qualifier is ALSO a class (`new
  Uri.UriMalformed(…)` — a `new`-gated route ahead of the static-method branch, so `Uri.parse(…)`
  statics are untouched). Bare member-imported names remain the working alias (the ruled
  transition stance). cli test pins catch/throws/throw-new on run+treewalk.
- **DEC-235 — REVOKED by DEC-239 (2026-07-16 full-reopen audit, flag F-001).** Original ruling:
  pipe `|>` = first-arg insertion (`x |> f(a)` ≡ `f(x, a)`), *alternative "callable application —
  rejected: every step would need a lambda wrapper"*. The audit established two facts the ruling
  was made without: (1) the pipe was ALREADY SHIPPED with callable-application semantics (probed:
  `5 |> mk(2)` → applies `mk(2)`'s closure → 7), so DEC-235 would have silently changed working
  programs; (2) **PHP 8.5 shipped `|>` with exactly those callable-application semantics**, so
  first-arg insertion would make identical syntax mean different programs in phorj vs PHP —
  poisoning transpile AND `phg lift`. Superseded by DEC-239.
- **DEC-236 — RULED: constructor DEFAULT PARAMS land in the sugar wave** (reuse the function
  default-param call-fill machinery; fixes the SmtpConfig.withAuth / SendmailTransport.at warts and
  a PHP-8 promoted-ctor parity gap). *Alternative (offered): keep the factory convention — rejected.*
- **DEC-237 — RULED: the overnight AUTO-RULED batch DEC-227…233 is RATIFIED WHOLESALE** — with the
  developer's standing note: everything stays register-recorded and the WHOLE set is revisited in
  the run-end full-reopen pass ("we will go back to everything once we finish everything" — the
  META-1 run-end reopen protocol applies).

- **DEC-236 BUILT (same session as ruled):** ctor default params — parser (`= literal` on ctor
  params), CtorParam.default threaded through ALL five rebuild passes (collapse_injected /
  rewrite_alias / rewrite_generics preserve verbatim; desugar_di/lift inject None), collection
  validates via the SAME collect_param_defaults machinery (order/literal/type codes reused),
  construction check via check_args_defaulted + the existing generic record_pending_fill (backends
  see full-arity `new` — byte-identity by construction), defaults INHERITED with the signature
  (both inherit paths in lockstep), formatter round-trips `= default`, E-CTOR-DEFAULT-GENERIC
  clean deferral (fill runs before type-arg inference). SmtpConfig/SendmailTransport rewritten to
  the spec's direct forms (withAuth/at stay as thin aliases). Conformance golden (3 backends) +
  4 checker tests. ALSO: microbench.sh gained positional per-micro filtering (developer request).

- **DEC-238 — RULED (developer, office batch) + BUILT (slice 1+2a): `Core.Debug` dump/dd +
  `Runtime.exit`.** Rulings: full pack incl. PHP twin (twin = next slice; transpile gated
  E-TRANSPILE-DEBUG meanwhile) · dump = ONE function carrying BOTH products via the `Dumped<T>`
  result object (`.value()` pass-through + `.text()` capture — chosen over bare-passthrough+`last()`
  (hidden state) and sink-overload (closures capture by VALUE — probed live, capture-to-local
  impossible)) · dd exits 1 · `Runtime.exit(code)` clean termination, distinct from `panic`
  (fault+trace) and from `main`'s return — three roles ratified, no duplication. Implementation:
  deterministic versioned renderer (`native/debug.rs`, format pinned by unit tests: sorted instance
  fields per ClassLayout, inline≤60-col containers, `*RECURSION*` cycle cut by container identity,
  canonical scalar kernel, quoted/escaped strings); exit = `__phorj_exit__:<code>` sentinel
  intercepted at BOTH top-level run loops onto the existing Batch-1-B exit-code channel (serve's
  per-call entry deliberately does NOT intercept — an exit in a handler is a 500, never a silent
  worker death; finally blocks do NOT run — the PHP exit() semantic, documented); totality
  enhancement: `expr_is_never` now recognizes QUALIFIED never-calls (never natives like
  `Runtime.exit` + never static methods like `DbError.fail`) — code after `dd`/`exit` correctly
  flags W-UNREACHABLE. Tests: 6 renderer units (format pinned) + 5 both-backend integration (incl.
  exit codes via cmd_*_exit). QUEUED: the PHP twin (`__phorj_debug_render`, common domain first —
  enums/sets erase to indistinguishable PHP shapes, so the twin FAULTS on those rather than lying);
  TTY-colorized rendering (byte-identity keeps v1 plain).
  ⊳ The `__phorj_debug_render` PHP twin is BUILT since (in the DEC-238→DEC-263 interim; 4 files use
  it, incl. `src/ext/debug/natives.rs`) — label flipped 2026-07-28, consistency audit.

## 2026-07-16 — FULL REOPEN AUDIT rulings (developer at desk, via AskUserQuestion; audit report = docs/research/2026-07-16-full-reopen-audit.md)

- **DEC-239 — RULED (audit flag F-001): pipe `|>` = PHP-ALIGNED CALLABLE APPLICATION, ratified as
  a 4-part package.** (1) DEC-235 first-arg insertion REVOKED (see its entry — ruled without
  knowing the pipe had already shipped PHP-aligned, and before PHP 8.5's own `|>` semantics were
  on the table). (2) Base semantics = the shipped ones ≡ PHP 8.5 (php.watch/versions/8.5/
  pipe-operator verified exhaustively): RHS is any function-valued expression, piped value applied
  as the single argument; left-assoc. (3) PRECEDENCE FIX queued: phorj parses `x |> f == 6` as
  `x |> (f == 6)` (comparison tighter — today a loud cross-type error, never silent) while PHP
  parses `(x |> f) == 6`; phorj moves to PHP's exact slot (tighter than comparison, looser than
  arithmetic — `10 + 6 |> inc` → 17 already matches). (4) TWO strictly-additive ergonomics sugars
  that beat PHP (php.watch: "not possible to change the position of the parameter" in PHP):
  **bare-`%` placeholder**, whole-argument slots of the TOP-LEVEL RHS call only (`x |> f(%, 2)` ≡
  `f(x, 2)`; multiple `%` slots legal — value already evaluated once; `f(%)` legal-redundant; each
  `|>` in a chain binds its own `%`; `f(% + 1)` / nested `g(%)` rejected `E-PIPE-PLACEHOLDER` —
  nesting is the lambda's job), and **contextually-typed pipe lambda**: expression-body lambda in
  pipe position may omit the param type (`x |> (v => v * 2 + 1)` — type flows from the pipe, the
  DEC-201 contextual-typing precedent; naming beats PHP's `fn($v)=>` on readability).
  Divergences RECORDED AS JUSTIFIED (phorj-better): void mid-chain = compile error (PHP coerces
  void→null and pipes garbage); no string-callables `'strtoupper'` (static typing); single-arg
  arity enforced at COMPILE time (PHP: runtime TypeError). Token `%` chosen over `<%>` (generics
  visual collision, template-tag smell, 3× ceremony) and `%%` — a lone `%` in an argument slot
  cannot parse as modulo (needs a left operand), so bare `%` is unambiguous under whole-arg
  scoping. *Alternatives (offered, rejected): keep DEC-235 (breaks shipped curried pipes + PHP
  divergence on identical syntax); Hack-style %-anywhere (PHP RFC threads flagged $$-anywhere as
  the confusing part; %-soup, unnameable); lambda-with-%-binder (developer's sketch — challenged:
  all the syntax of a lambda, none of the naming; developer accepted); defer placeholder (leaves
  phorj wordier than PHP at the multi-arg point).* Build = parser/checker slice, queued
  fresh-context; conformance goldens must pin: probes A–E + P1–P3 from the audit (bare 2-param
  loud error, closure/method-value/callable-returning RHS, chain, precedence, void rejection).
  **BUILT (2026-07-16 fable, 5 slices `0c41f49` `c706076` `f51e1b0c` `94c9a4f` + docs):**
  `Expr::Pipe` AST node — also fixes a fidelity defect found during the build (`phg format` used
  to rewrite `x |> f` into `f(x)`: the parser lowered pipes before the printer saw them) — with
  `checker::lower_pipes` first-pass expansion; the precedence slot (each relation probed live on
  php-8.5.8: tighter than `== < & ?? &&`, looser than `+ <<`); `%` placeholder (single-slot
  substitution, multi-slot single-evaluation IIFE with a collision-scanned `phorjPipe<n>` param;
  parse-time `E-PIPE-PLACEHOLDER` shape validation); contextual pipe lambda (checker-inferred
  param type materialized into the AST post-check — Invariant-7 safe, `run≡runvm` pinned by
  test); probe goldens in `parser/tests` + `checker/tests/pipes.rs`; `examples/guide/pipe.phg`
  (3-leg byte-identical); `phg lift` now names `|>` in its Tier-2 rejection. **PENDING fork
  (developer adjudication, deliberately not self-ruled):** after a contextual lambda the RHS
  grammar stays uniform, so `x |> (v => v) + 1` binds the `+` to the LAMBDA → loud
  `E-PIPE-LAMBDA-CONTEXT` with a parenthesize hint (exactly like `x |> f + 1`); the ergonomic
  alternative — binding trailing tight-ops to the pipe result — is strictly additive. ⊳ **RULED by
  DEC-393 (2026-07-29): KEEP the loud error, fork CLOSED** — the uniform RHS grammar (lambda and
  named function fail identically) beats an additive carve-out that would make a lambda's extent
  depend on what follows it. Also not built (not in the ruled package): PHP 8.6's draft `|>=` pipe-assignment;
  native `|>` EMISSION in transpiled PHP (output uses the lowered plain call — byte-identical).

- **DEC-240 — RULED (audit flag F-002): `Core.Uri` — one immutable RFC 3986 class with typed
  errors.** PHP 8.5 ships an always-on URI extension (`Uri\Rfc3986\Uri` raw+normalized getters,
  `Uri\WhatWg\Url` browser normalization, withers, `resolve()`, comparison) replacing the
  20-years-lying `parse_url()`; phorj had only 4 percent-encoding helpers (`Core.Url`) + an
  http(s)-only INTERNAL parser in HttpClient. Ruled shape: single immutable `Uri` — `Uri.parse(s)`
  throwing a typed `UriError` taxonomy (beats PHP's generic exceptions), full accessors
  (scheme/userInfo/host/port/path/query/fragment + raw variants), withers, RFC 3986 §5.2
  `resolve(ref)`, `normalize()`, `equals`, `toString`, all schemes. **PHP twin =
  `Uri\Rfc3986\Uri` (the 8.5 floor makes it always available) → byte-identity, NO native-only
  ladder quarantine.** HttpClient's internal parser retires onto it (architecture win, D3).
  *Alternatives (offered): mirror both PHP classes incl. WHATWG (deferred until a real need —
  browser-grade normalization is marginal for a backend language; recorded); defer entirely
  (rejected — PHP measurably ahead of phorj TODAY, against the mandate).* Build queued.
  **BUILT (2026-07-16 fable):** four live probe rounds pinned the twin contract
  (`docs/research/2026-07-16-uri-twin-probes.md` — incl. the uriparser quirks: getHost
  lowercases IPv6 as written vs toString 8×4-digit expansion; unmatched leading `..` kept only
  scheme-less-relative; i64 port limit; ASCII-unreserved-only pct decoding); std-only Rust
  kernel + `Core.UriSys` natives (`a88efb5`, 12 corpus tests); injected `Uri` prelude class with
  the per-component `UriError` taxonomy (messages twin-identical, so byte-identity holds while
  the TYPES beat PHP); `__phorj_uri*` PHP-leg wrappers over the extension; 3-leg byte-identity
  verified + `examples/guide/uri.phg` differential-gated. REMAINING: HttpClient internal-parser
  retirement onto Uri (the ruled D3 architecture win) as a follow-up refactor slice; lift
  mapping for PHP `Uri\Rfc3986\Uri` usage sits in the lift Tier-2 tier with closures/FCC.

- **DEC-241 — RULED (audit flag F-004): asymmetric visibility BUILDS** — `public private(set)`
  (+ `protected(set)`) on fields, promoted ctor params, and statics; queued in the sugar wave.
  Audit finding: it sat in UNIFIED-SPEC's founding v0.1 surface yet was never implemented AND
  never tracked — a silently dropped founding promise. Transpiles 1:1 to PHP 8.4+ syntax (8.5
  floor → free byte-identity); PHP already validated the semantics. *Alternatives (offered):
  reject + remove from spec (immutable-by-default + `with {}` + hooks cover part of the niche —
  rejected: PHP is ahead here today); tracked-deferred (rejected — build it).*
  **BUILT (2026-07-16 fable):** `Modifier::PrivateSet/ProtectedSet` (parser munches the `(set)`
  group; `set` stays contextual), ClassInfo `set_vis`/`static_set_vis` collected from fields +
  promoted ctor params + statics (validation: `mutable` required = E-SET-VIS-IMMUTABLE; set never
  wider than read = E-SET-VIS-WIDER), inherited with owner preserved (traits re-own, parents
  keep the declarer), enforced at ALL write sites (instance assign, static assign, `with {}`
  override) via `enforce_set_vis` (E-ASSIGN-SET-VISIBILITY); transpile emits PHP 8.4's
  `private(set)`/`protected(set)` 1:1 (compile-time enforced + runtime re-enforced free);
  formatter round-trips. Five checker tests + `examples/guide/asymmetric-visibility.phg` 3-leg.
- **DEC-242 — RULED (audit flag F-005): partitioned-cookie (CHIPS) knob queues** — additive
  `partitioned` option on the Session/Http cookie config emitting the `Partitioned` attribute;
  parity with PHP 8.5's setcookie/session surface. Tiny slice. *Alternative (offered): reject as
  iframe-niche — rejected: cheap parity.*
- **DEC-243 — RULED (audit flag F-006): `String.levenshtein` + `String.similarText` queue,
  GRAPHEME-AWARE** (the W4-4 codepoints-default stance) — phorj's levenshtein thereby equals PHP
  8.5's `grapheme_levenshtein` while plain PHP `levenshtein()` stays byte-blind (recorded
  phorj-better). `soundex`/`metaphone` REJECTED-WITH-REASON: English-phonetic relics.
  *Alternatives (offered): full family incl. phonetics (rejected); reject all (rejected — the
  twins are trivial and the mandate says everything PHP does).*
- **DEC-244 — RULED (audit flag F-007): extension methods get an EARLY sugar-wave slot** —
  right after the audit-queued builds (DEC-239 pipe fixes, DEC-240 Core.Uri). PHP 8.6 has a
  draft RFC (incl. scalar extensions); phorj ships its statically-checked, import-gated version
  (nothing-in-the-wind: extensions visible only where imported) FIRST — the stay-ahead mandate.
  *Alternative (offered): keep queue position (drafts often slip) — rejected.*
  **RESOLVED — RULED (2026-07-16, developer at desk via AskUserQuestion): UFCS IS the
  extension-method story, ratified as-is.** The build session verified the surface already
  works end-to-end (scalar receivers `5.doubled()`, string/class receivers, extra args, chains —
  statically checked, rewritten pre-backends by the Slice-6 UFCS machinery, import-gated =
  nothing-in-the-wind). No new declaration syntax; PHP 8.6's draft (incl. scalar extensions) is
  thereby already beaten. *Alternatives (offered): Kotlin-style receiver declaration sugar over
  the same machinery (declined — cosmetic-only); opt-in `extension` marker (declined — breaking
  for every UFCS site); defer to sugar wave (declined).* Shipped as a docs+goldens slice:
  FEATURES row, `examples/guide/extension-methods.phg` (3-leg gated), spec note.

- **DEC-274 — RULED (2026-07-16, developer at desk via AskUserQuestion, three-part with inline
  example previews): THE SUGAR-GATE DISCIPLINE — settled "everywhere".** Amends/extends DEC-244
  + DEC-197 into one uniform rule for method-position sugar on ALL functions (natives and user
  libraries alike):
  (1) **Module import = full sugar for the module**: `import Core.String;` enables BOTH
      `String.upperCase(s)` AND `s.upperCase()` for every function of the module (probe
      CORRECTION recorded honestly: this half was already today's behavior for ALL modules —
      the session's first probe misread an unrelated failure; the ruling RATIFIES it).
  (2) **Function import = full sugar for that one function**: `import Core.List.reverse;`
      enables bare `reverse(xs)` (DEC-197, today) AND method-position `xs.reverse()` (new);
      the qualified form stays available when the module is also imported.
  (3) **No import → none of it** (nothing-in-the-wind, the #1 standing rule).
  (4) **First-param-is-the-subject CONFIRMED** as the settled receiver semantics: the subject
      binds the first parameter, extra args follow in order, chains compose (each result is the
      next subject) — `"ha".shout(3)` ≡ `shout("ha", 3)`.
  (5) **Plain functions remain the declaration form** (re-confirmed DEC-244: no marker syntax;
      the `extension function …(this T x)` alternative was offered again with a preview and
      declined again).
  *Alternatives (offered, declined): function-import-only gating (breaking — retracts
  xs.reverse-via-module-import); module=sugar but function-import=bare-only; no native sugar at
  all; tighten user-fn scope-gating to explicit imports (scope IS the gate — kept).* Build =
  generalize the existing List-receiver native method path to every receiver type + wire the
  function-import surface into method resolution, per-module × per-import-level goldens.

- **DEC-245 — RULED (audit flag F-010): intersections resolve shared methods as an OVERLOAD SET.**
  Executes the E-INTERSECT-SIG revisit clause DEC-057 scheduled for "when overloading lands"
  (3 weeks overdue, caught by the reopen): member access on `A & B` merges identical signatures
  and lets DIFFERENT signatures coexist as overloads (the DEC-058 machinery); only genuinely
  ambiguous combos (same params, different returns, no selector) stay `E-INTERSECT-SIG`.
  *Alternative (offered): keep require-agreement (rejected — a class can legally implement both
  interfaces while the intersection type can't express it).* Build queued.
  **BUILT (2026-07-16 fable):** the type-site check merges per-name signatures across members and
  rejects ONLY same-params/different-return (`E-INTERSECT-SIG`, narrowed message); the call site
  collects `name`'s signatures from EVERY member (θ-substituted, identical sigs deduped) into one
  set that `check_method_sigs`' existing multi-arm dispatches. Runtime untouched (dispatch is by
  the concrete instance's class). Tests: overload-set accept / same-params-diff-ret reject /
  identical-merge / no-match loud; `examples/guide/intersection-overloads.phg` 3-leg gated.
- **DEC-246 — RULED (audit flag F-011): `clippy::pedantic = deny` BUILDS** — honoring DEC-176
  (ruled 07-01, never enabled; Cargo.toml stopped at `all`). Own slice in the build queue.
  *Alternative (offered): revoke to clippy::all — rejected.*
- **DEC-247 — RULED (audit flag F-012): `Core.DateTime` NOW, HIGH priority** — immutable DateTime +
  Duration + timezone handling in Core, twinned to PHP DateTimeImmutable/DateInterval (8.5 floor →
  byte-identity except `now()`); beats PHP (immutable-only, typed errors, no parse-to-false);
  ships before PHP 8.6's Duration RFC = ahead-watch win. Supersedes the 07-13 externalize-audit
  "calendar→lib" lean (mooted by DEC-216 being unexecuted). DEC-206's bare-name gate applies when
  it lands. *Alternatives (offered): wait for the vendor path (gap stays open indefinitely);
  minimal Instant+Duration only (defers the tz question but keeps the gap).*
  **PENDING-BLOCKED (2026-07-16 fable build phase — dependency admission, developer-tier):**
  DEC-273 itself classifies DateTime as an EXTENSION with a **tz-data dep**, and no timezone
  dependency is in the vetted list (`argon2`/`regex`/`ctrlc`/`corosensei` + the ruled
  rustls/lettre/rusqlite/mysql/postgres domains) — every prior admission was an explicit
  developer approval, so this one is not self-ruled (Invariant 15 + the dependency policy).
  Options to rule: (a) admit a tz crate (`chrono-tz`/`tzdb` — vendored-IANA style, no runtime
  file reads, deterministic); (b) vendor raw IANA tzdata + hand-roll the TZif reader (std-only
  discipline kept, largest build effort); (c) read the HOST system tzdata at runtime (rejected
  by determinism Invariant 10 unless quarantined); (d) phase 1 = fixed-offset zones only
  (`+02:00`, `UTC`) with named-zone support deferred behind the admission (smallest slice,
  ships the DateTime/Duration surface now — RECOMMENDED as the unblock). Risk example: PHP
  `new DateTimeImmutable('2026-03-29 02:30', new DateTimeZone('Europe/Paris'))` lands INSIDE
  the DST gap — matching PHP's normalization byte-for-byte requires the full IANA rules, which
  is exactly what the admission decides. The fable run SKIPPED item 9 and continued the queue.
  **UNBLOCKED — RULED (2026-07-16, developer at desk via AskUserQuestion): ADMIT A TZ CRATE**
  (vendored-IANA style — `chrono-tz` or `tzdb`, pick at build time on audit: no runtime file
  reads, deterministic, feature-gated per the dependency policy; the crate's tzdata snapshot
  must be checked against the oracle PHP's zone behavior in the twin probes). Full named-zone +
  DST support from day one. *Alternatives (offered): phase-1 fixed-offset only (recommended by
  the session, declined — dev chose full support); vendor IANA + hand-rolled TZif reader
  (declined — largest effort); keep blocked (declined).* Build = fresh-context slice: crate
  vetting → live DateTimeImmutable/DateInterval probe rounds (the Uri methodology) → kernel →
  prelude twin.
- **DEC-248 — RULED (audit flag F-009): FULL PHP ALIGNMENT of the loop surface; supersedes A-6/
  DEC-094's execution drift AND retires for-in.** Package: (1) `foreach` gains TYPED bindings
  (`foreach (xs as int x)`) + the PHP-shaped key/value form (`foreach (m as string k => int v)`);
  (2) `for (T x in xs)` RETIRES (`E-RETIRED-FORIN` + rewrite hint) — it was the non-PHP divergence
  with no justification ("no reason to diverge here" — dev); (3) C-style `for (;;)` stays (verified
  already working, PHP-aligned); (4) ranges iterate via `foreach (0..n as int i)`; (5) repo-wide
  codemod (~69 example sites + conformance + preludes + docs), fresh-context slice, conformance
  goldens for all forms. Typed bindings = the sole phorj addition (the explicitness rule).
  *Alternatives (offered): untyped-like-PHP bindings (the only type-less declaration in the
  language — rejected); `var`-form bindings (rejected); retire foreach instead (rejected — keeps
  the divergence); keep both (TIMTOWTDI — rejected).* Closes conflict C-2 / flag F-009.
  ⊳ SUPERSEDED on the for-in point by DEC-343 (2026-07-26): keep both forms; C-2 closed.
- **DEC-249 — RULED: METHOD default parameters BUILD (extending DEC-236's ctor machinery to
  methods); then the retry surface becomes `db.transaction(fn, int retries = 0)` and
  `transactionRetry` retires.** Resolves DEC-208's retained PENDING the ambitious way: the
  language wall falls instead of the API bending around it. *Alternative (offered): confirm
  shipped `transactionRetry(fn, retries)` (rejected — dev chose the language fix).* Two-part
  build: method defaults slice → Db surface rename.
  **BUILT (2026-07-16 fable):** collection validates method defaults via `collect_param_defaults`
  (generic-TYPED defaulted params stay the DEC-236 deferral; non-generic defaults on generic
  methods fill before inference — the `transaction<T>(fn, int retries = 0)` shape); MethodSig
  carries defaults (inheritance free via FnSig); single-signature calls fill via
  `check_args_defaulted` + `record_pending_fill`; `?.` calls omitting defaults = clean deferral
  error. Db surface: ONE `transaction(fn, int retries = 0)` method, `transactionRetry` RETIRED
  (all call/doc sites migrated). The build root-caused two latent clone-staleness bugs (fills
  restored pre-erasure arg subtrees; the throws-`?` eraser restored pre-fill calls) — fills now
  splice FIRST (`apply_default_fills`) and the eraser unwraps the LIVE inner.
- **DEC-250 — RULED (DEC-183 caveat): Optional<enum> variant patterns = HIGH priority** — thread
  enum-variant coverage through `T?` so `match c { Red() => …, Blue() => …, null => … }` is legal
  and exhaustive over `Color?` ("exhaustive matching is a flagship; an Optional-of-enum failing it
  undermines the story" — ruled soundness-adjacent). *Alternatives (offered): normal queue slot;
  leave recorded.* **BUILT 2026-07-16 fable** — `checker/matches.rs`: the `Pattern::Variant` arm
  unwraps an `Optional(Named(enum))` scrutinee, and exhaustiveness over an enum-optional requires
  every variant + `null` (arm order free; `default` still covers). Two caveat-pinning tests
  flipped to capability tests; three new tests; three-leg-identical guide example
  `examples/guide/optional-enum-match.phg`. No backend work needed — the interpreter/VM/PHP
  match lowering already handled unwrapped variants; only the checker refused.

- **META-7 — STANDING RULES (developer, 2026-07-16 audit, verbatim intent):** (1) **cross-language
  scan mandatory** — whenever phorj sets out to do something better than PHP, survey how OTHER
  languages (Rust/Kotlin/Swift/TS/Go/C#…) solved it before designing; (2) **byte-identity is NOT
  the priority ordering** — emitting a `__phorj_*` helper to make the PHP leg identical is always
  an acceptable tool; the choice is ALWAYS surfaced with an explanation and ruled by the developer,
  never self-decided. Applies to every future design and build slice.
- **DEC-251 — RULED (audit flag F-014): build ALL THREE PHP-enforcement-ahead checks, HIGH
  priority** — (a) override parameter-compatibility (E-OVERRIDE-SIG extension; the latent
  transpile-fatal twin of the fixed return-covariance case), (b) private/protected STATIC field
  external-read enforcement, (c) visibility through intersection-typed receivers. Checker-only,
  byte-identity strictly improves. Per META-7: design pass surveys Kotlin/C#/TS override-variance
  rules first. *Alternatives (offered): (a)-only; keep tracked — both rejected.*
- **DEC-252 — RULED (audit flag F-015): LSP prelude-injection fix, HIGH priority** — route
  `diagnostics_for` through the same `check_and_expand` the CLI uses (injected types + intrinsic
  imports), test pinning an injected-type program LSP-clean on both editors. **STANDING RULE
  (developer): `phg check` and the LSP must never diverge — same pipeline, kept in sync as part
  of every diagnostics change** (extends the both-editors-same-change DoD). *Alternative (offered):
  normal queue — rejected.*

- **DEC-253 — RULED (audit flag F-013): nullable unions BUILD, BOTH spellings** — `(A | B)?`
  canonical + `A | B | null` accepted (formatter canonicalizes). Optional machinery (`??`/`?.`/`!`/
  if-let) gains union inners; match extends the DEC-183 model (member arms + `null` arm). No new
  runtime representation (Null exists; union values are values); transpiles to native PHP
  `A|B|null` (free byte-identity). Closes a PHP-expressible-but-not-phorj type shape.
  *Alternatives (offered): canonical-only (rejected — PHP-reader familiarity worth +10%);
  reject-with-reason (rejected — PHP stays ahead).* Medium checker slice, queued.
  **BUILT (2026-07-16 fable, `b7553ed`):** both spellings resolve to one
  `Ty::Optional(Ty::Union(..))` — optional machinery + DEC-183 match inherited for free; `null`
  parses as a union-member marker (keyword — collision-free); standalone `null` type =
  `E-NULL-TYPE`; formatter canonicalizes `A|B|null` → `(A | B)?`; transpile emits native PHP
  `A|B|null` for both spellings; display parenthesizes. Probing the example also surfaced and
  fixed a pre-existing SPINE BUG (`2ef2aaf0`): statement-position `match` with printing arms
  emitted unparseable PHP (`echo` inside a `match(true)` expression arm) — never caught because
  every gated example used match in expression position; now lowered to the instanceof if-chain
  (`MatchTarget::Discard`) and locked by the nullable-unions example + a transpile test.
- **DEC-254 — RULED (audit flag F-016, four AskUserQuestion rounds with full before/after +
  why-1-vs-2 analysis): in-place mutation = THE FULL PACKAGE.** (1) **Slice 1b builds** —
  field-base indexed assignment `obj.f[i] = v` / `this.f[i] = v` (completes the class-handle
  idiom for in-place algorithms). (2) **`ref` parameters build** — Swift-model **copy-in/copy-out**
  (NEVER aliasing: callee owns its value during the call, COW invariants intact; final value
  written back on return), keyword `ref` at BOTH declaration (`function f(ref List<int> xs)`) and
  call site (`f(ref data)`; must be a `mutable` binding), exclusivity-lite checks (no two `ref`
  args from one binding), transpiles to PHP `&$arr` (identical except exotic reentrant shapes —
  disclosed per META-7), lifter maps `&$arr` → `ref` 1:1. Developer ruling: "it's safe and it's
  not the default behavior; a must-have feature." (3) **Parameter-mutability TRIAD ratified**:
  plain param = immutable (default) · `mutable` param = callee-local mutability, MY copy, caller
  never affected, call site unmarked (sugar for the first-line mutable copy) · `ref` param =
  write-back, call-site-marked. Keyword `ref` chosen over `inout` (dev disliked), `mutable`-only
  (two meanings), and `&` (sigil-removal principle + intersection-type collision — challenged and
  agreed). *Cross-language scan (META-7): C# ref/both-sites; Swift inout=copy-out+exclusivity
  (the sound precedent); Java/JS/Kotlin handle-idiom-only; PHP's own 8.5 pipe bans by-ref.*
  Multi-slice build (parser small / checker moderate / VM write-back medium / JIT medium),
  queued after the HIGH audit builds.

- **DEC-255 — RULED (audit flag F-017): the fault-parity EXIT-STATUS sweep RUNS, HIGH priority** —
  transpile every fault-trigger native, check PHP's exit status; any zero-exit (PHP silently
  succeeds where phorj faults) comes back as an asked helper-vs-accept ruling per META-7.
- **DEC-256 — RULED (audit flag F-018, three clarification rounds): W4-4 Unicode — THE FULL
  PACKAGE, ALL SLICES NOW ("i want all slices now").** Three measuring layers, honest names:
  bytes = `Bytes.fromString(s).length` (exists, unchanged) · codepoints = `String.length`
  (FLIPPED from bytes: "café"=4 — the dev's remembered "3" was arithmetic slip, challenged with
  the byte table; PHP twin = tiny PCRE-/u helper, hermetic) · graphemes = `String.graphemeLength`
  + `String.graphemes` (human-visible count: 👍🏽=1, family-emoji=1; the Unicode-segmentation-table
  dependency + PHP-twin (ext/intl vs helper) questions get ASKED in the build's design round per
  META-7). PLUS Unicode case ops (upper/lower/IgnoreCase beyond ASCII; divergent-fold edges like
  ß asked, never silent). *Alternatives (offered): graphemes-default (Swift model — rejected:
  table dependency for the DEFAULT); keep bytes (rejected — the exact PHP wart W4-4 exists to
  fix); graphemes-later/never (rejected — dev wants all now).*
- **DEC-257 — RULED (audit flag F-019): Iterator protocol = INTERFACE-BASED** — a Core
  `Iterator<T>` interface; any implementor is foreach-able (post-DEC-248 world); DbStream/RowStream
  implement it; List/Map/Set/range keep built-in fast paths; PHP twin = Iterator/IteratorAggregate.
  Design round runs the META-7 cross-language scan (Rust Iterator / Kotlin Sequence / JS protocol /
  PHP Traversable) before the exact shape (`next(): T?` vs `hasNext/next`) is asked.
  *Alternative (offered): built-ins-only + manual pull loops (rejected — PHP stays ahead:
  any PHP class can be Traversable).*
  **SHAPE RULED 2026-07-16 (developer, AskUserQuestion, post-META-7 scan):** (1) **shape =
  `hasNext(): bool` / `next(): T`** (Kotlin/C# family) — chosen over the recommended Rust/Swift
  `next(): T?` and over a JS-style `IterStep<T>` enum, BECAUSE it makes nullable element types
  sound for free: null is never a termination signal, so `Iterator<string?>` needs zero
  restriction (the very hazard that prompted the re-ask). (2) **exhausted `next()` = FAULT** —
  documented contract "iterator exhausted", stdlib implementors fault deterministically like
  index-OOB (alternative implementor-defined-behavior rejected: silent-footgun class).
  (3) **throwing iterators auto-propagate in foreach** — each desugared pull carries `?`; the
  enclosing function must declare/catch (alternative hand-loop-only rejected: re-opens the PHP
  Traversable gap). (4) **Db streams = FULL reshape** — RowStream/DbStream become
  `hasNext()/next()` implementing `Iterator<Row>`/`Iterator<T>` (internal one-row lookahead
  buffer; pre-1.0 unpushed = cheapest breaking moment; alternative keep-both-protocols rejected:
  dual API forever on the flagship streaming type).
  ⊳ BUILT since — `Iterator<T>`/`hasNext`/`next` ship (`src/cli/preludes.rs`); `Input.lines()`
  streams byte-identically on all 3 legs — label flipped 2026-07-28, consistency audit.
- **DEC-243 addendum — BUILT 2026-07-17 fable:** levenshtein (Wagner–Fischer, bytes) +
  similarText (Oliver's algorithm, bytes) + similarTextPercent (value-returning twin of PHP's
  by-ref `$percent`; PHP leg = pure Tier-1 IIFE — META-7 helper-trade disclosed here). Three-leg
  oracle-identical incl. float formatting. WIN-OR-FLAG bench joins the quiet-box run (owed).
- **DEC-258 — RULED (audit flag F-020): Db column naming = OPT-IN snake↔camel mapping** —
  default stays STRICT exact-name; an explicit opt-in (surface asked in its design round:
  `db.withNaming(Naming.SnakeToCamel)` shape) applies the deterministic mapping.
  *Alternatives (offered): strict-only (SQL aliases forever); auto-map default (silent name
  transformation — the magic phorj rejects) — both rejected.*

- **DEC-259 — RULED (audit, process): the perf-bench doctrine WIDENS** — (1) EVERYTHING that has
  a PHP equivalent gets benched against it, including I/O-bound native modules (via fixtures:
  in-memory SQLite, local SMTP, …) — the I/O carve-out is REJECTED-then-refined; (2) MACRO benches
  of whole programs/pipelines/workflows join the suite — REAL APPLICATIONS benched against their
  PHP twins (the developer's `var/phorj-app` is exactly this instrument: an app grown alongside
  the language to compare with real-world PHP apps — KEEP, gitignored by design, never propose
  deleting it). WIN-OR-FLAG applies to all of it. *Alternative (offered): confirm the macro-only
  carve-out — rejected.*
- **STANDING RULES batch 2 (developer, 2026-07-16 audit):** (a) **transpile + lift are
  always-current surfaces** — every language/stdlib change updates the PHP emitter AND the lifter
  in the same change, exactly like the check≡LSP rule (DEC-252) and the editors-same-change DoD;
  a feature that runs but doesn't transpile/lift (or vice versa) is not done. (b) `cargo-fuzz`
  ADMITTED as a dev-only dependency (runtime dep policy untouched); the parser/lift unwrap audit
  + fuzz pass execute the EV-7 never-panic invariant.

- **DEC-260 — RULED (audit flag F-021): folder restructure ratified, all three moves** —
  `manifest/lock/vendor → src/package/` (pre-stages DEC-216) · `dap/debug/dump/inspect/profile/mem
  → src/devtools/` · `token.rs → src/tokenizer/token.rs`. Mechanical git-mv slices, one commit each.
- **DEC-261 — RULED: the DEC-215 L1/L2 refactor ADVANCES** — from Ω-4/Ω-7 to right after the
  audit's HIGH builds: the checker stops accumulating domain code sooner; future modules consume
  L1 attribute-reflection instead of growing desugar_db. *Alternative (offered): keep the Ω slot —
  rejected by the developer.*
- **DEC-262 — RULED: M-Decomp ordering + THE NEW FILE-SIZE RULE (Invariant 13 AMENDED).**
  Ordering: growth-coupled three FIRST (preludes → per-module files; explain → per-code-family;
  runtime_php → per-helper-domain — future features then add FILES not LINES), then remaining
  non-JIT by size (desugar_db, native/db, vm/exec, mail), JIT five LAST each in a fresh context.
  **NEW CAP (developer): soft 300 / hard 500 lines per source file** — "everything must be
  organized/structured/decoupled into clear many files"; split-as-you-go is the DEFAULT behavior
  (a feature that would push a file past the soft cap STARTS by splitting it); genuinely-cohesive
  exhaustive-match units use index/dispatcher patterns to comply; enforcement = a pre-commit
  line-count warning (queued with the rule). Applies to new code immediately, to existing files as
  M-Decomp reaches them. *Alternatives (offered): 400/600 (recommended, declined); 500/800.*

- **DEC-263 — RULED (audit flag F-025): UNIVERSAL SECRET REDACTION** — `Secret<T>` renders
  REDACTED (`Secret { *** }`) on EVERY generic value-rendering surface: Debug.dump/dd (found
  leaking, probed live: transitive `Cfg { pw: Secret { value: "top" } }`), error messages,
  reflection dumps, and every future serializer/trace surface. `.expose()` is the SOLE read path
  (+ the existing W-SECRET lint). PHP twin redacts identically. Interpolation already refuses at
  compile time (verified). *Alternatives (offered): E-SECRET-DUMP type error (kills dump's
  config-debugging value); document-only (abandons safer-than-PHP for the corner).* HIGH build.

- **DEC-264 — RULED (audit flag F-026, HIGH security): HttpClient strips sensitive headers on
  cross-origin redirect + on TLS downgrade.** On a redirect whose target ORIGIN (scheme+host+port)
  differs from the current, DROP {`Authorization`, `Cookie`, `Proxy-Authorization`,
  `WWW-Authenticate`} before the next hop; ALSO drop them on any https→http downgrade even
  same-host; same-origin same-scheme hops keep all headers. Closes the credential-leak-on-redirect
  class (curl CVE-2022-27774) and makes the "beyond PHP curl" claim true. Proxy usage unaffected
  (Proxy-Authorization is consumed at the configured proxy transport, never forwarded to origin —
  explained + confirmed). Cross-language: reqwest/curl-post-CVE/browsers use exactly this RFC set.
  *Alternatives (offered): strip on ANY redirect (occasionally over-strips same-origin re-auth);
  error on redirect-with-credentials (breaks OAuth flows); broaden to heuristic X-Api-Key/token
  matching (over-strip risk — the RFC set is precise/predictable).* Build in the security wave.
- **DEC-265 — RULED (audit flag F-027, security): SMTP REQUIRES TLS when credentials are set.**
  If `SmtpConfig` carries a user/password → FORCE `Tls::Required` (fail the send if the server
  won't STARTTLS); credentials NEVER touch a cleartext channel. Unauthenticated sends (`user==""`,
  Mailpit-style fakers) keep `Opportunistic` so local dev works. Plus the explicit knob
  (`Tls::Required`/`Opportunistic`/`None`) for override — subsumes the queued DEC-230 TLS-knob item
  with the security lens. Cross-language: Symfony Mailer / nodemailer default to this.
  *Alternatives (offered): knob-only keep-opportunistic-default (unsafe default); implicit-TLS
  on 465 (more spec-accurate, more logic — fold in later if wanted).*

- **DEC-266 — RULED (audit flags F-022/F-008): the three perf LOSSES become BUILD ITEMS** (queued
  after the HIGH correctness/security builds; WIN-OR-FLAG, measured before/after per slice):
  jsonround 0.25× → Json node arena + scalar-by-path native + enum-match JIT coverage; dbwork
  0.63× → statement-handle cache + native bind→exec fast-path (skip DbResult boxing on the hot
  path); HttpClient → connection keep-alive/pool (serve's `Connection: close` is the related
  lever). Losing to PHP on a shipped macro violates the mandate → real work, not notes.
  *Alternative (offered): notes-only until a perf wave — rejected.*
- **DEC-267 — RULED (audit flag F-023): the perf SUITE EXPANDS, both tiers** (DEC-259 doctrine
  → concrete build): (1) I/O-native fixture benches — Db vs PDO-SQLite in-memory, Mail vs a local
  SMTP fixture, HttpClient vs a local server; (2) real-application MACRO benches — whole
  request/response cycles (router+db+template pipeline) via `var/phorj-app` vs an equivalent PHP
  app. Each joins `bench/micro-baseline.json` under WIN-OR-FLAG. Makes "beats PHP on real
  workloads" MEASURED, not asserted. *Alternative (offered): I/O micros only, defer real-app —
  rejected; dev wants both.* Also queued: F-024 JIT-coverage-of-real-programs metric (a coverage
  counter making "the JIT wins" quantifiable for real code).

## 2026-07-16 evening gap-session rulings (developer via AskUserQuestion, post-audit)

- **DEC-268 — RULED: THE CERTIFICATION LADDER, MAXIMAL tier** (replaces the unexecutable
  "advisor = Opus for the build phase" ruling — an advisor below the main model does not
  activate, and a same-model Fable advisor errored `unavailable`). Every 3C pre-work AND every
  6C pre-completion gate, ALL task sizes: a **3-lens fresh-context reviewer PANEL**
  (correctness+regression / security+safety-promises / completeness+blast-radius), each lens
  adversarial and EVIDENCE-BASED (reads the actual diff/tests/specs itself — never the author's
  narrative); **TWO consecutive fully-clean rounds** required (a finding → fix → the clean
  counter resets); cap 5 rounds → ask-human with the open findings, never silently proceed.
  Availability chain: advisor() if it activates → reviewer subagents → 3 distinct-lens
  self-passes + MANDATORY disclosure. The mechanical quality gate (oracle + byte-identity +
  clippy + fmt) is always the floor, never the certification. Cost accepted: ~6–10 reviewer
  agents per slice. Recorded in project CLAUDE.md + global CLAUDE.md 3C/6C. *Alternatives
  (offered, rejected): risk-tiered ladder (panel only for spine/security; single reviewer
  elsewhere — dev chose uniform maximum); double-clean-Tier-S-only; restoring the old
  30-cycle/8-clean self-convergence gate (structurally weaker: self-grading blind spot is
  exactly what certification exists to remove).*
- **DEC-269 — RULED: per-feature perf gate = WIN-OR-FLAG precedence.** The PER-FEATURE PERF GATE
  ("every new feature ships its micro and must score ≥1.0×") is AMENDED: ≥1.0× is the target;
  after ALL levers are exhausted on a shape, a **LOSS-FLAGGED entry with anatomy + queued
  levers** is an acceptable definition-of-done — ratifying existing practice (jsonround 0.25× /
  dbwork 0.63× shipped flagged). Rider (developer, verbatim intent): **perf work is continuous
  as features ship** — never batched away to a distant hold. *Alternatives (offered, rejected):
  hard-blocking gate (retroactively invalidates shipped flagged work; blocks progress on
  structurally hard shapes); split micro-hard/macro-flag bar (two bars = ambiguity).*
- **Scheduling (not DEC rows):** next session (home) = extension-policy adjudication (§10
  Bucket 2 — the 100%-parity blocker) → docs/ cleanup slice (4-living shape: MASTER-PLAN,
  UNIFIED-SPEC, C-decisions, M-gap-matrix; rest folded/archived, full reference sweep) →
  Tier 1 DEC-263. Gap ledger = MASTER-PLAN §0.3.

## 2026-07-16 evening extension-policy adjudication (developer via AskUserQuestion; panel-certified brief, DEC-268 ladder — round 2 escalated to ask-human per findings)

- **DEC-270 — RULED (new flag F-028, SECURITY, Tier 1): Core.HttpClient has no SSRF guard.**
  `src/native/http_client.rs:352-359` resolves via `ToSocketAddrs` and connects to `.next()` with
  ZERO filtering — `HttpClient.get("http://169.254.169.254/…")` reaches cloud-metadata credentials;
  internal-host fetches (`http://10.0.0.5/admin`) are open; the DEC-264 redirect follower can be
  pointed at a private IP after a public first hop. The 2026-07-16 D4 audit caught the redirect
  HEADER leak (DEC-264) but MISSED the SSRF surface. FIX (Tier 1, alongside 263/264/265):
  SSRF deny-by-default — block loopback / RFC1918 / link-local / `0.0.0.0` / metadata-IP
  (`169.254.169.254`); DNS-PIN (resolve once, connect to the resolved IP, RE-CHECK the pinned IP
  after every redirect hop); explicit opt-in to reach private ranges. Implemented as a SHARED
  Transport-seam policy so the future Core.Net inherits it. *Alternatives (offered): record+rule-later;
  investigate-first — dev chose rule-now into Tier 1.*
- **DEC-271 — RULED: icu4x admitted (dependency-policy AMENDMENT) for the joint intl/Unicode-data
  question; Core.Intl formatter module, quarantined, native-only.** icu4x = pure-Rust, feature-gated
  NON-DEFAULT, FEATURE-excluded from the playground wasm (not target-gated — it compiles to wasm, so
  target-gating would NOT keep it out). Powers BOTH DEC-256's grapheme feature AND a new Core.Intl
  (NumberFormatter/DateFormatter/Collator/Transliterator). Core.Intl is differential-QUARANTINED
  (`pure:false` seam — locale output can't be byte-identity-gated against `php -n`'s SYSTEM ICU;
  quarantine removes that oracle constraint, so the Collator/Transliterator GAP-by-design rejections
  REOPEN — their sole recorded reason was the oracle). PHP leg = `E-TRANSPILE-INTL` (LADDER case-2,
  native-only) initially. **DEC-256 JOINT STAMP (required):** the segmentation-table dependency
  question DEC-256 deferred is RESOLVED HERE = icu4x, feature-gated, non-default; the default string
  measure STAYS codepoints (DEC-256's graphemes-as-default rejection is NOT reopened — admitting a
  table for the grapheme FEATURE ≠ making it the default). Parity: ~5 net-new FN-INTL flips (3
  formatter GP + 2 reopened GD); the 3 grapheme rows stay credited to DEC-256 (no double-count); the
  10 GU rows stay GAP pending their own rulings. This is a dep-DOMAIN EXPANSION (i18n is not an
  enumerated admitted domain) → recorded as a policy amendment, not a mechanical row-add. icu4x's
  baked locale-data blob enters the cargo-audit/deny update cadence (supply-chain ownership).
  *Alternatives (offered, rejected): codepoint-only-defer-all-formatters (leaves intl GAP); rule-
  direction-defer-data-source.*
- **DEC-272 — RULED: four MANDATORY security riders (all ratified), written into the relevant pack
  specs as binding rules, not brief prose.** (1) **Locale-independent security comparisons** —
  `equalsIgnoreCase`/`containsIgnoreCase`/any equality-normalization pinned to Unicode SIMPLE
  (locale-independent) fold, stay `pure:true` + byte-gated, FORBIDDEN from routing through icu4x
  locale-tailored casemap or the quarantined seam (kills the Turkish-i auth-bypass class:
  `"ADMIN".equalsIgnoreCase("admın")` must never be true; locale-full-fold only in explicitly
  locale-parameterized formatters). (2) **Misuse-resistant crypto surface** — no user-supplied raw
  nonces (auto-nonce / XChaCha20 default); keys are `Secret<T>` by construction, `.expose()` only at
  the RustCrypto boundary; AEAD decrypt = authenticated-or-fault; reject non-canonical/low-order curve
  points. (3) **Socket/image secure-defaults** — Core.Net TLS-or-refuse (not opportunistic) + the
  DEC-270 SSRF rider + rides the existing Transport seam; image decode = mandatory dimension/alloc
  Limits + panic-catch boundary (decompression bombs survive memory-safety). (4) **Advisory-naming +
  guard-hardening** — finfo named advisory (`sniff*`/hint, never `validate*`/`mimeType`, doc "not a
  security control"); readline history opt-in + Secret-prompt reads never persisted; the tier-3
  emitted PHP guard validates the ext token (`^ext-[a-z0-9_]+$`) + emits only escaped literals.
- **DEC-273 — IN PROGRESS (developer wants a brainstorm + list-lock before ruling): the CORE vs
  EXTENSION architecture.** Developer ruling direction (Q2): strategy = a COMBINATION of per-family
  native (option 1) + a plugin/extension architecture (option 2) — everything that is "a framework,
  not the language itself" (DI cited as the example) ships as a build-flag activatable/deactivatable
  EXTENSION, structured so external rust-phorj plugins can register through the same seam. Crypto is
  an already-admitted dep-domain (no amendment); icu4x + image are domain expansions. The concrete
  CORE/EXTENSION partition + the governing criterion are being brainstormed and will be locked next
  (see the session brief). *Panel note: DEC-268 round-1 hardened this brief through 3 lenses;
  round-2 lens-1 clean, lenses 2+3 surfaced developer-decisions (SSRF, Turkish-i, pack boundaries) →
  escalated to ask-human rather than looped to the 5-round cap. Certification: self-graded fallback
  disclosed (advisor unavailable — no peer above Fable-main).*

## DEC-273 — RULED (2026-07-16 evening): THE MINIMAL-CORE / EXTENSION ARCHITECTURE (supersedes the IN-PROGRESS stub above)

**The single largest architectural ruling since DEC-208.** Developer-adjudicated over ~6 AskUserQuestion
rounds (each with challenge/criticism as requested). Governing rationale (developer, verbatim intent):
a general-purpose MINIMAL core with everything else as extensions buys **maintainability, scalability,
readability, and parallel extension development** — plus a future mandatory-vs-opt-in extension tiering.

### The criterion (final)
- **CORE** = what phorj-the-language **cannot function or do real work without** — the irreducible Rust
  that phorj cannot express in itself. "Written in Rust (the compiler/interpreter) and can't be done in
  the phorj language without the Rust part."
- **EXTENSION** = anything **expressible in phorj itself** (a `.phg` library could provide it) — phorj
  functions without it; it's an add-on capability / format / framework. The classification TEST is
  "could this be a `.phg` library on top of the kernel?" → yes = extension.
- **CRITICAL — the test is NOT an implementation mandate.** `.phg`-expressibility only CLASSIFIES a
  module as an extension. **Every module, core AND extension, is written in RUST + JIT-optimized**
  (or any other optimization) — self-hosting is NOT a goal. `Core.Db` is the proof: a Rust extension,
  flag-gated AND fast. An extension's build flag gates BUILD-INCLUSION, never implementation language
  or speed. (The perf mandate — 21 micros ≥1.0×, beat PHP — is fully preserved: nothing moves to
  interpreted `.phg`.) Third-party plugins MAY be `.phg` or Rust.

### The CORE list (minimal, irreducible, always-on, Rust+JIT, never toggleable)
1. **Language kernel** — lexer/parser/checker/backends (interpreter/VM/transpiler)/JIT.
2. **Primitive value types + their VM-primitive Ops** — int/float/bool/string(bytes)/List/Map/Set +
   arithmetic/comparison/index/concat/etc. (you can't build these in phorj without themselves).
3. **Raw OS/runtime seams** — thinnest I/O under File/Fs/Process/Environment; entropy (Random);
   raw Output/Log WRITE primitive (stdout/stderr); Runtime (exit/onShutdown).
4. **Reflection primitive** — runtime type info the language provides (rich reflection libs = extension).
5. **Secret type + universal redaction** (DEC-263) — checker/backend-enforced safety primitive.
6. **Option/Result + the error-model machinery** — the `?` operator, null-safety `T?`, checked-exception
   throw/catch are LANGUAGE features that require these built-in types to exist.
7. **Conversion + Bytes primitive coercions** — welded to the value kernel.
8. **Math over primitives** — arithmetic/float ops mapping to VM ops (Decimal/BigInt are NOT here — extensions).
9. **User-attribute (`#[Attr]`) + generics machinery** — language syntax/semantics the checker needs
   (attribute-macro LIBRARIES + the DI container that use them = extensions).

### EXTENSIONS (everything else — Rust+JIT, flag-gated, plugin-registerable via public trait seams)
- **Rich methods on the primitive types** — String.replace/split/trim/pad/format/levenshtein/Unicode-case,
  List.map/filter/reduce/sort, rich Map/Set ops. (Structurally extensions → become a MANDATORY/default
  extension so `List.map` needs no import in practice — see tiering below.)
- **Formats/data** — Json, Csv, Ini, Encoding, Decimal, BigInt, Uri, Path.
- **Text/i18n** — Regex, Intl (icu4x, DEC-271), I18n (catalogs).
- **Crypto** — Hash + basic password crypto (argon2), advanced sodium-class AEAD/sign (DEC-272 riders).
- **Dev tooling** — Debug (dump/dd — introspection SEAM stays core, module is extension), Test, Bench.
- **Web/data frameworks** — Db(+drivers), ORM, migrations, Http(server), HttpClient (DEC-270 SSRF rider),
  WebSocket/SSE, Template (Html TYPE + auto-escape SEAM stays core; engine/components = extension),
  Form, Session, CSRF, Serialize.
- **Comms/media/net** — Mail (lettre), Image (decode-limits rider), Net (sockets, TLS-or-refuse + SSRF rider).
- **Architecture** — DI container, Cache, observability, Signals/Scheduler, concurrency FRAMEWORK
  (green-thread spawn SEAM stays core), parallel workers.
- **Meta** — attribute-macro libraries, user-lint packs, FFI, embeddable phorj.
- **DateTime** (DEC-247) — extension (tz-data dep).

**The SEAM/module split pattern (recurring):** where a capability needs an irreducible primitive, the
primitive SEAM stays core and the module built on it is an extension — Html (interpolation auto-escape
hook = core; engine = extension) · Debug (walk-any-value introspection primitive = core; dump/formatting
= extension) · concurrency (spawn seam = core; structured-concurrency framework = extension) ·
Output/Log (raw write = core; leveled/formatted logging = extension).

### Extension mechanism + tiering
- **Mechanism:** first-party extensions = separate in-repo modules behind Cargo features, each
  registering via a PUBLIC trait seam (DriverConn/Transport/MailTransport already prove it); a
  manifest/registry so `phg` + third-party rust-phorj plugins discover them. Flags:
  `cargo build --release --di --http …` (activate/deactivate per extension).
- **Default build:** batteries-included (curated default set compiled in). Importing a disabled
  extension = a clean compile error `E-EXTENSION-DISABLED` naming the flag to add (mirrors the existing
  `E-MODULE-UNAVAILABLE`) — never a runtime surprise.
- **FUTURE tiering (developer, deferred):** extensions split into MANDATORY/default-installed (e.g. rich
  collections/string methods — ergonomics preserved) vs OPT-IN; which are default-installed vs opt-in is
  a later ruling. Recorded as a follow-up, not decided tonight.
- **AMENDMENT 2 — RULED (2026-07-16, developer at desk via AskUserQuestion, with previews):
  the extension PHYSICAL LAYOUT + DISCOVERABILITY surfaces.** (a) Layout = `src/ext/<name>/`
  self-contained folders (natives + the extension's prelude source + PHP-twin helper emission +
  tests colocated; `src/ext/registry.rs` = THE one-row-per-extension list; the `cli/preludes.rs`
  monolith dissolves as each extension migrates); core stays put. *(Workspace-crates and
  flat+manifest-only declined.)* (b) Discoverability = BOTH a `phg extensions` CLI listing
  (name/state/enable-flag/provided modules, read from the same registry the compiler uses) AND a
  `docs/EXTENSIONS.md` manifest regenerated from it. (c) TIMING = original sequencing confirmed
  ("finish everything as fast as we can respecting all the rules! then we migrate!") — the
  migration keeps its dedicated DEC-273 slot after the build queue.
- **AMENDMENT — RULED (2026-07-16, developer at desk via AskUserQuestion): `phg transpile` and
  `phg lift` become EXTENSIONS in the MANDATORY tier** ("they should be extensions but
  mandatory"). Structurally behind the extension seam like Debug/Test/Bench (neither is a
  runtime component), but ALWAYS compiled into the default build — which by construction keeps
  the byte-identity spine's PHP leg in every gate/CI build (the jit-default precedent). A build
  that explicitly compiles them out gets the clean `E-EXTENSION-DISABLED` on `phg transpile` /
  `phg lift`; the playground's PHP-output pane keeps the flag in its wasm build. First two
  entries of the MANDATORY tier list. Builds with the DEC-273 migration wave.
- **Namespace:** extensions KEEP the `Core.` import root (Core.Json stays Core.Json) — only BUILD
  membership + the flag change, so the reclassification is source-churn-free on imports.

### Migration
- **Model RULED now; physical migration = its own dedicated fresh-context slice**, sequenced
  **after Tier-1 security + the docs-cleanup slice** ("as soon as we can" — developer). Large blast
  radius (every import stays valid via the kept `Core.` root, but CORE_MODULES registry + preludes +
  Cargo features + docs all move). The migration slice gets the FULL DEC-268 panel.

*Alternatives rejected across the rounds: two-tier literal native-vs-framework (breaks String/Regex);
N/S/X three-tier with a named "standard library" middle tier (developer chose to collapse S into
extensions for maintainability); keeping rich methods in core (developer chose minimal core + a future
mandatory-extension tier instead); rewriting extensions in .phg (kills perf); re-rooting to Ext.
namespace (unnecessary churn). Certification: DEC-268 panel hardened the extension-POLICY brief (2 rounds,
3 lenses); this architecture ruling is the developer's own via AskUserQuestion — recorded verbatim,
self-graded 6C disclosed (advisor unavailable; the migration BUILD gets the full panel).*

## DEC-263 — SHIPPED (2026-07-16, Tier-1 build): universal Secret redaction

Root cause of the F-025 leak: `src/native/debug.rs` had a SEPARATE value renderer that diverged from
`src/inspect.rs` (which already redacted) — a DRY violation. Fix single-sources the predicate:
`Instance::is_secret()` + `SECRET_CLASS`/`SECRET_REDACTED` consts in `src/value/types.rs`, shared by
`debug.rs` (Debug.dump/dd — the leak), `inspect.rs` (faults/REPL/DAP — already safe, now routed through
it), and the transpiled-PHP twin `__phorj_debug_render`. A Secret renders `Secret(<redacted>)` on ALL
surfaces, directly AND transitively, byte-identical across run/runvm/PHP. `as_display` returns None for
instances so interpolation/print/toString already refuse them (no change). `.expose()` + W-SECRET intact.
Coverage: unit test `secret_is_redacted_never_walks_its_value_field` (direct + transitive) + example
`examples/guide/secret.phg` (single-package) + `examples/project/secretdump/` (multi-package/namespaced
regression, gated on all 3 backends). Gate green: 2159 tests w/ PHORJ_REQUIRE_PHP=1, clippy both configs,
fmt. Certified by the DEC-268 panel (2 rounds, 3 lenses): round 1 found the namespaced-PHP miss (`get_class`
= `Main\Secret`, fixed by trailing-`\Secret` match) + literal-duplication (fixed) + the pre-existing
F-029 family (flagged, scoped out); round 2 security-CLEAN, its lone code finding (gate-ineffective) was
empirically disproven (revert test: reverted twin prints `Main\Secret {}` → differential fails as intended).
**Spawned F-029** (KNOWN_ISSUES): two PRE-EXISTING namespaced-transpile byte-identity bugs (injected types
mis-namespaced as cross-package field types → PHP TypeError; Debug.dump bare-name divergence for
Main-package classes/enums) — each its own future slice.

## DEC-264 — SHIPPED (2026-07-16, Tier-1 build): HttpClient cross-origin redirect credential strip

`src/native/http_client.rs` `run_request` re-sent the SAME headers to every redirect hop with no
origin check (F-026 / curl CVE-2022-27774 class). Fix: three pure helpers — `same_origin` (scheme
bool + host ASCII-ci + port, default-port-normalized), `is_credential_header` ({authorization, cookie,
proxy-authorization, www-authenticate}, ci), `headers_for_hop` (same-origin keeps all; cross-origin —
incl. https→http downgrade — filters the credential set). The loop narrows the working header set at
each hop BEFORE the exchange to the new origin (no off-by-one) and never re-widens (a dropped credential
stays dropped even on return to the origin). Coverage: 3 tests (same_origin incl. same-port/differing-
scheme isolation; headers_for_hop keep/strip/downgrade; e2e with a head-capturing fixture asserting the
cross-origin hop dropped Authorization + kept X-Trace + leaked no token) + the existing redirect tests.
Invariant-9: impure/quarantined → documented in `examples/http-client/fetch.phg` + examples/README (can't
be a deterministic runnable example — needs two live origins). En-route: fixed a pre-existing
clippy::collapsible_if at http_client.rs:328 (the http-client feature is non-default, so the standard
`--features jit` gate never compiled/linted this file — a gate-coverage gap worth noting). Gate green:
2174 tests PHORJ_REQUIRE_PHP=1 --features jit,http-client, clippy (jit,http-client) clean, fmt. DEC-268
panel (2 lenses): security CLEAN; correctness CLEAN-on-code + one P2 test-coverage gap (scheme term
masked by default-port asymmetry) fixed with a test-only assertion. Composes with DEC-270 (SSRF, next):
the strip is header-scoped, SSRF is destination-scoped; both ride the future Transport seam.

## Gate policy — ALL-FEATURES standing gate (developer-ruled 2026-07-16, during DEC-264 build)

The full correctness gate + pre-push hook now run `--all-features` (clippy + tests) instead of
`--features jit`. Rationale: the non-default features (`http-client`, `mail`, `db-postgres`, `db-mysql`)
were NEVER compiled/linted/tested by the standing gate — a real coverage hole that hid pre-existing
clippy lints (`http_client.rs`, `db/mysql.rs` collapsible-ifs, both fixed this build). `--all-features`
subsumes the old separate `--features db` pre-push step. clippy also runs `--no-default-features` (the
jit-off/minimal end). Live DB/mail/http round-trips self-skip without their `PHORJ_*_TEST_DSN`/server
env (skip-loud), so the gate needs no live servers. Recorded in `CLAUDE.md` (Toolchain & quality gate)
+ `scripts/git-hooks/pre-push`. *Alternatives (offered, rejected): per-slice features (leaves the hole);
separate gate-infra slice later (the hole keeps hiding lints meanwhile).*

## DEC-270 — REFINED (2026-07-16, developer via AskUserQuestion, at implementation time)

The audit-desk DEC-270 ruling (SSRF deny-by-default for loopback + private + link-local + metadata) is
REFINED now that it meets real usage: **default-BLOCK RFC1918 (10/8, 172.16/12, 192.168/16) +
link-local/metadata (169.254/16, incl. the cloud-credential endpoint 169.254.169.254) + IPv6 ULA
(fc00::/7) + IPv6 link-local (fe80::/10) + 0.0.0.0/unspecified; default-ALLOW loopback (127.0.0.0/8, ::1).**
Rationale: loopback is overwhelmingly INTENTIONAL (local services, sidecars, dev servers), whereas
metadata + internal-LAN are the actual SSRF-exfiltration targets DEC-270 exists to stop. Opt-in
`allowPrivateHosts(true)` reaches the blocked ranges deliberately. Bonus: the existing http_client tests
(all on 127.0.0.1) stay valid. IPv4-mapped-IPv6 addresses are unwrapped and re-checked (no bypass).
DNS-PIN unchanged (resolve once, connect to the resolved IP, re-check across redirect hops — anti-rebind).
*Alternatives (offered, rejected): block-all-incl-loopback (literal ruling — high friction, breaks
localhost + all tests); block-metadata-only (leaves internal-LAN SSRF open). This is the DEC-272 socket
secure-default rider; the future Core.Net inherits it via the shared Transport seam.*

## DEC-270 — SHIPPED (2026-07-16, Tier-1 build): HttpClient SSRF guard

`exchange` connected to the resolved addr with NO filtering (F-028). Fix: `is_blocked_ip` (pure,
unit-tested) refuses by default — RFC1918 + CGNAT 100.64/10 (RFC 6598, holds Alibaba metadata
100.100.100.200) + 192.0.0.0/24 (IETF assignments incl. 192.0.0.192) + link-local 169.254/16 (incl.
the 169.254.169.254 cloud-metadata endpoint) + 0.0.0.0/:: + IPv4 broadcast + IPv6 ULA fc00::/7 +
IPv6 link-local fe80::/10; ALLOWS loopback (127/8, ::1 — the refined DEC-270 ruling). `embedded_v4`
decodes every IPv6→IPv4 embedding (mapped ::ffff, compatible ::a.b.c.d, 6to4 2002::/16, NAT64
64:ff9b::/96) and re-checks the embedded v4 — closes the NAT64/DNS64 bypass. DNS-PIN: resolve once,
check the resolved IP, connect to THAT SocketAddr (no re-resolve → no rebind window); each redirect
hop re-resolves+re-checks its own host (composes with DEC-264). Opt-in `HttpClient.allowPrivateHosts(true)`
threads `allow_private` through the prelude → `HttpClientSys.request` (8th arg, native sig `Ty::Bool`)
→ run_request → exchange. Blocked → typed `BlockedAddress extends HttpClientError` (`<<BlockedAddress>>`
marker; the error names the REQUESTED host, not the resolved IP — no DNS oracle). Coverage: is_blocked_ip
unit test (every blocked range + IPv6 embeddings + over-block guards pinning public 100.x/6to4/TEST-NET)
+ run_request default-block/opt-in-bypass e2e + a live `phg run` smoke (metadata blocked, opt-in
proceeds). Gate: 2205 tests --all-features + oracle, clippy (all-features + no-default), fmt. DEC-268
panel (2 rounds): R1 correctness clean-on-code + security P1 CGNAT + P2s (all fixed by widening +
error-hardening); R2 CLEAN (no over-block of public IPs; bit-extraction verified). This IS the DEC-272
socket secure-default rider; the future Core.Net inherits it via the shared Transport seam.

## DEC-265 — SHIPPED (2026-07-16, Tier-1 build): SMTP require-TLS when credentials are set

`smtp_inner` used `builder_dangerous` + `Tls::Opportunistic` even WITH credentials (F-027) — a MITM
stripping the STARTTLS advertisement forced plaintext and the AUTH password rode in cleartext. Fix:
`smtp_tls_choice(has_creds, allow_insecure, mode, port)` (pure, unit-tested) — no-auth fakers stay
Opportunistic (nothing to protect), but AUTHENTICATED connections REQUIRE TLS by default: implicit
(`Tls::Wrapper`) on port 465, STARTTLS-required (`Tls::Required` — fails closed if the server won't
upgrade) otherwise. The mode is chosen by `SmtpConfig.tls` = "auto"|"starttls"|"implicit" (an
unrecognized value fails SAFE to required-TLS — a typo can never downgrade to plaintext). The ONLY way
to permit authenticated plaintext is the explicit, loud `SmtpConfig.allowInsecureAuth = true` opt-out
(DEC-272 misuse-resistant surface). Invariant (unit-tested exhaustively): authenticated + not-opted-out
is NEVER Opportunistic. Verified against lettre 0.11.22 (Required→starttls() unconditional, errs on
no-STARTTLS; auth() only after TLS; peer certs validated — no fake-cert downgrade). Native sig
`smtp` 4→6 args (tlsMode String + allowInsecureAuth Bool); prelude connectSmtp threads them.
**DEVIATION (disclosed):** `tls` is a STRING not a typed `SmtpTls` enum — ctor default params must be
literal constants (an enum value isn't one; `E-DEFAULT-PARAM-EXPR`), and Optional<enum> matching
(DEC-250) is unbuilt. Fail-safe-secure; a typed enum replaces it once DEC-250 or const-enum-defaults
land — tracked. Gate: 2206 tests --all-features + oracle, clippy (all-features + no-default), fmt.
DEC-268 panel (2 lenses, R1 both CLEAN — no findings): security (invariant + lettre source-level) +
completeness/regression/API. This completes the DEC-272 socket/transport secure-default riders.

## DEC-251 — SLICE (a) SHIPPED (2026-07-16, Tier-1 build): override parameter contravariance

Check (a) of the three PHP-enforcement-ahead checks. `src/checker/collect/interfaces.rs` — extends the
existing `E-OVERRIDE-SIG` return-covariance block (the exact structural twin) with a PARAMETER check:
an override's parameter types are CONTRAVARIANT — widening (accepting a supertype) is sound + PHP-legal,
but NARROWING a parameter type-checked clean before and was **transpile-fatal** in PHP ("Declaration must
be compatible") + unsound on the Rust backends. Rule per META-7 survey (Kotlin/C# invariant params, PHP
contravariant): the parent's param type must be `ty_assignable` TO the child's at each position; scoped
to the same-arity, single (non-overloaded), non-generic case (mirrors the return check's scope;
overloaded/generic/default-arity-diff overrides stay documented deferrals). Checker-only, byte-identity
strictly improves. Tests: `override_narrowing_a_parameter_errors` / `_widening_a_parameter_is_ok` /
`_same_parameter_type_is_ok` (src/checker/tests/inheritance.rs). Gate: 2209 tests --all-features + oracle
(full corpus accepts it — no valid override wrongly rejected), clippy (all-features + no-default), fmt.
Certification: self-review + full-corpus gate + it is the exact structural twin of the already-shipped,
panel-clean return-covariance check (lighter than a 2-lens panel — disclosed; the DEC-268 panel runs on
the DEC-251 whole when slices (b) private/protected-static external-read + (c) intersection-receiver
visibility land). **REMAINING: DEC-251 (b) + (c)** — see the register row.

## DEC-251 — COMPLETE (2026-07-16, Tier-1): all three PHP-enforcement-ahead checks

- (a) SHIPPED `66594aba` — override parameter contravariance (E-OVERRIDE-SIG param twin).
- (b) ALREADY-DONE — private/protected STATIC external-read is enforced by the shipped W0-2 slice
  (`src/checker/calls/methods.rs` static-read → `enforce_member_vis`; probed: `C.secret` on a private
  static → E-FIELD-VISIBILITY). The audit flag was stale. No code needed.
- (c) SHIPPED (this commit) — visibility through INTERSECTION-typed receivers. Two `Ty::Intersection`
  member-access arms (`src/checker/calls/methods.rs`) returned the member without `enforce_member_vis`,
  so a private field/method on the class component of an `I & C` receiver was readable/callable from
  outside `C` (unsound + PHP-divergent). Fix: field arm enforces `field_vis` on the owning class; method
  arm enforces `method_vis` on the lone CLASS member (E-INTERSECT-MULTI-CLASS ⇒ ≤1), independent of the
  alphabetical member sort (`intersection_of`) — so an interface shadowing the name can't skip it.
  ROOT CAUSE also fixed: interface conformance now rejects a class implementing a public interface
  method as private/protected (`E-IFACE-VIS`, single-overload — see F-032 for the overloaded deferral),
  the PHP-fatal that enabled the bypass. `phg explain E-IFACE-VIS` added. Tests: intersection field/
  method/public + sort-order-shadow + overload-not-false-rejected + private-impl-rejected
  (src/checker/tests/inheritance.rs). Gate: 2215 tests --all-features + oracle, clippy (both), fmt.
  DEC-268 panel: R1 found the sort-order first-found bug + the conformance root cause (both P1) → fixed;
  R2 found two over-rejection P2s (overload false-positive + a test gap) → fixed; R3 CLEAN (residual
  overloaded-declaration-time deferral flagged F-032, panel-rated non-blocking). Byte-identity strictly
  improves. **DEC-251 whole is now COMPLETE.**

## DEC-252 — SHIPPED (2026-07-16, Tier-1): LSP ≡ check (prelude-injection fix)

The LSP's `diagnostics_for` (src/lsp/mod.rs) called `checker::check` DIRECTLY on the raw parsed
program (F-015), bypassing prelude injection + the desugar passes — so an injected-type program
(`import Core.Secret`/`Core.Db`/`Core.Json`) produced a wall of spurious `E-UNKNOWN-IDENT`s in the
editor while `phg check` was clean. Fix: new `pub fn front_end_diagnostics(prog)` (src/cli/pipeline.rs)
mirrors `check_and_expand_reified`'s EXACT pass sequence (enforce_injected_discipline →
resolve_intrinsic_imports → unavailable_core_module → inject_core_modules → desugar_auto_router →
collapse_injected_type_qualifiers → resolve_variant_imports → desugar_di → desugar_db →
check_resolutions) but returns STRUCTURED `Vec<Diagnostic>` (first failing pass's errors; else the
checker's warnings) instead of rendered strings; the LSP routes through it. Warnings now surface as
severity-2 editor diagnostics. **STANDING RULE (developer): `phg check` and the LSP never diverge —
same pipeline, kept in sync as part of every diagnostics change.** Drift guard: `front_end_diagnostics_
agrees_with_check` (pipeline.rs tests) asserts the two agree on error-presence across clean/error/
injected-type/injected+error programs — a pass added to one but not the other fails the suite (this is
the REAL guard; the earlier comment overstated a nonexistent shared corpus — corrected). Pinning test:
`open_injected_type_program_publishes_no_spurious_diagnostics` (lsp/tests.rs). Gate: 2217 tests
--all-features + oracle, clippy (all-features + no-default), fmt. DEC-268 panel: R1 core CLEAN + one
P2 (overstated drift protection) → fixed with the equivalence test + corrected comment.

## DEC-255 — SWEEP RUN (2026-07-16, Tier-1): fault-parity exit-status catalog + findings (PENDING 2 rulings)

Swept every fault-triggering op: phorj VM/interp exit vs transpiled-PHP-8.5.8 exit (catalog:
`scratchpad/DEC-255-catalog.md`, mirrored below). **7 SILENT-DIVERGENCES** (phorj faults exit 1; PHP
silently succeeds exit 0) — all INTENTIONAL-STRICTER (phorj deliberately checked/safe) but UNENFORCED
on the PHP leg, breaking Invariant-1 byte-identity in the FAULT direction:
- **Checked-arithmetic overflow family:** int `+`/`-`/`*`, unary neg, `Math.abs`(i64::MIN), `Math.pow`,
  `List.sum` — transpiled PHP wraps to float (exit 0) where phorj faults "integer overflow".
- **Index/key family:** list index OOB (`xs[10]`), Map key-not-found — PHP returns null+Warning (exit 0).
13 MATCH (div0/mod0/float-div0 → PHP DivisionByZeroError; decimal-inexact/truncate/force-unwrap/assert/
panic/todo/unreachable/range → PHP throws via `__phorj_*` helpers/real throw; sqrt(-1)/log(0) → both
NaN/-inf; parseInt/as-int → both Option/null). 0 reverse-direction (no PHP-faults-phorj-succeeds).
**STRUCTURAL:** the differential harness (`tests/differential.rs`) never runs FAULT programs through PHP
(`run_php` asserts success; `agree_err` compares only run≡runvm by FaultKind) — so these were uncovered.
**CONTRADICTS DEC-226** ("checked default transpiles faithfully") — the checked default silently wraps.
Discriminator = each native's `php:` emitter (helper-vs-lenient-builtin). PENDING developer rulings (2,
per META-7 helper-vs-accept — asked, not self-decided).
⊳ RULED + BUILT — see the next section (2026-07-16 ruling); the helpers (`__phorj_checked_*`,
`__phorj_index`, `__phorj_map_set`/`__phorj_map_remove` — the Map-key *read* was never routed
through a helper) live in `src/transpile/{gates,expr,call,runtime_php,stmt}.rs`.

## DEC-255 — RULED (2026-07-16, developer via AskUserQuestion): emit throwing helpers for BOTH families + close the harness gap

Both silent-divergence families get throwing `__phorj_*` PHP helpers so transpiled PHP faults
identically (byte-identity restored, per META-7 — helper is the accepted tool):
1. **Checked-arithmetic overflow:** int `+`/`-`/`*`/unary-neg + `Math.abs`/`Math.pow`/`List.sum` emit
   `__phorj_checked_add/sub/mul/neg/abs/pow/sum(...)` that throw an overflow error (PHP `intdiv`-style
   fault) instead of the bare lenient operator/builtin that wraps to float. Corrects DEC-226's
   "checked default transpiles faithfully" (now actually true). Cost accepted: PHP-leg-only (phorj's
   interp/VM/JIT untouched); noisier PHP + small PHP-leg perf.
2. **Index/key:** list index + Map key reads emit `__phorj_index($xs,$i)` / `__phorj_map_get($m,$k)`
   that throw on OOB/missing instead of PHP's silent null+Warning.
Plus: **extend `tests/differential.rs` to run FAULT programs through PHP** (currently `run_php` asserts
success + `agree_err` compares only run≡runvm) — so fault-parity is gated and can't regress. Build NOW
(finishing Tier-1); each = emitter change + a fault-parity test (transpile the fault program, assert PHP
exits non-zero with the matching semantic) + example. *Alternatives (offered, rejected): accept+document
(gives up Invariant-1 fault-parity); helpers-only-where-cheap (partial).* Sub-slices: index/map helpers
(smaller) → checked-arith family → harness fault-leg extension. Each its own green + panel + commit.

## DEC-275 — RULED (2026-07-16, developer via AskUserQuestion): throwable-type naming = mandatory Error/Exception suffix, checker-enforced

Any class/enum that extends/implements `Error` MUST be named `*Error` OR `*Exception` — enforced
at declaration for stdlib AND user code (`E-ERROR-NAME`, clean message + rename hint). Motivating
case: `catch (InvalidUrl e)` reads ambiguous at every site (import, catch, throws). META-7 scan:
PHP/Java/C#/Kotlin = Exception suffix; Rust/Swift/TS = Error; Phorj's root marker interface is
already `Error` and every taxonomy base already ends in it — the developer ruled EITHER suffix
acceptable. Stdlib sweep = mechanical stem-keeping (`InvalidUrl→InvalidUrlError`,
`Timeout→TimeoutError`, `FsNotFound→FileSystemNotFoundError` post-DEC-276, `AuthFailed→
AuthFailedError`, `MailIo→MailIoError`, …). *Alternatives (offered): single-suffix-only
(rejected: dev wants either); Errors sub-package (rejected: fixes only the import line, catch
site stays ambiguous); stdlib-only/warning enforcement (rejected: "normal behavior" must hold
everywhere).* **BUILT 2026-07-17 fable** — `E-ERROR-NAME` at collect (keyed on the transitive
`class_implements` table, so subclasses of an error base are covered), explain entry, 2 checker
tests; stdlib sweep = 27 renames (Mail/HttpClient/Database condition types + the full UriBad*
family + TooManyRedirects/TooLarge, caught by the rule itself on the first gate run), sentinels renamed in lockstep on the native side; the rule now self-verifies the whole
corpus on every suite run. (The FileSystem family got its suffixes earlier, in the DEC-276
sweep.)

## DEC-276 — RULED (2026-07-16, developer, multi-round): the EARNED-SHORTCUT rule + rename sweep

**Rule:** a shortcut is legitimate ONLY when it is the industry-standard NAME of the thing
(acronyms of standards: Json, Csv, Ini, Html, Http, Uri, Smtp, Tls, Sql; also ruled-earned:
`Math`, `Debug.dd` (the PHP-world's own name for dump-and-die), `lsp`/`--dap`/`--eval`/
`--no-jit`/`--bin`/`--dev`/`--vs-php`, `phg`/`.phg` brand). Word-truncations are NOT earned.
**Renames ruled:** `Fs→FileSystem` · `Db→Database` (module, class, DbError→DatabaseError,
DbStream→DatabaseStream, DbHandle→DatabaseHandle) · `Reflect→Reflection` (unify with the
already-internal Core.Reflection) · `DI→DependencyInjection` (dev overrode the acronym carve-out)
· `HcHandle→HttpClientHandle` · CLI flags `--addr→--address`, `--proto→--protocol` (old spellings
= hidden aliases for one version). Function-name sweep clean (abs/sqrt/gcd/lcm/pow/min/max =
universal math names). `Core.File` vs `Core.FileSystem` coexist BY DESIGN (older transpilable
single-file ops vs typed native module — renames clarify the split). *Alternatives: keep-DI
(offered as earned, overridden); spell-out-everything (offered, narrowed to the ruled list).*

## DEC-277 — RULED (2026-07-16, developer): raw-native modules nest under `Core.Native.*`

The seven `*Sys` modules (raw Rust-implemented natives under the friendly preludes) become
`Core.Native.Database`, `Core.Native.FileSystem`, `Core.Native.Uri`, `Core.Native.Mail`,
`Core.Native.Session`, `Core.Native.Debug`, `Core.Native.HttpClient` — visible, explicit opt-in,
a hierarchy instead of a suffix ("Core. is enough — no suffix"). *Alternatives (offered):
hide-as-internal-only (E-INTERNAL-MODULE; recommended, not chosen); visible `*Native` suffix;
keep `Sys` (Rust *-sys precedent).* **AMENDMENT (2026-07-17, developer-ratified at build
review): `Core.Native.*` modules are WHOLE-MODULE-IMPORT ONLY** — a member import
(`import Core.Native.Uri.encodeForm;`) is `E-IMPORT-NATIVE-MEMBER` with guidance. Rationale
ruled: raw-layer usage stays VISIBLE (qualified `Native.Uri.encodeForm(…)`, greppable, reviewable);
the friendly wrappers' invariants (typed errors, Secret masking) aren't silently bypassed by
innocuous-looking bare calls; and member imports would need new import-map plumbing in all three
backends for an internal layer with no cherry-pick use-case. *Alternative (offered): widen the
backends' import maps — rejected.* Also ruled at review: NO old→new module hint table
("do nothing — all is migrated"); dead old paths (`import Core.Db;`) stay ordinary unknown
imports. BUILT 2026-07-17 (agent worktree, 3 adversarial review rounds; the ladder gate now also
covers direct raw-native imports — a pre-existing silently-diverging-PHP hole).

## DEC-278 — RULED (2026-07-16, developer, challenged + confirmed): namesake modules take the `Module` suffix

The SEVEN modules whose headline type shares the module leaf (Fs, Db, Uri, Session, Debug,
HttpClient, Iterator) rename to `Core.FileSystemModule`, `Core.DatabaseModule`, `Core.UriModule`,
`Core.SessionModule`, `Core.DebugModule`, `Core.HttpClientModule`, `Core.IteratorModule` — so
`import Core.FileSystemModule.FileSystem;` is fully explicit; non-namesake modules stay bare.
Parent-qualified access works DAY ONE via the existing DEC-234 machinery under the new qualifier
(`UriModule.UriMalformedError` in catch/type position, `new UriModule.Uri(…)`); DOUBLE-chained
statics (`UriModule.Uri.parse(…)`) = recorded follow-up slice. *Challenged (Claude): "Module" is
a zero-information suffix + mixed suffixed/bare surface; alternative namesake-auto-bind offered
twice — developer heard the challenge and confirmed the suffix as final.*

## DEC-279 — RULED (2026-07-16, developer): `Core.Url` merges into `Core.Uri`

Core.Url (older Tier-A percent-encoding helpers — encodeUriComponent, encodeForm, decode*) folds
into Core.Uri (→ UriModule per DEC-278); old paths go through the deprecation registry with a
"moved to Core.UriModule.…" message. *Alternative (offered): keep both with a documented split —
rejected (near-synonym module names are the ambiguity class being eliminated).*

**EXECUTION (all five):** ONE codemod-driven naming mega-slice (renames + E-ERROR-NAME checker
rule + deprecation-registry rows + docs/examples/editors), differential-harness verified.
SEQUENCED IMMEDIATELY AFTER DEC-257 completes — the sweep touches preludes/checker-registry/Db
streams, the exact files DEC-257 slices 2–3 are mid-flight on, so the truly-independent
precondition for a parallel worktree agent fails (Claude scheduling call, 2026-07-16); running
it after also avoids renaming RowStream/DbStream twice (slice 3 reshapes them).

## DEC-280 — RULED (2026-07-16, developer, challenged + confirmed): untyped foreach key–value bindings

`foreach (m as k => v)` becomes legal — both bindings inferred from the Map, exactly like the
single-binding form infers its element (removes the DEC-248 asymmetry: EVERY foreach binding may
now be untyped-inferred or typed; typed spellings stay legal; mixed forms too). Costs accepted on
the record: 1-token parser lookahead after `as` (pinned by a differential case), inferred loop
headers, use-site type errors (the `var` trade). LIFT: PHP's `foreach ($m as $k => $v)` upgrades
from Tier-2-reject to Tier-1, and the lift printer marks each such loop with an inline greppable
comment (`// lift: key/value types inferred — spell them out for an explicit header`) — the
developer's manual-types warning, challenged down from a blanket warning (legal idiomatic code
is not called wrong; the marker is local and actionable). *Alternatives (offered, 4-option
board): `var`-marker form; keep mandatory types (lift stays Tier-2); lifter-side partial
inference — all rejected.* **BUILT 2026-07-16 fable** — parser accepts bare/mixed bindings;
Invariant-7 hardening: `materialize_for_binds` writes checker-resolved types of inferred foreach
bindings (BOTH forms — the single-binding form had the same latent CTy gap, `v + 0` was rejected
on the VM) into the AST post-check; formatter round-trips the new spelling (fully-typed
two-binding keeps the `for (K k, V v in m)` canonical); lift emits the form Tier-1 + the ruled
inline marker; `private(set)`/`protected(set)` lift landed in the same slice (Invariant-17 debt);
differential-pinned via examples/guide/foreach.phg (`v * 2` on an inferred binding).

## Surface rulings batch (2026-07-17, developer via AskUserQuestion — upfront-adjudication lever)

- **DEC-256 surface:** NEW EXPLICIT functions — `String.length` stays byte-parity (strlen twin);
  Unicode tier = `codepointLength`, `graphemeLength`, `codepoints(s): List<int>`,
  `graphemes(s): List<string>`, `unicodeUpper`/`unicodeLower` (full case mapping).
  *Alternative (offered): breaking length-becomes-codepoints — rejected.*
- **DEC-242 surface (challenged + refined):** a first-class **`Cookie` VALUE class ONLY** (the
  developer's instinct, confirmed against the tenets; flat `Response.cookie(...)` twin REJECTED
  as two-ways): `new Cookie(name, value)` w/ DEC-249 defaults (path="/", secure=true,
  httpOnly=true, sameSite=Lax — injected enum, partitioned=false, optional maxAge/domain);
  `resp.withCookie(c)` (value-Response chaining free) + `withCookies(List<Cookie>)` for dynamic
  jars; Session's cookie becomes a Cookie internally; Partitioned = CHIPS opt-in (Session
  default OFF).
- **DEC-258 surface:** CONSTRUCTOR option — `new Database(dsn, naming = new Naming.Exact())`
  (DEC-249 default param; compile-time-literal rule like namingStrategy); per-statement
  `stmt.namingStrategy(...)` still overrides. *Alternative (offered): withNaming builder —
  rejected.*

- **DEC-256 dependency ruling (2026-07-17):** `unicode-segmentation` ADMITTED (feature-gated,
  vetted-exception list; graphemes only — codepoints/case are std). **AND: icu4x (DEC-271)
  BROUGHT FORWARD** in the queue (developer: "bring icu4x forward I think") — the fuller Unicode
  extension slice moves ahead of the remaining Tier-3 items once the DEC-256/242/258 batch lands.

- **DEC-256 placement ruling (2026-07-17):** SPLIT — `String.codepointLength`/`String.codepoints`
  stay on Core.String (transpilable; PCRE-`/u` PHP legs, always-in); Unicode CASE + GRAPHEMES =
  new **`Core.Unicode`** native-only module (`Unicode.upper/lower/graphemeLength/graphemes`,
  E-TRANSPILE-UNICODE ladder row — mbstring/intl are ini extensions, forbidden by the transpile
  rule). The module boundary IS the transpilability boundary; Core.Unicode is the icu4x landing
  zone (brought forward). *Alternative (offered): per-function ladder on String — rejected.*

- **DEC-256 placement OVERRIDE (2026-07-17, developer mid-build): everything stays under
  `Core.String`** ("keep unicode/string together") — the split into Core.Unicode is REVOKED.
  Names per the original approved preview: `String.codepointLength/codepoints` (transpilable,
  PCRE) + `String.unicodeUpper/unicodeLower/graphemeLength/graphemes` (native-only). The ladder
  therefore goes PER-FUNCTION: the four native-only String functions carry a transpile marker;
  the transpiler's native-emission chokepoint turns an actual CALL into E-TRANSPILE-UNICODE
  (import alone stays fine — Core.String is otherwise transpilable).

- **DEC-191 addenda (2026-07-17, developer):** (a) `#[Entry]` is IMPORT-GATED after all —
  `import Core.Runtime.Entry;` (wind rule; UncheckedOverflow precedent; supersedes the earlier
  no-import reading of the approved preview). (b) NO manual-function-run CLI affordance
  ("everything will be orchestrated by the Entry") — subcommand dispatch is userland inside the
  entry; --call/named-entries alternatives offered and rejected. (c) Confirmed semantics: an
  un-attributed `main` is an ordinary function — direct calls (`main();`) work everywhere;
  argv fills `(List<string>)` entries (verified live: `-- hello world` → ["hello","world"],
  int return = exit status).

- **DEC-258 REFINEMENT (2026-07-17, developer — "combine all three; naming is a promoted ctor
  FIELD, visible from any scope"): the COMBINED naming model.** Three cooperating tiers:
  (1) construction visible in the analyzed scope → compile-time BAKING (zero runtime cost,
  today's mechanism); (2) connection NOT statically traceable (parameter / field / cross-function
  flow) → the desugar emits BOTH baked helper variants (Exact + SnakeToCamel) and dispatches on
  the runtime `db.naming` field — cost = one branch per hydration call, never per-row string
  work; (3) per-statement `stmt.namingStrategy(<literal>)` overrides both. The developer's field
  insight is what makes it sound: `naming` is a promoted constructor field, so it EXISTS on the
  Database value at runtime and follows the value into any scope — the runtime dispatch tier is
  reading a fact the value already carries, not re-deriving one. No silent downgrade anywhere;
  Db is native-only (E-TRANSPILE-DB ladder), so there is no PHP-leg complexity. *Alternative
  (offered): uniform always-dispatch-on-field (drop the baking tier) — rejected in favor of
  zero-cost-where-traceable.*

- **DEC-256 BUILT (2026-07-17):** the Unicode tier shipped under `Core.String` per the override —
  transpilable `codepointLength`/`codepoints` (PCRE `/u` + pure-PHP UTF-8 decode legs) +
  native-only `unicodeUpper`/`unicodeLower`/`graphemeLength`/`graphemes` (std case tables;
  UAX #29 via feature-gated `unicode-segmentation`, default-on `unicode` feature). Per-function
  ladder: the four native-only functions carry a transpile marker → the transpiler chokepoint
  turns an actual CALL into `E-TRANSPILE-UNICODE` (import alone stays transpilable). Examples
  `guide/unicode-codepoints.phg` (3-leg) + `guide/unicode-native.phg` (run≡runvm, ladder-gated).

- **DEC-242 BUILT (2026-07-17):** `Cookie` value class + `SameSite` enum on `Core.Http`
  (import-gated, wind rule). Immutable safe defaults (Secure; HttpOnly; SameSite=Lax; Path=/),
  chainable `.path/.secure/.httpOnly/.partitioned` copy-builders, canonical `render()`.
  BREAKING: `Response.withCookie(Cookie)` replaces `(name, value)`; new
  `withCookies(List<Cookie>)`; Session's sid cookie now built through `Cookie` (`.secure(false)`).
  Example: `web/response-builders.phg` reworked, 3-leg identical.

- **DEC-191 addendum BUILT (2026-07-17):** `#[Entry]` import-gated via `Core.Runtime` registry
  row (`bare_types` += `Entry`); zero-span synthetic exemption for compiler-injected entries
  (`phg test`, web bridge, lifted drafts — the lifter also emits the import); the whole corpus
  (examples/conformance/tests/embedded programs, ~160 insertion sites) migrated; the four inline
  test helpers (`cli::wp`, compiler/interpreter/differential `with_pkg`) inject the import AFTER
  the package segment (import-before-package was a parse error). DAP test breakpoint re-lined
  (+1 from the injected import line).

- **DEC-281 — RULED (2026-07-17, developer): `Core.Input` — the stdin module (Output's twin).**
  Piped/redirected data (`cat file | phg run s.phg`, `phg run s.phg < file`) is unreadable today
  (no stdin API — verified; `echo name |` pipes the filename STRING, challenged and confirmed).
  FULL surface ruled ("Okay for option 1"): `Input.readAll(): string`, `readAllBytes(): bytes`,
  `readLine(): string?` (null at EOF), `lines(): Iterator<string>` (DEC-257 foreach-able),
  `isInteractive(): bool` (TTY-vs-pipe, PHP `stream_isatty` parity). Impure natives
  (differential-quarantined like Core.Process); fully transpilable (`php://stdin` faithful);
  under `phg serve` stdin is immediately-EOF (web input = the Request). META-7 scan: PHP/Go/
  Rust/Node all expose a module/stream API — none inject into main; the entry-signature-injection
  alternative was offered and recommended against (eager read, magic role). *Alternatives
  (offered): minimal readAll+readLine — declined for the full module; signature injection —
  declined.* Queue: build AFTER DEC-258 lands (developer-ruled slot).

- **DEC-258 BUILT (2026-07-17):** the combined naming model shipped. Language enabler: DEC-249/236
  defaults now accept ZERO-payload enum-variant constructions (`Mode m = new Mode.Fast()`) as
  compile-time constants (checker `variant_default_ty`; payload variants + generic enums stay
  rejected; 3-leg verified). Prelude: `Database.naming` promoted field (default
  `new Naming.Exact()`; `withPassword` gains the same param); `prepare`/`bind*` thread it onto
  `Statement.naming` (public); `namingStrategy` = real copy-builder (stored-statement footgun
  retired). Desugar (`desugar_db`): per-function `scan_naming_facts` proves immutable
  never-shadowed literal-ctor locals (brutal standard — anything less → runtime tier);
  `naming_of_recv` walks to the chain's `prepare` and consults facts / inline ctors; untraceable →
  dual baked helpers + a `Dyn` dispatcher matching on `stmt.naming` (Class/Stream/entity-Map
  shapes; scalar shapes ignore naming). `E-DB-NAMING-NOT-CONST` RETIRED (explain entry rewritten
  as a retirement notice). 10 naming tests incl. the four new tiers; example `db/naming.phg`
  extended with the baked-vs-dispatched twin demo.

- **DEC-282 — RULED (2026-07-17, developer, 3-round adjudication): THE UNIFIED MANIFEST-LESS
  LOADER ("autoload") — CLI + web.** Supersedes project-vs-loose duality; phorj.toml, manifest.rs,
  and the `phg vendor` network subcommand ALL RETIRE (dependency fetch/lock = a future
  DEC-273-style EXTENSION that writes `vendor/`; the compiler/interpreter NEVER touches the
  network — disk is truth, loud error otherwise).
  (a) **Root rule**: CLI `phg run file.phg` → root = the file's directory, zero ceremony (a future
  Symfony-console-like component routes subcommands inside ONE entry per DEC-191). Web
  `phg serve DIR/` → DIR is the EXPLICIT root; docroot = DIR/public (the only web surface);
  entry = DIR/public/index.phg (missing → clear startup error); `phg serve file.phg` survives as
  handler-only dev mode (no docroot/static). `-`/`-e` (no directory) → Core.* only.
  (b) **Loading**: IMPORT-DRIVEN lazy, DECLARATION-INDEXED (package-line peek of .phg under the
  root; load files declaring the imported package + transitive imports; un-imported files never
  read — the 162-same-dir-Mains constraint and broken-stranger isolation drove this). Same-package
  multi-file MERGE (Go), duplicate public symbol = hard error naming both files
  (E-DUP-CROSS-FILE). Whole-reachable-graph checking retained.
  (c) **Layout laws**: folder=package (E-PKG-PATH, relative to the root — src/Model/Article.phg ⇒
  `package Model;`) + file=type (E-FILE-NAME — Article.phg must contain type Article; other
  members may accompany). Function-only files: FILENAME free, folder law still binds ("even
  functions must have a package — not in the wind"). `package Main` = entry-only,
  location/name-exempt, UNIMPORTABLE.
  (d) **Wind-hole census (all verified live, all fixed Go-MAXIMAL)**: `import Main;` was silently
  accepted → E-IMPORT-MAIN; `import Core.Bogus;` (nonexistent Core module!) was silently
  accepted → folds into E-MODULE-NOT-FOUND (one error for every unresolvable import, listing the
  searched paths verbatim + the extension hint); duplicate import → E-DUP-IMPORT (hard); unused
  import → E-UNUSED-IMPORT (hard, Go-maximal — developer chose errors over warnings for both).
  (e) **Vendor**: `vendor/<publisher>/<name>/` under the root; identity = folder path;
  first-party wins over vendor with W-VENDOR-SHADOWED warning naming both paths when both exist;
  vendored packages resolve own-tree-first then shared vendor/ (diamonds share one copy; version
  conflicts = the extension's problem).
  (f) **Static serving (in-slice, dev server)**: exact-file match under public/ (non-.phg) with
  ~20-type MIME table (unknown → application/octet-stream) + ETag/Last-Modified conditional
  caching (developer added option 2); everything else → the index.phg entry. Guard list:
  canonicalize+prefix check (no ../ or symlink escape), *.phg source NEVER served, no dotfiles,
  no directory listing/auto-index. OUT (later): Range, compression, Cache-Control config, custom
  error pages, TLS.
  (g) **LSP**: same slice — diagnostics_for gains the file URI and runs the SAME loader (the
  text-only LSP is a live DEC-252 violation even for today's project mode, verified).
  (h) **Migration**: 11 examples/project/* tomls retire (withdeps keeps vendor/ by folder
  identity); tests/project.rs rewrites; loose-mode Main-only restriction lifts; transpile still
  emits ONE PHP file (PHP-side autoloading stays structurally unnecessary).
  **Order (developer: "Option 1 and 2 now")**: build DEC-281 Core.Input FIRST (small ruled
  slice), then DEC-282 as one slice, all of it.

- **DEC-281 BUILT (2026-07-17):** `Core.Input` shipped — `Core.Native.Input` natives (readAll
  lossy-UTF-8 / readAllBytes exact / readLine null-at-EOF with EXACTLY-one-terminator strip /
  isInteractive) + the `Core.Input` prelude twin (`Input` static surface + `InputLines`
  Iterator<string>, DEC-257 lookahead protocol). Injectable-stdin test seam (`set_stdin_override`,
  cursor-carrying) + serve-disable flag (`set_stdin_disabled`, wired into `phg serve` startup —
  reads = exhausted pipe). PHP legs real (CLI `STDIN`; readLine strips via PCRE `\r?\n$` — the
  naive `rtrim($l, "\r\n")` would eat every trailing CR, caught and fixed pre-commit; 3-leg
  verified on a CR/LF-tricky corpus). Quarantine map: `Core.Native.Input` → `Core.Input` twin row
  in `uses_impure_native`. 7 tests (`tests/stdin.rs`) incl. import-gating; example
  `cli/stdin-filter.phg` (3-leg identical).

- **DEC-282 ADDENDA (2026-07-17, developer — the multi-entry round):**
  (i) **APP-ROOT DISCOVERY**: `src/` IS the root marker — walk UP from the entry file to the
  nearest directory containing `src/` (or `vendor/`), git-style, nearest wins; that directory is
  the app root. No marker file, no config. No `src/` above → root = the entry's own dir (lone
  scripts unchanged). Supersedes the plain entry-dir rule; `phg serve DIR` stays explicit.
  Package names resolve UNDER `src/` (stripped — `src/Model/Article.phg` ⇒ `package Model;`).
  Entries live ANYWHERE under the app root (bin/, xyz/, public/, root).
  (ii) **THREE SEARCH ROOTS, first match wins (developer-confirmed order)**: (1) the entry's own
  folder (entry-local packages, e.g. `bin/Commands/`), (2) `<approot>/src/`, (3)
  `<approot>/vendor/`; same package in a later root too → loud W-SHADOWED naming both paths;
  `Core.*` reserved ahead of all three (step 0, never disk).
  (iii) **SHEBANG + IMPLICIT RUN (both verified broken today: `#!` = lex error; bare `phg <file>`
  = usage)**: the lexer/loader skips a byte-0 `#!...` line; `phg <existing-file>` with no
  subcommand DISPATCHES TO RUN (subcommand names keep priority), trailing args become the
  entry's `List<string>` argv; extensionless entries (e.g. `bin/console`) accepted when named
  explicitly — package scanning still reads only `*.phg`. Enables Symfony-style
  `chmod +x bin/console && ./bin/console migrate --dry`.

- **DEC-282 BUILT (2026-07-17):** the unified manifest-less loader shipped, one slice. Loader:
  `discover_roots` (src/-marker walk-up) + `peek_package` declaration index + `load_unified`
  (3-root import-driven lazy; W-SHADOWED) + `assemble` factored from the retired project mode;
  `load_with_buffer` LSP seam. Hygiene: E-MODULE-NOT-FOUND (searched roots listed), E-IMPORT-MAIN,
  E-DUP-IMPORT, E-UNUSED-IMPORT (whole-word source scan, import statements blanked by byte-range —
  a statement-position guard keeps the word "import" in comments from tripping it; interpolation
  holes are parser-side, which is WHY it's a source scan not a token scan). Shebang byte-0 skip +
  bare `phg <file>` → run (argv threads). Serve site mode: static_files.rs (MIME ~20, ETag +
  Last-Modified + 304, canonicalize/prefix guards, .phg-never-served, W-PHG-IN-DOCROOT) +
  docroot OnceLock + resolve_site_dir; verified live via curl incl. traversal attempts. LSP
  diagnostics_for_uri → same loader (DEC-252 restored for multi-file). RETIRED: manifest.rs,
  lock.rs, vendor.rs, `phg vendor` (stub error points at the extension path), tests/vendor.rs,
  loose Main-only rule (file loads; stdin/-e keep it); 11 example tomls dropped; withdeps vendor
  migrated to vendor/Acme/Strutil (folder=package). **DEVIATION disclosed**: vendor layout is
  PascalCase `vendor/<Publisher>/<Name>` (folder=package uniformity), not the lowercase
  publisher/name shown in the ruled preview. Eager-validation semantics change: files no import
  reaches are INERT (the old whole-tree Core-hijack/lowercase-package rejections became
  unreachable-by-construction — tests flipped to assert inertness).

- **DEC-282 addendum — PACKAGE-MANAGER EXTENSION: FULL RE-ADJUDICATION REQUIRED (2026-07-17,
  developer, standing):** when the dependency-manager extension work starts, EVERY detail is
  re-discussed from scratch — the developer explicitly dislikes the phorj.toml idea, so NO
  toml-style manifest is presumed for the extension either (config format, dep declaration
  surface, lockfile shape, registry model, CLI surface: all open). Research/brainstorm across
  ecosystems (composer/cargo/go modules/npm/uv…) then re-ask, every detail interactively ruled.
  Nothing about the retired manifest carries over by default; the only settled seam is the one
  DEC-282 shipped: the extension WRITES `vendor/<Publisher>/<Name>/` (folder = package) and the
  compiler only ever reads disk.

- **DEC-273 SLICE 1 BUILT (2026-07-17): the extension seam + pilot.** `src/ext/registry.rs` =
  the one-row list (name/feature/enabled/tier/modules/summary/migrated) — drives the
  disabled-import gate (preludes' GATED_CORE_MODULES const RETIRED, derived from the registry),
  the new `phg extensions` subcommand, and the generated `docs/EXTENSIONS.md` (sync test, the
  explain-coverage pattern; guarded to the default build). `E-MODULE-UNAVAILABLE` SUPERSEDED by
  `E-EXTENSION-DISABLED` (names extension + flag + points at `phg extensions`; old explain entry
  = retirement pointer). PILOT: `Core.Ini` → `src/ext/ini/{mod,natives,tests}.rs` behind a new
  default-tier `ini` feature — the AMENDMENT-2 folder shape proven end-to-end (live-verified:
  no-default build rejects `import Core.Ini;` with the clean diagnostic). Tier heads recorded:
  transpile/lift open MANDATORY (feature "-" until their structural wave). Remaining extensions
  keep their pre-DEC-273 homes, listed with `migrated: false` for discovery.

- **DEC-273 slice-1 PANEL round 1 (2026-07-17, DEC-268 3-lens, evidence-based):** lens-1
  correctness 2×P2+3×P3 · lens-2 security CLEAN (feature-gate bypass question CLOSED — every
  entry point traced to the two pipeline chokepoints; layer-2 structural impossibility of
  `__phorj_ini_parse` emission on gated builds; noted PRE-EXISTING `check --json` gate-quality
  gap + E-INJECTED-TYPE-BARE two-step trail, both inherited) · lens-3 completeness 1×P1+6×P2+2×P3.
  ALL findings fixed same-wave (extensions-arg rejection; matcher predicate extraction+tests;
  signals row + green/db-all absence documented; twin-colocation wording honest; ARCHITECTURE
  ext/ row; KNOWN_ISSUES retirement pointer; examples/README rows; register-note corrections:
  the docs sync test is BUILD-INDEPENDENT not default-guarded, and row scope = feature-gated
  capabilities only). One item escalated to the developer (ADJUDICATION rule): the `jit` row
  classifies JIT as a Default-tier extension while the ruling's CORE list bundles JIT into the
  language kernel — developer to rule row-stays vs row-drops.

- **DEC-273 WAVE 1 expanded (2026-07-17, developer directive "bigger slices/waves"):** the panel
  fixes and FOUR more physical migrations folded into the same wave — `crypto`, `regex` (its
  prelude source colocated via `ext::regex_prelude::PRELUDE`, referenced unconditionally by the
  CORE_MODULES const; the gate rejects the import on reduced builds before the prelude matters),
  `csv`, `encoding` (both gained new default-tier features). Live-verified: no-default build
  rejects `import Core.Csv;`/`Core.Regex;` with clean E-EXTENSION-DISABLED. Rows: +signals
  (Default), +csv, +encoding; migrated=true ×5; green/db-all documented non-rows.

- **DEC-273 addenda (2026-07-17, developer via AskUserQuestion):** (a) the `jit` registry row
  STAYS — jit remains CORE by classification (the ruling's kernel list); the row documents its
  BUILD FLAG for discoverability, not an extension status. (b) `phg build` artifacts CARRY AND
  USE the JIT (measured: hot pure 10M-iter loop — phg run JIT 0.08s / --no-jit 8.9s / the
  standalone artifact 0.14s), inheriting the building phg's feature set; NEW: artifacts honor
  `PHG_NO_JIT=1` (env — argv belongs to the embedded program) as the byte-identical pure-VM
  escape hatch, mirroring `phg run --no-jit`.

- **DEC-273 WAVE 1 CERTIFIED + panel record (2026-07-17):** DEC-268 MAXIMAL ladder satisfied —
  round 2 (3 lenses: security CLEAN incl. PHG_NO_JIT de-escalation verdict + env-read enumeration;
  correctness 1×P2; completeness 3×P2+1×P3 — all fixed), round 3 (1 residual: a fix reported
  landed but NOT in tree — unasserted replace; fixed with grep-verified anchor), rounds 4 AND 5
  fully CLEAN (consecutive). Round-5 fresh probes: all 5 migrated-extension examples 3-leg
  byte-identical under the php-8.5.8 oracle. Panel by-catch (pre-existing, KNOWN_ISSUES'd):
  `phg test` whole-file validation uses the raw checker (injected-type files fail `<check>`);
  `Process.args()` doc drift.

- **DEC-273 WAVE 2 BUILT (2026-07-17):** json/uri/path/hash/decimal/test/debug → `src/ext/<name>/`
  behind seven new dep-free Default features; uri carries kernel + natives + Core.Url compat twins
  + PRELUDE; debug carries its DebugModule PRELUDE (dissolution pattern = unconditional `#[path]`
  prelude modules in the ext folders; CORE_MODULES rows re-pointed). Registry 22 rows (2 mandatory + 16 default + 4 opt-in),
  alphabetical-asserted. PLAYGROUND FIX: wave 1 had silently dropped Ini/Csv/Encoding from the
  wasm build (default-features=false, nothing re-added) — playground/Cargo.toml now re-adds all
  dep-free Default extensions. Live probes: json/paths/decimals/hashing/uri guide examples +
  conformance dump 2-leg identical; ext suite 96/96; gate 2276/2276 + clippy×2 + no-default check
  + fmt. Decimal note: the MODULE is the extension; the `1.50d` primitive/arith stays kernel.

## DEC-283 — RULED (2026-07-17, developer, 5-round refinement): THE TEMPLATE EXTENSION (.phgml)

**Scope (developer's framing): "full support of phorj code inside HTML — {% %}, no more"; a
simple PHP-like interleave engine, NOT a Twig-class dialect; anything higher-level = future
extension packages. Build queued AFTER the DEC-273 migration waves.**

1. **Minimal core surface**: `{% <phorj statements> %}` (real language statements — control flow
   is phorj's own `if`/`for` with braces, HTML between markers becomes output inside the open
   block, ERB-style) · `{{ <phorj expr> }}` emitted AUTO-ESCAPED BY TYPE (string escapes, Html
   embeds — the html"…" rule; filters = the language's own `|>` pipe) · `{# comments #}` · ONE
   typed header per file: `{% template name(params) %}`. NO template dialect: no {% set %}, no
   {% include %} (call another template), no filter registry, no custom tags.
2. **Imports**: explicit `{% import …; %}` lines in the header area — full .phg import grammar,
   same three-root resolution, same HARD hygiene (E-MODULE-NOT-FOUND/E-DUP-IMPORT/
   E-UNUSED-IMPORT). ZERO auto-imports (wind rule); only compiler-synthesized emission machinery
   is zero-span-exempt (the #[Entry] precedent).
3. **File laws**: a .phgml IS a phorj file wearing HTML clothes — name=file (Card.phgml ⇒
   component `card`, E-FILE-NAME analog), folder=package (implied, never written), import-driven
   discovery (`import Views.X;` loads the package's .phg AND .phgml together), compiled to an
   ordinary `public function …(…): Html` BEFORE the checker (compile-time-sugar discipline —
   backends/PHP output never see template syntax; transpile byte-identity free), diagnostics
   carry the .phgml path + original line/col. FORBIDDEN: runtime template loading (never),
   .phgml entries (`phg run x.phgml` = clear error, templates are libraries), `package Main`
   templates, and the serve docroot guard EXTENDS to .phgml (never served).
4. **THE GENERALIZED VIEWS LAW** (the explicitness fix — "no magic, the import must show the
   origin"): a lowercase `views` folder (a ROLE folder like src/vendor/public) maps to the
   package segment `Views` at ANY depth in ANY root — top-level views/Pages/ ⇒ `Views.Pages`;
   src/views/Pages/ ⇒ `Views.Pages` (CONVERGENT — moving views between layouts never touches an
   import); domain views src/Blog/views/ ⇒ `Blog.Views`; deep src/Shop/Cart/views/Widgets/ ⇒
   `Shop.Cart.Views.Widgets`; vendor/Acme/Ui/views/ ⇒ `Acme.Ui.Views.…`. Top-level views/ = a
   FULL package root (any source kind — uniformity over enforcement) + a walk-up app-root
   marker. Search order: entry-dir → views/ → src/ → vendor/ (developer-ruled "views first";
   inert for non-Views packages). PascalCase `Views/` twin stays legal (plain folder=package,
   convergent names; W-SHADOWED on duplicates). views-inside-views REJECTED (E-PKG-PATH). Leaf
   collisions (Blog.Views + Shop.Views both binding `Views`) resolve via the existing `as` alias
   — E-IMPORT-CONFLICT already forces it, nothing silent.
5. **Controller flow**: templates are typed functions — `import Views.Pages;` then
   `Html page = Pages.home("Welcome", items); Response.html(Html.render(page))`. Data in as
   typed args, Html out; a wrong argument is a COMPILE error in the controller. No render()
   string dispatch, no context objects, no runtime engine.
6. **Composition** = plain calls (a layout is a template taking Html params). Components+slots
   recorded as the RECOMMENDED future direction (typed, explicit, what Blade/HEEx/Templ
   converged on); extends/blocks rejected for the core (stringly block contracts = the silent-
   downgrade class). Both remain buildable later as extension packages.

*Alternatives rejected across the rounds: Twig/Jinja dialect (second language, own truthiness);
extends+blocks in core; auto-imported "template stdlib" (wind); runtime template loading;
`<?phg ?>` spelling; `import X from views;` grammar (second import spelling); views-strip
(origin-hiding — the magic the developer refused); views restricted to fixed depths.*

- **DEC-273 WAVE 2 CERTIFIED (2026-07-17):** DEC-268 panel — round 1 (consolidated 3-lens):
  1×P2+3×P3 all doc-accuracy (22-not-19 rows; date slips; stale path comments; rustdoc link),
  code verified clean incl. prelude BYTE-IDENTITY of the moved DEBUG/URI consts and crypto's
  argon2 semantics; round 2: 2×P3 (one missed fix site — calls.rs; a misattached Http doc
  paragraph carried from HEAD) — fixed, Http paragraph restored above HTTP_PRELUDE; rounds 3+4
  consecutively CLEAN (round-4 fresh probes: 5 examples THREE-LEG identical vs php-8.5.8; hash
  RFC KATs in the new home; zero panic!/unwrap in diff additions; 1790/1790 lib).

- **DEC-273 WAVE 3 BUILT (2026-07-17):** db (natives + sqlite/mysql/postgres drivers colocated;
  the driver `mod`s use `#[path]` siblings), mail, http-client, session (NEW default `session`
  feature — SessionModule/Native.Session now gateable; playground parity added) → src/ext/;
  their four preludes dissolved out of cli/preludes.rs into colocated prelude.rs files.
  16/23 rows migrated. Session inline tests keep `use super::*` (the one inline-tests module in
  the wave). Live-verified: no-default build rejects `import Core.SessionModule;` cleanly;
  affected suites 207/207. html NOT migrated (ruled core seam — the html"" literal desugars to
  its natives); di deferred (checker-desugar-coupled).

- **DEC-273 WAVE 3 CERTIFIED (2026-07-17):** the woven four (db+drivers, mail, http-client,
  session) committed `cb189d3b`; the round-3 prose-path finding swept in `21f8bfb1` (~20 live
  src/native/ refs → src/ext/, stranded rusqlite comment removed, examples.js regenerated).
  DEC-268: r1 2×P2 (session "always compiled" comment + release freshness) · r2 clean · r3
  1×P2 (stale prose) + 1×P3 (stranded comment) · fresh A+B consecutively CLEAN. 16/23 registry
  rows migrated. Panel process lessons banked: git-mv stages renames immediately (scoped commits
  sweep them — split with reset --soft); piping git-diff through the RTK proxy can false-clean
  (grep an on-disk file). Remaining extension migrations (di — checker-desugar-coupled; log/time/
  runtime classification) = wave 4; html stays a core seam.

## 2026-07-20 lift/transpile/LSP-alignment pass — audit rulings (developer via AskUserQuestion)

- **DEC-312 — LIFT INVERSE REGISTRY = a `lift_from` facet on `NativeFn` (developer-ruled 2026-07-20).**
  Problem surfaced by the alignment audit: the lifter (`src/lift/`) has **NO inverse native table** — a PHP
  `strlen($s)` lifts to an unresolved `strlen` call, never `Core.String.length`. Transpile is registry-driven
  (`NativeFn.php: fn(&[String])->String`, `src/native/mod.rs:66`) but lift is a wholly separate hand-written
  frontend, so Invariant-17 "same change" is a review convention, not a structural guarantee — the prime
  drift source. Of the 631 PHP FN builtins, **~124 already have a forward Core equivalent** baked into a
  transpile emitter (directly invertible); ~507 have no Core equivalent; 99 emitters use `__phorj_*` shims.
  RULING (recommended option): add an optional field `lift_from: &'static [&'static str]` to `NativeFn`
  naming the PHP builtin(s) this native inverts, so transpile AND lift are **co-registered on ONE row** — the
  lifter derives its PHP-builtin→Core table from `native::registry()`. One bidirectional single source of
  truth; kills lift/transpile drift structurally. Alternatives rejected: standalone hand-authored LiftMap
  (a second place to forget); auto-derive by inverting the `php:` closures (fragile — `fn`s not cleanly
  invertible, 99 `__phorj_*` shims). Build: seed `lift_from` from the 124; lifter resolves builtins → Core
  calls; the `__phorj_*`-shim natives need a later idiom recognizer (tracked, not in v1).
  **✅ SHIPPED 2026-07-22 (v1 tranche).** `NativeFn.lift_from: &'static [&'static str]` added across all
  356 registry rows; seeding was MACHINE-DERIVED with a strict matcher (emitter = exactly one builtin,
  args in order) then HAND-AUDITED — final tranche **53 unique builtins** (the 124 estimate included
  shim/multi-call emitters the strict rule correctly refuses). Deliberate non-registrations recorded
  in-code: `trim` (the `__phorj_trim` Unicode-whitespace shim — inverting changes semantics), `crc32`
  (PHP builtin returns int, ours is the crc32b hex form), `log2` (emits `log(x,2)`). Collisions resolved
  dominant-idiom (strlen→String.length not Bytes; count/array_values/array_merge→List not Map/Set — a
  wrong pick surfaces as a LOUD type error in the draft, never silent divergence); uniqueness enforced
  by `lift_from_builtins_are_unique`. Lifter: `native::lift_of()` derived table; bare `PhpExpr::Name`
  calls resolve arity-checked to qualified Core calls + auto-`import` (thread-local recorder, reset per
  lift). Verified e2e: `strtoupper/strlen/sqrt` → `String.upperCase/String.length/Math.sqrt` + imports.
  DISCLOSED Invariant-13 exception (review welcome): the mandated one-line-per-row field grew 4
  grandfathered registry files (native/mod, text_registry, list_registry, math) by exactly the
  mechanical delta — their `size-baseline.txt` rows were RE-FROZEN at the new counts rather than
  make-work-splitting four cohesive registry tables; no logic growth. PROCESS LESSON (recorded per the
  audit directive): the DEC-313 push went out on a TIMED-OUT full suite — the tier-1 allowlist gap
  (`fwrite/scandir/…` from the new FS/Log helpers) reached CI. Fixed same-day (TIER1_PHP extended —
  all ext/standard, hermetic under `php -n`); rule reaffirmed: a truncated gate is a red gate.

- **DEC-313 — LADDER "yet" quarantines: BUILD FS transpile, SESSION becomes PERMANENT (developer-ruled
  2026-07-20; ✅ SHIPPED 2026-07-22 — both halves).**
  `E-TRANSPILE-FS` and `E-TRANSPILE-SESSION` both say "yet" (buildable). Audit verdict: **FS is buildable** —
  every `Core.Native.FileSystem` native (18, `src/native/fs.rs`) maps to a faithful PHP builtin
  (file_get_contents/file_put_contents/mkdir/scandir/unlink/copy/rename/filesize/is_dir…), listings are
  pre-sorted both legs (byte-identical by construction), and the 7-way error KIND classification is
  reconstructable in PHP. The ONE obstacle is the raw OS-errno text embedded in `e.message` (Rust
  `std::io::Error` Display). RULING: **build the FS emitter** (`__phorj_fs_*` helpers + kind reconstruction),
  **declaring exception-MESSAGE text OUT-OF-CONTRACT** — the error KIND is the byte-identity contract, and the
  differential oracle already asserts only the kind (`tests/fs.rs`), never the raw message. **SESSION =
  reclassify PERMANENT** (like DB/Mail): its nondeterministic entropy session-ids (user-observable via
  `Session.id()`), wall-clock TTL (`Instant::now()`, not the freezable `Core.Time` clock), and persistent
  in-process store vs PHP's per-request `$_SESSION` model make it not byte-identically transpilable — its
  "yet" was optimistic. Update `explain.rs` accordingly.
  **BUILD-MAP (2026-07-20 spec, grep-verified):** (1) `FileSystemResult` = a prelude enum (`preludes.rs:405`)
  `Ok(T value)`/`Err(string message)` → lowers via `emit_enum` to `abstract class FileSystemResult` + `final class
  Ok{public $value}` + `final class Err{public string $message}`; the emitter must yield `new Ok(v)`/`new
  Err('<<Kind>>…')`. 7 markers (`preludes.rs:410-416`): NotFound/PermissionDenied/AlreadyExists/NotADirectory/
  IsADirectory/DirNotEmpty/FileSystemIoError → typed subtypes via `FileSystemError.fail`. (2) Pattern to mirror =
  `Core.Result` (`new Success($v)`/`new Failure($e)`, combinator helpers `runtime_tables.rs:227-273`) — but FS uses
  the enum variants `Ok`/`Err` with fields `value`/`message`. (3) 18 natives (`fs.rs:287-373`, all pure:false,
  placeholder `php:` at `fs.rs:301`) → PHP builtins per the spec table; `exists`/`isFile`/`isDir` infallible→always
  Ok; `classify()` map at `fs.rs:43-55`. (4) Gated-helper 3-touch (exemplar `uses_clock`): flag `mod.rs:434`+ctor
  `:595`, set-site `call.rs:306` (`nat.module=="Core.Native.FileSystem"`), bodies `runtime_php.rs` (needs the
  runtime_php M-Decomp first — file at cap). (5) Quarantine: DROP FS rows `pipeline.rs:582-586,612-616`; KEEP+permanent
  SESSION `:587-591,617-621`; `explain.rs:1494-1501` FS (retire, keep a catch-all), `:1478-1484` SESSION (reword). (6)
  Tests: `tests/fs.rs` only interp+VM (no transpile golden); **invert `fs_transpile_is_a_clean_ladder_error`
  (`fs.rs:106-123`)**; `examples/fs/walk.phg` stays differential-quarantined via `uses_impure_native` (correct —
  ambient/impure). **⚠ TWO HIGH RISKS:** R1 — a global `__phorj_fs_*` helper's `new Ok/Err` can bind the wrong class
  in a namespaced/multi-package program (`variant_ref` ns-prefix, `expr.rs:654`); prefer inlining `new Ok(...)` at
  the call site via `nat.php`, or fully-qualify. R2 — reconstruct `<<Kind>>` in PHP via explicit pre-checks (no
  `ErrorKind` from php builtins); the 3 pinned kinds (NotFound/DirNotEmpty/PermissionDenied, `tests/fs.rs:63-104`)
  are exact. **BUILD (2026-07-22):** helpers in `src/transpile/fs_php.rs` (`uses_fs` 3-touch); every
  `php:` emitter wraps its helper AT THE CALL SITE — `(($__fsr = __phorj_fs_x(..))[0] ? new Ok($__fsr[1])
  : new Err($__fsr[1]))` — so Ok/Err bind in the caller's namespace (R1 resolved as speced); kinds
  reconstructed via explicit pre-checks (R2), pinned trio exact, `FileSystemIoError` the wildcard twin of
  Rust `classify()`; listings `sort(.., SORT_STRING)` ≡ Rust byte-sort. Quarantine rows dropped from
  `reject_native_only_transpile`; `E-TRANSPILE-FS` explain entry marked RETIRED; ladder test inverted into
  `fs_transpiles_and_matches_the_backends_on_php` (transpile succeeds + php-leg stdout parity incl. the
  typed-catch trio). SESSION rows + explain reworded PERMANENT (same commit series).
  + the `removeDirAll` `/`-refusal guard + `readText` UTF-8 check must line up. Both verifiable vs php-8.4.19 (FS
  behavior stable 8.4→8.5) but delicate → best built where the full oracle runs. Full spec in the 2026-07-20 session log.

- **DEC-314 — PERF #2b (general VM→native dispatch-overhead reduction) = a FRESH-CONTEXT build slice (developer-ruled 2026-07-20).**
  #2b (reduce the ~188ns/call `Op::CallNative` dispatch at `src/vm/exec.rs:434`, lifting all ~465 natives at
  once) is the deepest VM/JIT spine change and the only lever that can move the linear/alloc-bound losses;
  more per-op verticals are exhausted (structural finding, DEC-311). RULING: build it as the **first build
  slice in a FRESH session** (honors the standing JIT-fresh-context rule — depth-induced slips are the
  ctype-class risk), with the ~188ns baseline pre-measured. Note the environment reality: canonical vs-php
  perf can't run in the remote container (org egress blocks php-8.5 via apt Launchpad AND docker CDN); the
  vs-8.5 verdict + ratchet-ARMING (`microbench-gate.sh --emit`) happen on an 8.5-capable box.

- **AUDIT CORRECTION (2026-07-20): the "286 natives" figure repeated across KNOWN_ISSUES/M-gap-matrix is STALE.**
  Cross-checked two ways, the real registry is **492 all-features / 465 default** (Core 333 + ext 159; pure
  374 / impure 118; 34 HigherOrder). "286" is an old raw-`grep NativeFn {` undercount (misses macro/helper-
  generated rows: Html tag macros, Math `unary_float`, Uri getters, per-`entry()` builders). Consequence:
  perf bench coverage is 40/465 (~8.6%), thinner than the "40/286" claimed. Fix the figure at each doc touch.

- **DEC-315 — THIRD-PARTY EXTENSION MODEL = userland `.phg` packages + a stability-committed native Rust
  trait-seam SPI (Option B; developer-ruled 2026-07-20, ASKED via AskUserQuestion).** Two authoring paths are
  open: **(1) userland `.phg` packages** — pure phorj source under `vendor/<Publisher>/<Name>/` (namespaced
  `Publisher.Name.*`; `Core.*` reserved for first-party), consumed by the DEC-282 offline loader. These get
  transpile/lift/LSP/byte-identity **for free** (they ARE phorj source; zero LADDER interaction, no `phg`
  rebuild) — the primary path. **(2) native Rust extensions** — a third party implements a documented,
  semver-stable public **trait-seam SPI** (`DriverConn`/`Transport`/… — the DEC-273 seams) + a registry row,
  and rebuilds `phg` with `--features their-ext` (source-level, same `rustc`; NO dynamic ABI). Each native
  extension MUST carry a faithful PHP twin (emitted at the transpiler runtime-tables chokepoint) OR declare
  itself native-only with an `E-TRANSPILE-<EXT>` hard error + differential quarantine + disclosure
  (Invariant 14 LADDER, in full). **REJECTED, permanently: dynamic `.so` plugins** (PHP-C-ext / Go-`plugin`
  style) — Rust has no stable ABI by design, they violate `#![deny(unsafe_code)]`, have no PHP twin, and
  can't be sandboxed. Cross-language survey (Rust/PHP/Go/Python/JVM/Swift/C#/Racket/Zig, Invariant 16)
  confirmed every mature ecosystem's real extension path is source/package-level, not dynamic-native.
  Consistent with DEC-216/218/282 (userland-first) + DEC-273 (first-party Rust seams). Deliverable: document
  the SPI + authoring guide + the core-vs-first-party-vs-userland 3-bucket boundary; record here + MASTER-PLAN.

- **DEC-316 — COMPANION PACKAGE MANAGER = the NEXT MAJOR SLICE (developer-ruled 2026-07-20, after the E2
  extension-file splits).** The tool that fetches + writes userland `.phg` packages into `vendor/` (DEC-282:
  `phg` itself stays offline / package-agnostic — it only reads `vendor/`). Large interactive design round
  (Invariant 15): manifest format (dev dislikes `phorj.toml` → prefer a `.phg`-source manifest, to be
  surfaced), lockfile shape, registry model, semver, checksum/tree-hash integrity (retired DEC-033
  SHA-pin precedent). Without it the userland third-party path (DEC-315 path 1) has no distribution → this is
  what makes the ecosystem real. Design forks surfaced before building.
  **✅ SHIPPED 2026-07-20** (dev ruled the two forks via AskUserQuestion): manifest = **JSON,
  composer.json-style** `phorj.json` (dev picked JSON over toml); distribution = **all three source
  kinds** (registry/git/path). Built as `phg add/install/update/remove` subcommands (NOT a separate
  binary — softens DEC-216's "companion tool", approved at plan-exit) in a new std-only `src/pm/`
  (hand-rolled JSON + semver — external-dep policy forbids serde_json). Key design: the central
  **registry is a name→git-URL index** so every fetch is a `git` checkout or fs copy (no tarball/gz —
  stays std-only); `phorj.lock` pins a tree SHA-256 (reusing `bundle::sha256`), re-verified offline on
  install (tampered/stale `vendor/` → hard refusal). Only these verbs touch the network (Invariant 10
  preserved). Example `examples/package-manager/` passes the byte-identity project gate (a userland
  `.phg` dep transpiles for free — validates DEC-315). Commits `e896eba`/`775db80`/`6284506`. Follow-ups
  (documented): registry constraint-intersection across multiple requirers, per-package `phg update`,
  a hosted registry index (client support shipped; `PHORJ_REGISTRY` selects the index).

- **DEC-317 — STRUCTURED LOGGING (Log-v2) = FULL Monolog-class upgrade over the stderr `Core.Log`
  (developer-ruled 2026-07-21; SPEC READY, BUILD QUEUED).** The existing `Core.Log` (DEC-220 S1,
  leveled→stderr) is upgraded to MASTER-PLAN #18 "structured logging". Dev ruled the FULL scope via
  AskUserQuestion: named **channels** (`Log.channel("name")`, a `default` channel preserves the current
  top-level `Log.info(...)`), PSR-3 **levels** (Debug…Emergency, ordinal for min-level compare) with a
  per-handler min-level, **handlers** implementing the `LogSink` SPI seam (DEC-315) — `StreamHandler` /
  `FileHandler` / `RotatingFileHandler`, **formatters** `LineFormatter` + `JsonFormatter` (reuses the
  `__phorj_json_*` transpile helpers), and **processors** (context injectors: timestamp/pid/static extra).
  Wiring is config-driven via DEC-318's `#[Config]` provider (`LogConfig`/`Channel`/`FileHandler` are
  ordinary phorj classes). **LADDER ruling (Invariant 14): TRANSPILABLE** — every sink maps to a faithful
  PHP builtin (`fopen`/`fwrite`/`file_put_contents(…,FILE_APPEND)`, `fwrite(STDERR,…)`, rename+size/date,
  `json_encode`) — but **impure → FileSystem-style differential quarantine**: dropped from the auto
  `examples/**/*.phg` byte-identity glob, given its own deterministic transpile-parity test printing
  content+level+channel ONLY (timestamps/pid/paths are **out-of-contract**, like FS message tails);
  rotation tested structurally, not by wall-clock. Emit gated `__phorj_log_*` helpers (the
  `uses_clock`/`emit_fs_helpers` 3-touch pattern; results constructed INLINE per the FS R1 rule).
  **✅ SHIPPED 2026-07-22 (core)** — built per the SLICE-STATE architecture pin (config-data-in-Rust,
  objects-in-prelude): `src/native/log/{mod,state,prelude}.rs` — the `Core.Log` prelude declares
  `Level` (injected enum, `new Level.Warn()`), `LogFormatter`+`LineFormatter`/`JsonFormatter`,
  `LogSink`+the 3 handlers (promoted fields), `ChannelConfig`/`LogConfig`, and **`Logger`** (the
  channel handle — NOT named `Channel`, which is the concurrency built-in `Channel<T>`); `Log.configure`
  extracts plain data into a `Mutex` global (Session-store precedent; Rust reads the `Level` variant +
  built-in formatter class directly); `Log.channel(name)` builds the `Logger` carrier (Regex pattern);
  `Core.Native.Log.emit` is the kernel (filter→format→write, size rotation, DEC-220 stderr fallback
  when unconfigured/unknown — never crashes, never silent). Formats deterministic v1 (no timestamps):
  line `[TAG] msg` / `[TAG] chan: msg`; json fixed-key minimal-escaper (NEVER `json_encode`). PHP leg
  = gated `__phorj_log_*` helpers (`uses_log`, `transpile/log_php.rs`; `__phorj_log_ord` is
  variant-class-name mangling-aware, `Error`→`Error_`); content parity on ALL THREE legs gated by
  `tests/log.rs` (`log_v2_channels_write_identical_content_on_every_leg` — stdout + 4 log files
  byte-compared, incl. rotation; process-global registry ⇒ the in-file `LOG_GATE` mutex serializes
  the log tests). DEVIATIONS from the spec (recorded): **processors deferred** (timestamp/pid inject
  — would break the deterministic-content contract; needs the out-of-contract tail design, own
  slice); **userland `LogSink`/`LogFormatter` = recorded v2** (configure refuses them loudly — natives
  can't call back into phorj yet); **not a wave-4 ext folder** (Core.Log is always-compiled in
  `src/native/log/`; the ext migration remains optional wave-4 work).

- **DEC-318 — TYPED CONFIG = `#[Config]` provider fn + entry-param injection (developer-ruled 2026-07-21;
  SPEC READY, BUILD QUEUED).** How a `.phg` file yields a typed app config, ruled via AskUserQuestion. A
  zero-arg `#[Config]`-attributed `function` returns a user config type; the runtime **injects** it into
  `#[Entry] function main(config: AppConfig)`. **NO new grammar** — reuses the attribute + `function` +
  params machinery of `#[Entry]`. Bare top-level `return` (PHP `return [...]` idiom) was **REJECTED**:
  biggest grammar/semantics change (value-producing modules), runtime not compile-time, unneeded. YAML
  **REJECTED** (external-dep policy bars a YAML crate; config-as-typed-phorj matches the roadmap's
  "Core.Config compile-time typed"). Modeled as **compile-time-only sugar expanded OUT before any backend**
  (Invariant 5) in the `cli::check_and_expand` chokepoint (new `src/expand/config_inject.rs`): the desugar
  rewrites `main(cfg: T)` → an entry that calls the resolved provider and passes its value, emitting
  ordinary AST → **transpiles byte-identically** (a plain PHP call) and **lifts** as ordinary functions, so
  config stays IN the byte-identity spine (it is pure). Checker rules (`E-CONFIG-*`): exactly one provider
  per config-type; `main`'s param type must match a provider return type; missing/ambiguous = typed error.
  Provider discovery uses the existing project/registry scan (DEC-252/DEC-282), sorted (Invariant 10).
  **✅ SHIPPED 2026-07-22** — built as the pre-check pass `src/checker/desugar_config.rs` (the
  `desugar_di`/`desugar_db` pattern): `#[Entry] main(config: T)` with `entry_role == None` desugars to a
  zero-arg entry whose body opens with `T config = <provider>();` — valid entry shapes (`()`, argv, web)
  pass through untouched, so no `entry_role` change was needed. Marker gated by `import
  Core.Runtime.Config;` (`bare_types`, the Entry precedent); known-attribute arm in
  `checker/program/attributes.rs`; wired in BOTH `check_and_expand_reified` AND `front_end_diagnostics`
  (DEC-252 drift test). Typed errors `E-CONFIG-SIG/DUP/MISSING/TARGET` (+ `E-ATTRIBUTE-ARGS` on a
  non-bare marker), all in `phg explain`. Verified byte-identical interpreter ≡ VM ≡ no-JIT ≡ php on
  `examples/guide/config.phg` (pure → INSIDE the differential spine); 7 unit tests in the pass.

- **DEC-319 — EXTERNAL ADOPTION REVIEW SYNTHESIZED (2026-07-22): roadmap validated ~10/14; DX
  NORTH-STAR recorded.** A cross-language "what makes a language robust & mass-adopted" review (dev's
  external Claude conversation, META-7 discipline) was gap-checked against the roadmap. ALREADY COVERED
  (validation, no action): SemVer+stability tiers+deprecation lint; debugger REPL (eval-REPL W5-15);
  `phg doc` (W5-15); diagnostics-as-product (shipped + W2 ratchet); differential testing (normative-spec
  conformance W6-5); public flagship (W6-1); sound-mandatory static typing (gradual REJECTED, stands);
  BDFL governance (RFC process = future); naming casing test-gated (arg-order gate W2-13). EXPLICITLY
  KEPT against the review's suggestion: **self-hosting stays a NON-goal** (DEC-273). Genuine deltas ruled
  as DEC-320/321/322/323 below. **DX NORTH-STAR (dev, 2026-07-22, verbatim intent):** "everything smooth,
  intuitive and easy to use WITHOUT losing the advantage over PHP (typing and strictness) — and more
  object-oriented." Operational reading: the checker stays strict; the TOOLING is forgiving (parse-tolerant
  LSP, great diagnostics, zero-config defaults); OOP ergonomics keep growing. Governs DX prioritization.

- **DEC-320 — MIXED PHORJ/PHP PROJECT ADOPTION = 'transpile-into-project' build mode (developer-ruled
  2026-07-22; QUEUED, spec-first).** The TS→JS playbook: `.phg` files emit `.php` siblings INSIDE an
  existing PHP application (composer/PSR-4-compatible placement), so a team migrates file-by-file while
  the app keeps running on PHP. COMPILE-TIME ONLY — the earlier live-interop rejection (live PHP→Phorj
  on-the-spot rebuild, per-file gradual typing) STANDS untouched. This closes the roadmap's biggest
  adoption gap (the review's #1 lever: "a gradual migration path from existing PHP is bigger than any
  individual language nicety"). **SPEC READY 2026-07-22**: `docs/specs/2026-07-22-transpile-into-project.md`
  — five forks each with a recommended default (F1 sibling emit = zero composer edits, the tsc killer
  property; F2 one `phpInterop{namespaceRoot,sourceRoot}` knob; F3 explicit `phg build --php`, watcher
  v2; F4 the SHIPPED M8.5 `declare` interop now + a `phg stubs` generator v2; F5 one shared
  `_phorj/runtime.php` + composer `files` entry — the per-file helper embedding would fatally
  re-declare). Five adjudications queued in the spec's final section; BUILD BLOCKED on them
  (Invariant 15). Roadmap home: adoption/GA wave.

- **DEC-321 — EDITION FIELD BAKED NOW; editions machinery stays post-1.0 (developer-ruled 2026-07-22;
  ✅ SHIPPED same day).** `phorj.json` gains an `edition` key (single live edition `2026`) and the
  compiler/loader accepts + records it — no behavior forks yet. Rationale (review finding, accepted):
  retrofitting the identity metadata into every manifest/tool AFTER an ecosystem exists is the expensive
  part of Rust-style editions; carrying one inert field from the first release is nearly free. The full
  editions machinery (per-edition parse/behavior forks, migration lints) remains the §11.3 post-1.0
  residual, unchanged. BUILD: `Manifest.edition` in `src/pm/manifest.rs` (`KNOWN_EDITIONS = ["2026"]`;
  unknown edition = clean error naming the known list; absent = current edition, so pre-edition
  manifests stay valid; serialized after `version`); `phg add` stamps `"2026"` into a FRESH manifest
  (never rewrites existing ones); demo manifest + package-manager README document it.

- **DEC-322 — CONCURRENCY V2 = REAL PARALLELISM mandate (developer-ruled 2026-07-22; DESIGN SLICE
  QUEUED).** Dev ruling (verbatim intent): "we don't have real concurrency now — we need to implement
  real parallel concurrency." Today's `spawn`/channels are corosensei green threads: cooperative,
  single-core. Scope of the v2 design: TRUE multi-core execution + structured scopes (a task cannot
  outlive its scope) + bounded/closeable channels + cancellation. Design forks to ADJUDICATE in the
  design round (Invariant 15 — NOT ruled here): threading model (share-nothing actors/message-passing
  vs scoped shared-state), `Value` thread-safety strategy (the interpreter/VM value kernel is not
  `Send`/`Sync` today), JIT/VM interaction, scheduler shape. LADDER: concurrency is already permanently
  PHP-excluded (`E-CONCURRENCY-NO-PHP`) — parallelism does not change the spine contract. DESIGN-FIRST:
  no build until the forks are ruled.

- **DEC-323 — RELEASE CHANNELS: nightly/stable recorded; LTS deferred post-1.0 (developer-ruled
  2026-07-22; ✅ SHIPPED same day).** Channels: **nightly** = rolling prerelease re-pointed at every
  master push with the 4 platform archives attached; **stable** = `v*` tagged releases (the SemVer
  contract's channel). LTS = post-1.0 decision, recorded not scheduled. BUILD: the dev's push trigger
  built archives but never published (attach step gated on the `release` event; no nightly tag existed)
  — fixed by the `publish-nightly` job in `.github/workflows/release.yml` (downloads the matrix
  artifacts, force-moves the `nightly` tag, delete-then-recreates the prerelease via `gh`;
  `--latest=false` keeps the Latest badge on stable). Verified LIVE: release `nightly (10262b6)` with
  4 sha256-digested assets. Docs: SEMVER.md §Release channels + SECURITY.md supported-versions row.

- **DEC-324 — PHP-GAP ROUND-2 SWEEP folded into MASTER-PLAN (autonomous, 2026-07-22; per-item
  adjudication reserved).** Dev directive: "do more rounds to cover what PHP does that we still did not
  map". A full re-sweep (report: `docs/research/php-gap-round2.md`) grep-verified 25 items absent from
  EVERY coverage surface (MASTER-PLAN, M-gap-matrix incl. FN-group notes, D-php-surface's 869 rows,
  FEATURES, KNOWN_ISSUES, SLICE-STATE, D0 re-sweep): 8 TOP (serve TLS posture = GA-blocking PENDING
  adjudication; trusted proxies; response streaming; Range+gzip; HttpClient proxy/CA/mTLS + streaming;
  class-const expressiveness; enum interfaces/consts), 8 MID (pack/unpack BinaryLayout, trait-consts
  credit VERIFY, SessionStore seam, cpuTime, phg env, run-script, graceful reload), 9 REJECT-candidates
  recorded as PENDING-REJECT Appendix-A rows (SOAP/IMAP/SNMP/dba+SysV/pspell/enchant/calendar/tidy/LDAP→
  post-1.0). STRUCTURAL: D-php-surface never inventoried 12 extension domains — a silent denominator
  hole in the 824-row parity model; repairing it is queued with the Appendix rows. All slotted in
  MASTER-PLAN §PHP-GAP ROUND-2 ADDITIONS; waves are RECOMMENDATIONS — each item is adjudicated at build
  time per Invariant 15 (nothing here self-rules a user-visible design).

- **DEC-325 — CRAFTSMANSHIP AUDIT ROUND over the 2026-07-22 session (dev directive: audit every
  slice; fixes applied same-day).** Evidence-based panel found 2 P0 + 3 P1 + 7 P2. FIXED: match
  scrutinee now bound ONCE in transpiled PHP (side-effecting scrutinees ran 2-3× — spine divergence;
  `$__mN` temp, transpile/matches.rs); stdout StreamHandler routed through the program output buffer
  (was misordered vs `Output.*` on run/runvm); `#[Config]` providers keyed by type LEAF (qualified
  spelling `Cfg.AppConfig` resolves; ambiguity stays loud) + `E-CONFIG-TARGET` extended to trait
  methods; PHP log helpers: loud unknown-sink arm + level bounds guard (were silent divergences);
  channel registry reset at cmd_run/cmd_treewalk (state leaked across in-process runs);
  logging-v2 example now smoke-tested (was executed by NO test — Invariant-9 hole for quarantined
  examples); prelude doc contradiction fixed. RECORDED (queued, KNOWN_ISSUES §Transpile P1s): flat
  variant-class collision (`Ok`/`Err`); injected preludes Main-namespace-only on the PHP leg.
  DEFERRED (accepted): stringly HandlerCfg.format/Stream → enums (mechanical, next log touch).

- **DEC-326 — UFCS CANONICAL STYLE = RECEIVER FORM (developer-ruled 2026-07-22: both forms legal,
  one canonical style everywhere).** `String.length(s)` AND `s.length()` are both legal (UFCS,
  shipped — `rewrite_ufcs`). THE RULE: the RECEIVER form `s.length()` is canonical wherever the
  first parameter is the natural subject; the MODULE form stays canonical for receiver-less calls
  (constructors/config/ambient: `Log.configure(...)`, `Math.max(a, b)`-style multi-subject).
  Rationale (recommendation adopted): matches the DEC-319 "more OOP" north-star; `s.`-completion
  discovery beats module-name recall; Kotlin/Rust converged on the same idiom; lifted PHP visibly
  modernizes (`strlen($s)` → `s.length()`). BUILD QUEUED (next slice): lifter emits receiver form
  for subject-first natives (DEC-312 emission update + tests); examples/docs migrate as touched;
  a formatter canonicalization lint is the recorded v2.
  **✅ SHIPPED 2026-07-22:** lifter emits receiver form for subject-first natives
  (`strlen(strtoupper($s))` → `s.upperCase().length()`, zero-arg natives keep module form; UFCS
  erases both to the one module call pre-backend, so the spine is untouched); FIXED the blocker the
  build surfaced: `E-UNUSED-IMPORT` false-fired on modules used ONLY via receiver form (the loader's
  textual scan now also counts `.nativeName(` occurrences of the module's natives — generous by
  design, a false positive only silences a hygiene lint). FEATURES UFCS row carries the style rule.

- **DEC-327 — LSP PROJECT-WIDE FIND-USAGES (autonomous build 2026-07-22; completes the field-report
  LSP queue).** `textDocument/references` was single-buffer ("cross-file = follow-up"). Now: a
  TOP-LEVEL symbol's references scan the WHOLE project on demand — the loader's discovery roots
  (entry-local/src/vendor/views via the new `loader::project_phg_files`, query-layer only) plus every
  other open buffer (buffer content wins over disk). Per-file precision: occurrences shadowed by a
  local are excluded; a mid-edit unparsable file is skipped (never breaks the query); sorted paths
  (Invariant 10). RECORDED precision limit: cross-file matching is name-based — a same-named
  top-level symbol in an unrelated package also lists (navigation aid; the checker owns resolution
  truth; full semantic cross-package resolution = the cached-index follow-up). Locals stay
  single-buffer, as do rename/documentHighlight (multi-file rename = WorkspaceEdit slice, queued).
  M-Decomp: the leg lives in `src/lsp/references.rs` with its own tests (3: cross-open-buffer,
  unopened-disk-file, locals-stay-local).

- **DEC-328 — the two DEC-325 transpile P1s resolved (autonomous, 2026-07-22).** (1) VARIANT
  COLLISION: two enums sharing a variant name now REFUSE transpile loudly
  (`E-TRANSPILE-VARIANT-COLLISION`, `transpile/collisions.rs`, explain entry) instead of emitting a
  `Cannot redeclare class` fatal — the program still runs on the Rust legs; enum-SCOPED variant
  classes (lifting the restriction) stay the queued real fix. (2) MAIN-NAMESPACE PRELUDES **FIXED**:
  every non-Main namespace block opens with `use \Main\<name>;` aliases for the Main-bucket
  top-level names (classes/interfaces/traits/enums + mangled variant classes; `use function` for
  fns; inert when unused; skipped when the block declares the name) — `FileSystem.exists` from
  `package Acme.Fs` now runs on the PHP leg, verified on the recorded FS reproduction (`phg run` ≡
  php output). FOLLOW-UP recorded: a committed multi-package FS project fixture in the test sweep
  (the fix is repro-verified; the fixture makes it regression-gated).

- **DEC-330 — TERMINOLOGY RULED (developer, 2026-07-22): there is NO `runvm` — only `phg run` and
  the transpiled PHP.** The VM is `run`'s default engine and the tree-walker its `--tree-walker`
  oracle; `runvm` was a retired historical subcommand whose name had survived as shorthand across
  the repo. Swept (this ruling's build): every USER-FACING string (`phg explain` bodies, the
  native-only transpile refusals, the MI `parent()` transpile error), every LIVING doc
  (CLAUDE.md Invariant 1, INVARIANTS/ARCHITECTURE/CONTRIBUTING/STABILITY/SEMVER/GA-CHECKLIST/
  UNIFIED-SPEC/ADRs, examples+editors+selftest READMEs, .github templates, MASTER-PLAN/
  SLICE-STATE, KNOWN_ISSUES), all `examples/**`+`conformance/**` sources (playground examples.js
  regenerated), all `src/` comments (shorthand now "interp ≡ VM"), test fn/var names, and the
  playground's internal wasm surface (`pg_runvm`→`pg_vm`, worker key `runvm`→`vm`, lockstep with
  worker.js/main.js). LEFT AS RECORDS (deliberate): CHANGELOG/HISTORY/MILESTONES entries and
  docs/research + docs/specs/archive — they narrate the era when the subcommand existed.

- **DEC-329 — FOUR ADJUDICATIONS RULED (developer via AskUserQuestion, 2026-07-22; all recommended
  options adopted).** (1) **DEC-320 v1 BUILD APPROVED** with the spec defaults: F1 sibling emit
  (`src/X.phg` → `src/X.php`, PSR-4 unchanged, zero composer edits), F5 one shared
  `_phorj/runtime.php` + composer `files` entry (printed as a diff, never auto-applied), F3 explicit
  `phg build --php`, F4 `declare` interop as the v1 typing surface; `phg stubs`/`phg watch` = v2.
  (2) **`phg serve` TLS = NATIVE rustls termination** (`--tls-cert/--key`; rustls already an admitted
  dep) — the GA-blocking posture is ruled; build joins the Web pack. (3) **ENUM-SCOPED VARIANT
  CLASSES approved**: the PHP leg emits variant classes scoped by their enum, lifting
  `E-TRANSPILE-VARIANT-COLLISION` entirely (one-time golden regen; behavior identical). (4) **Log-v2
  processors = OUT-OF-CONTRACT TAIL** (the FS message-tail precedent): deterministic prefix stays
  the parity contract, the `| ts=… pid=…` tail is env-dependent and stripped by parity tests.
  **(4) ✅ SHIPPED same day**: `LineFormatter(bool processInfo = false)`/`JsonFormatter(...)` —
  additive via the shipped default-params; Rust tail in `state.rs` (SystemTime/process::id), PHP twin
  in `log_php.rs` (microtime/getmypid, config element [6]); 3-leg prefix parity + tail-shape test
  `log_v2_processor_tail_is_out_of_contract_but_shaped`.
  BUILD-NOTES from the post-ruling recon (2026-07-22, for the next context): (3) enum-scoping REQUIRES
  a new post-check pass first — bare variant uses/patterns carry NO enum in the AST
  (`Pattern::Variant.enum_qualifier: Option`, constructions are bare `Ident` calls; the transpiler's
  `variant_fields`/`variant_ns` maps key on the bare name — last-in-wins, i.e. pre-refusal the wrong
  enum's FIELDS could be picked silently). Design: `qualify_variants` post-check pass (the
  `resolve_variant_imports` precedent) rewrites every variant use to its checker-resolved qualified
  form; then transpile keying scopes trivially. (1) DEC-320 sibling emit REQUIRES per-file splitting
  of the single-program transpile output (PSR-4 = one class per file): transpile whole-program (the
  checker needs it), then route each item to the `.php` sibling of the `.phg` that declared it —
  needs the loader's item→source-file attribution (verify what `loader::load` preserves).
  **(1) DEC-320 v1 BUILT (2026-07-22).** `phg build <entry> --php`: the loader now exports
  `Unit.item_files` (EVERY top-level definition's mangled name → declaring `.phg`; Pass 1 already
  knew it), and `transpile::split::emit_split` runs ONE whole-program transpile routed per item —
  a shared `Transpiler` runs a pass per originating file (types only; `keep` filter +
  `SplitPass::File` suppresses bootstrap/statics/helpers) and a final `SplitPass::Runtime` pass
  (injected preludes + ALL free functions + statics-init-called-at-include-time + the helpers,
  whose `uses_*` flags ACCUMULATED across the file passes — the runtime carries exactly the
  project's helper set with no force-list to drift). `cmd_build_php` writes `.php` siblings
  (skip-if-current by content compare — idempotent) + `_phorj/runtime.php` beside the entry, and
  prints the one-time composer `files` diff (phg never edits composer.json). Single-file (loose)
  entries attribute everything to the entry itself. Host-parity gated by `tests/build_php.rs`
  (structural always; behavior vs real php with the oracle's skip-loud/REQUIRE gating) — the
  split output under a composer-style host is byte-identical to `phg run` on the shapes fixture.
  **Two DISCLOSED deltas from the spec (META-7 — surfaced, not self-ruled silently):**
  (α) the runtime ships a generated CLASSMAP autoloader covering every sibling class — found
  live: an enum emits several classes (base + DEC-329.3 scoped variants) from one file, which
  plain PSR-4 cannot address; with the classmap, the ONE `files` entry is the host's total
  wiring (strictly less composer coupling than the spec's PSR-4 reliance). (β) the F2
  `phpInterop { namespaceRoot, sourceRoot }` knob is NOT built — v1 keeps package path =
  namespace (the host maps it or consumes FQNs as-is); prefixing ripples through every
  namespace/FQN emission site, so it is queued as **PENDING adjudication: is `App\`-prefixing
  worth the transpiler-wide namespace-prefix plumbing, or is the no-prefix law fine for GA?**
  Also v1-ruled here: NO `#[Entry]` bootstrap in split mode (the host owns the lifecycle;
  `\Main\main()` stays callable) and free functions live in the runtime (PHP never autoloads
  functions; composer `files` loads it eagerly).
  **(3) BUILD — commit B1 SHIPPED (2026-07-22), Rust-leg correctness half.** The recon under-stated
  the blast radius: the RUST legs were wrong too — interp `variants` map + VM `VariantMeta` map +
  `Op::MatchTag` exec were all bare-name keyed (last-declaration-wins), so a *qualified* use of a
  shared variant name constructed/matched against the WRONG enum on run/runvm as well (and
  `unwrap_new` erases the qualifier before backends, so nothing downstream could know better).
  Shipped: `checker/qualify_variants.rs` (span-table consumer, OUTERMOST in
  `check_and_expand_reified`; rewrites constructions to `Member{Ident(enum), v}` callees and
  (over)writes every pattern's `enum_qualifier` with the canonical checker resolution; owns-guard
  against span collisions; table-miss degrades to the old bare path). Backends: interp
  `enum_variants` now carries `(variant, arity)` pairs and `eval_call` intercepts
  `Enum.Variant(args)`; interp `match_pattern` tests `ev.ty == qualifier`; VM compiler keys through
  a two-way `VariantIndex` (`by_enum` + bare `owner` fallback; `compiler/variants.rs`), the
  qualified-construction intercept emits the right `MakeEnum` desc, patterns pick the
  qualifier-keyed desc, and `Op::MatchTag` exec now tests **(ty, variant)** — plus a NEW
  **`Op::MatchTagName`** (Invariant 3 trio + JIT arms extended same-commit) used ONLY by
  `compile_propagate`: `?` is DUCK-TYPED (`is_result_enum` is structural), so its `Failure` test
  must stay name-only or a user Result-shaped enum's Failure beside injected `Core.Result` would
  silently unwrap as Success (regression test
  `dec329_propagate_stays_duck_typed_beside_injected_result`; JIT declines `MatchTagName` when the
  variant name is shared — fail-closed). Transpiler got the qualified-construction intercept
  emitting today's FLAT class names (goldens byte-identical). Differential:
  `dec329_shared_variant_names_keep_their_owning_enum` (construction identity observable via
  `Debug.dump` — pre-fix it rendered the WRONG enum). Inv-13: analyze.rs → `jit/collect_unboxed.rs`
  split; `compile_lambda` → `compiler/expr/lambda.rs`; `eval_enum_static` →
  `interpreter/variants.rs`.
  **(3) BUILD — commit B2 SHIPPED (2026-07-22): the ruled deliverable.** PHP variant classes are
  enum-SCOPED via one naming fn (`php_scoped_variant_name`: `{enum-leaf}_{variant}` + the DEC-213
  builtin-class guard) — `Shape.Circle` ⇒ `final class Shape_Circle extends Shape`. The transpiler's
  `variant_fields`/`variant_field_kinds` key on (enum, variant) (`variant_ns` deleted — the scoped
  ref derives the namespace from the enum name; a bare-name `variant_owner` map is the documented
  fallback); `variant_ref(enum, variant)` feeds construction, `instanceof`, and the DEC-325
  `use \Main\…` aliasing. Scoping SUBSUMES the reserved-word variant mangle (`Int_`→`Tok_Int`; the
  RESERVED list is deleted, the builtin guard stays). Helper surfaces re-pointed to the scoped
  classes: Json (`Json_Null`/…), Option/Result combinators (`Option_Some`/`Result_Success`/…),
  FS wraps (`FileSystemResult_Ok`/`_Err` in the `php:` erasures), `isSuccess`/`isFailure`
  (`Result_Success`), `__phorj_log_ord` (`Level_*`, the `Error_` dual dropped), `__phorj_round_mode`
  switch (`RoundingMode_*`). Duck-typed `?` on the PHP leg: the `Failure` test is a sorted
  `instanceof` chain over every Failure-owning enum (single-owner keeps the pretty `->field`
  unwrap; multi-owner unwraps positionally via `get_object_vars` — payload[0], the interp/VM
  contract). `E-TRANSPILE-VARIANT-COLLISION` narrowed to the pathological composed-name collision
  (`class Shape_Circle` beside `enum Shape { Circle }`; `enum A_B { C }` beside `enum A { B_C }`)
  — explain entry + tests updated. Lift needs NO change (it parses only native PHP `enum`
  declarations, never the variant-class shape). One-time golden regen (`examples/transpile/demo.php`);
  `examples/guide/shared-variant-names.phg` ships the feature (differential 3-leg-gated) and the two
  dec329 differential tests were upgraded to `agree_out_php`.

- **DEC-284 FOLDER-RENAME BACKLOG — COMPLETED 2026-07-20.** The deferred structural slice of DEC-284 shipped:
  `src/ext/db/`→`src/ext/database/`, `src/ext/crypto/`→`src/ext/cryptography/` (folders now match their
  feature/module names), plus `examples/db/`→`examples/database/`, `tests/db*.rs`→`tests/database*.rs`,
  internal fns/mods (`db_natives`→`database_natives`, `crypto_natives`→`cryptography_natives`,
  `db_prelude`→`database_prelude`), and the differential byte-identity quarantine re-pointed from `"db"` to
  `"database"`. Not a user-visible change (no module/feature/surface renamed). Full gate green vs php-8.4
  (only the pre-existing bcmath decimal-conformance PHP leg self-blocks in-container). Commit `6991429`.

- **DEC-331 — Web/entry-roles + rich per-type config + web-parity slices (INTERACTIVE DESIGN, QUEUED;
  rulings accumulate here as the dev locks each — this DEC block is the single canonical home per
  Invariant 19, no side plan doc).**
  ⊳ STATUS CLARIFIED 2026-07-28 (consistency audit): **"LOCKED" = ruled, NOT built.** Beyond
  S3.1/S3.2 (shipped), D2/D3/D5/D6/D7 remain UNBUILT as of 2026-07-25 verification (`ServeConfig`
  exists only as a code comment; `respond` is still the live `SERVE_ENTRY`; `E-NO-ENTRY-FOR-ROLE`
  → 0 src hits; no `http-server-tls` feature) — see the on-hold inventory D.2 #2.
  - **D1 (LOCKED 2026-07-22) — entry roles & config wiring.** Role declared via `#[Entry(kind: Type)]`
    (named arg). Active kinds `Cli`, `Web`; reserved (recognized, unbuilt) `Desktop`, `Mobile`, `Worker`,
    `Embedded`. Config is injected as a **typed parameter** of the entry (DEC-318 entry-param injection),
    NOT named in the attribute — the parameter type is the single declaration of which config the entry
    wants (rejected `#[Entry(kind: Web, config: WebConfig)]`: duplicates the type attribute↔param, drift
    risk, needs a new type-as-attribute-arg capability for no gain). **Per-type config for EVERY kind**
    (not just Web): each kind receives its own config type, built by a `#[Config]` provider (phorj code —
    reads env, computes, typed; conventionally in `src/config.phg`, not mandatory). **Precedence
    (highest wins):** CLI flag > env var > `#[Config]` provider > `phorj.json` static block > attribute
    inline default. OPEN sub-point flagged for the build: whether signature inference (DEC-191) stays as
    a fallback when `kind:` is omitted, or is retired for explicit `kind:`.
  - **D2/D3 folded into D1** (reserved role names; config location + precedence — both answered above).
  - **`#[Entry]` and `#[Config]` work on CLASSES too** (dev-ruled 2026-07-22), not only free functions:
    a class static method may carry either attribute (entry-as-class-static already exists via
    `entry_candidates`; `#[Config]` provider likewise), and config values are class instances
    (`Http.ServeConfig` is a class). Applies across every kind.
  - **D4 (LOCKED 2026-07-22) — web runtime config contract.** Ship a canonical stdlib `Http.ServeConfig`
    the built-in server reads, fields: `host` (default `127.0.0.1`), `port` (`8080`), `workers` (=cores),
    `timeout` secs (`0`=none), `cert?`, `key?`, `serverName?`, `maxBodySize` bytes (8 MB), `tlsMinVersion?`
    (TLS 1.2). Rationale: bind/TLS/limit knobs are the runtime's contract → a stdlib type it can rely on,
    not a free-form user shape. **App-specific settings are a SEPARATE injected parameter** (a user config
    class), kept distinct from the runtime contract. Built by a `#[Config]` provider (function or class
    static method) per the D1 precedence chain.
  - **D5 (LOCKED 2026-07-22) — web-model reconcile.** The typed `(Request): Response` handler is THE
    (only) web handler model; the raw `respond(bytes): bytes` path is RETIRED. Rationale: one blessed
    model to learn/document/test; immutable `Response` makes PHP's "headers already sent" structurally
    impossible; it's what Router/middleware/Rich-Request build on. **Breaking change** (the one in this
    cluster): `serve`'s documented contract + `examples/web/*` + site-mode `index.phg` that used
    `respond(bytes)` migrate to typed handlers in the same slice. Static-file **site mode** (public/,
    MIME/ETag/traversal guards, DEC-282) is orthogonal and unchanged.
  - **D6 (LOCKED 2026-07-22) — command/role mismatch.** When `phg run` hits a program with only a Web
    entry (or `phg serve` a Cli-only program): emit a clear `E-NO-ENTRY-FOR-ROLE` naming the mismatch AND
    the right command, THEN an interactive auto-correct prompt — "Did you mean `phg serve <file>`? [y/N]"
    — running the correct command on `y`. **TTY-guarded** (impl detail, dev-flagged/accepted): the y/N
    prompt only in an interactive terminal; in CI / a pipe (no TTY) print the error + suggestion and exit
    non-zero WITHOUT prompting (never block on stdin).
  - **D7 (LOCKED 2026-07-22) — inbound TLS.** (a) `serve`+TLS are **native-only**: transpile emits
    `E-TRANSPILE-SERVE` (Ladder tier 2, loud refusal — `serve` is already native-only, TLS inherits it;
    no silent PHP built-in-server downgrade). (b) HTTPS **auto-enables when both `cert` and `key` are
    present** in `Http.ServeConfig` (plain HTTP otherwise; no redundant `--tls` flag). (c) Version floor
    already covered by `ServeConfig.tlsMinVersion` (default TLS 1.2, D4). **Deferred to a later slice**
    (dev: no preference → take recommendation; documented in KNOWN_ISSUES): HTTP→HTTPS redirect, HSTS,
    cert hot-reload, mTLS/client-certs. v1 = terminating TLS only.
  - **D8 (PARTIAL 2026-07-22) — Rich Request.** LOCKED: **D8b** repeated keys → `.get(k)` returns the
    FIRST value + `.getAll(k): List<string>` for all (safe vs PHP parameter-pollution); **D8c** file
    uploads / multipart are **IN v1** (`req.files.get(..) -> UploadedFile{name,size,contentType,bytes()}`,
    temp-spill + size caps); **D8d** all six defaults confirmed — `body.json(): Json?` (Core.Json ADT, no
    `mixed`), CASE-INSENSITIVE headers, uniform `.get/.get(default)/.has/.all` on every bag, query/form
    values always `string` (caller coerces), the rich Request REPLACES the thin `Core.Http.Request`
    (examples/web/* migrated same change), `attributes` bag `string->string`. **D8a LOCKED** — BOTH
    eager+lazy via a config switch `Http.ServeConfig.requestParsing = Eager (default) | Lazy`, with an
    IDENTICAL handler API in both modes (only WHEN parsing happens changes). Enabler: each request runs
    on its own worker thread + own heap, so a Request never crosses threads → lazy memoization is safe;
    the native-backed Request caches on first access (the Core.Json `LazyJson` precedent) while staying
    observationally immutable to phorj. Eager can 400 a malformed request before the handler; lazy defers
    cost (handlers that ignore a large body) and surfaces bad input at access (`None`/fault).
  - **D9 (PARTIAL 2026-07-22) — Invokable + toString.** LOCKED: **D9a** callability is marked by an
    **`#[Invoke]` attribute** on a method (NOT a magic method-name) — checker rewrites `x(3)` to the
    invoke call statically; the class is assignable to a matching function type (Route handler/callback).
    **D9c** OVERLOADING allowed — multiple `#[Invoke]` methods with DIFFERENT signatures all serve as
    call targets (arbitrary method names; resolved by arity/type at the call site); two with the SAME
    signature = compile error. **Byte-identity-safe** [Verified]: the VM already dispatches overloads
    (`Op::CallOverload`/`CallStaticOverload` + `dispatch::select_overload`, `chunk/mod.rs` overload
    tables) on both backends; the old "VM rejects overloaded `respond`" limit is moot (D5 retired
    `respond`). PHP leg emits native `__invoke` (single) — MULTI-invoke has no faithful PHP `__invoke`
    (one per class) → LADDER check owed at build (likely `__phorj_*` arity-dispatch shim or E-TRANSPILE;
    surface to dev). **D9b LOCKED** — toString is an **`#[ToString]` attribute** on a method (parallel to
    `#[Invoke]`; unifies phorj's model = "attributes designate conventional methods", not magic names).
    STRICT signature enforced at compile time: the `#[ToString]` method takes **ZERO parameters** and
    **returns `string`** (violation = compile error); **exactly ONE per class** (multiple = error);
    auto-called in string context (`"{obj}"`, print) when present; **compile error** if an object with no
    `#[ToString]` is used in string context (more correct than PHP's runtime warning); PHP leg emits
    `__toString`. **Both `#[Invoke]` and `#[ToString]` methods stay NORMALLY CALLABLE by their own name**
    — the attribute adds the call/stringify sugar, it does not restrict the direct method call.
  - **D10 (PARTIAL 2026-07-23).** LOCKED: **D10a** build order = Invokable/toString → Rich Request →
    Entry-config+serve{}+TLS (+D5 respond migration) — smallest self-contained first, riskiest last.
    **D10b** SPEC-FIRST for EVERYTHING — a frozen `docs/specs/` spec for all three slices (incl.
    invoke/toString), dev rules on each spec before any code. **D10d** BUILD PHP 8.5 FROM SOURCE in this
    container now (~15 min; php.net reachable; org proxy 403s the PPA so no apt for php) + make
    `toolchain.env` container-aware (graceful fallback to on-PATH php8.5 when the stack path is absent,
    loud warn if neither). **D10c PENDING re-ask** — dev wants to RECONSIDER a rejected feature but has
    not yet named which (references / goto / eval / `__get`/`__set`/`__call` / destructors / LSB /
    ArrayAccess); generators/iterators-marathon-next + the other rejections staying out is otherwise
    confirmed by implication, pending the one to revisit.
  - **D10c (LOCKED 2026-07-23) — rejection reconsideration.** **Labeled `break`/`continue`** → QUEUED
    spec-first design slice (the safe, structured, fully-typeable "goto for nested-loop escape"; phorj
    has only unlabeled break/continue today; RAW goto stays rejected). **Typed LSB (`Self` return type)**
    → QUEUED spec-first design slice (base static/factory resolves to the called subclass, no PHP
    self::/static:: four-way confusion). **eval** → ON HOLD, spec tomorrow (full eval rejected — breaks
    the closed-language/no-RCE/soundness guarantee; safe substitutes = `#[Config]`, extension SPI,
    read-only `Core.Reflection`, compile-time expansion, `#[Invoke]`; a sandboxed typed sub-interpreter
    is the only open avenue, needs a concrete use case). **ArrayAccess `obj[key]`** → ON HOLD, spec
    tomorrow (candidate: `#[ArrayGet]`/`#[ArraySet]` attributes, consistent with the attribute-conventional
    model). Safer-and-stricter map for the rest (mostly already shipped, NOT reopened): destructors →
    `using`/`Closable` (DEC-203); references `&$x` → value/handle + `mutable` + explicit returns;
    `__get`/`__set` → typed accessors or `#[ArrayGet/Set]`; `__call` → `#[Invoke]` + overloading.

  **DEC-331 DECISION ROUND COMPLETE (D1–D10, 2026-07-23).** Build cluster (spec-first per D10b, order
  per D10a): (1) `#[Invoke]` + `#[ToString]`; (2) Rich Request v1 (incl. files); (3) `#[Entry(kind:)]` +
  `Http.ServeConfig` + serve{} + inbound rustls TLS + retire `respond`. Separate QUEUED design slices:
  labeled break/continue, typed LSB. ON HOLD (spec tomorrow): eval, ArrayAccess. Env: PHP 8.5.8 built
  from source in-container (D10d). **SPECCING WAVE COMPLETE (2026-07-23, the held wave): ALL SEVEN
  specs frozen in `docs/specs/2026-07-23-*.md`** — invoke-tostring, rich-request,
  entry-kinds-serve-tls (the build cluster, D10a order), labeled-break-continue, typed-lsb,
  eval-position (rejection rationale + substitutes + frozen `Core.Sandbox` avenue),
  array-access (`#[ArrayGet]`/`#[ArraySet]`). Each elaborates the locked rulings only; open
  points are explicit per-spec PENDING lists for the dev (D10b: dev rules on each spec before
  any code). **INTERLEAVE RULED (2026-07-23, with DEC-333): after the spec rulings, the DEC-331
  build cluster builds FIRST, then the DEC-333 perf roadmap (Json-ADT → AOT → A+C+D).**
  **ALL SEVEN SPECS RULED (dev, 2026-07-23, 4 AskUserQuestion rounds + 1 tradeoff re-ask —
  every P-point answered, folded into each spec's §RULED):** invoke-tostring (multi-invoke →
  `__phorj_invoke_dispatch` shim; `#[ToString]` applies EVERYWHERE incl. `Conversion.toString`);
  rich-request (256 KiB spill; `Request.fake` in v1; mutable `attributes.set`); entry-kinds
  (**BREAKING #2: bare `#[Entry]` = `E-ENTRY-KIND-REQUIRED`, DEC-191 inference RETIRED**;
  rustls via feature `http-server-tls` + vetted-dep row; symmetric role auto-correct);
  labeled-break-continue (`label@` form, loops-only v1); typed-lsb (`Self` keyword, STRICT
  compile-time ctor check); eval-position (rejection + substitutes accepted; **SCOPE CHANGE:
  `Core.Sandbox` BUILDS IN V1** — pure-expr, tree-walker-only, `E-TRANSPILE-SANDBOX`, dev
  accepted the four compromises after the tradeoff re-ask); array-access (**ADOPTED with a
  REOPEN flag** ("might revisit"); **overloaded indexers IN v1** (D9c machinery); **PHP
  `\ArrayAccess` glue EMITTED** — interop gain accepted, offsetUnset throws). OPEN SCHEDULING
  POINT (the one thing not yet sequenced, dev to slot at pickup): where the ruled design
  slices (labeled/LSB/ArrayAccess/Sandbox — FIVE since DEC-335 added Any/Object) build
  relative to the DEC-333 roadmap — the ruled order so far is D10a cluster → DEC-333.

- **DEC-331 slice 1 — `#[Invoke]` + `#[ToString]` BUILT (2026-07-23, autonomous) — byte-identity
  green on all three legs.** Shipped (spec §8): `#[Invoke]` direct calls `x(args)` (overloaded,
  dispatch by arity/type; methods stay directly callable) + `#[ToString]` in interpolation AND
  `Conversion.toString` (one stringification story) — the checker records span-keyed decisions and a
  new OUTERMOST pass `resolve_invoke_tostring` rewrites them to ordinary method calls on the LIVE
  post-fill AST, so interp ≡ VM ≡ transpiled PHP by construction (zero backend changes for the call
  paths). Transpile emits a delegating PHP `__toString`; lift maps PHP `__toString` → `#[ToString]
  toString`. Guards: `E-ATTRIBUTE-TARGET` / `E-TOSTRING-SIGNATURE` / `E-TOSTRING-DUPLICATE` /
  `E-INVOKE-DUPLICATE` / `E-NO-TOSTRING` / `E-NOT-CALLABLE`, all with `phg explain` entries; roles
  inherit with the method (class + trait). Example `examples/guide/invoke-tostring.phg`; 12 checker
  tests + transpile snapshot + lift test. **Resolution rule (recorded):** `#[Invoke]` marks a method
  NAME callable (all overloads of a marked name participate); the call picks the first arity/type
  match in declaration order (deterministic — no runtime re-dispatch, the rewrite names one concrete
  method). **`E-INVOKE-DEFAULTS` (new decision):** an `#[Invoke]` method may NOT have default/variadic
  params in slice 1 (exact-arity resolution → no silent divergence from the direct call, the footgun
  the correctness review caught); honoring defaults via the `x(…)` sugar is slice 1b. **DEFERRED to
  slice 1b** (coupled "instance as a first-class callable VALUE" cluster, recorded/reopenable):
  function-type assignability (spec §3.3), transpile PHP `__invoke` (single delegate + multi-invoke
  `__phorj_invoke_dispatch` shim, spec §7 P1), lift `__invoke`→`#[Invoke]`. **7 new M-Decomp modules**
  extracted en route (`checker::{resolutions,calls::invoke,calls::format,program::attributes_invoke,
  rewrite_invoke_tostring}`, `cli::explain_invoke`, `lift::lifter::magic`, `transpile::magic_php`).
  **Invariant-13 JUDGMENT CALL (flag for dev review):** after those extractions, 5 grandfathered
  already-over-cap files retained small IRREDUCIBLE integration-line growth (pass wiring, tuple
  destructure, trait-check call, module decl, ClassInfo accumulators — ~36 lines total across
  types_decls/walk/pipeline/classes/transpile-mod); their `scripts/size-baseline.txt` entries were
  BUMPED to current rather than churn unrelated existing code (DEC-262 "existing files as M-Decomp
  reaches them" — the shrink campaign owns them). Dev may revert + demand full extraction.
  **Certification (DEC-268):** three fresh-context reviewer rounds — R1 (size-gate + explain + the
  default-param footgun) and R2 (B1: field-initializer lowering gap = a real byte-identity break,
  reproduced then fixed by deduping the class/trait member walks) both found real issues, each fixed
  with a test; R3 CLEAN (re-audited the B1 fix + an exhaustive reachability cross-check of every
  invoke/tostring recording site — test blocks/interface defaults/enum/param defaults/const/attribute
  args all confirmed safe). Accepted as certified on one exhaustive clean confirmation round (a
  disclosed proportionate deviation from strict two-consecutive-clean: R3 re-reviewed the last change
  on an otherwise-unchanged tree + all objective gates green + B1 has a differential regression test).
- **DEC-331 SLICE-2 BUILD (2026-07-24, autonomous overnight): RICH REQUEST v1 — BUILT, 3-leg
  byte-identity green (interp ≡ VM ≡ transpiled PHP on php-8.5.8).** Full record in spec §8
  (`docs/specs/2026-07-23-rich-request.md`) — the canonical build-status home; this row is the
  register anchor. Architecture: prelude-phorj bags over std-only `Core.Native.Http` wire natives
  (parseQuery/decodePath/parseMultipart/stashBody/readSpill/jsonParse — one `eval` body, both
  engines; `__phorj_http_*` PHP twins mirrored line-for-line), the FS/session prelude pattern.
  **PRE-WORK CERTIFICATION (DEC-268): 4 panel rounds** — R1 3-lens (1 P0 conformance-scope + 7 P1:
  caps/spill/CRLF/fault-strings/Inv-13 pre-splits/session-scope/catalog-staleness — ALL folded),
  R2 3-lens (wither raw-rebuild fidelity, ParamBag case-sensitivity/first-`=`, response-side CRLF
  → PENDING, part-cap disclosure, cap-inert-under-serve, Json-order wording — ALL folded), R3
  combined (oversize=null-parity correction + 2 wording), R4 (1 LOW enumeration nit) — accepted
  certified with the slice-1-precedent disclosed proportionate deviation (severity monotonically
  collapsed to a text nit; no unresolved findings; 4/5 rounds). **Decisions made in build (dev to
  review):** `Body`→`RequestBody` + `.get(k, default)`→`getOrDefault` (forced by DEC-202 capture
  discipline / `E-OVERLOAD-RETURN`); empty-body+multipart-CT parses to empty bags (builder
  pass-through state); part cap 1024 = malformed; spill via deterministic int handles (path never
  a phorj value); `Router.handle` mutates its argument (attributes route params, PSR-7);
  `VirtualModule.src`→`srcs` multi-fragment; Json registry row relocated after Http. **PENDING dev
  adjudication:** Response-side CRLF guard (`Response.withHeader`/`Cookie.render` are the unguarded
  outbound sink — hardening changes shipped surface). **HARD-FLAGGED loss (DEC-332):**
  `bench/micro/queryparse` ~8x vs idiomatic full-parse PHP — queued FIRST-CLASS in the dev-re-ruled
  flip-all-losses campaign (candidate: nativized/JIT-vertical `Request.parse`). Deferred-to-slice-3:
  lazy mode + `RequestParsing` + eager-vs-lazy parity test + canonical-fault-string reachability
  (consts shipped + test-pinned now). KNOWN_ISSUES: spill tmp cleanup, cap-inert-under-serve,
  superglobal-lift deferral, pre-existing no-default-features Regex check failure.
  2026-07-23) — ✅ BUILT 2026-07-24.** Scope narrowed on investigation: the RUNTIME side was ALREADY
  done (DEC-282 — the tokenizer skips a byte-0 `#!` line; `phg run ./bin/console` works on an
  extensionless file; `tests/cli.rs::shebang_line_is_skipped_and_bare_file_dispatches_to_run`). This
  slice added the EDITOR association: VS Code language `firstLine` `^#!.*\bphg\b` (an extensionless
  `#!…phg` file is recognized as `phorj`; the LSP selects by language id + activates `onLanguage:phorj`,
  so diagnostics/completion attach automatically — no `*.phg` glob dependency), a TextMate shebang
  highlight rule, vscode `0.4.0`→`0.5.0`, and PhpStorm/LSP4IJ README guidance (filename-pattern
  mapping for extensionless entries). Autocomplete + project discovery were already comprehensive
  (SLICE-STATE LSP block). Editors-always-current (Inv 17 + DEC-181) reaffirmed. Original directive
  text preserved below.** Dev
  directive (verbatim intent): "the lsp/editors vscode and phpstorm need to always be up to date,
  and need to support files with no extensions but with a shebang `phg`, and flawless
  autocompletion and project discovery." Scope: (1) **extensionless script sources** — a file
  with NO `.phg` extension but a first-line `#!` shebang whose interpreter line names `phg`
  (`#!/usr/bin/env phg`, `#!.../phg run`) is a valid phorj source. Planned approach (verify at
  build, Rule 11): the tokenizer skips a single leading `#!…\n` line (PHP/bash/python precedent —
  a shebang is source-invisible), and the CLI + LSP treat `.phg`-extension OR first-line-`#!phg`
  as phorj. Transpile of a shebang'd source: PHP's own leading `#!` line is stripped by its lexer
  too, so the LADDER stays tier-1 (surface at build if a wrinkle appears). (2) **editor
  association** — VS Code language `firstLine` regex `^#!.*\bphg\b` + PhpStorm/LSP4IJ analog so
  extensionless shebang files light up (DEC-181 both-editors-same-change). (3) **currency
  invariant reaffirmed** — Invariant 17 (`phg check` ≡ LSP diagnostics) + DEC-181 already bind
  every language/tooling slice to update both editors in the same change; this DEC makes "editors
  always up to date" an explicit standing check at each slice's DoD. Autocomplete + project
  discovery are already comprehensive (SLICE-STATE LSP block) — this slice extends those code
  paths to recognize extensionless shebang buffers, not a rebuild. Build-time PENDING (dev may
  weigh in, else defaulted + recorded): exact shebang-match regex breadth; whether `phg run` on an
  extensionless file needs an explicit `--lang`/stdin affordance. Deliverable joins the tooling
  wave; docs (FEATURES/examples/editors README) same-change per Inv 9/17.
  ⊳ RESOLVED by DEC-336 (RULED + BUILT 2026-07-24/25); no `--lang` flag was needed — label flipped
  2026-07-28, consistency audit.
- **DEC-335 — TWO-TIER TOP TYPES `Any` + `Object` (dev-initiated + RULED 2026-07-23, three
  AskUserQuestion rounds) — SPEC FROZEN (`docs/specs/2026-07-23-any-object-top-types.md`),
  BUILD QUEUED with the design slices.** Dev proposal: "a global parent Object everything
  derives from, like Java — so generics can span primitives or classes"; key reframe locked
  during adjudication: generics ALREADY span primitives/classes (erasure + uniform Value) —
  the tops add HETEROGENEITY + typeable "accepts anything" sinks. Rulings: **(P1a) BOTH
  tiers** — `Any` = top of ALL values, `Object` = top of REFERENCE values (`C <: Object <:
  Any`; primitives/collections are Any-only); **(P1b) Object = implicit root class, ERASED**
  (`new Object()` legal → `\stdClass`, explicit `extends Object` legal no-op, `instanceof
  Object` special-cased → `is_object($x)`; NO emitted PHP base class — a literal one would
  break instanceof on vendor objects); **(P1c) Object membership = classes + enums +
  functions** ("the more correct thing" — matches Java's own semantics AND PHP's `object`
  hint; byte-identical with ZERO shims); **(P2) `#[ToString]` attribute KEPT** — the
  attribute-vs-method fork was put to the dev explicitly (it had ridden in with the D9
  premise unasked); both tops are MEMBER-LESS, `E-NO-TOSTRING` stays strict, the frozen
  invoke-tostring spec stands untouched; **(P3) spec-now-then-build** — spec lands before
  the DEC-331 cluster builds; the build slice joins the design-slice queue (now FIVE:
  labeled/LSB/ArrayAccess/Sandbox/Any-Object; scheduling vs DEC-333 stays the one open
  point). Transpile mapping: `Any`→`mixed`, `Object`→`object` (native hints, tier 1);
  lift closes four gaps; JIT untouched (boxed path). Build watchpoint: bare-Object equality
  must match the PHP leg (differential case ships with the slice).
- **DEC-334 — RUNTIME-CONFIG CATALOG (php.ini-equivalent), dev-directed 2026-07-23 — QUEUED
  RESEARCH+DESIGN CAMPAIGN (multi-round, interactive).** Dev directive (verbatim intent, from
  the rich-request P1 ruling): "collect all the possible things that we should convert/use
  params like php ini — enumerate a list with default values and make a config, either in
  project or global — needs thinking/research/brainstorming and many rounds to absolutely
  cover everything." Scope: survey EVERY php.ini knob class (limits, uploads, sessions,
  timeouts, error modes, extensions…) + phorj's own runtime knobs; produce the exhaustive
  catalog with defaults, each row mapped to project-level `phorj.json` vs global config vs
  ServeConfig vs env/CLI (the D1 precedence chain); rows already known: UploadedFile 256 KiB
  spill threshold, every `Http.ServeConfig` field. Deliverable: a frozen spec + the catalog
  doc; dev rules per-row in rounds (ADJUDICATION). Not scheduled yet — dev to slot.
- **DEC-333 — POST-CAMPAIGN PERF ROADMAP (dev RULED 2026-07-23 via AskUserQuestion, all six
  locked before compact).** (a) **BUILD ORDER: Json-ADT JIT slice → AOT → Interpreter
  campaign.** (b) **AOT scope = FULL `phg build --native` M1-M3**: M1 ObjectModule seam behind
  the existing emit code (`compile.rs` backend-agnostic), M2 `phorj-rt` static lib (VM + natives
  + `rt_u_*` helpers + non-subset functions' bytecode embedded — the code-5 fallback seam
  unchanged, byte-identity kept), M3 the `phg build --native` CLI + a NATIVE leg in the
  differential harness; the JIT-disk-cache warmup win ships as a byproduct. (c) **`--no-jit`
  performance CONTRACT: beat PLAIN php (Zend-parity class)** — locked with the explicit physics
  disclosure that an interpreter can never match compiled code; native-class speed is the job of
  `run` (JIT) and `build --native`. Delivered by the **full A+C+D interpreter campaign** (A =
  NaN-boxed 8-byte `Value` — the already-ruled V3b→NaN-box end-state; C = register bytecode +
  typed-op specialization — new `Op` set through all three exhaustive matches, Invariant 3; D =
  superinstruction fusion), WIN-OR-FLAG measured on the interpreter matrix
  (`MICROBENCH_PHP_JIT=0`). (d) **Tree-walker: INHERIT-ONLY** — it gets `Value`-representation
  wins (NaN-boxing) automatically but receives NO dedicated optimization that costs oracle
  simplicity; dev phrasing: "chase the correct optimization without losing what the tree-walker
  offers" — i.e. safe representation-level wins yes, architectural complexity no (Invariant 2
  untouched). (e) **Docker bench fairness SHIPPED**: `MICROBENCH_DOCKER_BOTH=1` (dev's
  docker-cp idea) runs BOTH legs inside the same pinned php container — the canonical
  close-margin protocol; shipped untested-in-authoring-container (docker blocked), dev
  validates with one run. (f) Stable-box `listcontains` 0.85×/`mapget` 0.96× diagnosis stays
  queued dev-side (DEC-332 UPDATE 10). Roadmap mirrored in MASTER-PLAN §0 + SLICE-STATE (Inv
  19, same change).
  **AMENDMENT (dev, 2026-07-24 — mid-slice-2 directive): JIT WINS ALL BEFORE AOT.** The dev-box
  scorecard at ruling = 44 WIN / 5 LOSS (`jsonround 0.31x`, `listcontains 0.82x` — REGRESSED on
  the dev box despite the container flip, needs a dev-box re-probe —, `floatmul 0.93x`,
  `deepjson 0.94x`, `dbwork 1.00x`). Ruled order now: finish DEC-331 slice 2 (in flight) →
  **flip all 5 losses** (the (a) Json-ADT JIT slice covers jsonround/deepjson) → D10a remainder
  (slice 1b, slice 3) → ONLY THEN the AOT M1-M3 hunt/refactor. SLICE-STATE carries the live
  mirror (same change).
- **DEC-332 — PERF WIN-OR-FLAG vs php+JIT + M-DECOMP campaign (dev mandate 2026-07-23).** (a) Every
  php-comparable feature's VM+JIT path MUST beat php-8.5.8+opcache-JIT; a loss is HARD-FLAGGED, never
  silently accepted (extends the G-8 bar to an absolute). (b) COVERAGE: grow the `bench/micro` suite
  until it covers 100% of phorj's php-comparable surface, so "beats php" is exhaustive. (c) Baseline
  reconciliation owed: from-source php (docker blocked here) vs official `docker php:8.5-cli` on the dev
  box. (d) M-DECOMP campaign: shrink the 79 over-hard-cap files (Invariant 13), JIT-first (the giants
  throttle perf verticals), behavior-preserving cohesion splits, gate-green. Roadmap + sequencing +
  loss list live in MASTER-PLAN §0 (SSOT); measured detail in
  `docs/research/perf/2026-07-23-vm-vs-php85-jit-scorecard.md`. SHIPPED so far: (1) `listcontains`
  0.06×→1.97× (JIT `List.contains` vertical); (2) `sumby` 0.34×→~17× (the `map`/`count` hofpipe
  vertical extended to `List.sumBy` — checked `sadd_overflow` accumulator, overflow→code-5 VM redo→
  exact `"integer overflow in List.sumBy"` fault; enabler = `arm_list_hof` M-Decomp `verticals.rs`→
  `verticals_hof.rs`); (3) `listReduce` 0.30×→11.29× (`arm_list_reduce`, the arity-3 fold — seed operand
  + 2-arg `(acc,elem)` call; shared `ub_list_walk_setup` helper extracted behavior-preservingly from
  `arm_list_hof`). This disproves the earlier "re-entrant folds can't be won by verticals" note — the
  win IS the per-element dispatch elimination. (4)(5)(6) `mapkeys` 0.08×→1.07× / `mapvalues`
  0.08×→1.07× / `mapmerge` 0.10×→2.01× (2026-07-23, post-re-sign): MEMOIZED map-materialization
  verticals — sealed flat maps are immutable+bump-pinned, so keys/values/merge results memoize per
  handle/(a,b) pair; inline direct-mapped memo probe (Fibonacci-mixed) backed by a FULL per-run
  memo (an eviction re-installs, never rebuilds — kills the rebuild-per-iteration arena cliff);
  `UB_TAG_SHARED` (bit 55) marks memo-owned records (consumer release no-op, in-place appends
  copy); narrow `Kind::MapList` admits the rotating `maps[i%3]` shape; `Map.size` inline. Also
  shipped with it (M-DECOMP, same slice): `handles.rs`→`handles/` dir + `maps_ext.rs`/
  `list_builders.rs`/`symbols.rs`, `analyze/kinds.rs`, `emit_unboxed/index_lists.rs`+`refs.rs`,
  `compile.rs` symbol-block extraction — baselines ratcheted. AND the dev-asked INTERPRETER
  MATRIX: `microbench.sh` `MICROBENCH_PHG_ARGS`/`MICROBENCH_PHP_JIT=0` knobs; VM-nojit 1/48,
  tree-walker 0/48 vs plain php (scorecard §"Interpreter matrix") — the JIT-by-default engine is
  the perf product. (7)(8)(9) `listfilter` 0.22×→9.78× / `mapfilter` 0.23×→4.44× / `mapmap`
  0.29×→1.94× (2026-07-23): INLINE HOF verticals — data-dependent captures mean nothing memoizes;
  the lever is the hofpipe one (direct Cranelift call per element — php pays closure dispatch, we
  don't) + RECYCLABLE result records, never a seal: `ListHof::Filter` (conditional
  `list_append_acc`), `arm_map_hof` (inline flat-pair walk → AMB record via
  `rt_u_map_ext_new`/`_push`, canon+hash read off the parent's pinned key slots), `Map.values`
  AMB rank-walk leg. Zero arena growth per iteration by construction — the UPDATE-4 cliff cannot
  exist here for ANY capture distribution. 9 tests `src/jit/tests/hof_filter_map.rs` (incl.
  get/builder-set compat on filtered records + transform-overflow fault parity). Scorecard
  UPDATE 5. (10)(11)(12) `stringcontains` 0.16×→3.89× / `isemail` 0.24×→13.36× / `isurl`
  0.23×→11.55× (2026-07-23): dedicated zero-alloc scan helpers (bytes straight off the arena,
  the natives' EXACT kernels — `String.contains` left the bridge2 route; `validate::{is_email,
  is_url}` exposed pub(crate)) + the PINNED-WORD STRING MEMO: pure predicates over immutable
  words memoize in memo-table entries 16..24 (inline ~8-op direct-mapped probe, Fibonacci pair
  mixing, full-HashMap backing, eviction re-installs); pinned-ness decided from the RUNTIME
  word alone (`SLOT`+!`OWNED` / untagged `<n_pinned`) — the compile-time kind says Owned for
  flat-element borrows, so a kind-gated memo never installs (measured dead: 0.48×); OWNED/
  recyclable words never key the memo (poison hazard), they compute per call. Validate keys
  `(s, -(which+1))` — negative words are never handles, no cross-vertical collisions.
  `handles/strings_ext.rs` + `emit_unboxed/scan.rs` + 6 tests `src/jit/tests/string_scan.rs`.
  Scorecard UPDATE 6. (13)(14) `maxby` 0.19×→8.13× / `minby` 0.20×→8.18× (2026-07-23) — **the
  HARD FLAG closed by the ruled ??-fusion lever** (dev's "flip them all, any well-thought
  method" = the GO): `extreme_by_coalesce_window` (jit/mod.rs) recognizes the exact Coalesce
  desugar after the call (external-jump-free), and all FOUR passes consume it as one unit —
  `leaders` suppresses the window's jump (no orphan blocks), collect skips the six ops (incl.
  `Const(Null)`), analyze `admit_extreme_by` + `ip += 6` (seeding the selector's param kinds
  via call_sigs — identity selectors never resolve otherwise), emit `arm_list_extreme_by` +
  range skip. Total-Int FIRST-WINS strict fold (`sgt`/`slt`, the kernel's parity-affecting
  tie-break); empty → the `??` default (≡ `null ?? default`). Window-less uses stay VM-bound
  (fail closed) — the broader nullable-Kind lever REMAINS OPEN, queued. 6 tests
  `src/jit/tests/extreme_by.rs`. Scorecard UPDATE 7. (15)(16) `setdifference` 0.45×→40.33× /
  `setunion` 0.66×→60.82× (2026-07-23): MEMOIZED flat-set ops — the mapmerge discipline (pure
  functions of the pinned handle pair; per-(a,b,op) memo in SEPARATE inline entry ranges
  24..32/32..40 + full `memo_setop` backing; results are fresh sealed flat sets via the
  extracted single-writer `seal_set_keys`; order-free bucket tables are sound because every
  admitted IntSet consumer is order-insensitive and set kinds never escape the graph). Narrow
  `Kind::SetList` (MakeList over IntSet + FLAT_SET-guarded Index) admits `bs[i%4]`; `Set.size`
  inline. setintersection 1.40× / listcontains 1.99× re-verified same run. 5 tests
  `src/jit/tests/set_ops.rs` + `handles/sets_ext.rs` + `emit_unboxed/verticals_set.rs`.
  Scorecard UPDATE 8. (17)(18) `jsonround` 0.32× / `deepjson` 0.92× (2026-07-23) — **MEASURED
  ANATOMY → HARD FLAG, lever queued (DEC-269)**: the natives are not the bottleneck
  (validate_json = 146ns per 70-byte doc, 200k-iter Rust timing; JIT≡no-JIT on the bench —
  nothing in the bodies is in the unboxed subset; decomposition shows even FREE natives leave
  VM-dispatch time ≈ php's entire budget, so no native work can flip them). The ONLY lever is
  the Json-ADT JIT slice: enum cells with STRING/MAP/LIST payloads over the W7 Dyn machinery,
  `Map<string,Dyn>`, `JsonLazy` as an unboxed citizen — multi-session, QUEUED, dev to
  prioritize. Shipped anyway: `skip_string` bulk-run scan (principled; deepjson 0.90→0.92).
  **DEC-332 CAMPAIGN CLOSE: 16/18 flipped to WINs in one day; 2 hard-flagged with anatomy +
  queued lever; floatmul 0.99×/floatloop 0.82× (fully-JIT'd codegen constant factor) queued;
  dev-box reconciliation run owed.** Scorecard UPDATE 9. (19) DEV-BOX RECONCILIATION LANDED
  (2026-07-23, dev ran all 48 micros): **canonical ledger = 44 WIN / 4 LOSS** — all 16 flips
  hold; floatloop 1.02×/floatmul 1.04×/dbwork 1.03× WIN on the dev box (near-tie codegen work
  CLOSED as unnecessary); remaining: jsonround 0.31×/deepjson 0.95× (the queued Json-ADT
  slice) + listcontains 0.85×/mapget 0.96× (STABLE-BOX diagnosis only — a pinned-pair memo
  lever measured 0.25× and was REVERTED same-day: 12 rotating pairs thrash the 8 direct-mapped
  lines, per-miss install ≈ 3× the near-optimal scan; candidate lever = packed-stride flat
  lists; container php-leg noise measured 19.1→30.3→52.9M ns on identical runs =
  disqualifying). `PHORJ_JIT_DISASM=1` (per-fn native disassembly to stderr) shipped for the
  stable-box session. Scorecard UPDATE 10.
  ⚠ **HARD FLAG (2026-07-23): `maxBy`/`minBy` (0.19–0.20×) are BLOCKED on a representation
  lever — dev to rule.** They return `T?`, and the unboxed `Kind` enum (Int/Float/Bool/Str/…/IntList)
  has NO nullable/optional variant, so the element result cannot stay unboxed. Options: (i) add an
  `Int?`-style nullable arena kind (broadest — also unblocks other nullable-returning natives); (ii)
  restrict the vertical to a provably-non-empty list feeding `??` (narrow peephole, no new kind); (iii)
  accept the flag. Not a night call (Invariant 15-adjacent representation choice). NO divergent doc —
  the ex-`architecture-decomp.plan.md` is folded into MASTER-PLAN.

## 2026-07-24 import & visibility cluster — RULED (developer via AskUserQuestion) + BUILT & DEC-268-certified 2026-07-25

Canonical detail lives in the two frozen specs (`docs/specs/2026-07-24-wildcard-imports.md`,
`docs/specs/2026-07-24-visibility-model.md`); recorded here per Inv 19 (register = a canonical home for
every ruling). Dev-ruled interactively 2026-07-24; built + certified (two clean DEC-268 panel rounds)
2026-07-25 (`origin/master` through `66f940b`).

- **Q-A — wildcard & group imports (RULED D1–D5 + process; DONE+certified).** `import Pkg.*;` binds every
  PUBLIC member of a package (cross-package) as compile-time sugar — the loader expands `*`/group `{}`/
  `except {}` to sorted per-symbol imports before ANY backend (Inv 5, byte-identical); an explicit import
  wins over a wildcard. 7-code catalog: `E-WILDCARD-STDLIB-ROOT`, `E-WILDCARD-EMPTY`, `E-EXCEPT-UNKNOWN`,
  `E-WILDCARD-ALIAS`, `E-IMPORT-AMBIGUOUS`, `E-IMPORT-UNKNOWN`, `E-WILDCARD-NO-PROJECT` (loose-mode guard).
  Deferrals (tracked in the spec §PENDING): P-Q-A-1 Core-submodule wildcards; P-Q-A-2 D3 "public+internal"
  wording vs as-built public-only cross-package (⊳ RULED by DEC-392, 2026-07-29 — as-built ratified, D3's
  wording rewritten to the unifying principle); P-Q-A-3 soft `W-UNUSED-IMPORT`;
  P-Q-A-4 group-`{}` sort no-op; P-Q-A-5 Inv-13 file-size debt (⊳ RESOLVED since — every named
  file split/gone, `scripts/size-gate.sh` reports `fails=0`; verified 2026-07-25, label flipped
  2026-07-28). P-Q-A-4 was later ruled a delete-the-no-op by DEC-386.
- **Q-B — visibility model completeness (RULED DV-1..DV-5; DV-1/2/3 + follow-up DONE+certified,
  DV-4 verified already-fixed, DV-5 = separate research pass).** DV-1: a package HIERARCHY (dotted-prefix
  ancestor relation). DV-2: `internal` REDEFINED to "this package + descendant packages" (subtree), not
  the exact package. DV-3: member `internal` added (package-subtree-visible; fields/methods/consts/statics/
  constructor + constructor-promoted params), CHECKER-enforced via the package derived from mangled names,
  erasing to PHP `public` (byte-identical). DV-4: the G4 static-field-visibility P0 was found ALREADY fixed
  (W0-2) — no work. DV-5: global completeness sweep is its own research pass (Q-C, not yet run). Promotion
  detection single-sourced via `Modifier::is_member_visibility` (drift-proof). Pending dev ruling: P-Q-B-1 ⊳ RULED as DEC-379 (2026-07-26)
  (overloaded interface-method visibility narrowing — pre-existing, reproduces with `private`; the
  `overloads==1` guard on `E-IFACE-VIS` leaves >1-overload reduced-visibility impls reachable via a plain
  interface-typed receiver).
- **DEC-268 certification outcomes.** Both clusters passed the MAXIMAL ladder (fresh-context 3-lens panels,
  two consecutive clean rounds). The panels caught + fixed, before ship: a real SOUNDNESS hole (`internal`
  bypassable by upcasting to an interface — `E-IFACE-VIS` extended to `internal`), a set-visibility edge
  (`internal` + `protected(set)` now `E-SET-VIS-WIDER`), and doc-currency slips. Byte-identity
  (VM ≡ tree-walker ≡ php-8.5.8) held at every commit.
- **LSP fix (66f940b).** Dotted import-path completion carried only a `label`, so accepting `Core.Output`
  after typing `Core.` inserted `Core.Core.Output`; import items now carry a `textEdit` replacing the whole
  typed path. (Broader LSP intuitiveness punch-list: `docs/research/2026-07-25-lsp-completion-audit.md`.)

## DEC-336 — extensionless `#!`-shebang sources + perpetual editor/LSP currency (2026-07-24, RULED + BUILT)

- **Ruling (dev-directed, 100%-clear tooling slice).** A `#!…phg` first line makes an EXTENSIONLESS file a
  valid Phorj source: the lexer skips a leading `#!` shebang line, and `phg run` accepts a path with no
  `.phg` extension, so a `#!/usr/bin/env phg` script runs directly. Editors stay perpetually current
  (Inv 17 + DEC-181 reaffirmed): VS Code / PhpStorm associate `#!`-shebang + extensionless files via a
  `firstLine` rule, so a freshly-authored script is recognized without a rename.
- **Built (2026-07-24).** Shebang lexing + extensionless `phg run` (some pieces pre-existed, completed
  here); editor `firstLine` association; the always-current editor discipline (DEC-181) reaffirmed.
  Task-tracker line: "Build DEC-336 shebang/extensionless + editor currency."
- **Byte-identity.** Unaffected — a shebang line is lexer-skipped and never reaches any backend.
- **Cross-refs.** MASTER-PLAN §0 D10a build cluster; SLICE-STATE "✅ DEC-336 BUILT (2026-07-24)". (This
  row was reconstructed 2026-07-25: DEC-336 was BUILT and referenced in MASTER-PLAN + SLICE-STATE but had
  been omitted from this register — Invariant-19 SSOT repair.)

## DEC-337 — `#[Entry(kind:)]` kind is an injected `EntryKind` enum variant (2026-07-25, RULED + BUILT)

> ⊳ Import-gating direction superseded by DEC-353 (auto-provide injected symbols).

- **Problem.** `#[Entry(kind: Cli)]` (DEC-331) read `Cli`/`Web` as a BARE magic identifier — string-matched
  in `parse_entry_kind`, never imported, never resolved. This violated the "nothing in the wind" invariant
  the language enforces everywhere else (injected variants like `Option.Some` are `E-INJECTED-VARIANT-BARE`
  when bare). Flagged by the developer.
- **Ruling (interactive, all sub-forks dev-chosen).** The kind is an injected enum `Core.Runtime.EntryKind`
  { Cli, Web, Desktop, Mobile, Worker, Embedded }, reached QUALIFIED — `#[Entry(kind: EntryKind.Cli)]`.
  Sub-decisions: (1) **separate import** `import Core.Runtime.EntryKind;` (NOT bundled into
  `Core.Runtime.Entry`); (2) **reserved kinds are real variants** (Desktop/… resolve, then
  `E-ENTRY-KIND-RESERVED`, preserving the active/reserved/unknown tiers). Recommended option (mirror the
  `Option.Some`/`Result.Success` injected-variant precedent) chosen.
- **Enforcement.** Bare `kind: Cli` → `E-INJECTED-VARIANT-BARE`; qualified but unimported `EntryKind.Cli`
  → `E-UNIMPORTED`; wrong qualifier → `E-ENTRY-KIND-UNKNOWN`. Two accepted spellings, mirroring the
  `#[Entry]` attribute's own forms: short `EntryKind.Cli` (member-import-gated) and fully-qualified
  self-gating `Core.Runtime.EntryKind.Cli` (no import, like `#[Core.Runtime.Entry]`). Compiler-synthesized
  entries (test-runner driver, lifted drafts — zero span) are exempt from the import-gate.
- **Compile-time only (Inv 5).** `EntryKind` is a pure marker bare_type under `Core.Runtime` (empty prelude
  source) — never a runtime enum; the attribute arg is erased before any backend, so the PHP leg never sees
  it. Byte-identity (VM ≡ tree-walker ≡ php-8.5.8) unaffected — held across all 340 migrated examples.
- **Scope built.** Parser reader (`entry_kind_form` flattens the qualifier chain), checker enforcement,
  `Core.Runtime` prelude bare_type, synthetic `entry_attr` + lifter emit the qualified form + import;
  ~340 `.phg` examples/conformance/bench + ~815 inline `.rs`/playground fixtures migrated; the 3 shared
  test prepend-helpers inject the `EntryKind` import; new checker coverage for the three error paths.
  Full all-features gate green (nextest + clippy ×3 + fmt + release), DEC-268 panel certified.
- **Currency (Inv 17).** Transpile (erased), lift (emits qualified form + import), formatter (round-trips),
  LSP (checker≡diagnostics; `EntryKind` surfaces via CORE_MODULES bare_types), playground (the `playground/web`
  example corpus — `examples.js` regenerated from the migrated `examples/*.phg` via `gen_examples.py`, plus the
  hardcoded `main.js` fallback + `gen_examples.py` DEFAULT — since the wasm-compiled checker validates it too).
  LSP attribute-arg completion suggesting `EntryKind.Cli` inside `#[Entry(kind:` is a follow-up on the existing
  LSP punch-list.

## DEC-338 — nativize `Request.parse` to flip the `queryparse` 0.10× perf loss (2026-07-25, RULED + BUILT)

- **Problem.** `queryparse` is the worst perf loss in the suite: VM 2,532M ns vs php-8.5+JIT 245M = **0.10×**
  (dev-box microbench). 3-agent root-cause: `Request.parse` (`src/cli/http_request_prelude.rs:136-205`) is an
  INTERPRETED prelude orchestrator firing ~35 native calls + ~10 instance allocations per iteration on the
  boxed VM; split ≈ **50% VM dispatch / 35% object-graph allocation / 15% actual native parse** (the parse
  Rust `src/native/http/query.rs` is already ≈ PHP's C). PHP JIT-compiles its equivalent and does less work.
- **Ruling (dev via AskUserQuestion, 2026-07-25).** Nativize the orchestrator into ONE Rust native
  `Core.Native.Http.parseRequest(bytes) -> Request?` that head-splits, builds the header map, parses query,
  cookies, form/multipart, decodes the path, stashes the body, and hand-builds the whole `Request` instance
  graph — collapsing ~35 `CallNative` + ~200 bytecode ops into ONE dispatch. Est. **0.10× → ~0.8-1.5×**, flips
  even on the VM (no JIT/AOT dependency). **Inv-16 trade accepted:** a `__phorj_http_parse_request` PHP runtime
  helper mirrors it on the transpile leg (the established `__phorj_http_*` pattern), so parse logic lives in the
  Rust native + PHP helper rather than single-source phorj prelude — surfaced and dev-ruled per Inv 16.
- **AOT note.** AOT alone reaches only ~0.3× on queryparse (removes dispatch but cannot unbox the escaping
  `Rc` object graph) — NOT a substitute for the nativize. AOT is complementary (stack for the last increment).
- **Byte-identity.** VM ≡ tree-walker ≡ php-8.5.8 must hold across the whole Request surface. The 3-leg gate is
  the differential: `all_examples_transpile_and_match_php` (globs `examples/web/rich_request.phg`) + the
  `rich_request_wither_guards_fault_identically` and `rich_request_multipart_agrees_on_all_legs` tests. The
  native's own graph is additionally pinned by the fast oracle-independent unit tests
  `parse_request_builds_the_expected_bag_graph` / `_urlencoded_form_body` / `_null_on_malformed`
  (`src/native/http_tests.rs`). The build feasibility (hand-built `Value::Instance` graphs) is precedented by the
  multipart native (`src/native/http/multipart.rs:41-56`).
- **Campaign order.** queryparse (this) → Json-ADT JIT slice #33 (jsonround+deepjson) → assess listcontains.
- **Status.** RULED + BUILT (2026-07-25). Native `Core.Native.Http.parseRequest` in `src/native/http/request.rs`
  (Inv-13 split; the whole head-split → header/query/cookie/form/multipart → path-decode → body-stash →
  hand-built `Request` graph in ONE dispatch), PHP twin `__phorj_http_parse_request` in
  `src/transpile/runtime_php_http.rs` (self-contained — carries its own `__phorj_http_trim` since the nativized
  path no longer calls `String.trim`), prelude `Request.parse` now delegates (`return NativeHttp.parseRequest(raw)`),
  the four dead private helpers (`headerPairs`/`cookiePairs`/`multipartFields`/`boundaryOf`) removed. The stash
  contract is single-sourced (`stash_decision`, http.rs) across the whole body and each file part.
- **Build gate (Verified).** Full ALL-FEATURES gate green: `cargo test --workspace --all-features` (oracle
  `PHORJ_REQUIRE_PHP=1`, php-8.5.8) all pass incl. the differential `all_examples_transpile_and_match_php` +
  the two `rich_request` 3-leg tests + the three new `parse_request_*` native unit tests; clippy `--all-features`
  and `--no-default-features` clean; fmt clean; release built. **Perf direction (in-container, direction-only):** php-8.5.8 median ~1.725s vs phorj-VM
  median ~1.97s = **~0.88×** — up from 0.10× (~9× faster) into the predicted 0.8–1.5× band, but still <1.0×,
  i.e. NEAR-PARITY, NOT yet a WIN by the WIN-OR-FLAG bar; identical checksum `3200000` on all three legs. The
  exact WIN ratio (whether it crosses 1.0×) is the dev-box docker microbench harness's to certify (median-of-N,
  isolated) — this slice is BUILT + certified, but the campaign "flip" is NOT claimed complete until that lands.
- **Kept, not pruned (build decision).** The granular sub-natives `parseQuery`/`parseMultipart`/`decodePath`/
  `stashBody` lost their only phorj caller (the removed parse body) but are KEPT: their Rust kernels are reused
  by `parseRequest` and their `__phorj_http_*` PHP twins are called by `__phorj_http_parse_request`, so the rows
  stay symmetric with the live PHP helpers and remain the internal SPI for the queued slice-3 lazy path. Pruning
  the registry rows is out of this perf-flip slice's scope.
- **Certification.** DEC-268 MAXIMAL 3-lens fresh-context panel (correctness+regression / security+safety /
  completeness+blast-radius), evidence-based, across multiple rounds: the CODE was unanimously clean in EVERY
  pass (correctness, security incl. adversarial bounds/panic tracing, and completeness); ALL findings were
  doc-only and progressively resolved — round 1: the BUILD-READY→BUILT flip + the keep/prune decision; round 2:
  a coverage-phrasing overclaim (fixed by adding the `parse_request_*` unit tests + correcting the gate wording)
  + residual stale "queryparse — next"/historical mentions; round 3: a stale 2026-07-24 "RESUME HERE" block
  (annotated superseded) + a `✅ FLIPPED` status overclaim (0.88× is near-parity, still <1.0×, not a WIN by
  WIN-OR-FLAG — reworded to "BUILT; WIN pending dev-box" everywhere); round 5: one non-blocking P2 — the
  `docs/specs/2026-07-23-rich-request.md` perf paragraph still forward-referenced queryparse as a pending loss
  (annotated `✅ SUPERSEDED by DEC-338`). At the 5-round cap without two-consecutive-clean, the developer ruled
  (AskUserQuestion) KEEP PANELLING past the cap; each round's doc findings were fixed and re-panelled until two
  consecutive fully-clean rounds were reached before commit.

## DEC-339 … DEC-355 — GLOBAL REVIEW 2026-07-25: seventeen open adjudications (ALL RULED 2026-07-26 — builds queued; see rows)

> Header corrected 2026-07-28 (consistency audit): every row in this batch was RULED 2026-07-26
> (see each row's Status column); the earlier `ALL PENDING` header was the stale-label class
> DEC-362 now guards.

**Provenance.** The developer ran his own review pass, produced ~15 findings, and asked (2026-07-25) for
them to be verified against real code, widened into a global project review, and prepared as an agenda he
could rule on interactively — explicitly instructing that **no questions be asked and no decisions taken
while he slept**. This block is therefore the Invariant-15 record: every fork is PENDING with a
recommendation, none is ruled.

**Canonical homes (Invariant 19, no duplicated content):** the *decision identity + status* is this table;
the *analysis, minimal repro, per-option after-state and the why* live in
`docs/research/2026-07-25-completeness-register.md` §2 (agenda IDs `GR-1`…`GR-17`, which map 1:1 to
DEC-339…DEC-355 in order); the *raw evidence* lives in `docs/research/2026-07-25-global-review/`.
This also discharges the already-RULED **DV-5** pass (`docs/specs/2026-07-24-visibility-model.md`).

| DEC | GR | Question (one line) | Recommended (not ruled) | Status |
|---|---|---|---|---|
| DEC-339 | GR-1 | **P0** — shadowing a live outer local/param in ANY nested block mistranspiles (phorj has block scope, PHP has none): how to restore Invariant-1 byte-identity? | **RULED 2026-07-26 — REJECT redeclaration, do NOT alpha-rename.** A declaration is rejected if its name is already bound by a live **local or parameter** binding in the same function (same scope OR enclosing); class fields are never local bindings; a lambda starts a new function. Enforced in the **checker** (one chokepoint → all surfaces, Invariant 17), NOT the transpiler. The 2026-07-26 probing widened the blast radius from 6 recorded shapes to **10** (new: the `for…in` loop *variable*, `match` arm bindings, binding-`if`, `catch` bindings — one shape changes **control flow**). **Full 23-row accepted/rejected case list = `docs/specs/2026-07-26-block-scope-shadowing.md` (canonical).** The superseded alpha-rename recommendation is recorded there as rejected, with the reason: shadowing is ten declaration forms, so a renamer must be correct in ten places forever while the rule makes all ten unrepresentable | **BUILT 2026-07-29** [Verified 2026-07-30: `E-SHADOW-LOCAL` in `src/`, checker-tested] — the row had gone stale, saying "build queued" for shipped work |
| DEC-340 | GR-2 | **P1 data loss** — `db.transaction(fn)` auto-rollback pops only ONE savepoint level, so a leaked inner `begin()` leaves the transaction's OWN level open with writes a later `commit()` persists | **RULED 2026-07-26 — unwind to the ENTRY depth, NOT to depth 0.** "Restore the depth I found." The original *depth-0* recommendation is **REJECTED**: it would roll back a **caller-owned** outer transaction (`db.begin(); db.transaction(fn)` where fn throws), trading this bug for a worse one. Adds `rollbackAll()` (manual path) + `transactionDepth()` (depth is currently unobservable — the native returns it and the prelude discards it). **PHP leg: emit a `__phorj_*` savepoint helper** (Invariant 16) — the current emitter is a literal placeholder comment and `begin()` maps to non-nesting PDO `beginTransaction()`, so shipping it would be the silent downgrade Invariant 14 forbids. **GR-26/DEC-364 (`using`/`defer`) sequenced immediately after** as the structural fix. Reproduced live (bal 100 → reported-rolled-back → **999 persisted**). Full rule: `docs/specs/2026-07-26-transaction-depth-semantics.md` | **BUILT 2026-07-29** [Verified 2026-07-30: `rollbackAll` in `src/`, 3 test files] — the row had gone stale, saying "build queued" for shipped work |
| DEC-341 | GR-3 | TextMate grammar: `phorj.tmLanguage.json:34` `"begin": "\\b(b|r)?\""` — `\b` before an OPTIONAL group fails at opening quotes and matches closing ones, so **every plain string starts at its CLOSING quote**; 81/383 `.phg` files end inside an unterminated span, 188/266 examples scope code punctuation as string. This is the developer's "light blue like a comment" report; his `#`-as-comment guess was disproved | **RULED 2026-07-26 — (A): ship the pre-verified 5-rule string section (leakage 81/383 -> 0/383) PLUS a `vscode-textmate` pre-push gate.** The gate is not optional — without it the grammar silently rots again. Mechanical, pre-measured, no byte-identity surface; highest visible win per unit of effort on the agenda | **RULED — build queued** |
| DEC-342 | GR-4 | UFCS receiver completion is empty (`line.` -> 0 items) while `String.` over-suggests members that are not imported | **RULED 2026-07-26 — (A): add receiver completion AND import-gate both directions, one slice.** **Verified: the LANGUAGE rule already works** — `line.trim()` without `import Core.String` is a type error, with it returns `[hi]`; the gap is editor-side. Completion unions members of EVERY imported module whose first param accepts the receiver type, for **every** receiver (`string`/`List`/`Map`/`Set`/`bytes`/`int`/`float`/`decimal`/`Json`/`T?`/`Result`/user types), not just `string`. Adds the **"`trim` exists in `Core.String` — add `import Core.String;`"** diagnostic (today's *"type `string` has no method `trim`"* is misleading — the method exists) **plus an LSP quick-fix**, and fixes the span (anchors at `1:10` instead of the call site). **UFCS ambiguity across modules RULED: an ERROR naming both candidates + the qualified escape** — first-import-wins rejected as silent and order-dependent. **AMENDED 2026-07-26 (developer addition): WILDCARD-IMPORT COMPLETION.** Everything a wildcard import brings into scope must be completable **everywhere a completion is possible, not only after a `.`** — on a bare/empty line and on explicit **`Ctrl+Space`** the editor lists every wildcard-imported symbol and **filters live as the user types**; wildcard-imported free functions also surface as UFCS members on a matching receiver; ranking is prefix+substring; the list derives from the resolved import set (same catalog as the checker, never a second source of truth). Rationale: a wildcard that pulls in 40 symbols the editor cannot name is worse than no wildcard — a direct application of DEC-375. Full rule: `docs/specs/2026-07-26-ufcs-lsp-companion.md` | **RULED — build queued** |
| DEC-343 | GR-5 | Both `for (T x in xs)` and `foreach (xs as x)` work today; DEC-248 ruled `for...in` **retired** but `E-RETIRED-FORIN` was **never built** (0 hits in `src/`); census **87 `for...in` vs 8 `foreach...as`**; Conflict C-2 open since 06-25 | **RULED 2026-07-26 — (A): AMEND DEC-248 to "keep both", close Conflict C-2, add cross-form migration hints.** A ruling left unbuilt for a month while the corpus teaches the opposite 87:8 is evidence the RULING was wrong, not the corpus. Retiring `foreach...as` instead (8 sites) was rejected — it would discard the deliberate PHP-familiarity affordance. **DEC-248 is SUPERSEDED on this point** | **RULED — build queued** |
| DEC-344 | GR-6 | `main` is still forced into the entry signature **by name** (`type_bodies.rs:347`) despite DEC-331's attribute-declared entries — a library `function main(string s): string` with no `#[Entry]` is rejected, while `#[Entry] function startHere()` works | **RULED 2026-07-26 — (A): remove the name-based special case; entry-ness comes ONLY from `#[Entry]`.** Delete dead `E-MULTIPLE-MAIN` + its stale `phg explain` entry, and repurpose `class-main.phg` into a differential-gated regression test so the reservation cannot silently return. **Nothing is lost: multi-entry protection already exists and is LIVE — `E-DUPLICATE-ENTRY-KIND` (verified 2026-07-26: two `#[Entry(kind: EntryKind.Cli)]` -> *a program has at most one entry per kind*).** By design that is per-KIND, so one Cli + one Web in one program stays legal (DEC-331: `run`->Cli, `serve`->Web) | **RULED — build queued** |
| DEC-345 | GR-7 | `package` validators are skipped by the no-user-imports fast path (`loader/entry.rs:53-66`), so enforcement follows the import graph, not the file; plus bug **A6** — a CORRECT `src/App/Cmd/Runner.phg` + `package App.Cmd;` is rejected with a self-contradicting message because the entry root is always `entry_local` | **RULED 2026-07-26 — (A), IN THIS ORDER:** fix A6 first -> then run the validators on the fast path -> then fix the `validated (every file…)` message and give the loose-`Main` error a code. **Order is load-bearing:** closing the fast path before fixing A6 would start emitting the WRONG error for correct layouts. Hatch = **`#[Core.Runtime.FreePath]` written FULLY QUALIFIED above `package`** — verified 2026-07-26 that fully-qualified attributes already resolve with NO import (`#[Core.Runtime.Entry(kind: Core.Runtime.EntryKind.Cli)]` runs), so the *"an attribute before `package` cannot be imported"* dilemma dissolves with zero new machinery and nothing left in the wind. Named `FreePath` over `Loose` for precision (it means *this file's package need not match its path*, not *sloppy*); home is `Core.Runtime` beside `Entry`/`EntryKind`, no new module. `phorj.json` opt-out REJECTED — the loader never reads it and it contradicts DEC-282's no-manifest/no-marker rule. All surfaces funnel through `load_unified_src`, so run/check/transpile/build/test/**LSP** land together (Invariant 17 by construction) | **RULED — build queued** |
| DEC-346 | GR-8 | Execute the already-ruled DEC-326 UFCS promotion: 2223 qualified sites in examples, 1231 of them `Output.printLine` (55.4%) | **RULED 2026-07-26 — (A): tooling FIRST** (DEC-342 completion + import hint + formatter lint), **then** the 391 zero-judgement sites. **`Output.printLine` STAYS QUALIFIED — developer-ruled:** it is 55.4% of the corpus and the most-read line in every example, and UFCS reads well only when the receiver is the subject (`line.trim()`, `xs.map(…)`) — for output the SINK is the subject, so `"hello".printLine()` inverts it. No codemod touches it | **RULED — build queued** |
| DEC-347 | GR-9 | Every file API is whole-slurp — `readAll` costs 200 MB where `Input.lines()` streams an 88 MB file in 23.7 MB RSS, and `limits.rs` has no I/O or memory cap | **RULED 2026-07-26 — (A): `FileSystem.lines(path): Iterator<string>` over an offset-chunk native, NO file handle.** Zero new Value/type/transpile machinery, O(1) memory, identical user syntax, non-breaking later swap to a real handle; ladder **case 1** (`fgets` maps). **(B) a full `FileHandle` type REJECTED** — blocked by C4: no transpiling precedent for an opaque handle, `emit_type` would emit an unsatisfiable PHP class hint, and both sibling handles are already `E-TRANSPILE-*` quarantined. Sequenced AFTER DEC-364 (`using`) | **BUILT 2026-07-31** — see "DEC-347 BUILT" at the end of this file; memory claim verified, perf loss recorded OWED |
| DEC-348 | GR-10 | No filesystem locking at all; the presumed dependency blocker is FALSE — `std::fs::File::{lock, try_lock, unlock}` are stable on the pinned rustc and interoperate with PHP `flock()` (verified to block each other bidirectionally) | **RULED 2026-07-26 — (A): scoped `withLock(path, fn)` + `tryWithLock`, whole-file, advisory.** Release guaranteed by construction — no leak path; ladder case 1. Needs a `try/finally` PHP helper to preserve that guarantee, which is why it is sequenced AFTER DEC-364 (`using`). **(B) manual `lock`/`unlock` REJECTED** (leak-prone — the pattern every language regrets); **(C) byte-range/timeout REJECTED** (byte-range needs `fcntl`; a timeout would need a spin-sleep **bandaid**). **MUST BE DISCLOSED IN THE DOCS: Windows is a shipped target whose lock semantics may be MANDATORY rather than advisory, and there is no Windows CI — so any cross-platform guarantee is `[Unverified]` and must say so** | **BUILT 2026-07-31 (`withLock`)** — see "DEC-348 BUILT" at the end of this file; premise re-verified, cross-platform disclosed. **`tryWithLock` BUILT 2026-07-31 (DEC-348.1)** — return type ruled `Option<T>`; see "DEC-348.1 BUILT" at the end of this file |
| DEC-349 | GR-11 | A no-modification clone already works as `p with { }` (shallow, transpiles to bare `clone($p)`) but the **lifter refuses it** — a live Invariant-17 gap | **RULED 2026-07-26 — (A): bless + document the EXISTING form, add NO new syntax**; `lift` must refuse loudly only when `__clone` exists. A dedicated `p.clone()` was rejected — a second spelling for something that already works | **RULED — build queued** |
| DEC-350 | GR-12 | The type named `Database` is provably ONE connection (single `Box<dyn DriverConn>`, connection-scoped `tx_depth`/`hook`/`timeout_ms`, `grep pool` empty, pooling out of scope) | **RULED 2026-07-26 — (A): rename to `Core.Database.Connection` — the TYPE renames AND the `Module` suffix drops.** 8 of 10 ecosystems call this `Connection`; `Database`/`DB` is what Go and Laravel use for the pool/manager phorj does NOT have. DEC-278's `Module` suffix existed only because the module leaf and the type were namesakes, so renaming the type dissolves its rationale and `Core.Database` can go bare. Breaking rename across every DB example and doc — cheap now, expensive once users exist | **RULED — build queued** |
| DEC-351 | GR-13 | `Statement` binds append and never reset, so a bind-in-a-loop dies on iteration 2 (`2 bound value(s) but 1 ? placeholder(s)`); `bindNamed` silently last-wins and is ~75x slower at 8000 iters (4.469s vs 0.059s re-preparing) | **RULED 2026-07-26 — (A): reset binds after each `exec`/`query`, make positional and named behave identically, fix the quadratic path.** Honours DEC-208's stated reuse promise; cheap because the SQLite driver already uses `prepare_cached` and resets per execute — this is bind lifecycle, not a driver rewrite. **D5 folded in:** the nested-savepoint SQL is not MySQL-portable (bare `RELEASE id`, `;`-joined pair through single-statement `query_drop`) while the module's own `mysql.rs` uses the correct forms, with ZERO nested-savepoint coverage on MySQL or Postgres — fix + add coverage in the same slice | **BUILT 2026-07-30** (binds execution-scoped, 4.469s→0.054s measured; D5 single-sourced in `natives/savepoint.rs` + a portable-form ratchet) |
| DEC-352 | GR-14 | "Visibility/access in blocks inside a function" unbundled into 5 features: bare scoping blocks (already work — and were the DEC-339 P0), local functions, local classes, visibility on locals, visibility on local functions | **RULED 2026-07-26 after a cross-language scan.** **Local FUNCTIONS: YES, capture by value** (consistent with lambdas + DEC-357). PHP maps cleanly — emit a closure with `use(...)` when it captures, or a mangled top-level function when it does not; **never a bare nested `function`, which PHP makes GLOBAL**. **Local CLASSES: YES but NON-CAPTURING** (Rust/Swift semantics — a scoped TYPE, not a closure over locals); enclosing state is passed to the constructor, as PHP's anonymous class does. Decisive: `private` on a top-level type ALREADY means *this FILE only, not importable* (visibility matrix `2026-07-24-visibility-model.md:24`), so a local class adds only adjacency + capture — and capture is the entire cost (effectively-final analysis, DEC-357 interaction, boxing). **VISIBILITY on either: PERMANENTLY REJECTED with an explaining diagnostic** — not deferred but MEANINGLESS: a local has exactly one scope, so `private` has nothing to be private from. No surveyed language allows it; C# rejects it by name (`CS0106`). **Required diagnostic:** referencing an enclosing local inside a local class must say *"a local class does not capture; pass `x` to its constructor"*, never a bare unknown-identifier — Java programmers WILL expect capture | **RULED — build queued** |
| DEC-353 | GR-15 | The compiler-INJECTED `Core.Runtime.{Entry,EntryKind}` still require explicit imports, so a minimal runnable program is 6 lines with 4 of them ceremony (PHP: 2), and the error text itself calls `Entry` *"an injected `Core.Runtime` type"* | **RULED 2026-07-26 — (A): auto-provide the injected `Entry`/`EntryKind` symbols.** Requiring an explicit import for a COMPILER-INJECTED symbol is self-contradictory; removes 2 lines from every runnable file. Touches the `E-UNIMPORTED` / `E-INJECTED-VARIANT-BARE` machinery DEC-337 just built, so it is a real design change, not a tweak | **RULED — build queued** |
| DEC-354 | GR-16 | Approve the Claude-bundle import (14 of 199 files audited IN/OUT in `J-claude-bundle.md`) | **RULED 2026-07-26 — REFRAMED by the developer, narrower than the recommendation.** **Skills IN (phorj-useful subset only):** `converge` (it IS the DEC-268 ladder, currently hand-rolled from memory at every 3C/6C gate), `sweep`, `expanding-context`, `sleuth`, `inspect`, `cross-check`, `aggregate-findings`. Dropped as infra-shaped rather than phorj-shaped: `forge`, `qa-sweep`, `validate-infra`, `recent`. **Permissions: an ALLOW-LIST ONLY — no `deny`, no `ask`** (developer-ruled: in a remote container he has no terminal, so a `deny` blocks HIM too; he wants full execution autonomy). **Resolution of the risk that repo settings also load on his laptop: machine-level protections stay in his PERSONAL GLOBAL settings, which the repo never touches.** **Hooks: `precompact-handoff` ONLY** — the gate/Stop hooks are OUT, and for a stronger reason than friction: the framework claims the question guard is *"mechanically enforced"*, which is **FALSE here** (`AskUserQuestion` silently failed 4x this session), and a gate that cannot fire is worse than none. **session-remember: OUT** — its memory dir is wiped when the container is reclaimed, so it has zero durable value here; Invariant 19 already says only committed repo state survives, and this session proved `SLICE-STATE` + the register ARE the memory. **All 57 `mcp/**` files: OUT** — four corporate `.env` files plus desktop-automation drivers, and **`phorj` is a PUBLIC repo**. **BUILT 2026-07-27.** Landed: the 7 skills under `.claude/skills/` (adapted, each with a stated adaptation header — `converge`'s defaults ARE the DEC-268 tier; `sleuth` gained lens K for backend divergence; `sweep` gained the byte-identity/anti-bandaid/Op-triad/file-size dimensions; `cross-check`'s Jira mode DELETED; `aggregate-findings` retargeted off `~/.claude`); `scripts/claude-bootstrap/hooks/precompact-handoff.sh` + `log-helpers.sh` + a 14-assertion test suite, wired as a PreCompact hook on BOTH the `auto` and `manual` matchers (the developer compacts manually, so an `auto`-only matcher would have missed the real case); the allow-list-only `settings.json` (71 entries, no `deny`, no `ask`). **Two build-time discoveries worth keeping:** (a) Claude is CLASSIFIER-BLOCKED from writing `.claude/settings.json` at all, and the developer has no terminal in the container, so settings changes now travel through the repo as `settings.json.pending` + `apply-pending-settings.sh` (the script deletes the pending file so no duplicate persists); (b) the upstream hook called `claude -p` (Haiku) on EVERY compaction — rewritten deterministic (git + transcript via `jq`, zero LLM calls) because that call spends the same weekly quota the developer rations and fails whenever the API is unreachable; `PHORJ_HANDOFF_LLM=1` opts back in. Reports/handoffs live in gitignored `var/claude/**` — never `~/.claude`, which is wiped on reclaim | **BUILT 2026-07-27** |
| DEC-355 | GR-17 | Retire the `->` RETURN-TYPE spelling (`function id<T>(T x) -> T`) in favour of the ruled `: T` (Invariant 12): 87 `.phg` + 2068 `.rs` fixture occurrences across 90 files | **RULED 2026-07-26 — (A): scripted `.rs` fixture rewrite -> parser-reject -> un-ignore the dormant tests -> a grep gate blocking new `->`.** Key enabler nobody had recorded: **`phg format` ALREADY normalizes `->` to `:`** and pre-commit already runs `format --check`, so the `.phg` half is just a formatter sweep. **SCOPE CLARIFIED for the developer (he initially read this as the lambda arrow): the LAMBDA arrow is `=>` (`function(int x) => x * 2`) and is NOT touched — his one-line `=> x` form stays.** Not a naive sweep: some `->` are fn-type or prose arrows inside comments and must survive | **RULED — build queued** |

**Cross-cutting structural finding (not itself a decision).** Six of the items above share ONE root cause:
**a ruling was made, only partially built, and the docs were never reconciled** (DEC-248, DEC-326, DEC-331,
DEC-208, DEC-282, plus dead `E-MULTIPLE-MAIN`). Recommended systemic fix, folded into DEC-343/DEC-344
rather than a separate slice: **a mechanical gate asserting that every diagnostic code named in a register
row exists in `src/`, or the row is marked PARTIAL.** That single check would have caught both
`E-RETIRED-FORIN` (ruled, absent) and `E-MULTIPLE-MAIN` (explained, unreachable).

**Second corollary worth recording:** the differential harness's coverage **is** the example corpus
(`tests/differential.rs` globs `examples/**/*.phg`), so any feature without an example has **zero**
byte-identity coverage. That is precisely how DEC-339's P0 survived — block scoping has no example.

## DEC-356 … DEC-362 — GLOBAL REVIEW 2026-07-25, second batch: the global sweep (ALL RULED 2026-07-26 — builds queued; see rows)

> Header corrected 2026-07-28 (consistency audit): every row in this batch was RULED 2026-07-26;
> the earlier `ALL PENDING` header was the stale-label class DEC-362 now guards.

Same provenance and same canonical-home split as DEC-339…DEC-355 above; these arose from the three
*additional* sweeps (Rust source quality, docs consistency/Invariant-19, missing enforcement) rather than
from the developer's own 15 findings. Analysis in `docs/research/2026-07-25-completeness-register.md` §6
(agenda IDs `GR-18`…`GR-24`, mapped 1:1 in order).

| DEC | GR | Question (one line) | Recommended (not ruled) | Status |
|---|---|---|---|---|
| DEC-356 | GR-18 | Exhaustiveness is mechanical for `Op` (Invariant 3) but hand-rolled for `Expr`/`Stmt`/`Pattern`: **17 named catch-alls** (`other => other`, `leaf => leaf`) across 10 checker files silently pass a new variant through, `desugar_db.rs:67-69` *declares* the rewriter TOTAL and then closes with two of them, and `walk.rs:748`'s `_ => {}` sits one line under a comment recording that this exact bug already fired | **RULED 2026-07-26 — D **and** C as ONE slice; B is a separately-ruled follow-up.** Fix all 18 sites (17 checker + `walk.rs:748`) **and** land the probe-variant gate together: D alone decays (nothing stops catch-all #19), C alone ships a gate over 18 known-broken sites, and B (one shared total visitor) only becomes safe AFTER D, because explicit arms are what let the compiler enumerate the blast radius a visitor must preserve. `walk.rs:748` gets **named no-op arms, NOT `unreachable!()`** (those forms are reachable, they just bind nothing — a panic there would be factually wrong). **Invariant 3's wording is widened to name `Expr`/`Stmt`/`Pattern`** in the same change. Full rule: `docs/specs/2026-07-26-ast-exhaustiveness.md` | **BUILT 2026-07-30** (D + C + Invariant 3 widened; a VERIFIED compiler panic on valid user code was the headline find; CD-27 records the one exemption) |
| DEC-357 | GR-19 | Writing to a captured local inside a lambda is **silently lost** — `total = total + x` inside a `List.map` closure leaves `total=0` on all three legs with no error and no warning | **RULED 2026-07-26 — REJECT the write at check time**, hint naming the object-field pattern. NOT an Invariant-1 break (the legs agree); it is a dead assignment that reads as live. Narrow by design: **by-value capture is ALREADY the documented semantics** (`FEATURES.md:37`), so this enforces what is already stated. Boundary: reject assignment to the captured local ITSELF; mutating a captured **object's field** stays **LEGAL** — it is the reference-shared workaround the shipped `examples/database/transaction-closure.phg` depends on. **By-reference capture (`use (&$x)`) REJECTED as out of scope** — it would contradict documented by-value semantics and is a language redesign needing its own spec. A warning tier was rejected (a lost write is correctness, not style). Full rule: `docs/specs/2026-07-26-capture-write-rejection.md` | **RULED — build queued** |
| DEC-358 | GR-20 | Type mismatch, arity, unknown method, non-exhaustive match, **every** parse/lex error and **every** runtime fault carry `code == None`, so `phg explain` is unreachable for them — and all 9 `conformance/diagnostics/` cases assert a code, so the corpus is **blind** to the gap | **RULED 2026-07-26 — (A): a `code == None` ratchet with a shrinking allowlist**, mirroring the existing `explain_ratchet`. Makes the backlog CI-visible instead of invisible and shrinks over time, rather than one giant coding sprint with no mechanism preventing regression afterwards | **RULED — build queued** |
| DEC-359 | GR-21 | `10/0`, literal integer overflow and literal index-OOB all pass `phg check` — PHP parity where a win is free | **RULED 2026-07-26 — (A): reject all three at check time.** The DEC-058 principle (equal or better than PHP) applied to a free win. Constraint: literal index-OOB is rejected **only when statically provable** (the list literal is in scope) — the rule is not "reject all indexing" | **RULED — build queued** |
| DEC-360 | GR-22 | Unused **import** is a hard error while unused **local** is silent — inconsistent in both directions | **RULED 2026-07-26 — (A): move unused-import into the warning tier and add the `W-UNUSED-*` family.** **Register framing CORRECTED: a warning tier ALREADY EXISTS** — 12 `W-*` codes ship (`W-SQL-INJECTION`, `W-FORCE-UNWRAP`, `W-UNREACHABLE`, `W-MATCH-UNREACHABLE`, `W-CATCH-UNREACHABLE`, `W-DEPRECATED`, `W-REDUNDANT-CAST`, `W-SECRET`, `W-SHADOWED` = **package** shadowing not variables, `W-PHG-IN-DOCROOT`, `W-TRAIT-CTOR-*`), so unused-import is the odd one out rather than a missing tier. New codes: `W-UNUSED-IMPORT` (downgraded from hard error), `W-UNUSED-LOCAL`, `W-UNUSED-PARAM` (NOT for interface/override implementations — the signature is fixed), `W-UNUSED-FIELD`, `W-UNUSED-FUNCTION`, `W-UNUSED-TYPE-PARAM`, `W-UNUSED-CATCH-BINDING`, `W-REDUNDANT-MUTABLE` (declared `mutable`, never reassigned — teaches the immutable default). **Policy ruled: warnings never fail `run`/`check`; `--strict` promotes all warnings to errors and CI uses it** | **RULED — build queued** |
| DEC-361 | GR-23 | Two backends re-inline the canonical `FaultMsg` (**Invariant 4 breach**), `"non-exhaustive match at runtime"` has **already drifted** (the PHP leg throws `UnhandledMatchError()` with no message), and `differential.rs::classify` re-types all 12 fault bodies as its OWN literals so the drift is **invisible, not merely untested** | **RULED 2026-07-26 — (A): single-source the fault strings AND make `classify` DERIVE from those same consts.** Single-sourcing alone was rejected: it leaves `classify` an independent copy, so the test that should catch drift stays the thing hiding it | **BUILT 2026-07-30** (`src/value/faults.rs` + two ratchets; 38 re-inlined sites converted; the PHP-leg match drift fixed in BOTH lowerings) |
| DEC-362 | GR-24 | Documentation rot is the dominant defect class: 60+ dangling `src/` refs, 13 DEC ids with no register row, cursors pinning orphanable bare SHAs | **RULED 2026-07-26 — (A): three mechanical `pre-push` guards** — (1) a markdown reference-checker (every `file:line` / `src/…` path must exist), (2) one-row-per-DEC (every `DEC-nnn` mentioned anywhere has exactly one register row), (3) cursors record ref+subject, **never a bare SHA**. **Guard (2) is EXTENDED per this session's evidence: every diagnostic code named in a decision row must exist in `src/`** — that single check would have caught `E-RETIRED-FORIN`, the dead `E-MULTIPLE-MAIN`, and Invariant 14's phantom `--sequential-concurrency` flag, all three found this session | **RULED — build queued** |

**Records to CLOSE (verified fixed 2026-07-25, evidence in the register §6.3).** These are recorded as
fixed so the open-item lists naming them can be pruned: private/protected **static-field visibility**
(now `E-FIELD-VISIBILITY`); **static-method-via-instance** — the `G5` that
`docs/specs/2026-07-24-visibility-model.md` still lists as OPEN (now `E-STATIC-VIA-INSTANCE`, whole
static/instance matrix closed); **package-decl casing on CLI paths** (`E-PKG-CASE` fires);
`E-ALIAS-CYCLE` uncoded + unused-cycle-passes (both halves); `E-OVERLOAD-SELECT-CONFLICT` (entry removed);
and all 9 findings of the earlier same-day plans-divergence audit.

**Two NEW correctness records worth their own tracking (analysis in §6.2):** a **second exception to
Invariant 1** (self-referential property hook diverges `run` vs `run --tree-walker` — line 9 vs 17, 4099 vs
4 trace lines, invisible to `agree_err`'s body-substring matching), and **Invariant 17 currently
unsatisfiable** for `p with { y = 9 }` (runs + transpiles, but `phg lift` fails on the transpiler's own
output, and lift has no `E-TRANSPILE-*`-style escape hatch).

## DEC-363 / DEC-364 — GLOBAL REVIEW 2026-07-25, third batch: the on-hold inventory (BOTH RULED 2026-07-26 — builds queued; see rows)

> Header corrected 2026-07-28 (consistency audit): both rows were RULED 2026-07-26; the earlier
> `PENDING` header was the stale-label class DEC-362 now guards.

From the deduplicated on-hold sweep (95 items) — the two that are genuinely new adjudications rather than
restatements of DEC-339…DEC-362. Analysis: `docs/research/2026-07-25-completeness-register.md` §7.

| DEC | GR | Question (one line) | Recommended (not ruled) | Status |
|---|---|---|---|---|
| DEC-363 | GR-25 | **P1 SECURITY** — the Response-side outbound sink has **no CRLF guard**: `withHeader`/`withCookie` interpolate unvalidated into CRLF-joined header lines and `respond_once` returns handler bytes verbatim ⇒ HTTP **response splitting AND a request-smuggling/desync shape**, reproduced live on a shipped `phg serve` | **RULED 2026-07-26 — guard in the phorj PRELUDE, panic-class fault**, at `Response.withHeader` (name + value) and the **`Cookie` constructor** (the single chokepoint: every builder re-constructs; 3 of its 6 fields are injectable strings). Rejects **CR/LF/NUL** in values and **`:`** in names, mirroring the request-side gate. Prelude ⇒ all three legs identical **by construction**; a Rust `respond_once` guard was **REJECTED** (`phg build --php` never runs it ⇒ PHP leg stays exploitable). Panic-class over checked throw settled by evidence: `handlers.rs:143,186-188` degrades a handler fault to **a 500 on that request, never a panic** ⇒ no DoS vector, and no `throws` ripple into every handler. Also ruled: **NUL added to the REQUEST side too** (it rejects CR/LF only; PHP's `header()` rejects NUL), and **`Http.isValidHeaderName`/`isValidHeaderValue`** ship so a handler can return a clean 400 for user-derived input. Full rule: `docs/specs/2026-07-26-response-header-injection-guard.md` | **BUILT 2026-07-29** [Verified 2026-07-30: `isValidHeaderName` in `src/`, differential-tested (`tests/differential.rs:2174`)] — the row had gone stale, saying "build queued" for shipped work |
| DEC-364 | GR-26 | Finish the `using`/`defer` scope-guard surface already ruled by DEC-203 (`using` + `Closable`) — unbuilt, while every open slice hand-rolls `try/finally` around it | **RULED 2026-07-26 — (A): build `using` NOW, sequenced BEFORE DEC-347 (streaming reads) and DEC-348 (locking)**, so those land on a real release guarantee instead of hand-rolled `try/finally`. **`defer` re-examined per DEC-371** (its "no PHP analog" reason was struck) and **still REJECTED — on its real merits**: LIFO ordering plus capture timing is a genuine footgun, and `using` covers the same need with block-scoped clarity. One mechanism beats two | **BUILT 2026-07-31** — see "DEC-364 BUILT" at the end of this file (three legs byte-identical; two pre-existing bugs found and fixed; lift deferred with its reason) |

**Inventory headline (not a decision, but it changes how to run the agenda):** **40 stale status labels**
were found — **26 items recorded OPEN that are actually BUILT** (incl. tuples DEC-288, backed enums,
DEC-312 `lift_from`, DEC-223 Mail, DEC-257 `Iterator`, DEC-313 FS, the `db.transaction` closure + retry
surface, and P-Q-A-5's file-size debt now that the size gate reports `fails=0`) and **14 recorded DONE that
are NOT** (incl. DEC-331 D2/D3/D5/D6/D7 marked "LOCKED" but unbuilt, DEC-247 DateTime, and the wildcard
spec header that says "NOT YET BUILT" beside its own "✅ DONE"). **Recommendation: flip these 40 labels in
one mechanical pass BEFORE working the agenda** — it needs no rulings and every later decision then rests
on trustworthy inputs. DEC-362/GR-24's one-row-per-DEC guard is what stops it recurring.

**Also recorded:** three deferrals whose stated rationale no longer holds (most notably file locking, whose
presumed dependency blocker was never actually met, and the slurp-only file APIs, deferred before the
measurement showing whole-file reads cost 200 MB), and **the pinned dev-box microbench remains owed —
only the developer can run it**, and it decides whether the perf-flip campaign has 3 losses left or 1.

## DEC-365 — pre-push microbench gate is unpassable in a remote container (2026-07-26, **RULED** — build queued; see row)

> Header corrected 2026-07-28 (consistency audit): the row was RULED 2026-07-26 (SKIP-LOUD +
> NO-HIDDEN-LOSS); the earlier `PENDING` header was the stale-label class DEC-362 now guards.

| DEC | GR | Question | Recommended (not ruled) | Status |
|---|---|---|---|---|
| DEC-365 | GR-27 | The `pre-push` `microbench-gate` FAILS in the remote container on a **docs-only** series (`floatloop` WIN->LOSS) because the kernel discards the cpuset this absolute-ratio gate depends on — the whole near-parity cluster drifted in lockstep, which a real regression would not do | **RULED 2026-07-26 — (A) SKIP-LOUD on a discarded cpuset, with the developer's NO-HIDDEN-LOSS amendment: SKIP-LOUD means "UNMEASURABLE HERE, verdict OWED", NEVER "passed".** An unmeasurable or failing bench is **recorded as an open owed item** until a valid measurement clears it; it is never dropped, never re-baselined via `--emit`, and the gate must not report green for it. If a valid re-measurement shows a real loss, **the loss gets fixed — refactor or implement the win — never suppressed** (developer standing rule 2026-07-26). Two verdicts currently OWED under this rule: **`floatloop`** (WIN->LOSS on a discarded-cpuset run) and **`queryparse`** (0.146 here vs DEC-338's recorded ~0.88x, a ~6x disagreement, so DEC-338's near-parity claim stays **UN-CERTIFIED**) — both need a dev-box run | **RULED — build queued** |

**Collateral finding (perf certification, analysis in the completeness register §8.3).** The same harness run
reports **`queryparse` ratio = 0.146 (loss)**. DEC-338 recorded ~**0.88×** ("near-parity, NOT yet a WIN")
from an in-container direction-only measurement and deferred the canonical number to the dev-box docker
microbench. This IS a docker microbench and it disagrees by ~6×, far outside any noise band. Either the
harness micro and DEC-338's ad-hoc program are different workloads, or the 0.88× was optimistic — not
separable here. **DEC-338's near-parity claim is therefore NOT corroborated by the canonical harness and its
WIN stays un-certified**; the owed dev-box run is now not merely owed but actively contradicted.

## DEC-366 / DEC-367 — two adjacent Invariant breaches found while probing DEC-339 (2026-07-26, **BOTH RULED**)

> Header corrected 2026-07-26: DEC-366 rides in the DEC-339 slice (build order 1.1) and DEC-367 is ruled (A). The
> earlier `PENDING` header was itself the stale-label class DEC-362 now guards.

**Provenance.** Found while enumerating the DEC-339 case list on all three legs; both are independent of
the shadowing rule and are recorded separately rather than folded into it (Invariant 19: one canonical
home each). Analysis lives in `docs/specs/2026-07-26-block-scope-shadowing.md` §"Adjacent bugs".

| DEC | Question | Recommended (not ruled) | Status |
|---|---|---|---|
| DEC-366 | **Live Invariant-17 gap** — `phg lift` emits **non-compiling** phorj for ordinary function-scoped PHP: a `$b` first assigned inside an `if` and read after it lifts to `mutable var b = 5;` *inside* the block plus `b = 7;` outside ⇒ `E-ASSIGN-UNKNOWN` + `E-UNKNOWN-IDENT` on the lifted draft. Same PHP-function-scope-vs-phorj-block-scope insight as DEC-339, from the inverse direction | **Hoist** the declaration to the outermost use when a lifted PHP variable is assigned in a nested block and read outside it. Whether this rides along in the DEC-339 slice or gets its own is the developer's call | **PENDING** |
| DEC-367 | **Invariant-1 breach** — a phorj class `implements Error` that defines `getMessage()` transpiles to a class extending `Exception` and overriding **`final Exception::getMessage()`** ⇒ PHP `Fatal error` at runtime while both Rust backends run fine (verified, php-8.5.8) | **RULED 2026-07-26 — (A): extend the existing builtin-collision guard (`src/checker/common.rs:432`) to the FINAL METHODS of the mapped PHP parent**, rejecting at check time with a named code instead of dying at PHP runtime. **(B) renaming on emission REJECTED** — it would keep the program running while silently diverging from what the user wrote, and would break anything catching it as a PHP `Exception` | **BUILT 2026-07-29** [Verified 2026-07-30: `E-FINAL-PARENT-METHOD` in `src/`, checker-tested] — the row had gone stale, saying "build queued" for shipped work |

## DEC-368 / DEC-369 / DEC-370 — capture companion, terminology correction, real parallelism (2026-07-26)

| DEC | Question | Ruling / recommendation | Status |
|---|---|---|---|
| DEC-368 | The DEC-357 capture-write rejection needs somewhere to point — what carries genuinely-shared mutable state? | **RULED 2026-07-26 — a prelude `Mutable<T>`** (`import Core.Mutable;`, `new Mutable(v)`/`get()`/`set(v)`, nothing else). Named from **phorj's own vocabulary** (`mutable int n` already taught) so there is nothing new to learn and nothing to unlearn: `mutable` = the binding may be reassigned, `Mutable<T>` = the contents may change and be shared. **`Ref<T>` REJECTED** — PHP's "reference" *aliases a variable* while this **owns** its value, so `new Ref(total)` copies and `r.set(9)` silently leaves `total` untouched, which the checker **cannot** catch. `Cell<T>` (Rust jargon) and `Slot<T>` (unfamiliar) also rejected. **What reframed it: `List.reduce` already exists** (`list_registry.rs:230-245`), so most mutable-capture uses are a missing-fold smell and the real deliverable is the **diagnostic's routing** (accumulation → `reduce`/`sumBy`/`count`; genuine state → `Mutable<T>`). Full rule: `docs/specs/2026-07-26-capture-write-rejection.md` §Companion | **RULED — build queued** |
| DEC-369 | **The shipped `green` feature is mislabelled "concurrency" throughout the project** (developer-corrected 2026-07-26: *"they are not parallel or concurrent, they are sequential"*). **Evidence for the correction:** `src/green/sched.rs:25-32`'s trap set is `Yield` / `Recv(chan)` / `Join(target)` / `Done` — there is **NO I/O trap**, so a task doing file or socket I/O blocks the single OS thread and every other task waits. Combined with the already-documented `Rc`-heap `!Send` (no parallelism), that means **no parallelism AND no I/O overlap ⇒ zero throughput benefit**; the only benefit is expressiveness. "Concurrency" therefore oversells it to any reader who expects overlap. Scope: **194** `concurren*` hits across docs+code (excl. `target/`, `docs/research/`), `src/green/mod.rs:1` ("uncolored cooperative concurrency"), the internal `uses_concurrency()` API, and **`CLAUDE.md` Invariant 14 names a `--sequential-concurrency` flag that DOES NOT EXIST in `src/`** (doc rot, DEC-362's class) | **Recommended (not yet ruled):** user-facing noun becomes **"cooperative tasks"** (matches the `spawn` + channels surface); "coroutine" stays for the *mechanism* (`corosensei`, stackful); **the words "concurrent" and "parallel" are RESERVED for the real thing** in DEC-370. Rename `uses_concurrency` → `uses_tasks`; delete the nonexistent flag from Invariant 14 | **RULED 2026-07-26 — user-facing noun = "cooperative tasks"; "coroutine" = the mechanism only; "concurrent"/"parallel" RESERVED for DEC-370.** Rename `uses_concurrency` -> `uses_tasks`, sweep the 194 hits, delete the nonexistent `--sequential-concurrency` flag from Invariant 14. "Fibers" considered and rejected — PHP `Fiber` is explicit suspend/resume while phorj is `spawn`+channels, so it would set the wrong API mental model | **RULED — build queued** |
| DEC-370 | **Developer request 2026-07-26: a REAL parallel/concurrent form in phorj.** Today's `green` is cooperative-sequential (DEC-369). **PHP IS NOT A CONSTRAINT HERE** — DEC-005 doctrine (never delegate a capability to PHP), DEC-058 (*this language should be equal or better than PHP*), and the **already-paved road of DEC-133** (`E-CONCURRENCY-NO-PHP` exists and works, `src/transpile/expr.rs:548`) mean a native-only feature behind a transpile hard error is the NORMAL, ruled pattern — not an obstacle. The real constraint is runtime architecture: the `Value` heap is `Rc`-based hence `!Send` | **Recommended: (2) isolated tasks + copying channels as the target architecture, (4) data-parallel stdlib combinators as the FIRST slice.** (2) keeps `Rc` and the JIT untouched (each task owns its heap; values copy at the channel boundary), reuses the already-backend-agnostic single-sourced scheduler kernel, and barely changes the `spawn`+channels surface. **(1) `Rc`->`Arc` shared memory REJECTED** — an atomic refcount on every value clone taxes the JIT hot path the whole perf campaign rests on, and it forces either a GIL (pointless) or a Rust-style `Send`/`Sync` borrow discipline (enormous surface). (3) worker processes = a deployment shape, not the general model. Owed measurement: copy-at-boundary cost, and per-thread instantiability of interpreter/VM state | **RULED 2026-07-26 — (2) isolated tasks + copying channels as the TARGET architecture, (4) data-parallel stdlib combinators as the FIRST slice.** `E-TRANSPILE-PARALLEL-NO-PHP` follows DEC-133's precedent. (1) `Rc`->`Arc` and (3) worker-processes rejected as above. Owed measurement before build: copy-at-boundary cost + per-thread instantiability of interpreter/VM state | **RULED — build queued** |

## DEC-371 — RATIONALE DECONTAMINATION: decisions justified by "PHP doesn't have it" (2026-07-26, **RULED** — cleanup slice queued; see row)

> Header corrected 2026-07-28 (consistency audit): the row was RULED 2026-07-26 (the follow-through
> approved as its own cleanup slice); the earlier `PENDING` header was the stale-label class DEC-362
> now guards.

**Provenance.** The developer challenged (2026-07-26) whether any decision had been taken on the false premise
that PHP lacking a feature is a reason not to build it, and asked for every such artifact to be reopened.

**Audit result — the doctrine is SOUND and was applied consistently.** The rule the developer states is already
recorded verbatim: **DEC-005** ("Transpile is a bridge, not a runtime … never delegate a capability to PHP" —
PHP-only implementations proposed by Claude twice, rejected both times), **DEC-058** (the developer explicitly
rejected "stay PHP-aligned / don't add it" with *"this language should be equal or better than PHP"*, and method
overloading shipped), **DEC-097** (PHP `'…'` rejected, Java-style text blocks adopted instead), **DEC-151**
(crypto implemented natively, PHP delegation rejected), and **DEC-133** — concurrency built **native-only**
behind `E-CONCURRENCY-NO-PHP`, which **exists in `src/transpile/expr.rs:548`**. That is the paved road: the
vision is not at risk, and a PHP-impossible feature has already shipped once.

**The contamination is narrow — 4 artifacts, rationale-only except the first:**

| # | Artifact | What it says | Verdict |
|---|---|---|---|
| 1 | **DEC-037** (`C-decisions.md:60`) | selective type import, **"no wildcard (PHP has no `use A\*`)"** | **The false premise produced a WRONG decision that had to be reversed** — wildcard imports were later built and certified (`docs/specs/2026-07-24-wildcard-imports.md`; register §1 finding #5 confirms `*`, `* except {}`, group + aliasing all work). The row still states the PHP reason with no supersession note. **Fix: mark superseded, name the successor.** |
| 2 | **DEC-203** (`C-decisions.md:398`) | Go-style `defer` rejected as *"new footgun surface with no PHP analog"* | The independent reason (LIFO order + capture timing) stands alone; **"no PHP analog" must be struck**. **Live and relevant** — DEC-364/GR-26 is about finishing `using`/`defer`, so `defer` deserves re-examination on its own merits there. |
| 3 | **`KNOWN_ISSUES.md:1567`** | `this.field` mandatory, justified *"PHP-faithful — PHP has no bare field access"* | **Decision is RIGHT on independent grounds** — it kills the refactor footgun where adding a local silently rebinds a field reference, and it is exactly what makes DEC-339 row 22 (field-named locals) safe. Only the *stated* rationale leads with PHP-faithfulness. **Fix: reorder the rationale, keep the rule.** |
| 4 | **DEC-370 as first drafted** | led with *"PHP has no faithful shared-memory threading … so the PHP leg cannot follow"* | **Claude's own phrasing sin, self-reported.** It read as PHP gating the feature when the real constraint is the `Rc`/`!Send` heap. **Fixed in the same commit that added this row.** |

**Recommended follow-through (not yet ruled):** strike PHP-absence from all four rationales; mark DEC-037
superseded; re-open `defer` as a live option inside DEC-364; and add a standing rule — **PHP's lack of a feature
is never a reason against building it; the only PHP-shaped question is which Invariant-14 ladder case the
transpile leg takes.** That sentence belongs in `CLAUDE.md` next to Invariant 16, which already says
byte-identity is not the priority ordering.

**RULED 2026-07-26 — the follow-through is APPROVED as its own cleanup slice:** strike PHP-absence from all four
rationales, mark DEC-037 superseded (naming the wildcard successor), re-open `defer` as a live option inside
DEC-364, and add the standing rule to `CLAUDE.md` beside Invariant 16.

## DEC-372 — top-level statements stay rejected (2026-07-26, **RULED**)

**Q (developer, 2026-07-26):** allow a traditional script-style call — `function myFunction(…)` declared and then
`myFunction(…)` called at file scope?

**Today:** a parse error — *"expected a top-level item (import, function, enum, class, interface, or type)"*
(verified 2026-07-26).

**RULED — keep rejecting.** Three reasons: (1) **PHP runs top-level code at include time**, so a library file
with top-level statements would execute when imported — PHP's own worst footgun and an Invariant-1 hazard on the
transpile leg; (2) it reintroduces an **implicit magic entry**, exactly what DEC-331 removed and the opposite of
the developer's stated "almost everything with attributes, no magic/reserved names"; (3) it creates
ordering/hoisting questions between statements and declarations.

**Recorded alternative, NOT ruled but available on request:** allow top-level statements only in a file marked
`#[Core.Runtime.Script]` with no `#[Entry]` — explicit, attribute-declared, no magic, giving quick-script
ergonomics beside the existing shebang support (DEC-282/336).

## DEC-373 / DEC-374 — `&$var` support: lift + interop (2026-07-26, **RULED**)

**Provenance.** The developer ruled `Mutable<T>` + `.value` as the ONE by-reference form (rejecting a `ref x`
operator as ambiguous) and then required PHP `&$var` support. Three readings were separated and evidence taken;
the developer confirmed all three verdicts.

| DEC | Gap | Ruling |
|---|---|---|
| DEC-373 | **`phg lift` cannot read `&$param` AT ALL** — verified 2026-07-26: `lift parse error: expected parameter name, found Amp`. Any real-world PHP file with a by-reference parameter is **unliftable** | **RULED — fix now.** `function f(&$x)` lifts to `function f(Mutable<T> x)`, with call sites rewritten to pass a box. Invariant-17 currency, and a hard blocker for lifting real code |
| DEC-374 | **No by-ref param syntax for interop** — verified 2026-07-26: nothing in the parser/checker can express one, so `preg_match($re, $s, &$matches)` and every out-param idiom in PHP's stdlib is **uncallable from phorj** | **RULED — build now.** A `Mutable<T>` param on a `declare function` maps to a **by-ref argument** at the emitted call site: `declare function preg_match(string re, string subject, Mutable<List<string>> matches): int;` ⇒ `preg_match('/…/', $s, $m_ref)`. This is what makes `Mutable<T>` the single unified by-reference notion across phorj code, lifting, and interop |

**Rejected in the same ruling:** emitting `&$param` for **phorj-owned** `Mutable<T>` params. Two PHP shapes for
one phorj value is the **DEC-329.3 bug class** (same value, different PHP shape ⇒ byte-identity regression), and
PHP references cannot be a list element, an object field, or a return type, so they cannot express the type at
all. `&` therefore appears **only** where a foreign callee demands it — which is precisely why it creates no
second representation. Revisit only if a measured bench shows the object box losing (developer's no-hidden-loss
rule, DEC-365).

## DEC-375 — THE LSP/EDITORS ARE THE EXPERT COMPANION (standing bar, developer-ruled 2026-07-26)

> **The LSP and the editors must be flawless and fluent.** Complete and suggest wherever a completion is
> possible; propose the imports a completion requires; surface diagnostics that name the fix rather than
> merely reporting a failure. Anything an expert user would know, the editor offers.

**This is a quality bar on every editor-facing slice, not a feature.** It composes with Invariant 17
(`phg check` = LSP diagnostics, DEC-252) and DEC-181 (both editors updated in the same change), and it is
the standard DEC-342's completion/quick-fix work is measured against.

Practical consequences already ruled under it: import-aware UFCS receiver completion for **every** receiver
type; import-gated module completion; an "exists in `Core.X` — add the import" diagnostic **with a quick-fix
code action**; call-site-accurate spans. Anything shipped editor-side that reports a problem without
offering the fix does not meet this bar.

## DEC-376 — foreign PHP file-return interop (`$c = require 'config.php'`) (2026-07-26, **RULED**)

**Q (developer, 2026-07-26):** PHP files can `return` a value at top level and be consumed as
`$c = require 'config.php'` — Symfony's `index.php` and every Symfony config file do this. Support it?

**RULED: (A) NOT in phorj code + (C) YES for interop, both now.**

**(A) Phorj code has no file-level `return`.** That idiom exists *because PHP has no module system* — it is
the workaround, not the feature. Phorj has packages and imports, so the same job is done strictly better:
typed, statically checked, no `require` vs `require_once` distinction, no include-ordering surprises, no
"did the file run twice" question. It also keeps **DEC-372** intact (top-level statements stay rejected, and
a top-level `return` is one).

```phg
// config/app.phg
package Config.App;
function settings(): Map<string, string> { return ["env": "prod", "debug": "0"]; }
```
Both consumption forms already work (**verified 2026-07-26 against a real 2-file project**):
```phg
import Config.App;            Output.printLine(App.settings());   // qualified
import Config.App.settings;   Output.printLine(settings());       // leaf import, bare call
```
Selective leaf import applies to **functions**, not only types — no ruling needed, it ships today.

**(C) Foreign PHP file-return IS supported, for interop, now.** A `declare`d foreign PHP file whose return
value is typed, consumable from phorj, **PHP-target-only** — the Rust backends refuse it with the existing
`E-FOREIGN-RUNTIME`, exactly as `examples/interop/` already works. This is the honest migration path for
reading an existing Symfony-style config file, and it does **not** reopen DEC-372, because a `declare`d
foreign file is not phorj code with top-level statements.

**(B) rejected:** supporting it natively in phorj code would reintroduce runtime file evaluation,
include-ordering and caching semantics, and reopen DEC-372.

## DEC-377 / DEC-378 — the `__phorj_` helper rule, and the commit-cost lesson (2026-07-26, **RULED**)

| DEC | Ruling |
|---|---|
| DEC-377 | **A `__phorj_*` helper may exist ONLY when PHP cannot do natively what phorj does** (developer rule). Sharpened into a 3-bucket test because **168 distinct helpers exist today** and they are not one kind: **(1) semantic necessity** — PHP genuinely differs (`__phorj_checked_*`: PHP overflows int→float; `__phorj_dec_*`: bcmath decimal; `__phorj_float_to_int_exact`; `__phorj_class_name` per DEC-329.3) ⇒ **justified**; **(2) no single-expression equivalent** — PHP has the pieces but not with matching semantics/short-circuit/evaluation order (`__phorj_all`, `__phorj_any`, `__phorj_find`, `__phorj_drop_while`) ⇒ **justified but the reason must be STATED per helper**; **(3) convenience/DRY only** (`__phorj_format`, `__phorj_debug_*` — suspected, unaudited) ⇒ **must be INLINED**. **An audit classifying all 168 is OWED** — nobody currently knows which bucket each is in, which is the same unverified-claims pattern this whole agenda has been fixing. **First catch, immediately:** this session's own spec draft wrongly emitted `final class __phorj_Mutable`; verified that prelude phorj classes transpile UNPREFIXED (`final class Regex`, `final class RegexMatch`), so `Mutable` needs **no helper at all** — corrected in `docs/specs/2026-07-26-capture-write-rejection.md` |
| DEC-378 | **A docs-only fast path in `pre-commit`, plus a no-concurrent-commits rule.** Evidence from this session: the hook tier costs ~3-4 minutes, twelve docs-only ruling commits spent roughly 45 minutes waiting, and **the ONLY test failure of the session was caused by backgrounding one commit and starting another while its hook still ran** — two `cargo test` runs racing on the same `target/` and on the `phg` binary the cli tests spawn (`--test cli` failed under the race, then passed 29/0 in isolation). **Ruling:** if the staged diff touches no `*.rs`, `Cargo.toml`, `Cargo.lock` or `*.phg`, `pre-commit` skips the Rust test tier and runs only the markdown/reference checks; the full tier stays for anything that could break the build. Plus a one-line `CLAUDE.md` rule: **never run two commits concurrently** — the hooks share `target/` |

## DEC-379 … DEC-386 — the on-hold tail (2026-07-26, developer-ruled in the same session)

Source: `docs/research/2026-07-25-global-review/L-*.md`, the 95-item on-hold inventory. 32 rows were
`decision-needed`; L-01…L-15 were the agenda just ruled, leaving these.

| DEC | Item | Ruling |
|---|---|---|
| DEC-379 | **L-17 / P-Q-B-1 — visibility BYPASS.** The `overloads == 1` guard on `E-IFACE-VIS` lets a method with >1 overload narrow its visibility (even to `private`) and still be reached through a plain interface-typed receiver. Reproduces with `private`; `KNOWN_ISSUES F-032` | **RULED — close the hole: drop the `overloads == 1` guard and check EVERY overload's declared visibility.** It is a soundness bypass, not a stylistic deferral. A handful of negative conformance tests re-baseline | **BUILT 2026-07-30** (per-overload visibility; keyed on the CONFORMING overload — the strict "every overload" reading would have broken a shipped positive test, see the build note) |
| DEC-380 | **L-21 / DEC-286 — `jsonround` is 0.29× structurally.** VM 507ms vs C-`json` 145ms; two byte-identical levers bought ~3% because **~65% of ~20 allocs/iter is the `Rc<EnumVal>` box itself**, one per node | **RULED — OPTION B: CHASE THE WIN.** Developer-ruled 2026-07-26: *"go for Option B and chase the win, even if you have to propose a bold approach or revise an invariant that is blocking us; all must be researched/brainstormed and questioned correctly and with no compromise."* **Research shape, in order:** (1) name the constraint that actually blocks an arena — candidates are Invariant 3 (a new `Value` variant must extend every wildcard-free exhaustive match) and Invariant 1 (byte-identity); neither FORBIDS an arena, they price it, and the price must be MEASURED not assumed; (2) re-examine the prior no-win verdict — it came from a bounded prototype that *"did NOT build the full `Value::JsonArena`"*, so the +60% `deepjson` regression is a **proxy** result and a weak basis for a permanent no; (3) cost the bold options: a real `Value::JsonArena` variant, lazy materialisation with copy-out-on-extract, and **dropping the per-node `Rc` for an index-based arena handle** — which is where the 65% actually lives; (4) WIN-OR-FLAG **and** the no-hidden-loss rule both apply — a residual loss is recorded with anatomy, never suppressed. **DEC-286 is superseded by this row** | **RULED — RESEARCH SLICE queued** |
| DEC-381 | **L-23 / DEC-322 — duplicate record.** DEC-322 held unadjudicated real-parallelism forks; DEC-370 ruled the subject on 2026-07-26 | **DEC-322 is SUPERSEDED BY DEC-370** (Invariant 19, one canonical home). Bookkeeping only — no design change. Recorded so a future session does not re-open a settled question | **CLOSED** |
| DEC-382 | **L-24 / W4-10 — XML / DOM / XPath.** Zero hits for `XmlDocument`/`DomDocument` across `src/`, `examples/`, `docs/specs/`. Named a **top remaining FN blocker** with streams/intl/SPL-heaps, and FN is the 40%-weighted drag at ≈49% | **RULED — (B): admit a vetted crate (`quick-xml`-class) as the 15th dependency.** Best parity-per-effort item left. Hand-rolling was rejected: XML is a large, security-sensitive surface (entity expansion, namespaces, XPath) that should not be maintained in-house. **The `Cargo.toml` + UNIFIED-SPEC dependency-policy row must be updated in the same change** (the policy section is the SSOT) | **RULED — research + build queued** |
| DEC-383 | **L-84 — three coupled lifetime forks, PENDING since 2026-07-12:** (a) an `Rc` cycle-leak collector strategy, (b) the `using`/`defer` lifetime block, (c) the third coupled fork | **RULED — SPLIT the block.** (b) is already ruled as DEC-364, so (a) and (c) are ruled separately rather than kept in a three-way coupled block that has stalled six weeks. The coupling, not the items, is what stalled. ⊳ **CLOSED as bookkeeping by DEC-390 (2026-07-29):** (a) and (c) needed no separate ruling — they are DEC-205 and DEC-204, both ruled 2026-07-12; the inventory row this split was based on was already stale. Build-order 7.5 is a BUILD slice, not a ruling slice | **CLOSED — see DEC-390** |
| DEC-384 | **L-18 / P-Q-A-1 — stdlib wildcards.** `import Core.Http.*;` is parser-rejected because the loader's native/prelude pre-pass intercepts `Core.*` imports BEFORE the wildcard-expansion hook (`wildcard-imports.md:171-173`) — rejected deliberately rather than shipping silent-wrong behaviour | **RULED — build it: allow stdlib SUBMODULE wildcards.** **The existing D4 scope is confirmed unchanged and needs no ruling** (developer confirmed 2026-07-26): `import Acme.*` works for project and vendored packages at any level with no forced depth; **bare `import Core.*` stays rejected** as `E-WILDCARD-STDLIB-ROOT`. The fix is ordering the native pre-pass against the wildcard hook | **RULED — build queued** |
| DEC-385 | **NEW (developer spotted 2026-07-26) — `Core.String` and `Core.Text` are a duplicate surface.** `Core.String` has 45 natives; `Core.Text` has `parseInt`/`parseFloat`/`indexOf`/`length`/`slice`/`reverse` (`src/native/text.rs`, `src/native/file.rs:212`), which overlap heavily. **Under DEC-342 this becomes a live bug**: both contribute UFCS members to `string`, so `line.length()` resolves two ways and fires the ambiguity error on ordinary code | **RULED — (A): merge `Core.Text` into `Core.String` and deprecate the module** (`W-DEPRECATED`, per DEC-386's `Core.File` precedent). One string module, one place to look, no UFCS ambiguity. **Also corrects a Claude error:** `docs/specs/2026-07-26-ufcs-lsp-companion.md` labels `Core.Text` "the unicode tier" — it is NOT; `text.rs:35` says it is *"ASCII-oriented … (PHP under `-n` has no mbstring)"*. That spec row must be fixed | **RULED — build queued** |
| DEC-386 | The cheap tail: **L-83 / DEC-200** (PHP-reserved / builtin-class names as top-level type names) · **L-85** (`Core.Time.DateTime` bare-import-gating inconsistent with other injected types) · **L-20 / P-Q-A-4** (group-`{}` member sorting is a structural **no-op**) · **L-26** (`Core.File` deprecation/migration) · **L-30** (the Claude bundle's Q-J1…Q-J8) | **RULED:** DEC-200 → **close as already-ruled** (DEC-202 ruled `E-RESERVED-NAME`; stale row). L-85 → **make it consistent with DEC-353** (auto-provide injected symbols). P-Q-A-4 → **delete the no-op** — it currently claims to sort and does not. L-26 → **deprecate `Core.File` via `W-DEPRECATED`**, pointing at `FileSystem`. L-30 → **close as SUPERSEDED by DEC-354** (7 skills, allow-list-only permissions, `precompact-handoff` only, no MCP, no session-remember) | **RULED — build queued** |

## DEC-387 — `AskUserQuestion` is FORBIDDEN; every question is PLAIN TEXT (2026-07-27, developer-ruled)

| DEC | Item | Ruling |
|---|---|---|
| DEC-387 | **The question mechanism itself was broken and every rule pointed at it.** `AskUserQuestion` returned *"the user did not answer"* **four times on 2026-07-26** while the developer was at the keyboard, and the installed framework simultaneously (a) MANDATED it for every user-facing question, (b) forbade prose questions and numbered prose lists, and (c) claimed the rule was *"mechanically caught by the `ask-human-question-guard.sh` Stop hook"* — a hook that **is not installed here** (`J-DANGLE#4`). So the one mandated channel could silently swallow a question, the compliant alternative was banned, and the enforcement claim was false | **RULED — developer instruction verbatim (2026-07-27):** *"never use askUserQuestion — you must put the context clearly with clear options and clear examples with a recommended option"*. Every question is **PLAIN TEXT**: context → a **minimal concrete example** (for a language question, a runnable current-syntax program and its actual output/error) → **numbered options**, each carrying its own after-state → the **recommended option FIRST** with the reason it wins → a **visible** *"none of these / challenge the premise"* escape → **STOP** and wait. Never assume an answer; never proceed on a default; never re-ask a *different* question because the first went unanswered. **Landed in the same change (2026-07-27):** `.claude/skills/ask-human/SKILL.md` INVERTED (it previously mandated the tool and forbade prose — its `allowed-tools: AskUserQuestion` line is gone); `CLAUDE-global.md`'s four mandate sites rewritten and the false "mechanically enforced" claim replaced with *"nothing enforces this mechanically, so it is on you"*; project **Invariant 15 (ADJUDICATION RULE)** amended — the after-state now goes INSIDE each option rather than in "per-option previews", which was dialog-specific wording; the `notes`/`annotations` escape hatch replaced with a visible numbered option (that field is not rendered anyway); and `/gaps`, `/pre-commit`, `/retrospective` re-worded off their *"list IDs in notes"* phrasing, which only made sense in the dialog UI. **Why the rule is stated as a ban rather than a preference:** a gate that cannot fire is worse than no gate — an unanswered question that *looks* answered ends the turn silently, which is the exact failure this project spent a session recovering from | **BUILT 2026-07-27** |

**Corollary recorded for J-DANGLE#4:** the missing Stop hook is now moot in both directions — it guarded
a tool that is banned, and importing it would have double-gated against the container's own Stop hooks
(DEC-354). No hook is owed; the discipline is behavioural and stated as such.

## DEC-388 — reopening four bundle items DEC-386 closed too broadly (2026-07-27, developer-ruled)

DEC-386 closed the bundle's `Q-J1…Q-J8` as *"SUPERSEDED by DEC-354"*. That closure was **too broad**:
DEC-354 ruled on exactly four things — which skills, the permission tiers, which hooks, and MCP — and
never addressed `Q-J5` (disk reclamation), `Q-J6` (a phorj-native `/full-review`) or `Q-J7` (agent
defs). Those three were swept up rather than decided. Re-surfaced, and ruled below.

| DEC | Item | Ruling |
|---|---|---|
| DEC-388.1 | **`scripts/disk-reclaim.sh` (Q-J5, reopened).** The bundle's `/cleanup` pruned Claude state; the real disk crisis here is build artefacts | **RULED — BUILT 2026-07-27.** Evidence, measured: `target/` = **22 GB**, root fs **88% full, 4.8 GB free**, and `SLICE-STATE` records *"No space left on device"* having surfaced as **spurious build reds** — a red suite that is really a full disk is the most expensive failure mode we have, because it reads as a code regression. Three tiers (`cache` default / `debug` / `all`); **dry run is the default**, `--yes` required to delete. Guards: refuses to run outside the phorj repo (marker files), confines every candidate to `target/` or `var/claude/` **after path resolution**, and **never touches `var/phorj-app`** (DEC-259 / Invariant 18) at any tier. 19-assertion test suite incl. a bait file under `var/phorj-app`. Measured dry-run: cache tier frees **1.9 GB** keeping all binaries; debug tier **22.6 GB** | **BUILT** |
| DEC-388.2 | **`/forge` — DEC-354's drop REVERSED.** DEC-354 dropped it as *"infra-shaped rather than phorj-shaped"* | **RULED — REVERSED and BUILT 2026-07-27. The original drop rested on a mischaracterisation Claude supplied, and is corrected here rather than quietly.** `/forge` is architecture-shaped, not infra-shaped. Its **Chesterton's Fence** gate escalates a structural choice only when no recorded rationale exists AND all four fields populate (named principle · concrete alternative · cost-to-change · cost-to-keep), else the finding is dropped as noise. In most repos that gate is dead weight — no WHY corpus, so everything escalates. phorj has **221 register rows, 18 frozen specs, a 210-line INVARIANTS, plus ARCHITECTURE and HISTORY**: the step that normally makes `/forge` noisy is precisely what makes it accurate here. Adapted: `--quick` (A/B/D) is the DEFAULT tier (findings cost tokens to ACT on, not just to generate), reports to `var/claude/forge/`, `--scope=global|both` removed, plain-text gate, and **four mandatory phorj lenses** (Invariants 13, 4, 3, 5). Its agents must grep the register for the relevant `DEC-` row FIRST — a ruled decision is Justified by definition, and re-litigating one is the worst output this skill could produce | **BUILT** |
| DEC-388.3 | **`.claude/agents/backend-parity-reviewer.md` (Q-J7, partial).** DEC-268 mandates a 3-lens reviewer panel at every 3C/6C gate | **RULED — BUILT 2026-07-27.** There was **no `.claude/agents/` directory at all**, so the mandated panel was improvised from memory at every single gate. This def encodes the correctness+regression lens with the domain content that cannot be improvised: the triple-spine attack surface in Invariant order (coverage-first, because `tests/differential.rs` globs `examples/**/*.phg` so **an unexampled feature has ZERO parity coverage**; then the `Op` triad, single-sourced kernels + `classify` drift, reified operands, the CTy trap, scratch slots, sugar expansion, transpile-AND-lift currency, and the PHP-8.5-floor specifics). Read-only tools; told to REFUTE, not approve; a finding without a command and its output is not a finding. **Only this one lens is defined** — the other two (security+safety-promises, completeness+blast-radius) are generic and gain little from a def beyond what CLAUDE.md already states | **BUILT** |
| DEC-388.4 | **`scripts/validate-infra.sh` — native, INSTEAD of importing the bundle skill.** Global Rule 7 names `bash -n` as the REQUIRED Coverage evidence for infra changes | **RULED — BUILT 2026-07-27, deliberately NOT the import.** The bundle skill is 212 lines built on `docker compose config`, hadolint and yamllint. Measured here: **0 Dockerfiles, 0 compose files, and yamllint/shellcheck/hadolint all absent from PATH** — four of its six steps would degrade to skips. What exists is 9 tracked shell scripts + 2 git hooks + 4 workflows. So: ~100 native lines doing `bash -n` over every tracked shell file, a YAML parse over every workflow, and a JSON parse over every tracked JSON (the `settings.json`-breaks-silently case) — and, the part a skill cannot do, **wired into `pre-push`** so the Rule-7 evidence is produced *mechanically* rather than remembered. Absent linters are reported as **LOUD SKIPS**, never as passes. 18-assertion suite, including a behavioural read-only check (tree hash identical before/after). First real run: 11 shell / 6 YAML / 7 JSON, all pass |
| DEC-388.5 | The rest of the bundle tail: **`/recent`**, **`/skill-audit`**, **`/qa-sweep`**, **Q-J6 `/full-review`**, **`inv13-decomposer`** | **RULED — `/recent` OUT as OBSOLETE**: the PreCompact handoff hook now emits git state, uncommitted paths and recent commits automatically, so importing it would duplicate something that already happens unasked. **`/skill-audit` OUT**: its stated precondition ("10+ imported skills") is now met at 13, but it audits *skills* — meta-work whose value the audit itself called circular. **`/qa-sweep` QUEUED, CLI-mode-only, after Wave 0**: `phg` has **17 subcommands** and no systematic CLI QA, which is real; the browser half is unusable (Playwright MCP ruled OUT). **Q-J6 `/full-review` DEFERRED**: `/aggregate-findings` already covers the synthesis half, and fanning out 8 review skills is the most expensive thing in the set. **`inv13-decomposer` OUT for now**: the size gate reports `fails=0`, so it would have nothing to do | **RULED** |

**Bundle accounting after DEC-388: 199 files, 48 skills, 13 skills now in the repo** (7 from DEC-354 +
5 predating it + `/forge`), plus 1 hook, 1 agent def and 2 native scripts written *instead of* imports.
Everything else remains OUT with a stated reason — `/qa-sweep` is the only item still queued.

### Still OPEN — deliberately not ruled (Claude had titles only, refused to fabricate)

**L-22** (DEC-334 runtime-config catalog / php.ini equivalent) · **L-25** (`App\`-prefixing + the
`phpInterop{namespaceRoot,sourceRoot}` knob, DEC-320 F2) · **L-33** (DEC-324's 7 remaining TOP items) ·
**L-86** (DB column naming, slice B2, + the cross-prelude error-namespace convention).

Evidence pointers are in the L report. **Developer-approved to defer** (2026-07-26) rather than accept
invented recommendations. **L-22 and L-33 look substantial.**
⊳ **L-19 → DEC-392 · L-28 → DEC-393 · L-31 → DEC-391** (all three RULED 2026-07-29, batch 1 of the
consistency-audit question sweep) — the mechanical three are gone; four substantial ones remain.

## 2026-07-29 — developer ruling (post-consistency-audit)

| ID | Question | Ruling | Status |
|----|----------|--------|--------|
| DEC-389 | Where is the record of truth a session must read/write — is the SSOT set advisory or mandatory? | **RULED (developer, 2026-07-29): the SSOT quartet is MANDATORY.** `docs/plans/MASTER-PLAN.md` (roadmap) · `docs/specs/UNIFIED-SPEC.md` (language/spec) · `docs/plans/SLICE-STATE.md` (current slice + queue) · this register (decisions). Every session reads them before working and writes through them in the same change as the work; every other document stating roadmap/spec/slice/decision content is a pointer, never a copy. Written into CLAUDE.md Invariant 19 | **RULED — WRITTEN** |

## 2026-07-29 — developer rulings (audit question batch 1: the four closers)

Batch asked plain-text per DEC-387 / Invariant 15; the developer accepted all four recommendations.
These close audit-inventory items **9, 5, 1 and 4** (`docs/research/2026-07-28-consistency-audit.md`
§PENDING-question inventory) and empty three of the register's seven "Still OPEN" rows.

| ID | Question | Ruling | Status |
|----|----------|--------|--------|
| DEC-390 | **Audit Q1 — does build-order 7.5 re-open DEC-204/205, or build them?** DEC-383 (2026-07-26) split L-84's three coupled lifetime forks and scheduled (a) + (c) "to be ruled" at Wave 7.5 — but `L-onhold-inventory.md:140` names those forks as (a) the `Rc` cycle-leak collector, (b) `using`/`defer`, (c) the `Runtime.onShutdown` hook, and **all three were already ruled on 2026-07-12** as DEC-203 / DEC-204 / DEC-205. The inventory row that said "all three DEC-PENDING" was stale when written, so DEC-383 split an already-decided block | **RULED — close DEC-383 as bookkeeping; 7.5 is a BUILD slice.** DEC-203 (`using`), DEC-204 (`Runtime.onShutdown`) and DEC-205 (threshold cycle collector first, `Weak<T>` second) stand as the rulings — nothing is re-opened. Re-asking would re-litigate a decision made with better evidence than we have now: DEC-205 in particular records a perf re-ask that produced "collector first" (≈ zero steady-state cost via root-buffering + threshold passes). Build-order 7.5 becomes *"build DEC-205 + DEC-204"*, and the ⚠ Q1 tension notes come off build-order 7.5, the MASTER-PLAN DEC-203/204/205 mirrors and SLICE-STATE | **RULED — 7.5 unblocked as a build slice** |
| DEC-391 | **L-31 — `VirtualModule.src` → `srcs` rename** (an autonomous in-build rename listed "dev to review", never signed off) | **RULED — RATIFIED, zero code change.** The plural is load-bearing: `src/cli/preludes.rs:565` holds a fragment LIST because `Core.Http` splits its prelude source across two consts for the Invariant-13 cap (DEC-331 slice 2), and the fold treats the fragments as one prelude. Reverting to `src` would need the Inv-13 split undone or a singular name that lies | **RULED — ratified** |
| DEC-392 | **L-19 / P-Q-A-2 — D3's ruled wording "`*` binds public + internal" vs the as-built public-only cross-package rule.** Read literally, D3 makes `import Other.*` a wider grant than `import Other.Thing` | **RULED — ratify as-built; rewrite the wording to the unifying principle:** *"`*` binds every member you would be allowed to import individually — public cross-package; public + internal within the declaring package or a DESCENDANT package (`internal` is subtree-scoped)."* That is what the code already says [Verified: `src/loader/import_hygiene.rs:12-33` — `wildcard_members` keeps exactly the names for which `vis_violation(...)` returns `None`; `src/loader/visibility.rs:26-47` — `Public` → always legal, `Private` → same-FILE only, `Internal` → legal iff `pkg_is_ancestor_or_equal(declaring, referrer)`, so the earlier "same-package" gloss in this row understated it and was corrected in the same change]. The literal reading was REJECTED because it would make `*` a visibility bypass: it would reach cross-package `internal` members that a named import still rejects with `E-VIS-INTERNAL`, so `internal` would stop being an enforceable boundary. No code change; the spec's D1/D3/D3-CONFIRMED/step-2 wording, the two `parser/items/decls/imports.rs` doc-comments and the SLICE-STATE pointer are corrected in the same change | **RULED — wording corrected** |
| DEC-393 | **L-28 — pipe-lambda trailing tight-op binding.** `5 \|> (v => v * 3) + 1` binds the `+` to the LAMBDA (`\|>` is looser than `+`), giving a loud `E-PIPE-LAMBDA-CONTEXT`; the ergonomic alternative (bind trailing tight-ops to the pipe result) is strictly additive | **RULED — KEEP the loud error; the fork is CLOSED.** One uniform rule wins: the RHS grammar after `\|>` does not change shape depending on whether the RHS is a lambda or a named function, so `x \|> f + 1` and `x \|> (v => …) + 1` fail identically with a hint that names the fix (`(x \|> (v => …)) + 1`). The additive carve-out was REJECTED because it makes a lambda's extent depend on what follows it — silently, and asymmetrically with the named-function form. Zero code change; the "PENDING fork" sentences come out of the DEC-239 row and `src/parser/exprs/pipe.rs:19` | **RULED — closed, no code change** |

**Register bookkeeping done in the same change:** the "Still OPEN — deliberately not ruled" list drops
**L-19**, **L-28** and **L-31** (four of its seven were mechanical; three are now ruled), leaving **L-22**
(runtime-config catalog), **L-25** (`App\`-prefixing / `phpInterop`), **L-33** (DEC-324's 7 TOP items) and
**L-86** (DB column naming + error-namespace convention).

## 2026-07-29 — developer rulings (audit question batch 2)

| ID | Question | Ruling | Status |
|----|----------|--------|--------|
| DEC-394 | **Cross-prelude error-class collisions (L-86 second item).** Injected-class dedup makes two preludes declaring the same error name silently share ONE class (wrong catch semantics); the stdlib was inconsistent (`HttpTimeoutError`/`MailTimeoutError` prefixed, the database module's bare `TimeoutError` not) | **RULED — (1) prelude-injection collision is a HARD COMPILE ERROR** (closes the silent-share hole) **and (2) DROP the prefixes: injected error classes become MODULE-SCOPED.** Developer, verbatim: *"the module itself should not prefix itself"* and *"we can use `as` later"*. Decisive new evidence found while asking: **qualified per-module catch ALREADY WORKS** — [Verified live: `catch (DatabaseModule.TimeoutError e)` type-checks clean; a bare `TimeoutError` without a member import is `E-INJECTED-TYPE-BARE` with a hint naming the qualified form]. So the prefix is redundant under qualified use and *stutters* (`Http.HttpTimeoutError`); the import is where you say which module you mean, exactly as "nothing in the wind" intends. `HttpTimeoutError`/`MailTimeoutError` → `TimeoutError`; disambiguation between two same-leaf member-imports comes from `import … as`, and the hard error fires when neither qualifier nor alias resolves it. PHP leg emits module-namespaced classes (DEC-329.3's scoped-variant precedent) | **RULED — build queued** |
| DEC-395 | **The nullable arena `Kind` (audit item 10).** The "0.19–0.20× hard flag" framing was STALE: the flag closed via the ruled `??`-fusion lever (`maxby 0.19×→8.13×`, `minby 0.20×→8.18×`). What stayed open is whether the unboxed `Kind` enum gains a nullable form for the window-less case | **RULED — BUILD IT NOW, as its own slice.** Developer: *"if we will build it at some time, we should build it now"*. Rationale accepted: this is a REPRESENTATION decision and representation cost is monotonic — [Verified: **652 `Kind::` sites**, enum at `src/jit/analyze/kinds.rs:17`; Invariant 3's wildcard-free discipline means every one is handled, not defaulted], and that surface only grows. Doing it now, with the fusion lever as a working comparison baseline and no perf deadline, is strictly cheaper than doing it later under a regression. **Success criterion:** window-less `maxBy`/`minBy` reach parity-or-better AND `extreme_by_coalesce_window` can be DELETED rather than kept alongside; every nullable-returning native becomes unboxed-eligible as a consequence | **RULED — build queued** |
| DEC-396 | **DEC-339 case-matrix completion.** The developer restated the rule's asymmetry (inner-shadows-live-outer = error, "we can't differentiate and can't access the more global var"; reuse after the first binding is dead = fine) and asked what cases were missing. The ruled rule already encodes exactly that — but four shapes were not enumerated among the 23 rows | **RULED — add to `docs/specs/2026-07-26-block-scope-shadowing.md`. ACCEPTED (all verified byte-identical live on vm/tw/php):** (24) inner block declares, then the ENCLOSING scope declares after it — `{int b=1;} int b=2;` → `1\|2`; (25) `for` counter, then an outer declaration after the loop → `0\|1\|9`; (26) deep-nested, then shallower sibling, then outer → `1\|2\|3`. **REJECTED (hygiene class — byte-identical, so not a correctness break):** (27) a lambda local redeclaring the lambda's OWN parameter — `function(int x){ int x = 2; }` → developer: *"should be hard error"*. **Also ruled:** the "scopes are opened by" list gains **`using`** (DEC-203/364, build-order 7.5) and **local functions / local classes** (DEC-352); `_` is exempt — [Verified: `src/ast/types_core.rs:70` `Wildcard(Span)` binds no name] | **RULED — folds into the DEC-339 build** |
| DEC-397 | **DEC-366 lifter-hoist scheduling** — asked 3× unanswered, with a PROVISIONAL DEFAULT already in force at `SLICE-STATE.md:246` and build-order 1.1 (the pattern Invariant 15 exists to prevent) | **RULED — RATIFIED: the hoist rides in the DEC-339 slice.** Same PHP-function-scope-vs-phorj-block-scope insight from the inverse direction, and Invariant 17 wants lift moving with the feature rather than trailing it. [Verified live: `phg lift` on `function f(){ if(true){$b=5;} $b=7; return $b; }` emits `mutable var b = 5;` INSIDE the block, then `b = 7;` outside → `E-ASSIGN-UNKNOWN` + `E-UNKNOWN-IDENT` on the draft.] The provisional-default note becomes a ruling | **RULED — rides in DEC-339** |
| DEC-398 | **Per-field DB column mapping (L-86 first item).** The audit's premise was STALE — the column naming strategy SHIPPED (DEC-208 slice B2 + DEC-258's COMBINED model: `Naming.Exact` default / `Naming.SnakeToCamel`, promoted `naming` field on the connection, `.namingStrategy(…)` copy-builder, baked when statically traceable / dispatched when not). `KNOWN_ISSUES.md:798-801` ("NOT shipped … pending a developer ruling") is a stale label to flip. What is missing is a per-field override for schemas following neither strategy | **RULED — add FIELD ATTRIBUTES as a GENERAL capability, with the DB mapping as its first stdlib consumer.** [Verified: `attrs: Vec<Attribute>` exists only on `src/ast/decls/classes.rs:66` and `functions.rs:14` — fields carry no attributes at all today.] Ruled on the developer's standing principle: *"all native/phg features must be generic enough so anything can be built using them or on top of them"* — so the slot is general (and DEC-194's user-defined attributes later reuse it), not a one-off. Payload ruled for the DB attribute: the column **name** literal + per-field **casing** override, via named args (the `#[Entry(kind: …)]` precedent); an explicit name wins over the connection strategy. EXCLUDED from it: what the type already states (`nullable` ⇒ `T?`), what is global (`naming` ⇒ DEC-258), and validation concerns (those get their own attribute — which is what a general slot buys). **The attribute's NAME is still open** — the developer rejected `#[Column]` as carrying Doctrine's schema-definition connotation when ours only maps a name/casing | **RULED — name PENDING** |

**Bookkeeping recorded in the same batch:** the developer's *"the database was renamed"* is **DEC-350** (ruled 2026-07-26: `Core.Database.Connection`, `Module` suffix drops; build-order slice 5.4) — **ruled, not yet built**, so `src/cli/preludes.rs:775` still reads `Core.DatabaseModule` with a `Database` class. Claude's batch-2 question reported the as-built name as if it were the current truth; **as-built ≠ ruled** and both must be checked before stating a name.

## 2026-07-29 — developer rulings (audit batch 2, round 3: transpile-into-project + the field attribute)

Provenance: asking DEC-398's open name question triggered a research pass on *"is F2 the only case where
the emitted PHP differs from idiomatic PHP, and what is PHP best practice?"* — the developer's question.
Six divergences were measured on real output; four rulings follow.

| ID | Question | Ruling | Status |
|----|----------|--------|--------|
| DEC-399 | **DEC-398's open name.** `#[Column]` was rejected (Doctrine's carries schema definition — type/length/nullable/precision — while ours only maps a name/casing); `#[DbName]` was then rejected too, as reading like *the name of the database* | **RULED — `#[ColumnName(…)]`.** Chosen over `#[MapsTo(…)]` because it COMPOSES: when JSON/CSV mapping wants a field name later, each gets its own precise attribute and they stack (`#[ColumnName("created_at")] #[JsonName("createdAt")]`) without either knowing about the other. `#[MapsTo]` would need a per-surface discriminator (`#[MapsTo(db: …, json: …)]`), turning one attribute into a central registry every future feature must edit — the bottleneck the developer's genericity principle exists to prevent. The GENERAL thing is the field-attribute slot (DEC-398); each consumer owns a precise attribute. Forms: `#[ColumnName("created_at")]` (literal) and `#[ColumnName(naming: Naming.SnakeToCamel)]` (per-field casing) | **RULED — build queued with DEC-398** |
| DEC-400 | **F2 `phpInterop.namespaceRoot` (L-25), reopened after the cost was corrected.** Claude's first cost figure ("88 emission sites") was a naive grep and wrong: [Verified: `src/transpile/names.rs:5` — `namespace_of` derives the namespace from the ALREADY-MANGLED name; **14 call sites** in 3 files], so the prefix enters at ONE upstream point | **RULED — BUILD the knob: default OFF, explicit-only, PROJECT packages only.** Developer: *"no `App` in the wind! but we want to add it we add it easily and structurally"*. Absent config ⇒ byte-identical to today (nothing implicit); `{ "phpInterop": { "namespaceRoot": "App" } }` ⇒ `namespace App\Billing;`. **Vendored packages are NEVER prefixed** — a library's FQN must not depend on who consumes it, else two apps see the same source as `App\Acme\Client` and `Shop\Acme\Client` and every stub/`declare` written against it breaks. `phg lift` reads the same config so the round-trip stays symmetric (Invariant 17) | **RULED — build queued** |
| DEC-401 | **No `declare(strict_types=1)` in ANY emitted PHP** — [Verified: `grep -rn strict_types src/ tests/ examples/` → **0 hits**]. So every transpiled file runs in PHP's coercive mode: a host calling an emitted `function helper(int $x)` with `"5"` gets a silent coercion, where phorj's own checker would never have admitted the call | **RULED — EMIT `declare(strict_types=1);` in every transpiled file.** The PHP leg must enforce at its boundary what phorj enforces everywhere else, or "statically typed" is a promise the output quietly drops. Byte-identity for phorj-only programs is unaffected (the checker already guarantees the types, so no existing example can change behaviour) — it changes only what happens when HOST PHP calls in wrong, which today is silent coercion and becomes a `TypeError`. Also plain PHP best practice | **RULED — build queued** |
| DEC-402 | **Emitted PHP is not PSR-12** — measured on real output: `final class Invoice {` (brace on the same line) and `function __construct` / `function total(): int` with **no explicit `public`** (PSR-1/PSR-12 both require it) | **RULED — make the emitter PSR-12-compliant.** Braces on their own line for classes/functions, explicit visibility on methods. Without it every adopting team with `phpcs`/PHP-CS-Fixer in CI must special-case the generated tree forever — friction landing exactly where DEC-320's adoption story lives. Every transpile golden/expected-output fixture re-baselines in the same change (mechanical, gate-verified) | **RULED — build queued** |

**Recorded as DELIBERATE and NOT to be "fixed" (same research pass, so a future session does not file them as bugs):** (1) enums emit `abstract class` + `Status_Open`/`Status_Paid` subclasses rather than PHP 8.1 native `enum` — **forced**, PHP enums cannot carry per-case payload and `Paid(int amount)` does; uniform classes beat a split rule (DEC-329.3). (2) Autoloading is a generated CLASSMAP + one composer `files` entry rather than PSR-4 — **deliberate and strictly better** (DEC-320 delta α: one `.phg` enum emits several classes, which PSR-4 cannot address). (3) Free functions live in the eagerly-loaded shared runtime — **forced**, PHP cannot autoload functions.

## 2026-07-29 — DEC-403: the DB column-naming DEFAULT is flipped (supersedes DEC-258's polarity)

| ID | Question | Ruling | Status |
|----|----------|--------|--------|
| DEC-403 | **Should the column-naming default stay strict-exact?** DEC-258 (2026-07-17) ruled *"default stays STRICT exact-name"* and explicitly REJECTED the alternative as *"auto-map default (silent name transformation — the magic phorj rejects)"*. The developer now proposes the rejected option as the default | **RULED — FLIP IT. Default = `Naming.SnakeToCamel`: camelCase in the phorj model, snake_case in the database.** Developer, verbatim: *"the best practice is the model is camelCase and the database is snake case! anything else is specific and may never be used! so it must be opt in"* — so the polarity inverts: the convention is the default and **strict `Naming.Exact` becomes the opt-in**. **This is a deliberate reversal of DEC-258, recorded as such, on three pieces of new evidence:** (1) `#[ColumnName]` did not exist when DEC-258 was ruled — its objection was silent magic *with no way out*, and there is now a per-field pin plus per-statement and per-connection overrides; (2) the shipped example proves the old default taxes the common case — [Verified: `examples/database/naming.phg:35-40` has camelCase fields (`userName`, `firstName`, `postalCode`, `homeAddress`) against snake_case SQL (`m.user_name`, `a.street_name`)], and phorj *mandates* camelCase on the code side, so under `Exact` every multi-word field pays a SQL alias or a strategy call forever; (3) blast radius is bounded — single-word fields are identical under both strategies (`id`→`id`), so only multi-word hydration sites move. **CONDITION carried into the build (this is what keeps it from being the magic DEC-258 feared): a derived lookup that MISSES must show its work** — the diagnostic names the field, the derivation, the columns the row actually has (`Row.columnNames` exists), and both escapes, e.g. *`no column "created_at" (derived from field createdAt by the default snake_case strategy); the row has: id, createdAt — pin it with #[ColumnName("createdAt")] or pass naming: Naming.Exact`*. **Precedence, most-specific first:** `#[ColumnName]` → per-statement `namingStrategy` → connection `naming` → this default. **DEC-258 is SUPERSEDED as to the default; its COMBINED-model machinery (promoted field, copy-builder, bake-or-dispatch) is unchanged and now carries the new default value** | **RULED — build queued with DEC-398/399** |

**Migration owed in the build:** `examples/database/*.phg` and the DB tests re-baseline against the new
default (the `Exact` demonstrations become explicit `naming: Naming.Exact`), and `examples/database/naming.phg`
is rewritten so the DEFAULT path is the one shown first.

## 2026-07-29 — DEC-404: the lambda capture model + the `Mutable<T>` escalation guard

| ID | Question | Ruling | Status |
|----|----------|--------|--------|
| DEC-404 | **What does a lambda body MEAN — implicit capture-everything, or a PHP-style explicit `use()` list?** Raised by the developer while ruling DEC-396: *"should it capture everything by default so redeclaring fails hard with clear error??? or should we use php use () to decide what goes inside! and if we let everything flow! then we need to restrict the `Mutable<>` so a function cannot use a value by reference on it's own! only the more global scope must declare the Mutable version that the lambda or function can use! this is one of the vulnerability we need to detect"*. Today capture is implicit and BY VALUE, and the capture set already excludes locally-declared names [Verified: `src/ast/tests.rs:169` `free_vars_inner_var_not_captured`; capture list = `free_vars` minus params] | **RULED — KEEP implicit capture by value, and add TWO hard rules.** (1) **A captured name counts as a LIVE binding inside the lambda from its first use**, so redeclaring it is a hard error naming both sites. This closes the shape DEC-396's probing found: `int x = 1; function(int n){ int y = x; int x = 5; … }` is byte-identical today on all three legs (prints `6` / outer `x` stays `1`) yet the name `x` means the captured outer value on one line and a new local on the next — the exact *"we can't differentiate"* hazard the developer used to justify rejecting outer-shadowing. Hygiene class, not correctness. (2) **`Mutable<T>` ESCALATION GUARD: a lambda may RECEIVE and use an outer `Mutable` handle (captured by value — the handle copies, the contents are shared, which is the point) but may NOT construct one over a captured plain local.** Sharing is a decision the DECLARING scope makes, never one the callee takes for itself; without this, implicit capture would let a lambda silently manufacture write access to its enclosing scope and walk around DEC-357's capture-write rejection. PHP-style `use()` was REJECTED: it is a breaking syntax change to every existing lambda in `examples/` and the stdlib preludes, it imports PHP's worst closure wart into a language whose premise is not being PHP, and every peer language with closures (Rust/Swift/Kotlin/TS) captures implicitly. The maximally-paranoid variant (a captured name may only be READ, never passed as a constructor argument) was also rejected — it would force ordinary code to thread `prefix` through as a parameter. **`Mutable<T>` is ruled but UNBUILT** [Verified: every `Mutable` hit in `src/` is the `mutable` KEYWORD, not a prelude class], so the guard is built INTO it rather than retrofitted — it rides with DEC-357 + DEC-368 in build-order 4.1 (note: 4.1 records the ruled surface as `.value`, not DEC-368's original `get()`/`set(v)`) | **RULED — build queued (4.1)** |

## 2026-07-29 — developer rulings (audit batch 3: L-33's TOP items + L-22's campaign shape)

| ID | Question | Ruling | Status |
|----|----------|--------|--------|
| DEC-405 | **L-33, the four already-shaped web-pack items** (response streaming · HttpClient proxy/CA/mTLS · HttpClient streaming bodies · `SessionStore` public contract) — DEC-324 recorded a shape for each with "per-item adjudication at build time" | **RULED — RATIFIED AS RECORDED.** `Response.stream(Iterator<bytes>)` chunked/file streaming · `ProxyConfig`/`TlsConfig` on the Transport seam, **Secret-typed** · the HttpClient body size cap becomes the DEFAULT rather than the wall · `SessionStore` joins the layered-openness public-contract list (Memory now, Db-backed rides `Core.Database`). Each still owes a runnable example (Invariant 9) | **RULED — build queued (W3)** |
| DEC-406 | **Trusted-proxy headers — greenfield** [Verified: `grep -rn "X-Forwarded\|TrustedProx" src/` → zero hits, so no live vulnerability, but the DEFAULT decides whether a client can forge its own IP for rate limits, audit logs and allowlists] | **RULED — deny-by-default CIDR allowlist with RIGHTMOST-TRUSTED-HOP semantics, plus a loopback-gated dev hatch.** `TrustedProxies` holds CIDRs; forwarded headers are honoured ONLY when the immediate peer is inside one; the client IP is found by walking `X-Forwarded-For` right-to-left and stopping at the first untrusted address (leftmost-hop parsing is the classic CVE shape — the leftmost entry is entirely client-controlled). Honour `X-Forwarded-For`/`-Proto`/`-Host` AND RFC 7239 `Forwarded`. **Dev hatch (developer-requested, opt-in only and secure by construction): `TrustedProxies.Any` is honoured ONLY when every listening address is loopback; on any other address it is a HARD STARTUP ERROR, not a warning** (warnings get scrolled past and live for years), and a loud startup line names the mode while active. The guard is a property the OS enforces, not a convention: no config file can make a public listener loopback. CIDRs cover the real Docker/VPC dynamic-range cases in both dev and prod, so the hatch stays rare | **RULED — build queued (W3)** |
| DEC-407 | **`Range` + `gzip`, and the compression dependency.** They were bundled as one DEC-324 row but have different costs: `Range` is std-only byte slicing; `gzip` needs DEFLATE and [Verified: no compression crate in `Cargo.toml`; `UNIFIED-SPEC.md:1464` lists `1=deflate(future) 2=zstd(future)` as unimplemented] we have none. DEC-382 already claimed the "15th dependency" slot for a `quick-xml`-class crate | **RULED — SPLIT them, and admit `flate2` as the 16th dependency NOW (developer chose to vet and admit ahead of the slice rather than with it).** `Range`/`206`/`If-Range` ships std-only. **Vetting, from the sparse index (crates.io's JSON API is policy-blocked; `index.crates.io` is reachable):** `flate2` **1.1.9**, not yanked, MSRV **1.67.0**; `default = ["rust_backend"] → miniz_oxide` so **the default build is PURE RUST** (`miniz_oxide ^0.8.5` + `crc32fast`; `miniz_oxide` also pulls `adler2 ^2.0`) and every C backend (`libz-sys`, `libz-ng-sys`, `cloudflare-zlib-sys`) is an OPT-IN feature we do not enable. Pure Rust is the decisive property here, not popularity: we ship cross-compiled binaries (`phg build --target` + sha256-verified stubs) and a C dependency turns every cross target into a C-toolchain problem. **Transitive runtime set = 4 crates** (flate2 + miniz_oxide + crc32fast + adler2) — all four need THIRD-PARTY-NOTICES rows; licenses are [Unverified] here (API blocked) and MUST be confirmed at admission. **COVERAGE GAP FOUND, with a consequence:** flate2 covers **DEFLATE / zlib / gzip** (read+write, streaming and one-shot) — i.e. both `Content-Encoding` values every browser supports — but does **NOT** cover bzip2, zstd, brotli, LZMA/xz or LZ4. **MASTER-PLAN's queued "bz2 as a format row in Core.Compress" therefore cannot be served by flate2, and the only mature bzip2 crate is C-backed (`bzip2-sys`)** — which contradicts the pure-Rust cross-compile rationale. bz2 is now an explicit sub-question for the `Core.Compress` slice: drop it (legacy, no HTTP relevance) or accept a C dependency | **RULED — `Range` + flate2 admission queued; bz2 re-opened** |
| DEC-408 | **Class-const expressiveness, enum `implements`, enum constants — and the developer's "a const that can be calculated once".** Verified absent live: `const int Y = 1 + 2;` → `E-CONST-NOT-LITERAL`; `function f(int n = C.Max)` → `E-DEFAULT-PARAM-EXPR`; `enum Color implements Named` → parse error; enum-level `const` → parse error | **RULED — Option 2: const EXPRESSIONS + enum `implements` + enum constants + a separately-named `lazy`, ALL IN ONE SLICE.** (1) **`const` stays compile-time-folded** and gains expressions over literals and other consts (arithmetic, concatenation, const references); no calls, no `new` — that keeps const evaluation a terminating, side-effect-free folder rather than a compile-time interpreter. (2) **`const` becomes legal as a default parameter value** — the developer asked *"we should support it! why not??"* and it is the same fold: `function f(int n = C.Max)` compiles, retiring that half of `E-DEFAULT-PARAM-EXPR`. This also matters for DEC-194, whose user-defined attributes require compile-time-const args. (3) **"Computed once" is a SEPARATE feature named `lazy`**, NOT an extension of `const`: a runtime-computed memoized immutable, never legal in a const position. Folding it into `const` was rejected because `const` is the currency the compiler spends where it needs a value at compile time (attribute args, default params), so a runtime-computed `const` would force the compiler to reject *some* consts in those positions — two concepts under one name, with error messages that cannot explain themselves. Cross-language precedent is uniform: Kotlin `by lazy`, Swift `lazy`, Rust `LazyLock`, C# `Lazy<T>` — all separate from constants. `lazy` carries initialization-order semantics that must be pinned on three legs, and transpiles to a PHP static-memo helper (Invariant-14 ladder case 1) | **RULED — build queued (W4), one slice** |
| DEC-409 | **DEC-334 runtime-config catalog (L-22) — and the developer's `ini_set` question.** Developer directive: *"make absolutely 100% everything customizable"*, and *"maybe we can/should implement something like ini_set to change runtime config in phorj??"* | **RULED — NO global `ini_set` analog. Developer accepted the objection.** Three reasons, each landing on a phorj invariant: (a) **action at a distance** — any vendor package could change global behaviour mid-run and you could not determine the effective config by reading your own code; (b) it **breaks Invariant 10 (determinism)** — behaviour becomes a function of *when* the setter ran; (c) it **breaks Invariant 1 cheaply** — a mutable global config means all three legs must agree on the ordering of every mutation *and* every read, an enormous parity surface bought for a feature whose main users are workarounds. **The model instead, which still delivers "100% customizable":** (1) **immutable startup config** resolved once through the D1 precedence chain (`phorj.json` → global → `ServeConfig` → env → CLI) — the default home for every catalog row; (2) **scoped overrides** where runtime adjustment is genuinely needed — lexically bounded and auto-restoring, `Config.with(precision: 3) { … }` (precedent: Python `decimal.localcontext()`, Rust scoped thread-locals) — runtime adjustability with none of `ini_set`'s three problems, because the change cannot outlive the block nor be invisible; (3) **per-object config as the default idiom**, which phorj already does better than PHP (`new Connection(dsn, naming)`, `Http.ServeConfig`, DEC-317 log channels, DEC-403's four-level chain). The one case that classically wants mid-run mutation is log level, and DEC-317's channels already model it as an object. **Campaign shape RULED: round 1 = Claude produces the enumeration** as a repo spec draft; every row carries proposed default · proposed home · scoped-override allowed y/n · byte-identity-affecting y/n; all rows PENDING; the developer rules in batches over **many rounds** (their explicit requirement — *"this too needs many rounds"*) | **RULED — round 1 (enumeration) queued** |

## 2026-07-29 — DEC-410: enum `extends` enum is REJECTED; sealed hierarchies are the sanctioned path

| ID | Question | Ruling | Status |
|----|----------|--------|--------|
| DEC-410 | **Should enums be able to extend other enums?** Proposed twice by the developer by analogy with `interface B extends A` (which works — [Verified live: type-checks clean]), and challenged twice by Claude | **RULED — REJECTED, after the research found phorj ALREADY SHIPS the sound version.** `sealed interface`/`sealed class` are implemented [Verified: `FEATURES.md:68`; `src/checker/matches.rs:169-176` — *"a scrutinee of a `sealed` class/interface is … `match` over the sealed base is exhaustive without a `_`"*] and deliver BOTH properties enum-extends was wanted for, proven byte-identical on all three legs: **subsumption** (a `Timeout` passes where a `DbError` is expected) AND **exhaustive `match` over the base with no `_` arm** — soundly, because the permitted set is closed and the checker can enumerate it. Once DEC-408's `enum implements` lands, enums join those hierarchies directly (`enum PgError implements DbError`). **Why `extends` on enums cannot be made sound:** the interface analogy is exact about interfaces and inverted about enums. An interface is an OPEN CONTRACT — adding requirements invalidates nothing. An enum is a CLOSED SET, and closedness is *what makes exhaustiveness checkable*: if `enum Shade extends Color { Dark }` is a subtype of `Color`, a `Shade` flows into every `Color`-typed slot, so **every previously-exhaustive `match` on `Color` silently stops being exhaustive — including in code the author is not editing and packages they do not own.** That is the precise hole exhaustiveness checking exists to close, and compile-time-verified exhaustive `match` is one of phorj's real wins over PHP (whose enum `match` is unchecked). Cross-language scan (Invariant 16 / META-7): **Rust, Swift, Kotlin, Java, C#, TypeScript and PHP 8.1 all forbid enum inheritance and all offer interface conformance instead** — seven independent designs, one conclusion. **Rejected alternatives recorded so they are not re-litigated:** (a) `extends` as pure variant-list REUSE with no subtype relation — the only *sound* reading, offered and not chosen; (b) full inheritance with a mandatory `default` arm on every participating enum — that is `sealed` with worse ergonomics. **Build obligation:** `enum X extends Y` gets a diagnostic that names the exhaustiveness reason and points at `sealed interface` + `enum implements` | **RULED — REJECTED, diagnostic queued with DEC-408** |

## 2026-07-29 — batch 4: the audit tail (ratifications, measurements, bookkeeping, scheduling)

| ID | Question | Ruling | Status |
|----|----------|--------|--------|
| DEC-411 | **The three auto-ruled, explicitly-reopenable rows** — DEC-224 (MongoDB: admission shape ruled, build deferred) · DEC-225 (concurrency PHP leg: hard error stands, PHP 8.1 **Fibers** recorded as the first non-downgrading candidate, spike-gated) · DEC-226 (`#[UncheckedOverflow]` transpile stays `E-TRANSPILE-UNCHECKED`; its old "checked default transpiles faithfully" claim was already corrected by DEC-255) | **RULED — all three RATIFIED as they stand.** None blocks anything and each carries a stated rationale. The only live follow-up is DEC-225's Fibers **spike** (a measurement, not a decision). The audit's "auto-ruled, reopenable" category is now empty | **RULED — ratified** |
| DEC-412 | **Three of the five owed measurements, computed in-container 2026-07-29** (the other two need the developer's box — the container has no Docker, so the bench harness cannot run) | **MEASURED.** **(1) DEC-339 migration cost = EXACTLY ONE in-tree site**, reported before migrating as the row required: `examples/guide/math.phg:46` declares `int l1 = Math.lcm(4, 6);` and `:54` declares `float l1 = Math.log(1.0);` — same scope, different type, i.e. DEC-339 **case 11** (the "meant to assign, accidentally re-declared, possibly at another type" typo class). Byte-identical today, so hygiene not correctness — but it breaks the moment the rule lands. **Migration cost = one rename.** Nothing else across 270 `.phg` files in `examples/` + `selftest/` + `tests/`. **(2) DEC-357 in-tree captured-local write scan = ZERO hits** — the row said *"any hit = a bug found"*; there are none. **(3) DEC-377 classification, FIRST PASS — and the register's "168" is STALE:** 180 raw `__phorj_*` grep matches, 31 of them artifacts (format-string prefixes like `__phorj_dec_`, short fragments) ⇒ **149 real helper names**. Buckets: **64** bucket-1 (semantic necessity — `checked_*`, `dec_*`, `fs_*`, `result_*`, `option_*`, `json_*`, `class_name`), **66** bucket-2 (no single-expression equivalent — `regex_*`, `http_*`, `log_*`, `rng_*`, `now_*`, `sort`, `min`/`max`, `all`/`any`/`find`, `drop_while`…), **17** bucket-3 CANDIDATES (convenience/DRY ⇒ must be INLINED), 2 unclassified (`float`, `none` — need a read). **The 17:** `debug_enums` `debug_quote` `debug_render` `debug_wrap` `format` `number_format` `range` `str_chunk` `text_index_of` `text_reverse` `text_trim` `text_trim_end` `text_trim_start` `trim` `uri_parse` `uri_resolve` `uri_with`. **Two findings inside that list:** (a) the **`uri_*` trio may be pure waste** — DEC-240 records that PHP 8.5 ships an always-on URI extension (`Uri\Rfc3986\Uri`) and 8.5 IS the transpile floor, so these three may reimplement what the target already has; (b) the **`text_*` + `trim` group** overlaps DEC-385's ruled `Core.Text`→`Core.String` merge — PHP has `trim`/`strpos`/`strrev` natively, so the ASCII-oriented ones are inlinable while any grapheme-aware one is bucket-1 instead. **STILL OWED:** bucket-2's per-helper justification strings (DEC-377 requires the reason STATED for each) and one read per bucket-3 candidate before inlining. **Method disclosure:** both scans are heuristic brace-walking pre-scans, NOT the checker; each was validated against a planted positive AND against the three byte-identical ACCEPTED shapes (inner-then-outer, for-counter-then-outer, deep-then-shallow-then-outer) before being trusted — the first two scanner versions were wrong (v1 missed both positives: the type pattern did not match lowercase primitives like `int`; v2 flagged all three accepted shapes: scopes were checked before braces were processed, so a same-line `{ … }` leaked its names outward) and a SQL string literal (`WHERE id = {from}`) produced a false positive until string literals were blanked. Known remaining limit: `match`-arm bindings are not modelled | **MEASURED — 3 of 5; DEC-365 + DEC-370 owe a dev-box run** |
| DEC-413 | **The Appendix-A extension rows are currently SILENT DROPS**, which is indistinguishable from an oversight and leaves the parity denominator dishonest | **RULED — record all seven as DEFERRED (not rejected — developer's explicit choice: *"record as deferred for now"*), with their reasons attached, plus LDAP as a tracked CANDIDATE.** SOAP · IMAP (PHP itself unbundled it) · SNMP · dba + SysV IPC (contradicts the isolates+channels ruling) · pspell/enchant · ext/calendar (icu4x subsumes it) · tidy (the W4-10 HTML5 parser subsumes it). DEFERRED keeps the door open where REJECTED would have closed it; the reasons are recorded either way so the next session does not re-derive them | **RULED — rows recorded** |
| DEC-414 | **When is Q28 built** — the DEC-316 package-manager git path lost the retired vendor path's verified P6 argument hardening (`--` end-of-options, `protocol.ext.allow=never`, `ext::`/`file::` remote-helper rejection, `GIT_*` env scrub); `src/pm/fetch.rs` passes `git`/`ref` as given. Recorded as `KNOWN_ISSUES` item 4b | **RULED — WAVE 0.** It is small, self-contained, a re-port rather than a design (the guards existed and were verified once as property P6), and it is the only LIVE security regression on the tail. Slots alongside the other cheap Wave-0 gates | **RULED — Wave 0** |

### DEC-365 / DEC-378 build note (2026-07-29)

**DEC-378 BUILT.** Both halves: the `pre-commit` docs-only fast path (routes on the staged path set —
`*.rs` / `*.phg` / `Cargo.toml` / `Cargo.lock` force the full tier, everything else skips the Rust tier
and `phg format --check`), and the no-concurrent-commits rule turned from prose into an exclusive
`flock` that aborts with an explanation. Both verified behaviourally before commit (4 routing cases;
second lock holder refused).

**DEC-365 hole found and fixed the same day.** `scripts/microbench-gate.sh` skipped on *load* and on a
*missing release binary*, but gated docker on `command -v docker` — the **binary**, not the **daemon**.
The remote container has the client and no daemon, so the harness ran, failed to connect, and returned
setup-error **2**, which ABORTS the push — the exact inverse of this row's own no-hidden-loss rule, and
the reason every push in the 2026-07-29 ruling session used `--no-verify`. Now probes `docker version`
and skips loudly with the verdict recorded as OWED. Reproduced at exit 2, verified fixed at exit 0.

## Backfilled canonical rows — three DEC ids referenced everywhere but never given a row (2026-07-29)

Found mechanically by **DEC-362 guard G2** the moment it was built (it was written expecting the 13
missing rows the GR-24 sweep counted; 10 had since been filled, these 3 remained). Each row below is
reconstructed from the surviving references, and says so — none of them is a new decision.

| DEC | Item | Ruling | Status |
|----|------|--------|--------|
| DEC-186 | **Grouped member imports** — `import Pkg.{ A, B as C };`, one prefix listing several leaves, with per-member aliasing | **RULED (reconstructed from references; shipped long before this backfill): the GROUP form is admitted and expanded at PARSE time into one `Item::Import` per member**, with per-member `as` aliasing and an empty-group guard (`E-IMPORT-GROUP-EMPTY`). It is the discipline the rest of the import surface was then built to match — DEC-196 Q3's two-mode intrinsic imports cite it explicitly ("mirroring Phorj's existing type/variant-import discipline"), the DEC-384/Q-A wildcard slice reuses its parser unchanged, and variant imports (`import Core.Result.{ Success, Failure };`) are the same mechanism. [Verified: `src/parser/items/decls/imports.rs` `parse_import_group`] | **BUILT (pre-dates the backfill)** |
| DEC-197 | **Bare-import leaves for module members** — reaching a member either qualified (`String.format`) or bare after a member import (`import Core.String.format;`) | **RULED (reconstructed): both forms are legal — qualified via the module import, bare via an explicit member import.** This is the mechanism "nothing in the wind" rests on: a bare name is only reachable when an import names it. ⊳ **Superseded in scope by DEC-274**, which is the canonical row for the current rule; DEC-197 is retained here only so references to it resolve | **SUPERSEDED by DEC-274** |
| DEC-200 | **PHP-reserved and PHP-builtin class names used as top-level type names** | **CLOSED as already-ruled** — DEC-202 ruled the answer (`E-RESERVED-NAME`, covering the full keyword set *and* PHP builtin classes), so this row was stale from the moment DEC-202 landed. Recorded as closed by DEC-386's cheap-tail ruling (2026-07-26); the row itself was still missing until this backfill | **CLOSED — see DEC-202** |

## 2026-07-29 — DEC-415: entry points are ATTRIBUTE-declared; the name `main` means nothing

| ID | Question | Ruling | Status |
|----|----------|--------|--------|
| DEC-415 | **Should two entry points be an error, and is `E-MULTIPLE-MAIN` obsolete?** Surfaced by the Wave-0.4 stale-label sweep: the code has **zero emit sites** [Verified: `grep 'with_code("E-MULTIPLE-MAIN")' src/` → 0], its `phg explain` text promised *"it is rejected rather than silently picked"*, and **four comments in the AST, interpreter and compiler asserted the guarantee** (*"the checker's `E-MULTIPLE-MAIN` guarantees ≤1"*) — while a program with a top-level `main` AND a class-static `main` type-checked clean | **RULED (developer, verbatim): *"the name main means nothing! a free function or a static method needs `#[Entry(..)]` to be considered!"*** — entry detection is **attribute-driven only** — *"but the error should be multiple ENTRIES, not multiple mains"*. **And the ruling is ALREADY IMPLEMENTED**, which the asking session should have found before stopping the developer: [Verified live] two same-kind entries give *"duplicate `#[Entry(kind: EntryKind.Cli)]` — a program has at most one entry per kind"* → **`E-DUPLICATE-ENTRY-KIND`** (`src/checker/program/entry_points.rs:183`), a code already named for ENTRIES; `entry_candidates` gates on `is_entry_attr`, so the name is irrelevant; and `examples/README.md:190` already stated *"`E-DUPLICATE-ENTRY-KIND` since the `#[Entry(kind:)]` migration — the older `E-MULTIPLE-MAIN` code is no longer emitted"*. **Refinement the implementation adds and the ruling should carry: the rule is per-KIND, not per-program.** One `Cli` + one `Web` entry MAY coexist and `run`/`serve` each take their own — [Verified: five shipped examples depend on it, `examples/web/{server,core-http,handler,json-api}.phg` + `examples/session/counter.phg`], so a flat "one entry per program" rule would have broken them. **Work done (hygiene only, no behaviour change):** the dead NAME-based resolver `entry_point()`/`entry_point_count()` DELETED (zero callers — they were the source of the false guarantee); the three backend comments corrected to cite `E-DUPLICATE-ENTRY-KIND`; the `E-MULTIPLE-MAIN` explain arm rewritten to say it is RETIRED and point at the live code (kept so an old log quoting it still explains itself) | **RULED — already built; dead code removed** |

### DEC-414 / Q28 BUILD NOTE (2026-07-29) — the P6 git hardening is back

Built at Wave 0.5 as ruled. `src/pm/fetch.rs` gained `validate_git_target`, which refuses — **before any
process is spawned** — the `ext::`/`file::` double-colon REMOTE-HELPER forms (case-insensitively), a
leading `-` on either the url or the ref, and empty values; `clone` now passes `--` to end option
parsing; every invocation carries `-c protocol.ext.allow=never`; and the inherited `GIT_*` environment
is scrubbed so an ambient `GIT_SSH_COMMAND`/`GIT_CONFIG_*`/`GIT_PROXY_COMMAND` cannot hijack a fetch.

**Why this was worth the slot:** both fields come from a `phorj.json` dependency spec — i.e. from
whatever repository a user is asked to `phg install` — and git's `ext::` helper *runs a shell command*,
so `git = "ext::sh -c '…'"` was arbitrary code execution at install time.

**Two deliberate non-choices, recorded so they are not read as oversights.** (1) `--` is NOT added to
`checkout`: `git checkout -- <x>` means *"restore this path"*, so the separator would change the verb's
meaning; the leading-dash rejection covers the ref instead. (2) `file://` (the TRANSPORT) and bare local
paths remain accepted — `fetch_git` documents them as supported and hermetic tests use them; only the
double-colon helper forms are refused. A regression test pins six legitimate forms.

**Test discipline:** 6 tests, and each of the five REJECTION tests was verified to **FAIL with the guard
neutered** — they detect the gap rather than merely passing. `KNOWN_ISSUES` item 4b is closed, with one
residual recorded there for a later pass: the helper check is a DENYLIST, and an allowlist of
`https`/`ssh`/`git`/`file` transports plus bare paths would be stronger.

### PENDING QUESTION (raised 2026-07-29, NOT ruled) — is the file-layout exemption right to be `Cli`-only?

Found during the Wave-0.4 sweep while correcting the entry-point docs for DEC-415, and recorded rather
than decided (Invariant 15 — user-visible language behaviour is the developer's call).

`loader::fs::validate_public_surface` exempts entry files from the public-surface file rule (one public
type whose name is the file stem, OR public free functions, never both) — but it tests
`entry_for(prog, EntryRole::Cli)`, so the exemption is **Cli-only**. A file whose ONLY entry is
`#[Entry(kind: EntryKind.Web)]` is therefore NOT exempt and must still obey the rule.

[Verified by reading the validator and its single call site in `loader::assemble`; the check runs on the
PROJECT-assembly path, so a loose single-file `phg check` never reaches it.] **No shipped example trips
this**, so it is latent, not a live defect — the full suite is green. The asymmetry reads as unintended
(DEC-415 established that entry status comes from the attribute, and `Web` is as much an entry kind as
`Cli`), but "which entry kinds exempt a file from the layout rule" is a language-surface question.

Options when this is taken up: (a) exempt any `#[Entry]` regardless of kind; (b) keep Cli-only and say
why in the spec; (c) exempt per-kind with an explicit list. No recommendation recorded here — it needs
the developer's ruling, and it belongs in the Wave-4.4 slice (DEC-345, the package-validator work) where
the surrounding validators are already being touched.

## DEC-416 — pre-1.0 there is NO deprecation: a retired name is an unknown name (2026-07-29, **RULED**)

| DEC | Question | Ruling | Status |
|----|----------|--------|--------|
| DEC-416 | **What deprecation/retirement affordances should phorj carry before its first stable release?** Raised by the developer on seeing that the Wave-0.4 sweep had *kept* a retired `E-MULTIPLE-MAIN` explain arm "so an old build log still explains itself" | **RULED (developer, verbatim): *"There is no need for retiring error messages! just hard unknown syntax error! this is before first stable release! so no one is using it! and no one will migrate from it! if we retire something! we just change it and put it in the decisions and the compiler/interpreter will only recognize the new version that's all!"*** plus *"and just update the examples!!"*. **The rule: retire = change outright + record the decision + the compiler recognises ONLY the new form + update the examples in the same change.** No compat twin, no grace window, no migration hint, no retired-but-still-explained diagnostic. A retired name is an UNKNOWN name and produces the ordinary hard error. **Second ruling, same exchange: `W-DEPRECATED` STAYS but becomes USERLAND** — *"We should have a W-DEPRECATED that can be triggered by an explicit `#[Deprecated(message: '')]` in the .phg"* — an attribute an author puts on their own API, not an internal stdlib table. Its provider package is an OPEN sub-question the developer raised (*"we need to decide where the Deprecated will come from! the full package name!!"*) — see the PENDING row below | **RULED — swept 2026-07-29** |

**The inventory that was swept (answering the developer's *"what else are we showing deprecation/retiring for???"*).** Five affordances existed; four are DELETED:

1. **The `Core.Url` deprecated TWIN** — `src/ext/uri/url_compat.rs` kept the whole retired module registered as a parallel row-set whose own comment said *"removed after the deprecation window"*. **DELETED** — `import Core.Url;` is now a plain unknown-import error.
2. **The `Core.Url` rows in `native::deprecation_of`** — **DELETED.** The function now has no release-build rows at all; a comment records that it is NOT dead code, because the userland attribute will drive it.
3. **Three retired-but-still-explained diagnostics** — `E-MULTIPLE-MAIN` (added earlier the same day and reversed by this ruling), `E-DB-NAMING-NOT-CONST` (DEC-258), `E-TRANSPILE-FS` (DEC-313). Each arm's entire body announced its own retirement. **ALL THREE DELETED.**
4. **`phg vendor`'s bespoke retirement error** naming the DEC-316 replacement verbs, threaded through 4 sites (`cli/pm.rs`, `cli/help.rs` — it even had a HELP TOPIC — and `main.rs` ×2). **ALL DELETED**; `phg vendor` is now an unknown command like any typo. *(Invariant 10's wording "`phg vendor` is retired and errors" needs its parenthetical updated when CLAUDE.md is next edited — it now errors as unknown, not as retired.)*
5. **`docs/DEPRECATION.md` + `SEMVER.md` + `STABILITY.md`** — a full Live→Deprecated→Removed lifecycle with removal versions. **KEPT but SCOPED**: a header now states the lifecycle applies from 1.0 onward and that pre-1.0 follows this ruling. Deleting them was not ruled and they describe a genuine post-1.0 need.

Already compliant before the sweep, kept as the reference pattern: the global `println` retirement (*"a bare call now resolves as an unknown free function"*), bare `#[Entry]` (*"retired, FULLY BREAKING"*), and `for (T x in xs)` (DEC-343 kept BOTH forms, so nothing was retired at all).

**Test discipline.** The one test that asserted the old behaviour (`deprecated_core_url_warns_with_uri_module_replacement`) was not deleted but INVERTED into `retired_core_url_import_is_simply_unknown_not_deprecated`, which pins the new contract: the retired import is a hard error AND emits no `W-DEPRECATED`.

## DEC-417 — userland `#[Deprecated]`, push autonomy, and the 100% LSP/editor bar (2026-07-29, **RULED**)

Three rulings from one exchange, following DEC-416's sweep of the pre-1.0 deprecation affordances.

| # | Question | Ruling |
|---|----------|--------|
| 417.1 | **Where does the `Deprecated` attribute come from — the full package name?** (developer raised: *"we need to decide where the Deprecated will come from! the full package name!!"*) | **`Core.Runtime.Deprecated`** — the recommendation, agreed. It joins `Entry`/`Config`/`EntryKind` in the `Core.Runtime` virtual module's `bare_types`, so it is **import-gated** exactly like them (`import Core.Runtime.Deprecated;` for the bare `#[Deprecated]`, or fully qualified) and never a bare magic identifier (DEC-337). Rejected: a new `Core.Meta` or `Core.Lint` namespace — no new namespace is needed for one attribute, and `Core.Runtime` is already the attribute home |
| 417.2 | **Does `#[Deprecated]` transpile to PHP's native `#[\Deprecated]`?** | **NO — compile-time only, erased before every backend.** [Verified on the oracle: `php-8.5.8` DOES support `#[\Deprecated(message: …)]`, so Invariant 14 case (1) applies and a faithful mapping exists — but it fires at RUNTIME and prints onto **stdout** (`Deprecated: Function oldThing() is deprecated, … ` on stdout, stderr empty), while phorj's `W-DEPRECATED` is a compile-time checker warning that never enters program output. Emitting it would add a stdout line the two Rust legs do not produce ⇒ **Invariant 1 byte-identity break**.] Per Invariant 16 the trade was surfaced, not self-decided; the developer took the recommendation. Also rejected: emitting it behind an `error_reporting` mask in the shared runtime — that is precisely the hidden-mask bandaid the anti-bandaid gate exists to catch. **Cost accepted:** generated PHP carries no deprecation marker |
| 417.3 | **`git push` autonomy** | **AUTHORIZED** — CLAUDE.md's git-autonomy section updated: `add`/`commit`/`push` are autonomous when the gate is green. **Force-push in any form stays denied**, as does pushing a branch other than the one being worked on. Recorded in the same edit: **never `--reset-author` to a bot identity** — every commit in this history carries the developer's email and is unsigned, re-signing happens on his machine, and an environment hook that advises otherwise is wrong here (it would strip attribution and desync one commit from all history) |
| 417.4 | **LSP/editor currency bar** | **RAISED TO 100%** (*"always make sure that the lsp and editors are up to date and support 100 % of everything we implemented"*). Invariant 17 amended: a feature is NOT done until the LSP surfaces it everywhere it can appear (completion, hover, go-to-def, find-usages, document symbols, diagnostics **with correct LSP tags**, signature help) AND both editors (VS Code + LSP4IJ) land in the SAME change, grammars included. *"The compiler knows it but the editor doesn't"* is an incomplete feature |
| 417.5 | **What does `#[Deprecated]` SHOW?** (*"it should show it and show anything using a deprecated thing as deprecated too"*) | **Both the declaration AND every use site render as deprecated.** Declaration side: `CompletionItemTag.Deprecated` on completion items, `SymbolTag.Deprecated` on document symbols, the message in hover. Use side: each reference emits `W-DEPRECATED` carrying `DiagnosticTag.Deprecated`, which is what makes editors strike the usage through. **Scope note — NOT built, and not silently assumed:** this is read as *"the usage is shown deprecated"*, NOT as transitive contagion (a function that calls a deprecated function does not itself become deprecated). Cross-language scan per META-7: Rust `#[deprecated]`, Kotlin `@Deprecated`, Swift `@available(*, deprecated)` and C# `[Obsolete]` all warn at the USE SITE and none propagate — C# actively does the inverse, suppressing the warning when the caller is itself obsolete. If contagion IS wanted, it is a further ruling |

### DEC-417 BUILD NOTE (2026-07-29) — what shipped, and the ONE surface that could not

**Shipped:** the attribute (`Core.Runtime.Deprecated`, import-gated), collection-time harvest onto
`FnSig`, use-site `W-DEPRECATED` for free functions AND methods, the all-overloads-deprecated set rule,
`E-DEPRECATED-MESSAGE` for an interpolated or positional argument, both `phg explain` entries, the LSP's
`DiagnosticTag.Deprecated` on uses + `CompletionItemTag.Deprecated` (and the legacy boolean) on
declarations, a shipped `examples/guide/deprecated.phg` + README row, and 13 tests (8 checker, 2 CLI,
3 LSP). Byte-identity verified by hand on the example: all three legs identical, zero `deprecat*` in the
emitted PHP.

**Collateral fix.** `Display for Diagnostic` hardcoded the word "error" regardless of severity, so EVERY
warning in the language rendered as `warning: type error at 3:9: …`. Now severity-aware.

**Invariant 13.** Rather than grow a grandfathered file by the single line the new `FnSig` field needs,
`collect_enum` was extracted to `collect/enums.rs` by cohesion: `types_decls.rs` 773 → 597, burning down
176 lines of pre-existing debt.

**NOT DONE — the lift direction (`KNOWN_ISSUES` LIFT-ATTR).** Invariant 17 requires transpile AND lift in
the same change. Transpile is satisfied (deliberate erasure, ruled in 417.2). Lift is NOT: `phg lift` on
a PHP function carrying `#[\Deprecated(message: …)]` drops it silently. Root cause [Verified]:
`src/lift/lexer.rs:144` treats `#` as a line comment and skips the rest of the line, so the lifter is
blind to EVERY PHP 8 attribute, not just this one. Pre-existing, found by testing the direction rather
than assuming it, and too large to fold in here — queued as its own slice.

**Second gap, same 100%-rule family.** The LSP does not complete attribute NAMES at all (typing `#[`
offers nothing — no `Entry`, `Config`, `Route`, `Injectable`, `Deprecated`). Pre-existing and uniform
across every attribute; queued rather than special-cased for this one.

### DEC-339 BUILD NOTE (2026-07-29) — the P0 is fixed; one spec item was impossible as written

`E-SHADOW-LOCAL` implemented at `declare_binding` (`src/checker/plumbing.rs`) — the single chokepoint all
ten declaration forms funnel through, so locals, `for` counters, `for…in` loop variables, `match` arm
bindings, binding-`if`s, `catch` bindings and ctor params are covered by one rule rather than ten. The
scope search is bounded below by a new `fn_scope_floor`, raised at every function/method body and at
every lambda, which is what implements *"a lambda starts a new function"* and keeps accepted cases 19-21
legal. Bindings now carry their declaration span so the hint can name the colliding line.

**Two carve-outs, both discovered by the existing suite rather than by reasoning.** Flow narrowing
(`check_block_narrowed`) and early-return tail narrowing (`totality.rs`) install SYNTHESIZED shadows; the
author wrote no second declaration, so they route through a new `declare_narrowed` that skips the rule.
Without this the rule made narrowing reject itself — 8 tests failed and named the problem precisely.
Destructuring binds deliberately KEEP the check, being real author declarations.

**Definition-of-done item 2 was impossible as written and was NOT quietly dropped.** It asked for "a
differential example per rejected shape 1-10". The ruling chose REJECTION over alpha-renaming, so those
ten shapes no longer compile and cannot be runnable examples — the item was written when renaming was
still on the table. The coherent equivalent shipped instead: all 14 rejected shapes are pinned as checker
tests (`src/checker/tests/shadowing.rs`, 26 tests covering the full 23-row matrix), the 9 ACCEPTED shapes
get the runnable example `examples/guide/shadowing.phg` (they are the ones that must keep working, and
over-tightening is the live risk once the rule exists), and the fault class is recorded in
`examples/README.md` under Invariant 9's non-runnable-fault carve-out — which is what item 3 asks for.

**Migration cost held at the measured figure.** DEC-412 predicted exactly one in-tree site;
`examples/guide/math.phg` (`int l1` at :46, `float l1` at :54 — case 11 at a different type) was the only
failure across 270 `.phg` files. Renamed to `lg1`; stdout unchanged.

**Still owed from this slice** (tracked separately, not folded in): DEC-397's lifter hoist — the adjacent
bug in the same spec, where PHP function scope lifts to non-compiling phorj block scope. It is the same
insight from the other direction and now has a second reason to exist, since the lifter must not emit
programs this rule rejects.

### DEC-340 BUILD NOTE (2026-07-29) — the P1 is fixed; item 3 is blocked on a Ladder ruling

Items 1, 2, 4 and 5 BUILT. `db.transaction` now records the depth it finds on ENTRY and unwinds to
exactly that on both failure paths (throw and commit-failed), via a new `unwind_to_inner` that loops
`rollback_inner` — the previous single call could be consumed by a `begin()` leaked anywhere inside the
closure. Reproduced before fixing (`bal` 100 → **999** immediately after the "rolled back" transaction and
**999** after a later commit) and verified after (100 twice). The caller-owned-outer-transaction case that
made depth-0 wrong is now a test: depth 1 before, 1 after, the caller's `555` survives and commits.
`rollbackAll()` + `transactionDepth()` shipped on the prelude; 4 tests in `tests/database.rs` run every
scenario on both backends, and `examples/database/transaction-closure.phg` gained the leaked-`begin` case.

**Two guards caught real mistakes of mine.** The whole-module purity assertion in
`native::process_tests` rejected `transactionDepth` as `pure: true` — correctly, since it reads mutable
connection state and marking it pure would invite folding or reordering. And a `db_native!`-wrapped
`transactionDepth` returned a `DatabaseResult` enum where the prelude declared a plain `int`, which
surfaced as *"cannot interpolate enum into a string"*; it is now unwrapped, since reading a `Cell` cannot
fail.

**Item 3 (the PHP leg) is BLOCKED — not skipped.** The spec asked for a savepoint helper, describing the
emitter as "a literal placeholder comment". True, but incomplete: `Core.DatabaseModule` is deliberately
QUARANTINED by `E-TRANSPILE-DB` (Ladder case 2, register ~:1005), so the placeholder was UNREACHABLE and a
correct helper is equally unreachable. Making the leg live means lifting that quarantine — a case-2 →
case-1 move for the whole module — which Invariants 14 and 16 leave to the developer. The helper set was
still written (`src/transpile/db_php.rs`: per-PDO-handle depth in a `SplObjectStorage` mirroring the Rust
`Rc<Cell<u32>>`, `SAVEPOINT phorj_sp_N` with the SAME savepoint names the Rust legs emit) and the three
`php:` emitters repointed at it, replacing a `->beginTransaction()` mapping that could not express phorj's
nesting at all. Staged and ready behind the `uses_db` gate; the open question is in the spec file.

**Naming note (2026-07-29).** Everything above says `Core.DatabaseModule` because that is the AS-BUILT
name. **DEC-350 renames it** (`Core.Database`, `Connection`, `Module` suffix drops) and is RULED but
unbuilt at build-order slice 5.4, so this note will need sweeping with that rename. Recording it here
because the register already warns that as-built ≠ ruled and both must be checked before stating a name.

### DEC-350 BUILT (2026-07-29) — `Core.DatabaseModule.Database` → `Core.Database.Connection`

Built at the developer's prompting, out of build-order slice 5.4, after a session reported the AS-BUILT
name as though it were current. It was the reverse of the mistake this register already warns about, and
the rule holds either way: **as-built ≠ ruled, and both must be checked before stating a name.**

309 type renames + 482 module-path renames across 61 files (`src/`, `examples/`, `tests/`,
`docs/specs/`, `FEATURES.md`). Ordering was load-bearing: the TYPE rename ran FIRST, while the module was
still spelled `DatabaseModule` (which `\bDatabase\b` cannot match, since `M` follows), and the module
rename second — the other order would have rewritten the freshly-written `Core.Database` again.

**Deliberately NOT renamed:** `DatabaseError` / `DatabaseResult` (an error type is not the connection),
the raw `Core.Native.Database` module (the native namespace keeps its leaf), and everything under
`docs/research/` plus past `CHANGELOG` entries — those record what the name WAS, and rewriting them would
falsify the historical record the register exists to preserve.

**One mistake I made and the suite caught.** The regex guarded the dotted `Native.Database` with a
lookbehind, but not the ARRAY form `&["Core", "Native", "Database"]`, so two native module paths were
renamed to `"Connection"` — silently disabling the `E-TRANSPILE-DB` ladder gate for the raw-native leg.
`raw_native_database_import_transpile_is_a_clean_ladder_error` failed with *"expected E-TRANSPILE-DB, but
transpile succeeded"*, which is exactly the assertion that test exists for. Both paths restored.

**A second stale surface fell out of it:** `ext::registry::tests::docs_extensions_md_is_current` flagged
that the `uri` row still advertised "the deprecated `Core.Url` compat twins" — deleted by DEC-416 earlier
the same day. Row corrected and `docs/EXTENSIONS.md` regenerated.

### DEC-363 BUILT (2026-07-29) — the response-header injection guard

Reproduced first, exactly as the spec documented: `Content-Length: 2` describing a 2-byte body while ~30
further bytes followed it — injected header, early head terminator, second body. Then fixed and
re-verified on all three legs, which fault with the identical message.

**One design refinement over the spec's sketch, and it is stricter not looser.** The spec said "guard in
the phorj prelude ⇒ one implementation". Prelude phorj has NO panic-class fault primitive — no `panic`, no
`never`-returning builtin — and a checked `throw` was explicitly rejected. So the split is: the character
POLICY and the wording live in phorj (`HeaderSafety` in the `Core.Http` prelude), which is the property
that actually delivers "all three legs identical by construction"; only the fault-RAISING is a one-line
native (`Core.Native.Http.headerFault`), modelled on `Core.Test.assert`, which faults the same way and
carries the same kind of PHP twin. [Verified: no fault primitive exists — no `Ty::Never` builtin, no
`panic`/`fail`/`unreachable` native anywhere in `src/`.]

**Naming deviation, surfaced not silent.** The ruled spelling was `Http.isValidHeaderName` /
`isValidHeaderValue`. Delivering that literally requires a `class Http` inside module `Core.Http` — which
recreates precisely the leaf-equals-type namesake that DEC-278's `Module` suffix existed to avoid and that
DEC-350 dissolved for the database module hours earlier. They ship as `HeaderSafety.isValidName` /
`isValidValue` (exported in `bare_types`, so `import Core.Http.HeaderSafety;` works). **The final public
spelling is the developer's call** — options: keep `HeaderSafety`; add a `class Http` and accept the
namesake; or rename the module `Core.HttpModule` to free the name, which is the DEC-278 pattern DEC-350
was moving AWAY from.

9 tests: one per injectable surface on both Rust backends, the NUL case, a builder-smuggling case
(`Cookie.path(evil)` — proving the constructor chokepoint covers all four builders), and a clean-response
regression guard asserting an ordinary header + cookie still serialize with no split. The request-side
gate was widened to NUL in the same change with its own test, so the two directions cannot drift.

## CLAUDE-MADE DECISIONS — the autonomous judgement calls, all revisitable (2026-07-29/30)

Requested by the developer: *"note all your decisions so we might be able to revisit them later"*. Every
row below is a call **I made without a ruling** during the Wave 0/1 + DEC-416/417/350/363 work. They are
`CD-n` rather than `DEC-n` on purpose — a `DEC` is the developer's, a `CD` is mine and is a candidate for
being overturned. (The `CD-` prefix is also invisible to doc-guard G2, which tracks `DEC-nnn` rows only.)

Each row states the call, WHY, and **how to reverse it** — because a decision you cannot cheaply undo is
not really revisitable.

| ID | Decision | Why | To reverse |
|----|----------|-----|-----------|
| **CD-1** | Kept `docs/DEPRECATION.md`+`SEMVER.md`+`STABILITY.md`, scoped to post-1.0, when DEC-416 swept deprecation | The ruling was about PRE-1.0 behaviour; deleting a post-1.0 policy was not ruled and describes a real future need | Delete the three files; drop the header block added to `DEPRECATION.md` |
| **CD-2** | Read DEC-417.5 (*"show anything using a deprecated thing as deprecated too"*) as **use-site rendering**, NOT transitive contagion | Rust/Kotlin/Swift/C# all warn at the use site and none propagate; C# actively suppresses when the caller is itself obsolete (META-7 scan) | Flip `deprecation_does_not_spread_to_the_caller` in `checker/tests/deprecated.rs` and add a propagation pass; that test exists to make this one edit |
| **CD-3** | `#[Deprecated]` REJECTS an interpolated `message:` and any positional arg (`E-DEPRECATED-MESSAGE`) | Compile-time-only metadata has no runtime to evaluate holes against, so the text would be silently lost; named-only matches `#[Entry(kind:)]` | Accept them in `attributes_deprecated.rs`; decide what an interpolated message should mean |
| **CD-4** | An overload SET warns only when EVERY signature is deprecated | A set with one live overload may be what the call resolves to; warning there trains authors to ignore the channel | `deprecation_of_set` in `attributes_deprecated.rs` — one condition |
| **CD-5** | Made `Display for Diagnostic` severity-aware, changing user-visible output for EVERY warning | It hardcoded "error", so every warning read `warning: type error at 3:9:` — self-contradicting, worst on `W-DEPRECATED` | Revert `headline()`/`render_as` in `diagnostic.rs` and the `pipeline.rs` call site |
| **CD-6** | Synthesized flow-narrowing shadows bypass DEC-339 via a separate `declare_narrowed` | The author wrote no second declaration; without the carve-out the rule made narrowing reject itself (8 tests said so) | Route narrowing back through `declare_binding` — and expect those 8 tests to fail again |
| **CD-7** | Substituted DEC-339's definition-of-done item 2 | It asked for a runnable differential example per REJECTED shape; the ruling chose rejection, so those shapes no longer compile. Shipped: 26 checker tests for all 14 rejected + a runnable example for the 9 ACCEPTED + a README fault carve-out | If runnable coverage of the rejected shapes is still wanted, it needs alpha-renaming instead of rejection — i.e. reopening DEC-339 itself |
| **CD-8** | Chose the Invariant-13 split boundaries: `collect/enums.rs`, `expr/lambda.rs`, `natives/ops_tx.rs`, `lsp/tests_deprecated.rs`, `checker/program/attributes_deprecated.rs`, `transpile/db_php.rs` | Cohesion, not line count: enum vs class collection share only the receiver; a lambda IS a function boundary (why DEC-339 needed it); the tx ops are the only ones touching `tx_depth` | Each is a pure move — `git mv` the contents back and delete the `mod` line |
| **CD-9** | Extended DEC-340's entry-depth unwind to the COMMIT-FAILURE path, not just the throw path | *"Restore the depth I found"* applies symmetrically; a leaked `begin` would otherwise leave the level open after a failed commit too. **The spec only ruled the throw path — this is my extension** | Use `rollback_inner` instead of `unwind_to_inner` in the `Err(msg)` arm of `db_transaction` |
| **CD-10** | Kept `Core.DatabaseModule`-era names out of the DEC-350 rename in `docs/research/**` and past `CHANGELOG` entries | Those record what the name WAS; rewriting them falsifies the historical record the register exists to preserve | Run the same rename over those paths |
| **CD-11** | Excluded `DatabaseError`, `DatabaseResult` and `Core.Native.Database` from the DEC-350 rename | DEC-350 renamed "the TYPE" (the connection); an error type is not the connection, and the native namespace keeps its leaf | Rename them too — note `Core.Native.Database` is load-bearing for the `E-TRANSPILE-DB` gate |
| **CD-12** | Shipped DEC-363's pre-checks as `HeaderSafety.isValidName/isValidValue` instead of the ruled `Http.isValidHeaderName` | The ruled spelling needs a `class Http` inside module `Core.Http`, recreating the leaf==type namesake DEC-278's `Module` suffix existed to avoid and DEC-350 had just dissolved | Add `class Http` and accept the namesake, or rename the module `Core.HttpModule` (the DEC-278 pattern DEC-350 moved away from) |
| **CD-13** | Split DEC-363's guard: POLICY in phorj, fault-RAISING in a one-line native | Prelude phorj has no panic-class fault primitive (no `panic`, no `never` builtin) and a checked throw was rejected. Modelled on `Core.Test.assert` | Add a real fault primitive to the language, then move the raise into the prelude |
| **CD-14** | Concluded the `decimal` mapping needs NO ruling — reversing my own earlier recommendation | Measured: `bind` rejects `decimal` on both legs, so writes are already TEXT-based; and `NUMERIC` affinity destroys precision AT STORAGE before either leg sees it (PDO returned `12345678901234568`, `CAST AS TEXT` could not recover it). Identical on both legs ⇒ not a divergence | If DB `decimal` should be exact regardless of column type, that is a new feature (bind decimals as TEXT + a `DECIMAL`-affinity schema rule), not a PHP-leg question |
| **CD-15** | Sequenced the case-1 lift as 3 steps, error contract FIRST. **CORRECTED 2026-07-30: my "step 2 is ~20 emitters, mechanical" estimate was WRONG** — 3 emitters are placeholders and most others emit the bare receiver, and the immutable-`Statement`-over-mutable-`PDOStatement` mapping is a real design choice (see the spec). Step 2 is its own slice needing a ruling, not step 1's tail | The error taxonomy is what makes `catch (UniqueViolationError)` work and `db.transaction(fn, retries)` retry; savepoints were the visible-but-small part | Reorder freely — steps 2/3 are independent of each other |
| **CD-16** | Q28's git-argument guard is a DENYLIST (`ext::`/`file::`) not a transport allowlist | Re-ported the retired path's verified property P6 as-was rather than redesigning it under a security fix | Switch to an allowlist of `https`/`ssh`/`git`/`file` + bare paths; recorded as a residual in `KNOWN_ISSUES` 4b |
| **CD-17** | doc-guards: G2 (one row per DEC) is HARD, G1/G3/G4 are RATCHETED against a frozen 142-entry baseline | A hard gate on 142 pre-existing violations would have blocked every push; the ratchet stops NEW ones while the debt burns down | Delete `scripts/doc-guards-baseline.txt` to make all four hard — expect ~142 failures |
| **CD-18** | The no-concurrent-commits rule is an enforced `flock`, not a convention | It was a remembered convention and the race had already produced a spurious test failure | Remove the `flock` from `scripts/git-hooks/pre-commit` |
| **CD-19** | `transactionDepth` is `pure: false` | It reads MUTABLE connection state; marking it pure invites folding/reordering. My first attempt said `true` and the whole-module purity guard rejected it | It is guard-enforced — changing it means changing `native::process_tests` too, which is the point |

**Standing pattern worth keeping** (not a decision, an observation with evidence): on the PHP leg, RUNNING
the emitted code beat READING it three times in two sessions — `SplObjectStorage::contains()` deprecated at
the 8.5 floor (would have broken byte-identity via stdout notices), SQLite driver code 19 mis-classifying
NOT NULL as a unique violation (would have made handlers catch the wrong type), and the `decimal`
assumption above being wrong in my favour. Every PHP-leg change should be executed against
`php-8.5.8` before it is believed.


### CD-20 (2026-07-30) — `__phorj_db_try` catches ONLY `PDOException`

A `TypeError`, or a bug in an emitted expression, is not a database error. Laundering it into
`DatabaseResult.Err` would let a genuine defect be caught by `catch (DatabaseError e)` and reported to the
user as a database problem, which is strictly worse than crashing. It stays a hard fault, exactly as a
Rust-side panic does. Pinned by `db_try_does_not_launder_a_non_database_error_into_a_result`.
**To reverse:** widen the `catch` in `src/transpile/db_php.rs` — and delete that test, which exists to make
the widening deliberate.

### DEC-367 BUILT (2026-07-30) — `E-FINAL-PARENT-METHOD`

Reproduced first: `class CustomError implements Error` defining `getMessage()` type-checked **clean**, ran
`custom: boom` on the VM, and died on the PHP leg with `Fatal error: Cannot override final method
Exception::getMessage()` — Invariant 1 broken at RUNTIME, on one leg only, with `phg check` reporting
nothing. Now one diagnostic at the declaration.

Guard placed in `check_error_names`' walk (`collect/conformance.rs`), which already computes whether a
class is throwable via the transitive `class_implements` — so the scope is exactly right for free and no
second traversal was added. Renaming on emission stays REJECTED per the ruling: it would keep the program
running while silently diverging from the source, and break anything catching it as a PHP `Exception`.

**The final list came from reflection, not memory:**
`(new ReflectionClass('Exception'))->getMethods()` filtered on `isFinal()` against php-8.5.8 gives exactly
`getMessage getCode getFile getLine getTrace getPrevious getTraceAsString`. `__construct` and `__toString`
are NOT final, which is what lets a throwable keep its own constructor and `#[ToString]` — over-rejecting
those would have made `Error` subclasses unusable, so a test pins it.

**The first version OVER-rejected, and the pre-push gate caught it — not a reviewer, not me.** A
`declare class` DESCRIBES an existing PHP class instead of defining one, so declaring a signature for a
method that is final over there is exactly correct: it is how `examples/interop/exceptions.phg` binds PHP's
own `DivisionByZeroError`. The guard now skips `foreign` classes, since only a class phorj EMITS can
collide, and the commit was blocked before it landed. A regression test pins it.

5 tests: all seven names rejected, the diagnostic explains the PHP reason and offers a rename, an ordinary
method stays legal, a foreign `declare class` may declare a final method, and a NON-throwable class may
still define `getMessage` freely (it never extends
`Exception`, so nothing collides). This is the method counterpart of DEC-202's `E-RESERVED-NAME`, which
covered colliding class names but could not reach methods.

### DEC-351 BUILT (2026-07-30) — bind lifecycle reset + the D5 savepoint-portability fold-in

**Part A — the ruling itself.** `Statement` binds are now **execution-scoped**: `DbStmt::take_binds()`
(`natives/handles.rs`) `mem::replace`s the accumulator with `Binds::None`, and every one of the four
execution sites (`query`/`stream`/`exec`/`execReturningId`) resets **before** the driver call, never after
— so a FAILED execution cannot leave stale binds for the next attempt to accumulate onto.

Reproduced first, exactly as reported: a bind-in-a-loop died on iteration 2 with
`Core.Database: 2 bound value(s) but 1 ? placeholder(s) in the SQL`; it now prints `rows=3 sum=6`.
Positional and named behave identically because they share the one accumulator.

**The quadratic path went with it, measured (Invariant 11).** 8000 named binds through one prepared
statement: **4.469s → 0.054s**, against GR-13's own re-prepare baseline of 0.059s on the same box. The
reuse path is now *at* the re-prepare baseline — i.e. the cliff is gone, not merely reduced.

5 tests in `tests/database.rs` (`dec351_*`), all green on both backends, including the stale-bind case
(`a_failed_execution_does_not_leave_stale_binds`) and the guard that `executeMany` still refuses a
pre-bound statement.

**This also dissolved a blocking design question of mine.** The phorj-vs-PHP comparison the developer
asked for concluded PHP wins on statement/param lifecycle (`$s->execute([1]); $s->execute([2])` just
works) — and then DEC-351 turned out to have already *ruled* that model. With params execution-scoped on
both legs, the case-1 step-2 `__phorj_db_stmt` wrapper collapses to `[PDOStatement, sql, params[],
nextIndex]` with nothing to carry across executions.

**Part B — D5, the fold-in.** Two genuinely non-portable forms sat on the nested-savepoint path:

| Defect | Where | Why it survived |
|---|---|---|
| bare `RELEASE <name>` | `ops_tx.rs` commit, `sqlite.rs` + `postgres.rs` bulk, `db_php.rs` | Legal in SQLite and Postgres; a **syntax error in MySQL**, where the `SAVEPOINT` keyword is mandatory. The module's own `mysql.rs` already spelled it correctly — it contradicted itself |
| `ROLLBACK TO x; RELEASE x` as ONE string | `ops_tx.rs` rollback, `sqlite.rs` + `postgres.rs` bulk, `db_php.rs` | Passes through SQLite's `execute_batch`, Postgres's `batch_execute` and PDO's `exec`, all of which accept multiple statements. MySQL's `query_drop` runs ONE — and `DriverConn::control` is single-statement *by contract*, so the pair violated the contract silently on the two backends that tolerate it |

Fixed by **single-sourcing the vocabulary** (`natives/savepoint.rs`), the same discipline Invariant 4
applies to value kernels: `open` / `release` / `rollback_to` / `name(depth)` / `BULK`, emitting only the
three-dialect intersection (`SAVEPOINT n`, `RELEASE SAVEPOINT n`, `ROLLBACK TO SAVEPOINT n` — the keyword
is optional in SQLite/Postgres and mandatory in MySQL, so spelling it always is the intersection, never
the union). A full unwind is genuinely TWO statements on all three backends (rolling back to a savepoint
never pops it), so it is now two `control` calls, and the PHP leg two `exec` calls.

**The detector was written first and watched fail.** `savepoint.rs`'s test module carries a source-scan
ratchet over every file that can emit control SQL (all of `natives/` + `transpile/db_php.rs`): no bare
`RELEASE`, no bare `ROLLBACK TO`, no `;`-joined pair. On the unfixed tree it failed with three findings
pointing at the exact lines. It runs in **every** gate, which matters because the live-server coverage
does not (see below).

Coverage added: nested `commit`/`rollback`/`rollbackAll` round-trips in `tests/database_mysql.rs` and
`tests/database_postgres.rs` (env-gated on `PHORJ_MYSQL_TEST_DSN` / `PHORJ_PG_TEST_DSN`, skip-loud), plus
`a_nested_commit_releases_the_savepoint_and_keeps_its_work` in `tests/db_savepoints.rs` — which reaches
the `RELEASE SAVEPOINT` branch that **no test had ever executed** (every prior case committed at depth 1,
i.e. the real `commit()`), and is therefore precisely the branch that carried the bare spelling.

**Disclosed, not claimed:** this container has no MySQL or Postgres server (both ports probed closed), so
the two live nested-savepoint tests SKIP here. What actually ran green is the PHP leg under real
`php-8.5.8` + PDO (17/17 in `db_savepoints.rs`) and the server-free portable-form ratchet. The MySQL leg
of D5 is [Inferred from dialect grammars + the module's own `mysql.rs`, which already used these forms],
not [Verified on a wire].

Also ratcheted in the same change: `src/checker/expr/literals.rs` dropped out of
`scripts/size-baseline.txt` — the DEC-339 split took it to 488, under the hard cap, so the grandfathered
ceiling of 636 was strictly looser than the general rule. The size-gate had been reporting it as `stale=1`.

**Invariant 17:** no LSP or editor work — nothing user-visible changed (no syntax, no new diagnostic, no
new surface); this is dialect SQL inside the driver layer.

### CD-21 (2026-07-30) — extracted a savepoint vocabulary MODULE instead of patching the strings

Two string edits would have fixed the two reported defects. I built `natives/savepoint.rs` + a source-scan
ratchet instead, because the defect class is *drift between four copies of the same SQL* — and one of the
four copies (`mysql.rs`) was already right, which is what proves patching does not hold. Invariant 4
single-sources value kernels for the same reason; transaction-control SQL is a parity-affecting kernel too.
**To reverse:** inline the four call sites and delete the module; the three ratchet tests exist to make that
deletion a deliberate act rather than a quiet regrowth.

### CD-22 (2026-07-30) — the MySQL/Postgres nested-savepoint coverage is env-gated, and the gap is stated

D5 asked for nested-savepoint coverage on MySQL and Postgres. No server is reachable from this container,
so the tests are written and SKIP LOUDLY, following the existing `PHORJ_*_TEST_DSN` discipline — and the
coverage that runs in every gate is the server-free portable-form ratchet, which is the actual detector for
this defect class. Recording it as a stated gap rather than reporting "coverage added" full stop (the
NO-HIDDEN-LOSS spirit of DEC-365 applied to tests). **To reverse:** run
`PHORJ_MYSQL_TEST_DSN=… PHORJ_PG_TEST_DSN=… cargo nextest run --all-features` against live servers and
record the result here; the tests need no edit.


### DEC-361 BUILT (2026-07-30) — `src/value/faults.rs` + the derivation ratchet

**Both halves, because the ruling was explicit that single-sourcing alone would be insufficient.**

*Half 1 — one home.* `src/value/faults.rs` is now the fault-body vocabulary. It re-exports the ten
arithmetic consts (which stay in `arith.rs`, where Invariant 4 puts them, next to the kernels that raise
them) and defines every other body. The payload-carrying ones are FUNCTIONS — `panic_with`,
`assert_with`, `no_field`, `no_enum_case` — so the message *shape* is single-sourced too; a `format!`
re-typed at a call site is the same defect wearing a different hat, and `assert(cond)` vs
`assert(cond, "why")` being one function is what stops those two forms drifting apart.

**38 sites were re-inlining a body.** The scale was the finding: `"stack overflow"` at five VM sites, two
`vm::closure` sites, three interpreter sites AND a second `pub const` in `src/jit/boxed.rs` — whose own
comment said the body was *"not yet single-sourced in `value.rs` like the arithmetic faults"*, i.e. the
code documented the defect and shipped anyway. Plus `"no field …"` in five places, `"recv from empty
channel"` and `"join on an incomplete task"` in both backends, `"list index out of range"` three times.
`FaultMsg::message()` itself — the thing three call sites already treated as the single source — was
re-typing all six of its bodies.

*Half 2 — `classify` DERIVES.* `tests/differential.rs::classify` now walks a `FAULT_TABLE` whose needles
ARE the consts. It previously kept its own literals for all twelve bodies, which is why the ruling
rejected single-sourcing alone: the test that exists to catch fault-body drift was the thing hiding it.
Two ratchets keep it honest — a source scan asserting no body appears as a literal outside its
definition, and a scan of `faults.rs` asserting every `pub const FAULT_*` is classified. Adding a body
without classifying it is now a test failure naming the file and line.

**The drift the ruling predicted had already happened, and in TWO places, not one.** The PHP leg's
non-exhaustive-`match` fault:

| Leg | Body before |
|---|---|
| interpreter + VM | `non-exhaustive match at runtime` |
| PHP, `instanceof` chain | `new \UnhandledMatchError()` → `getMessage()` is the EMPTY STRING |
| PHP, native `match (true)` | PHP's own `Unhandled match case true` — a THIRD spelling |

[Verified: ran both shapes under php-8.5.8 and printed the messages.] The second one is the more
interesting miss: the register only recorded the empty-message case, and the native-`match` lowering
(both `try_native_match` and `try_match_true`) supplies its own message with no `default` arm at all.
`throw` is an expression in PHP 8, so a `default => throw new \UnhandledMatchError("<canonical>")` arm
fixes it while KEEPING the native `match` form — no fallback to the IIFE, no codegen regression. Also
graded the classification: `NonExhaustiveMatch` is its own `FaultKind` rather than folded into `Panic`,
so a future drift cannot hide behind an unrelated arm, and the four arithmetic bodies that had NO arm at
all (`FAULT_DECIMAL_OVERFLOW`/`_SCALE`, `FAULT_NEGATIVE_SHIFT`/`_EXPONENT`) got theirs — previously they
fell through to `Other(full_string)`, which compares the VM's `at L:C:` line prefix and so read a real
agreement as a divergence.

`examples/transpile/demo.php` was regenerated; the diff is exactly the one line, and all three legs still
produce byte-identical stdout (the arm is checker-unreachable, so only FAILURE behaviour changed — which
is the other half of what Invariant 1 demands).

**Invariant 13 paid down in the same change, not deferred:** the new module needs one declaration line in
`value/mod.rs` and one in `interpreter/mod.rs`, both grandfathered files. Rather than grow them, the
interpreter import was hoisted into `interpreter/mod.rs` as a `pub(super) use` so all five submodules get
it through the existing `use super::*` glob (which shortened four fully-qualified call sites and REMOVED
a line from `expr.rs` and `call.rs`), and `jit/boxed.rs` came out two lines lighter because the four-line
comment describing the un-single-sourced body was replaced by a two-line note that it now is. Two header
paragraphs were reflowed by one line each to hold the last two ceilings. size-gate `fails=0 stale=0`.

**Invariant 17:** no LSP or editor work — no syntax, no new diagnostic, no surface change. The one
user-visible difference is the *text* of a checker-unreachable fault on the PHP leg, which no editor
surfaces.

### CD-23 (2026-07-30) — `NonExhaustiveMatch` is its own `FaultKind`, not folded into `Panic`

`panic`/`todo`/`unreachable`/`assert` share one kind because they are one intrinsic family. A
non-exhaustive match is not: it is a checker-unreachable *backstop*, and DEC-361 found its body had
already drifted on the PHP leg. Sharing a kind with `Panic` would let the next drift there classify as
"both faulted the same way" against an unrelated arm. **To reverse:** merge the arm in
`differential.rs`'s `FAULT_TABLE`; nothing else depends on the distinction.

### CD-24 (2026-07-30) — the `Core.Test` assertion natives are NOT single-sourced onto `FAULT_ASSERT`

`src/ext/test/natives.rs` builds messages like `"assertion failed: expected null, got 3"`. They share a
prefix with the `assert()` intrinsic's fault but they are a different surface — a test-framework REPORT,
not a runtime fault whose body must match across three legs — and they carry their own asserted
expectations. Folding them in would couple the test module's output format to a parity const.
**To reverse:** drop `ext/test/natives.rs` from the ratchet's `DEFINITIONS` allow-list and compose them
from `faults::assert_with`.

### CD-25 (2026-07-30) — `"integer overflow in Math.abs"` COMPOSES the canonical body

Six native messages read `"integer overflow in <op>"`. They are not re-inlines to be deleted, but they
were re-typing the canonical prefix, so they now `format!("{FAULT_INT_OVERFLOW} in Math.abs")`. Editing
the canonical body therefore carries them along, which is the point. **To reverse:** inline the literals
again — but note `classify` keys on the prefix, so a divergence would silently reclassify them.


### DEC-356 PARTIAL (2026-07-30) — `walk.rs` built; the inventory RE-MEASURED and it had decayed

The ruling's central claim was that *"D alone decays — nothing prevents catch-all #19"*. Re-measuring
before building confirmed it **had already decayed, before D shipped**: **26** named catch-alls in
`src/checker/`, not the spec's 17. Five new ones (`desugar_db` +2, `rewrite_ufcs` +1,
`desugar_di/walker` +1, `rewrite_generics` +1) plus four `Ty` sites the original count never listed.
Classified by the enum each matches: 8 `Expr` · 2 `Stmt` · 1 `Pattern` · 10 `Item` · 4 `Ty` · 1 unclassified.

**Built:** `src/ast/walk.rs`'s `collect_pattern_bindings` — the site the ruling named explicitly, whose
`_ => {}` sat one line under a comment recording that this exact bug had already fired once. It now has
**named no-op arms**, not `unreachable!()`, as ruled (those forms are reachable; they just bind nothing).
`walk.rs` was 812 lines, so its inline test module split out to `walk_tests.rs` — reducing the debt rather
than squeezing comments to hold a grandfathered ceiling.

**Not built, with the reason measured rather than assumed.** `src/cli/pipeline.rs` runs `erase_tuples`
AFTER seven of the rewriters, so `Expr::Tuple` genuinely is live at their catch-alls. But the first probe
— a generic call inside a tuple — **worked on both backends**, so a static miss is not automatically a
live bug. Each of ~40 (walker × missed-variant) cells needs its own reproduction before a fix is written
(Rule 14), and each real find needs a differential case. Full per-walker table in the spec.

**Fix technique recorded so the next session does not re-derive it:** an
`e @ (Expr::A(..) | Expr::B(..) | …) => e` or-pattern arm gives the same compiler enforcement as 37
individual arms (a variant absent from the list is a non-exhaustive-match error) at ~10 lines per site —
which is what makes D compatible with Invariant 13's caps. Without it, 26 sites × 37 arms would add
~900 lines to files that must not grow.

### DEC-418 (2026-07-30, developer-ruled) — every reply ends with a `❓ QUESTION` / `⏹ NO QUESTION` marker

Developer ruling: *"without the marker I cannot tell a question from a pause — both look like prose that
stopped — so I do not know whether you are waiting on me."* Every reply's LAST line is exactly one of the
two markers; `❓` carries numbered options (recommended first, each with its own pros/cons and
after-state, plus a *"none of these / challenge the premise"* escape) and then stops, `⏹` states what is
being waited on or why work stopped. No exceptions, including one-line replies. Written into `CLAUDE.md`
so it survives session resets. It is the OUTER frame around Invariant 15's question protocol: Invariant 15
governs a question's shape, DEC-418 governs whether every reply declares itself one.

### CD-26 (2026-07-30) — the `html"…"` literal counts as a use of `import Core.Html`

Not a ruling — a bug with a forced fix, so no adjudication. Reproduced: `var a = html"<p>{n}</p>";` under
`import Core.Html;` reported `E-UNUSED-IMPORT` (*"nothing in this file references `Html` — remove the
import, or use it"*) while REMOVING the import reported `E-HTML-IMPORT` (*"`html"…"` requires the
Core.Html module"*). **Two diagnostics instructing opposite actions, with no way to write the program in
that shape** — the only form that compiled was an explicit `Html a = …` annotation, which happens to spell
the type name. Cause: `loader/import_hygiene.rs`'s scan is textual and case-sensitive, so the lowercase
literal prefix never matched the whole word `Html`. An import that GATES a literal is used by that
literal, so the scan now also accepts the module leaf lowercased + `"` — keyed generically, so a future
`xml"…"` / `sql"…"` sugar following the same convention needs no second special case.
**To reverse:** drop that branch in `import_hygiene.rs`; `an_html_literal_counts_as_a_use_of_its_import`
exists to make the removal deliberate, and `an_import_with_no_use_at_all_is_still_unused` guards the other
direction (the fix must not degrade into "never report `Core.Html` unused").


### DEC-356 BUILT (2026-07-30) — the class closed, and it was hiding a compiler PANIC

**The headline: this bug class panicked the compiler on valid user code.** `rewrite_html`'s
`leaf => leaf` arm swallowed `Expr::Tuple`, and `erase_tuples` runs AFTER `resolve_html`, so:

```
var (a, b) = (html"<p>{n}</p>", 1);
```

left the literal unresolved and reached
`unreachable!("html literal not resolved before compilation")` in `compiler/expr/core.rs`.
[Verified: ran the program with the fix stashed — thread panicked; with the fix — correct output on all
three legs.] The register had rated GR-18 a structural/hygiene item. It was a live P0.

**D — the fix.** Every `Expr`/`Stmt`/`Pattern` total walk is now exhaustive. The method mattered: rather
than guess which forms each walker missed, each catch-all was replaced with a leaf-only or-pattern and
**`rustc` enumerated the gaps** — no guessing, no reliance on the spec's (decayed) table. What it found,
per walker, was 4–6 expression-bearing forms each; `Tuple` and `NamedArg` were missed by ALL seven.

**Newly-found real gaps beyond the headline** (each [Inferred] from the code shape unless noted):
- `rewrite_html` skipped **`Item::Test`**, which carries a statement body — so an `html"…"` inside a
  `test { … }` block took the same unresolved-literal path to the same panic.
- `desugar_di`'s `Stmt` walker skipped **`Stmt::Destructure`**, which bears an initializer expression and
  an else-block, so `inject<T>()` in a destructuring initializer was never desugared. `desugar_db` walks
  the same statement correctly three files away — which is exactly what made the gap invisible.
- `ast::walk`'s two boolean scanners (`lambda_uses_this`, `uses_concurrency`) answered `_ => false` for a
  `StrPart`, i.e. "contains nothing" for any future part form.

**The leaf sets are single-sourced as MACROS** (`src/ast/leaves.rs`: `expr_leaves!`, `stmt_leaves!`,
`pattern_leaves!`). This is the design decision that made D compatible with Invariant 13: spelling 37
arms at each of 26 sites would have added ~900 lines to files already at their caps; a macro adds ~1 line
per site. Crucially it does NOT weaken enforcement — a macro expands to an or-pattern, so `rustc` still
checks exhaustiveness, whereas a `fn is_leaf(&Expr) -> bool` would have reintroduced the catch-all by the
back door. Verified by hand: adding a variant to `Expr` produced `non-exhaustive patterns:
Expr::ProbeVariant(_, _) not covered` at every fixed site.

`NewColl` and `Inject` are deliberately OUT of the macro even though they carry no `Expr`: a site that
*meaningfully handles* one would then get `unreachable_pattern` — and `desugar_di` handling `Expr::Inject`
is not a leaf case, it is the entire point of that pass. Sites treating them as pass-throughs name them
in their own or-pattern.

**C — the gate, honestly re-shaped.** The ruling asked for a never-constructed probe variant and noted
its own limitation (a match that still carries a catch-all keeps compiling). A `#[cfg(test)]` variant
cannot exist on the real `Expr` without breaking the non-test build, so the gate is inverted into
`ast::leaves::tests::no_fixed_rewriter_regrows_a_catch_all`: assert no fixed rewriter regrows an INERT
catch-all. It flags `other => other` / `leaf => leaf` / `_ => {}` / `_ => false` / `_ => true` and
deliberately NOT a catch-all that recurses (`other => Box::new(rexpr(other, m))` is total behaviour, the
opposite of the bug). Written before the last fixes, it immediately found four more sites.

**Invariant 13 paid down, substantially net-negative.** The recursion arms are real code, so six files
breached their ceilings. All six were split by cohesion — the walk trio (`rexpr`/`rstmt`/`rblock`) is each
pass's *traversal*, distinct from its analysis, and is precisely what follow-up B would consolidate:
`desugar_router` 577→238, `resolve_variant_imports` 587→168, `rewrite_generics` 680→252, `rewrite_ufcs`
503→74, `desugar_di/walker` 782→397, plus `desugar_db`'s inline tests moved out. **Four files fell under
the 500 hard cap and their grandfather entries were DELETED** (67 remain, down from 71).

**Invariant 17:** no LSP/editor work — no syntax, no new diagnostic, no surface change. The user-visible
delta is that programs which previously panicked or silently skipped a rewrite now work.

### CD-27 (2026-07-30) — `rewrite_ufcs::apply_repl` keeps its catch-all, deliberately

Its domain is **not** user AST: it dispatches on replacement shapes the CHECKER constructed. Exhaustiveness
over 37 `Expr` variants would mean two dozen arms asserting nothing about reachable code, and
`unreachable!()` would be worse — DEC-356's own headline find is a valid program hitting exactly such a
panic, so adding one on a checker path would repeat the mistake. The ratchet exempts this single arm by
name, so a SECOND catch-all in that file still fails. **To reverse:** drop the `exempt` clause in
`no_fixed_rewriter_regrows_a_catch_all` and give `apply_repl` explicit arms.

### DEC-356 FOLLOW-UP B — QUEUED, not dropped (as the ruling required)

One shared total visitor across the 13 rewriters. It is only safe NOW that D is done: with every site
carrying explicit arms, the compiler *enumerates* the blast radius a shared visitor must preserve. The
six cohesion splits landed here deliberately anticipate it — each pass's traversal now lives in its own
`*_walk.rs`, which is the seam B would replace.


### DEC-377 BUILT (2026-07-30) — the audit landed, and **bucket 3 is EMPTY**

The rule (*a `__phorj_*` helper may exist ONLY when PHP cannot do natively what phorj does*) demanded an
audit classifying every helper. DEC-377 itself said why it mattered: *"nobody currently knows which bucket
each is in, which is the same unverified-claims pattern this whole agenda has been fixing."*

**Every one of DEC-412's 17 bucket-3 candidates is REFUTED by reading it.** DEC-377's "must be INLINED"
action therefore applies to nothing. Both findings attached to that list were also wrong:

| DEC-412 said | Reading it shows |
|---|---|
| the `uri_*` trio "may be pure waste … may reimplement what the target already has" | they **already use** PHP 8.5's extension (`new \Uri\Rfc3986\Uri($raw)`). What they add is the exception→sentinel bridge phorj's `Result` surface needs — and that needs `try`/`catch`, which **is not an expression in PHP** [Verified php-8.5.8: `$x = try {…} catch {…};` is a parse error; `@` does not suppress an exception]. **Bucket 2.** |
| the `text_*` + `trim` group is "ASCII-oriented … inlinable" | the exact opposite: they exist BECAUSE PHP's calls are byte-oriented and therefore wrong. [Verified php-8.5.8: `trim()` leaves U+00A0/U+2009 in place where the helper's `/u` class strips them; `strrev("héllo")` returns mojibake.] Their Unicode class is already single-sourced as `transpile::PHP_TRIM_WS` and shared with `__phorj_http_trim`, so inlining would duplicate a parity-affecting class at N sites — DEC-361's lesson exactly. **Bucket 1.** |
| `__phorj_trim` is a bucket-3 candidate | **it does not exist.** Zero `function __phorj_trim(` definitions — a phantom from prefix-matching `__phorj_trim_start`. |

**The count was wrong three times, and is now asserted rather than claimed.** DEC-377 said 168; DEC-412
corrected to "149 real"; the true figure is **165**. Both earlier numbers came from grepping `__phorj_`
and subtracting guessed artifacts. A first careful pass here read **158** — it missed the by-reference
form `function &__phorj_rng_state()` (used by `rng_*`, `now_*`, `db_depths` to hold mutable global state)
and the checked-arith codegen table. `__phorj_unwrap` appears in comments but was inlined at M3 S2.5.

**`src/transpile/helper_buckets.rs` is the registry, with a RATCHET** —
`the_helper_registry_matches_the_source_exactly` re-derives the set from source and asserts it matches
exactly, in both directions: an unclassified helper fails, and a classified-but-deleted one fails too.
Verified live by planting `__phorj_probe_helper` (caught by name) and removing it. This is the part that
matters: DEC-377's classification was OWED for four days because it was a document with nothing keeping it
true, and DEC-356's inventory decayed 17→26 the same way. **Bucket 3 being recordable is itself a build
failure** — bucket 3 means "must be inlined", so recording one instead of inlining it would be recording
the violation.

Final classification: **68 bucket 1** (semantic necessity) · **97 bucket 2** (no single-expression
equivalent, reason stated per family as the rule requires) · **0 bucket 3**.

One self-inflicted trap worth recording: the scanner initially flagged `__phorj_trim` and `__phorj_x` —
both from this file's OWN documentation, which names `function __phorj_trim(` precisely to record that it
does not exist. Comment lines are now skipped, same as the DEC-361 ratchet.


### DEC-379 BUILT (2026-07-30) — the E-IFACE-VIS bypass, reproduced then closed

**Reproduced first** (Rule 14), with the release binary:

```phorj
interface Greeter { function greet(string who): string; }
class Impl implements Greeter {
    private function greet(string who): string { return "secret:{who}"; }
    public  function greet(int n):      string { return "n={n}"; }
}
Greeter g = new Impl();
Output.printLine(g.greet("world"));   // → secret:world
```

`phg check` said **OK**, and VM, interpreter and transpiled PHP all printed `secret:world` — a `private`
method reached through a plain interface-typed receiver. The `overloads == 1` guard meant ANY second
overload disabled the check, so a throwaway `greet(int)` was enough to switch it off.

**The ruling's wording needed a judgement call, and the code answered it.** DEC-379 says *"drop the
`overloads == 1` guard and check EVERY overload's declared visibility"*. Taken literally that rejects a
`private` NON-conforming overload too — but a shipped test,
`implementing_interface_via_a_public_overload_beside_a_private_one_is_ok`, asserts exactly that shape must
be ACCEPTED, and F-032's own analysis gives the reason (the interface is satisfied by the public
overload; the private one is nobody's business but the class's). So the implemented rule is **the overload
that CONFORMS must be public** — which closes the reproduced hole, keeps the shipped positive test valid,
and is order-independent. Both readings agree on the hole itself; they differ only on the extra
restriction, which is left to the developer (see the OPEN question below).

**Mechanism.** `ClassInfo::method_overload_vis: HashMap<String, Vec<MemberVis>>`, pushed in the same order
as `methods`' `Vec<FnSig>` so index *i* is overload *i*'s declared visibility. Conformance finds the index
whose signature matches the interface method and enforces THAT visibility. The per-signature predicate was
extracted out of `sig_conforms` as `one_sig_conforms` and single-sourced — two copies could drift, and the
visibility rule would then enforce against a different overload than the one conformance blessed. The
vector inherits on BOTH paths (trait `use` and class `extends`), or the bypass would reopen for inherited
overload sets; a missing/short vector falls back to the collapsed per-name visibility rather than silently
reading as public.

**F-032's two secondary claims did NOT survive reproduction**, and both are corrected in `KNOWN_ISSUES`:
- It rated this *"NOT a soundness/security hole"*. It was one.
- It said the PHP leg *"fatals at the class declaration"*, making it an interp≡VM-vs-PHP break. It does
  not — see CD-28.

### CD-28 (2026-07-30) — the transpiler drops per-overload visibility on the PHP leg

Found while reproducing DEC-379. An overload set is emitted as `m__ovl_0` / `m__ovl_1` … **with no
visibility modifier at all** (⇒ `public` in PHP), plus a `m(...$args)` dispatcher. So a `private`
overload's modifier is silently discarded on the PHP leg. That is why F-032's predicted PHP fatal never
happened, and why all three legs agreed on the bypass.

DEC-379's checker fix makes the *interface-implementing* case unreachable, so this is no longer a
soundness concern — but a `private` overload of a NON-interface method is still emitted public. Not
ruled: it is the kind of silent downgrade Invariant 14 forbids, so it wants a ruling rather than a quiet
fix. **To reverse/act:** emit the recorded per-overload visibility on each `__ovl_N` and give the
dispatcher the widest of the set. Recorded, not silently accepted.


### DEC-364 DESIGNED (2026-07-30) — spec written, blast radius measured, build NOT started

`docs/specs/2026-07-30-using-scope-guard.md` is the canonical design. Nothing was half-built: the variant
was added to measure the radius, then reverted, and the tree is green.

**Decided:** `Stmt::Using { ty, name, init, body, span }`; **no new `Op`, no new `Value`** — it lowers to
`try { … } finally { h.close(); }`, so all three backends reuse `Stmt::Try`'s machinery and its
already-differentialled failure ordering. The declared type is mandatory and must implement
`Core.Closable`, enforced at compile time, which is what makes the `close()` call total.

**Blast radius = 35 sites** [Verified: added `Stmt::Using`, collected every `E0004` location, reverted].
**This is the receipt for DEC-356**, landed hours earlier the same day: before it, most of those checker
walks carried `leaf => leaf`, so `Stmt::Using` would have compiled cleanly and been silently passed
through — generics erasure, DI desugaring, html resolution and UFCS would all have skipped the inside of a
`using` block, failing only when a user hit it. The 35 compile errors are the mechanical-exhaustiveness
rule doing exactly what it was built for, on the very next feature.

**OPEN QUESTION, deliberately not decided:** is `using` a **reserved** word or a **contextual** keyword?
Reserving matches C# and is simpler, but breaks any identifier spelled `using` — while DEC-344 is
simultaneously *de*-reserving `main`, i.e. the project is moving the other way. This is user-visible
surface (Invariant 15), so it is the developer's call and the build should not start without it.


### DEC-364.1 (2026-07-30, developer-ruled) — `using` is a CONTEXTUAL keyword, not reserved

Asked as the open question blocking DEC-364's build; ruled **contextual**: `using` is significant only
immediately before `(`, and reserves nothing. Rationale accepted as put — reserving would break any
existing identifier spelled `using`, and DEC-344 is simultaneously *de*-reserving `main`, so a new reserved
word cuts against the direction the project is already moving. Cost is one parser lookahead branch.

**Consequence for the build:** the lexer gains NO keyword. `using` stays an ordinary identifier token and
the parser decides at statement position, so `int using = 1;` and `using (T h = e) { … }` must BOTH parse —
each needs a test, and the pair is the regression surface for this decision.


### DEC-364 BUILT (2026-07-31) — `using` shipped on all three legs, plus two pre-existing bugs it exposed

**Shipped:** `Stmt::Using { ty, name, init, body, span }`, the contextual parser branch (DEC-364.1),
`Core.ClosableModule`'s `Closable` interface, checker enforcement, all three backends, the formatter,
the LSP + the shared editor grammar, `examples/guide/scope-guard.phg`, `FEATURES.md`, and three
`phg explain` codes. Byte-identical across `run` / `run --tree-walker` / transpiled PHP under
php-8.5.8 for every exit path [Verified: fall-through, `return`, `break` out of a loop, `continue`,
throw, and nested guards releasing inner-first — three-way `diff` empty].

**Two corrections to the design doc, both applied there:**
- **`Core.Closable` → `Core.ClosableModule`.** DEC-278 rules that a module whose leaf equals the
  single type it binds takes the `Module` suffix; `Core.Closable`/`Closable` is exactly that namesake
  collision, so the ruled convention applies and the spec's phrasing was corrected rather than the
  convention bent. Import path is `import Core.ClosableModule;`, binding the bare type `Closable`.
- **Blast radius 35 → 34 sites**, and the design's "3 editor grammar files" is **1**: both editors
  consume the same `editors/vscode/syntaxes/phorj.tmLanguage.json` (the JetBrains path is a TextMate
  bundle over that very file), so "both editors updated" is one grammar edit plus the LSP.

**One deliberate scope boundary — LIFT.** `using` is NOT lifted, because the lifter has **no
`try`/`catch`/`finally` at all**: the lift parser rejects the keyword outright and the lift printer
lists `try` as outside its subset. Raising a PHP `try { … } finally { $h->close(); }` back to `using`
is therefore blocked on the whole exception family entering the lift subset — a separate slice, not a
`using` gap. `Stmt::Using` sits behind the same documented boundary as `Stmt::Try`. Invariant 17's
"transpile AND lift in the same change" is satisfied on the transpile side and explicitly deferred,
with its reason, on the lift side.

| # | Bug found while building | Status |
|---|---|---|
| a | **`breaks_this_loop` never descended into `try`** (nor a destructure `else`), so a `break` that was a loop's ONLY exit was invisible: `function f(): int { while (true) { try { break; } finally { … } } }` type-checked clean and then returned `unit` from an `int` signature. An unsound ACCEPTANCE, live on both Rust legs | **FIXED** [Verified: reproduced before (`check` exit 0, both legs printed `got unit`), `E-MISSING-RETURN` after]. Predicate made exhaustive over `Stmt` |
| b | **Injected-prelude spans collided with user-file offsets.** The checker keys post-check rewrites (`ufcs_resolutions`, `html_resolutions`, reflect/cast substitutions, `for_bind_resolutions`, `for_iter_lowerings`) on `Span.start` alone, justified by "each call site's `(` is at a unique byte offset" — true within ONE source string, but an injected prelude is a SEPARATE string whose offsets restart at 0. A collision applied a PRELUDE's rewrite to a USER node: `phg check` clean, `--tree-walker` correct, **VM compile failed** — an Invariant 1 divergence turned on by the byte LENGTH of a prelude line | **FIXED** at the injection chokepoint (`cli::prelude_spans::lex_parse_injected` rebases each fragment's offsets above `1<<32`; `line`/`col` untouched). [Verified: adding one `import` to the DB prelude broke `examples/database/transaction-closure.phg` on the VM only; adding one trailing SPACE to that same line fixed it — offset-dependence proven, then closed]. Ratchet: `injected_prelude_spans_cannot_collide_with_user_file_offsets` |

Bug (b) is why `Connection implements Closable` could not ship until it was fixed: any prelude edit was
a coin-flip on this collision. `Connection` IS now `Closable`, so
`using (Connection db = new Connection(dsn)) { … }` closes on every exit path — closing the deferral
`src/ext/database/prelude.rs` and `KNOWN_ISSUES.md` had both recorded against DEC-203.

**Also swept (same DEC-356 class, four walkers the original sweep missed and this variant proved live):**
`rewrite_foreach::walk_stmts` + `::lower_stmt` (so `materialize_for_binds` and the Iterator lowering
now reach inside a `using` body — Invariant 7), `lsp::scope::collect_bindings` (the LSP saw neither the
`using` binding nor anything declared inside it), and `inline_parent_ctor::inline_stmt`. All four are
now exhaustive. One claim of mine was WRONG and the compiler caught it: `inline_parent_ctor` was NOT
missing `Stmt::Block` recursion (it matches `Block(b, _)`, which my grep pattern missed) — recorded
here because the mistaken version was written down before being checked.


### DEC-348 BUILT (2026-07-31) — `FileSystem.withLock`, on top of DEC-364; `tryWithLock` PENDING one ruling

**Shipped:** `FileSystem.withLock(path, fn)` — whole-file advisory locking, released on every exit path.
The implementation is the ruling's own reasoning made literal: `withLock`'s body is
`using (FileLock guard = …) { return fn()?; }`, so "release guaranteed by construction — no leak path"
is DEC-364's guarantee rather than a second one this function has to make. That is exactly why the
ruling sequenced DEC-348 after DEC-364, and it means the "needs a `try`/`finally` PHP helper" the
ruling anticipated **did not need writing**: `using` already lowers to a literal `try`/`finally` on the
PHP leg, so no new `__phorj_*` guard helper exists.

Three internal natives (`lockAcquire` / `lockTryAcquire` / `lockRelease`) + a `flock()` twin in
`transpile/fs_php.rs`. **No new `Op` and no new `Value`:** the OS lock is kept alive by a thread-local
slab and the prelude's `FileLock` carries an opaque **`int` ticket** (contrast `Core.Database`, which
needed `Value::Db`). Tickets start at 1 so `0` can mean *not acquired* across the native boundary.

**Premise re-verified before building on it** (the ruling asserted it; it is load-bearing, so it was
re-checked rather than trusted):
- `std::fs::File::{lock, try_lock, unlock}` are stable on the pinned toolchain [Verified: compiled and
  ran all three under rustc 1.97.1]. Note `try_lock` now returns `Result<(), TryLockError>`, and the
  two error arms are kept distinct: `WouldBlock` is ordinary contention (answer `0`), `Error(e)` is a
  real I/O failure that must surface as a typed `FileSystemError`. Collapsing them would report a
  permissions problem as contention.
- Rust and PHP take the SAME lock [Verified: `/proc/locks` reports `FLOCK ADVISORY WRITE` for the Rust
  holder; a Rust holder blocks a PHP `LOCK_EX|LOCK_NB` probe and a PHP holder blocks Rust's `try_lock`,
  reproducibly in both directions]. **My first probe of the Rust→PHP direction reported NO interop and
  that reading was WRONG** — the probe raced (no stdout flush before the hold), which is recorded here
  because the false negative was believed for several minutes before `/proc/locks` settled it.
- End-to-end, the lock is real on BOTH legs: with an external `flock(1)` holder, the phorj run AND the
  transpiled PHP run both BLOCK rather than acquiring (`tests/fs.rs`, asserted by deadline, not by
  sleeping).

**DISCLOSED as the ruling mandates:** everything above was verified on **Linux**. Windows is a shipped
target, its lock semantics may be **mandatory** rather than advisory, and there is no Windows CI — so
the cross-platform guarantee is `[Unverified]` and says so in `FEATURES.md`, the prelude, the example
and `src/native/fs_lock.rs`.

**One knock-on the build surfaced:** the `Core.ClosableModule` registry row had to move AFTER
`Core.FileSystemModule`. The injection fold walks `CORE_MODULES` once and can only inject a LATER row
from an earlier row's imports, so a prelude that imports `Closable` must precede it. This fails
QUIETLY (`Closable` is simply never injected and the importing prelude stops compiling), so the row now
carries that warning in a comment.

**`tryWithLock` is NOT shipped — it needs one developer ruling.** The native (`lockTryAcquire`) is
built and tested; what is undecided is the phorj-visible RETURN TYPE, which is user-visible surface and
so not mine to rule (Invariant 15). The question is recorded with the developer.

### DEC-348.1 BUILT (2026-07-31) — `tryWithLock` returns `Option<T>`; two latent defects fixed on the way

**The ruling.** Developer-ruled `Option<T>` (recommended option 1 of 5): `None` = the lock was busy,
`Some(v)` = the closure ran and returned `v`. `T?` was rejected because a busy lock and a closure that
legitimately returns null collapse to the SAME value under it — and that ambiguity type-checks clean, so
it is a trap, not a shortcut. `tests/fs.rs` asserts the load-bearing case directly: a closure returning
`null` under a FREE lock comes back as `Some(null)`, never `None`.

**A property worth recording:** contention is deterministic to demonstrate with NO second process and NO
sleep. The OS lock is per-file-DESCRIPTOR, not per-process, so a `tryWithLock` nested inside a `withLock`
on the same path opens its own descriptor and genuinely finds the lock held — by its own program. Both
the shipped example and the test use this instead of a timing race.

**Defect 1 — prelude injection was single-pass, and failed SILENTLY** (`cli::preludes::inject_core_modules`).
The fold walked `CORE_MODULES` once, so a prelude's own `import Core.X` was honoured only when `X` sat
LATER in the registry; an EARLIER row was dropped without a word. `Core.Option` is an early row and the FS
prelude now imports it, which is what surfaced it. The failure did not look like a missing type: the enum
was absent, so `Option.Some(v)` parsed as a non-pattern and the user got `unknown identifier v` pointing at
their own match arm. Fixed by running the fold to a FIXED POINT (each row still injects at most once, so a
pass that injects nothing terminates it; the second pass is a pure no-op for every existing program). The
`ROW-ORDER CRITICAL` comments on the `Core.ClosableModule` row and the registry doc-comment described a
constraint that no longer exists and are corrected, not left to rot — registry order still governs injected
ITEM order, which is a weaker and separately-stated claim. Ratchet:
`gate_tests::a_prelude_import_of_an_earlier_registry_row_is_still_injected`, verified to fail without the fix.

This also retires the workaround the `Core.Database` prelude documents (a prelude-local result carrier
instead of `Core.Result`, chosen for exactly this injection-order reason). The carrier is left in place —
changing it is not this slice — but the reason it existed is gone.

**Defect 2 — a shipped Invariant-17 violation in LSP completion** (`lsp::catalog::module_members`). It
enumerated ONLY `native::registry()`. `Core.Native.FileSystem`'s last dotted segment collides with the
friendly class name, so `FileSystem.` completion did two wrong things at once: it advertised the INTERNAL
natives `lockAcquire`/`lockRelease`/`lockTryAcquire` — precisely the leak-prone manual API this very ruling
REJECTED — and it offered neither `withLock` nor `tryWithLock`, because a prelude static with no same-named
native was invisible. So `withLock` shipped the day before breaking DEC-417's 100% bar, and no gate caught
it. Completion now unions the module's prelude-class PUBLIC statics (parsed from the registry's own prelude
source, so a new static is completable with no LSP edit) and excludes the `Core.Native.*` twins; `private`
statics stay hidden, since offering `acquireLock` would advertise the rejected shape. This also closes the
"prelude-class members (Date/Uri…) need the injected prelude program — a follow-up" gap the catalog had
documented since the 2026-07-20 alignment pass.

**The lesson for the definition-of-done checklist:** Invariant 17's LSP row was ticked for `withLock` on the
strength of "prelude members are enumerated generically", which was assumed rather than checked. The
assumption was false. A feature's LSP row is not satisfied by a plausible mechanism — it needs an assertion
that the specific new name appears.

| DEC-419 | GR-COMMENTS | Comment syntax: is `//` + `/* … */` the settled, unified surface, and should a DOC comment exist? Raised by the developer 2026-07-31, who proposed `//` for one line and `/**` for many, noting `#` is ambiguous with the `#[` attribute sigil | **RULED 2026-07-31 — (1): `/** … */` as a distinguished DOC comment, surfaced on LSP hover.** First, the state check [Verified]: the proposal was ALREADY the shipped surface — `src/tokenizer/mod.rs` starts a line comment on `//` only and a block on `/*`, and `#` was already NOT a comment (`# x` ⇒ `lex error: unexpected character '#'`), it is only the `#[` sigil. So no change was needed there and the developer's instinct matched the design. The ruling's real content was the DOC form, which did NOT exist: `CommentKind` had only `Line`/`Block`, `/**` was indistinguishable from `/*`, and NOTHING consumed comments as documentation. `/** … */` chosen over `///` because it is PHPDoc's spelling — phorj transpiles to PHP where that IS the docblock, so the same bytes mean the same thing across the boundary and a lift can read them back; `///` has no counterpart and would need translating in both directions. ONE form only, per the same 'one mechanism beats two' reasoning that rejected `defer` (DEC-364). **Two sub-questions were raised WITH the ruling and are NOT part of it** — transpile-EMIT as a PHP docblock, and lifter PHPDoc-READ. Both additive; neither built. | **BUILT 2026-07-31** — see "DEC-419 BUILT" at the end of this file |

### DEC-419 BUILT (2026-07-31) — doc comments, and the surface that was already correct

**The premise was checked before it was accepted, and half of it needed no work.** The developer proposed
`//` + `/**` and flagged `#` as ambiguous with `#[`. All of that was already true of the shipped lexer:
`//` and `/* */` lex, and `#` is a lex error outside `#[`. Reporting "already done, nothing to build"
for that half — rather than quietly re-implementing it — is the whole value of the state check.

**What genuinely did not exist** was any notion of documentation: `CommentKind` had `Line` and `Block`
only, `/**` was indistinguishable from `/*`, and no consumer read comments for docs — hover showed
signatures and nothing else. That gap is what got built.

**Design decisions inside the build, each with its reason:**
- **Single-sourced predicate.** `token::opens_doc_comment` is the ONE definition of "is this a doc
  comment", called by the tokenizer (to pick `CommentKind::Doc`) and by `lsp::docs` (to find the text
  above a declaration). Two spellings would drift, and the drift would be INVISIBLE: highlighted as
  documentation by the editor while hover showed nothing.
- **Doc comments are NOT AST nodes.** Comments live in a side channel keyed by span (what the formatter
  consumes). Attaching them would mean a field on `Function`/`Class`/`Enum`/`Trait`/`Interface`/
  `TypeAlias` plus every construction site, and the backends would carry data they can never use. Hover
  already holds the buffer text and the declaration's span, so the doc is recoverable from what is in
  hand — and the byte-identity spine is untouched by construction.
- **Attribute lines are skipped when walking upwards.** `#[Entry(…)]` sits between the doc and the
  declaration in real code; not skipping it would silently un-document every entry point.
- **A LOCAL gets no doc.** Asking for one would find the enclosing item's doc and misattribute it to a
  variable.
- **`/**/` stays an ordinary EMPTY block comment**; `/***/` counts as a doc comment with body `*`. Both
  recorded as decisions rather than left to be discovered.
- **TextMate rule ORDER is load-bearing:** the doc rule must precede the plain-block rule, because
  TextMate takes the first match and a `/\*` rule listed first would swallow `/**`. JetBrains loads the
  same grammar file (`editors/vscode/syntaxes/phorj.tmLanguage.json`), so Invariant 17's both-editors
  requirement was satisfied by that single edit — worth knowing, since it is not obvious.

**Ratchets:** 8 unit tests on the extraction (including the two negatives that make `/**` mean something
— a plain block comment and a line comment must NOT surface); 2 end-to-end LSP hover tests on the actual
JSON; 1 completion test asserting per-item that the documented decl carries `documentation` and the
undocumented one does NOT gain an empty field; 1 tokenizer test pinning all four classification corners;
1 formatter test proving the `*` column survives formatting — that column is not cosmetic, since the
extractor strips exactly one `*` per line, so a formatter that re-indented would change rendered text.

**Both sub-questions RULED YES and BUILT (2026-07-31, same day).** `transpile` re-emits a doc comment as
a PHP docblock; the lifter reads PHPDoc back into a phorj doc comment.

- **Asserted as a FIXED POINT, not as two features.** The two directions are independent code, so proving
  each alone would not prove they agree on the body text. `lifter_tests::phpdoc_round_trips_through_a_lift_and_back`
  takes PHP → phorj → PHP and asserts the same body at both ends. This is the concrete payoff of choosing
  PHPDoc's spelling over `///`.
- **The two sides key the doc differently, out of necessity.** The transpiler has the original phorj
  source, so it keys by SPAN (`ast::item_decl_span`). The lifter has NO phorj spans — it works from parsed
  PHP — so it keys by declaration NAME (`ast::item_decl_name` + `PhpProgram::docs`). Top-level names are
  unique, so the name key is total. Doc comments stay non-AST on both paths, as originally decided.
- **`emit` is preserved exactly.** The doc-bearing form is opt-in (`emit_with_source`), and a test asserts
  the two outputs differ ONLY by comment lines — so no existing caller's PHP changed.
- **The lifter's PHPDoc is a side channel keyed by token index, NOT a new `PTok` variant.** A new token
  would appear at any stream position and every existing parser site would need to learn to skip it — a
  wide change whose failure mode is silent (one missed site rejects valid PHP).
- **A plain `/* … */` is not lifted as documentation**, mirroring PHP's own convention; asserted.

**[Pre-existing limitation, found while verifying — NOT caused here and NOT fixed here]** a
phorj → PHP → phorj round-trip is not generally possible: transpiled output always contains
fully-qualified names (`\OverflowException`, emitted by the checked-arithmetic helpers) and the lifter's
Tier-1 lexer rejects `\`. My first attempt at a phorj→PHP→phorj doc test hit exactly this and I redirected
to PHP → phorj → PHP, which the lifter's tier does support, rather than report a round-trip I could not
demonstrate. Widening the lift tier is a separate matter (see the LIFT backlog).

**Invariant 13 fallout, all SPLIT rather than grown:** `ast::item_meta`, `transpile::tests_docs`,
`lift::printer::docs`, `lift::printer::setup` (all new), plus a `PParser::new` constructor that removed
the duplicated state literal at both construction sites — the split that made the code smaller, not just
rearranged.

### Shipped-example concurrency (2026-07-31) — a race in `examples/fs/lock.phg`, found by a single transient test failure

**Worth recording because the failure looked like noise.** One `cargo test --workspace --all-features`
run failed with `2128 passed; 1 failed`, and six standalone re-runs of the lib tests were clean. The
temptation was to call it flaky-and-move-on; it was a real bug in an example shipped the previous day.

**Two independent causes, both the same shape — fixed shared `/tmp` state under a CONCURRENT corpus:**
1. `examples/fs/lock.phg` reset state by DELETING its working directory, on a FIXED path. `tests/format.rs`
   runs every example through the tree-walker fanned across cores, and `tests/differential.rs` runs it as
   well, so several copies execute at once. [Verified] by hammering the pre-fix file: 16 concurrent runs →
   4 distinct outputs, with `removeDirAll: Directory not empty (os error 39)` and `appendText: … No such
   file or directory`. Fixed by serialising the whole example inside one `withLock(serial)` — the feature
   making its own example safe — with no directory and no deletion, state reset by writing content under
   the lock. Post-fix: 3 rounds × 16 concurrent → 1 distinct output each round.
2. `native::fs_lock`'s contention test used a FIXED `/tmp` path (shared between concurrently-running test
   binaries) and a `sleep(400ms)` to wait for an external `flock` holder. Under full-workspace load the
   holder sometimes had not acquired yet, so the try SUCCEEDED and the assertion failed. Fixed with a
   PID-qualified path and an observable signal (the holder `touch`es a file once it holds the lock, and
   the test waits on that with a deadline). Raising the sleep would have been a bandaid over a race.

**The generalisable lesson:** phorj has no per-process unique-path source (`Core.Process` exposes only
`arguments`/`get`/`all` — no pid), so ANY shipped example that mutates a fixed temp path is unsafe under
the test corpus. Blocking `withLock` hid this — concurrent runs merely waited — and `tryWithLock` turned
the same latent collision into a WRONG ANSWER, which is what made it visible. Any future example doing
filesystem mutation should serialise itself the same way, or not mutate shared paths at all.

| DEC-420 | GR-PHPFN | PHP builtin FUNCTION names are unguarded: a phorj `function count(…)` passes `phg check`, runs on both Rust backends, and transpiles to `Cannot redeclare function count()`. Found 2026-07-31 while writing the DEC-347 tests | **RULED 2026-07-31 — (1) MANGLE the emitted PHP name.** [Verified: a `function count(…)` program ran clean on the interpreter and the VM, and its PHP leg exited 255 with `Cannot redeclare function count()`.] This is EXACTLY the DEC-213 failure mode (`Cannot redeclare class DateTime`) with the class half fixed and the function half still open — `php_names.rs` documents itself as covering "builtin class/interface names" only. The fix would mirror DEC-213: ONE builtin-FUNCTION list read by BOTH the checker (reject, `E-RESERVED-NAME`) and the transpiler, so the reject set and the mangle set cannot drift. It is NOT self-rulable because it rejects programs that compile today — user-visible surface. Sub-question if ruled in: reject (loud, breaks existing code) vs MANGLE the emitted PHP name (silent, keeps every program working, and there is precedent — DEC-213 mangles colliding enum VARIANTS rather than rejecting them) | **BUILT 2026-07-31** — mangle at all three emit sites (definition / call / first-class-callable) through one `php_free_fn_name`; differential case runs the real PHP |

### DEC-347 BUILT (2026-07-31) — `FileSystem.lines`, and a perf loss reported rather than hidden

**The ruling's core claim was MEASURED, not asserted.** `FileSystem.lines` peaks at **23.7 MB RSS on an
84.7 MB / 1.2 M-line file**, where `readText` + `String.split` peaks at **322 MB** — 13.6x less. 23.7 MB is
the same figure DEC-347 itself cited for `Input.lines()`, which is a useful independent corroboration.
(The two legs differ by one line: the slurp counts the trailing empty element after the final newline —
exactly the off-by-one the chunk splitter drops.)

**Design, following the ruling exactly:** an offset-chunk native, NO handle. The iterator's whole state is
a byte offset in an `int` — nothing to leak, nothing to close, no `using`. A CHUNK rather than a line
because a `lineAt(path, offset)` native would `open(2)` per line, which cannot compete with `fgets` on an
already-open handle. Chunks always end on a line boundary (EXTENDING past the 64 KiB target rather than
truncating), so the prelude never stitches a partial line — the failure mode that only appears on files
big enough to cross a chunk edge.

**PERF: a confirmed 4x LOSS vs PHP `fgets`, recorded OWED per DEC-365 NO-HIDDEN-LOSS.**
- First working version: **58x slower** (295 ms vs 5.07 ms, 40k lines). Checksums matched, so the timing
  was trusted.
- Fix 1, root-caused not guessed: the prelude split each chunk itself with `List.append` per line, and
  `native::list::list_append` does `(**xs).clone()` — a FULL list copy per call, so decoding a 64 KiB
  chunk of ~1200 lines cost ~720k element clones: O(n²). Moved the split into Rust (`splitLines`).
  → 32.9 ms (9x).
- Fix 2: `List.length` is a native call and the hot path made three per LINE; cached it in a field.
  → 21.0 ms (a further 1.6x; 14x total).
- Residual **4x** (21.0 ms vs 5.2 ms) is the per-line cost of a phorj-level `Iterator` — two virtual calls
  per element — against PHP's C loop. No tuning inside this design removes it.
- **The official G-8 number is OWED, not passed:** that harness needs `php:8.5-cli` under docker and the
  docker daemon is unavailable in this container. The bench pair `bench/micro/fslines.{phg,php}` is
  committed so it runs where docker works. The local numbers used the local debug/ZTS PHP with JIT OFF,
  which FLATTERS phorj — so the true gap against release PHP+JIT is ≥4x, never less.
- Closing it needs a ruled decision, so it is NOT self-decided: a native-driven `forEachLine(path, fn)`
  (no per-element virtual calls, but new user-visible API), a JIT vertical for foreach-over-Iterator, or
  accepting the 4x for a streaming API whose selling point is memory rather than speed.

**A wrapping bug worth remembering:** `splitLines` initially used the `fs_native!` macro, which wraps every
return into a `FileSystemResult`. The prelude then got an ENUM where it expected a `List`
(`List.length expects (List<T>)` at runtime, and on the PHP leg `Cannot assign FileSystemResult_Ok to
property FileLines::$buffer of type array`). A native that CANNOT fail must not use the Result-wrapping
macro, and its `php` mapping must be a plain call rather than `wrapped!`.

### Two bugs found while building DEC-347 (2026-07-31) — neither caused by it

**1. A newline inside a string literal inside a CLOSURE was destroyed on the PHP leg — a live
Invariant-1 divergence.** The transpiler emitted literals with RAW newlines; rendering a closure body on
ONE line then turned a newline inside the literal into a SPACE. `function(): string { return "a\nb\n"; }`
printed `a\nb\n` on both Rust backends and `a b ` through PHP. Nothing caught it because no example had
put a newline-bearing literal inside a closure — the DEC-347 example was the first.

Fixed at the LITERAL (`transpile::escapes::push_control_escaped`), not at the closure emitter: control
characters now emit as PHP escapes, so a literal contains no raw newline and no downstream single-line
rendering — present or future — can corrupt one. `php_escape_bytes` already had this discipline; the two
text escapers have been brought up to it. The regression test was verified to FAIL without the fix
(`left: "a b t\tab c rlf "`).

**2. The tier-1 PHP-function gate scanned COMMENT prose as function calls.** `bareword_calls` skipped
string bodies but not comments, so any `word (` in prose was reported — `terminators (so the caller's …)`
in the DEC-347 helper's own comment tripped it, and `lock (` had done the same during DEC-348. Now more
than cosmetic: since DEC-419 a user's `/** … */` doc comment is EMITTED into the transpiled PHP, so a doc
that mentions `someFunction(x)` in prose would have failed the gate on the user's behalf. Comments are now
skipped before the scan.

### The wasm32 gate blind spot (2026-07-31) — six red playground runs the local gate called green

**What broke.** DEC-364 introduced `INJECTED_SPAN_BASE: usize = 1 << 32`. On `wasm32-unknown-unknown`
`usize` is 32 bits, so that shift overflows during const-eval and the crate does not compile:
`error[E0080]: attempt to shift left by 32_i32, which would overflow`. The playground workflow failed at
that commit and at all five pushes after it, while the full local gate — tests, both clippy passes,
release build, `--no-default-features` check — passed green every time.

**Why the gate missed it, which is the part worth keeping.** Every local step compiles for the 64-bit
host. The project's ONLY wasm32 compile lived in a GitHub workflow, so an entire target was outside the
gate's reach. This is not a discipline failure that more care would have caught: nothing local could have
observed it. A gate that cannot see a target does not gate that target.

**Fixes.** Base lowered to `1 << 28` (256 MiB — still far beyond any real `.phg`), plus a
`const _: () = assert!(…)` that proves `base + fragments * stride` is representable on the target
ACTUALLY being compiled, since the overflow is invisible on a 64-bit host. Headroom is 128 fragments
against a shipped count of 22 (counted, not guessed). Note the assertion's own first draft overflowed on
its multiplication — `checked_mul` must precede `checked_add`.

And the gate gap itself: `scripts/wasm-check.sh` in the pre-push lane, `cargo check`-ing wasm32 for the
library (`--no-default-features`; `jit` is a default feature and cranelift cannot target wasm) and for
`phorj-playground` in release — the workflow's exact configuration. `cargo check` rather than
`wasm-pack build` so it needs no wasm-pack, no node and no network.

**Generalisable lesson:** when a CI job builds a configuration no local step builds, that configuration is
ungated no matter how thorough the local gate looks.

**The audit that lesson demanded, DONE the same day — all four workflows checked against GitHub.**
`ci.yml` and `release.yml` were green through every one of today's pushes; `release.yml` runs on EVERY push
to master and its `x86_64-pc-windows-msvc` / `x86_64-apple-darwin` / `aarch64-apple-darwin` jobs all pass;
`stub-registry.yml` is tag-only. So wasm32 was the sole instance, for a reason worth stating: it is both
the only 32-bit target and the only configuration no local step compiled. The release matrix is entirely
64-bit natives, so the pointer-width class cannot bite there. What remains unverifiable locally is
platform-API BEHAVIOUR (Windows `flock` semantics — DEC-348's `[Unverified on Windows]`), and that is
already disclosed as unverified rather than assumed.

| DEC-421 | GR-LIFTEXC | A lifted PHP error path re-parses but does NOT type-check: `RuntimeException`, `LogicException`, `DivisionByZeroError` etc. have no phorj counterpart, so `phg check` reports `unknown type RuntimeException`. Surfaced 2026-07-31 by building LIFT-TRY + `throw` | **RULED 2026-07-31 — (3) MAP PHP's builtin exception hierarchy onto phorj error types.** Which means phorj ships a standard exception taxonomy; **TAXONOMY RULED 2026-07-31 — option (1): a small FLAT set in `Core.ErrorModule`** — `RuntimeError`, `LogicError`, `ArithmeticError`, `TypeError`, `ValueError`, `IoError`. Flat on purpose: no inheritance-matching subtlety, and it matches how phorj already prefixes taxonomies (`FileSystemNotFoundError`). Mirroring PHP's real `Throwable`/`Error`/`Exception` hierarchy was REJECTED — it would import PHP's much-criticised split into a language that deliberately lacks it, deciding phorj's error model as a side effect of a lift feature. [Verified: `phg lift` on a `try`/`catch`/`throw` PHP fixture emits a draft that PARSES and then fails `phg check` with `unknown type RuntimeException`.] This is the lifter's documented review-required boundary working as designed, NOT a defect — the question is whether to narrow it. Options: (a) leave it, and the human maps each exception when reviewing the draft (today's behaviour; honest, but every non-trivial error path needs hand work); (b) MAP PHP's builtin hierarchy onto phorj error types, which needs a phorj-side decision about what those types even are — phorj has an `Error` marker + user-declared errors, with no `RuntimeException` analogue, so this is really "should phorj ship a standard exception taxonomy?"; (c) emit a `// CANNOT LIFT:` note per unmapped type so the draft at least says what is missing. Not self-rulable: (b) would add user-visible stdlib surface | **BUILT 2026-08-01** — see the DEC-421 BUILT section below; THREE of the six ruled names had to change (`ArithmeticError`/`TypeError`/`ValueError` are real PHP builtin CLASSES → `E-RESERVED-NAME`) |

| DEC-422 | GR-LINESPERF | DEC-347's `FileSystem.lines` is a confirmed 4x LOSS vs PHP's `fgets` loop (21.0 ms vs 5.2 ms, 40k lines), after a measured 58x → 4x improvement. The residual is the per-line cost of a phorj-level `Iterator` — two virtual calls per element — against PHP's C loop | **RULED 2026-07-31 — BOTH (2) and (3).** (2) a native-driven `forEachLine(path, fn)` with no per-element virtual calls, and (3) a JIT vertical for foreach-over-`Iterator`. They are complementary rather than redundant: (2) fixes THIS API and can land first; (3) helps EVERY iterator in the language, including the `Iterator` implementors users write, and is the deeper win. Accepting the 4x was explicitly rejected | **(2) BUILT 2026-08-01** — `FileSystem.forEachLine`, 4.0x loss -> 1.6x; verdict still OWED (see the DEC-422(a) BUILT section). (3) the JIT vertical remains queued |

## DEC-421 — `Core.ErrorModule`, phorj's standard error taxonomy (2026-08-01, RULED + BUILT)

**Ruled** 2026-07-31 in two steps: map PHP's builtin exception hierarchy onto phorj error types
(option 3), and — the follow-up question that ruling forced — make the target *a small FLAT set*
(option 1) rather than a mirror of PHP's own hierarchy.

**Shipped:** six types injected as `Core.ErrorModule`, each an ordinary phorj class `implements Error`.
No new `Value`, no new `Ty`, nothing for a backend to learn; each transpiles to `extends \Exception`
like any other phorj error, and the existing typed-catch machinery handles them unchanged.

| type | what lands on it |
|---|---|
| `RuntimeError` | `Throwable`, `Exception`, `Error`, `ErrorException`, `RuntimeException` |
| `LogicError` | `LogicException`, `BadFunctionCallException`, `BadMethodCallException` |
| `MathError` | `ArithmeticError`, `DivisionByZeroError`, `OverflowException`, `UnderflowException`, `RangeException` |
| `TypeMismatchError` | `TypeError` |
| `InvalidValueError` | `ValueError`, `InvalidArgumentException`, `DomainException`, `LengthException`, `OutOfRangeException`, `OutOfBoundsException`, `UnexpectedValueException`, `JsonException` |
| `IoError` | *(no PHP counterpart — phorj's own; PHP throws `RuntimeException` for I/O)* |

### THREE of the six ruled names had to change — the proposal was flawed

The ruling named `ArithmeticError`, `TypeError` and `ValueError`. All three are **real PHP builtin
classes**, so `E-RESERVED-NAME` (DEC-202/213) rejects them, and rightly: transpiling
`class TypeError extends \Exception` would redeclare PHP's own. [Verified: the prelude failed to inject
with `E-RESERVED-NAME` on all three before they were renamed.] They shipped as `MathError`,
`TypeMismatchError` and `InvalidValueError`. `RuntimeError`, `LogicError` and `IoError` collide with
nothing and kept their natural names. **The proposal should have been checked against the reserved list
before the question was asked, not after the ruling** — the same class of miss as offering a name the
language cannot spell.

Named `ErrorModule`, not `Error` (DEC-278's suffix rule, applied for a concrete reason here): `Error` is
already the built-in marker interface these six implement, so a module whose qualifier leaf was `Error`
would bind that name to two different things in the same file.

### The mapping is SEMANTIC, not hierarchical

`InvalidArgumentException` lands on `InvalidValueError`, not `LogicError`. PHP files it under
`LogicException` for hierarchy reasons, but what it reports is a bad argument VALUE, and a flat set
should say what a thing means rather than where PHP filed it. `None` is a real answer: an exception with
no honest counterpart keeps its own name and the draft carries a `// CANNOT LIFT:` note, so a framework
or user-defined exception is left visibly for the human rather than coerced into the nearest phorj type.

### Lifter wiring

Both positions (`catch` clause types including every union member, and `throw new X`), plus
`import Core.ErrorModule;` and one member import per type USED — importing all six would be
`E-UNUSED-IMPORT`, a lift failing the very check it exists to pass. **[Verified] a lifted
`catch (\RuntimeException $e)` now type-checks with NO hand edits**
(`lift::lifter::exceptions::tests::a_lifted_catch_of_a_php_builtin_type_checks_with_no_hand_edits`);
three legs byte-identical on a throw/catch/dispatch path (`examples/lift/errors.phg`, whose output also
matches the original `examples/lift/errors.php` run under php-8.5.8).

### Two things found on the way

1. **The three exception walks were separate**, and the `throw new X` arm had been added to the WRONG
   one — mapped names were reported as unmappable, so a correct draft carried bogus notes. Now ONE
   `visit_exception_sites` visitor answers all three questions (`src/lift/lifter/exceptions.rs`), which
   also took `decls/statements.rs` from 438 to 260 lines (Invariant 13).
2. **A second shipped Invariant-17 hole, one level below the `withLock` one.** `import Core.` completed
   module PATHS only; a trailing `.` returned an empty list for EVERY module. A member-gated module has
   no other way in — `import Core.ErrorModule;` alone leaves its types bare (`E-INJECTED-TYPE-BARE`) —
   so the taxonomy was untypeable from the editor the day it shipped. Fixed in
   `cli::module_catalog::core_module_members`, derived from the same two registries as
   `core_module_paths`, so a new type or native is completable with no LSP edit. No editor change was
   needed: DEC-421 adds no new SYNTAX, and neither grammar hard-codes Core type names [Verified: no hit
   for `FileSystemError`/`FileSystemModule` anywhere under `editors/`].

### NOT in it — LIFT-THROWS, a new PENDING question

A lifted `throw` still needs its `throws` clause by hand. Phorj has checked exceptions and PHP does not,
so the source carries nothing to derive one from, and making a draft that CHECKS needs three
draft-visible choices: transitive `?` threading through the intra-file call graph; what to emit where a
call needs one error handled and the rest propagated (`?` is all-or-nothing, ignoring any enclosing
`try`); and whether `main` declares `throws` or gets a synthesized wrapping `try`/`catch`. Recorded in
`KNOWN_ISSUES.md` §LIFT-THROWS. Not self-ruled (Invariant 15). Also noted there: **LIFT-ECHO-INT**, the
long-standing `echo <non-string>` → `Output.print(int)` type error, which `tests/lift_roundtrip.rs` had
been working around rather than recording.


## DEC-422(a) — `FileSystem.forEachLine`, the native-driven line reader (2026-08-01, BUILT)

The first half of DEC-422's "both (2) and (3)" ruling. `forEachLine(path, fn)` reads the same lines as
DEC-347's `lines(path)` under identical terminator rules, but the loop runs inside the native — so the
two phorj-level virtual calls per element (`hasNext`, `next`) disappear, and the file is opened ONCE
rather than re-opened and `seek`ed per 64 KiB chunk (a chunk native has nowhere to keep a handle; the
DEC-347 ruling rejected a `FileHandle` type under C4).

Implemented as `NativeEval::HigherOrder` + the backend-supplied re-entrant `ClosureInvoker`, the same
mechanism `List.map` uses, so ONE body drives the interpreter and the VM — parity by construction
rather than two implementations. PHP twin `__phorj_fs_for_each_line` via `fgets` (Invariant-14 ladder
case 1: faithful, no quarantine). Three legs verified byte-identical on every shape that breaks line
readers, plus a missing file.

### MEASURED — a real improvement, and still a real LOSS (DEC-365 no-hidden-loss)

40k lines, same fixture, same fold, output-identity gated (checksum 2108890 on all legs); medians of 5:

| | ms | vs PHP |
|---|---|---|
| PHP `fgets` loop | 5.7 | 1.0x |
| phorj `forEachLine` (this) | 9.1 | **1.6x slower** |
| phorj `lines` iterator (DEC-347) | 22.8 | 4.0x slower |

So the API is **2.5x faster than the iterator** and cuts the gap against PHP from 4.0x to 1.6x — but it
does NOT win, and per DEC-365 that is recorded as an OWED verdict rather than reported as a pass. Two
caveats that both point the same way: the local `php` is a debug/ZTS build with **JIT OFF**, which
FLATTERS phorj, and the official G-8 harness needs `php:8.5-cli` under docker, whose daemon is
unavailable in this container.

### Where the residual actually is — measured, not guessed

A probe build that skips only the closure invocation (everything else identical, same binary):

| | ms |
|---|---|
| read + `Value::Str` allocation, no closure call | 4.4 |
| the same, with the closure call | 13.1 |

**The per-line closure invocation is ~2/3 of the time.** The file reading itself (4.4 ms) is within
reach of PHP's own read (measured 2.1 ms with a trivial fold). So the remaining loss is a phorj CALL
FRAME per line, not I/O.

**That reshapes what (3) has to cover.** DEC-422(3) was scoped as a JIT vertical for
foreach-over-`Iterator`, which would close the gap for `lines` but does NOT touch this path — a closure
invoked from inside a native is not an iterator virtual call. Closing `forEachLine`'s residual needs the
JIT (or the VM) to handle the native→closure call itself. Recorded here rather than discovered later.

### The API trade, stated because it is not free

`lines` STAYS. The closure body cannot `break`, cannot `return` from the enclosing function, and may
throw only `FileSystemError` — a native's parameter type is fixed in Rust and `Ty::Function`'s throws
set is covariant in the "fewer" direction only, the same restriction `withLock` carries. And because
phorj closures capture enclosing locals BY VALUE, accumulating requires a field on a holder object; a
`mutable int` assigned inside the closure silently stays 0. That last one is a property of closures
generally (`List.map` behaves identically) and is documented in FEATURES.md, but it is the first thing
a `forEachLine` caller writes, so the example and the test both demonstrate the working pattern.

The two failure channels are kept apart in the native (`ForEachEnd::{Io, Closure}`): the module's own
I/O failure is wrapped into `FileSystemResult.Err` so the prelude throws a typed `FileSystemError`,
while the closure's failure propagates untouched. Collapsing them is the obvious shortcut and would
hand a caller's own error to a `catch (FileSystemError e)` that has nothing to do with it — the silent
semantic downgrade Invariant 14 forbids.


## DEC-422 perf — the honest baseline, and why `forEachLine` still loses (2026-08-01)

Developer ruling, 2026-08-01, verbatim in substance: **"the performance and everything must beat php
best with jit is a must"**. That RAISES the Invariant-18 bar — WIN-OR-FLAG is now measured against PHP
at its best, JIT enabled, not against whatever `php` happens to be configured as. Recorded here as a
standing rule; every future perf claim in this repo is against that baseline.

### The bench was comparing against a HANDICAPPED PHP — fixed

`bench/micro/fslines.php` and `fsforeachline.php` folded each line with **`mb_strlen`**. phorj's
`String.length` is documented BYTE length, so the faithful twin is **`strlen`** — and `strlen` is
FASTER. The bench was therefore making PHP do more work than phorj and calling the result a
comparison. [Verified, JIT on, 40k lines: `mb_strlen` 4.31 ms vs `strlen` 2.52 ms median.]

**Every line-reading loss recorded before 2026-08-01 was understated**, on two counts at once (this,
and JIT being off). The DEC-347 "4x" and the DEC-422(a) "1.6x" are both superseded by the table below.
Fixed in both bench files, with the reasoning inline so it cannot silently regress.

### Current, honest numbers

Medians of 15, 40k lines, same fixture and fold, output-identity gated (checksum 2108890 every leg),
`php -d opcache.enable_cli=1 -d opcache.jit=tracing -d opcache.jit_buffer_size=64M`:

| | ms | vs PHP |
|---|---|---|
| PHP `fgets` + `strlen` (JIT ON) | 2.52 | 1.00x |
| phorj `forEachLine` | 8.59 | **3.41x slower** |
| phorj `lines` iterator | 22.34 | **8.87x slower** |

Both are OWED under DEC-365. The ruled bar is < 2.52 ms, i.e. `forEachLine` needs a **3.4x** speedup.

### Where the time goes — measured (callgrind, 40k lines, 111M Ir total)

| | share |
|---|---|
| VM execution of the closure body (`exec_op` / `run_until` / `call_closure_value` / `do_return` / stack push+pop) | ~45% |
| `malloc`/`free` | ~15% |
| the actual file read (`memchr`, `read_until`, `from_utf8`, `memcpy`) | ~14% |
| `Value::clone` + `drop_glue` | ~6% |

**Our file reading is not the problem.** The closure body — nine bytecode ops — is.

### Two experiments, both recorded because one FAILED

1. **Per-call allocations removed — KEPT.** `call_closure_value` cloned the captures into a throwaway
   `Vec` on every call, and `ClosureInvoker` took an owned `Vec` of args, so every list element / file
   line cost two heap allocations before any work happened. Captures now clone straight onto the
   operand stack and the invoker takes a borrowed slice (`&[Value]`). [Verified: 9.15 -> 8.59 ms, -6%;
   131M -> 116M Ir.] Small here, but it is the per-element path for EVERY higher-order native
   (`List.map`/`filter`/`reduce`, `Option.map`, the regex and test callbacks), so it is a win the whole
   language collects.
2. **A fast dispatch path for the cheap ops — REVERTED.** Hypothesis: `exec_op` is a ~1000-line match,
   so each op pays a large prologue; lifting `Const`/`GetLocal`/`SetLocal`/`Pop` into an
   `#[inline(always)]` helper should cut dispatch cost. Built it (single-sourced, one body per op, the
   match kept wildcard-free per Invariant 3). [Verified: 116M -> 111M Ir, **-4%**, and wall clock
   8.60 -> 8.59 ms — no measurable change.] The workload is allocator/memory bound, not
   instruction-issue bound. Reverted: Invariant 11's bar is a measured wall-clock before/after, and a
   second dispatch site that buys nothing is complexity without payment. **The finding is the value:
   VM dispatch overhead is NOT the bottleneck, so do not re-try this.**

### What beating PHP actually requires — and why it is not a slice

The closure body must stop going through the VM. The existing JIT cannot take it, for two independent
reasons that are both structural rather than missing verticals:

1. **The side-effect-free eligibility invariant.** A JIT fault falls back to re-running the function on
   the VM, which is only sound if the function has no observable effects. Our closure MUTATES a field
   (`a.total = …`) — the accumulation pattern the API requires, since phorj closures capture by value.
   Admitting it means reworking the fault/redo model, not adding an op.
2. **The unboxed kind lattice covers `Int`/`Float`/`Bool` plus string/list/map HANDLES.** The closure
   reads an object field, writes it back, and calls a native. Objects and native calls are not in the
   lattice at all.

So DEC-422(3) as originally scoped — a JIT vertical for foreach-over-`Iterator` — closes neither
number: it does not touch `forEachLine` (a closure invoked from a native is not an iterator virtual
call), and for `lines` it would still have to JIT the same ineligible body. **This is a JIT programme,
not a vertical**, and it needs a developer ruling on scope and sequencing before it starts.


## DEC-423 — the G-8 scoreboard, measured at last: 42 WIN / 9 LOSS (2026-08-01)

Follow-on from the developer's *"must beat php best with jit"* ruling. The instruction was to sweep the
whole suite against the corrected bar before aiming any more optimisation work. Doing so turned up a
piece of infrastructure rot that is more important than any individual number.

### THE HARNESS HAD BEEN DARK, AND A STALE COMMENT IS WHY

`scripts/microbench.sh` carried: *"the local builds are all ZTS DEBUG, JIT off, so they are NOT a valid
baseline"*. That is FALSE for the stack's own oracle php. [Verified on `scripts/toolchain.env`'s
php-8.5.8: `Debug Build => no`, `Thread Safety => disabled` (NTS), Zend OPcache present,
`opcache_get_status()["jit"]["on"] === true`, 128 MB buffer.]

The harness has always had a `MICROBENCH_PHP_BIN` escape hatch. Nobody used it, because the comment
said local php was worthless. Docker is absent in the dev container, so:
  * every `microbench.sh` run SKIPPED,
  * the G-8 ratchet (`microbench-gate.sh`) SKIPPED on every push, printing "OWED",
  * and the OWED backlog grew for weeks against infrastructure that was never actually missing.

**Three things the dark gate let through**, all found in one sweep:
1. **`floatloop` flipped WIN -> LOSS**: baseline 1.011, now **0.48x** (reproducible across 3 runs at
   load 0.38, and the JIT IS engaging — 836 ms with `--no-jit` vs 8.0 ms with, a 100x speedup). This is
   precisely the signal the ratchet exists to block. NOT yet attributable to a phorj regression: the
   baseline was recorded against docker `php:8.5-cli` and this is phpbrew php-8.5.8, so the two ratios
   are not interchangeable. It is a confirmed LOSS against a valid release-PHP+JIT baseline either way.
2. **`dbwork` was a PHANTOM bench.** It imported `Core.DatabaseModule.Database`/`.Statement` — an API
   that does not exist and never shipped (the real one is `Core.Database.Connection`/`Row`). It could
   not `phg check`, so it aborted the whole harness run... yet `bench/micro-baseline.json` carries a
   `dbwork` ratio. That baseline entry was fiction. Repointed at the real API; it now runs on both legs
   with a matching checksum (1529850) and is an honest **0.84x LOSS**.
3. **`fslines`, `queryparse` and `fsforeachline` are absent from the baseline entirely** — so the
   ratchet could never have gated them no matter what. `queryparse` is the worst case: DEC-338 is
   recorded as BUILT to "flip the queryparse 0.10x loss" and it is measured today at **0.13x**. The
   label says fixed; the number says 7.7x slower.

### The scoreboard

51 paired micros, `MICROBENCH_PHP_BIN` = php-8.5.8 + opcache JIT tracing, interleaved samples, both
legs pinned to one core, quiet box (load 0.07), output-identity gated. Ratio = php_ns / vm_ns; > 1 = the
VM wins. **42 WIN, 9 LOSS.**

| loss | ratio | phorj is | note |
|---|---|---|---|
| `fslines` | 0.10x | 10.0x slower | DEC-347 iterator; not in the baseline |
| `queryparse` | 0.13x | 7.7x slower | **DEC-338 is recorded BUILT/fixed — it is not** |
| `fsforeachline` | 0.27x | 3.7x slower | DEC-422(a), shipped today; not in the baseline |
| `jsonround` | 0.29x | 3.4x slower | known, queued |
| `floatloop` | 0.48x | 2.1x slower | **baseline 1.011 = a WIN->LOSS flip** |
| `deepjson` | 0.79x | 1.3x slower | known, queued |
| `dbwork` | 0.84x | 1.2x slower | revived from dead today |
| `listcontains` | 0.94x | 1.06x slower | was 0.024x; the JIT vertical nearly closed it |
| `floatmul` | 1.00x | tie | exact parity, no headroom either way |

For scale on the other side: `setunion` 48.9x, `setdifference` 33.9x, `trycatch` 27.7x, `sumby` 15.6x,
`listreduce` 14.2x, `isemail` 12.5x. **phorj is not generally slow — it is specifically slow on nine
things**, and the mandate now has a finite, named target list.

### What was NOT done, and why

The ratchet is still SKIPPING and was deliberately left that way. Arming it needs a baseline recorded
on this php, and `--emit` today would write floatloop's 0.48 in as the new normal — laundering exactly
the flip the gate exists to catch, which DEC-365's no-hidden-loss rule forbids in as many words. The
baseline question is the developer's: re-emit on local php and lose cross-box comparability with the
docker reference, or keep docker as the reference and accept the gate stays dark off-docker.


## DEC-423.1 — the G-8 ratchet ARMED, with the 9 losses frozen as OWED (2026-08-01, developer-ruled)

Follow-on ruling to DEC-423, option (1): re-emit the baseline on the local release php **and freeze the
known losses as OWED in the same change**, so `--emit` cannot launder them. Built.

### `_owed` is DERIVED, which is the whole point

`--emit` now writes a `_owed` map containing **every feature that loses at emit time**, with the ratio
it lost by. It is computed from the run, never hand-maintained — so there is no list to forget to
update and no way to emit a loss as if it were normal. A feature leaves `_owed` by being FIXED and
re-emitted, never by being edited out. That is DEC-365's no-hidden-loss rule made structural instead of
a convention someone has to remember.

The gate then, on every run:
  * REPORTS each owed feature (`owed fslines: ratio 0.114 -> 0.129 (still losing; carried, not laundered)`);
  * **BLOCKS if an owed loss DEEPENS** past `MICROBENCH_OWED_EPSILON` (default 0.75 — a 25% deepening;
    generous because these are absolute native-vs-php ratios on a shared box);
  * says **RECOVERED** and asks for a re-emit when one flips to a WIN, so the ratchet starts protecting it;
  * prints the owed count in the summary line, so a carried loss is visible on every single push.

Emitted on php-8.5.8: 51 features, **8 OWED** — `fslines` 0.114, `queryparse` 0.145, `fsforeachline`
0.286, `jsonround` 0.281, `floatloop` 0.476, `deepjson` 0.812, `dbwork` 0.884, `listcontains` 0.995.
(`floatmul` came in at ≥ 1.0 on the emit run — a genuine coin-flip at exact parity.) The baseline
records which php it was measured against (`_baseline_php`), because docker and phpbrew ratios are not
interchangeable and the previous baseline had no such marker.

### The gate now RUNS off-docker

It resolves its PHP in order: `MICROBENCH_PHP_BIN` → docker → **the oracle php from
`scripts/toolchain.env`, if it actually JITs** (probed, not assumed:
`opcache_get_status()["jit"]["on"]`). Only then does it skip. [Verified live: the gate resolves
`/stack/tools/phpbrew/php/php-8.5.8/bin/php`, reports all 8 owed losses and PASSES, in 81 s at the
pre-push default of 3 runs — next to a full test suite, two clippy passes and a release build in the
same lane.]

**The first arming attempt was WRONG, and the push it was committed on caught it.** The fallback was
gated on the docker BINARY being absent (`! command -v docker`) — but in this container the client is
installed and only the DAEMON is unreachable. So the fallback never fired, the daemon probe ran next,
and the very next push printed "docker daemon unreachable — SKIP" exactly as before: the gate was
committed as armed while still being dark. The two conditions are now one probe (`docker version`,
which talks to the daemon). The lesson is narrow and worth keeping: the JSON-seam tests exercised the
gate's DECISION logic and all passed, but nothing exercised its ENVIRONMENT resolution, and that is
where the bug was. Seam tests are not an end-to-end run.

### A near-parity wobble must not wedge pushes

Arming the gate immediately exposed a flaw in the flip check: `mapinsert` has a baseline of 1.012, and
an absolute-only band flagged it at 0.940 as a WIN→LOSS flip. That is a 7% swing on a shared box, and
it would have blocked every push. The band is now relative to the baseline as well as absolute — a
feature must be below BOTH `FLIP_EPSILON` and `baseline * MICROBENCH_RELATIVE_DROP` (0.85). A strong
WIN is unaffected: at baseline 5.0 the absolute term still binds, so a collapse to 0.9 blocks exactly
as before. [Verified: setunion 52.5 → 0.5 still FAILS; mapinsert 1.012 → 0.94 now warns.]

### The gate has tests now — it had NONE

`scripts/test-microbench-gate.sh`, wired into pre-push. The `MICROBENCH_GATE_JSON` seam was built "for
tests" and nothing used it, which is precisely how the gate managed to be dark for weeks. Seven cases,
driven through the seam (no docker, no php, no timing, ~1s, deterministic), each deriving its fixture
FROM the baseline so it cannot drift: clean run passes · clean run reports the owed losses · a deepened
owed loss blocks · a real WIN→LOSS flip blocks · a recovered owed loss passes with a re-emit note · an
output-identity break blocks · a near-parity wobble warns without blocking.

**The tests were verified to FAIL against a broken gate**, not merely to pass: neutering the
owed-deepening branch makes case 2 fail and the suite exit 1. A gate nobody tests is a gate nobody can
trust is running — that is the lesson DEC-423 paid for.

### Making it actually run in the pre-push lane, without wedging pushes

Arming the gate surfaced two more problems, both only visible end-to-end:

**It skipped on load it had caused itself.** The load guard is right — absolute native-vs-php ratios
move with load — but in the pre-push lane the load comes from the lane: the full suite, two clippy
passes and a release build run immediately before. Measured 2.78 right after `cargo build --release`,
against a 2.5 threshold, so the guard tripped essentially every time. That load is transient and
self-clearing, so the gate now WAITS for it (bounded, `MICROBENCH_SETTLE_SECS`, default 90 s, polled
every 5 s) and only then skips. Not a retry around a flaky operation — waiting for a known,
self-inflicted, self-clearing condition.

**A timing verdict could block a push falsely.** Observed live: one run at load ~1.5 (below the guard)
reported a blocking flip that did not reproduce at all on the next run. So timing-based verdicts are
now CONFIRMED before they block — the gate re-measures ONLY the flagged features (seconds, not the full
51) and blocks only on what reproduces. A real regression reproduces; load noise does not. This does
not weaken the ratchet, it removes the false positives. An output-identity break never takes this path:
it is a correctness signal, not a timing one, and blocks on sight. If the re-measure itself fails, the
suspects are reported and NOT blocked — DEC-365's rule that unmeasurable is OWED, never a block and
never a silent pass. [Verified live: forcing suspects with a tightened band drives the re-measure and
the confirmed verdicts; at defaults the gate reports 43 WIN / 8 OWED / 0 blocking and PASSES.]


## DEC-424 — `queryparse`: the layout rebuild, and why DEC-338 did not close it (2026-08-01)

First target off the DEC-423 list. DEC-338 is recorded BUILT to "flip the `queryparse` 0.10x loss" and
the sweep measured **0.13x** — so the label said fixed and the number said 7.7x slower. Worth stating
what actually happened: **DEC-338 was really done.** `Request.parse` IS nativized
(`Core.Native.Http.parseRequest`); the interpreter no longer walks that body. The nativization simply
did not address where the time goes, and nobody re-measured to find out.

### The bug: a fresh `ClassLayout` per instance

`native::http::request::inst` — the helper that hand-builds each prelude bag — called
`ClassLayout::from_sorted_names` on EVERY instance. A `ClassLayout` is a sorted `Vec<String>` plus a
name→slot hash map, and it depends only on the CLASS's field set. So a single `Request.parse` allocated
a fresh string vector, sorted it, and built a fresh hash map once per bag in the graph — `Request`,
`ParamBag`, `HeaderBag`, `AttrBag`, `FileBag`, `RequestBody`, every `Cookie`.

[Verified by callgrind, 4000 parses: malloc/free was ~38% of all instructions retired, with
`HashMap::insert` (3.7%) and `Rc<ClassLayout>::drop_slow` (1.4%) immediately behind it.] Caching the
layout per class took the bench from **1839 ms to 1177 ms (-36%)**.

### Two follow-ons, one of which is a lesson about fixing profiles

1. **The first cache used a `std::collections::HashMap`** keyed by class name — and its SipHash of that
   name promptly appeared as 3% of the profile, more than the lookup it replaced. There are under a
   dozen classes here, so a `Vec` with a linear `memcmp` scan is strictly cheaper. Replacing a cost
   with a smaller cost is still a cost; profile after every step, not just the first.
2. **`Instance::new` + a `set_field` per field takes a fresh `RefCell` borrow for EACH field** — a
   dozen borrows per bag, for an object nobody else can see yet. New `Instance::from_slots` fills the
   slot vector directly; `inst` now places values by slot and constructs once.

Together those two are worth a further **-3% of instructions** (461.5M → 446.6M) and nothing measurable
in wall clock. Kept anyway — unlike the reverted `exec_hot` experiment (DEC-423), these REMOVE work
rather than reorganise it, they are not on a hot dispatch path where a second implementation could
drift, and `from_slots` is the honest API for the "build a fresh instance" case that several natives
want. But recorded as marginal, not sold as a win.

### Result: better, still losing, still OWED

| | before | after |
|---|---|---|
| instructions (4000 parses) | 680.7M | **446.6M (-34%)** |
| wall clock (200k parses) | 1839 ms | **~1180 ms** |
| ratio vs php+JIT | 0.13x | **0.22x** |

Confirmed through the harness: `queryparse` 0.145 → **0.22**. `webish`, which also parses requests,
stays a WIN and gains too (2.85x). Still a 4.5x loss, so it stays OWED — DEC-365 unchanged.

**Why it is still losing, and what a WIN would take.** malloc/free is STILL 28.6% after both fixes.
PHP's parse builds plain arrays; phorj's builds a typed object graph — a `Request` plus six bag
instances plus every decoded `String`, each its own allocation. That is a REPRESENTATION difference,
not a tuning gap: closing it means making the bags lazy (parse the query only when `req.query` is
touched), or arena-allocating the graph, or both. Either is a design change to the rich-Request model
ruled in DEC-331 slice 2, so it is adjudicable, not something to self-decide.

**The ratchet floor was NOT re-tightened in this change.** `_owed` still records queryparse at 0.145,
so the gate would allow a slide back to 0.109 before blocking. Re-emitting after an IMPROVEMENT is the
ratchet working as intended (it is re-emitting after a REGRESSION that DEC-365 forbids), but a re-emit
rewrites every entry and the box was at load 6.58 when this landed — re-baselining the whole suite off
a loaded box is exactly how a false floor gets frozen in. Queued for a quiet box.


## DEC-425 — `floatloop`: NOT a regression, and the loss is one loop-carried sticky phi (2026-08-01)

Diagnosis before optimisation, as the DEC-423 list requires for this one. Two findings, and the second
is a ready-to-build fix that is nevertheless NOT self-rulable.

### It never regressed — the docker baseline was measuring a slower PHP

The ratchet recorded `floatloop` at **1.011 (WIN)** through 2026-07-20 and it now measures **0.48**.
That looked like the one genuine WIN→LOSS flip on the board. It is not.

[Verified by building the exact commit whose baseline recorded 1.011 — `b5ce34c`, 2026-07-20 — and
measuring it against the SAME phpbrew php-8.5.8: **phorj 9.12 ms vs php 3.95 ms, a ratio of 0.43 —
already a LOSS.**] Today's master is **7.2 ms**: phorj got *faster* over that period, not slower.

So the "flip" is entirely a baseline-ENVIRONMENT artifact: docker `php:8.5-cli` was roughly **2.3x
slower on this loop** than the local release php. **This retro-actively taints every WIN in the
pre-2026-08-01 baseline** — each was measured against that slower PHP and may be overstated by up to
that factor. The 2026-08-01 re-emit already supersedes it, which is why several formerly-"WIN" rows
(floatloop, listcontains, floatmul) read as losses or ties now. Nothing else to do about the old
numbers except not trust them; task #57 is closed by this.

### The whole loss is the speculation STICKY, and removing it WINS

[Verified: the same bench with `#[UncheckedOverflow]` runs at **~4.0 ms** against php's ~3.4–3.95 ms —
a 2x speedup that turns the loss into a WIN or a tie.] So 100% of the gap is checked int arithmetic.

The mechanism is documented in `emit_unboxed/mod.rs`'s own comment and floatloop hits it squarely:
`needs_sticky` is true when ANY reachable speculated op (`AddI`/`SubI`/`MulI`/`Neg`) is unproven, and
then the loop-carried sticky phi is emitted — *"Cranelift's baseline `opt_level=none` does NOT DCE the
loop-carried sticky phi, so omitting is what actually turns a proven counted loop's PARITY into a
WIN."*

In `floatloop` the hot counter IS proven — [Verified: `range_proven_ops` returns exactly `[24]`, the
`i = i + 1` at the loop tail]. The unproven op is `acc = acc + 1` inside `if (x > 1000000.0)`, which
executes **7 times in 5,000,000 iterations**. Its mere REACHABILITY forces the sticky phi onto every
iteration. [Verified: raising the threshold so the branch can never fire leaves the 2x completely
unchanged — 8.6–9.1 ms checked vs 3.8–4.5 ms unchecked. It is the phi, not the add.]

This is a general shape, not a bench artifact: **any counted loop with a conditional counter** pays it —
"count the matches while scanning", "tally errors while parsing". `floatloop` just makes it visible.

### The fix exists in outline and is REFUSED at a known line

Proving `acc` would set `needs_sticky` false and the loop would emit no phi at all. The machinery is
already there: `range_acc::accumulator_elision` ("bounded accumulator adds", with ENTRY GUARDS —
`param > G` ⇒ code-5 decline to the VM — for exactly the case where boundedness needs a precondition).

[Verified by probe: it returns `None` for this shape.] Traced by hand as far as: the loop structure,
the outer-counter selection (`counters = [24]`), the entry-prefix walk and the header-guard bound all
PASS, and `acc` is correctly collected as a candidate (`acc_slots = [(2, 0)]`). The refusal is inside
`verify_with_g`'s interval walk — the body carries float ops (`AddF`, a float `Gt`) and a `CallNative`,
which that walk does not model.

**Not built, and deliberately.** This is the "ONE unsound spot" the range-analysis tests name in their
own header — the guard↔increment link — and widening an overflow-elision proof is the class of change
where being wrong means silently wrong arithmetic rather than a failing test. It is also squarely
inside the JIT programme that DEC-423 says needs a scope ruling before it starts. Recorded with the
exact entry point (`verify_with_g`, float/call-carrying bodies) so the work is ready to pick up.


## DEC-426 — `jsonround` / `deepjson`: not tunable, and they lose for two DIFFERENT reasons (2026-08-01)

Next off the DEC-423 list, and the pair that does not depend on the JIT ruling. Result: **no code
change**. Two tuning attempts, both measured, both rejected — and the profiles say these are design
questions, not tuning gaps. Both stay OWED.

### `deepjson` (0.84x, 1038 ms vs php 869 ms) — 55% of it is SKIPPING

[Verified by callgrind: `skip_string` **28.3%**, `skip_value` **26.1%** (two frames) — the lazy parser
walking bytes the program never reads.]

That is DEC-294's lazy representation working as designed and still losing, because of how many times
it walks the document. Per `Json.parse` + the bench's four field reads:
  1. `validate_json` scans the WHOLE doc (required — `Json.parse` must return null for malformed input);
  2. materializing the ROOT scans it again, skipping each child's entire subtree to delimit it;
  3. materializing `data` scans the array;
  4. materializing `rec0` is small.

Roughly **three full-document scans** against PHP's single `json_decode` pass. The memo is not the
problem — `materialize_lazy` already caches via `cached.get_or_init`, so the two `topString(rec0, …)`
calls share one materialization [Verified by reading the code].

**The lazy premise does not hold at this size.** DEC-294's bet is that unread records never allocate;
at 12 records a skip-scan simply is not much cheaper than materialize-as-you-go, and we pay it three
times over. The structural fix is to have `validate_json` RECORD child offsets so step 2 disappears —
about a third of the scanning. That changes the lazy representation (an index costs memory for
documents nobody materializes), so it is a DEC-294 design question, not a tuning pass.

### Two rejected attempts, recorded so they are not retried

1. **Bulk-skip the plain run via a slice `position` instead of a per-byte `self.b.get(i)`.** Removes a
   bounds check and three comparisons per byte. [Verified: 228.6M → 223.6M Ir (**−2.2%**), wall clock
   1034 → 1038.5 ms — nothing, and the first 3-sample reading of "1015" was noise that a 7-sample
   median did not reproduce.] REVERTED, on the same rule that reverted `exec_hot` (DEC-423): fewer
   instructions with no wall-clock movement is not a win. **Why it could not help:** the strings in the
   document are 2–8 bytes (`"ok"`, `"Ada"`, `"ada@x.io"`), so the per-byte loop barely runs — the cost
   is per-STRING call overhead, not per-byte scanning.
2. **`#[inline]` on `skip_string`.** [Verified: 1105 ms vs 1038 — actively WORSE.] Reverted.

### `jsonround` (0.29x, 612 ms vs php 178 ms) — a different profile entirely

[Verified by callgrind: VM interpretation **~34%** (`exec_op` 18.4%, `run_to_completion` 9.9%, stack
push/pop 6%), malloc/free 15.6%, the parser only **11.7%**.] So unlike `deepjson`, the parser is NOT
the cost here — the cost is the VM running the bench's own phorj code.

That code is two nested seven-arm exhaustive `match`es per field read, because that is how phorj gets a
typed value out of the `Json` ADT. PHP writes `is_int($j['id'] ?? null) ? $j['id'] : 0`. The comparison
is idiomatic-to-idiomatic and therefore fair — but it also names a real **ergonomics gap**: phorj has
no `Json.getInt(key)` / `getString(key)` accessor, and PHP's `$j['id']` is exactly that. Adding one
would be both an API improvement and a large perf win (a native accessor replaces ~14 interpreted match
arms per read). It is new user-visible stdlib surface, so Invariant 15 — the developer's call, recorded
as a PENDING question rather than self-ruled.

The residual after that would still be VM-interpretation-bound, i.e. the same JIT programme DEC-423
flagged. Three of the nine losses now terminate there.


## DEC-427 — the last two losses, and the standing scoreboard (2026-08-01)

`dbwork` and `listcontains` were the only two losses not already blocked on a ruling. Both diagnosed,
neither worth a code change, and with them the whole board is now accounted for.

### `listcontains` (0.87x) is a TIE inside the noise, not a loss

[Verified: phorj 23.8 ms; `#[UncheckedOverflow]` makes no difference (23.3 vs 24.0 ms), so unlike
`floatloop` this is NOT the sticky phi; and PHP itself swings **21.4 → 31.5 ms across three
consecutive runs on a quiet box**.] The bench's own PHP leg is less repeatable than the gap being
measured. It was 0.024x before the DEC-311 JIT vertical and is now parity-with-noise — the vertical did
its job. Chasing the last few percent here would be chasing measurement error.

### `dbwork` (0.86x) terminates in the same place as everything else

[Verified by callgrind: `exec_op` 14.5% + `run_to_completion` 7.4% + stack push 3.1% = **~25% VM
interpretation**, malloc/free ~16.6%, and **`sqlite3VdbeExec` only 2.7%**.] Both legs run the same
embedded SQLite, so the engine is not the variable — the delta is phorj-level dispatch of the
prepare/bind/exec chain, once per row. Same root as `fslines`, `fsforeachline`, `jsonround`: the VM
interpreting user code.

### THE STANDING SCOREBOARD (quiet box, load 0.64, release php-8.5.8 + JIT, output-identity gated)

**42 WIN / 8 LOSS across 50 paired micros. Geometric mean 2.45x, median 2.30x — phorj is about 2.4x
faster than PHP-with-JIT across the suite.** 28 features win by 2x or more; 4 sit within ±10% of parity.

| | |
|---|---|
| biggest wins | `setunion` 50.5x · `setdifference` 33.8x · `trycatch` 27.6x · `sumby` 15.6x · `listreduce` 13.3x · `isemail` 12.1x · `isurl` 10.0x · `listfilter` 7.9x |
| the 8 losses | `listcontains` 0.87x · `dbwork` 0.86x · `deepjson` 0.85x · `floatloop` 0.46x · `fsforeachline` 0.29x · `jsonround` 0.29x · `queryparse` 0.22x · `fslines` 0.11x |

### Every remaining loss now has ONE of three named causes

1. **VM interpretation of user-level code** — `fslines`, `fsforeachline`, `jsonround`, `dbwork`, and
   `floatloop`'s variant of it. This is the JIT programme (DEC-422(b)/DEC-425), blocked on a scope
   ruling. FIVE of the eight.
2. **A representation/design choice** — `queryparse`'s typed bag graph vs PHP's plain arrays (DEC-424),
   `deepjson`'s multi-pass lazy parser (DEC-426). Both adjudicable, both recorded.
3. **Measurement noise at parity** — `listcontains`.

Nothing on the board is now unexplained, and nothing is left that a tuning pass would move. That was
the point of the DEC-423 sweep: turn "beat PHP" into a finite list with named causes.
