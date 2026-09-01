# Unified import + injected-type discipline

> Status: **ADOPTED** 2026-07-03 (developer, interactive adjudication). Frozen design; implementation
> in gated slices S0→S2. Supersedes the split `import` / `import type` surface and closes the
> "injected Core types in the wind" bug (bare `Route`/`Router`/etc. usable with no import discipline).
> Governs: parser, AST, checker (type resolution + import classification), loader, the six injection
> preludes (`src/cli/mod.rs`), transpiler, and every `.phg` under `examples/` + `conformance/`.

## Motivation

The developer found that `#[Route(...)]`, `Router`, `Request`, `Response` (from `import Core.Http`)
were usable **bare** — no qualifier, no member import — violating the language's "everything is
imported / nothing in the wind" rule already enforced on injected **enum variants**
(`Json.Object`, `E-INJECTED-VARIANT-BARE`). Inspection found six injection preludes and two
pre-existing import kinds; the fix unifies the import surface and extends the discipline to all
injected Core types.

## The model (locked)

### 1. One `import` keyword — `import type` is REMOVED (no back-compat)

The resolver classifies each `import PATH [as ALIAS];` by resolving `PATH`:

- `PATH` resolves to a **module/package** ⇒ bind a **call-qualifier** (last segment or alias):
  `import Core.Http` → `Http.foo()`; `import Acme.Geometry` → `Geometry.foo()`.
- `PATH` resolves to a **type** (class / enum / interface / trait — the four `Item` type kinds) ⇒
  bind the **bare type name** (last segment or alias): `import Core.Http.Router` → `Router`;
  `import Acme.Geometry.Rect` → `Rect`.
- Resolves to neither ⇒ error (`E-IMPORT-UNKNOWN`).

The former `import type PATH` is deleted from the grammar; all existing sites migrate to plain
`import`. The `type_only` AST field is removed; the four `E-TYPE-IMPORT-*` codes are re-homed onto
the unified path (`E-IMPORT-BUILTIN`, `E-IMPORT-UNKNOWN`, `E-IMPORT-CONFLICT`, `E-IMPORT-SHADOW`).

### 2. Injected Core types get import discipline

The six preludes and their classification by **module-leaf vs member-name**:

| Module | Injected | Leaf | Discipline |
|---|---|---|---|
| `Core.Json` | `Json` enum | `Json` | leaf==type ⇒ compliant as-is; variants stay `Json.Object` |
| `Core.Regex` | `Regex` class | `Regex` | leaf==type ⇒ compliant as-is |
| `Core.Secret` | `Secret<T>` class | `Secret` | leaf==type ⇒ compliant as-is |
| `Core.Decimal` | `RoundingMode` enum | `Decimal` | member ⇒ `Decimal.RoundingMode` (or member-import) |
| `Core.Http` | `Request`,`Response`,`Route`,`Router` (+ `#[Route]`) | `Http` | members ⇒ `Http.X` / `#[Http.Route]` |
| `Core.Time` | `Duration`,`Date`,`Instant` | `Time` | members ⇒ `Time.X` |

Rules for the multi-type modules (Http/Time/Decimal):
- **Default = qualified by leaf**: `Http.Router`, `Time.Duration`, `Decimal.RoundingMode`,
  `#[Http.Route(...)]`.
- **Bare only via member-import**: `import Core.Http.Router;` → `Router` usable bare.
- **`E-INJECTED-TYPE-BARE`** (mirror of `E-INJECTED-VARIANT-BARE`): a bare injected member type
  without a member-import is an error, with a fix-it suggesting the qualified form or the import.
- The injected preludes' **own internal** references are exempt (they are the declaring block).
- The qualifier is **Phorj-surface only** — the transpiler erases it; PHP stays bare (`new Router()`).

Requires **qualified type resolution** `Qualifier.Type` in type position (new): `Http.Router` as an
annotation resolves `Router` in the module bound to qualifier `Http`.

### 3. Functions are NOT bare-importable; no associated functions

- Functions/natives stay **module-qualified** (`String.trim(s)`) or **UFCS** (`s.trim()`,
  method-first per DEC-087) — always traceable. A bare imported free call (`trim(s)`) is exactly the
  "in the wind" problem and is rejected by omission (no function-import form exists).
- **No associated functions**: `MyClass.stringify(x)` does NOT resolve a free `stringify(MyClass x)`
  (a class is not a free-function namespace; only modules are). Use `x.stringify()` (UFCS) or a
  `static function` on the class. `Module.fn(x)` works only because `Module` is a module, not a type.

