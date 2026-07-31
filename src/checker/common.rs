//! Stateless checker helpers (no `Checker` state) — case conversion, Levenshtein,
//! substitution, totality/loop predicates, built-in type names. (M-Decomp W2.)

use super::*;

/// Classic two-row Levenshtein edit distance (ASCII-oriented; M1 identifiers are ASCII), used to
/// suggest the nearest in-scope name for an unknown identifier.
/// The intrinsic-provider module that owns a fault intrinsic (DEC-196 Q3). `Core.Assert` holds the
/// conditional check `assert`; `Core.Abort` holds the unconditional aborts `panic`/`todo`/
/// `unreachable`. Importing the whole module enables the QUALIFIED call form (`Assert.assert(...)`);
/// a member import (`import Core.Abort.panic;`) enables the BARE form (`panic(...)`). Returns `None`
/// for non-intrinsic names. This is the single source of truth for the intrinsic name set.
pub(crate) fn intrinsic_module_of(name: &str) -> Option<&'static str> {
    match name {
        "assert" => Some("Core.Assert"),
        "panic" | "todo" | "unreachable" => Some("Core.Abort"),
        _ => None,
    }
}

/// The reserved fault-intrinsic names (M-faults 2a) — `panic`/`todo`/`unreachable` (`never`) and
/// `assert` (`unit`). Recognized at call sites and rejected as user function names.
pub(super) fn is_intrinsic_name(name: &str) -> bool {
    intrinsic_module_of(name).is_some()
}

pub(super) fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The original leaf identifier of a possibly loader-mangled name: the substring after the last
/// `\` (`Acme\Util\compute` ⇒ `compute`), or the whole string when unmangled. Casing is a property
/// of the source identifier, not the FQN the loader synthesizes (M5 S2c).
pub(super) fn leaf_ident(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}

/// camelCase: a lowercase ASCII first letter and no `_`. A single lowercase word (`main`, `area`,
/// `hi`) qualifies. Empty strings are not valid (the parser never produces them, but be total).
pub(super) fn is_camel(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && !s.contains('_')
}

/// PascalCase: an uppercase ASCII first letter and no `_` (`Shape`, `Circle`, `HttpRequest`).
/// `pub(crate)` so the loader can reuse the one canonical definition for its per-file package-decl
/// casing gate (W0-4), avoiding a drifting second copy.
pub(crate) fn is_pascal(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && !s.contains('_')
}

/// SCREAMING_SNAKE_CASE (Feature A — `const` names): an uppercase ASCII first letter, and every
/// character an uppercase letter, a digit, or `_` — no lowercase. `MAX`, `TAG`, `MAX_SIZE`, `HTTP_2`
/// qualify; `maxVal`, `Max` do not. The PHP/C/Java constant convention, chosen for legibility.
pub(super) fn is_screaming_snake(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && s.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Convert an identifier to the suggested SCREAMING_SNAKE_CASE form (`maxVal` → `MAX_VAL`,
/// `max_size` → `MAX_SIZE`, `PI` → `PI`): split on `_` and on camelCase humps, uppercase each word,
/// join with `_`.
pub(super) fn to_screaming_snake(s: &str) -> String {
    // First split existing `_` words, then split each on uppercase humps (`maxVal` → `max`,`Val`).
    let mut words: Vec<String> = Vec::new();
    for w in case_words(s) {
        let mut cur = String::new();
        for c in w.chars() {
            if c.is_ascii_uppercase() && !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            cur.push(c);
        }
        if !cur.is_empty() {
            words.push(cur);
        }
    }
    words
        .iter()
        .map(|w| w.to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join("_")
}

/// Split a snake_case-or-otherwise identifier into its `_`-delimited words, dropping empties (so a
/// leading/trailing/doubled `_` does not yield a blank word). Shared by both converters.
pub(super) fn case_words(s: &str) -> Vec<&str> {
    s.split('_').filter(|w| !w.is_empty()).collect()
}

/// Uppercase the first ASCII letter of a word, leaving the rest unchanged (`shape` → `Shape`,
/// `once` → `Once`). Non-alphabetic leads pass through.
pub(super) fn upper_first(w: &str) -> String {
    let mut cs = w.chars();
    match cs.next() {
        Some(c) => c.to_ascii_uppercase().to_string() + cs.as_str(),
        None => String::new(),
    }
}

/// Convert an identifier to the suggested camelCase form (`split_once` → `splitOnce`,
/// `c_to_f` → `cToF`, `shape` → `shape`): the first word lowercased-first, each later word
/// capitalized, joined with no separator.
pub(super) fn to_camel(s: &str) -> String {
    let words = case_words(s);
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if i == 0 {
            let mut cs = w.chars();
            if let Some(c) = cs.next() {
                out.push(c.to_ascii_lowercase());
                out.push_str(cs.as_str());
            }
        } else {
            out.push_str(&upper_first(w));
        }
    }
    out
}

/// Convert an identifier to the suggested PascalCase form (`shape` → `Shape`,
/// `http_request` → `HttpRequest`): every word capitalized, joined with no separator.
pub(super) fn to_pascal(s: &str) -> String {
    case_words(s).iter().map(|w| upper_first(w)).collect()
}

/// True for the built-in type names `resolve_type` handles directly — a `type` alias may not
/// shadow them (else the checker and the backend expansion would disagree; see `collect`).
/// Apply a unification substitution `θ` to a type, replacing each `Ty::Param(p)` by `θ[p]` (an
/// unbound parameter is left as-is). Used to compute a generic call's result type from the bindings
/// inferred at the call site (M-RT S7).
pub(super) fn apply_subst(ty: &Ty, theta: &HashMap<String, Ty>) -> Ty {
    match ty {
        Ty::Param(p) => theta
            .get(p)
            .cloned()
            .unwrap_or_else(|| Ty::Param(p.clone())),
        Ty::List(e) => Ty::List(Box::new(apply_subst(e, theta))),
        Ty::Set(e) => Ty::Set(Box::new(apply_subst(e, theta))),
        Ty::Optional(e) => Ty::Optional(Box::new(apply_subst(e, theta))),
        Ty::Map(k, v) => Ty::Map(
            Box::new(apply_subst(k, theta)),
            Box::new(apply_subst(v, theta)),
        ),
        Ty::Function(ps, r, es) => Ty::Function(
            ps.iter().map(|p| apply_subst(p, theta)).collect(),
            Box::new(apply_subst(r, theta)),
            es.iter().map(|e| apply_subst(e, theta)).collect(),
        ),
        // A generic class instance type carries its arguments — substitute through them so a
        // `Box<T>` return / field resolves to `Box<int>` (M-RT generics-all).
        Ty::Named(n, args) => Ty::Named(
            n.clone(),
            args.iter().map(|a| apply_subst(a, theta)).collect(),
        ),
        // A tuple substitutes through each position (DEC-288) — `zip`'s `List<(T, U)>` return resolves
        // to `List<(int, string)>` at the call site.
        Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| apply_subst(t, theta)).collect()),
        other => other.clone(),
    }
}

