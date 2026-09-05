//! Phorj → PHP transpiler: walks the untyped AST and emits runnable PHP 8.x. Entry point: [`emit`].
use crate::ast::*;
use crate::dispatch::ParamKind;
use std::collections::{BTreeSet, HashMap, HashSet};

// cohesion split (M-Decomp): program/types/stmt/expr/call/matches clusters + the driver / state /
// runtime-helper gates / string-escape / member-modifier / kind-mapper clusters. This root keeps
// only the shared type definitions (`OpKind`, `Transpiler`, `Origin`, `MatchTarget`) and the wiring.
mod attributes;
mod call;
mod charset_php;
mod classes;
mod classes_synth;
mod collect;
mod collisions;
pub mod db_php;
mod driver;
mod enums;
mod escapes;
mod expr;
mod fold_php;
mod fs_php;
mod functions;
mod gates;
mod helper_buckets;
mod kinds;
mod lambda_stmt;
mod log_php;
mod magic_php;
mod matches;
mod modifiers;
mod names;
mod process_php;
mod program_emit;
mod runtime_php;
mod runtime_php_http;
mod runtime_php_regex;
mod runtime_tables;
pub mod split;
mod state;
mod stmt;
mod types;
mod wordwrap_php;

/// The public transpiler entry point (defined in `driver`) re-exported at `crate::transpile::emit`.
pub use driver::{emit, emit_with_source};
// `decomposed_classes` (in `driver`) is also called from `split.rs`; re-glob it so that module's
// `use super::*` keeps reaching it.
use self::driver::decomposed_classes;
use self::escapes::*;
use self::kinds::*;
use self::modifiers::*;
use self::names::*;
use gates::HelperGates;

/// The Unicode White_Space PCRE character class used by every emitted PHP trim helper
/// (`__phorj_text_trim`/`_start`/`_end` in `runtime_php.rs` AND `__phorj_http_trim` in
/// `runtime_php_http.rs`). SINGLE SOURCE OF TRUTH (Invariant 4): exactly `char::is_whitespace`'s set,
/// verified byte-identical to Rust `str::trim` across the multibyte + form-feed edges (UA-1.1) — NOT
/// PHP's ASCII-ish `trim`. Both trim families derive from this one literal so trim parity can't drift.
/// Lives here (module root, reached via `super::PHP_TRIM_WS`) so neither runtime file grows past its cap.
const PHP_TRIM_WS: &str = r"[\x{09}-\x{0D}\x{20}\x{85}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}]";

/// A statically-resolved operand "kind" used by the transpiler's T6 specialization to pick a native
/// PHP operator over a runtime helper. Deliberately scalar-only — the cases where PHP's loose
/// semantics diverge from Phorj's (`+` concat-vs-add, `/` int-vs-float, interpolation display).
/// Anything the resolver cannot pin down is [`OpKind::Other`], which routes to the existing helper
/// (the safe fallback), so a wrong guess can never happen — only "known" or "fall back".
/// The PHP prologue every emitted file opens with — **DEC-401**.
///
/// `declare(strict_types=1);` is emitted in EVERY transpiled file. The ruling's reason: the PHP leg must
/// enforce at its boundary what phorj enforces everywhere else, or "statically typed" is a promise the
/// output quietly drops. Without it, a HOST PHP caller invoking an emitted `function helper(int $x)` with
/// `"5"` gets a silent coercion where phorj's own checker would never have admitted the call; with it,
/// that becomes a `TypeError` at the boundary.
///
/// **Byte-identity is unaffected for phorj-only programs**, which is why this is safe: the checker has
/// already proven every type inside the program, so no call the emitter generates can be one that
/// coercion was papering over. It changes only what happens when PHP code *outside* the program calls in
/// with the wrong type. [Verified by the differential suite over every `examples/**/*.phg`.]
///
/// SINGLE SOURCE for both emit paths (flat and namespaced) so they cannot drift — and it must stay the
/// first *statement* in the file, which is why `build_php`'s generated-file marker is inserted as a
/// COMMENT after `<?php` (comments are not statements, so they may precede a `declare`).
pub(crate) const PHP_PROLOGUE: &str = "<?php\ndeclare(strict_types=1);\n";