## Implementation slices (each gates green + commits independently)

- **S0 — Unify import.** Parser drops the `type` keyword. Loader classifies each import as
  module-vs-type by path resolution (merge `user_import_map` + `build_type_imports`); remove
  `type_only`. Migrate the 18 `import type` sites (`examples/project/**`) to plain `import`.
  Fmt/lift printers stop emitting `type`. Gate.
- **S1 — Qualified type references.** Parser + AST + checker resolve `Qualifier.Type` in type
  position → the module-exported type; transpiler erases the qualifier. Gate.
- **S2 — Injected-type discipline.** Register the injected Core types as module-exported types;
  `E-INJECTED-TYPE-BARE`; member-import for Core injected types; `#[Http.Route]` (parser dotted attr
  name + `desugar_router` match). Migrate the injected preludes' user surface + ~40 `.phg`
  (examples/conformance) + docs (`FEATURES.md`, `examples/README.md`) to the qualified/member form. Gate.

## S1 build recipe (mechanical — investigation done 2026-07-03)

Fully traced; the next build is copy-paste. **Scope: type-ANNOTATION positions only** (params,
returns, fields, ctor-params, locals, catch, hook, enum-variant payloads, interface/trait methods,
test bodies) — the exact set `expand_aliases` covers. `new`/`instanceof`/`as` are expr-position →
S2 (they ride the shipped `new Enum.Variant()` variant-qual machinery).

1. **Parser** (`src/parser/types.rs`, `parse_type_atom` ~L141): after reading the first ident, while
   the next token is `Dot`, consume `.Ident` and append → `name = "Http.Router"` (PRESERVE the dotted
   form in `Type::Named{name}`; do NOT collapse at parse — `phg fmt` reads the pre-check AST and must
   print the qualified form, and S2's migration must be fmt-idempotent). Additive: a `.` after a type
   name is currently a parse error, so nothing regresses.
2. **Collapse pass** — new `src/checker/collapse_injected.rs`, `pub fn collapse_injected_type_qualifiers(prog: Program) -> Program`.
   Model the walker EXACTLY on `expand_aliases` (rewrite_alias.rs) — same rt/rparam/rstmt/rfunc/rmember
   + Item assembly — but: (a) KEEP `Item::TypeAlias` (do NOT drop — runs before check); (b) the `rt`
   Named-node rule: if `name` splits on `.` into `(qual, member)` and `INJECTED.get(qual)` contains
   `member` → replace with `Type::Named{name: member, args, span}`. Registry (static, mirrors the
   preludes): `Http`→{Request,Response,Route,Router}, `Time`→{Duration,Date,Instant},
   `Decimal`→{RoundingMode}. (Single-type Json/Regex/Secret excluded — leaf==type.) *Consider DRY-ing
   the three type-walkers (expand_aliases/erase_generics/this) into one `rewrite_type_names(prog, lookup)`
   later; standalone is fine for now — matches the codebase's per-pass style.*
3. **Wire** (`src/cli/mod.rs` `check_and_expand_reified` ~L980): insert AFTER `desugar_auto_router`,
   BEFORE `check_resolutions` — `let routed = collapse_injected_type_qualifiers(routed);` — so both the
   checker AND every backend see the bare `Router`. Register `mod collapse_injected;` + `pub use` in
   `src/checker/mod.rs`.
4. **Test** (integration, ZERO `.phg` edits): a program `import Core.Http; function f(Http.Router rt): void {}`
   + construct via the `Http.autoRouter()` factory (no `new` — that's S2) — assert `cli::check` Ok and
   `run`≡`runvm`≡PHP identical to the bare-`Router` version. Bare `Router` still works (no enforcement
   until S2). Verify the differential gate is unchanged (no example touched).

## Acceptance

- `import type` no longer parses; grep for it across the repo → 0.
- Bare `Router`/`Route`/`Request`/`Response`/`RoundingMode`/`Duration`/`Date`/`Instant` without a
  member-import → `E-INJECTED-TYPE-BARE`; `Http.Router` etc. + `#[Http.Route]` resolve.
- `import Core.Http.Router;` → bare `Router` allowed.
- Single-type modules unchanged (`Json`/`Regex`/`Secret` bare + `Json.Object` variants).
- Full correctness gate green (PHP oracle) at each slice; every migrated example byte-identical.
