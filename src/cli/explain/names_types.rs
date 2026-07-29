//! `phg explain` sub-catalog: idents, types, aliases, packages, file-module layout, assignment, optionals, entry-kind, pipes, property hooks
//! (M-Decomp, Invariant 13 — dispatched from `explain/mod.rs`; same
//! `text(code) -> Option<&'static str>` contract as `explain_config`).

/// Explanation text for a code in this band, or `None` when `code` is not this catalog's.
pub(super) fn text(code: &str) -> Option<&'static str> {
    Some(match code {
        "E-UNKNOWN-IDENT" => {
            "E-UNKNOWN-IDENT — a name was used that is not in scope.\n\n\
             Phorj resolves identifiers lexically: block-scope locals (including `var` bindings\n\
             and `for` loop variables), parameters, top-level functions, and — inside a method —\n\
             the current class's fields. A typo or an out-of-scope reference triggers this; the\n\
             diagnostic suggests the nearest in-scope name when one is close.\n"
        }
        "E-UNKNOWN-TYPE" => {
            "E-UNKNOWN-TYPE — a type name was used that is not defined.\n\n\
             Built-in types are `int`, `float`, `bool`, `string`, `List<T>`, `Map<K,V>`, `Set<T>`.\n\
             User types come from `class`, `enum`, and `type` alias declarations. Check the\n\
             spelling and that the declaration is present.\n"
        }
        "E-INFER-NULL" => {
            "E-INFER-NULL — `var` cannot infer a type from `null` alone.\n\n\
             `null` has no element type on its own, so `var x = null;` is rejected. Annotate the\n\
             optional instead, e.g. `int? x = null;`.\n"
        }
        "E-ALIAS-CYCLE" => {
            "E-ALIAS-CYCLE — a `type` alias refers to itself.\n\n\
             `type A = B; type B = A;` has no underlying type. Break the cycle so every alias\n\
             bottoms out at a built-in, class, or enum type.\n"
        }
        "E-RANGE-TYPE" => {
            "E-RANGE-TYPE — a range bound is not an `int`.\n\n\
             Both bounds of `a..b` / `a..=b` must be `int`; the range materializes to a\n\
             `List<int>` (its role this slice is `for (int i in 0..n)`). Use integer bounds, or\n\
             build a `List` explicitly if you need other element types.\n"
        }
        "E-MAP-KEY" => {
            "E-MAP-KEY — a map's key type is not hashable.\n\n\
             A `Map<K, V>` key must be `int`, `bool`, or `string` (the hashable subset) — a\n\
             `float`, list, instance, or other composite can't be a key. Change the key type, or\n\
             model the lookup differently (e.g. key by a `string` id).\n"
        }
        "E-EMPTY-LITERAL" => {
            "E-EMPTY-LITERAL — a bare empty `[]` literal has no element type (DEC-214).\n\n\
             An empty collection is CONSTRUCTED with mandatory `new`, self-typed from its type\n\
             arguments — `new List<T>()` or `new Map<K,V>()` — never inferred from the surrounding\n\
             declaration, return, or argument type (\"nothing in the wind\": no type-from-later-use).\n\
             A non-empty literal `[1, 2, 3]` / `[\"a\" => 1]` is unchanged — its element type is\n\
             locally obvious. Write `List<int> xs = new List<int>();` (not `List<int> xs = [];`).\n"
        }
        "E-NO-PACKAGE" => {
            "E-NO-PACKAGE — a file has no `package` declaration.\n\n\
             Everything is namespaced (\"nothing in the wind\"): every file must declare its package\n\
             as its first line, never inferred. A runnable program declares `package Main;` (the\n\
             reserved entry); library code declares a dotted path like `package app.util;`.\n"
        }
        "E-RESERVED-PACKAGE" => {
            "E-RESERVED-PACKAGE — a user file claimed a `core` package root.\n\n\
             The `core.` root is reserved for the standard library (`Core.Console`, `Core.Math`,\n\
             `Core.File`, …), like a built-in type name. Root your own packages elsewhere, e.g.\n\
             `package app;` or `package app.util;`.\n"
        }
        "E-RESERVED-NAME" => {
            "E-RESERVED-NAME — a function / class / enum / interface / trait / type was named with a\n\
             word PHP reserves for that symbol position (e.g. `var`, `list`, `print`, `array`, `int`),\n\
             or a class-position symbol collides with a PHP BUILTIN class (Core/SPL/date/json —\n\
             e.g. `Exception`, `DateTime`, `ArrayObject`): the transpiled declaration would be a\n\
             parse error or a fatal redeclare, so Phorj rejects the name up front.\n\n\
             These words are perfectly good Phorj *value* identifiers — a variable, parameter, field,\n\
             property, or method may be named `var` / `list` / `int` (they map to a legal PHP `$list`\n\
             / `->list()`). But PHP rejects them as a *symbol* name: `function list()` or `class int {}`\n\
             is a PHP parse error, so Phorj rejects them there rather than emitting invalid PHP. The\n\
             check is kind-aware — the type words (`int`/`float`/`object`/…) are legal PHP *function*\n\
             names but illegal as *class* names. Rename the function/class/type (the value/parameter/\n\
             field/method name can keep the word).\n"
        }
        "E-PKG-PATH" => {
            "E-PKG-PATH — a file's `package` does not match its location.\n\n\
             In a project, the directory under the source root IS the package (folder = path, Go's\n\
             model): `src/app/util/*.phg` must declare `package app.util;`. `package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;` is exempt\n\
             (runnable anywhere). Move the file, or fix its package to match the directory.\n"
        }
        "E-FILE-NAME" => {
            "E-FILE-NAME — a public type lives in a file not named after it.\n\n\
             A file's public face is one thing (the public-surface rule): a non-`main` file that exports\n\
             exactly one public type must be named after it, byte-exactly including casing —\n\
             `public class Circle` lives in `Circle.phg` (not `circle.phg`, not `shapes.phg`). Rename the\n\
             file, or mark the type `private`/`internal` if it is not part of the package's public API.\n\
             A file that declares `main` is exempt (programs mix freely).\n"
        }
        "E-FILE-MULTI-PUBLIC" => {
            "E-FILE-MULTI-PUBLIC — a file declares more than one public type.\n\n\
             A non-`main` file exports at most one public type (class/enum/interface/trait), so its name\n\
             can identify it. Split the extra public types into their own `<TypeName>.phg` files, or mark\n\
             the helpers `private`/`internal` (those ride along free — they are single-file-scoped). This\n\
             keeps Phorj's function-heavy model: free functions and non-public helpers are unconstrained.\n"
        }
        "E-FILE-MIXED-PUBLIC" => {
            "E-FILE-MIXED-PUBLIC — a file mixes a public type with public free function(s).\n\n\
             A non-`main` file is either a *type module* (one public type, named after the file) or a\n\
             *function module* (public free functions, topic-named) — not both. Move the function(s) to\n\
             their own function module, turn them into methods/static methods of the type, or mark them\n\
             `private`/`internal`. `main` files are exempt.\n"
        }
        "E-SHADOW-IMPORT" => {
            "E-SHADOW-IMPORT — a local binding shadows an imported module qualifier.\n\n\
             Everything is namespaced (\"nothing in the wind\"): after `import Core.Output;` the\n\
             name `console` is a module qualifier, so a value binding (variable, parameter, loop or\n\
             match binding) of the same name would make `Console.x()` ambiguous — the run backends\n\
             would read a method call, the transpiler a native. Rename the binding, or drop the\n\
             matching import.\n"
        }
        "E-SHADOW-LOCAL" => {
            "E-SHADOW-LOCAL — a declaration reuses the name of a live local or parameter.\n\n\
             Phorj has block scope; PHP does not. A nested redeclaration therefore means two different\n\
             things on the two legs — the Rust backends make a NEW binding, while the transpiled PHP\n\
             writes through to the OUTER variable. That is a silent wrong answer, and in a nested `for`\n\
             reusing a counter name it silently changes the ITERATION COUNT.\n\n\
             So a declaration is rejected when its name is already bound by a live local or parameter\n\
             IN THE SAME FUNCTION — same scope or an enclosing one. Fix it by renaming the inner one,\n\
             or by ASSIGNING to the existing binding (`a = 2;`) instead of declaring a second one\n\
             (`int a = 2;`) if writing through was what you meant.\n\n\
             These stay legal, and are not shadowing:\n    \
             * sibling blocks reusing a name — the first binding is already dead\n    \
             * sequential `for` loops reusing the counter — the ubiquitous idiom\n    \
             * sibling `match` arms, or sibling binding-`if`s, reusing a binding name\n    \
             * a LAMBDA parameter shadowing an outer local — a lambda starts a new function, and PHP\n      \
               arrow-fn params shadow correctly, so both legs already agree\n    \
               * a method local sharing a FIELD's name — `this.field` is mandatory, so a field is not a\n      \
               local binding and nothing is shadowed\n\n\
             Full 23-row case list: `docs/specs/2026-07-26-block-scope-shadowing.md`.\n"
        }
        "E-SHADOW-FN" => {
            "E-SHADOW-FN — a local binding shadows a top-level function name.\n\n\
             Functions are first-class values, so a bare `f` resolves to the function and a bare\n\
             `f(…)` calls it. A local binding (variable, parameter, loop or match binding) of the\n\
             same name would be ambiguous — the run backends dispatch functions-first while the\n\
             transpiler emits the local, a silent divergence. Rename the binding so a local never\n\
             shares a name with a function.\n"
        }
        "E-OPT-ASSIGN" => {
            "E-OPT-ASSIGN — an optional `T?` was used where a non-optional `T` is required.\n\n\
             A non-optional value can never be `null`, so a `T?` cannot flow into a `T` binding,\n\
             parameter, field, or return without handling absence first. Unwrap it with `??`\n\
             (default), `?.` (safe access), `if (var x = opt) { … }`, or `opt!` (checked).\n"
        }
        "E-ASSIGN-IMMUTABLE" => {
            "E-ASSIGN-IMMUTABLE — a reassignment targeted an immutable binding.\n\n\
             Bindings are immutable by default. Only a binding declared `mutable` may be reassigned\n\
             with `x = …;`. Declare it `mutable int x = …;` (or `mutable var x = …;`) — or, if it\n\
             never changes, keep it immutable and introduce a new binding instead.\n"
        }
        "E-ASSIGN-TYPE" => {
            "E-ASSIGN-TYPE — a reassigned value's type does not match the binding's type.\n\n\
             Reassignment keeps the binding's declared type; the new value must be assignable to it\n\
             (the same rule as the original declaration). Convert the value, or change the binding's\n\
             declared type.\n"
        }
        "E-ASSIGN-UNKNOWN" => {
            "E-ASSIGN-UNKNOWN — a reassignment targeted a name that is not an in-scope local.\n\n\
             `x = …;` reassigns an existing local variable; the name must already be declared in\n\
             scope. Declare it first (`mutable int x = …;`), or check for a typo.\n"
        }
        "E-UNUSED-VALUE" => {
            "E-UNUSED-VALUE — a non-`void`/`empty` result was used as a bare statement and dropped.\n\n\
             Every value a function or expression produces must be used: bind it (`int x = f();`),\n\
             return it, or pass it on. If you genuinely want the side effect and not the value,\n\
             discard it explicitly with `discard f();`. Only `void` and `empty` results (and a\n\
             diverging `never` call like `panic(…)`) may be dropped silently.\n"
        }
        "E-ASSIGN-SET-VISIBILITY" => {
            "E-ASSIGN-SET-VISIBILITY — a field with asymmetric visibility was assigned outside its\n\
             set scope.\n\n\
             `public private(set) mutable int x;` (DEC-241) reads everywhere but may be ASSIGNED\n\
             only inside the owning class (`protected(set)`: the owner and its subclasses). Move\n\
             the write into the owning scope — usually behind a method — or widen the `(set)`\n\
             modifier. Transpiles 1:1 to PHP 8.4 asymmetric visibility.\n"
        }
        "E-SET-VIS-IMMUTABLE" => {
            "E-SET-VIS-IMMUTABLE — `private(set)`/`protected(set)` on an immutable member.\n\n\
             Phorj fields are immutable by default; a set-visibility modifier gates assignments,\n\
             and an immutable member has none to gate. Add `mutable`, or drop the `(set)` modifier\n\
             (DEC-241).\n"
        }
        "E-SET-VIS-WIDER" => {
            "E-SET-VIS-WIDER — a member's set visibility is wider than its read visibility.\n\n\
             Writes cannot be more visible than reads (`private protected(set)` would let a\n\
             subclass assign a field it cannot read) — PHP rejects the same shape. Narrow the\n\
             `(set)` modifier or widen the read visibility (DEC-241).\n"
        }
        "E-ENTRY-KIND-REQUIRED" => {
            "E-ENTRY-KIND-REQUIRED — `#[Entry]` without a usable `EntryKind` variant.\n\n\
             The entry role is DECLARED, not inferred (DEC-331 D1): write `#[Entry(kind: EntryKind.Cli)]`\n\
             for a `phg run` entry, or `#[Entry(kind: EntryKind.Web)]` for `phg serve`. This fires when the\n\
             `kind:` is missing entirely (bare `#[Entry]` — the retired DEC-191 signature-inference form),\n\
             when `kind:` is not an `EntryKind` variant at all (e.g. a literal), or when it names the\n\
             `EntryKind` enum with no variant (`kind: EntryKind` — add `.Cli`/`.Web`).\n"
        }
        "E-ENTRY-KIND-UNKNOWN" => {
            "E-ENTRY-KIND-UNKNOWN — `#[Entry(kind: …)]` that does not name a known `EntryKind` variant.\n\n\
             This fires two ways: the qualifier is not `EntryKind` (nor the fully-qualified\n\
             `Core.Runtime.EntryKind`) — e.g. `kind: Foo.Cli` — or the variant name is unrecognized —\n\
             e.g. `kind: EntryKind.Banana`. The active kinds are `Cli` and `Web`;\n\
             `Desktop`/`Mobile`/`Worker`/`Embedded` are reserved (recognized, not yet built).\n\
             Use `#[Entry(kind: EntryKind.Cli)]` or `#[Entry(kind: EntryKind.Web)]`.\n"
        }
        "E-ENTRY-KIND-RESERVED" => {
            "E-ENTRY-KIND-RESERVED — `#[Entry(kind: …)]` naming a reserved-but-unbuilt kind.\n\n\
             `Desktop`/`Mobile`/`Worker`/`Embedded` are recognized for forward-compatibility but not\n\
             yet implemented (DEC-331 D1). The active kinds are `Cli` and `Web`.\n"
        }
        "E-ENTRY-SIG" => {
            "E-ENTRY-SIG — an `#[Entry(kind: …)]` function whose signature does not match its kind.\n\n\
             The role is declared by `kind:` (DEC-331 D1) and the signature must AGREE with it: a\n\
             `Cli` entry is `(): void`, `(): int`, `(List<string>): void` or `(List<string>): int`\n\
             (an `int` return is the process exit status); a `Web` entry is `(Request): Response`.\n\
             Adjust the signature to the declared kind's shape.\n"
        }
        "E-ENTRY-TARGET" => {
            "E-ENTRY-TARGET — `#[Entry]` on an instance method.\n\n\
             An entry runs before any instance exists. Put `#[Entry(kind: …)]` on a top-level function\n\
             or a class `static` method: `class App { #[Entry(kind: EntryKind.Cli)] static function run(): void { … } }`.\n"
        }
        "E-DUPLICATE-ENTRY-KIND" => {
            "E-DUPLICATE-ENTRY-KIND — more than one `#[Entry]` of the same kind.\n\n\
             A program declares at most ONE entry per kind. A `Cli` and a `Web` entry may coexist\n\
             (`phg run` uses the `Cli` one, `phg serve` the `Web` one), but two of the same kind are\n\
             ambiguous (DEC-331 §3.1). Remove the extra, or give it a different kind.\n"
        }
        "E-ERROR-NAME" => {
            "E-ERROR-NAME — a throwable type whose name does not say it is one.\n\n\
             Any class that implements `Error` (directly, via a parent class, or via interface\n\
             extends) must be named `*Error` or `*Exception` (DEC-275): a `catch (InvalidUrl e)`\n\
             reads like a value type at every site — import, catch, throws clause — while\n\
             `catch (InvalidUrlError e)` is unambiguous everywhere. Rename the type; both\n\
             suffixes are accepted (`Error` matches the stdlib, `Exception` the PHP habit).\n"
        }
        "E-NULL-TYPE" => {
            "E-NULL-TYPE — `null` was written by itself in type position.\n\n\
             `null` is only a nullable-union member (DEC-253): `A | B | null` — which is the same\n\
             type as the canonical `(A | B)?` (the formatter canonicalizes). For a single type,\n\
             write the optional `T?`. There is no standalone `null` type: a binding that could\n\
             only ever hold null holds no information.\n"
        }
        "E-PIPE-PLACEHOLDER" => {
            "E-PIPE-PLACEHOLDER — a pipe placeholder `%` appeared somewhere other than a whole\n\
             argument of the pipe's top-level call.\n\n\
             `%` stands for the piped value in whole-argument slots of the pipe's own call —\n\
             `x |> f(%, 2)` means `f(x, 2)`, and several slots are fine (`x |> f(%, %)` evaluates\n\
             `x` once). It cannot sit inside a larger expression (`f(% + 1)`) or a nested call\n\
             (`f(g(%))`) — nesting is the lambda's job: `x |> (v => f(g(v), v + 1))` (DEC-239).\n"
        }
        "E-PIPE-LAMBDA-CONTEXT" => {
            "E-PIPE-LAMBDA-CONTEXT — a pipe lambda `(v => …)` was used as a plain value.\n\n\
             A contextually-typed pipe lambda omits its parameter type, which flows from the piped\n\
             value — so it only means something as the direct right-hand side of `|>` (DEC-239).\n\
             Because `|>` binds looser than arithmetic, `x |> (v => …) + 1` applies the `+ 1` to\n\
             the lambda itself, stranding it without a pipe. Parenthesize the pipe —\n\
             `(x |> (v => …)) + 1` — or write a fully-typed lambda `function(T v) => …` if you\n\
             really mean to operate on the function value.\n"
        }
        "E-ASSIGN-TARGET" => {
            "E-ASSIGN-TARGET — an assignment target is not a simple variable.\n\n\
             Only `name = expr;` (reassigning a local) is supported in this slice. Field assignment\n\
             (`obj.field = …`) and element assignment (`xs[i] = …`) land in a later mutation slice.\n"
        }
        "E-HOOK-NO-GET" => {
            "E-HOOK-NO-GET — a property hook with no `get` was read.\n\n\
             A property hook may be read-only, write-only, or both. Reading one that declares only a\n\
             `set` is not allowed. Add a `get => …;` clause, or do not read this property.\n"
        }
        "E-HOOK-NO-SET" => {
            "E-HOOK-NO-SET — a property hook with no `set` was assigned.\n\n\
             A read-only computed property (only a `get`) cannot be assigned. Add a `set(T v) { … }`\n\
             clause to make it writable, or do not assign this property.\n"
        }
        "E-HOOK-TYPE" => {
            "E-HOOK-TYPE — a property hook's `get` result or `set` parameter does not match its type.\n\n\
             A hook `T name { … }` reads as `T`, so its `get` expression must yield `T`; its `set`\n\
             parameter must be declared `set(T v)`. Align the get expression / set parameter with the\n\
             hook's declared type.\n"
        }
        "E-HOOK-DUP" => {
            "E-HOOK-DUP — a property hook collides with another member.\n\n\
             A hook is virtual (it has no storage), so its name must be distinct from every field,\n\
             static, method, and other hook in the class. Rename the hook or the colliding member.\n"
        }
        _ => return None,
    })
}