#[derive(Clone, PartialEq, Eq, Debug)]
enum OpKind {
    Str,
    Int,
    Float,
    /// `decimal` (M-NUM S1). A decimal operand routes `+ - *` to the `__phorj_dec_*` BCMath helpers
    /// (exact + i128-bounds-checked), and a decimal value erases to a PHP `string` for display.
    Decimal,
    Bool,
    /// A value of a user-defined class/enum/interface, carrying its name so a field read resolves
    /// through `class_field_kinds` (T6b). Never an arithmetic/display operand itself.
    Class(String),
    /// `List<E>` carrying its element kind, so `xs[i]` resolves to `E` (T6d) — `xs[i] + 1` / `"{xs[i]}"`.
    List(Box<OpKind>),
    /// `Map<K, V>` carrying key+value kinds, so `m[k]` resolves to `V` (T6d).
    Map(Box<OpKind>, Box<OpKind>),
    Other,
}

struct Transpiler {
    funcs: HashSet<String>,
    /// Foreign PHP free functions declared via `declare function …;` (M8.5 interop). They are **not**
    /// emitted as PHP definitions (PHP already has them) and a call to one is emitted as the global form
    /// `\name(…)` (so it resolves to the PHP builtin even inside a namespace block). Kept separate from
    /// `funcs` so the emit loop skips them and `emit_call` routes them to the `\`-prefixed form.
    foreign_fns: HashSet<String>,
    /// Foreign PHP classes declared via `declare class … { … }` (M8.5 S2). Also kept in `classes` so
    /// construction and member-call resolution work; this set additionally routes construction to
    /// `new \Name(…)` and static calls to `\Name::s(…)` (global PHP), and suppresses the class
    /// definition (PHP already has it). Instance method/field access (`$o->m`, `$o->f`) needs no name.
    foreign_classes: HashSet<String>,
    classes: HashSet<String>,
    /// `(class, NAME)` pairs that name a `const` class constant (Feature A), inheritance/traits already
    /// flattened (the shared [`crate::ast::class_consts`] table). A `ClassName.NAME` access whose pair
    /// is in this set emits as `ClassName::NAME` (no `$`) — checked before the static-field `::$name`
    /// path. PHP resolves an inherited `Sub::MAX` itself, so only the keys are needed.
    consts: HashSet<(String, String)>,
    variants: HashSet<String>,
    /// DEC-320 split emission: `Some` = only these top-level items emit; `None` = classic emit.
    keep: Option<HashSet<String>>,
    /// DEC-320: the running split pass (gates bootstrap / statics / trailing helpers).
    split: split::SplitPass,
    /// DEC-329.3: bare variant → declaring enum (last wins) — FALLBACK only (`qualify_variants`).
    variant_owner: HashMap<String, String>,
    /// DEC-302: declared enum names — routes `Enum.cases()`/`from(x)`/`tryFrom(x)` to a PHP static
    /// call (`Enum::method(...)`) rather than the instance-member fallback (`$Enum->method(...)`).
    enums: HashSet<String>,
    /// `(enum, variant) → payload field names` — keyed precisely since DEC-329.3.
    variant_fields: HashMap<(String, String), Vec<String>>,
    /// The ORIGINAL phorj source, when the caller has it (DEC-419). Only used to recover `/** … */`
    /// doc comments and re-emit them as PHP docblocks — comments are not AST nodes, so the source text
    /// is the only channel. `None` for callers that transpile a `Program` they did not read from a file
    /// (the benchmark path); the output then simply carries no docblocks, which is what it did before.
    src: Option<String>,
    out: String,
    indent: usize,
    locals: Vec<HashSet<String>>,
    /// Scoped operand-type environment (T6), parallel to `locals` (pushed/popped together). Maps a
    /// local/param/loop-var name to its scalar [`OpKind`] **where statically known** — so `+`, `/`,
    /// `%`, and interpolation can emit native PHP operators (`.`/`+`/`intdiv`/`fmod`/direct casts)
    /// instead of the `__phorj_add`/`_div`/`_rem`/`_str` runtime helpers. A name absent here resolves
    /// to [`OpKind::Other`] → the helper is emitted as a safe fallback (never a byte-identity risk).
    local_kinds: Vec<HashMap<String, OpKind>>,
    cur_class_fields: Option<HashSet<String>>,
    /// The class whose members are being emitted, for `this` operand-kind resolution (T6b). Set
    /// around `emit_class_members`, restored after.
    cur_class: Option<String>,
    /// B2 — active trait-alias map for `parent.m(…)` / `parent(A).m(…)` calls inside an **MI
    /// class** or **decomposed trait body** (PHP has no native `parent::`/`A::` target there).
    /// `Some` only while emitting such a body; keyed `(ancestor-as-written, method)`, valued by
    /// the `private` trait alias the `use` block declares. A miss while `Some` = a transitive MI
    /// jump — not yet lowerable, surfaced as a transpile error rather than invalid PHP.
    parent_aliases: Option<std::collections::BTreeMap<(Option<String>, String), String>>,
    /// `class → (field/hook/promoted-ctor-param name → OpKind)` — operand kinds of a class's *own*
    /// members (T6b). Field reads (`p.x`, `this.x`) resolve through here + the parent chain
    /// (`class_parents`), so `p.x + 1` / `"{p.x}"` emit native PHP instead of a runtime helper.
    class_field_kinds: HashMap<String, HashMap<String, OpKind>>,
    /// `class → direct parents` (`extends`), for inherited-field kind lookup (T6b).
    class_parents: HashMap<String, Vec<String>>,
    /// `(enum, variant) → payload OpKinds` (positional) — a variant-payload match binding
    /// resolves its kind for operand specialization (T6b; DEC-329.3 keying).
    variant_field_kinds: HashMap<(String, String), Vec<OpKind>>,
    /// `free-function name → return OpKind` (T6c), so a call result (`bulk(x)`, `"{f(x)}"`) resolves
    /// to a native operand. Overloads with differing return kinds collapse to `Other` (the fallback).
    fn_ret_kinds: HashMap<String, OpKind>,
    /// `(class, method) → return OpKind` (T6c), with `extends`-chain lookup, so a method-call result
    /// (`p.price()`, `c.get() + 1`) resolves. Differing overloads collapse to `Other`.
    method_ret_kinds: HashMap<(String, String), OpKind>,
    /// Active import map (leaf qualifier → full dotted module path) — how a namespaced native call
    /// `console.println(x)` is distinguished from a method call on a value (M3 Wave 1). The
    /// transpiler tracks no variable scope, so unlike the interpreter/compiler it cannot use a
    /// locals-first heuristic; the import map is the authority.
    imports: HashMap<String, String>,
    /// The once-per-file runtime-helper emission gates (the `uses_*` flags), grouped into
    /// [`HelperGates`] (M-Decomp: keeps this struct + the module root under the file-size cap). Each
    /// flag is set when its helper is first emitted and read by the runtime-helper emitters to define
    /// that helper exactly once per file — accessed throughout as `self.gates.uses_*`.
    gates: HelperGates,
    /// True when the program carries mangled (`\`-bearing) names — a multi-package project (M5 S2c).
    /// Switches emission from the flat single-package form to one `namespace …{}` brace-block per
    /// package + a nameless bootstrap block, and forces fully-qualified (leading-`\`) call emission.
    namespaced: bool,
    /// DEC-437: the phorj classes declared `#[Attribute]`, as (canonical dotted path, PHP class name),
    /// in DECLARATION order. Seeded by `collect` so attribute re-emission needs no `Program` threaded
    /// through the function emitter, and deterministic for free (a `Vec`, not a `HashMap` — Invariant 10).
    attr_classes: Vec<(String, String)>,
    /// The flattened `class_implements` oracle (M-RT overloading): used to order an overload set's
    /// PHP dispatch branches most-specific-first (subtypes before supertypes), so the emitted
    /// `if`-chain selects the same body the backends' `select_overload` does. Built once in `emit`.
    class_implements: std::collections::BTreeMap<String, Vec<String>>,
    /// Static class hierarchy for the reflection enumeration natives — emitted as the PHP
    /// `__phorj_reflect_of` static table when `uses_reflect_tables` is set, byte-identical to the
    /// `ClassTables` the Rust backends read (M-Reflect Tier-2).
    class_tables: crate::native::ClassTables,
    /// `(php variant class, phorj enum name, phorj variant name)` rows collected by `emit_enum`,
    /// consumed by the `__PHORJ_DEBUG_ENUMS` table when `uses_debug_render`.
    debug_enum_rows: Vec<(String, String, String)>,
    /// The `namespace … {` block currently being emitted (project/namespaced mode), `None` on the
    /// flat single-file path. Read by `emit_enum` to key its `__phorj_debug_enums` row by the FQN
    /// PHP's `get_class` will actually return (`Acme\Color_Green`), which is what the lookup compares.
    current_ns: Option<String>,
    /// Classes lowering to the **interface + trait** decomposition (M-RT S6b): every transitive
    /// ancestor of a multi-parent (`extends A, B`) class — PHP has no MI, so each ancestor needs
    /// an `I<name>` interface + `T<name>` trait + a concrete `class <name>`. Built once in `emit`;
    /// classes outside the set lower plainly. The multi-parent classes themselves are emitted via
    /// `emit_multi_class`, not listed here.
    decomposed: BTreeSet<String>,
    /// Monotonic counter for the hidden `$__phorj_d{N}` temporary that a let-destructuring spills its
    /// initializer into (Phase 1 slice 5). The name never collides with a user local (`$__phorj_` is
    /// not a writable Phorj identifier) and the value is immaterial to stdout, so any deterministic
    /// sequence is byte-identity-safe.
    tmp: usize,
}

