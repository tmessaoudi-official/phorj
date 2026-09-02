//! `phg explain` sub-catalog: declaration files, import diagnostics, String.format, backed enums
//! (M-Decomp, Invariant 13 — dispatched from `explain/mod.rs`; same
//! `text(code) -> Option<&'static str>` contract as `explain_config`).

/// Explanation text for a code in this band, or `None` when `code` is not this catalog's.
pub(super) fn text(code: &str) -> Option<&'static str> {
    Some(match code {
        "E-DECL-PACKAGE" => {
            "E-DECL-PACKAGE — a `.d.phg` declaration file declares a `package`.\n\n\
             A `*.d.phg` ambient-declaration file (M8.5) describes global foreign PHP symbols, which\n\
             have no package. Remove the `package` line. (Ordinary `.phg` files, by contrast, MUST\n\
             declare a package — see E-NO-PACKAGE.)\n"
        }
        "E-DECL-NONFOREIGN" => {
            "E-DECL-NONFOREIGN — a `.d.phg` declaration file contains a non-`declare` item.\n\n\
             A `*.d.phg` file (M8.5) may contain only foreign ambient declarations — every `function`\n\
             / `class` in it must be `declare`d (it describes existing PHP, it does not define Phorj\n\
             behavior). Move any real implementation into a normal `.phg` file, or mark the item\n\
             `declare`.\n"
        }
        "E-IMPORT-UNKNOWN" => {
            "E-IMPORT-UNKNOWN — an `import` names a member (type, function, or sub-module) a known\n\
             package does not export.\n\n\
             `import Acme.Geometry.Point [as P];` names a public member a package actually exports —\n\
             a type, a function, or (for a wildcard/group) a sub-module. This fires when the package is\n\
             known (it exports other members) but not the named one: a mistyped type, function, or\n\
             sub-module import, single (`import Acme.Geometry.Nope;`), grouped\n\
             (`import Acme.Geometry.{ Point, Nope };`), or wildcard-`except` (`except { Nope }`). Check\n\
             the package path and the member name. It also fires for a fault-intrinsic member import\n\
             that names a non-member — `import Core.Abort.bogus;` (the intrinsics are\n\
             `Core.Assert.assert` and `Core.Abort.{ panic, todo, unreachable }`).\n"
        }
        "E-UNIMPORTED" => {
            "E-UNIMPORTED — a name is used without a covering import (DEC-196 Q3; DEC-337).\n\n\
             The four fault intrinsics live in two reserved modules — `Core.Assert` (`assert`) and\n\
             `Core.Abort` (`panic`/`todo`/`unreachable`) — and follow the two-mode import discipline:\n\
             \n\
               * a WHOLE-MODULE import enables the QUALIFIED call — `import Core.Assert;` then\n\
                 `Assert.assert(cond)`;\n\
               * a MEMBER import enables the BARE call — `import Core.Abort.panic;` then `panic(\"m\")`\n\
                 (groups work: `import Core.Abort.{ panic, todo };`).\n\
             \n\
             A bare `assert(...)` needs the member import; a qualified `Assert.assert(...)` needs the\n\
             module import. The same code covers the `#[Entry(kind: EntryKind.Cli)]` role: a qualified\n\
             `EntryKind.Cli` needs `import Core.Runtime.EntryKind;` (or the whole module\n\
             `import Core.Runtime;`). Add the matching import to the file.\n"
        }
        "E-IMPORT-BUILTIN" => {
            "E-IMPORT-BUILTIN — an `import` names a built-in type.\n\n\
             Built-in types (`int`, `float`, `bool`, `string`, `bytes`, `List`, `Map`, `Set`, …) are\n\
             import-free — they are always in scope, like `int`. Remove the `import` for a built-in;\n\
             only user/library types are imported.\n"
        }
        "E-IMPORT-CONFLICT" => {
            "E-IMPORT-CONFLICT — two imports bind the same bare type name.\n\n\
             Each type import introduces a bare type name into the file; two imports that would bind\n\
             the same name collide. Alias one with `as`: `import Acme.B.Point as BPoint;`.\n"
        }
        "E-IMPORT-SHADOW" => {
            "E-IMPORT-SHADOW — an imported type name collides with a local type or module qualifier.\n\n\
             The bare name a type import introduces must not shadow a type declared in this file or an\n\
             imported module qualifier. Alias the import with `as` to give it a distinct name, or\n\
             rename the local declaration.\n"
        }
        "E-IMPORT-NATIVE-MEMBER" => {
            "E-IMPORT-NATIVE-MEMBER — a member function was imported from a raw `Core.Native.*` module.\n\n\
             The raw-native modules (`Core.Native.Uri`, `Core.Native.Database`, … — DEC-277) support\n\
             the whole-module import form only: `import Core.Native.Uri;` then the qualified call\n\
             `Uri.encodeForm(...)`. Member fn-imports (`import Core.Native.Uri.encodeForm;`) are not\n\
             bindable — prefer the friendly prelude module (`import Core.UriModule;` →\n\
             `Uri.encodeForm(...)`), which wraps the same natives with typed errors.\n"
        }
        "E-FORMAT-ARGS" => {
            "E-FORMAT-ARGS — `String.format` was not called with exactly two arguments (W3-5).\n\n\
             `String.format(spec, values)` takes a format string and a list of values:\n\
             `String.format(\"%s = %d\", [name, count])`.\n"
        }
        "E-FORMAT-SPEC-TYPE" => {
            "E-FORMAT-SPEC-TYPE — `String.format`'s first argument is not a `string` (W3-5).\n\n\
             The format string must be a `string` (a literal, or a runtime `string` value for a\n\
             dynamic/i18n template).\n"
        }
        "E-FORMAT-ARGS-TYPE" => {
            "E-FORMAT-ARGS-TYPE — `String.format`'s second argument is not a list (W3-5).\n\n\
             Pass the values as a list: `String.format(\"%s\", [x])`. `%s`/`%d` consume the list by\n\
             position.\n"
        }
        "E-FORMAT-ARG-TYPE" => {
            "E-FORMAT-ARG-TYPE — a `String.format` value is not a printable scalar (W3-5).\n\n\
             The values must be `int`/`float`/`decimal`/`bool`/`string` — the types `%s`/`%d` can\n\
             render. Convert a composite value to a string first.\n"
        }
        "E-FORMAT-ARG-COUNT" => {
            "E-FORMAT-ARG-COUNT — a literal `String.format` spec's value count doesn't match its directives (W3-5).\n\n\
             For sequential directives, each `%s`/`%d` consumes one value (`%%` is a literal `%`, not a\n\
             directive) — give exactly one value per directive. For positional `%N$`, every value must be\n\
             referenced by some `%N$` (reuse/reorder is allowed) and no index may exceed the value count.\n\
             (Checked at compile time for a literal spec + literal list; a dynamic spec is checked at runtime.)\n"
        }
        "E-FORMAT-MIXED-POSITIONAL" => {
            "E-FORMAT-MIXED-POSITIONAL — a `String.format` spec mixes positional (`%N$`) and sequential directives (W3-5, slice 4b).\n\n\
             PHP allows mixing (`%s %1$s`), but it is a footgun; Phorj rejects it. Use ALL positional\n\
             (`%1$s %2$s`) — which lets you reorder and reuse values — or ALL sequential (`%s %s`), never\n\
             both in one spec. (Checked at compile time for a literal spec; a dynamic spec faults at render time.)\n"
        }
        "E-FORMAT-UNSUPPORTED" => {
            "E-FORMAT-UNSUPPORTED — a literal `String.format` spec uses a directive not yet supported (W3-5).\n\n\
             This version supports `%s`/`%d`/`%f`/`%%`, scientific `%e`/`%E`, shortest-repr `%g`/`%G`,\n\
             integer-radix `%x`/`%X`/`%o`/`%b`, and `%N$` positional args, with flags `-`/`0`/`+`, a width,\n\
             and a `.precision` on `%s` (truncate to N chars) and the float conversions `%f`/`%e`/`%E`/`%g`/`%G`\n\
             (default 6). Precision on `%d` is deliberately unsupported (PHP silently ignores it). Still\n\
             coming: the `%c` char conversion and precision on the radix conversions. (A dynamic runtime spec\n\
             faults at render time on an unsupported directive instead of at compile time.)\n"
        }
        "E-ENUM-BACKING-TYPE" => {
            "E-ENUM-BACKING-TYPE — a backed enum's backing type is not `int` or `string` (DEC-302).\n\n\
             A backed enum backs each variant with a scalar literal: `enum Suit: string { Hearts = \"H\" }`\n\
             or `enum Priority: int { Low = 1 }`. Only `int` and `string` are valid backing types\n\
             (matching PHP 8.1). Change the backing type, or drop it for a plain enum.\n"
        }
        "E-ENUM-BACKING-GENERIC" => {
            "E-ENUM-BACKING-GENERIC — a generic enum also declares a scalar backing type (DEC-302).\n\n\
             `enum E<T>: int { … }` combines generic type parameters with a scalar backing — the two are\n\
             mutually exclusive this version. Drop the type parameters, or drop the `: int`/`: string`.\n"
        }
        "E-ENUM-VALUE-UNBACKED" => {
            "E-ENUM-VALUE-UNBACKED — a variant assigns a value but the enum declares no backing type (DEC-302).\n\n\
             `= value` on a variant is only meaningful for a backed enum. Add a backing type\n\
             (`enum E: int { A = 1 }`), or drop the `= value` for a plain enum (`enum E { A }`).\n"
        }
        "E-ENUM-BACKED-PAYLOAD" => {
            "E-ENUM-BACKED-PAYLOAD — a backed-enum variant carries a payload (DEC-302).\n\n\
             A backed enum's variants are scalar-valued, so they cannot also carry constructor payload\n\
             fields (`enum E: int { A(int x) = 1 }` is rejected). Use a plain payload enum, or drop the\n\
             payload fields from the backed variant.\n"
        }
        "E-ENUM-VARIANT-NO-VALUE" => {
            "E-ENUM-VARIANT-NO-VALUE — a backed-enum variant is missing its value (DEC-302).\n\n\
             Every variant of a backed enum must assign a literal of the backing type:\n\
             `enum Suit: string { Hearts = \"H\", Spades = \"S\" }`. Add the missing `= value`.\n"
        }
        "E-ENUM-VALUE-NOT-LITERAL" => {
            "E-ENUM-VALUE-NOT-LITERAL — a backed-enum value is not a literal constant (DEC-302).\n\n\
             A variant's backing value must be a plain `int`/`string` literal (no interpolation, no\n\
             expression) — it is baked in at compile time. Use a literal, e.g. `High = 9`.\n"
        }
        "E-ENUM-VALUE-TYPE" => {
            "E-ENUM-VALUE-TYPE — a backed-enum value's type does not match the backing type (DEC-302).\n\n\
             In `enum Priority: int { Low = \"x\" }`, the value `\"x\"` is a string but the backing type is\n\
             `int`. Give every variant a literal of the declared backing type.\n"
        }
        "E-ENUM-DUP-VALUE" => {
            "E-ENUM-DUP-VALUE — two backed-enum variants share the same value (DEC-302).\n\n\
             `enum E: int { A = 1, B = 1 }` would make `E.from(1)` ambiguous. Each backed variant must\n\
             carry a distinct value.\n"
        }
        "E-ENUM-CASES-PAYLOAD" => {
            "E-ENUM-CASES-PAYLOAD — `Enum.cases()` used on an enum with payload variants (DEC-302).\n\n\
             `cases()` enumerates an enum's value-like variants in declaration order, so every variant\n\
             must be payload-less. A payload variant (`Circle(float r)`) has no canonical value to\n\
             enumerate. Use `cases()` only on a plain payload-less enum or a backed enum.\n"
        }
        "E-ENUM-RESERVED-VARIANT" => {
            "E-ENUM-RESERVED-VARIANT — a variant is named `cases`, `from`, or `tryFrom` (DEC-302).\n\n\
             Those three names are an enum's static-method surface (`Enum.cases()`, `Enum.from(x)`,\n\
             `Enum.tryFrom(x)`), so they cannot also name a variant. Rename the variant.\n"
        }
        "E-ENUM-NOT-BACKED" => {
            "E-ENUM-NOT-BACKED — `.value` / `from` / `tryFrom` used on a plain (non-backed) enum (DEC-302).\n\n\
             `s.value`, `Enum.from(x)`, and `Enum.tryFrom(x)` exist only for a backed enum (one that\n\
             declares an `int`/`string` backing type). Declare a backing type, e.g.\n\
             `enum Suit: string { Hearts = \"H\" }`.\n"
        }
        "E-REGEX-UNSUPPORTED" => {
            "E-REGEX-UNSUPPORTED — a `Regex.compile` pattern uses syntax the LINEAR engine omits.\n\n\
             `Regex.compile` is the ReDoS-immune engine (RE2-style, guaranteed linear time). It omits\n\
             exactly PCRE's backtracking-only syntax: look-ahead/look-behind `(?=…)` `(?<=…)`,\n\
             back-references `\\1` `\\k<n>`, atomic groups `(?>…)`, possessive quantifiers `a++`,\n\
             conditionals, recursion, `(*VERB)`s, `{,n}` and the escapes `\\h` `\\R` `\\Z` `\\G` `\\K`.\n\
             If you need them, use `Regex.compileBacktracking(...)` (DEC-461): the same API on a\n\
             backtracking engine with a step budget — a catastrophic pattern raises a fault instead\n\
             of hanging. Both compile to the same `preg_*` under PHP.\n"
        }
        "E-REGEX-INVALID" => {
            "E-REGEX-INVALID — a literal regex pattern does not parse.\n\n\
             The pattern was validated at check time with the engine that would compile it at run\n\
             time, and it would fault on every backend (`invalid regex: …`). Fix the pattern; remember\n\
             that patterns are best written as raw strings (`r\"…\"`) so `{n}` is a quantifier, not\n\
             string interpolation.\n"
        }
        _ => return None,
    })
}
