# DI + Attribute-Reflection — Design Spec (DEC-194 downstream)

> Design captured 2026-07-09 from an interactive brainstorm with the developer. This is a DESIGN doc,
> not yet built. The attribute *foundation* (declare/apply/validate) is shipped (2a/2b-1/2b-3/2b-3b);
> everything here builds ON it and is gated on further prerequisites (see §Prerequisites).
> SSOT cross-ref: `docs/plans/perf-wave.plan.md` Decisions Log; register `C-decisions.md` (needs DEC #s).

## 0. The generic thesis (ruled)

Do NOT build "a DI system" as a bespoke feature. DI, controllers/routing, entities/ORM, templates,
validation, serialization are the SAME shape: **attribute-driven, whole-program-discovered metadata
processors**. Build the generic primitive; each framework feature is a *consumer* of it, not a compiler
special-case.

- **L1 — the generic primitive:** compile-time attribute reflection + **reverse discovery**
  (`subjectsWith<Attr>()` → every class/method/function/field carrying `#[Attr]`, with its structural
  metadata: type, ctor param types, methods, fields). This is the "get all subjects tagged with X" the
  developer asked for, generalized.
- **L2 — framework libraries ride L1:** DI (`#[Injectable]`), routing (`#[Route]`, extends the existing
  router), ORM (`#[Entity]`), etc.

**Resolution kind (ruled): BOTH, compile-time-FIRST.** Target = the full rich system (compile-time core
+ an opt-in runtime reflection API). Build order = compile-time core first (byte-identity-safe: discovery
feeds codegen, expands to plain construction BEFORE backends per Inv-5 → transpiles to ordinary PHP, NOT
quarantined; missing/ambiguous/cyclic = COMPILE error — the better-than-PHP story). The runtime reflection
API (`subjectsWith<A>()` callable at runtime) is a LATER layer, §14-quarantined when used.

## 1. DI v1 — the one-session core (ruled KEEP list)

The bounded, compile-time, byte-identity-clean DI core. All decisions below are developer-ruled.

- **`#[Injectable]`** on a class → compile-time registry (deterministic/sorted iteration, Inv-10).
- **Autowire by TYPE.** An injectable's autowired inputs = its **constructor params** + its
  **injectable-typed fields with no initializer**. Promoted ctor params are ctor params → wired.
- **Field injection is unified with ctor injection via SYNTHESIZED construction-time initialization** —
  injected fields are set once, AT construction, so they stay **immutable** (no post-construction
  assignment; respects immutable-by-default AND PHP `readonly`). An injectable-typed field WITH an
  initializer is user-provided → left alone.
- **`inject<T>()`** = a **composition-root** call (NOT usable mid-graph — prevents a second wiring path
  smuggling cycles/non-injectable deps past the graph analysis). At compile time it resolves T's full
  graph and **expands to plain construction before backends** (Inv-5) → byte-identical run≡runvm≡php.
- **Lifetime (ruled): default SHARED (singleton-per-resolution-root) + `#[Transient]` opt-out.** Within
  one `inject<T>()`, each type is constructed ONCE and shared (diamond `C(A(Db),B(Db))` → one `Db`;
  codegen hoists `let __db = new Db(); new C(new A(__db), new B(__db))`). `#[Transient]` = fresh instance
  per injection point. Spring's default-singleton model; avoids the silent stateful-duplication bug of a
  transient default. For a single composition root this default IS effectively app-wide sharing.
- **Interface deps — single-impl auto only (v1):** an interface-typed dep with EXACTLY ONE `#[Injectable]`
  impl program-wide auto-wires to it. Multiple impls → ambiguous → COMPILE error (qualifiers = v2).
- **Compile-time errors (all deterministic):** non-injectable dependency; interface/ambiguous impl; **cycle**
  (incl. field-injected — the synthesized-ctor model makes field cycles unbreakable too; state this: a PHP
  dev reaching for field injection to break a cycle gets a clear compile error); `inject<T>()` on a
  non-`#[Injectable]` T or one with a private/incompatible constructor.

## 2. Edge-case resolutions (ruled)

- **Immutability × field injection:** resolved by synthesized construction-init (see §1). No post-construction
  writes; injected fields stay immutable.
- **Which fields auto-wire:** injectable-typed fields with NO initializer.
- **Mixed injectable/non-injectable deps** (`constructor(Db db, string name)`): v1 = compile error on the
  non-injectable `name` (config-value provision = v2 `#[Provides]`).
- **Determinism (Inv-10):** the injectable registry + the ambiguity impl-set iterate sorted → reproducible
  compile errors + codegen.
- **Sharing in diamonds:** default-shared (see lifetime) → one instance per type per resolution.

## 3. v2 — later layers (captured, NOT designed to completion)

Everything pulled in after the v1 line. Each is its own future spec/slice:

- **Abstract-base flow:** an abstract class is never directly `inject`-able; its injected inputs (ctor
  params + injected fields) flow into any concrete `#[Injectable]` subclass's dependency graph.
- **Interface binding (per-subclass / multiple-impl / contextual):** three mechanisms considered —
  (C) single-impl auto [in v1]; (B) **binding/qualifier attribute** `#[Inject(FileLogger)] public Logger logger`
  or `#[Uses(Logger => FileLogger)]` on the subclass — RECOMMENDED for v2 (sound, standard = .NET
  `AddScoped<ILogger,FileLogger>` / Symfony bind, zero covariance machinery); (A) **covariant type-override**
  (subclass redeclares `public FileLogger logger`, narrowing the interface type) — elegant but needs NEW
  covariant-field-override type-system surface (Liskov-safe ONLY because injected fields are write-once);
  maybe-later sugar on top of B. **Advisor ruling: B is the default, A is v2-sugar.**
- **`#[Provides]` factories** for non-injectable values (config strings, connection URLs).
- **Generics injectables** (`Repo<T>`).
- **App-wide `#[Singleton]`** (one instance across ALL `inject` calls / process lifetime) — needs a RUNTIME
  singleton store + init ordering; NOT pure compile-time; the runtime-lifetime layer.
- **Request/session scopes.**
- **Runtime reflection API** (`subjectsWith<A>()` at runtime) — the §14-quarantined dynamic layer.
- **Field-injection cycle-breaking; lazy/proxy; decorators.**
- **Other L2 consumers:** routing (`#[Controller]`/`#[Route]`), ORM (`#[Entity]`/`#[Column]`), templates,
  validation, serialization — each a consumer of L1 discovery.

## 4. Prerequisites (what must exist before even v1-DI)

DI v1 is compile-time constructor+field autowiring → it needs LESS than the full L1 runtime reflection:
- ✅ attributes declare/apply/validate (2a/2b-1/2b-3/2b-3b — SHIPPED).
- ⏳ `#[Injectable]`/`#[Transient]` recognized (built-in attributes, small — reuse the marker machinery).
- ⏳ a compile-time DI resolution pass (collect injectables → resolve graph by type → cycle/ambiguity/
  missing checks → codegen expansion to plain construction). This is the real work; MEDIUM (a checker
  pass + a codegen expansion, reusing `ClassInfo.ctor` + the attribute registry).
- v1 does NOT need: named args, 2b-2 targets, 2c/2d runtime reflection, `subjectsWith` — those are v2/L1.

## 5. ⚠️ Scope reality (the honest tradeoff — developer decides)

This is a **multi-session framework programme**, not a feature. The session opened on PERF; **#3 (the
JIT object/enum/method perf frontier — the biggest remaining PHP-beating lever) is still untouched.**
"One session" is real ONLY for **DI v1** (§1) as a self-contained compile-time slice; the full rich system
(§3) is weeks. Two honest paths:
- **Build DI v1 now** (the §1 core), bank §3 as the v2 roadmap, return to perf after.
- **Bank this whole spec** (design captured durably) and return to perf/#3 now; build DI v1 in a fresh
  session.