/// Whether a type contains a `Ty::Param` anywhere (recursing through containers/optionals/functions).
/// A native whose stored signature contains one is checked via call-site unification, exactly like a
/// generic free function (M-RT S7b).
pub(super) fn ty_has_param(ty: &Ty) -> bool {
    match ty {
        Ty::Param(_) => true,
        Ty::List(e) | Ty::Set(e) | Ty::Optional(e) => ty_has_param(e),
        Ty::Map(k, v) => ty_has_param(k) || ty_has_param(v),
        Ty::Function(ps, r, es) => {
            ps.iter().any(ty_has_param) || ty_has_param(r) || es.iter().any(ty_has_param)
        }
        Ty::Named(_, args) => args.iter().any(ty_has_param),
        Ty::Tuple(ts) => ts.iter().any(ty_has_param),
        _ => false,
    }
}

pub(super) fn is_builtin_type_name(name: &str) -> bool {
    matches!(
        name,
        "int"
            | "float"
            | "bool"
            | "string"
            | "bytes"
            | "never"
            | "void"
            | "empty"
            | "Html"
            | "Attr"
            | "List"
            | "Map"
            | "Set"
            | "decimal"
            | "double"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            // The built-in `Error` marker interface (M-faults 2b) — reserved so user code can't
            // redefine it (as a class/enum/interface/alias).
            | "Error"
            // Green-thread handle types (M6 W4): `Channel<T>` / `Task<T>`. Reserved built-ins like
            // `List`/`Map` — import-free, and user code cannot redefine them as a class/enum/etc.
            | "Channel"
            | "Task"
    )
}

