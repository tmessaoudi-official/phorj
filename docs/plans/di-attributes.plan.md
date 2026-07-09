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