Recommendation: this spec is now the durable artifact; the developer steers from it. (Advisor-flagged: the
design was accreting scope across ~8 reasks without persistence — this file fixes the persistence; the
scope call is the developer's.)

## 6b. DI v1 SLICE 1 — SHIPPED 2026-07-10 (gate-green 1870/1870, byte-identical)

**Built:** `inject` reserved keyword; `Expr::Inject{ty,span}` (parse `inject<T>()` explicit + `inject()`
bare); `#[Injectable]` built-in class attribute (`Attribute::is_di_builtin`, whitelisted in
`check_class_attributes`); **`src/checker/desugar_di.rs`** — a PRE-CHECK desugar (mirrors
`desugar_router`) that builds an injectable registry (structural, via `ctor_plan` + `class_implements`),
resolves each requested `T`'s dependency DAG by type (ctor params; single-impl interface auto-binds),
and synthesizes a `phorjInject<T>()` factory (camelCase — `__phorj_` fails E-NAME-CASE) with
topologically-ordered `var` bindings so each type is built ONCE per root (**default-SHARED, diamond →
one instance**, no §14 downgrade); rewrites each `inject<T>()` → `phorjInject<T>()`. Wired into
`check_and_expand_reified` after `resolve_variant_imports`. Errors `E-DI-MISSING`/`E-DI-AMBIGUOUS`/
`E-DI-CYCLE`/`E-INJECT-NO-TYPE` (+ `phg explain`). `examples/guide/di.phg` (ctor injection + single-impl
interface + diamond, byte-identical run≡runvm≡php-8.5.8, transpiles to a plain PHP `phorjInjectApp()`
factory) + README + 5 integration tests. All 13 `Expr` match arms added (formatter renders `inject<T>()`
faithfully for the raw-AST `phg format` path; checker types gracefully for the LSP raw path; backends
`unreachable`).
**Slice-1 scope (disclosed KNOWN limits):** ctor injection only (no field injection); concrete
class/interface dep types only (alias/generic dep → clean E-DI-MISSING, pre-alias-expansion);
`#[Transient]`/`#[Provides]`/bare-`inject()`-annotation-driven are LATER slices; `phorjInject<T>` /
`di<Class>` synth names collide only with astronomically-unlikely identical user names (documented).
**NEXT slices:** (2) bare `inject()` annotation-driven; (3) field injection (synthesized construction-init,
immutable-safe); (4) `#[Provides]` factory + `#[Transient]`; then interface-binding qualifiers (v2 §3).

## 6. DI v1 SYNTAX — developer-ruled 2026-07-10 (interactive, ask-human; supersedes §1's `inject<T>()` shorthand)

Session restart (2026-07-10) BUILD of DI v1 began; these are the ruled user-visible syntax decisions
(Invariant 15 — surfaced, not self-ruled), resolving the open shape questions §1 left as `inject<T>()` shorthand:

- **Composition root = `inject` is a RESERVED KEYWORD** (like `new`/`match`/`function`). Reserving the bare
  word removes the `<`-ambiguity (`inject < foo` as a comparison of a var named `inject`) with no speculative
  backtracking. Cost accepted: `inject` is no longer usable as an identifier (niche).
- **TWO composition-root forms, both feeding ONE graph resolver** (only the type-source differs):
  - `inject<T>()` — EXPLICIT: `T` from the type-arg; standalone, works anywhere (no annotation needed).
    `<T>` after the reserved `inject` keyword is unambiguously a type-arg list (not general turbofish — only
    this one keyword form). e.g. `var app = inject<App>();`, `inject<Server>().run();`
  - `inject()` — ANNOTATION-DRIVEN: `T` from the expected type (LHS declaration / return position), reusing
    the shipped expected-type threading. e.g. `App app = inject();`, `return inject();`
- **Factory mechanism = ONE `#[Provides]` static method — RULED** (developer chose the single-attribute option
  after clarifying: lifetime is ORTHOGONAL to construction, so a factory result honors `#[Transient]`/default-shared
  identically to a `new`-built one; and `#[Factory]`-on-a-class is functionally just a `#[Provides]` method living
  on that class → a second attribute would be redundant). Shape: a `static function` annotated `#[Provides]` whose
  RETURN TYPE is the provided type; its own params are autowired; it takes PRECEDENCE over `new T(...)` wherever the
  graph needs T. Placeable on a provider module (types you don't own / interface→impl binding / config values) OR
  directly on the injectable class itself (the "class builds itself" case). Lifetime: `#[Transient]` on the
  `#[Provides]` method (or default shared) — the resolver caches the factory result per resolution-root exactly like
  a constructed instance. `#[Provides]` is a v1 scope ADDITION (was §3/v2), deliberately pulled forward.
- **Interface→impl binding in v1:** single-impl auto (§1) OR an explicit `#[Provides]` returning the interface type
  (the multi-impl disambiguator, folded in free with the factory mechanism). Multiple `#[Injectable]` impls with no
  `#[Provides]` → ambiguous → compile error (qualifiers stay v2).

## 7. DI IMPORT DISCIPLINE — developer-ruled 2026-07-10 (interactive, ask-human; CORRECTS shipped slice 1)

Slice 1 shipped `#[Injectable]` + `inject` as AMBIENT globals (recognized with no import) — a violation of
the LOCKED "nothing in the wind" principle ([[import-namespace-redesign]], 2026-07-03). RULED corrections:

- **Namespace = `Core.DI`.** A Core module exporting the attribute-types `Injectable`, `Provides`,
  `Transient` and the composition-root verb `inject` — the SAME injected-Core-type discipline as `Core.Http`.
- **Both the attributes AND the `inject` verb follow Option 3 (member-import OR qualified):**
  - Qualified-by-leaf (default, via `import Core.DI;`): `#[DI.Injectable]`, `#[DI.Provides]`,
    `#[DI.Transient]`, `DI.inject<T>()`, `DI.inject()`.
  - Bare via member-import: `import Core.DI.Injectable;` → `#[Injectable]`; `import Core.DI.inject;`
    → bare `inject<T>()` / `inject()`.
  - Un-imported bare use → `E-INJECTED-TYPE-BARE` (attrs) / the verb's equivalent.
- **`inject` is NO LONGER an ambient reserved keyword** — it is a `Core.DI` member; the identifier is free
  again when Core.DI is not imported.
- **STANDING RULE (dev, absolute): from now on ANYTHING added must follow this same principle** — no new
  symbol (type, attribute, verb, function) may be usable in the wind; it must be qualified or member-imported.

### Decisions Log
- [2026-07-10] AGREED: DI namespace = `Core.DI` (attributes + `inject` verb are its members).
- [2026-07-10] AGREED: Option 3 — DI symbols usable qualified (`DI.inject`, `#[DI.Injectable]`) OR bare via
  member-import (`import Core.DI.inject;`, `import Core.DI.Injectable;`); un-imported bare = error.
- [2026-07-10] AGREED (standing): every future symbol follows the same import discipline — nothing ambient.

### SHIPPED 2026-07-10 (import-discipline retrofit + slice 2, gate-green 1884/1884, byte-identical)
- **Attributes:** `module_of("Injectable") == "DI"` (`enforce_injected.rs`) → bare `#[Injectable]` needs
  `import Core.DI.Injectable;`, qualified `#[DI.Injectable]` needs `import Core.DI;`, else
  `E-INJECTED-TYPE-BARE`. `Attribute::is_di_builtin` matches `"Injectable" | "DI.Injectable"` (single
  recognition source; mirrors `desugar_router`'s `"Route" | "Http.Route"`).
- **Verb:** `inject` un-keyworded (freed identifier; `TokenKind::Inject` removed). Parser recognizes only
  the explicit turbofish forms as `Expr::Inject { ty, qualified }` — bare at primary, qualified
  `DI.inject<T>()` in the postfix Dot arm. `desugar_di` gates (bare → `Core.DI.inject`; qualified →
  `Core.DI`; else `E-DI-NO-IMPORT`) and converts the no-turbofish `inject()`/`DI.inject()` ordinary calls
  to the composition root **only when imported** (un-imported stays a plain user call).
- **Slice 2 (annotation-driven):** bare/qualified `inject()` resolves `T` from a typed `var`, a `return`,
  or a lambda return type via `current_ret` threading (save/restore across `rfn` + lambdas) + a typed
  `VarDecl` init position. Same resolver → identical `phorjInject<T>()` factory (byte-identical). Not an
  annotation source: call-arg / param-default / `Optional`/generic (→ `E-DI-MISSING`); see `KNOWN_ISSUES.md`.
- **Artifacts:** `examples/guide/di.phg` (both forms), `examples/README.md`, `CHANGELOG.md`, KNOWN_ISSUES,
  `phg explain E-DI-NO-IMPORT`, 14 integration tests (typecheck + run) incl. lambda-inferred-return and
  free-identifier cases.

### SLICE 3 — field injection SHIPPED 2026-07-10 (gate-green, byte-identical)
- **Mechanism = synthesized-ctor (§1):** `fold_injected_fields` (in `desugar_di`, BEFORE `build_registry`)
  folds each injectable's injectable-typed, no-initializer INSTANCE field into its constructor as an
  appended **promoted param** (sorted name order, Inv-10; synthesizes an empty-body ctor if none). The
  field then IS a ctor dep → the existing resolver wires/shares/cycle-checks it identically; transpiles to
  an ordinary promoted-constructor property (byte-identical). A field WITH an initializer is left alone; a
  non-injectable-typed field is untouched. `build_registry` refactored to `collect_injectable` +
  `collect_impls` helpers (reused by the fold). `examples/guide/di-field-injection.phg` (shared Clock
  across ctor- and field-injected holders), 3 integration tests (clean / cycle / initialized-field-alone).
- **Disclosed (KNOWN_ISSUES):** applies program-wide to every injectable (direct `new` arity grows); a
  field set in the ctor BODY instead of via an initializer double-assigns — opt out with an initializer.

### SLICE 4a — `#[Provides]` factories SHIPPED 2026-07-10 (gate-green, byte-identical)
- **`#[Provides]`** on a `static` method (return type = provided type) → the graph builds that type via
  `Owner.method(autowired-params)` instead of `new`. `is_di_provides` (bare + qualified), `module_of`
  "Provides"→"DI", target-validated in `check_attributes` (static + return type, else `E-PROVIDES-TARGET`,
  `E-PROVIDES-ARGS`). Registry `collect_providers` scans ALL classes' static methods (NOT just
  injectables — provider modules are plain classes); duplicate provider for a type → `ambiguous_providers`
  → `E-DI-AMBIGUOUS`. Resolver restructured: `resolve_node` (provider-for-T → injectable class → single-
  impl interface, provider WINS incl. over the ambiguity), `Plan` now carries a `Construct::New|Provides`
  per node, `synth_factory` emits `new`/`Owner.method` accordingly. `examples/guide/di-provides.phg`
  (config-value provision), 5 tests (clean / interface-disambiguation / duplicate-ambiguous / non-static /
  no-import). resolve_graph now returns the node KEY.

**NEXT:** slice 4b `#[Transient]` (let-float codegen: hoist shared, inline transient per-use; regression
guard = byte-identical PHP for di.phg + di-field-injection.phg before/after) → then v2 qualifiers/generics.

### Direction (2026-07-10, ask-human): build BOTH slice 3 (field injection) AND slice 4 (#[Provides]+#[Transient]).
- [2026-07-10] AGREED: proceed with slice 3 then slice 4, each a green committed slice under the §7 discipline.
