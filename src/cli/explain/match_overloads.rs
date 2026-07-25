//! `phg explain` sub-catalog: unions, match & pattern cluster, fixed-lists, intersections, overloads
//! (M-Decomp, Invariant 13 — dispatched from `explain/mod.rs`; same
//! `text(code) -> Option<&'static str>` contract as `explain_config`).

/// Explanation text for a code in this band, or `None` when `code` is not this catalog's.
pub(super) fn text(code: &str) -> Option<&'static str> {
    Some(match code {
        "E-UNION-MEMBER" => {
            "E-UNION-MEMBER — a union member is not an allowed type.\n\n\
             A union `A | B` (M-RT S4) may combine classes, interfaces, and primitives\n\
             (`int | string`). Enum members, optional `T?` members, and function-typed members are not\n\
             supported this slice — an enum is already a closed sum (match its variants directly), and\n\
             optional/function members complicate the PHP `A|B` emission. Replace the member, or model\n\
             the case differently.\n"
        }
        "E-VOID-IN-UNION" => {
            "E-VOID-IN-UNION — `void` cannot be a union member.\n\n\
             `void` is the *uncapturable* nothing: a value of type `void` can never be held, so a union\n\
             containing it (`int | void`) would be uninhabited. Use `empty` — the *holdable* nothing — if\n\
             you need a nothing-or-something union (`int | empty` is allowed). `void` must stand alone as a\n\
             return type. (`void` widens to `empty`, so a `-> void` function still flows into an `empty` slot.)\n"
        }
        "E-UNION-ARITY" => {
            "E-UNION-ARITY — a union needs two or more distinct types.\n\n\
             `A | A` (or any union whose members are all the same after normalization) collapses to a\n\
             single type, so it is not a union. Give the union at least two distinct members, or use the\n\
             single type directly.\n"
        }
        "E-MATCH-TYPE" => {
            "E-MATCH-TYPE — a `match` type pattern is invalid.\n\n\
             A type pattern (`Circle c => …`, M-RT S4) matches when the scrutinee is an instance of the\n\
             named **class or interface** — the same runtime test as `instanceof`. The name must be a\n\
             declared class or interface (not an enum — match an enum's variants directly), OR one of the\n\
             discriminable primitives `int`/`float`/`string`/`bool`/`null` (Wave A union narrowing). A\n\
             type pattern is allowed only at the **top level** of a match arm, not nested inside a\n\
             variant pattern. Use it to match over a union scrutinee.\n"
        }
        "E-MATCH-TYPE-ERASED" => {
            "E-MATCH-TYPE-ERASED — a type pattern names a type that erases to a PHP `string`.\n\n\
             Union narrowing (Wave A) discriminates a match arm by runtime type, and the transpiled PHP\n\
             leg does it with `is_int`/`is_float`/`is_string`/`is_bool`/`is_null`. `decimal`, `bytes`,\n\
             `html` and `attr` all erase to a PHP `string` at transpile, so `is_string` can't tell them\n\
             apart from a real `string` — a type pattern naming one could not be byte-identical across\n\
             `phg run` (both engines)/PHP. Only `int`/`float`/`string`/`bool`/`null` and classes/interfaces can be\n\
             type-tested; match the value's wrapping form, or use a class/interface, instead.\n"
        }
        "E-MATCH-ERASED-AMBIG" => {
            "E-MATCH-ERASED-AMBIG — a `string` type pattern is ambiguous in this union.\n\n\
             A `string` arm transpiles to PHP `is_string(...)`. If the union scrutinee ALSO holds a type\n\
             that erases to a PHP `string` (`decimal`/`bytes`/`html`/`attr`), then `is_string` would\n\
             match those too — the interpreter and VM distinguish them by runtime representation, but the\n\
             transpiled PHP cannot, breaking byte-identity. Split the union so the `string` arm is\n\
             unambiguous, or add a `default` arm and test the other members another way.\n"
        }
        "E-MATCH-GUARD-EXHAUST" => {
            "E-MATCH-GUARD-EXHAUST — a shape is covered only by guarded arms.\n\n\
             A match arm guard (`pat when <cond> => …`, pattern cluster) is an optional boolean\n\
             condition; a false guard falls through to the next arm. Because the guard might be false,\n\
             a guarded arm does NOT discharge its shape for exhaustiveness. If every arm matching a\n\
             given variant/type is guarded, the match can fall through with no arm — so add an\n\
             **unguarded** arm (or `default`) covering that shape as a fallback.\n"
        }
        "E-BOUND-NOT-SATISFIED" => {
            "E-BOUND-NOT-SATISFIED — a generic type argument does not satisfy its type-parameter bound.\n\n\
             A bounded type parameter `<T: Interface>` (DEC-211) constrains `T` to types that implement\n\
             the bound, so the function body may call the bound's methods on a `T` value. At a call site\n\
             the argument types fix `T` to a concrete type — which must implement the bound, or the\n\
             bound's methods would not exist on it after erasure. Make the type argument implement the\n\
             bound interface, or relax/remove the bound. (Erased before any backend — the bound is a\n\
             compile-time contract, like the parameter itself.)\n"
        }
        "E-MATCH-BARE-VARIANT" => {
            "E-MATCH-BARE-VARIANT — a bare name (or a standalone `_`) is used as a match arm.\n\n\
             PascalCase is the type/variant namespace, so a bare `Circle => …` LOOKS like it matches the\n\
             variant `Circle` but is actually a catch-all binding named `Circle` that matches EVERY value —\n\
             a silent footgun (DEC-209), so it is rejected. Write what you meant: `Circle() => …` to match\n\
             the variant, `Circle x => …` / `Circle _ => …` to type-test (optionally binding), a lowercase\n\
             name (`x => …`) to bind every value, or `default => …` for the catch-all arm. A standalone\n\
             `_ => …` arm is likewise rejected: `_` is an ignore-placeholder only, valid inside a pattern\n\
             (`Some(_)`) or a type-test (`Square _`), never the whole arm — use `default`.\n"
        }
        "E-FIXEDLIST-LEN" => {
            "E-FIXEDLIST-LEN — a fixed-length list literal has the wrong length.\n\n\
             A `[T; N]` fixed-length list (Phase 1 types slice) has a compile-time length `N`. When a\n\
             list literal initializes one, the literal must have exactly `N` elements: `[int; 3] rgb =\n\
             [255, 128, 0];` (ok) but `[int; 2] p = [1, 2, 3];` is this error. Adjust the literal or the\n\
             declared length.\n"
        }
        "E-FIXEDLIST-BOUNDS" => {
            "E-FIXEDLIST-BOUNDS — a literal index is out of bounds for a fixed-length list.\n\n\
             Indexing a `[T; N]` with a *constant* index is bounds-checked at compile time: valid\n\
             indices are `0..N`, so `pair[5]` on a `[int; 2]` is this error. A non-literal index\n\
             (`pair[i]`) is left to the runtime bounds check, exactly like a `List<T>`.\n"
        }
        "E-OR-PATTERN-BIND" => {
            "E-OR-PATTERN-BIND — an or-pattern alternative binds a variable.\n\n\
             An or-pattern groups alternatives that share one arm body: `match n { 1 | 2 | 3 => \"low\",\n\
             _ => \"hi\" }`. Because any alternative can match, the shared body cannot know which one\n\
             did — so no alternative may be a catch-all (`_` or a bare name) or introduce a binding\n\
             (`Some(n)`, `Circle c`, a struct-field binder). Concrete patterns and `_` *sub*-patterns\n\
             are fine (`Some(_) | None()`). If you need to bind, write separate arms instead.\n"
        }
        "E-GUARD-TYPE" => {
            "E-GUARD-TYPE — a match arm guard is not boolean.\n\n\
             The condition after `when` in a match arm (`pat when <cond> => …`) is a boolean test,\n\
             evaluated with the arm's pattern bindings in scope. It must have type `bool` — wrap a\n\
             non-boolean value in a comparison (`when n > 0`) rather than relying on truthiness.\n"
        }
        "E-STRUCT-PAT-TYPE" => {
            "E-STRUCT-PAT-TYPE — a struct pattern's head is not a class.\n\n\
             A struct pattern (`Point { x, y } => …`, pattern cluster S5.2) destructures a class\n\
             instance's named fields — its head must be a declared **class**. An interface has no\n\
             fields (use a type pattern `Iface x` to bind it); an enum is matched by its variants\n\
             (`Some(v)`), not by fields.\n"
        }
        "E-STRUCT-FIELD-UNKNOWN" => {
            "E-STRUCT-FIELD-UNKNOWN — a struct pattern names a field the class does not declare.\n\n\
             Each `field` (or `field: sub-pattern`) in a struct pattern (`Point { x, y }`) must be a\n\
             field declared on the class (including inherited fields). Destructure only declared\n\
             fields — check for a typo or a field on a different class.\n"
        }
        "E-PATTERN-DUP-BIND" => {
            "E-PATTERN-DUP-BIND — a pattern binds the same name twice.\n\n\
             A struct pattern (`Point { x, y: x }`) or any nested pattern must give each destructured\n\
             binding a distinct name — two bindings of `x` would have one silently shadow the other.\n\
             Rename one (`Point { x, y: y2 }`).\n"
        }
        "E-INTERSECT-MEMBER" => {
            "E-INTERSECT-MEMBER — an intersection member is not an allowed type.\n\n\
             An intersection `A & B` (M-RT S5) combines interfaces, plus *at most one* concrete class\n\
             (`Cls & I & J`). Primitives, enums, optional `T?` members, and function-typed members are\n\
             not allowed — a value satisfies an intersection by being a single instance that conforms to\n\
             every member, which only interfaces (and one class) express. Replace the member.\n"
        }
        "E-INTERSECT-MULTI-CLASS" => {
            "E-INTERSECT-MULTI-CLASS — an intersection names two or more concrete classes.\n\n\
             A value has exactly one class, so it can never be an instance of two distinct classes at\n\
             once — `Cat & Dog` is uninhabited. Name at most one class and compose the rest with\n\
             interfaces. (A second class becomes meaningful only once class `extends` lands in S6.)\n"
        }
        "E-INTERSECT-ARITY" => {
            "E-INTERSECT-ARITY — an intersection needs two or more distinct types.\n\n\
             `A & A` (or any intersection whose members are all the same after normalization) collapses\n\
             to a single type, so it is not an intersection. Give it at least two distinct members, or\n\
             use the single type directly.\n"
        }
        "E-INTERSECT-SIG" => {
            "E-INTERSECT-SIG — intersection members share a method with conflicting signatures.\n\n\
             Two members of `A & B` declare the same method with different parameter or return types.\n\
             A class satisfying the intersection would need that one method to conform to both — which\n\
             the current overload-agnostic intersection check cannot verify — so the intersection is\n\
             rejected. Align the shared method's signature across the members (or drop one).\n"
        }
        "E-INTERSECT-NO-MEMBER" => {
            "E-INTERSECT-NO-MEMBER — a member access on an intersection resolves to nothing.\n\n\
             A method/field call on an `A & B` value searches every member (each interface, plus the\n\
             lone class for fields). None of them declares the named method or field. Check the name, or\n\
             add it to one of the intersection's members.\n"
        }
        "E-OVERLOAD-RETURN" => {
            "E-OVERLOAD-RETURN — a name mixes parameter- and return-type overloading.\n\n\
             A name may be overloaded one of two ways, never both:\n\
             • PARAMETER overloading — distinct parameter signatures sharing ONE return type; the\n\
               runtime argument types pick the overload (dynamic multiple dispatch).\n\
             • RETURN-TYPE overloading (Slice C) — IDENTICAL parameter signatures with DIFFERENT\n\
               return types; the call's type context (a `<Type>f(…)` selector) picks the overload at\n\
               compile time, and each is emitted as a distinct PHP function.\n\n\
             Mixing the two (some overloads differing in parameters, others only in return) has no\n\
             sound dispatch — the runtime parameter dispatch cannot tell two identical-parameter\n\
             overloads apart. Likewise, parameter overloads that differ in return type also raise this\n\
             (keep their return type shared). Split the name into separate functions, or make all\n\
             overloads share one parameter signature (return-type overloading) or one return type\n\
             (parameter overloading).\n"
        }
        "E-OVERLOAD-NO-CONTEXT" => {
            "E-OVERLOAD-NO-CONTEXT — a return-type-overloaded call has no type context.\n\n\
             A function overloaded only by return type (identical parameters) is chosen by the type\n\
             expected at the call site. In this position there is none, so the compiler cannot pick a\n\
             member. Add a return-type selector naming the overload you want — `<Type>f(args)` — e.g.\n\
             `discard <int>parse(\"7\");` or `int x = <int>parse(\"7\");`. (A later slice will infer the\n\
             selector from a typed binding, return, or argument; for now it is explicit.)\n"
        }
        "E-OVERLOAD-AMBIGUOUS-RETURN" => {
            "E-OVERLOAD-AMBIGUOUS-RETURN — a selector type matches more than one overload.\n\n\
             The `<Type>` selector resolves an overload by: (1) the overload whose return type EQUALS\n\
             the selector, else (2) the UNIQUE overload whose return type is assignable to it. When two\n\
             or more overloads are assignable (e.g. `<Animal>` with both a `Dog`- and a `Cat`-returning\n\
             overload) the choice is ambiguous. Name the exact return type of the overload you mean\n\
             (`<Dog>` / `<Cat>`).\n"
        }
        "E-OVERLOAD-SELECT-UNKNOWN" => {
            "E-OVERLOAD-SELECT-UNKNOWN — a `<Type>` selector names no overload's return type.\n\n\
             `<Type>f(args)` selects the overload of `f` whose return type is `Type`. This error means\n\
             `f` has no overload returning that type — or `f` is not a return-type-overloaded free\n\
             function at all (the selector applies only to those; not to methods, parameter-overloaded\n\
             names, or ordinary functions). Use a return type one of the overloads actually declares.\n"
        }
        "E-OVERLOAD-DUPLICATE" => {
            "E-OVERLOAD-DUPLICATE — two overloads have identical parameter types.\n\n\
             Each overload of a name must be distinguishable by its parameter signature (arity or\n\
             parameter types). Two declarations with the same parameters are redundant and could never\n\
             be told apart at a call. Remove one, or change its parameters.\n"
        }
        "E-OVERLOAD-ERASE" => {
            "E-OVERLOAD-ERASE — two overloads are indistinguishable in transpiled PHP.\n\n\
             Phorj transpiles to PHP, whose runtime cannot tell some distinct Phorj types apart:\n\
             `string` and `bytes` both become PHP `string`, and `List`/`Map`/`Set` all become PHP\n\
             `array`. So two overloads that differ ONLY in such a position (e.g. `f(string)` vs\n\
             `f(bytes)`, or `g(List<int>)` vs `g(Set<int>)`) compile to a dispatch the PHP backend\n\
             can't resolve — an ambiguous call would fault on the Phorj backends but silently take\n\
             the first matching PHP branch. Differentiate the overloads by another parameter, or merge\n\
             them into one.\n"
        }
        "E-OVERLOAD-GENERIC" => {
            "E-OVERLOAD-GENERIC — a generic function/method cannot be overloaded.\n\n\
             A generic declaration (`f<T>(…)`) must be the only one with its name. Generic overloading\n\
             (mixing `<T>` overloads with concrete ones) is not supported. Remove the type parameters\n\
             and write concrete overloads, or rename one declaration.\n"
        }
        "E-OVERLOAD-NO-MATCH" => {
            "E-OVERLOAD-NO-MATCH — no overload accepts the call's argument types.\n\n\
             The call's static argument types match no overload's parameter types (by arity or\n\
             assignability). Check the argument types against the available overloads; an argument\n\
             whose static type is a supertype of every overload's parameter cannot be dispatched.\n"
        }
        "E-OVERLOAD-FN-VALUE" => {
            "E-OVERLOAD-FN-VALUE — an overloaded function has no single first-class value.\n\n\
             A bare reference to an overloaded function (`var g = f;`) is ambiguous — there is no one\n\
             signature to give the value. Call the function directly, or wrap the intended overload in\n\
             a lambda (`var g = fn(int x) => f(x);`).\n"
        }
        _ => return None,
    })
}
