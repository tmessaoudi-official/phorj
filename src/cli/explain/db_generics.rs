//! `phg explain` sub-catalog: database hydration, static fields, with-expressions, generics, duplicate declarations, override-sig, ufcs
//! (M-Decomp, Invariant 13 — dispatched from `explain/mod.rs`; same
//! `text(code) -> Option<&'static str>` contract as `explain_config`).

/// Explanation text for a code in this band, or `None` when `code` is not this catalog's.
pub(super) fn text(code: &str) -> Option<&'static str> {
    Some(match code {
        "E-DB-INTO-NO-TYPE" => {
            "E-DB-INTO-NO-TYPE — `queryInto()` / `queryOneInto()` had no type to infer its row class from.\n\n\
             The typed-generic hydration (DEC-208 S2) draws its row class `T` from the binding's declared\n\
             type — there is no turbofish. Bind the result to a typed declaration: `List<User> rows =\n\
             stmt.queryInto();` (one `User` per row) or `User? one = stmt.queryOneInto();` (0 → null,\n\
             1 → the object, >1 → `DatabaseError`). A `var` binding or a call argument gives it no target type.\n"
        }
        "E-DB-INTO-BAD-SINK" => {
            "E-DB-INTO-BAD-SINK — the binding type is not a valid hydration sink.\n\n\
             `queryInto()` maps rows into `List<T>` and `queryOneInto()` into `T?`, where `T` is a user\n\
             class with a promoted-field constructor. Declare the binding accordingly — `List<User> rows =\n\
             stmt.queryInto();` or `User? one = stmt.queryOneInto();` — naming a real class as the row type.\n"
        }
        "E-DB-HYDRATE-NO-CTOR" => {
            "E-DB-HYDRATE-NO-CTOR — the row class has no constructor to map columns into.\n\n\
             `queryInto()`/`queryOneInto()` hydrate a row by calling the class's constructor, one argument\n\
             per column, matched by field name. Give the row class a promoted-field constructor:\n\
             `class User { constructor(public string name, public int age) {} }`.\n"
        }
        "E-DB-HYDRATE-UNPROMOTED" => {
            "E-DB-HYDRATE-UNPROMOTED — a constructor parameter of the row class is not a promoted field.\n\n\
             Row→object mapping is by field name, so every constructor parameter must be a promoted field\n\
             (carry `public`/`private`/`protected`) — then its name is the column name. Rewrite plain\n\
             parameters as promoted fields: `constructor(public string name, public int age) {}`.\n"
        }
        "E-DB-HYDRATE-FIELD-TYPE" => {
            "E-DB-HYDRATE-FIELD-TYPE — a hydrated field has a type that cannot be mapped.\n\n\
             A hydrated field must be one of: a scalar column type — `int`, `string`, `float`, `bool`, or\n\
             `decimal` (exact money), or their optional forms (`int?`, …) which admit a SQL NULL; a phorj\n\
             `enum` (mapped from a TEXT column by variant name, zero-payload variants only); `Core.Json`\n\
             (parsed from a TEXT column, needs `import Core.Json`); OR a class with a promoted-field\n\
             constructor (a NESTED entity, hydrated eagerly from dotted `\"field.sub\"` aliased columns; an\n\
             optional entity field `T? x` is `null` when all its columns are NULL). A field of any other\n\
             type (list, map, …) cannot be hydrated from a result.\n"
        }
        "E-DB-HYDRATE-CYCLE" => {
            "E-DB-HYDRATE-CYCLE — a row class's nested-entity fields form a cycle.\n\n\
             Nested hydration is EAGER and whole-graph (one JOIN, dotted `\"order.total\"` aliases), so a\n\
             self-referential relation (`class Employee { …, public Employee? manager; }`) would recurse\n\
             without bound and cannot be resolved at compile time. Break the cycle: drop the back-reference\n\
             from the row class, or load the related rows with a second query. (This is a deliberate limit\n\
             of the primitive — recursive/graph loading is ORM territory, DEC-208.)\n"
        }
        "E-DB-HYDRATE-ENUM-PAYLOAD" => {
            "E-DB-HYDRATE-ENUM-PAYLOAD — an enum field's enum is not mappable from a single column.\n\n\
             An `enum`-typed hydration field maps from one TEXT column by matching the column value against\n\
             a variant NAME, so it supports ZERO-payload variants only (`enum Status { Active(),\n\
             Inactive() }`). An enum with a data-carrying variant (`Circle(float radius)`) cannot be built\n\
             from a single column, and an enum with no variants has nothing to map onto — both are this\n\
             error. Give the row class an enum whose variants are all nullary, or read the column as a\n\
             scalar and construct the richer value yourself.\n"
        }
        "E-DB-SCALAR-BAD-TYPE" => {
            "E-DB-SCALAR-BAD-TYPE — `queryScalar()`'s binding is not a scalar.\n\n\
             `queryScalar()` reads ONE typed value from a single-row, single-column result (`SELECT\n\
             COUNT(*)`, `SELECT MAX(price)`, …). Its type comes from the binding, which must be a scalar —\n\
             `int`, `string`, `float`, `bool`, or a `?` form: `int total = stmt.queryScalar();`. More than\n\
             one row, or more than one column, throws a catchable `DatabaseError` at runtime.\n"
        }
        "E-DB-MAP-BAD-SINK" => {
            "E-DB-MAP-BAD-SINK — `queryMap()`'s binding is not a `Map<K, V>`.\n\n\
             `queryMap()` indexes rows into a `Map<K, V>` keyed by the FIRST selected column (K). Bind it\n\
             to a `Map<K, V>` declaration so both types are inferred — `Map<int, User> byId =\n\
             stmt.queryMap();` (K = the id column, V = a hydrated `User`) or `Map<string, int> counts =\n\
             stmt.queryMap();` (V = the second column).\n"
        }
        "E-DB-MAP-KEY-TYPE" => {
            "E-DB-MAP-KEY-TYPE — `queryMap()`'s key type is not a valid map key.\n\n\
             A `Map` key is `int` or `string` only (matching the language's map-key rule). The key is read\n\
             from the FIRST selected column, so declare the binding `Map<int, V>` or `Map<string, V>`.\n"
        }
        "E-DB-MAP-VALUE-TYPE" => {
            "E-DB-MAP-VALUE-TYPE — `queryMap()`'s value type cannot be produced from a row.\n\n\
             The `V` in `Map<K, V>` is either a scalar (the SECOND selected column — `int`/`string`/\n\
             `float`/`bool` or a `?` form) or a class with a promoted-field constructor (hydrated by field\n\
             name from the remaining columns, nested rules identical to `queryInto`). A list/map/enum V is\n\
             not supported.\n"
        }
        "E-DB-NAMING-NOT-CONST" => {
            "E-DB-NAMING-NOT-CONST — RETIRED (DEC-258; no longer emitted).\n\n\
             A runtime `Naming` value is now legal everywhere: the strategy is a real field riding\n\
             the `Database`/`Statement` values, so a statically-untraceable strategy dispatches on\n\
             that field at run time instead of being rejected. A literal at the call site (or on the\n\
             connection constructor) still bakes the column names at compile time — zero-cost when\n\
             traceable, one branch per hydration call when not.\n"
        }
        "E-STATIC-NO-INIT" => {
            "E-STATIC-NO-INIT — a `static` field has no initializer.\n\n\
             A `static` field is class-level state with no constructor to set it, so it must be\n\
             initialized where it is declared: `static mutable int total = 0;`. Add an initializer.\n"
        }
        "E-STATIC-INIT-TYPE" => {
            "E-STATIC-INIT-TYPE — a `static` field's initializer type does not match its declared type.\n\n\
             A `static T name = expr;` requires `expr` to be assignable to `T`. Static initializers may\n\
             be any expression (evaluated once at program start, in declaration order), but the value's\n\
             type is still checked. Convert the value, or change the field's declared type.\n"
        }
        "E-STATIC-UNKNOWN" => {
            "E-STATIC-UNKNOWN — a `ClassName.field` access names no static field on the class.\n\n\
             `ClassName.name` reads a `static` field (or `const`) declared on the class or inherited\n\
             from an ancestor. The class declares no such static — check the name, or declare\n\
             `static … name = …;` on the class.\n"
        }
        "E-WITH-NONCLASS" => {
            "E-WITH-NONCLASS — the receiver of a `with` expression is not a class instance.\n\n\
             `value with { field: … }` produces a copy of a class instance with some fields replaced,\n\
             so `value` must be a class instance. A primitive, list, map, or optional has no fields to\n\
             copy — use a plain reassignment, or build the value directly.\n"
        }
        "E-WITH-FIELD" => {
            "E-WITH-FIELD — a `with` expression sets a field the class does not declare.\n\n\
             Each `field: value` in `inst with { … }` must name a field declared on the instance's\n\
             class (including inherited fields). Check for a typo, or set only declared fields.\n"
        }
        "E-WITH-TYPE" => {
            "E-WITH-TYPE — a `with` expression sets a field to a value of the wrong type.\n\n\
             In `inst with { field: value }`, `value` must be assignable to `field`'s declared type —\n\
             the same rule as constructing or assigning the field. Convert the value, or set a\n\
             different field.\n"
        }
        "E-GENERIC-PARAM" => {
            "E-GENERIC-PARAM — a generic type parameter is invalid.\n\n\
             A type parameter (`<T>` on a function, method, class, or enum) must be PascalCase, must\n\
             not shadow a built-in type name (`int`, `List`, …), and must be distinct from the other\n\
             parameters of the same declaration. Rename the parameter (e.g. `T`, `K`, `V`, `Elem`).\n"
        }
        "E-TYPE-ARG-COUNT" => {
            "E-TYPE-ARG-COUNT — a type or a turbofish call was given the wrong number of type arguments.\n\n\
             A generic type takes exactly its declared arity: `List<T>`/`Set<T>`/`Optional<T>` and a\n\
             one-parameter user type take one; `Map<K, V>` takes two; `Box<T>`/`Pair<A, B>` take their\n\
             declared count. A non-generic type (and an opaque type *parameter*) takes none — drop the\n\
             `<…>`. The same rule applies to a call-site turbofish (`identity<int>(5)`,\n\
             `obj.method<T, U>(…)`): the explicit type-argument list must match the callee's declared\n\
             type-parameter count — or omit it entirely to infer them from the arguments.\n"
        }
        "E-TURBOFISH-NON-GENERIC" => {
            "E-TURBOFISH-NON-GENERIC — explicit type arguments on a call that takes none.\n\n\
             A call-site turbofish (`f<int>(x)`, `obj.method<T>(…)`) is only valid on a generic function\n\
             or method — one declared with `<…>` type parameters. A non-generic function/method, a\n\
             constructor, an enum-variant construction, a lambda value, a built-in (native) function, and\n\
             a return-type-overloaded call take no explicit type arguments. Remove the `<…>`.\n"
        }
        "E-DUP-TYPE" => {
            "E-DUP-TYPE — a type name is declared more than once.\n\n\
             Class, enum, interface, trait, and `type`-alias names share one namespace within a package,\n\
             and each must be unique — two declarations of `Foo` (even of different kinds) collide.\n\
             Rename one declaration.\n"
        }
        "E-DUP-VARIANT" => {
            "E-DUP-VARIANT — an enum declares the same variant name twice.\n\n\
             Each variant of an `enum` must have a distinct name (`enum E { A, A }` is rejected) — a\n\
             duplicate used to silently overwrite the first, so a `match` could never reach it. Rename\n\
             one variant.\n"
        }
        "E-DUP-STATIC" => {
            "E-DUP-STATIC — a class declares the same `static` field twice.\n\n\
             Each `static` field of a class must have a distinct name. A duplicate used to silently\n\
             overwrite the first. Rename one, or remove the redundant declaration.\n"
        }
        "E-DUP-CONST" => {
            "E-DUP-CONST — a class declares the same `const` twice.\n\n\
             Each class constant (`const NAME = …;`) must have a distinct name. A duplicate used to\n\
             silently overwrite the first. Rename one, or remove the redundant declaration.\n"
        }
        "E-OVERRIDE-SIG" => {
            "E-OVERRIDE-SIG — an overriding method's return type is not compatible with the parent's.\n\n\
             An override must be substitutable for the method it replaces: its return type has to be\n\
             the overridden return type or a subtype of it (covariance). Returning a wider or unrelated\n\
             type (`Sub.k(): string` overriding `Base.k(): int`) would let a call typed by the parent\n\
             receive the wrong runtime value — and transpiled PHP would fatal on the incompatible\n\
             signature. Make the override return the parent's type, or a subtype of it. (Parameter\n\
             variance and overloaded/generic overrides are documented deferrals — see KNOWN_ISSUES.)\n"
        }
        "E-UFCS-AMBIGUOUS" => {
            "E-UFCS-AMBIGUOUS — a UFCS method-style call matches more than one native.\n\n\
             Uniform function call syntax lets `x.name(…)` resolve to a stdlib native whose first\n\
             parameter accepts `x` (e.g. `s.upper()` ⇒ `Text.upper(s)`). When two eligible natives\n\
             share that leaf name, the call is ambiguous. Call the native explicitly by its module\n\
             (`Text.upper(s)`), which is never ambiguous.\n"
        }
        _ => return None,
    })
}
