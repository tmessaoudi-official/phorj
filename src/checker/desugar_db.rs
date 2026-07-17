//! DEC-208 — typed-generic `Core.DatabaseModule` result hydration. S2 shipped `queryInto`/`queryOneInto`; slice B
//! adds NESTED hydration (a field that is itself an entity, from dotted `"order.total"` aliases, eager,
//! optional-entity→null on all-NULL) + `queryScalar<T>` (one value) + `queryMap<K,V>` (keyed rows).
//! Slice E adds VALUE MAPPING for a field's type: a phorj `enum` field (TEXT column → the variant whose
//! NAME matches; zero-payload variants only; unknown value → `DatabaseError`), a `decimal` field (exact money
//! via `Row.getDecimal`), and a `Core.Json` field (TEXT column parsed via `Json.parse`). Each composes
//! with nested hydration and admits NULL in its `?` form (timestamp→`DateTime` is deferred on DEC-206).
//! Slice B2 adds the COLUMN NAMING STRATEGY: a `.namingStrategy(new Naming.SnakeToCamel())` call in the
//! query chain makes the desugar emit the `snake_case` of each field name as its column literal
//! (`userName` → `getString("user_name")`), applied per dotted segment for a nested alias. It is read
//! at COMPILE TIME from the chain (the prelude method is a no-op returning `this`); the strict-exact
//! default (`Naming::Exact`) is unchanged, so a query with no `namingStrategy` is byte-for-byte as
//! before. A non-literal strategy argument is rejected (`E-DB-NAMING-NOT-CONST`) — never silently
//! downgraded. The transform touches only by-field-name hydration (`queryInto`/`queryOneInto`, and a
//! `queryMap` entity value); a scalar column (`queryScalar`, a scalar map value/key) is read by
//! position and ignores it.
//!
//! A PRE-CHECK desugar (mirrors [`crate::checker::desugar_di`]): it lowers the four type-directed
//! result calls into plain, already-working S1 primitives BEFORE the type-checker, so the generated
//! code type-checks like hand-written source and every backend sees the same explicit construction
//! (Inv-5). Byte-identity is trivial (`run ≡ runvm`: both backends run the one desugared AST) and there
//! is no runtime reflection — generics are erased before the backends, so a native could never see `T`'s
//! fields; the field layout is resolved HERE, at compile time, from `T`'s constructor (recursively for a
//! nested entity — cycles are rejected `E-DB-HYDRATE-CYCLE`, since eager whole-graph loading is
//! unbounded on a self-reference).
//!
//! SURFACE (contextual OR explicit turbofish — DEC-208 slice A wired):
//! ```text
//!   List<User> users = stmt.queryInto();           // T inferred from the sink type
//!   User? one       = stmt.queryOneInto();          // 0 rows → null, 1 → the object, >1 → DatabaseError
//!   var users       = stmt.queryInto<User>();       // T explicit at the call site (turbofish)
//!   var byId        = stmt.queryMap<int, User>();   // K, V explicit
//! ```
//! `T` is drawn from the binding's declared type (a typed `var` decl, a `return`, or a lambda expr-body
//! return) — the same three annotation sources `desugar_di` threads — OR written explicitly as a
//! turbofish, which WINS over any annotation (explicit > contextual; a disagreement surfaces as the
//! ordinary assignment type error on the helper's typed return). Turbofish arity is checked here
//! (`E-TYPE-ARG-COUNT`) because this pass consumes the call pre-check. A `queryInto()` with neither
//! source is `E-DB-INTO-NO-TYPE`; a sink that is not `List<Class>` / `Class?` is `E-DB-INTO-BAD-SINK`.
//!
//! MAPPING (by field NAME, STRICT — DEC-208): `T` is hydrated by calling its constructor, passing every
//! **promoted** constructor parameter extracted from the row by its name via the typed S1 `Row` accessor
//! matching its type (`int`→`getInt`, `string`→`getString`, `float`→`getFloat`, `bool`→`getBool`). The
//! strict semantics are inherited for free from those accessors: a missing column, a type mismatch, or a
//! SQL NULL into a non-optional field each throw `DatabaseError` (row `db.rs` `row_cell`/`get_int`/…). Extra
//! columns are ignored (only named columns are read). Requiring every ctor param to be a promoted field
//! makes "param name == field name" structural, so mapping-by-field-name and construction coincide
//! (`E-DB-HYDRATE-UNPROMOTED` / `E-DB-HYDRATE-NO-CTOR` / `E-DB-HYDRATE-FIELD-TYPE` otherwise).
//!
//! ERROR MECHANISM: the generated helpers are ordinary phorj functions declared `throws DatabaseError`; they
//! reuse the S1 `Statement.query()` + `Row` accessors (each already `throws DatabaseError`) with `?`
//! propagation — no new native, the same catchable model as S1.
//!
//! IMPORT DISCIPLINE (nothing in the wind): active only when `Core.DatabaseModule` is imported. Under that import
//! `queryInto`/`queryOneInto`/`queryScalar`/`queryMap` are the reserved result method names (like
//! `inject` under `Core.DependencyInjection`); each generated helper takes a `Statement` parameter, so one of these on
//! any other receiver is a clean argument-type error rather than silent misbehaviour. Disclosed in
//! KNOWN_ISSUES.
//!
//! INVARIANT — keep the rewriter TOTAL (matching `desugar_di`): `ritem`/`rfn`/`rmember`/`rexpr`/`rstmt`
//! recurse EVERY expression-bearing position so a `queryInto` in any position is either rewritten or
//! reported. A new expression-bearing AST node → add its arm here.

use crate::ast::{
    ctor_plan, BinaryOp, CatchClause, ClassMember, CollKind, CtorParam, Expr, FunctionDecl, Item,
    LambdaBody, MatchArm, MemberSep, Modifier, Param, Pattern, Program, Stmt, StrPart, Type,
    UnaryOp, Visibility,
};
use crate::diagnostic::{Diagnostic, Stage};
use crate::token::Span;
use std::collections::BTreeMap;

/// Synthetic-span base: every generated node gets a unique `Span.start` at or above this, so it never
/// collides with a real source byte offset (a key into the checker's span-keyed resolution maps —
/// UFCS/`?`-erasure/reified). `usize::MAX / 2` leaves the whole real-source range below and reflect's
/// `usize::MAX` sentinel above, with room for far more nodes than any program could generate.
const SYNTH_BASE: usize = usize::MAX / 2;

/// The four type-directed `Core.DatabaseModule` result calls this pass lowers (all nullary member calls).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Call {
    /// `List<T> = stmt.queryInto()` — one `T` per row.
    IntoList,
    /// `T? = stmt.queryOneInto()` — 0 → null, 1 → the object, >1 → `DatabaseError`.
    IntoOne,
    /// `T v = stmt.queryScalar()` — one typed value from a single-row, single-column result.
    Scalar,
    /// `Map<K, V> = stmt.queryMap()` — rows keyed by the first column (K), V from the rest.
    Map,
    /// `DatabaseStream<T> = stmt.streamInto()` (DEC-208 item H) — a lazy typed stream: one hydrated `T`
    /// per PULLED row (`next(): T?`); rows never pulled are never hydrated.
    Stream,
}

/// Whether a class-hydration helper produces a `List<T>` (`queryInto`) or a `T?` (`queryOneInto`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ClassKind {
    List,
    One,
}

/// DEC-208 slice B2 — the per-query COLUMN NAMING STRATEGY, read from a `.namingStrategy(new
/// Naming.X())` call in the receiver chain AT COMPILE TIME (the developer's ruling: compile-time,
/// per-query). It changes only the generated column-name string literal; there is no runtime component
/// (the prelude `Statement.namingStrategy` is a chainable no-op returning `this`, present solely so the
/// chain type-checks). Because the transform runs before the backends, `run ≡ runvm` is trivial.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Naming {
    /// The default (unchanged from slice B): the column name IS the field name (strict-exact).
    Exact,
    /// DB `snake_case` ↔ phorj `camelCase`: the column name is the `snake_case` of the field name
    /// (`userName` → `user_name`). Applied PER dotted segment for a nested alias (a nested
    /// `shipTo.postalCode` reads `"ship_to.postal_code"`).
    SnakeToCamel,
}

/// The result of scanning a `query…()` receiver chain for a `.namingStrategy(...)` call.
enum NamingFind {
    /// No `namingStrategy` in the chain — the default `Exact`.
    None,
    /// A `namingStrategy(new Naming.X())` with a recognized compile-time literal.
    Found(Naming),
    /// A `namingStrategy(...)` whose argument is NOT a `new Naming.{Exact,SnakeToCamel}()` literal:
    /// rejected (`E-DB-NAMING-NOT-CONST`). The strategy MUST be a compile-time literal — a runtime
    /// value would silently fall through to `Exact` (a forbidden silent downgrade). Carries the call
    /// span for the diagnostic.
    Bad(Span),
}

/// The synthetic-helper-name suffix distinguishing the naming strategies, so two calls that hydrate
/// the same class under different strategies dedup to two distinct helpers with distinct column
/// literals. `Exact` keeps the historic (unsuffixed) name → a program with no `namingStrategy` is
/// byte-for-byte unchanged.
fn naming_suffix(naming: Naming) -> &'static str {
    match naming {
        Naming::Exact => "",
        Naming::SnakeToCamel => "SnakeToCamel",
    }
}

