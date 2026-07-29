//! `phg explain` sub-catalog: declaration visibility, optional access, warning lints, module resolution & import hygiene, html, casing, casts
//! (M-Decomp, Invariant 13 — dispatched from `explain/mod.rs`; same
//! `text(code) -> Option<&'static str>` contract as `explain_config`).

/// Explanation text for a code in this band, or `None` when `code` is not this catalog's.
pub(super) fn text(code: &str) -> Option<&'static str> {
    Some(match code {
        "E-VIS-PRIVATE" => {
            "E-VIS-PRIVATE — a `private` declaration was referenced from another file.\n\n\
             A declaration marked `private` (visibility modifiers) is visible only within its own\n\
             `.phg` file. Referencing it from any other file — even one in the same package — fails.\n\
             Mark it `internal` (visible package-wide) or `public` (visible everywhere) to widen it.\n"
        }
        "E-VIS-INTERNAL" => {
            "E-VIS-INTERNAL — an `internal` declaration was referenced from another package.\n\n\
             A declaration marked `internal` is visible only within its own package (all its files),\n\
             not from other packages. A cross-package reference (a type import, or a qualified\n\
             `pkg.fn()` call) fails. Mark it `public` to export it across packages.\n"
        }
        "E-OPT-USE" => {
            "E-OPT-USE — a plain `.field` / `.method()` was used on an optional `T?` receiver.\n\n\
             The receiver could be `null`, so a plain member access risks a null dereference. Use\n\
             `?.` for null-safe access (the whole access yields `null` when the receiver is null),\n\
             or first narrow the optional with `if (var x = opt) { … }` or `opt!` (checked).\n"
        }
        "E-IF-LET-TYPE" => {
            "E-IF-LET-TYPE — `if (var x = …)` was given a non-optional scrutinee.\n\n\
             The if-let form narrows an optional `T?` to its non-null inner `T`, binding it inside\n\
             the then-block. A scrutinee that is already non-optional has nothing to narrow — use a\n\
             plain `if (cond)` for a boolean test, or make the scrutinee a `T?`.\n"
        }
        "E-OPT-UNWRAP" => {
            "E-OPT-UNWRAP — force-unwrap `!` was applied to a non-optional value.\n\n\
             `opt!` asserts that an optional `T?` is non-null and unwraps it to `T` (faulting at\n\
             runtime if it is null). A value that is already a non-optional `T` has nothing to\n\
             unwrap — remove the `!`.\n"
        }
        "W-FORCE-UNWRAP" => {
            "W-FORCE-UNWRAP — a force-unwrap `!` may fault at runtime (lint).\n\n\
             `opt!` aborts the program if the optional is null. This is a deliberate guardrail: it\n\
             flags every `!` so you can prefer a total alternative — `??` (default value), `?.`\n\
             (safe access), or `if (var x = opt) { … }` (narrow) — where null is a real possibility.\n"
        }
        "W-SECRET" => {
            "W-SECRET — a Secret's plaintext is exposed directly into a sink (lint).\n\n\
             `Secret<T>` is opaque: it cannot be printed or interpolated (that is a type error), and\n\
             `.expose()` is the only way to read the wrapped value. This lint fires when an\n\
             `.expose()` call is a *direct* argument to a sink — `Output.printLine`/`Output.print` or\n\
             `Core.File.write` — because the plaintext would then be logged or persisted. Bind the\n\
             exposed value and use it deliberately (hash it, compare it), or avoid sending a secret to\n\
             the sink at all. (The lint is syntactic on the direct argument; a value laundered through\n\
             a local is not flagged — the type-system non-printability is the real guarantee.)\n"
        }
        "W-SQL-INJECTION" => {
            "W-SQL-INJECTION — a value is string-interpolated into `Core.DatabaseModule` SQL (lint, DEC-208).\n\n\
             `db.prepare(\"SELECT * FROM users WHERE id = {userId}\")` splices `userId` straight into the\n\
             SQL text: if it carries user input, an attacker can inject arbitrary SQL. This lint is\n\
             type-directed — it fires only on `Core.DatabaseModule`'s `Database.prepare(...)` when the SQL is an interpolated\n\
             literal whose hole is a NON-constant value (a variable, field, or call). A fully-constant\n\
             interpolation (every hole a literal) and a plain non-interpolated literal never warn.\n\n\
             The fix is a bound placeholder — the value is sent to the database SEPARATELY from the SQL\n\
             text and can never be parsed as SQL:\n\n\
             \x20   Statement s = db.prepare(\"SELECT * FROM users WHERE id = ?\")?;\n\
             \x20   List<Row> rows = s.bind(userId)?.query()?;\n\n\
             (or a named placeholder `:id` with `.bindNamed(\"id\", userId)`). This is a WARNING, not an\n\
             error: a deliberately-built constant query still compiles — but interpolating a value is\n\
             almost always the wrong tool, so the lint is loud. Like every `W-…` lint it rides the warning\n\
             channel and never fails the build.\n"
        }
        "W-DEPRECATED" => {
            "W-DEPRECATED — a deprecated stdlib symbol is used (lint).\n\n\
             The symbol still works, but it is slated for removal: this lint names its replacement and\n\
             the version in which it will be removed. Per `SEMVER.md` a deprecated symbol emits this\n\
             warning for at least one minor release before it is removed (and the removal is a\n\
             documented `### Breaking` CHANGELOG entry). Migrate to the named replacement; see\n\
             `docs/DEPRECATION.md` for the policy and `STABILITY.md` for the deprecated tier. Like\n\
             every `W-…` lint it rides the warning channel and never fails the build.\n"
        }
        "E-LAMBDA-THIS" => {
            "E-LAMBDA-THIS — a field-initializer lambda captures `this`.\n\n\
             A method-body lambda MAY capture `this` (it is captured live, by the instance handle). The\n\
             one place it is rejected is a field or static initializer: that code runs while the\n\
             instance is only partially built, so capturing the receiver would expose half-initialized\n\
             fields. Move the closure into the constructor body, or capture a specific value\n\
             (`var v = this.x;`) before building the closure.\n"
        }
        "W-SHADOWED" => {
            "W-SHADOWED — the same package exists under more than one search root.\n\n\
             Imports resolve against three ordered roots (entry directory → `src/` → `vendor/`);\n\
             the MOST SPECIFIC root wins. That win is deliberate (local override of a vendored\n\
             package is the standard escape hatch) but never silent — this warning names both\n\
             locations so an accidental shadow is visible. Remove or rename one of the two to\n\
             silence it.\n"
        }
        "E-MODULE-NOT-FOUND" => {
            "E-MODULE-NOT-FOUND — an import does not resolve to any package on disk.\n\n\
             DEC-282 unified loading: imports resolve against THREE ordered search roots — the\n\
             entry file's own directory, then the app root's `src/`, then its `vendor/` (the app\n\
             root is the nearest ancestor directory containing `src/` or `vendor/`; with neither,\n\
             the entry's directory is the only root). Packages live in folders matching their name\n\
             (folder = package): `import Model;` needs `Model/*.phg` declaring `package Model;`\n\
             under one of those roots. The error lists exactly what was searched. Dependencies must\n\
             already be on disk under `vendor/` — phg never downloads code (a package-manager\n\
             extension writes `vendor/`; the compiler only reads it).\n"
        }
        "E-IMPORT-MAIN" => {
            "E-IMPORT-MAIN — `import Main;` (or `Main.…`) is never legal.\n\n\
             `Main` is the ENTRY package: location-free, name-free, and unimportable — every file's\n\
             own package is already in scope, and no other file can depend on an entry. Shared code\n\
             belongs in a folder-named package (`src/Model/…` ⇒ `package Model;`) and is imported\n\
             by that name. (DEC-282 — previously this import was silently accepted as a no-op.)\n"
        }
        "E-UNUSED-IMPORT" => {
            "E-UNUSED-IMPORT — an import that nothing in the file references.\n\n\
             Go-style import hygiene (DEC-282, developer-ruled HARD): an unused import is dead\n\
             text — remove it, or use it. The bound names checked are the import's leaf (or its\n\
             `as` alias) plus, for a whole-module `import Core.X;`, every type that module\n\
             injects (`Core.IteratorModule` binds `Iterator`; `Core.Runtime` binds `Entry`; …).\n\
             The check is a whole-word source scan and deliberately over-approximates — a mention\n\
             anywhere (even a comment) counts as a use — so it never mis-flags a real use.\n"
        }
        "W-PHG-IN-DOCROOT" => {
            "W-PHG-IN-DOCROOT — a `.phg` file (other than `index.phg`) sits inside `public/`.\n\n\
             The docroot is the ONLY web-exposed surface (`phg serve <dir>`); source under it is\n\
             never served (the static layer guards `.phg` bytes with a 404), but code does not\n\
             belong there either — move it outside `public/` (e.g. into `src/`), keeping\n\
             `public/` to the front controller + static assets.\n"
        }
        "E-DUP-IMPORT" => {
            "E-DUP-IMPORT — the same import is written twice in one file.\n\n\
             A repeated import is dead text with no legitimate reading (it binds nothing new), so\n\
             it is a hard error rather than a warning (DEC-282, Go-style import hygiene). Remove\n\
             the repeated line.\n"
        }
        "E-VENDOR-MISSING" => {
            "E-VENDOR-MISSING — a declared dependency is not vendored.\n\n\
             Dependencies resolve offline from the committed `vendor/` tree — Phorj never fetches on\n\
             `run`/`check`/`transpile`. Run `phg install` to clone each `phorj.json` dependency at its\n\
             pinned tag/rev into `vendor/` and write `phorj.lock`, then commit both (DEC-316; the\n\
             older `phorj.toml`/`[require]`/`phg vendor` mechanism is retired — DEC-282).\n"
        }
        "E-VENDOR-MAIN" => {
            "E-VENDOR-MAIN — a vendored dependency declared `package Main`.\n\n\
             A dependency is a library: it exports dotted packages (e.g. `package acme.strutil;`),\n\
             never the reserved `package Main` (which would collide with the consuming program's\n\
             entry). Fix the dependency to use a dotted package, or remove the stray `main` File.\n"
        }
        "E-DUP-DEF" => {
            "E-DUP-DEF — two functions share a name within one package.\n\n\
             After the project + its vendored dependencies are merged, every function is keyed by\n\
             `(package, name)` and must be unique. Two files declaring the same `package` cannot both\n\
             define a function of the same name — rename one, or move it to a different package.\n"
        }
        "E-HTML-HOLE" => {
            "E-HTML-HOLE — a value of an un-renderable type was interpolated into `html\"…\"`.\n\n\
             An `html\"…\"` hole `{e}` accepts an `Html` fragment (embedded as-is), a `string`, or a\n\
             primitive (`int`/`float`/`bool`, escaped). Anything else — a class, enum, list, optional\n\
             — has no safe HTML rendering. Render it first: build it with the html builders\n\
             (`Html.el(…)`), produce a `string` and let the hole escape it, or wrap audited markup in\n\
             `Html.raw(…)`.\n"
        }
        "E-UNKNOWN-TAG" => {
            "E-UNKNOWN-TAG — a tagged-template literal `tag\"…\"` used a tag that has no desugar.\n\n\
             The tagged-template syntax (any identifier immediately followed by `\"`, e.g. `sql\"…\"`)\n\
             is generalized, but only `html\"…\"` currently has an implementation. Every other tag is a\n\
             scaffold placeholder: the general two-mode (protocol / function) desugar is not yet added.\n\
             Use `html\"…\"`, or a plain string, until the tag you want is implemented.\n"
        }
        "E-HTML-IMPORT" => {
            "E-HTML-IMPORT — `html\"…\"` was used without importing Core.Html.\n\n\
             The `html\"…\"` literal desugars to `Html.raw`/`Html.text`/`Html.concat` kernel calls, so\n\
             the module must be in scope. Add `import Core.Html;` (or `import Core.Html as h;`) to the\n\
             File.\n"
        }
        "E-NAME-CASE" => {
            "E-NAME-CASE — a value identifier is not camelCase.\n\n\
             Functions, methods, parameters, fields, variable bindings, and lambda parameters must be\n\
             camelCase: a lowercase first letter and no underscores (e.g. `splitOnce`, `cToF`, `area`).\n\
             This is the value half of Phorj's casing rule (types/enums/variants are PascalCase via\n\
             E-TYPE-CASE); both are front-end-only, so they never change the generated PHP. Rename the\n\
             identifier — the diagnostic suggests the converted form (`split_once` → `splitOnce`).\n"
        }
        "E-TYPE-CASE" => {
            "E-TYPE-CASE — a type identifier is not PascalCase.\n\n\
             Class names, enum names, enum variant names, and `type` alias names must be PascalCase: an\n\
             uppercase first letter and no underscores (e.g. `Shape`, `Circle`, `HttpRequest`). This is\n\
             the type half of Phorj's casing rule (functions/variables/params are camelCase via\n\
             E-NAME-CASE); both are front-end-only, so they never change the generated PHP. Rename the\n\
             type — the diagnostic suggests the converted form (`shape` → `Shape`).\n"
        }
        "E-PKG-CASE" => {
            "E-PKG-CASE — a package or import segment is not PascalCase.\n\n\
             Every package/folder segment is PascalCase (e.g. `package Acme.StringUtil;` lives in\n\
             `src/Acme/StringUtil/`), and so are import path segments and an import `as` alias\n\
             (`import Acme.StringUtil as Strutil;`). This makes the source-to-PHP namespace mapping 1:1\n\
             with no casing transform (`Acme.StringUtil` ⇒ `Acme\\StringUtil`). The reserved roots\n\
             `Main` (the runnable entry) and `Core` (the standard library) are already PascalCase. It is\n\
             front-end-only, so it never changes the generated PHP — rename the segment to the suggested\n\
             form (`acme` → `Acme`).\n"
        }
        "E-INSTANCEOF-TYPE" => {
            "E-INSTANCEOF-TYPE — an `is` / `instanceof` type-test operand is not valid.\n\n\
             `value is T` (equivalently `value instanceof T`) tests a value's runtime type. The right\n\
             operand must name a declared **class or interface**, OR a **discriminable primitive**\n\
             (`int`/`float`/`string`/`bool`/`null`) — `is`/`instanceof` are full synonyms and both\n\
             accept either (DEC-184). `decimal`/`bytes`/`html`/`attr` can't be tested (they erase to a\n\
             PHP string — `E-MATCH-TYPE-ERASED`). The result is `bool`, and inside `if (x is T)` the\n\
             operand `x` is smart-cast to `T` in the then-block (a primitive narrows in the then-branch;\n\
             a class narrows in then and else).\n"
        }
        "E-CAST-TYPE" => {
            "E-CAST-TYPE — an `as` cast operand is not valid.\n\n\
             `as` has two axes. Over a **class/interface** it is a checked downcast: `value as T`\n\
             yields `T?` (the value when it really is a `T` at runtime, else `null` — Kotlin/Swift\n\
             `as?`); the right operand must name a declared class or interface and the left must be a\n\
             class instance (or a union/intersection of them). Compose with `??` or if-let\n\
             (`if (var c = v as T) { … }`).\n\n\
             Over a **primitive** it is a value conversion, fallibility-typed: lossless → total `T`\n\
             (`int as float`, `int as decimal`, `decimal as float`, any `as string`); lossy/fallible →\n\
             `T?` (`float`/`decimal as int` is exact-or-null — never a silent truncate; `string as\n\
             int`/`as float` is a strict parse). It never inherits PHP's loose coercion. This error\n\
             fires for a pair that is impossible or not yet supported (bool casts, `float as decimal`,\n\
             `string as decimal` ship in a later slice) — use `Core.Conversion` / `Core.String.parse*`, or\n\
             `Conversion.truncate` when you explicitly want truncation.\n"
        }
        "W-REDUNDANT-CAST" => {
            "W-REDUNDANT-CAST — a cast whose target is already the value's type (lint).\n\n\
             `value as T` where `value` is already a `T` does nothing — e.g. `n as int` when `n: int`.\n\
             It is harmless (the value passes through) but reads as if a conversion happens. Remove the\n\
             `as`. This is a non-fatal warning; it never gates the build.\n"
        }
        _ => return None,
    })
}