/// A resolved method origin: `(declaring class, method name)` — mirrors `ast::class_method_origins`.
type Origin = (String, String);

/// Where a `match` expression's arm values flow: a `return` or an assignment to `$name`.
enum MatchTarget {
    Return,
    Assign(String),
    /// A statement-position `match` (`match (x) { … };` — arms run for effect, no value captured).
    /// Always lowered to the `instanceof`/`===` if-chain, NEVER a native `match (true)` expression:
    /// a void arm body like `Output.printLine(…)` emits PHP `echo`, which is a STATEMENT — legal
    /// inside an if-chain block, a parse error inside a `match` expression arm. (Pre-DEC-253 this
    /// position fell through to the expression emitter and produced unparseable PHP — caught while
    /// building the nullable-union example.)
    Discard,
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_attributes;

#[cfg(test)]
mod tests_docs;
#[cfg(test)]
mod tests_enums;

impl Transpiler {
    /// The PHP form of a unary negation when a bare `-$x` would be WRONG, or `None` when the native
    /// operator is correct. Both cases are here so that adding a numeric kind means revisiting ONE
    /// place rather than remembering a second `if` in the expression emitter.
    ///
    /// * **`int` (DEC-255)** — negating `i64::MIN` faults in phorj, while bare PHP `-$x` silently
    ///   promotes to float. Routed through `__phorj_checked_neg`.
    /// * **`decimal` (DEC-401)** — a decimal erases to a PHP *string*, so `-$x` is PHP ARITHMETIC: it
    ///   coerces the string to a float and the exact value is gone. Under `declare(strict_types=1)` that
    ///   float then reaches `strpos()` inside `__phorj_dec_scale` and raises a `TypeError`; BEFORE
    ///   strict_types it silently stringified through PHP's float formatting — i.e. the PHP leg was
    ///   performing a conversion the two Rust legs never did, a latent byte-identity hazard that
    ///   coercion had been hiding. Negation IS subtraction from zero, so it reuses the existing exact
    ///   helper (`max(scales)` plus the same i128 bounds check) instead of adding a new one, which
    ///   reproduces the Rust kernel including `-0.00d` staying `0.00` rather than becoming `-0.00`.
    ///   [Verified against the tree-walker oracle: `-2.345|0.00|1.5|2.345`.]
    fn neg_via_helper(&mut self, operand: &Expr, inner: &str) -> Option<String> {
        let bs = if self.namespaced { "\\" } else { "" };
        match self.expr_kind(operand) {
            OpKind::Int => {
                self.gates.uses_checked_arith = true;
                Some(format!("{bs}__phorj_checked_neg({inner})"))
            }
            OpKind::Decimal => {
                self.gates.uses_dec_sub = true;
                Some(format!("{bs}__phorj_dec_sub(\"0\", {inner})"))
            }
            _ => None,
        }
    }
}