/// Whether `name` is reserved *in PHP* for a top-level symbol of the given `kind` ("function" /
/// "class" / "enum" / "interface" / "trait" / "type alias") and would therefore transpile to invalid
/// PHP. These are words that are usable Phorj value identifiers (not Phorj keywords — lexed as
/// `Ident`) but a PHP parse error in the corresponding symbol position. The split is **kind-aware**
/// (verified empirically against PHP 8.5): the type words `int`/`float`/`object`/… are legal as a PHP
/// *function* name but illegal as a *class* name, so a `function int(){}` is fine while `class int{}`
/// is not — guarding both uniformly would over-reject valid code. PHP names are case-insensitive, so
/// the function/class sets compare case-folded; the contextual-keyword collision (`var` as a type
/// alias) is exact. Methods are exempt (legal as `->var()`), so this is never consulted for them.
pub(super) fn is_php_reserved_symbol_name(name: &str, kind: &str) -> bool {
    // Illegal as a PHP *function* name (and, being keywords/constructs, also as a class name).
    const FN_RESERVED: &[&str] = &[
        "array",
        "list",
        "print",
        "echo",
        "unset",
        "isset",
        "empty",
        "eval",
        "exit",
        "die",
        "include",
        "include_once",
        "require",
        "require_once",
        "global",
        "goto",
        "clone",
        "and",
        "or",
        "xor",
        "yield",
        // PHP-8 reserved keywords illegal as a class/function name (case-insensitive) — a phorj
        // `class Match`/`Enum`/`Fn` (any case) would transpile to a PHP parse error. `readonly` is in
        // CLASS_EXTRA; these three were missing (found building DEC-295's RegexMatch type).
        "match",
        "enum",
        "fn",
        "declare",
        "namespace",
        "use",
        "switch",
        "case",
        "default",
        "foreach",
        "elseif",
        "endif",
        "endfor",
        "endforeach",
        "endwhile",
        "endswitch",
        "enddeclare",
        "insteadof",
        "callable",
        "as",
        "var",
    ];
    // Additionally illegal as a PHP *class* name: the type words + `readonly`.
    const CLASS_EXTRA: &[&str] = &[
        "readonly", "int", "float", "bool", "string", "void", "iterable", "object", "mixed",
        "never", "self", "parent",
    ];
    let lower = name.to_ascii_lowercase();
    match kind {
        "function" => FN_RESERVED.contains(&lower.as_str()),
        "class" | "enum" | "interface" | "trait" => {
            FN_RESERVED.contains(&lower.as_str()) || CLASS_EXTRA.contains(&lower.as_str())
        }
        // A type alias erases before any backend (no PHP symbol), so the only hazard is the
        // contextual-keyword collision: a `type var` would clash with `var x = …` inference. The
        // built-in type words are already rejected by the alias arm (`cannot redefine built-in type`).
        _ => name == "var",
    }
}

/// DEC-202: a top-level class-position name colliding with a PHP BUILTIN class/interface —
/// the transpiled `class Exception {}` would be a fatal "cannot redeclare" against php's
/// always-loaded core (Core + standard + date + json + spl, verified vs php-8.5-cli).
/// Rejected loudly (`E-RESERVED-NAME`) rather than invisibly mangled: a USER-chosen top-level
/// symbol silently renaming would surprise on PHP interop and in stack traces (the ruling's
/// legibility + no-surprises rationale; enum VARIANTS keep the invisible mangle — they are
/// implementation detail, not API surface). PHP class names are case-insensitive.
// Single-sourced in `crate::php_names` (DEC-213): the DEC-202 reject (here) and the transpiler's
// enum-variant mangle (`transpile::names::php_variant_name`) consult the SAME builtin-class list,
// so the reject set and the mangle set can never drift apart (they did before DEC-213 — a variant
// named after an SPL/date/json builtin passed the reject but redeclared the class in transpiled PHP).
pub(super) use crate::php_names::is_php_builtin_class_name;

/// The five PHP-`is_*`-discriminable primitives usable as a `match` type-pattern head (`int i`,
/// `string s`, …) or an `is`/`instanceof` type-test target (`x is int`). Returns the bound `Ty`.
/// `decimal`/`bytes`/`html`/`attr` are excluded — they erase to a PHP `string`, so a runtime
/// type-test can't be byte-identical in the transpiled leg. Shared by the `match` cluster and the
/// `is`/`instanceof` checker + narrowing (Wave A).
pub(super) fn prim_pat_ty(name: &str) -> Option<Ty> {
    match name {
        "int" => Some(Ty::Int),
        "float" => Some(Ty::Float),
        "string" => Some(Ty::String),
        "bool" => Some(Ty::Bool),
        "null" => Some(Ty::Null),
        _ => None,
    }
}

/// The union members a type-pattern / type-test discriminates — for both a bare `Ty::Union` and an
/// `Optional` wrapping one (`(A | B)?`, the `T?` a `List.first`/`Map.get`/`.last` returns). Threading
/// through `Optional` keeps the `string`-erasure byte-identity guard (`E-MATCH-ERASED-AMBIG`) from
/// being bypassed when the union sits behind a `?`: a decimal/bytes/html/attr sibling erases to a PHP
/// string, so `is_string` in the transpiled leg can't tell it apart from a real `string` (G-1).
/// Returns `None` for any other scrutinee.
pub(super) fn union_members_of(scrut: &Ty) -> Option<&[Ty]> {
    match scrut {
        Ty::Union(members) => Some(members),
        Ty::Optional(inner) => match &**inner {
            Ty::Union(members) => Some(members),
            _ => None,
        },
        _ => None,
    }
}

/// A checker type that erases to a PHP `string` at transpile — so PHP's `is_string()` can't tell it
/// apart from a real `string`. A `string` type-pattern / `is string` test over a union holding one
/// of these is ambiguous (`E-MATCH-ERASED-AMBIG`).
pub(super) fn erases_to_php_string(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::String | Ty::Decimal | Ty::Bytes | Ty::Html | Ty::Attr
    )
}
