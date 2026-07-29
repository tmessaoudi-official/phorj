//! `phg explain` sub-catalog: const rules, member visibility, entry/main, static access, wildcard imports, field init
//! (M-Decomp, Invariant 13 — dispatched from `explain/mod.rs`; same
//! `text(code) -> Option<&'static str>` contract as `explain_config`).

/// Explanation text for a code in this band, or `None` when `code` is not this catalog's.
pub(super) fn text(code: &str) -> Option<&'static str> {
    Some(match code {
        "E-CONST-NO-INIT" => {
            "E-CONST-NO-INIT — a `const` class constant has no initializer.\n\n\
             A constant is fixed at declaration, so it must be assigned a value: `const int MAX = 100;`.\n"
        }
        "E-CONST-NOT-LITERAL" => {
            "E-CONST-NOT-LITERAL — a `const` initializer is not a compile-time literal.\n\n\
             A class constant must be a literal (int/float/bool/string/null) this slice — not a call,\n\
             method, or another expression. For a computed class-level value, use a `static` field (or,\n\
             once available, an expression field initializer).\n"
        }
        "E-CONST-MUTABLE" => {
            "E-CONST-MUTABLE — a `const` was also declared `mutable`.\n\n\
             A constant is immutable by definition; `const mutable` is contradictory. Drop `mutable`, or\n\
             use a `static mutable` field for class-level mutable state.\n"
        }
        "E-CONST-INIT-TYPE" => {
            "E-CONST-INIT-TYPE — a `const` initializer's type does not match its declared type.\n\n\
             The literal must be assignable to the constant's type — e.g. `const int MAX = 100;`, not\n\
             `const int MAX = \"x\";`.\n"
        }
        "E-CONST-CASE" => {
            "E-CONST-CASE — a `const` name is not SCREAMING_SNAKE_CASE.\n\n\
             Class constants follow the PHP/C/Java convention: uppercase letters, digits, and `_`\n\
             (`MAX`, `MAX_SIZE`, `HTTP_2`). Rename `maxVal` to `MAX_VAL`.\n"
        }
        "E-CONST-VISIBILITY" => {
            "E-CONST-VISIBILITY — a `private`/`protected` constant was read from outside its class.\n\n\
             A `private const` is readable only inside the declaring class; a `protected const` only\n\
             inside that class and its subclasses. Make it `public` (the default) to read it elsewhere,\n\
             or access it from within the class hierarchy.\n"
        }
        "E-FIELD-VISIBILITY" => {
            "E-FIELD-VISIBILITY — a `private`/`protected` field was read or written from outside its scope.\n\n\
             A `private` field is reachable only inside the declaring class; a `protected` field only\n\
             inside that class and its subclasses (an un-annotated field is `public`). This covers both\n\
             instance fields (`o.f`) and `static` fields (`Class.s`) — reads and writes alike. The check\n\
             runs in the type-checker so every backend agrees — without it a `private` static read would\n\
             pass on the Phorj interpreter/VM but throw in the transpiled PHP (`Cannot access private\n\
             property`). Add a public accessor method (e.g. `function valueOf() -> int { return this.value; }`),\n\
             or declare the field `public`.\n"
        }
        "E-METHOD-VISIBILITY" => {
            "E-METHOD-VISIBILITY — a `private`/`protected` method was called from outside its scope.\n\n\
             A `private` method is callable only inside the declaring class; a `protected` method only\n\
             inside that class and its subclasses (an un-annotated method is `public`). Enforced in the\n\
             type-checker so the interpreter, VM, and transpiled PHP all reject the same call. Call it\n\
             through a public method of the class, or make the method `public`.\n"
        }
        "E-CTOR-VISIBILITY" => {
            "E-CTOR-VISIBILITY — a `private`/`protected` constructor was called from outside its scope.\n\n\
             A `private constructor` is callable only inside the declaring class; a `protected` one only\n\
             inside that class and its subclasses (an un-annotated constructor is `public`). This blocks\n\
             external `new C(...)` so construction is funneled through a factory — e.g. a static factory\n\
             method or a static field initializer (the singleton pattern), both of which run in the\n\
             class's own scope. Enforced in the type-checker so the interpreter, VM, and transpiled PHP\n\
             all reject the same construction.\n"
        }
        "E-CTOR-MODIFIER" => {
            "E-CTOR-MODIFIER — a non-visibility modifier was placed on a constructor.\n\n\
             A constructor takes at most one visibility modifier (`private`/`protected`/`internal`/\n\
             `public`). `abstract`/`static`/`const`/`open`/`mutable` are meaningless on a constructor\n\
             and are rejected rather than silently dropped. Remove the offending modifier.\n"
        }
        "E-DUP-PARAM" => {
            "E-DUP-PARAM — two parameters share a name.\n\n\
             Every parameter of a function, method, or constructor must have a distinct name —\n\
             otherwise the later one silently shadows the earlier (and a different-typed duplicate is a\n\
             trap). Rename one of them.\n"
        }
        "E-DUP-FIELD" => {
            "E-DUP-FIELD — an instance field is declared more than once.\n\n\
             Two explicit field declarations with the same name collide (the later silently won).\n\
             Give each field a distinct name. (An explicit field that also names a promoted constructor\n\
             param is allowed — the explicit declaration is authoritative.)\n"
        }
        "E-MAIN-SIGNATURE" => {
            "E-MAIN-SIGNATURE — the entry point `main` has an unsupported signature.\n\n\
             `main` is where a Phorj program starts. It may take no parameters, or a single\n\
             `List<string>` parameter (the program arguments — everything after `phg run file.phg --`).\n\
             It returns `void` (exit code 0) or `int` (the process exit code). Examples:\n\
             `function main(): void { … }`, `function main(): int { return 0; }`,\n\
             `function main(List<string> args): int { return args.length; }`. The same argv is also\n\
             available anywhere via `Core.Process.args()`.\n\n\
             The entry may also be a class `static` method named `main` (Java-style):\n\
             `class App { static function main(): int { return 0; } }` — same signature rules.\n"
        }
        "E-MULTIPLE-MAIN" => {
            "E-MULTIPLE-MAIN — RETIRED, never emitted. Kept only so an old build log or note that\n\
             quotes this code still explains itself.\n\n\
             The name `main` carries no meaning: a free function or a static method is an entry ONLY\n\
             if it is attributed `#[Entry(kind: EntryKind.…)]` (DEC-331/DEC-337). The live rule is at\n\
             most ONE entry PER KIND — see `phg explain E-DUPLICATE-ENTRY-KIND`. One `Cli` entry and\n\
             one `Web` entry may coexist in a program; `run` and `serve` each take their own.\n"
        }
        "E-TEST-OUTSIDE-TESTS" => {
            "E-TEST-OUTSIDE-TESTS — a `test \"name\" { … }` block appears in a normal build.\n\n\
             A `test` block is a unit test (M-Test). It is only valid in a file run by `phg test`, so\n\
             production code (run/check/transpile) cannot smuggle test blocks into a release. Move\n\
             the block into a `*.phg` file under a `tests/` directory and run `phg test`. `test` is a\n\
             contextual keyword, so it stays usable as an ordinary identifier everywhere else.\n"
        }
        "E-STATIC-CALL" => {
            "E-STATIC-CALL — a class-name method call `ClassName.method(…)` didn't resolve to a static method.\n\n\
             `ClassName.method(args)` calls a `static` method with no receiver. It is an error when\n\
             `method` is an *instance* method — call it on an instance instead (`x.method(…)`).\n\
             Inherited and trait-supplied static methods resolve fine (Statics-A), and overloaded static\n\
             methods are dispatched by argument type (Statics-B).\n"
        }
        "E-INJECTED-VARIANT-BARE" => {
            "E-INJECTED-VARIANT-BARE — a compiler-injected enum's variant was used bare.\n\n\
             `import Core.Json;` injects the `Json` enum, `Core.Decimal` injects `RoundingMode`, and\n\
             `import Core.Runtime.EntryKind;` injects `EntryKind` (the `#[Entry(kind:)]` role).\n\
             Their variants are names you never wrote, so — unlike a user-declared enum — they must be\n\
             reached *qualified* (\"nothing in the wind\"): write `new Json.Object(…)` / `new Json.Int(…)`\n\
             to construct and `Json.Object(es) => …` to match, never the bare `Object(…)`; and write\n\
             `#[Entry(kind: EntryKind.Cli)]`, never the bare `#[Entry(kind: Cli)]`. A user enum's\n\
             own variants stay bare (`new Some(7)`).\n"
        }
        "E-INJECTED-TYPE-BARE" => {
            "E-INJECTED-TYPE-BARE — a compiler-injected Core type was used bare without importing it.\n\n\
             The multi-type Core modules inject several types: `Core.Http` → `Request`/`Response`/`Route`/`Router`\n\
             + the bags (`ParamBag`/`HeaderBag`/`AttrBag`/`FileBag`/`RequestBody`/`UploadedFile`/`MultipartPart`) + `#[Route]`, `Core.Time` → `Duration`/`Date`/`Instant`,\n\
             `Core.Decimal` → `RoundingMode`. These are names you never wrote, so — like injected enum\n\
             variants (\"nothing in the wind\") — a bare use is only allowed when you explicitly\n\
             member-import it: `import Core.Http.Router;` then `Router` is bare. Otherwise write it\n\
             qualified — `Http.Router`, `#[Http.Route]`, `Time.Duration` — which needs the module import\n\
             `import Core.Http;`. A user-declared type of the same name shadows the injected one and is\n\
             unaffected. Single-type modules (`Core.Json`, `Core.Regex`, `Core.Secret`) are unaffected —\n\
             their leaf IS the type.\n"
        }
        "E-IMPORT-GROUP-EMPTY" => {
            "E-IMPORT-GROUP-EMPTY — a grouped import `import Prefix.{ … };` named no members.\n\n\
             A brace group must list at least one name: `import Core.Result.{ Success, Failure };`\n\
             (with an optional `as` alias per member, and a trailing comma allowed). An empty `{}`\n\
             imports nothing — delete the group, or fill in the members you meant to import.\n"
        }
        "E-WILDCARD-ALIAS" => {
            "E-WILDCARD-ALIAS — a wildcard import `import X.*` cannot be aliased (`* as Y`).\n\n\
             A flat wildcard binds every public/internal member of `X` under its own name, so there is\n\
             no single name for an alias to rename. Import the member explicitly to alias it\n\
             (`import X.Member as Y;`), or use a group (`import X.{ Member as Y };`). Namespace-object\n\
             binding (`import X.* as ns;` giving `ns.Member`) is a separate, not-yet-supported feature.\n"
        }
        "E-WILDCARD-STDLIB-ROOT" => {
            "E-WILDCARD-STDLIB-ROOT — a bare `import Core.*;` is not allowed.\n\n\
             `Core` is the whole standard-library root; a wildcard there would flood the file with\n\
             every module and member. Import a specific SUBMODULE instead (`import Core.Http.*;`,\n\
             `import Core.List.*;`) or a single member (`import Core.Output.printLine;`).\n"
        }
        "E-WILDCARD-EMPTY" => {
            "E-WILDCARD-EMPTY — a wildcard import `import X.*;` bound no names.\n\n\
             The package exports nothing this file can import (cross-package: only `public` members\n\
             are reachable), or an `except { … }` / an explicit import removed them all. Delete the\n\
             wildcard, or import the specific member(s) you meant.\n"
        }
        "E-WILDCARD-NO-PROJECT" => {
            "E-WILDCARD-NO-PROJECT — a wildcard import `import X.*;` was used outside a project.\n\n\
             A wildcard is compile-time sugar the loader expands into per-member imports against a\n\
             package graph. In single-file or `-e` mode there is no such graph — the reserved single\n\
             `Main` package has nothing to wildcard-import — so the `*` cannot be expanded. Import the\n\
             members explicitly (`import X.Member;`), or run inside a project (a `src/`-rooted tree)\n\
             where `X` resolves.\n"
        }
        "E-EXCEPT-UNKNOWN" => {
            "E-EXCEPT-UNKNOWN — an `except { … }` clause named a member the package does not have.\n\n\
             `import X.* except { A };` may only exclude names that `X.*` would actually bind. A typo\n\
             or a removed/renamed member trips this. Fix the name, or drop it from the `except` list.\n"
        }
        "E-IMPORT-AMBIGUOUS" => {
            "E-IMPORT-AMBIGUOUS — two wildcard imports would bind the same name.\n\n\
             If `import A.*;` and `import B.*;` both export `Thing`, the reference `Thing` is ambiguous.\n\
             Phorj rejects this eagerly (whether or not `Thing` is used). Disambiguate by importing the\n\
             one you want explicitly (`import A.Thing;` — an explicit import wins over a wildcard), or\n\
             exclude it from one side (`import B.* except { Thing };`).\n"
        }
        "E-RESULT-TOOPTION-NEEDS-OPTION" => {
            "E-RESULT-TOOPTION-NEEDS-OPTION — `Result.toOption` was used without importing `Core.Option`.\n\n\
             `Result.toOption(r)` (or `r.toOption()`) bridges a `Result<T, E>` to an `Option<T>` —\n\
             `Success(x)` becomes `Some(x)`, `Failure` becomes `None`. Its result IS a `Core.Option`\n\
             value, and (like every injected Core type) `Option`'s `Some`/`None` are only available when\n\
             you import the module. Add `import Core.Option;` alongside `import Core.Result;`. Without it\n\
             the call would run on the interpreter/VM but fail once transpiled to PHP (the `Some`/`None`\n\
             classes are never emitted), so the checker rejects it up front to keep every backend in step.\n"
        }
        "E-VARIANT-QUALIFIER" => {
            "E-VARIANT-QUALIFIER — a qualified variant pattern named the wrong enum.\n\n\
             In a `match`, a qualified pattern `Enum.Variant(…)` must name the *scrutinee's* enum. If\n\
             the scrutinee is a `Shape`, an arm `Color.Red(c) => …` is a mistake — the qualifier says\n\
             `Color` but the value is a `Shape`. Use the scrutinee's enum (`Shape.Circle(…)`) or the\n\
             bare form (`Circle(…)`), which resolves against the scrutinee automatically.\n"
        }
        "E-STATIC-VIA-INSTANCE" => {
            "E-STATIC-VIA-INSTANCE — a `static` method was called through an instance.\n\n\
             A static method belongs to the class, not an instance, so it is reached only as\n\
             `ClassName.method(…)` — never `instance.method(…)` or `this.method(…)`. This mirrors the\n\
             static-field rule (`instance.staticField` is likewise not an instance member). PHP tolerates\n\
             `$a->staticMethod()`, but Phorj keeps the class/instance boundary explicit. Rewrite the call\n\
             with the class name: `Account.make(…)` rather than `a.make(…)`.\n"
        }
        "E-STATIC-FIELD-VIA-INSTANCE" => {
            "E-STATIC-FIELD-VIA-INSTANCE — a `static` field was read through an instance.\n\n\
             A static field belongs to the class, not an instance, so it is read only as\n\
             `ClassName.field` — never `instance.field`. This is the field sibling of\n\
             E-STATIC-VIA-INSTANCE (static methods). Rewrite the access with the class name:\n\
             `Account.count` rather than `a.count`.\n"
        }
        "E-STATIC-THIS" => {
            "E-STATIC-THIS — a static method accessed instance state.\n\n\
             A `static` method belongs to the class, not an instance, so it has no `this` and cannot\n\
             read instance fields (bare or via `this`). Access static members as `Class.member`, pass\n\
             the value in as a parameter, or make the method non-static (drop `static`). A static\n\
             factory may still construct the class (`new Self(…)`).\n"
        }
        "E-BARE-FIELD" => {
            "E-BARE-FIELD — an instance field was referenced without `this.`.\n\n\
             Phorj has no bare field access: a field is always written `this.field`, exactly like\n\
             PHP's `$this->field`. A bare name inside a method resolves to a parameter, a local, or a\n\
             captured variable — never silently to a field — so that adding a local can never quietly\n\
             rebind what looked like a field. Qualify it:\n\n\
             \tfunction total(): int { return this.amount + this.tax; }  // not `amount + tax`\n\n\
             (In a static method there is no instance at all — that is `E-STATIC-THIS`.)\n"
        }
        "E-CONST-INSTANCE-ACCESS" => {
            "E-CONST-INSTANCE-ACCESS — a constant was read through an instance.\n\n\
             A `const` lives on the class, not the instance: read it as `ClassName.NAME`, never\n\
             `instance.NAME` (the same class-name-only rule as a `static` field).\n"
        }
        "E-CONST-REASSIGN" => {
            "E-CONST-REASSIGN — a `const` class constant was assigned to.\n\n\
             Constants are fixed at declaration and can never be reassigned. For class-level state that\n\
             changes, use a `static mutable` field instead.\n"
        }
        "E-FIELD-INIT-FORWARD-REF" => {
            "E-FIELD-INIT-FORWARD-REF — a field initializer reads a not-yet-initialized field.\n\n\
             Expression field initializers run per-instance at construction, in declaration order, after\n\
             the promoted constructor params are bound. An initializer may read `this` and any\n\
             EARLIER-declared field (or a promoted param) — but not a later field, nor itself. Declare\n\
             the field it depends on first, or set this one in the constructor.\n"
        }
        "E-FIELD-UNINITIALIZED" => {
            "E-FIELD-UNINITIALIZED — a non-optional instance field is never definitely assigned.\n\n\
             A non-optional field carries a `T` that the type system guarantees holds a value, so it must\n\
             be set on EVERY path of the constructor — otherwise the object is built with the field unset\n\
             and reading it faults at runtime (`no field x`). Four ways to satisfy it: assign\n\
             `this.x = …` unconditionally in the constructor (a one-branch `if` is not 'every path'),\n\
             give the field an initializer (`int x = 0;`), make it a promoted ctor param\n\
             (`constructor(public int x)`), or make it optional (`int? x;` — defaults to `null`).\n"
        }
        "E-FIELD-INIT-TYPE" => {
            "E-FIELD-INIT-TYPE — a field initializer's type does not match the field's declared type.\n\n\
             The initializer expression must be assignable to the field's type — e.g. `int weight =\n\
             compute(3);`, not `int weight = \"x\";`.\n"
        }
        _ => return None,
    })
}