/// `camelCase` → `snake_case` for the DB column of a phorj field under `Naming::SnakeToCamel`
/// (`userName` → `user_name`, `firstName` → `first_name`). ACRONYM rule: an `_` is inserted before an
/// uppercase letter that is either (a) preceded by a lowercase letter or digit, OR (b) the start of a
/// new word inside an acronym run (an uppercase FOLLOWED by a lowercase). So `userId`/`userID` both →
/// `user_id`, `parseHTTPResponse` → `parse_http_response`, and an already-lowercase `id` → `id`.
/// Deterministic and ASCII-oriented (phorj identifiers are ASCII; any non-ASCII passes through
/// lowercased by `char::to_lowercase`). A leading uppercase gets no leading `_` (fields are camelCase
/// anyway, so this is defensive).
fn snake_case(field: &str) -> String {
    let chars: Vec<char> = field.chars().collect();
    let mut out = String::with_capacity(field.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            let next = chars.get(i + 1).copied();
            let boundary = matches!(prev, Some(p) if p.is_ascii_lowercase() || p.is_ascii_digit())
                || matches!((prev, next), (Some(p), Some(n)) if p.is_ascii_uppercase() && n.is_ascii_lowercase());
            if boundary {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// How the VALUE of a `queryMap` entry is produced from a row: a scalar column, or a hydrated entity.
enum MapVal {
    /// A single column read by `accessor` (the second selected column); its declared type is the
    /// enclosing [`HelperSpec::Map::val_ty`].
    Scalar { accessor: &'static str },
    /// An entity hydrated by field name from the row (same rules as `queryInto`, dotted for nested).
    Entity { class: String },
}

/// A resolved hydration helper to synthesize (deduped by name). Nested class hydration is resolved
/// recursively at synthesis time from `ctor_params` (not precomputed), so a `Class`/`Map`-entity
/// helper carries only the top class name.
enum HelperSpec {
    /// `phorjQueryIntoList<Class>[<Naming>]` / `phorjQueryOneInto<Class>[<Naming>]`. `naming` bakes the
    /// column-name transform (DEC-208 slice B2) into the generated `getX("…")` literals.
    Class {
        kind: ClassKind,
        class: String,
        naming: Naming,
    },
    /// `phorjQueryScalar<Label>` — a single typed value; `ret` is the scalar (or `T?` scalar) type.
    /// (No `naming`: a scalar is read by column POSITION, not by field name.)
    Scalar { accessor: &'static str, ret: Type },
    /// `phorjQueryMap<KLabel><VLabel>[<Naming>]` — a `Map<key_ty, val_ty>` keyed by the first column.
    /// `naming` applies only to an ENTITY value (hydrated by field name); a scalar value is by
    /// position, so a scalar-value map always carries `Naming::Exact`.
    Map {
        key_acc: &'static str,
        key_ty: Type,
        val: MapVal,
        val_ty: Type,
        naming: Naming,
    },
    /// `phorjStreamInto<Class>[<Naming>]` (DEC-208 item H) — a `DatabaseStream<Class>` whose per-row
    /// hydration closure reuses the SAME `build_class` machinery as `queryInto`, but runs it lazily
    /// per pulled row.
    Stream { class: String, naming: Naming },
}

/// A camelCase synthetic free-function name (free functions are `E-NAME-CASE`-checked; `Class`/type
/// labels are PascalCase, so the whole name is camelCase). Collision with a hand-written
/// `phorjQuery…` is astronomically unlikely and disclosed (KNOWN_ISSUES), matching `phorjInject…`.
fn class_helper_name(kind: ClassKind, class: &str, naming: Naming) -> String {
    let sfx = naming_suffix(naming);
    match kind {
        ClassKind::List => format!("phorjQueryIntoList{class}{sfx}"),
        ClassKind::One => format!("phorjQueryOneInto{class}{sfx}"),
    }
}

/// A PascalCase label for a scalar (or optional scalar) type, for use inside a synthetic helper name:
/// `int` → `Int`, `string?` → `StringOpt`. No `?`/`_` so the composed name stays camelCase.
fn scalar_label(ty: &Type) -> Option<String> {
    fn cap(name: &str) -> Option<String> {
        match name {
            "int" => Some("Int".into()),
            "string" => Some("String".into()),
            "float" => Some("Float".into()),
            "bool" => Some("Bool".into()),
            "decimal" => Some("Decimal".into()),
            _ => None,
        }
    }
    // DEC-208 slice K: `List<scalar>` sinks label as `IntList`/`StringList`/… (array-column reads).
    fn list_label(inner: &Type) -> Option<String> {
        match inner {
            Type::Named { name, args, .. } if name == "List" && args.len() == 1 => match &args[0] {
                Type::Named { name: e, args, .. } if args.is_empty() && e != "decimal" => {
                    Some(format!("{}List", cap(e)?))
                }
                _ => None,
            },
            _ => None,
        }
    }
    match ty {
        Type::Named { name, args, .. } if args.is_empty() => cap(name),
        Type::Named { .. } => list_label(ty),
        Type::Optional { inner, .. } => match &**inner {
            Type::Named { name, args, .. } if args.is_empty() => Some(format!("{}Opt", cap(name)?)),
            t @ Type::Named { .. } => Some(format!("{}Opt", list_label(t)?)),
            _ => None,
        },
        _ => None,
    }
}

/// The classification of a hydrated field: a scalar column, or a nested entity (a class), which may
/// be optional (`Order? order` → `null` when all its columns are NULL, for a LEFT JOIN).
enum FieldKind {
    Scalar {
        accessor: &'static str,
    },
    Entity {
        class: String,
        optional: bool,
    },
    /// DEC-208 slice E: a phorj `enum` field, mapped from a TEXT column by matching the column value
    /// against the variant NAME (zero-payload variants only — validated in [`Database::validate_class`]).
    /// `optional` (`Status?`) admits a NULL column (→ `null`).
    Enum {
        name: String,
        optional: bool,
    },
    /// DEC-208 slice E: a `Core.Json` field, mapped from a TEXT column by parsing it via `Json.parse`
    /// (requires the program's own `import Core.Json`). `optional` (`Json?`) admits a NULL column.
    Json {
        optional: bool,
    },
}

/// True iff a constructor parameter is a promoted field (carries a visibility modifier) — the S2
/// invariant that makes "parameter name == field name == column name" hold.
fn is_promoted(p: &CtorParam) -> bool {
    p.modifiers.iter().any(|m| {
        matches!(
            m,
            Modifier::Public | Modifier::Private | Modifier::Protected
        )
    })
}

/// The S1 `Row` accessor for a hydrated field type: the non-nullable accessor for a scalar (`int`→
/// `getInt`, …) — which throws on a SQL NULL — or the nullable accessor for a `T?` scalar (`int?`→
/// `getIntOrNull`, …) — which admits NULL. Any other type has no column accessor (`None`).
fn accessor_for(ty: &Type) -> Option<&'static str> {
    // DEC-208 slice K: a `List<scalar>` field maps an ARRAY column (Postgres `int[]` → `List<int>`,
    // `text[]` → `List<string>`, …) via the typed list accessors. `decimal` arrays are excluded
    // (Postgres `numeric[]` is read via a `::text[]` cast into `List<string>` — the slice-E
    // store-decimal-as-TEXT discipline, element form).
    fn list_accessor(elem: &Type, or_null: bool) -> Option<&'static str> {
        let Type::Named { name, args, .. } = elem else {
            return None;
        };
        if !args.is_empty() {
            return None;
        }
        Some(match (name.as_str(), or_null) {
            ("int", false) => "getIntList",
            ("string", false) => "getStringList",
            ("float", false) => "getFloatList",
            ("bool", false) => "getBoolList",
            ("int", true) => "getIntListOrNull",
            ("string", true) => "getStringListOrNull",
            ("float", true) => "getFloatListOrNull",
            ("bool", true) => "getBoolListOrNull",
            _ => return None,
        })
    }
    match ty {
        Type::Named { name, args, .. } if args.is_empty() => match name.as_str() {
            "int" => Some("getInt"),
            "string" => Some("getString"),
            "float" => Some("getFloat"),
            "bool" => Some("getBool"),
            // DEC-208 slice E: a `decimal` field maps exact-money via `Row.getDecimal`.
            "decimal" => Some("getDecimal"),
            _ => None,
        },
        Type::Named { name, args, .. } if name == "List" && args.len() == 1 => {
            list_accessor(&args[0], false)
        }
        Type::Optional { inner, .. } => match &**inner {
            Type::Named { name, args, .. } if args.is_empty() => match name.as_str() {
                "int" => Some("getIntOrNull"),
                "string" => Some("getStringOrNull"),
                "float" => Some("getFloatOrNull"),
                "bool" => Some("getBoolOrNull"),
                "decimal" => Some("getDecimalOrNull"),
                _ => None,
            },
            Type::Named { name, args, .. } if name == "List" && args.len() == 1 => {
                list_accessor(&args[0], true)
            }
            _ => None,
        },
        _ => None,
    }
}

/// Is `ty` `Core.Json`'s `Json` (or `Json?`)? Returns `Some(optional)` (DEC-208 slice E). Matched by
/// name — the only way `Json` is nameable is the program's own `import Core.Json`, which injects the
/// enum (so no user type can shadow it here).
fn json_kind(ty: &Type) -> Option<bool> {
    match ty {
        Type::Named { name, args, .. } if args.is_empty() && name == "Json" => Some(false),
        Type::Optional { inner, .. } => match &**inner {
            Type::Named { name, args, .. } if args.is_empty() && name == "Json" => Some(true),
            _ => None,
        },
        _ => None,
    }
}

/// A short human label for a field type, for the `E-DB-HYDRATE-FIELD-TYPE` message.
fn type_label(t: &Type) -> String {
    match t {
        Type::Named { name, args, .. } if args.is_empty() => name.clone(),
        Type::Optional { inner, .. } => format!("{}?", type_label(inner)),
        _ => "<type>".into(),
    }
}

/// True iff the program imports `Core.DatabaseModule` in any form (module or member) — the gate for the whole pass.
fn imports_core_db(program: &Program) -> bool {
    program.items.iter().any(|it| {
        matches!(it, Item::Import { path, .. }
            if path.len() >= 2 && path[0] == "Core" && path[1] == "DatabaseModule")
    })
}

pub fn desugar_db(program: Program) -> Result<Program, Vec<Diagnostic>> {
    // A no-op unless `Core.DatabaseModule` is imported — so a program that never touches the DB is byte-for-byte
    // unchanged and a user method named `queryInto` outside `Core.DatabaseModule` is never hijacked.
    if !imports_core_db(&program) {
        return Ok(program);
    }
    // Constructor param layout for every class (flattened `ctor_plan` → inherited promoted params too),
    // computed before destructuring so the resolver needs no `&Program`.
    let mut ctor_params: BTreeMap<String, Vec<CtorParam>> = BTreeMap::new();
    for it in &program.items {
        if let Item::Class(c) = it {
            let params: Vec<CtorParam> = ctor_plan(&program, &c.name)
                .into_iter()
                .flat_map(|(ps, _)| ps)
                .collect();
            ctor_params.insert(c.name.clone(), params);
        }
    }
    // Enum variant layout for every declared/injected enum (DEC-208 slice E) — name → `[(variant,
    // payload-arity)]`. Drives enum-field hydration (a TEXT column → the matching variant) and the
    // zero-payload validation (a variant carrying data cannot be built from one column). Injected
    // enums are present here too (injection runs before this pass); `Json` is name-special-cased ahead
    // of the generic-enum path, so its payload variants never reach the rejection.
    let mut enum_variants: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();
    for it in &program.items {
        if let Item::Enum(e) = it {
            let variants = e
                .variants
                .iter()
                .map(|v| (v.name.clone(), v.fields.len()))
                .collect();
            enum_variants.insert(e.name.clone(), variants);
        }
    }
    let Program {
        package,
        items,
        span,
    } = program;
    let mut db = Database {
        ctor_params: &ctor_params,
        enum_variants: &enum_variants,
        helpers: BTreeMap::new(),
        diags: Vec::new(),
        current_ret: None,
        next_span: SYNTH_BASE,
        next_local: 0,
        current_naming: Naming::Exact,
    };
    let mut items: Vec<Item> = items.into_iter().map(|it| db.ritem(it)).collect();
    if !db.diags.is_empty() {
        return Err(db.diags);
    }
    // Append one hydration helper per (kind, class) used, sorted by name (Inv-10 determinism).
    let helpers = std::mem::take(&mut db.helpers);
    for (name, spec) in &helpers {
        let f = db.synth_helper(name, spec);
        items.push(f);
    }
    Ok(Program {
        package,
        items,
        span,
    })
}

struct Database<'a> {
    ctor_params: &'a BTreeMap<String, Vec<CtorParam>>,
    /// enum name → `[(variant, payload-arity)]` for every declared/injected enum (DEC-208 slice E).
    enum_variants: &'a BTreeMap<String, Vec<(String, usize)>>,
    /// helper name → spec, deduped (one helper per (kind, class)) and iterated sorted.
    helpers: BTreeMap<String, HelperSpec>,
    diags: Vec<Diagnostic>,
    /// The enclosing function/method/lambda return type — the annotation source for `return
    /// stmt.queryInto();`. Saved/restored across every function and lambda.
    current_ret: Option<Type>,
    /// Monotonic synthetic-span allocator (see [`SYNTH_BASE`]).
    next_span: usize,
    /// Monotonic counter for unique per-field local names (`phorjV0`, `phorjV1`, …) in a synthesized
    /// helper body — camelCase (lowercase head, no `_`) so it passes `E-NAME-CASE`, and disjoint from
    /// the fixed `phorjRows`/`phorjOut`/… locals and from every user field name.
    next_local: usize,
    /// The column naming strategy of the helper CURRENTLY being synthesized (DEC-208 slice B2). Set as
    /// the first line of [`Database::synth_helper`] from the helper's spec, read by [`Database::col`]/[`Database::seg`]
    /// while building that one helper's body. Synthesis is strictly sequential (one helper fully built
    /// before the next), so a transient field is sound — no interleaving.
    current_naming: Naming,
}

impl Database<'_> {
    fn sp(&mut self) -> Span {
        let s = self.next_span;
        self.next_span += 1;
        Span {
            start: s,
            len: 0,
            line: 0,
            col: 0,
        }
    }

    fn diag(&mut self, span: Span, msg: String, code: &'static str, hint: String) {
        self.diags.push(
            Diagnostic::new(Stage::Type, msg, span.line, span.col)
                .with_code(code)
                .with_hint(hint),
        );
    }

    /// A non-cascading placeholder returned after a diagnostic (the non-empty `diags` aborts the
    /// pipeline before the checker ever sees it).
    fn placeholder(&mut self) -> Expr {
        Expr::Null(self.sp())
    }

    // ── recognition + rewrite ────────────────────────────────────────────────────────────────────

    /// If `callee` is a nullary `recv.query{Into,OneInto,Scalar,Map}()` member call, which one.
    fn query_call_kind(callee: &Expr) -> Option<Call> {
        match callee {
            Expr::Member {
                name, safe: false, ..
            } => match name.as_str() {
                "queryInto" => Some(Call::IntoList),
                "queryOneInto" => Some(Call::IntoOne),
                "queryScalar" => Some(Call::Scalar),
                "queryMap" => Some(Call::Map),
                "streamInto" => Some(Call::Stream),
                _ => None,
            },
            _ => None,
        }
    }

    /// DEC-208 slice B2 — scan a `query…()` receiver chain (down the `.object` spine) for a
    /// `.namingStrategy(arg)` call and read its compile-time strategy. The FIRST one found (the call
    /// closest to the `query…()`) wins; a chain with no `namingStrategy` is `Exact`. The strategy MUST
    /// be a `new Naming.{Exact,SnakeToCamel}()` literal — anything else is `Bad` (`E-DB-NAMING-NOT-
    /// CONST`) rather than a silent downgrade. The `namingStrategy` call itself is a no-op left intact
    /// in the receiver (a real chainable prelude method), so `recv` still type-checks as a `Statement`.
    fn naming_of_recv(recv: &Expr) -> NamingFind {
        let mut cur = recv;
        loop {
            match cur {
                Expr::Call {
                    callee,
                    args,
                    span,
                    type_args: _,
                } => {
                    if let Expr::Member {
                        object,
                        name,
                        safe: false,
                        ..
                    } = &**callee
                    {
                        if name == "namingStrategy" {
                            return match args.first().and_then(Self::naming_from_arg) {
                                Some(n) => NamingFind::Found(n),
                                None => NamingFind::Bad(*span),
                            };
                        }
                        cur = object;
                    } else {
                        return NamingFind::None;
                    }
                }
                Expr::Member { object, .. } => cur = object,
                _ => return NamingFind::None,
            }
        }
    }

    /// A `namingStrategy` argument → a compile-time [`Naming`], iff it is a `new Naming.{Exact,
    /// SnakeToCamel}()` literal — the only form resolvable at desugar time. Anything else → `None`
    /// (mapped to `NamingFind::Bad`): a runtime `Naming` value cannot drive a compile-time column
    /// rewrite, and falling back to `Exact` would be a silent semantic downgrade.
    fn naming_from_arg(e: &Expr) -> Option<Naming> {
        let Expr::New(inner, _) = e else {
            return None;
        };
        let Expr::Call { callee, args, .. } = &**inner else {
            return None;
        };
        if !args.is_empty() {
            return None;
        }
        let Expr::Member { object, name, .. } = &**callee else {
            return None;
        };
        let Expr::Ident(id, _) = &**object else {
            return None;
        };
        if id != "Naming" {
            return None;
        }
        match name.as_str() {
            "Exact" => Some(Naming::Exact),
            "SnakeToCamel" => Some(Naming::SnakeToCamel),
            _ => None,
        }
    }

    /// The DB column SEGMENT for a field under the current naming strategy (DEC-208 slice B2): the
    /// field name verbatim (`Exact`) or its `snake_case` (`SnakeToCamel`).
    fn seg(&self, field: &str) -> String {
        match self.current_naming {
            Naming::Exact => field.to_string(),
            Naming::SnakeToCamel => snake_case(field),
        }
    }

    /// The full column name for `field` under `prefix`: the (strategy-transformed) segment at the top
    /// level, `prefix.<segment>` (the dotted alias convention, DEC-208 slice B) when nested — the
    /// string key the S1 `getX` accessor looks up. Under `SnakeToCamel` each dotted segment is
    /// transformed independently, and since a nested entity's `prefix` is itself an already-transformed
    /// column built by the parent, a deep alias reads e.g. `"ship_to.postal_code"`.
    fn col(&self, prefix: &str, field: &str) -> String {
        let f = self.seg(field);
        if prefix.is_empty() {
            f
        } else {
            format!("{prefix}.{f}")
        }
    }

    /// Rewrite a recognized `recv.query…()` into a call to its synthesized helper (`helper(recv)`),
    /// drawing the target type(s) from `expected`. On any resolution failure a diagnostic is recorded
    /// and a placeholder is returned (the pipeline aborts on the non-empty `diags`).
    /// DEC-208 slice A wiring — an explicit turbofish (`stmt.queryInto<User>()`) IS the sink type,
    /// synthesized per method shape. It beats any annotation (explicit > contextual; a disagreement
    /// then surfaces as the ordinary assignment type error on the helper's typed return). Arity is
    /// validated HERE because this pass runs pre-check and consumes the call — the generic checker
    /// never sees these type arguments.
    fn turbofish_sink(&mut self, call: Call, mut type_args: Vec<Type>, span: Span) -> Option<Type> {
        let (method, want, shape) = match call {
            Call::IntoList => ("queryInto", 1, "`queryInto<RowClass>()`"),
            Call::IntoOne => ("queryOneInto", 1, "`queryOneInto<RowClass>()`"),
            Call::Scalar => (
                "queryScalar",
                1,
                "`queryScalar<int>()` (a scalar, or a `?` form)",
            ),
            Call::Map => ("queryMap", 2, "`queryMap<KeyType, ValueType>()`"),
            Call::Stream => ("streamInto", 1, "`streamInto<RowClass>()`"),
        };
        if type_args.len() != want {
            self.diag(
                span,
                format!(
                    "`{method}` takes {want} type argument(s), found {}",
                    type_args.len()
                ),
                "E-TYPE-ARG-COUNT",
                format!("write {shape}, or no type arguments to infer from the binding's type"),
            );
            return None;
        }
        Some(match call {
            Call::IntoList => Type::Named {
                name: "List".into(),
                args: vec![type_args.remove(0)],
                span,
            },
            Call::IntoOne => Type::Optional {
                inner: Box::new(type_args.remove(0)),
                span,
            },
            Call::Scalar => type_args.remove(0),
            Call::Map => Type::Named {
                name: "Map".into(),
                args: type_args,
                span,
            },
            Call::Stream => Type::Named {
                name: "DatabaseStream".into(),
                args: vec![type_args.remove(0)],
                span,
            },
        })
    }

    fn rewrite(
        &mut self,
        callee: Expr,
        call: Call,
        expected: Option<&Type>,
        type_args: Vec<Type>,
        span: Span,
    ) -> Expr {
        let Expr::Member { object, .. } = callee else {
            unreachable!("query_call_kind guarantees a Member callee");
        };
        // DEC-208 slice B2: read the per-query column naming strategy from the chain BEFORE the recv is
        // consumed. A non-literal `namingStrategy` argument is rejected here (no silent downgrade).
        let naming = match Self::naming_of_recv(&object) {
            NamingFind::None => Naming::Exact,
            NamingFind::Found(n) => n,
            NamingFind::Bad(bad) => {
                self.diag(
                    bad,
                    "`namingStrategy` needs a compile-time `Naming` literal argument".into(),
                    "E-DB-NAMING-NOT-CONST",
                    "pass a literal `new Naming.SnakeToCamel()` or `new Naming.Exact()` — the column naming strategy is resolved at compile time, so a variable or computed value cannot drive it".into(),
                );
                return self.placeholder();
            }
        };
        let recv = self.rexpr(*object);
        let expected = if type_args.is_empty() {
            expected.cloned()
        } else {
            // Turbofish present: it wins over any annotation. A `None` here means the turbofish
            // itself was malformed (arity) — already diagnosed, so bail to the placeholder.
            match self.turbofish_sink(call, type_args, span) {
                Some(t) => Some(t),
                None => return self.placeholder(),
            }
        };
        let name = match call {
            Call::IntoList => self.resolve_class(ClassKind::List, expected.as_ref(), span, naming),
            Call::IntoOne => self.resolve_class(ClassKind::One, expected.as_ref(), span, naming),
            Call::Scalar => self.resolve_scalar(expected.as_ref(), span),
            Call::Map => self.resolve_map(expected.as_ref(), span, naming),
            Call::Stream => self.resolve_stream(expected.as_ref(), span, naming),
        };
        let Some(name) = name else {
            return self.placeholder();
        };
        let helper = Expr::Ident(name, span);
        Expr::Call {
            callee: Box::new(helper),
            args: vec![recv],
            span,
            type_args: Vec::new(),
        }
    }

    /// Is `ty` a phorj `enum` (or `enum?`) — a non-generic named type in `enum_variants`? Returns
    /// `Some((name, optional))` (DEC-208 slice E). `Json` is handled by [`json_kind`] BEFORE this, so a
    /// `Json` field never reaches here (it is also in `enum_variants` as an injected enum).
    fn enum_kind(&self, ty: &Type) -> Option<(String, bool)> {
        match ty {
            Type::Named { name, args, .. }
                if args.is_empty() && self.enum_variants.contains_key(name) =>
            {
                Some((name.clone(), false))
            }
            Type::Optional { inner, .. } => match &**inner {
                Type::Named { name, args, .. }
                    if args.is_empty() && self.enum_variants.contains_key(name) =>
                {
                    Some((name.clone(), true))
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Classify a field type for hydration: a scalar column, an entity (a class in `ctor_params`,
    /// optionally `?`), or unhydratable (`None`). A `T?` scalar routes through `accessor_for` (which
    /// handles the optional scalar accessors); an `Optional` wrapping a class routes to `Entity`.
    fn classify_field(&self, ty: &Type) -> Option<FieldKind> {
        if let Some(accessor) = accessor_for(ty) {
            return Some(FieldKind::Scalar { accessor });
        }
        // `Core.Json`'s injected `Json` enum — matched by NAME before the generic-enum arm below
        // (`Json` has payload variants, which that path rejects). The program's own `import Core.Json`
        // is what makes `Json` nameable as a field type (nothing in the wind). A user-declared
        // `class Json` (in `ctor_params`) takes precedence — it hydrates as an ordinary entity, not the
        // parse path (see KNOWN_ISSUES for the residual `enum Json` collision).
        if let Some(optional) = json_kind(ty) {
            if !self.ctor_params.contains_key("Json") {
                return Some(FieldKind::Json { optional });
            }
        }
        // A phorj `enum` field (zero-payload variants validated in `validate_class`).
        if let Some((name, optional)) = self.enum_kind(ty) {
            return Some(FieldKind::Enum { name, optional });
        }
        match ty {
            Type::Named { name, args, .. }
                if args.is_empty() && self.ctor_params.contains_key(name) =>
            {
                Some(FieldKind::Entity {
                    class: name.clone(),
                    optional: false,
                })
            }
            Type::Optional { inner, .. } => match &**inner {
                Type::Named { name, args, .. }
                    if args.is_empty() && self.ctor_params.contains_key(name) =>
                {
                    Some(FieldKind::Entity {
                        class: name.clone(),
                        optional: true,
                    })
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Recursively validate that `class` (and every entity it nests) is hydratable: a promoted-field
    /// constructor, every parameter promoted, every field a scalar or a valid nested entity. `path` is
    /// the DFS ancestor stack — a class already on it is a CYCLE (unbounded eager hydration), rejected
    /// with `E-DB-HYDRATE-CYCLE`. A non-cyclic diamond (two fields reaching the same class by distinct
    /// column prefixes) is fine — only a true ancestor-on-path is a cycle.
    fn validate_class(&mut self, class: &str, span: Span, path: &mut Vec<String>) -> bool {
        if path.iter().any(|c| c == class) {
            let chain = {
                let mut c = path.clone();
                c.push(class.to_string());
                c.join(" → ")
            };
            self.diag(
                span,
                format!("cannot hydrate `{class}`: its fields form a cycle ({chain})"),
                "E-DB-HYDRATE-CYCLE",
                "eager whole-graph hydration cannot resolve a self-referential relation; break the cycle (drop the back-reference from the row class, or load it with a second query)".into(),
            );
            return false;
        }
        let params = self.ctor_params.get(class).cloned().unwrap_or_default();
        if params.is_empty() {
            self.diag(
                span,
                format!("cannot hydrate `{class}`: it has no constructor to map columns into"),
                "E-DB-HYDRATE-NO-CTOR",
                format!(
                    "give `{class}` a promoted-field constructor — `constructor(public string name, public int age) {{}}`"
                ),
            );
            return false;
        }
        path.push(class.to_string());
        let mut ok = true;
        for p in &params {
            if !is_promoted(p) {
                ok = false;
                self.diag(
                    p.span,
                    format!(
                        "cannot hydrate `{class}`: constructor parameter `{}` is not a promoted field",
                        p.name
                    ),
                    "E-DB-HYDRATE-UNPROMOTED",
                    "every constructor parameter must be a promoted field (carry `public`/`private`/`protected`) so its name is the column name".into(),
                );
                continue;
            }
            match self.classify_field(&p.ty) {
                Some(FieldKind::Scalar { .. }) | Some(FieldKind::Json { .. }) => {}
                Some(FieldKind::Enum { name, .. }) => {
                    // A single column maps only to a ZERO-payload variant — reject an enum that has
                    // any data-carrying variant (cannot be built from one column) or no variants.
                    let variants = self.enum_variants.get(&name).cloned().unwrap_or_default();
                    if variants.is_empty() {
                        ok = false;
                        self.diag(
                            p.span,
                            format!(
                                "cannot hydrate field `{}` of `{class}`: enum `{name}` has no variants",
                                p.name
                            ),
                            "E-DB-HYDRATE-ENUM-PAYLOAD",
                            "a hydrated enum needs at least one zero-payload variant to map a column value onto".into(),
                        );
                    } else if variants.iter().any(|(_, arity)| *arity > 0) {
                        ok = false;
                        self.diag(
                            p.span,
                            format!(
                                "cannot hydrate field `{}` of `{class}`: enum `{name}` has a variant with associated data",
                                p.name
                            ),
                            "E-DB-HYDRATE-ENUM-PAYLOAD",
                            "a DB column maps only to a ZERO-payload enum variant (`enum Status { Active(), Inactive() }`); a variant carrying data cannot be built from a single column".into(),
                        );
                    }
                }
                Some(FieldKind::Entity { class: d, .. }) => {
                    if !self.validate_class(&d, p.span, path) {
                        ok = false;
                    }
                }
                None => {
                    ok = false;
                    self.diag(
                        p.span,
                        format!(
                            "cannot hydrate field `{}` of `{class}`: type `{}` has no DB column accessor",
                            p.name,
                            type_label(&p.ty)
                        ),
                        "E-DB-HYDRATE-FIELD-TYPE",
                        "a hydrated field must be a scalar (`int`/`string`/`float`/`bool`, or their `?` forms), or a class with a promoted-field constructor (nested/optional entity)".into(),
                    );
                }
            }
        }
        path.pop();
        ok
    }

    /// Validate + register a class-hydration helper (`queryInto`/`queryOneInto`), returning its name.
    fn ensure_class_helper(
        &mut self,
        kind: ClassKind,
        class: &str,
        span: Span,
        naming: Naming,
    ) -> Option<String> {
        let name = class_helper_name(kind, class, naming);
        if self.helpers.contains_key(&name) {
            return Some(name);
        }
        if !self.validate_class(class, span, &mut Vec::new()) {
            return None;
        }
        self.helpers.insert(
            name.clone(),
            HelperSpec::Class {
                kind,
                class: class.to_string(),
                naming,
            },
        );
        Some(name)
    }

    /// Resolve the row class `T` from the sink type: `List<T>` (`queryInto`) or `T?` (`queryOneInto`),
    /// then validate + register its helper.
    fn resolve_class(
        &mut self,
        kind: ClassKind,
        expected: Option<&Type>,
        span: Span,
        naming: Naming,
    ) -> Option<String> {
        let (method, want) = match kind {
            ClassKind::List => ("queryInto", "List<Row>"),
            ClassKind::One => ("queryOneInto", "Row?"),
        };
        let Some(expected) = expected else {
            self.diag(
                span,
                format!("`{method}()` has no type to infer its row class from"),
                "E-DB-INTO-NO-TYPE",
                format!(
                    "bind it to a typed declaration whose class it maps into — e.g. `{want} rows = stmt.{method}();`"
                ),
            );
            return None;
        };
        let inner: &Type = match (kind, expected) {
            (ClassKind::List, Type::Named { name, args, .. })
                if name == "List" && args.len() == 1 =>
            {
                &args[0]
            }
            (ClassKind::One, Type::Optional { inner, .. }) => inner,
            _ => {
                self.diag(
                    span,
                    format!(
                        "`{method}()` maps rows into `{want}`, but the binding's type is `{}`",
                        type_label(expected)
                    ),
                    "E-DB-INTO-BAD-SINK",
                    match kind {
                        ClassKind::List => "declare the binding `List<YourClass>`".into(),
                        ClassKind::One => "declare the binding `YourClass?`".into(),
                    },
                );
                return None;
            }
        };
        match inner {
            Type::Named { name, args, .. }
                if args.is_empty() && self.ctor_params.contains_key(name) =>
            {
                let class = name.clone();
                self.ensure_class_helper(kind, &class, span, naming)
            }
            _ => {
                self.diag(
                    span,
                    format!(
                        "`{method}()` must map into a user class; `{}` is not one",
                        type_label(inner)
                    ),
                    "E-DB-INTO-BAD-SINK",
                    "name a class with a promoted-field constructor as the row type".into(),
                );
                None
            }
        }
    }

    /// Resolve `streamInto<T>()` (DEC-208 item H): the sink type must be `DatabaseStream<Class>`; validates
    /// the class exactly like `queryInto` and registers a per-class stream helper.
    fn resolve_stream(
        &mut self,
        expected: Option<&Type>,
        span: Span,
        naming: Naming,
    ) -> Option<String> {
        let Some(expected) = expected else {
            self.diag(
                span,
                "`streamInto()` has no type to infer its row class from".into(),
                "E-DB-INTO-NO-TYPE",
                "bind it to a `DatabaseStream<YourClass>` declaration, or write the class explicitly — `stmt.streamInto<YourClass>()`".into(),
            );
            return None;
        };
        let inner: &Type = match expected {
            Type::Named { name, args, .. } if name == "DatabaseStream" && args.len() == 1 => {
                &args[0]
            }
            _ => {
                self.diag(
                    span,
                    format!(
                        "`streamInto()` produces a `DatabaseStream<Row-class>`, but the binding's type is `{}`",
                        type_label(expected)
                    ),
                    "E-DB-INTO-BAD-SINK",
                    "declare the binding `DatabaseStream<YourClass>` (or use the turbofish form)".into(),
                );
                return None;
            }
        };
        match inner {
            Type::Named { name, args, .. }
                if args.is_empty() && self.ctor_params.contains_key(name) =>
            {
                let class = name.clone();
                let helper = format!("phorjStreamInto{class}{}", naming_suffix(naming));
                if !self.helpers.contains_key(&helper) {
                    if !self.validate_class(&class, span, &mut Vec::new()) {
                        return None;
                    }
                    self.helpers
                        .insert(helper.clone(), HelperSpec::Stream { class, naming });
                }
                Some(helper)
            }
            _ => {
                self.diag(
                    span,
                    format!(
                        "`streamInto()` must map into a user class; `{}` is not one",
                        type_label(inner)
                    ),
                    "E-DB-INTO-BAD-SINK",
                    "name a class with a promoted-field constructor as the row type".into(),
                );
                None
            }
        }
    }

    /// Resolve `queryScalar<T>()`: the sink type IS `T` (a scalar, or an optional scalar), which must
    /// have a `Row` accessor. Registers a per-type scalar helper.
    fn resolve_scalar(&mut self, expected: Option<&Type>, span: Span) -> Option<String> {
        let Some(expected) = expected else {
            self.diag(
                span,
                "`queryScalar()` has no type to infer its value type from".into(),
                "E-DB-INTO-NO-TYPE",
                "bind it to a typed declaration — e.g. `int total = stmt.queryScalar();`".into(),
            );
            return None;
        };
        let Some(accessor) = accessor_for(expected) else {
            self.diag(
                span,
                format!(
                    "`queryScalar()` reads one column into a scalar, but the binding's type is `{}`",
                    type_label(expected)
                ),
                "E-DB-SCALAR-BAD-TYPE",
                "declare the binding a scalar — `int`/`string`/`float`/`bool`, or a `?` form".into(),
            );
            return None;
        };
        let label = scalar_label(expected).expect("accessor_for ⇒ a scalar label exists");
        let name = format!("phorjQueryScalar{label}");
        if !self.helpers.contains_key(&name) {
            self.helpers.insert(
                name.clone(),
                HelperSpec::Scalar {
                    accessor,
                    ret: expected.clone(),
                },
            );
        }
        Some(name)
    }

    /// Resolve `queryMap<K, V>()`: the sink type must be `Map<K, V>`. `K` must be an `int`/`string`
    /// scalar (the only map-key types); `V` is a scalar OR a hydratable entity (same rules as
    /// `queryInto`). Registers a per-(K, V) map helper.
    fn resolve_map(
        &mut self,
        expected: Option<&Type>,
        span: Span,
        naming: Naming,
    ) -> Option<String> {
        let Some(expected) = expected else {
            self.diag(
                span,
                "`queryMap()` has no type to infer its key/value types from".into(),
                "E-DB-MAP-BAD-SINK",
                "bind it to a `Map<K, V>` declaration — e.g. `Map<int, User> byId = stmt.queryMap();`"
                    .into(),
            );
            return None;
        };
        let (k_ty, v_ty) = match expected {
            Type::Named { name, args, .. } if name == "Map" && args.len() == 2 => {
                (args[0].clone(), args[1].clone())
            }
            _ => {
                self.diag(
                    span,
                    format!(
                        "`queryMap()` maps rows into `Map<K, V>`, but the binding's type is `{}`",
                        type_label(expected)
                    ),
                    "E-DB-MAP-BAD-SINK",
                    "declare the binding `Map<KeyType, ValueType>`".into(),
                );
                return None;
            }
        };
        // Key: int|string only (the map-key types), read by a non-nullable accessor.
        let key_acc = match accessor_for(&k_ty) {
            Some(a @ ("getInt" | "getString")) => a,
            _ => {
                self.diag(
                    span,
                    format!(
                        "`queryMap()` key column type `{}` is not a valid map key",
                        type_label(&k_ty)
                    ),
                    "E-DB-MAP-KEY-TYPE",
                    "a map key must be `int` or `string` (the first selected column)".into(),
                );
                return None;
            }
        };
        // Value: a scalar (second column) OR a hydratable entity (by field name).
        let val = if let Some(accessor) = accessor_for(&v_ty) {
            MapVal::Scalar { accessor }
        } else if let Type::Named { name, args, .. } = &v_ty {
            if args.is_empty() && self.ctor_params.contains_key(name) {
                if !self.validate_class(name, span, &mut Vec::new()) {
                    return None;
                }
                MapVal::Entity {
                    class: name.clone(),
                }
            } else {
                self.map_value_error(&v_ty, span);
                return None;
            }
        } else {
            self.map_value_error(&v_ty, span);
            return None;
        };
        let k_label = scalar_label(&k_ty).expect("int/string key ⇒ a scalar label exists");
        let v_label = match &val {
            MapVal::Scalar { .. } => scalar_label(&v_ty).expect("scalar value ⇒ a label"),
            MapVal::Entity { class } => class.clone(),
        };
        // The naming strategy only affects an ENTITY value (hydrated by field name); a scalar value is
        // read by column POSITION, so a scalar-value map is `Exact` regardless of any `namingStrategy`
        // prefix — the same helper is reused (no meaningless suffix proliferation).
        let eff_naming = match &val {
            MapVal::Entity { .. } => naming,
            MapVal::Scalar { .. } => Naming::Exact,
        };
        let name = format!(
            "phorjQueryMap{k_label}{v_label}{}",
            naming_suffix(eff_naming)
        );
        if !self.helpers.contains_key(&name) {
            self.helpers.insert(
                name.clone(),
                HelperSpec::Map {
                    key_acc,
                    key_ty: k_ty,
                    val,
                    val_ty: v_ty,
                    naming: eff_naming,
                },
            );
        }
        Some(name)
    }

    fn map_value_error(&mut self, v_ty: &Type, span: Span) {
        self.diag(
            span,
            format!(
                "`queryMap()` value type `{}` is neither a scalar column nor a hydratable class",
                type_label(v_ty)
            ),
            "E-DB-MAP-VALUE-TYPE",
            "the value must be a scalar (`int`/`string`/`float`/`bool`, or a `?` form) — the second selected column — or a class with a promoted-field constructor".into(),
        );
    }

    // ── synthesis (all nodes get unique synthetic spans) ─────────────────────────────────────────

    fn named(&mut self, n: &str) -> Type {
        let span = self.sp();
        Type::Named {
            name: n.into(),
            args: Vec::new(),
            span,
        }
    }

    fn generic1(&mut self, n: &str, arg: Type) -> Type {
        let span = self.sp();
        Type::Named {
            name: n.into(),
            args: vec![arg],
            span,
        }
    }

    fn list_ty(&mut self, class: &str) -> Type {
        let c = self.named(class);
        self.generic1("List", c)
    }

    fn opt_ty(&mut self, class: &str) -> Type {
        let c = self.named(class);
        let span = self.sp();
        Type::Optional {
            inner: Box::new(c),
            span,
        }
    }

    fn row_list_ty(&mut self) -> Type {
        let r = self.named("Row");
        self.generic1("List", r)
    }

    /// A fresh-span clone of a field type (scalars / optional scalars) — keeps every synthetic node's
    /// span unique even though the type text came from user source.
    fn retype(&mut self, t: &Type) -> Type {
        match t {
            Type::Named { name, args, .. } if args.is_empty() => self.named(name),
            Type::Optional { inner, .. } => {
                let i = self.retype(inner);
                let span = self.sp();
                Type::Optional {
                    inner: Box::new(i),
                    span,
                }
            }
            other => other.clone(),
        }
    }

    fn ident(&mut self, n: &str) -> Expr {
        let span = self.sp();
        Expr::Ident(n.into(), span)
    }

    fn str_lit(&mut self, s: &str) -> Expr {
        let span = self.sp();
        Expr::Str(vec![StrPart::Literal(s.into())], span)
    }

    fn int_lit(&mut self, n: i64) -> Expr {
        let span = self.sp();
        Expr::Int(n, span)
    }

    /// `obj.method(args)` — an instance method call (UFCS-resolved downstream).
    fn member_call(&mut self, obj: Expr, method: &str, args: Vec<Expr>) -> Expr {
        let msp = self.sp();
        let callee = Expr::Member {
            object: Box::new(obj),
            name: method.into(),
            safe: false,
            sep: MemberSep::Dot,
            span: msp,
        };
        let span = self.sp();
        Expr::Call {
            callee: Box::new(callee),
            args,
            span,
            type_args: Vec::new(),
        }
    }

    /// `Qualifier.method(args)` — a qualified module/native call (`List.append`, `List.length`).
    fn qual_call(&mut self, qualifier: &str, method: &str, args: Vec<Expr>) -> Expr {
        let q = self.ident(qualifier);
        self.member_call(q, method, args)
    }

    fn propagate(&mut self, inner: Expr) -> Expr {
        let span = self.sp();
        Expr::Propagate {
            inner: Box::new(inner),
            span,
        }
    }

    /// `new Class(args)` — construction (an `Expr::New` wrapping the plain `Call`, like the parser emits).
    fn new_obj(&mut self, class: &str, args: Vec<Expr>) -> Expr {
        let callee = self.ident(class);
        let csp = self.sp();
        let call = Expr::Call {
            callee: Box::new(callee),
            args,
            span: csp,
            type_args: Vec::new(),
        };
        let span = self.sp();
        Expr::New(Box::new(call), span)
    }

    /// A fresh unique per-field local name (`phorjV0`, `phorjV1`, …) — camelCase, disjoint from the
    /// fixed helper locals and every user field.
    fn fresh_local(&mut self) -> String {
        let n = self.next_local;
        self.next_local += 1;
        format!("phorjV{n}")
    }

    fn str_list_ty(&mut self) -> Type {
        let s = self.named("string");
        self.generic1("List", s)
    }

    fn map_ty(&mut self, k: &Type, v: &Type) -> Type {
        let kk = self.retype(k);
        let vv = self.retype(v);
        let span = self.sp();
        Type::Named {
            name: "Map".into(),
            args: vec![kk, vv],
            span,
        }
    }

    /// `list[n]` — an index expression on a local list binding.
    fn index_ident(&mut self, list: &str, n: i64) -> Expr {
        let obj = self.ident(list);
        let idx = self.int_lit(n);
        let span = self.sp();
        Expr::Index {
            object: Box::new(obj),
            index: Box::new(idx),
            span,
        }
    }

    /// `Row phorjRow = phorjRows[0];`
    fn row0_local(&mut self) -> Stmt {
        let index = self.index_ident("phorjRows", 0);
        let ty = self.named("Row");
        let span = self.sp();
        Stmt::VarDecl {
            ty,
            name: "phorjRow".into(),
            init: index,
            mutable: false,
            span,
        }
    }

    /// `if (List.length(<list_var>) <op> <n>) { throw new DatabaseError(<msg>); }` — an arity guard.
    fn len_guard(&mut self, list_var: &str, op: BinaryOp, n: i64, msg: &str) -> Stmt {
        let v = self.ident(list_var);
        let len = self.qual_call("List", "length", vec![v]);
        let rhs = self.int_lit(n);
        let cmp_span = self.sp();
        let cond = Expr::Binary {
            op,
            lhs: Box::new(len),
            rhs: Box::new(rhs),
            span: cmp_span,
        };
        let m = self.str_lit(msg);
        let err = self.new_obj("DatabaseError", vec![m]);
        let throw_span = self.sp();
        let throw = Stmt::Throw {
            value: err,
            span: throw_span,
        };
        let if_span = self.sp();
        Stmt::If {
            cond,
            bind: None,
            then_block: vec![throw],
            else_block: None,
            span: if_span,
        }
    }

    /// `new EnumName.Variant()` — construct a zero-payload enum variant (qualified, DEC-208 slice E).
    fn new_variant(&mut self, enum_name: &str, variant: &str) -> Expr {
        let obj = self.ident(enum_name);
        let msp = self.sp();
        let callee = Expr::Member {
            object: Box::new(obj),
            name: variant.into(),
            safe: false,
            sep: MemberSep::Dot,
            span: msp,
        };
        let csp = self.sp();
        let call = Expr::Call {
            callee: Box::new(callee),
            args: Vec::new(),
            span: csp,
            type_args: Vec::new(),
        };
        let nsp = self.sp();
        Expr::New(Box::new(call), nsp)
    }

    /// `string <fresh> = <row_var>.getString("col")?;` — emit the statement, return the local name.
    fn getstr_local(&mut self, col: &str, row_var: &str, out: &mut Vec<Stmt>) -> String {
        let local = self.fresh_local();
        let row = self.ident(row_var);
        let col_e = self.str_lit(col);
        let acc = self.member_call(row, "getString", vec![col_e]);
        let init = self.propagate(acc);
        let ty = self.named("string");
        let span = self.sp();
        out.push(Stmt::VarDecl {
            ty,
            name: local.clone(),
            init,
            mutable: false,
            span,
        });
        local
    }

    /// `!(<row_var>.isNull("col")?)` — the guard for an OPTIONAL enum/Json field (a NULL column → the
    /// field stays `null`; reuses the strict `isNull` accessor, matching the optional-entity path).
    fn not_is_null(&mut self, col: &str, row_var: &str) -> Expr {
        let row = self.ident(row_var);
        let col_e = self.str_lit(col);
        let call = self.member_call(row, "isNull", vec![col_e]);
        let p = self.propagate(call);
        let span = self.sp();
        Expr::Unary {
            op: UnaryOp::Not,
            expr: Box::new(p),
            span,
        }
    }

    /// `match (<scrutinee_local>) { "V0" => new Enum.V0(), … default => DatabaseError.fail(msg)? }` — the
    /// enum-column → variant match (DEC-208 slice E). `enum_name`'s variants are all zero-payload
    /// (validated), so each arm is a nullary construction; an unmatched value throws a catchable
    /// `DatabaseError` via the prelude's single `DatabaseError.fail` classification point.
    fn enum_match(&mut self, enum_name: &str, scrutinee_local: &str, col: &str) -> Expr {
        let variants = self
            .enum_variants
            .get(enum_name)
            .cloned()
            .unwrap_or_default();
        let mut arms = Vec::new();
        for (variant, _arity) in &variants {
            let pat_span = self.sp();
            let pattern = Pattern::Str(variant.clone(), pat_span);
            let body = self.new_variant(enum_name, variant);
            let arm_span = self.sp();
            arms.push(MatchArm {
                pattern,
                guard: None,
                body,
                span: arm_span,
            });
        }
        let msg = format!(
            "Core.DatabaseModule: column `{col}` value is not a variant of enum `{enum_name}`"
        );
        let m = self.str_lit(&msg);
        let fail = self.qual_call("DatabaseError", "fail", vec![m]);
        let body = self.propagate(fail);
        let pat_span = self.sp();
        let arm_span = self.sp();
        arms.push(MatchArm {
            pattern: Pattern::Wildcard(pat_span),
            guard: None,
            body,
            span: arm_span,
        });
        let scrut = self.ident(scrutinee_local);
        let span = self.sp();
        Expr::Match {
            scrutinee: Box::new(scrut),
            arms,
            span,
        }
    }

    /// `Json.parse(<row_var>.getString("col")?) ?? DatabaseError.fail(msg)?` — the JSON-column → `Json` parse
    /// (DEC-208 slice E). Requires the program's own `import Core.Json` (nothing in the wind); an
    /// invalid JSON string (`Json.parse` → null) throws a catchable `DatabaseError`.
    fn json_parse_expr(&mut self, col: &str, row_var: &str) -> Expr {
        let row = self.ident(row_var);
        let col_e = self.str_lit(col);
        let getstr = self.member_call(row, "getString", vec![col_e]);
        let getstr_p = self.propagate(getstr);
        let parse = self.qual_call("Json", "parse", vec![getstr_p]);
        let msg = format!("Core.DatabaseModule: column `{col}` does not contain valid JSON");
        let m = self.str_lit(&msg);
        let fail = self.qual_call("DatabaseError", "fail", vec![m]);
        let fail_p = self.propagate(fail);
        let span = self.sp();
        Expr::Binary {
            op: BinaryOp::Coalesce,
            lhs: Box::new(parse),
            rhs: Box::new(fail_p),
            span,
        }
    }

    /// Recursively construct `class` from `row_var`, resolving fields by the dotted column convention
    /// (`prefix.field`). Emits the per-field extracting locals into `out` (a scalar → one `getX` local;
    /// a required entity → its sub-locals then a `new D(..)` local; an optional entity → a
    /// `D? = null` local guarded by an `if (!(all-columns-null)) { … }`), then returns `new Class(..)`.
    /// Assumes `class` is validated (`validate_class`), so `classify_field` never returns `None`.
    fn build_class(
        &mut self,
        class: &str,
        prefix: &str,
        row_var: &str,
        out: &mut Vec<Stmt>,
    ) -> Expr {
        let params = self.ctor_params.get(class).cloned().unwrap_or_default();
        let mut args = Vec::new();
        for p in &params {
            let col = self.col(prefix, &p.name);
            let local = self.fresh_local();
            match self
                .classify_field(&p.ty)
                .expect("validate_class ⇒ every field classifies")
            {
                FieldKind::Enum { name: en, optional } => {
                    if optional {
                        // mutable EnumT? local = null; if (!row.isNull("col")?) { string s =
                        // row.getString("col")?; local = match (s) { … }; }
                        let opt_ty = self.opt_ty(&en);
                        let null_init = self.placeholder();
                        let decl_span = self.sp();
                        out.push(Stmt::VarDecl {
                            ty: opt_ty,
                            name: local.clone(),
                            init: null_init,
                            mutable: true,
                            span: decl_span,
                        });
                        let cond = self.not_is_null(&col, row_var);
                        let mut ifb = Vec::new();
                        let sn = self.getstr_local(&col, row_var, &mut ifb);
                        let m = self.enum_match(&en, &sn, &col);
                        let target = self.ident(&local);
                        let assign_span = self.sp();
                        ifb.push(Stmt::Assign {
                            target,
                            value: m,
                            span: assign_span,
                        });
                        let if_span = self.sp();
                        out.push(Stmt::If {
                            cond,
                            bind: None,
                            then_block: ifb,
                            else_block: None,
                            span: if_span,
                        });
                    } else {
                        // string s = row.getString("col")?; EnumT local = match (s) { … };
                        let sn = self.getstr_local(&col, row_var, out);
                        let m = self.enum_match(&en, &sn, &col);
                        let ty = self.named(&en);
                        let span = self.sp();
                        out.push(Stmt::VarDecl {
                            ty,
                            name: local.clone(),
                            init: m,
                            mutable: false,
                            span,
                        });
                    }
                    args.push(self.ident(&local));
                }
                FieldKind::Json { optional } => {
                    if optional {
                        // mutable Json? local = null; if (!row.isNull("col")?) { local =
                        // Json.parse(row.getString("col")?) ?? DatabaseError.fail(…)?; }
                        let opt_ty = self.opt_ty("Json");
                        let null_init = self.placeholder();
                        let decl_span = self.sp();
                        out.push(Stmt::VarDecl {
                            ty: opt_ty,
                            name: local.clone(),
                            init: null_init,
                            mutable: true,
                            span: decl_span,
                        });
                        let cond = self.not_is_null(&col, row_var);
                        let j = self.json_parse_expr(&col, row_var);
                        let target = self.ident(&local);
                        let assign_span = self.sp();
                        let ifb = vec![Stmt::Assign {
                            target,
                            value: j,
                            span: assign_span,
                        }];
                        let if_span = self.sp();
                        out.push(Stmt::If {
                            cond,
                            bind: None,
                            then_block: ifb,
                            else_block: None,
                            span: if_span,
                        });
                    } else {
                        // Json local = Json.parse(row.getString("col")?) ?? DatabaseError.fail(…)?;
                        let j = self.json_parse_expr(&col, row_var);
                        let ty = self.named("Json");
                        let span = self.sp();
                        out.push(Stmt::VarDecl {
                            ty,
                            name: local.clone(),
                            init: j,
                            mutable: false,
                            span,
                        });
                    }
                    args.push(self.ident(&local));
                }
                FieldKind::Scalar { accessor } => {
                    // <ty> local = row.accessor("col")?;
                    let row = self.ident(row_var);
                    let col_e = self.str_lit(&col);
                    let acc = self.member_call(row, accessor, vec![col_e]);
                    let init = self.propagate(acc);
                    let ty = self.retype(&p.ty);
                    let span = self.sp();
                    out.push(Stmt::VarDecl {
                        ty,
                        name: local.clone(),
                        init,
                        mutable: false,
                        span,
                    });
                    args.push(self.ident(&local));
                }
                FieldKind::Entity {
                    class: d,
                    optional: false,
                } => {
                    // required: emit D's locals, then `D local = new D(..);`
                    let sub = self.build_class(&d, &col, row_var, out);
                    let ty = self.named(&d);
                    let span = self.sp();
                    out.push(Stmt::VarDecl {
                        ty,
                        name: local.clone(),
                        init: sub,
                        mutable: false,
                        span,
                    });
                    args.push(self.ident(&local));
                }
                FieldKind::Entity {
                    class: d,
                    optional: true,
                } => {
                    // optional: `D? local = null; if (!(all-null)) { <D locals>; local = new D(..); }`
                    let opt_ty = self.opt_ty(&d);
                    let null_init = self.placeholder();
                    let decl_span = self.sp();
                    out.push(Stmt::VarDecl {
                        ty: opt_ty,
                        name: local.clone(),
                        init: null_init,
                        mutable: true,
                        span: decl_span,
                    });
                    let all_null = self.all_null(&d, &col, row_var);
                    let not_span = self.sp();
                    let cond = Expr::Unary {
                        op: UnaryOp::Not,
                        expr: Box::new(all_null),
                        span: not_span,
                    };
                    let mut ifb = Vec::new();
                    let sub = self.build_class(&d, &col, row_var, &mut ifb);
                    let target = self.ident(&local);
                    let assign_span = self.sp();
                    ifb.push(Stmt::Assign {
                        target,
                        value: sub,
                        span: assign_span,
                    });
                    let if_span = self.sp();
                    out.push(Stmt::If {
                        cond,
                        bind: None,
                        then_block: ifb,
                        else_block: None,
                        span: if_span,
                    });
                    args.push(self.ident(&local));
                }
            }
        }
        self.new_obj(class, args)
    }

    /// A boolean expression true iff EVERY (recursively-reached) leaf column of `class` at `prefix` is
    /// SQL NULL — `row.isNull("c0")? && row.isNull("c1")? && …`. Drives an optional nested entity's
    /// "LEFT JOIN missed → the whole entity is null" test. A validated tree always has ≥1 leaf.
    fn all_null(&mut self, class: &str, prefix: &str, row_var: &str) -> Expr {
        let mut leaves = Vec::new();
        self.collect_leaves(class, prefix, &mut leaves);
        let mut terms = Vec::new();
        for col in leaves {
            let row = self.ident(row_var);
            let col_e = self.str_lit(&col);
            let call = self.member_call(row, "isNull", vec![col_e]);
            terms.push(self.propagate(call));
        }
        let mut it = terms.into_iter();
        let mut acc = it
            .next()
            .expect("a validated entity has at least one scalar leaf column");
        for t in it {
            let span = self.sp();
            acc = Expr::Binary {
                op: BinaryOp::And,
                lhs: Box::new(acc),
                rhs: Box::new(t),
                span,
            };
        }
        acc
    }

    /// Collect every scalar leaf column name of `class` at `prefix`, recursing through nested entities
    /// (required and optional alike). Terminates: a validated tree is acyclic.
    fn collect_leaves(&self, class: &str, prefix: &str, acc: &mut Vec<String>) {
        let params = self.ctor_params.get(class).cloned().unwrap_or_default();
        for p in &params {
            let col = self.col(prefix, &p.name);
            match self.classify_field(&p.ty) {
                // A scalar, an enum, and a Json field each occupy exactly ONE column (a leaf).
                Some(FieldKind::Scalar { .. })
                | Some(FieldKind::Enum { .. })
                | Some(FieldKind::Json { .. }) => acc.push(col),
                Some(FieldKind::Entity { class: d, .. }) => self.collect_leaves(&d, &col, acc),
                None => {}
            }
        }
    }

    fn synth_helper(&mut self, name: &str, spec: &HelperSpec) -> Item {
        // DEC-208 slice B2 — set the strategy for THIS helper's column literals before any body node is
        // built (the recursive `build_class`/`collect_leaves`/`all_null` all read `current_naming` via
        // `col`/`seg`). Must be the first line: synthesis is one-helper-at-a-time.
        self.current_naming = match spec {
            HelperSpec::Class { naming, .. }
            | HelperSpec::Map { naming, .. }
            | HelperSpec::Stream { naming, .. } => *naming,
            HelperSpec::Scalar { .. } => Naming::Exact,
        };
        let stmt_ty = self.named("Statement");
        let psp = self.sp();
        let param = Param {
            ty: stmt_ty,
            name: "phorjStmt".into(),
            default: None,
            span: psp,
        };
        let (ret, body) = match spec {
            HelperSpec::Class {
                kind: ClassKind::List,
                class,
                ..
            } => {
                let ty = self.list_ty(class);
                let c = class.clone();
                (ty, self.list_body(&c))
            }
            HelperSpec::Class {
                kind: ClassKind::One,
                class,
                ..
            } => {
                let ty = self.opt_ty(class);
                let c = class.clone();
                (ty, self.one_body(&c))
            }
            HelperSpec::Scalar { accessor, ret } => {
                let r = self.retype(ret);
                (r, self.scalar_body(accessor))
            }
            HelperSpec::Map {
                key_acc,
                key_ty,
                val,
                val_ty,
                ..
            } => {
                let ret = self.map_ty(key_ty, val_ty);
                let k = key_ty.clone();
                let v = val_ty.clone();
                // `val` (MapVal) is a struct ref; extract its data before the mutable-self body call.
                let body = self.map_body(key_acc, &k, val, &v);
                (ret, body)
            }
            HelperSpec::Stream { class, .. } => {
                let c = class.clone();
                let ct = self.named(&c);
                let sp = self.sp();
                let ret = Type::Named {
                    name: "DatabaseStream".into(),
                    args: vec![ct],
                    span: sp,
                };
                let body = self.stream_body(&c);
                (ret, body)
            }
        };
        let throws = vec![self.named("DatabaseError")];
        let span = self.sp();
        Item::Function(FunctionDecl {
            modifiers: Vec::new(),
            attrs: Vec::new(),
            vis: Visibility::Public,
            name: name.to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            params: vec![param],
            ret: Some(ret),
            throws,
            body,
            foreign: false,
            generic_ret_from_param: None,
            span,
        })
    }

    /// `List<Row> phorjRows = phorjStmt.query()?;` — the shared first statement of every helper.
    fn query_rows_stmt(&mut self) -> Stmt {
        let s = self.ident("phorjStmt");
        let q = self.member_call(s, "query", vec![]);
        let init = self.propagate(q);
        let ty = self.row_list_ty();
        let span = self.sp();
        Stmt::VarDecl {
            ty,
            name: "phorjRows".into(),
            init,
            mutable: false,
            span,
        }
    }

    fn list_body(&mut self, class: &str) -> Vec<Stmt> {
        let mut body = Vec::new();
        body.push(self.query_rows_stmt());
        // mutable List<Class> phorjOut = new List<Class>();
        let out_ty = self.list_ty(class);
        let ct = self.named(class);
        let coll_span = self.sp();
        let coll = Expr::NewColl {
            kind: CollKind::List,
            args: vec![ct],
            span: coll_span,
        };
        let out_span = self.sp();
        body.push(Stmt::VarDecl {
            ty: out_ty,
            name: "phorjOut".into(),
            init: coll,
            mutable: true,
            span: out_span,
        });
        // for (Row phorjRow in phorjRows) { <build locals>; phorjOut = List.append(phorjOut, new Class(..)); }
        let mut loop_body = Vec::new();
        let newc = self.build_class(class, "", "phorjRow", &mut loop_body);
        let out_ref = self.ident("phorjOut");
        let appended = self.qual_call("List", "append", vec![out_ref, newc]);
        let target = self.ident("phorjOut");
        let assign_span = self.sp();
        loop_body.push(Stmt::Assign {
            target,
            value: appended,
            span: assign_span,
        });
        let for_ty = self.named("Row");
        let iter = self.ident("phorjRows");
        let for_span = self.sp();
        body.push(Stmt::For {
            ty: for_ty,
            name: "phorjRow".into(),
            val: None,
            iter,
            body: loop_body,
            span: for_span,
        });
        let ret_val = self.ident("phorjOut");
        let ret_span = self.sp();
        body.push(Stmt::Return {
            value: Some(ret_val),
            span: ret_span,
        });
        body
    }

    /// The `phorjStreamInto<Class>` body (DEC-208 item H):
    /// ```text
    /// RowStream phorjRows = phorjStmt.stream()?;
    /// return new DatabaseStream(phorjRows, function(Row phorjRow): Class throws DatabaseError {
    ///     <build_class locals>; return new Class(...);
    /// });
    /// ```
    /// `T` of the generic `DatabaseStream<T>` is inferred at construction from the closure's declared return
    /// type; the per-row hydration is the SAME `build_class` output `queryInto` inlines into its loop,
    /// here wrapped in a throwing lambda (DEC-222) so hydration runs lazily per pulled row.
    fn stream_body(&mut self, class: &str) -> Vec<Stmt> {
        let mut body = Vec::new();
        // RowStream phorjRows = phorjStmt.stream()?;
        let s = self.ident("phorjStmt");
        let call = self.member_call(s, "stream", vec![]);
        let init = self.propagate(call);
        let rs_ty = self.named("RowStream");
        let span = self.sp();
        body.push(Stmt::VarDecl {
            ty: rs_ty,
            name: "phorjRows".into(),
            init,
            mutable: false,
            span,
        });
        // The hydration lambda: function(Row phorjRow): Class throws DatabaseError { …; return new Class(..); }
        let mut lam_body = Vec::new();
        let newc = self.build_class(class, "", "phorjRow", &mut lam_body);
        let ret_span = self.sp();
        lam_body.push(Stmt::Return {
            value: Some(newc),
            span: ret_span,
        });
        let row_ty = self.named("Row");
        let psp = self.sp();
        let lam_span = self.sp();
        let ret_ty = self.named(class);
        let err_ty = self.named("DatabaseError");
        let lambda = Expr::Lambda {
            params: vec![Param {
                ty: row_ty,
                name: "phorjRow".into(),
                default: None,
                span: psp,
            }],
            ret: Some(ret_ty),
            throws: vec![err_ty],
            body: LambdaBody::Block(lam_body),
            span: lam_span,
        };
        // return new DatabaseStream(phorjRows, <lambda>);
        let rows_ref = self.ident("phorjRows");
        let stream = self.new_obj("DatabaseStream", vec![rows_ref, lambda]);
        let rsp = self.sp();
        body.push(Stmt::Return {
            value: Some(stream),
            span: rsp,
        });
        body
    }

    fn one_body(&mut self, class: &str) -> Vec<Stmt> {
        let mut body = Vec::new();
        body.push(self.query_rows_stmt());
        // int phorjN = List.length(phorjRows);
        let rows_ref = self.ident("phorjRows");
        let len = self.qual_call("List", "length", vec![rows_ref]);
        let int_ty = self.named("int");
        let n_span = self.sp();
        body.push(Stmt::VarDecl {
            ty: int_ty,
            name: "phorjN".into(),
            init: len,
            mutable: false,
            span: n_span,
        });
        // if (phorjN == 0) { return null; }
        let n0 = self.ident("phorjN");
        let zero = self.int_lit(0);
        let eq_span = self.sp();
        let eq = Expr::Binary {
            op: BinaryOp::Eq,
            lhs: Box::new(n0),
            rhs: Box::new(zero),
            span: eq_span,
        };
        let null_val = self.placeholder();
        let ret_span = self.sp();
        let null_ret = Stmt::Return {
            value: Some(null_val),
            span: ret_span,
        };
        let if0_span = self.sp();
        body.push(Stmt::If {
            cond: eq,
            bind: None,
            then_block: vec![null_ret],
            else_block: None,
            span: if0_span,
        });
        // if (phorjN > 1) { throw new DatabaseError("…"); }
        let n1 = self.ident("phorjN");
        let one = self.int_lit(1);
        let gt_span = self.sp();
        let gt = Expr::Binary {
            op: BinaryOp::Gt,
            lhs: Box::new(n1),
            rhs: Box::new(one),
            span: gt_span,
        };
        let msg = self.str_lit(&format!(
            "Core.DatabaseModule.queryOneInto: expected at most one row for `{class}`"
        ));
        let dberr = self.new_obj("DatabaseError", vec![msg]);
        let throw_span = self.sp();
        let throw = Stmt::Throw {
            value: dberr,
            span: throw_span,
        };
        let if1_span = self.sp();
        body.push(Stmt::If {
            cond: gt,
            bind: None,
            then_block: vec![throw],
            else_block: None,
            span: if1_span,
        });
        // Row phorjRow = phorjRows[0];
        body.push(self.row0_local());
        // <build locals>; return new Class(..);
        let mut pre = Vec::new();
        let newc = self.build_class(class, "", "phorjRow", &mut pre);
        body.extend(pre);
        let ret_span2 = self.sp();
        body.push(Stmt::Return {
            value: Some(newc),
            span: ret_span2,
        });
        body
    }

    /// `queryScalar` body: exactly one row AND exactly one column → the typed value; else `DatabaseError`.
    /// The sole column name is unknown at compile time (`COUNT(*)`), so it is read via `columnNames`.
    fn scalar_body(&mut self, accessor: &str) -> Vec<Stmt> {
        let mut body = Vec::new();
        body.push(self.query_rows_stmt());
        body.push(self.len_guard(
            "phorjRows",
            BinaryOp::NotEq,
            1,
            "Core.DatabaseModule.queryScalar: expected exactly one row",
        ));
        body.push(self.row0_local());
        // List<string> phorjCols = phorjRow.columnNames()?;
        let row = self.ident("phorjRow");
        let cn = self.member_call(row, "columnNames", vec![]);
        let cn_init = self.propagate(cn);
        let cols_ty = self.str_list_ty();
        let cols_span = self.sp();
        body.push(Stmt::VarDecl {
            ty: cols_ty,
            name: "phorjCols".into(),
            init: cn_init,
            mutable: false,
            span: cols_span,
        });
        body.push(self.len_guard(
            "phorjCols",
            BinaryOp::NotEq,
            1,
            "Core.DatabaseModule.queryScalar: expected exactly one column",
        ));
        // return phorjRow.accessor(phorjCols[0])?;
        let row2 = self.ident("phorjRow");
        let cols0 = self.index_ident("phorjCols", 0);
        let acc = self.member_call(row2, accessor, vec![cols0]);
        let ret_v = self.propagate(acc);
        let ret_span = self.sp();
        body.push(Stmt::Return {
            value: Some(ret_v),
            span: ret_span,
        });
        body
    }

    /// `queryMap` body: index rows into a `Map<K, V>` keyed by the FIRST column; V is the SECOND column
    /// (scalar) or an entity hydrated by field name from the whole row (nested rules as `queryInto`).
    fn map_body(&mut self, key_acc: &str, key_ty: &Type, val: &MapVal, val_ty: &Type) -> Vec<Stmt> {
        let mut body = Vec::new();
        body.push(self.query_rows_stmt());
        // mutable Map<K,V> phorjOut = new Map<K,V>();
        let out_ty = self.map_ty(key_ty, val_ty);
        let kk = self.retype(key_ty);
        let vv = self.retype(val_ty);
        let coll_span = self.sp();
        let coll = Expr::NewColl {
            kind: CollKind::Map,
            args: vec![kk, vv],
            span: coll_span,
        };
        let out_span = self.sp();
        body.push(Stmt::VarDecl {
            ty: out_ty,
            name: "phorjOut".into(),
            init: coll,
            mutable: true,
            span: out_span,
        });
        // for (Row phorjRow in phorjRows) { … }
        let mut loop_body = Vec::new();
        // List<string> phorjCols = phorjRow.columnNames()?;
        let row = self.ident("phorjRow");
        let cn = self.member_call(row, "columnNames", vec![]);
        let cn_init = self.propagate(cn);
        let cols_ty = self.str_list_ty();
        let cols_span = self.sp();
        loop_body.push(Stmt::VarDecl {
            ty: cols_ty,
            name: "phorjCols".into(),
            init: cn_init,
            mutable: false,
            span: cols_span,
        });
        // K phorjKey = phorjRow.<key_acc>(phorjCols[0])?;
        let row2 = self.ident("phorjRow");
        let cols0 = self.index_ident("phorjCols", 0);
        let kacc = self.member_call(row2, key_acc, vec![cols0]);
        let k_init = self.propagate(kacc);
        let k_ty2 = self.retype(key_ty);
        let k_span = self.sp();
        loop_body.push(Stmt::VarDecl {
            ty: k_ty2,
            name: "phorjKey".into(),
            init: k_init,
            mutable: false,
            span: k_span,
        });
        // V phorjVal = <scalar second column | hydrated entity>;
        let val_expr = match val {
            MapVal::Scalar { accessor, .. } => {
                loop_body.push(self.len_guard(
                    "phorjCols",
                    BinaryOp::Lt,
                    2,
                    "Core.DatabaseModule.queryMap: expected at least two columns for a scalar value",
                ));
                let row3 = self.ident("phorjRow");
                let cols1 = self.index_ident("phorjCols", 1);
                let vacc = self.member_call(row3, accessor, vec![cols1]);
                self.propagate(vacc)
            }
            MapVal::Entity { class } => {
                let c = class.clone();
                self.build_class(&c, "", "phorjRow", &mut loop_body)
            }
        };
        let v_ty2 = self.retype(val_ty);
        let v_span = self.sp();
        loop_body.push(Stmt::VarDecl {
            ty: v_ty2,
            name: "phorjVal".into(),
            init: val_expr,
            mutable: false,
            span: v_span,
        });
        // phorjOut = Map.set(phorjOut, phorjKey, phorjVal);
        let out_ref = self.ident("phorjOut");
        let key_ref = self.ident("phorjKey");
        let val_ref = self.ident("phorjVal");
        let set = self.qual_call("Map", "set", vec![out_ref, key_ref, val_ref]);
        let target = self.ident("phorjOut");
        let assign_span = self.sp();
        loop_body.push(Stmt::Assign {
            target,
            value: set,
            span: assign_span,
        });
        let for_ty = self.named("Row");
        let iter = self.ident("phorjRows");
        let for_span = self.sp();
        body.push(Stmt::For {
            ty: for_ty,
            name: "phorjRow".into(),
            val: None,
            iter,
            body: loop_body,
            span: for_span,
        });
        let ret_val = self.ident("phorjOut");
        let ret_span = self.sp();
        body.push(Stmt::Return {
            value: Some(ret_val),
            span: ret_span,
        });
        body
    }

    // ── the total walk (mirrors desugar_di) ──────────────────────────────────────────────────────

    fn ritem(&mut self, it: Item) -> Item {
        match it {
            Item::Function(mut f) => {
                self.rfn(&mut f);
                Item::Function(f)
            }
            Item::Class(mut c) => {
                for m in &mut c.members {
                    self.rmember(m);
                }
                Item::Class(c)
            }
            Item::Trait(mut t) => {
                for m in &mut t.members {
                    self.rmember(m);
                }
                Item::Trait(t)
            }
            other => other,
        }
    }

    fn rfn(&mut self, f: &mut FunctionDecl) {
        let prev_ret = std::mem::replace(&mut self.current_ret, f.ret.clone());
        let body = std::mem::take(&mut f.body);
        f.body = self.rblock(body);
        for p in &mut f.params {
            if let Some(d) = p.default.take() {
                p.default = Some(Box::new(self.rexpr(*d)));
            }
        }
        self.current_ret = prev_ret;
    }

    fn rmember(&mut self, m: &mut ClassMember) {
        match m {
            ClassMember::Method(f) => self.rfn(f),
            ClassMember::Field { init, .. } => {
                if let Some(e) = init.take() {
                    *init = Some(self.rexpr(e));
                }
            }
            ClassMember::Constructor { body, .. } => {
                let b = std::mem::take(body);
                *body = self.rblock(b);
            }
            ClassMember::Hook { get, set, .. } => {
                if let Some(e) = get.take() {
                    *get = Some(self.rexpr(e));
                }
                if let Some((_param, stmts)) = set {
                    let b = std::mem::take(stmts);
                    *stmts = self.rblock(b);
                }
            }
        }
    }

    fn rblock(&mut self, stmts: Vec<Stmt>) -> Vec<Stmt> {
        stmts.into_iter().map(|s| self.rstmt(s)).collect()
    }

    /// An expression in an *annotation position* (typed var init, `return`, lambda expr-body): a
    /// `queryInto`/`queryOneInto` here draws `T` from `expected`. `Type::Infer` (`var`) is not an
    /// annotation → stripped to `None`.
    fn rexpr_expected(&mut self, e: Expr, expected: Option<&Type>) -> Expr {
        let expected = expected.filter(|t| !matches!(t, Type::Infer(_)));
        match e {
            Expr::Call {
                callee,
                args,
                span,
                type_args,
            } if args.is_empty() => match Self::query_call_kind(&callee) {
                Some(kind) => self.rewrite(*callee, kind, expected, type_args, span),
                None => {
                    let callee = Box::new(self.rexpr(*callee));
                    Expr::Call {
                        callee,
                        args: Vec::new(),
                        span,
                        type_args,
                    }
                }
            },
            // `List<User> u = stmt.queryInto()?;` — a `?`-propagation in annotation position: look
            // THROUGH the `?` so the sink type still reaches the recognizer, then keep the `?` on the
            // rewritten throwing helper call (the idiomatic form inside a `throws DatabaseError` function).
            Expr::Propagate { inner, span } => match *inner {
                Expr::Call {
                    callee,
                    args,
                    span: cspan,
                    type_args,
                } if args.is_empty() && Self::query_call_kind(&callee).is_some() => {
                    let kind = Self::query_call_kind(&callee).expect("guarded above");
                    let rewritten = self.rewrite(*callee, kind, expected, type_args, cspan);
                    Expr::Propagate {
                        inner: Box::new(rewritten),
                        span,
                    }
                }
                other => Expr::Propagate {
                    inner: Box::new(self.rexpr(other)),
                    span,
                },
            },
            other => self.rexpr(other),
        }
    }

    fn rexpr(&mut self, e: Expr) -> Expr {
        match e {
            // A nullary call may be a `recv.queryInto()` — here there is no annotation, so an explicit
            // turbofish is the ONLY sink source (`var users = stmt.queryInto<User>();` now works);
            // without one it is `E-DB-INTO-NO-TYPE` (via `rewrite` with `expected = None`).
            Expr::Call {
                callee,
                args,
                span,
                type_args,
            } if args.is_empty() => match Self::query_call_kind(&callee) {
                Some(kind) => self.rewrite(*callee, kind, None, type_args, span),
                None => {
                    let callee = Box::new(self.rexpr(*callee));
                    Expr::Call {
                        callee,
                        args: Vec::new(),
                        span,
                        type_args,
                    }
                }
            },
            Expr::Call {
                callee,
                args,
                span,
                type_args,
            } => Expr::Call {
                callee: Box::new(self.rexpr(*callee)),
                args: args.into_iter().map(|a| self.rexpr(a)).collect(),
                span,
                type_args,
            },
            Expr::ParentCall {
                ancestor,
                method,
                args,
                span,
            } => Expr::ParentCall {
                ancestor,
                method,
                args: args.into_iter().map(|a| self.rexpr(a)).collect(),
                span,
            },
            Expr::OverloadSelect { ty, call, span } => Expr::OverloadSelect {
                ty,
                call: Box::new(self.rexpr(*call)),
                span,
            },
            Expr::Str(parts, span) => Expr::Str(self.rparts(parts), span),
            Expr::Html(parts, span) => Expr::Html(self.rparts(parts), span),
            Expr::TaggedTemplate { tag, parts, span } => Expr::TaggedTemplate {
                tag,
                parts: self.rparts(parts),
                span,
            },
            Expr::List(items, span) => {
                Expr::List(items.into_iter().map(|e| self.rexpr(e)).collect(), span)
            }
            Expr::Map(pairs, span) => Expr::Map(
                pairs
                    .into_iter()
                    .map(|(k, v)| (self.rexpr(k), self.rexpr(v)))
                    .collect(),
                span,
            ),
            Expr::NewColl { kind, args, span } => Expr::NewColl { kind, args, span },
            Expr::Unary { op, expr, span } => Expr::Unary {
                op,
                expr: Box::new(self.rexpr(*expr)),
                span,
            },
            Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
                op,
                lhs: Box::new(self.rexpr(*lhs)),
                rhs: Box::new(self.rexpr(*rhs)),
                span,
            },
            Expr::InstanceOf {
                value,
                type_name,
                span,
            } => Expr::InstanceOf {
                value: Box::new(self.rexpr(*value)),
                type_name,
                span,
            },
            Expr::Cast {
                value,
                type_name,
                span,
            } => Expr::Cast {
                value: Box::new(self.rexpr(*value)),
                type_name,
                span,
            },
            Expr::Member {
                object,
                name,
                safe,
                sep,
                span,
            } => Expr::Member {
                object: Box::new(self.rexpr(*object)),
                name,
                safe,
                sep,
                span,
            },
            Expr::Index {
                object,
                index,
                span,
            } => Expr::Index {
                object: Box::new(self.rexpr(*object)),
                index: Box::new(self.rexpr(*index)),
                span,
            },
            Expr::Force { inner, span } => Expr::Force {
                inner: Box::new(self.rexpr(*inner)),
                span,
            },
            Expr::Propagate { inner, span } => Expr::Propagate {
                inner: Box::new(self.rexpr(*inner)),
                span,
            },
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => Expr::Match {
                scrutinee: Box::new(self.rexpr(*scrutinee)),
                arms: arms
                    .into_iter()
                    .map(|a| MatchArm {
                        pattern: a.pattern,
                        guard: a.guard.map(|g| self.rexpr(g)),
                        body: self.rexpr(a.body),
                        span: a.span,
                    })
                    .collect(),
                span,
            },
            Expr::Range {
                start,
                end,
                inclusive,
                span,
            } => Expr::Range {
                start: Box::new(self.rexpr(*start)),
                end: Box::new(self.rexpr(*end)),
                inclusive,
                span,
            },
            Expr::If {
                cond,
                then_expr,
                else_expr,
                span,
            } => Expr::If {
                cond: Box::new(self.rexpr(*cond)),
                then_expr: Box::new(self.rexpr(*then_expr)),
                else_expr: Box::new(self.rexpr(*else_expr)),
                span,
            },
            Expr::Lambda {
                params,
                ret,
                body,
                span,
                throws,
            } => {
                let prev_ret = std::mem::replace(&mut self.current_ret, ret.clone());
                let new_body = match body {
                    LambdaBody::Expr(e) => {
                        let expected = self.current_ret.clone();
                        LambdaBody::Expr(Box::new(self.rexpr_expected(*e, expected.as_ref())))
                    }
                    LambdaBody::Block(stmts) => LambdaBody::Block(self.rblock(stmts)),
                };
                self.current_ret = prev_ret;
                Expr::Lambda {
                    params,
                    ret,
                    body: new_body,
                    span,
                    throws,
                }
            }
            Expr::CloneWith {
                object,
                fields,
                span,
            } => Expr::CloneWith {
                object: Box::new(self.rexpr(*object)),
                fields: fields
                    .into_iter()
                    .map(|(n, e)| (n, self.rexpr(e)))
                    .collect(),
                span,
            },
            Expr::New(inner, span) => Expr::New(Box::new(self.rexpr(*inner)), span),
            Expr::Spawn { call, span } => Expr::Spawn {
                call: Box::new(self.rexpr(*call)),
                span,
            },
            // Leaves (Int/Float/Decimal/Bool/Null/Bytes/Ident/This) and `Inject` (already expanded by
            // `desugar_di` upstream) carry no `queryInto` to rewrite.
            leaf => leaf,
        }
    }

    fn rparts(&mut self, parts: Vec<StrPart>) -> Vec<StrPart> {
        parts
            .into_iter()
            .map(|p| match p {
                StrPart::Expr(e) => StrPart::Expr(Box::new(self.rexpr(*e))),
                lit => lit,
            })
            .collect()
    }

    fn rstmt(&mut self, s: Stmt) -> Stmt {
        match s {
            Stmt::VarDecl {
                ty,
                name,
                init,
                mutable,
                span,
            } => {
                let init = self.rexpr_expected(init, Some(&ty));
                Stmt::VarDecl {
                    ty,
                    name,
                    init,
                    mutable,
                    span,
                }
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => Stmt::Assign {
                target: self.rexpr(target),
                value: self.rexpr(value),
                span,
            },
            Stmt::Return { value, span } => Stmt::Return {
                value: value.map(|e| {
                    let expected = self.current_ret.clone();
                    self.rexpr_expected(e, expected.as_ref())
                }),
                span,
            },
            Stmt::If {
                cond,
                bind,
                then_block,
                else_block,
                span,
            } => Stmt::If {
                cond: self.rexpr(cond),
                bind,
                then_block: self.rblock(then_block),
                else_block: else_block.map(|b| self.rblock(b)),
                span,
            },
            Stmt::For {
                ty,
                name,
                val,
                iter,
                body,
                span,
            } => Stmt::For {
                ty,
                name,
                val,
                iter: self.rexpr(iter),
                body: self.rblock(body),
                span,
            },
            Stmt::While {
                cond,
                body,
                post_cond,
                span,
            } => Stmt::While {
                cond: self.rexpr(cond),
                body: self.rblock(body),
                post_cond,
                span,
            },
            Stmt::CFor {
                init,
                cond,
                step,
                body,
                span,
            } => Stmt::CFor {
                init: init.map(|s| Box::new(self.rstmt(*s))),
                cond: cond.map(|e| self.rexpr(e)),
                step: step.map(|s| Box::new(self.rstmt(*s))),
                body: self.rblock(body),
                span,
            },
            Stmt::Block(stmts, span) => Stmt::Block(self.rblock(stmts), span),
            Stmt::Destructure {
                pat,
                init,
                else_block,
                span,
            } => Stmt::Destructure {
                pat,
                init: self.rexpr(init),
                else_block: else_block.map(|b| self.rblock(b)),
                span,
            },
            Stmt::Expr(e, span) => Stmt::Expr(self.rexpr(e), span),
            Stmt::Discard(e, span) => Stmt::Discard(self.rexpr(e), span),
            Stmt::Throw { value, span } => Stmt::Throw {
                value: self.rexpr(value),
                span,
            },
            Stmt::Try {
                body,
                catches,
                finally_block,
                span,
            } => Stmt::Try {
                body: self.rblock(body),
                catches: catches
                    .into_iter()
                    .map(|c| CatchClause {
                        ty: c.ty,
                        name: c.name,
                        body: self.rblock(c.body),
                        span: c.span,
                    })
                    .collect(),
                finally_block: finally_block.map(|b| self.rblock(b)),
                span,
            },
            leaf => leaf, // Break / Continue carry no expression
        }
    }
}

#[cfg(test)]
mod tests {
    use super::snake_case;

    #[test]
    fn snake_case_camel_boundaries() {
        // The `SnakeToCamel` core case (DEC-208 slice B2): a camelCase field → its snake_case column.
        assert_eq!(snake_case("userName"), "user_name");
        assert_eq!(snake_case("firstName"), "first_name");
        assert_eq!(snake_case("streetName"), "street_name");
        assert_eq!(snake_case("postalCode"), "postal_code");
        assert_eq!(snake_case("homeAddress"), "home_address");
    }

    #[test]
    fn snake_case_acronyms() {
        // An interior/trailing ACRONYM run stays together; the run ends where a lowercase word begins.
        assert_eq!(snake_case("userId"), "user_id");
        assert_eq!(snake_case("userID"), "user_id");
        assert_eq!(snake_case("httpServer"), "http_server");
        assert_eq!(snake_case("parseHTTPResponse"), "parse_http_response");
    }

    #[test]
    fn snake_case_digits_and_noop() {
        // A digit is a word boundary before an uppercase; an all-lowercase name is unchanged.
        assert_eq!(snake_case("field2Name"), "field2_name");
        assert_eq!(snake_case("id"), "id");
        assert_eq!(snake_case("plain"), "plain");
    }
}
