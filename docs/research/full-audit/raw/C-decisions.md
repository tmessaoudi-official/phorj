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
| DEC-047 | 07-01 | **No-wind closure** (design-locked, NOT implemented): fault intrinsics `panic/todo/unreachable/assert` move behind mandatory `import Core;`, called `Core.assert(...)` etc. (`E-UNIMPORTED`); deep imports `import Core.A.B.C` any depth binding bare leaf AND parent-qualified; aliasing extended to stdlib+deep; de-reserve `Attr`→Core.Html, `Error`→Core.Error, `Channel`/`Task`→**`Core.Async`** (dev rejected "Concurrent" as misnomer — tasks are cooperative, never parallel) | keep intrinsics in the wind; `Core.Concurrent` | specs/2026-07-01-no-wind-namespace-and-language-surface-design.md | ASKED | 📐 |
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
| DEC-057 | 06-21 | S5 intersections: **D1 = ≤1 concrete class + N interfaces** (dev overruled Claude's interface-only rec — correctly); `E-INTERSECT-MULTI-CLASS` for ≥2 classes; **D2 = require-agreement `E-INTERSECT-SIG`** (revisit when overloading lands) | interface-only members; first-member-wins conflict rule | m-rt plan; specs/2026-06-20-s5-intersection-types-design.md | ASKED (2 challenge rounds) | ✅ (D2 revisit still open — see CONFLICTS C-8) |
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
| DEC-094 | 06-25 | **A-6: `foreach (coll as BINDING)` adopted to REPLACE `for (x in coll)`**; one keyword `as`; 4 binding forms; optional `with int i` counter; `of`/`in` rejected as synonyms | keep `for in`; `of` keyword | same plan | ASKED | ◐ shipped **alongside** for-in, not replacing (see CONFLICTS C-2) |
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
| DEC-286 | 07-18 | **`EnumVal.payload` inline (`Payload { Zero, One(Value), Many(Vec) }`)** — every 0/1-payload enum node (all Json variants, Option/Result, the common user variant) now stores its payload INLINE, paying no per-node heap `Vec`; only 2+-field variants keep a `Vec`. Byte-identical (2279 tests + differential + php-8.5.8 oracle + all-micro output-identity); microbench-gate PASS, no WIN→LOSS flip, `enum`/`match` benches improved. A broad alloc reduction across the whole value model, single-sourced via `Payload::as_slice`. **PENDING (developer review — the "what's blocking jsonround" answer):** jsonround stays **0.29× (LOSS)** — VM 507ms vs C-`json` 145ms, a 3.4× gap. TWO byte-identical levers tried (DEC byte-cursor parse + this inline-payload) bought only ~3% because ~65% of the ~20 allocs/iter is the `Rc<EnumVal>` BOX ITSELF (one per node), the boxed-enum value model PHP's C zval-array beats structurally. Flipping jsonround needs a **value-model rebuild (arena / lazy-materialize Json nodes)** — a spine-deep architectural change to the user-visible, pattern-matched `Json` enum, possibly still short of C; **Invariant-15 developer decision, NOT autonomously attempted.** The two byte-identical wins are banked regardless | keep the per-node `Vec` (wasteful); autonomously attempt the arena rebuild (Invariant-15 violation + spine risk) | developer all-night session 2026-07-18; measured `phg benchmark`/microbench pinned | AUTONOMOUS (byte-identical perf) + PENDING (arena = ASK) |

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
| DEC-287 | 07-18 | **dbwork perf arc → AT PARITY (0.64×→~0.98× vs C PDO-sqlite)** + two OPERATIONAL notes for review (dev-requested "log inconvenient things"). PERF: 3 byte-identical levers — `prepare_cached` (rusqlite LRU stmt cache, 0.64→0.85; PDO doesn't cache), chainable `bind` returns `this` not `new Statement` (0.85→~0.95), `DbStmt.sql` String→PhStr (0.95→~0.98). Residual <1% = the per-op catchable-`DatabaseError` enum (semantically required — NOT a lever). Per the MATCH-not-beat-on-C mandate this is success; NOT claimed a >1.0 WIN (reads 0.96–0.98 under load; microbench baseline stays 0.63 until a quiet-box `--emit` re-baseline, OWED). OPS-1: **heavy full-tree cargo runs (`nextest --all-features`, `clippy --all-targets`) get SIGKILLed on this box** (load ~8, 2 terminal deaths); worked around with targeted `-E 'binary(...)'` tests + `NEXTEST_TEST_THREADS=4` + `clippy --lib` = [[heavy-cargo-runs-killed-on-this-box]]. OPS-2 (⚠ VALIDATION SCOPE): the 3 dbwork commits (`a90c4f8c`/`80e5d9b3`/`e8dd5dd3`) were validated by TARGETED db tests + the pre-commit fast tier only — the full `--all-features` suite + the two heavy pre-push sweeps (incl. `shipped_manual_example_runs_on_both_backends`, which runs `examples/db/*` on both backends) have NOT run on final HEAD since gate4 (predates all three). Isolated db-gated code, low risk, but the dev's first `pre-push` is the first FULL validation | chase the last 2% (noise + required semantic); claim a flipped WIN on loaded reads | dev all-night session 2026-07-18; advisor-certified | AUTONOMOUS (byte-identical perf) + OPS notes for review |

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
| DEC-184 | 07-04 | **Type-test operator = FULL SYMMETRY `is` + `instanceof`** (Wave A slice 3). Both operators test/narrow over primitives AND classes, interchangeably: `x is int` ≡ `x instanceof int`, `x is Circle` ≡ `x instanceof Circle`. Both flow-narrow in `if` branches. Developer chose full symmetry OVER the recommended `is`-universal-/-`instanceof`-class-only split (challenged on TIMTOWTDI + `instanceof int` having no PHP precedent; ruled symmetry anyway). Supersedes UNIFIED-SPEC's deferred `is`=identity (identity → named stdlib form later if needed). Discriminable set = match's (int/float/string/bool/null; decimal/bytes/html/attr erase → rejected); same `string`-over-erased-union byte-identity guard | is-universal + instanceof-class-only (recommended, declined); is=identity (spec, superseded) | MASTER-PLAN §0/§13.2 Wave A slice 3 | ASKED (dev challenged, ruled symmetry) | 📐 (slice 3) |
| DEC-183 | 07-04 | **Flat wildcard-free `match` over `T?` is exhaustive** — `Optional<T>` treated as `T \| null` for match totality: member arms + a `null` arm discharge it, no `_` needed (`int?`, `Circle?`, `(A\|B)?`). Completion of slice-1 (null already discriminable); byte-identity holds (`is_int`/`is_null`/`=== null`, pattern-driven). Bounded caveat: `Optional<enum>` (`Color?`) still needs `_` until enum-variant coverage is threaded through `?` (separate follow-up). Surfaced PENDING by Wave A slice 2, ruled Option A | keep requiring `_`/smart-cast (Option B) | MASTER-PLAN §0/§13.2 Wave A slice 2b | ASKED (dev asked for recommendation, then ruled A) | ✅ (slice 2b) |

---

## CONFLICTS (contradictory records — adjudicate)

| # | Conflict | Trace | Status |
|---|----------|-------|--------|
| **C-1** | **D-L3 (06-18) REJECTED multiple inheritance** ("realized as traits/mixins + interfaces") — yet **S6 shipped real MI** (`class C extends A, B`) *and* S8 shipped traits. | Traced: 06-18 D-L3 reject (next-intuitive-features spec) → 06-21 dev: "multi-inheritance wanted, real game changer, WITHOUT removing traits" (ga-direction memory) → 06-22 dev rejected the single+traits framing **twice**, demanded research → S6 Model-1 explicit-resolution MI ASKED + shipped; Model-3 C3 deferred. So: a legitimate developer reversal, properly recorded each step — but **D-L3's text was never amended**, so the two specs still contradict. | Developer-driven supersession; needs doc reconciliation, not re-adjudication. |
| **C-2** | **A-6 (06-25) adopted `foreach (coll as …)` to REPLACE `for (x in coll)`** ("free `for` for C-style only") — but commit `0747385` (06-26) shipped foreach **"alongside the typed `for (T x in xs)` form"**; examples still use for-in everywhere; FEATURES.md lists `for … in` ✅ with no replacement note; B1 (06-30) *extended* for-in (string/Map two-binding). | The decided replacement was silently softened into an addition during an autonomous slice. Either the decision or the implementation is wrong. [Verified: both forms parse today.] | **Open — adjudicate** (keep both / execute the replacement / amend A-6). |
| **C-3** | **Zero-dep locked framing (06-26): "NO TLS, NO regex, NO http/serde crates, `[dependencies]` empty, verified"** — days later `regex` admitted as dep #2, plus argon2/ctrlc/corosensei (4 deps total). | Each dep individually developer-authorized under the 06-27 dependency policy; but the 06-26 "LOCKED FRAMING" text (native-modules-research plan) explicitly names regex as forbidden and was never updated. | Superseded-in-practice; framing doc stale. |
| **C-4** | **`text` leaf chosen 06-18 explicitly "not `string` (avoids shadowing the `string` type)"** — naming overhaul (06-30) renamed `Core.Text` → **`Core.String`**. | The original rationale (shadowing) is mooted by PascalCase (`String` ≠ `string`), but no record shows the old rationale being revisited when the rename was made. | Likely fine; confirm the shadowing concern was consciously dismissed. |
| **C-5** | **Ternary: two same-day records disagree (06-24)** — perimeter spec says "ternary ✅ add"; master plan says "DEFERRED, not rejected" (postfix-`?` collision + expression-if coverage). | [Verified: `? :` is a parse error in the current binary → DEFERRED won.] The perimeter spec was never corrected. | Resolved in practice; fix the stale record. |
| **C-6** | **M6 design (06-18): OS-thread pools "off the table"** (Rc heap) — yet **M6 W3 shipped an OS-thread pool for `phg serve`** (memory: m6-w3-serve-concurrency), later superseded by green threads. | The W3 pool isolated per-connection state so it didn't share `Value`s, but it contradicts the design's blanket statement; superseded anyway by DEC-132. | Historical; no action beyond doc note. |
| **C-7** | **CLAUDE.md/docs still document `phg bench`, `phg disasm`, `phg fmt`** while DEC-113 renamed the CLI verbs to `benchmark`/`disassemble`/`format`/`tokenize`. | Doc drift from the unpushed naming overhaul; e.g. project CLAUDE.md instructs `phg bench <file>`. | Doc reconciliation task. |
| **C-8** | **`E-INTERSECT-SIG` (require-agreement) was decided with "revisited when overloading lands"** — overloading landed (param + return-type); no record shows the revisit happening. | m-rt plan D2 note vs overloading completion. | **Open — adjudicate** (allow intersections whose shared method differs per overloading rules?). |
| **C-9** | **"Nothing in the wind" (06-18) vs shipped import-free intrinsics** — `panic`/`todo`/`unreachable`/`assert` shipped usable with no import, violating the standing principle for weeks. | Caught by the developer 07-01; fix designed (DEC-047: `import Core;`) but NOT implemented. | Designed fix pending implementation. |
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
    `tests/db.rs` (9 native unit + 6 phorj fixtures), `examples/db/transactions.phg`. `run ≡ runvm`;
    spine-quarantined (impure); nothing-in-the-wind (subtypes member-gated in `bare_types`).
    - **PENDING (Invariant 15) — the closure form `db.transaction(() => { … })` + closure `retry`.** NOT a
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
  deferred (no empty-set VM op → would need a new `Op`). **PART-2 PENDING**: remove the empty-`[]`
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
  - **PENDING adjudication (Invariant 15) — retry SURFACE.** The spec (§5) illustrates one method
    `db.transaction(retries: N, fn)`, but the language supports NEITHER named args, NOR method default
    params, NOR generic-method overloading — three independent walls that make a single generic
    `transaction` carrying an optional `retries` impossible. Realized as a distinct
    `db.transactionRetry(fn, retries)` (retries trailing, positional). *Alternatives (all unbuildable):*
    (a) `transaction(fn, retries = 0)` — `E-DEFAULT-PARAM-CONTEXT` (methods can't default); (b)
    `transaction(fn)` + `transaction(fn, retries)` overload — `E-OVERLOAD-GENERIC` (generic methods can't
    overload); (c) `transaction(retries: N, fn)` — no named args. Developer to confirm the final
    name/shape. Isolation-arg retry (`db.transaction(Isolation.Serializable, fn)`) rides with the
    deferred isolation slice. Example `examples/db/transaction-closure.phg`; `tests/db.rs`; both backends.
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
  (`E-CONCURRENCY-NO-PHP` + `--sequential-concurrency` opt-in warn). ⚠ Any PHP mapping serializes the
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
  documented `examples/db/basic.phg` — unusable with an incomprehensible error. RULED: (1) `db` joins
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
  alternative — binding trailing tight-ops to the pipe result — is strictly additive and awaits a
  ruling. Also not built (not in the ruled package): PHP 8.6's draft `|>=` pipe-assignment;
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
