//! Namespaced native (built-in) function registry — the stdlib's runtime + type + transpile
//! surface, addressed by `(module, name)` (e.g. module `Core.Console`, name `println`). One entry
//! single-sources all four facets of a native, so the four backends cannot drift:
//!   * `params` / `ret` — the checker's signature for a call to this native;
//!   * `eval` — the runtime behavior, shared by the tree-walking interpreter *and* the VM (the
//!     structural parity guarantee, exactly like the value kernels: one impl, two callers);
//!   * `php` — the transpile-time PHP emission (a `core.*` native erases to PHP's flat builtins;
//!     the namespace is a compile-time organizing layer, decisions N-2/D-L9).
//!
//! The registry is the load-bearing target of `import Core.Output;` (M3 namespace reshape, Wave 1,
//! `docs/specs/2026-06-18-m3-namespace-system-design.md`). The former free global `println` is
//! retired in favor of `Core.Output.printLine`, and `Op::Print` in favor of
//! `Op::CallNative(index, argc)` indexing this table.

use crate::ast::Item;
use crate::types::Ty;
use crate::value::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

// Per-leaf stdlib modules: each owns its `*_natives()` builder + bodies; `build()` below is the sole
// ordering coordinator (the pinned-slot invariant). `Core.Console` stays here (slot 0, inlined).
mod bytes;
mod convert;
mod file;
mod fs;
mod html;
mod input;
mod list;
mod list_registry;
mod log;
mod map;
mod math;
mod option;
mod process;
mod random;
mod reflect;
mod result;
mod runtime;
mod set;
mod text;
mod text_format;
mod text_registry;
pub(crate) use text_format::parse_format_directive;
mod time;
mod validate;

pub use input::{set_stdin_disabled, set_stdin_override};
pub use process::{process_args_value, set_process_args};

/// One built-in function, addressed by `(module, name)`. See the module docs for the four facets.
pub struct NativeFn {
    /// Dotted module path the native lives under — e.g. `"Core.Output"`.
    pub module: &'static str,
    /// Bare function name — e.g. `"println"`.
    pub name: &'static str,
    /// Parameter types — the checker validates call arguments against these.
    pub params: Vec<Ty>,
    /// Return type.
    pub ret: Ty,
    /// Runtime behavior, shared by the interpreter and the VM (the structural parity guarantee —
    /// one body, two callers). See [`NativeEval`].
    pub eval: NativeEval,
    /// PHP emission: given the already-emitted PHP for each argument, return the PHP snippet this
    /// native erases to (decision N-2). For `Output.printLine`: `echo {a}, "\n"`.
    pub php: fn(&[String]) -> String,
    /// Whether this native is **deterministic** w.r.t. the program text (`true` for all but the
    /// ambient-environment natives — `Core.Process`/`Core.Environment`, whose result depends on the process,
    /// not the source). A program that calls an impure native is *quarantined* from the byte-identity
    /// differential (the PHP leg runs in a separate process whose argv/env need not match) and tested
    /// separately under a controlled environment — see `tests/process.rs`. Declared per-native here
    /// (not hardcoded in the harness) so the differential stays generic: it reads this flag via
    /// `program_uses_impure_native` (`docs/specs/2026-06-25-process-io-quarantine-seam-design.md`, Q1).
    pub pure: bool,
}

/// A deprecated stdlib symbol's replacement guidance (GA rock 3 / `W-DEPRECATED`). Kept in a side
/// table ([`deprecation_of`]) rather than a [`NativeFn`] field so flagging a symbol touches one place,
/// not all ~166 registry literals — and the common (non-deprecated) native pays nothing.
pub struct Deprecated {
    /// What to use instead — a fully-qualified symbol or a short phrase.
    pub replacement: &'static str,
    /// The version in which the symbol will be removed (per `SEMVER.md` / `docs/DEPRECATION.md`).
    pub removed_in: &'static str,
}

/// Whether the stdlib native `(module, name)` is deprecated, with its replacement guidance (the
/// policy lives in `docs/DEPRECATION.md`; a deprecated symbol keeps working but emits
/// `W-DEPRECATED` for ≥1 minor release before removal). First shipping entries: the `Core.Url`
/// module merged into the Uri module (DEC-279) — its four natives stay registered as a twin of
/// the `Core.Native.Uri` rows and warn here. A `#[cfg(test)]` sample additionally exercises the
/// lint wiring against a never-shipping fixture.
#[must_use]
pub fn deprecation_of(module: &str, name: &str) -> Option<Deprecated> {
    #[cfg(test)]
    if (module, name) == ("Core.Math", "abs") {
        // Test fixture only (never in a release build): proves the native-call → W-DEPRECATED wiring.
        return Some(Deprecated {
            replacement: "Math.absolute (sample — test fixture)",
            removed_in: "0.99.0",
        });
    }
    // DEC-279: `Core.Url` merged into the Uri module (`import Core.UriModule;` → `Uri.<fn>`).
    if module == "Core.Url" {
        let replacement = match name {
            "encodeForm" => "Core.UriModule — Uri.encodeForm",
            "encodeUriComponent" => "Core.UriModule — Uri.encodeComponent",
            "decodeForm" => "Core.UriModule — Uri.decodeForm",
            "decodeUriComponent" => "Core.UriModule — Uri.decodeComponent",
            _ => return None,
        };
        return Some(Deprecated {
            replacement,
            removed_in: "0.7.0",
        });
    }
    None
}

/// A backend's re-entrant closure invoker, handed to a [`NativeEval::HigherOrder`] body: given a
/// `Value::Closure` and its call arguments, run it on the calling backend and return its result (or
/// a fault as a plain `String`, the backend-shared contract). The interpreter wraps `call_closure`;
/// the VM wraps `call_closure_value` (a nested `run_until` over the shared `exec_op`).
pub type ClosureInvoker<'a> = dyn FnMut(&Value, Vec<Value>) -> Result<Value, String> + 'a;

/// A backend's re-entrant **capturing** closure invoker, handed to a [`NativeEval::Capturing`] body
/// (`Core.Output.capture`, DEC-220-S3): given a zero-arg `Value::Closure`, run it on the calling
/// backend and return the STRING it appended to the program output buffer while running (the closure's
/// `Output.*` output), diverted out of the buffer via `out.split_off(start)`. Unlike [`ClosureInvoker`]
/// (which returns the closure's *value*), this returns the *captured output*; the split_off happens in
/// each backend arm (interpreter/VM) — the one spot with both the output buffer and the closure runner
/// borrowed together — so the native body itself never touches `out`. A closure fault propagates as a
/// plain `String` (the backend-shared contract), exactly like the higher-order path.
pub type CapturingInvoker<'a> = dyn FnMut(&Value) -> Result<String, String> + 'a;

/// How a native computes its result (M-RT S7b-3). Most natives are [`Pure`](NativeEval::Pure): a
/// function of their argument values, threading the program output buffer so a side-effecting native
/// (`Output.printLine`) can append to it. A [`HigherOrder`](NativeEval::HigherOrder) native instead
/// needs to *call back* into the calling backend to invoke a `Value::Closure` argument
/// (`Core.List.map`/`filter`/`reduce`); the backend supplies the invoker so the one `eval` body
/// drives both the interpreter and the VM — exactly the parity discipline of the pure path. Both
/// variants are `fn` pointers, so the enum stays `Copy` (a `CallNative` dispatch reads it by value,
/// ending the registry borrow before the invoker captures the backend).
#[derive(Clone, Copy)]
pub enum NativeEval {
    /// `(args, out) -> result`. Arguments arrive in source order; `out` is the program's output
    /// buffer (ignored by pure natives, appended to by side-effecting ones).
    Pure(fn(&[Value], &mut String) -> Result<Value, String>),
    /// `(args, invoke) -> result`, where `invoke(closure, call_args)` executes a `Value::Closure`
    /// on the calling backend and returns its value. The native never touches the output buffer
    /// directly — any side effect happens inside the invoked closure.
    HigherOrder(fn(&[Value], &mut ClosureInvoker) -> Result<Value, String>),
    /// `(args, tables) -> result` (M-Reflect Tier-2): a native that needs the program's static class
    /// hierarchy, which a runtime `Value` doesn't carry (`Core.Reflection.interfaces`/`parents`/…). The
    /// backend supplies a [`ClassTables`] built once from the program. The transpiler emits the same
    /// table as a PHP static map, so the result is byte-identical by construction (both sides read the
    /// one `ClassTables`, never PHP's `class_*`/`get_class_methods` builtins with their own semantics).
    Reflective(fn(&[Value], &ClassTables) -> Result<Value, String>),
    /// `(args, capture) -> result` (DEC-220-S3, `Core.Output.capture`): a higher-order native that runs
    /// a zero-arg closure and needs the STRING it appended to the output buffer, not the closure's
    /// value. `capture(closure)` runs it on the calling backend and returns that captured output
    /// (`out.split_off(start)`); the native wraps it into the result `Value`. Distinct from
    /// [`HigherOrder`](NativeEval::HigherOrder) precisely by the invoker's return (captured output vs
    /// closure value) — the reason a new variant + [`CapturingInvoker`] exists. Transpiles to a gated
    /// `__phorj_capture($fn){ ob_start(); $fn(); return ob_get_clean(); }`, byte-identical for the
    /// happy path (a closure that prints and returns).
    Capturing(fn(&[Value], &mut CapturingInvoker) -> Result<Value, String>),
}

/// The program's static class hierarchy, precomputed once and shared by the interpreter, the VM
/// (a field on `BytecodeProgram`), and the transpiler (which emits it as a PHP static table). Each
/// list is **sorted** (the deterministic-order invariant), so a [`NativeEval::Reflective`] native and
/// its PHP erasure return identical bytes. Keyed by class name. (M-Reflect Tier-2,
/// `docs/specs/2026-06-25-core-reflect-design.md`.)
#[derive(Debug, Clone, Default)]
pub struct ClassTables {
    /// class → sorted transitive interface names ([`crate::ast::class_implements`]).
    pub interfaces: std::collections::BTreeMap<String, Vec<String>>,
    /// class → sorted transitive ancestor class names ([`crate::ast::class_supertypes`]).
    pub parents: std::collections::BTreeMap<String, Vec<String>>,
    /// class → sorted method names (own + inherited).
    pub methods: std::collections::BTreeMap<String, Vec<String>>,
    /// class → sorted declared field names (own + inherited).
    pub fields: std::collections::BTreeMap<String, Vec<String>>,
}

impl ClassTables {
    /// Build the tables once from a (fully expanded, post-loader) program. `interfaces`/`parents`
    /// reuse the shared `class_implements`/`class_supertypes` queries (already sorted + cycle-safe),
    /// so reflection can never disagree with `instanceof`. `methods`/`fields` are populated by their
    /// own slice.
    pub fn from_program(program: &crate::ast::Program) -> ClassTables {
        use crate::ast::{ClassMember, Item, Modifier};
        use std::collections::{BTreeMap, BTreeSet};
        let parents = crate::ast::class_supertypes(program);
        // Each class's OWN method / instance-field names (before inheritance).
        let mut own_methods: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let mut own_fields: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let promoted = |m: &[Modifier]| {
            m.iter().any(|x| {
                matches!(
                    x,
                    Modifier::Public | Modifier::Private | Modifier::Protected
                )
            })
        };
        for item in &program.items {
            if let Item::Class(c) = item {
                let methods = own_methods.entry(c.name.as_str()).or_default();
                let fields = own_fields.entry(c.name.as_str()).or_default();
                for m in &c.members {
                    match m {
                        ClassMember::Method(f) => {
                            methods.insert(f.name.clone());
                        }
                        // Instance fields only: a `static`/`const` member is class-level, a hook is
                        // virtual (no storage) — neither is an instance field.
                        ClassMember::Field {
                            modifiers, name, ..
                        } => {
                            if !modifiers.contains(&Modifier::Static)
                                && !modifiers.contains(&Modifier::Const)
                            {
                                fields.insert(name.clone());
                            }
                        }
                        // A constructor-promoted param (one carrying a visibility modifier) is a field.
                        ClassMember::Constructor { params, .. } => {
                            for p in params {
                                if promoted(&p.modifiers) {
                                    fields.insert(p.name.clone());
                                }
                            }
                        }
                        ClassMember::Hook { .. } => {}
                    }
                }
            }
        }
        // Flatten with inheritance: a class's set is its own ∪ every ancestor's own (sorted, deduped).
        fn flatten(
            program: &crate::ast::Program,
            parents: &BTreeMap<String, Vec<String>>,
            own: &BTreeMap<&str, BTreeSet<String>>,
        ) -> BTreeMap<String, Vec<String>> {
            let mut out = BTreeMap::new();
            for item in &program.items {
                if let Item::Class(c) = item {
                    let mut set: BTreeSet<String> =
                        own.get(c.name.as_str()).cloned().unwrap_or_default();
                    if let Some(anc) = parents.get(&c.name) {
                        for a in anc {
                            if let Some(o) = own.get(a.as_str()) {
                                set.extend(o.iter().cloned());
                            }
                        }
                    }
                    out.insert(c.name.clone(), set.into_iter().collect());
                }
            }
            out
        }
        let methods = flatten(program, &parents, &own_methods);
        let fields = flatten(program, &parents, &own_fields);
        ClassTables {
            interfaces: crate::ast::class_implements(program),
            parents,
            methods,
            fields,
        }
    }
}

/// Pinned registry slot for `Core.Output.printLine` — the migrated former `Op::Print`. The compiler
/// bakes `Op::CallNative(CONSOLE_PRINTLN, 1)`; [`build`] self-checks this slot so the constant can
/// never silently drift from the table.
pub const CONSOLE_PRINTLN: usize = 0;

/// `console.println(string)` — append the argument's display rendering plus a newline to the
/// program's output buffer. Shared verbatim by both backends (the former `interpreter::
/// builtin_println` / VM `Op::Print` body); the space-join over multiple args is dead generality
/// (the checker fixes the arity at one `string`) kept for a future variadic.
fn console_println(args: &[Value], out: &mut String) -> Result<Value, String> {
    let mut line = String::new();
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            line.push(' ');
        }
        match a.as_display() {
            Some(t) => line.push_str(&t),
            None => return Err(format!("println cannot print {}", a.type_name())),
        }
    }
    out.push_str(&line);
    out.push('\n');
    Ok(Value::Unit)
}

/// `Output.print` — like `println` but with no trailing newline (primitives P3). Space-joins multiple
/// args, same as `println`; transpiles to a bare PHP `echo`.
fn console_print(args: &[Value], out: &mut String) -> Result<Value, String> {
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        match a.as_display() {
            Some(t) => out.push_str(&t),
            None => return Err(format!("print cannot print {}", a.type_name())),
        }
    }
    Ok(Value::Unit)
}

/// `Output.capture(() -> void) -> string` (DEC-220-S3) — run the zero-arg closure and return the
/// string it echoed via `Output.*`, WITHOUT any of it reaching the program's own output. The
/// backend-supplied `capture` invoker diverts the closure's output (`out.split_off(start)`) and hands
/// it back; this body just wraps it into a `Value::Str`. Opt-in, explicit scope, no ambient state —
/// the import-gated primitive users wrap as `Response.html(Output.capture(() => { … }))`. Shared
/// verbatim by both backends (structural parity, like `List.map`); byte-identical across `run`/`runvm`
/// /PHP for the gated happy path (a printing, returning closure). A lambda cannot introduce a throw
/// here (a lambda can't declare `throws`, and a throwing lambda body is `E-THROW-UNDECLARED`), but a
/// NAMED throwing function may be passed by reference — on such a mid-capture throw `run`≡`runvm` still
/// holds (the throw propagates via the sentinel BEFORE `split_off`, so the partial output stays in
/// `out`), while the PHP `ob_get_clean` path is not a gated byte-identity claim (see `KNOWN_ISSUES.md`).
fn output_capture(args: &[Value], capture: &mut CapturingInvoker) -> Result<Value, String> {
    let captured = capture(&args[0])?;
    Ok(Value::Str(captured.into()))
}

/// Index helper for a native's PHP emission: the already-emitted PHP for argument `i`, or `""` if
/// absent (the checker guarantees arity before `php` is ever called). Keeps the `php` closures terse.
pub(crate) fn parg(args: &[String], i: usize) -> &str {
    args.get(i).map_or("", String::as_str)
}

/// Construct the native table once. Order is load-bearing: [`CONSOLE_PRINTLN`] pins slot 0; every
/// other native is resolved by `(module, name)` (or leaf+name) at compile time, so appended order is
/// free. Modules are grouped by `*_natives()` builders (one per `core.*` leaf).
fn build() -> Vec<NativeFn> {
    let mut registry = vec![NativeFn {
        module: "Core.Output",
        name: "printLine",
        params: vec![Ty::String],
        ret: Ty::Void,
        pure: true,
        eval: NativeEval::Pure(console_println),
        php: |args| {
            let a = args
                .first()
                .map_or_else(|| "\"\"".to_string(), String::clone);
            // B-2: echo's comma list, not concatenation — one universal rule for any arg type,
            // byte-identical output, and no forced interpolation-wrapping of a non-string value.
            format!(r#"echo {a}, "\n""#)
        },
    }];
    // `Output.print` — no trailing newline (primitives P3). Not slot-pinned; resolved by (module,name).
    registry.push(NativeFn {
        module: "Core.Output",
        name: "print",
        params: vec![Ty::String],
        ret: Ty::Void,
        pure: true,
        eval: NativeEval::Pure(console_print),
        php: |args| {
            let a = args
                .first()
                .map_or_else(|| "\"\"".to_string(), String::clone);
            format!("echo {a}")
        },
    });
    // `Output.capture(() -> void) -> string` (DEC-220-S3) — the explicit, import-gated capture
    // primitive. Reachable ONLY through the user's own `import Core.Output;` (no prelude imports
    // Core.Output), so it can never leak `Output.*` into a program that didn't ask for it (the "nothing
    // in the wind" invariant — the reason the ruled `Response.capture` prelude wrapper was rejected).
    registry.push(NativeFn {
        module: "Core.Output",
        name: "capture",
        params: vec![Ty::Function(vec![], Box::new(Ty::Void), vec![])],
        ret: Ty::String,
        pure: true,
        eval: NativeEval::Capturing(output_capture),
        // Gated `__phorj_capture($fn){ ob_start(); $fn(); return ob_get_clean(); }` (set the flag in
        // transpile/call.rs — a native's `php` closure has no `&mut self`). The closure's `Output.*`
        // (`echo …`) writes into the started buffer; `ob_get_clean()` returns exactly the captured bytes.
        php: |a| format!("__phorj_capture({})", parg(a, 0)),
    });
    registry.extend(math::math_natives());
    registry.extend(text_registry::text_natives());
    registry.extend(file::file_natives());
    registry.extend(bytes::bytes_natives());
    registry.extend(html::html_natives());
    registry.extend(list_registry::list_natives());
    registry.extend(map::map_natives());
    registry.extend(set::set_natives());
    registry.extend(convert::convert_natives());
    #[cfg(feature = "decimal")]
    registry.extend(crate::ext::decimal::decimal_natives());
    #[cfg(feature = "encoding")]
    registry.extend(crate::ext::encoding::encoding_natives());
    #[cfg(feature = "hash")]
    registry.extend(crate::ext::hash::hash_natives());
    #[cfg(feature = "ini")]
    registry.extend(crate::ext::ini::ini_natives());
    #[cfg(feature = "uri")]
    registry.extend(crate::ext::uri::uri_natives());
    #[cfg(feature = "uri")]
    registry.extend(crate::ext::uri::url_natives());
    #[cfg(feature = "path")]
    registry.extend(crate::ext::path::path_natives());
    registry.extend(validate::validate_natives());
    #[cfg(feature = "csv")]
    registry.extend(crate::ext::csv::csv_natives());
    registry.extend(random::random_natives());
    #[cfg(feature = "json")]
    registry.extend(crate::ext::json::json_natives());
    registry.extend(option::option_natives());
    registry.extend(result::result_natives());
    registry.extend(reflect::reflect_natives());
    registry.extend(process::process_natives());
    registry.extend(runtime::runtime_natives());
    registry.extend(log::log_natives());
    #[cfg(feature = "test")]
    registry.extend(crate::ext::test::test_natives());
    registry.extend(time::time_natives());
    #[cfg(feature = "cryptography")]
    registry.extend(crate::ext::cryptography::cryptography_natives());
    #[cfg(feature = "regex")]
    registry.extend(crate::ext::regex::regex_natives());
    #[cfg(feature = "database")]
    registry.extend(crate::ext::database::database_natives());
    #[cfg(feature = "mail")]
    registry.extend(crate::ext::mail::mail_natives());
    #[cfg(feature = "http-client")]
    registry.extend(crate::ext::http_client::http_client_natives());
    registry.extend(fs::fs_natives());
    #[cfg(feature = "session")]
    registry.extend(crate::ext::session::session_natives());
    registry.extend(input::input_natives());
    #[cfg(feature = "debug")]
    registry.extend(crate::ext::debug::debug_natives());
    // Pinned-slot invariant: the constant the compiler bakes into `Op::CallNative` must address the
    // entry it names. Cheap one-time check at first `registry()` access.
    assert_eq!(
        registry[CONSOLE_PRINTLN].module, "Core.Output",
        "CONSOLE_PRINTLN slot drifted"
    );
    assert_eq!(registry[CONSOLE_PRINTLN].name, "printLine");
    registry
}

/// The process-wide native table, built once. A `Vec<Ty>` isn't const-constructible, so this can't
/// be a plain `static` — `OnceLock` defers the allocation to first use (design §5).
pub fn registry() -> &'static [NativeFn] {
    static REG: OnceLock<Vec<NativeFn>> = OnceLock::new();
    REG.get_or_init(build)
}

/// Index of the native `(module, name)`, or `None`. Used by the checker and the transpiler, which
/// carry the import map and resolve the *exact* module a leaf qualifier was imported as.
pub fn index_of(module: &str, name: &str) -> Option<usize> {
    registry()
        .iter()
        .position(|n| n.module == module && n.name == name)
}

/// A native parameter's **default value** (M4 default parameters). A tiny literal enum kept separate
/// from the ~50 `NativeFn` literals (only `Core.Text.parseFloat` carries a default today, so a
/// per-native field would be near-pure churn): [`native_defaults`] returns the defaults for a native's
/// trailing parameters, and the checker converts each to an `Expr` literal when filling an omitted arg.
#[derive(Clone, Copy)]
pub enum NativeDefault {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
    Null,
}

/// The default values for a native's **trailing** parameters (`&[]` for every native but the few that
/// opt in). `native_defaults(m, n).len()` is how many trailing parameters are optional; the required
/// arity is `params.len() - that`. Single small lookup, so no churn across the registry literals.
pub fn native_defaults(module: &str, name: &str) -> &'static [NativeDefault] {
    match (module, name) {
        // `parseFloat(string, bool permissive = false)` — the permissive flag defaults to strict.
        ("Core.String", "parseFloat") => &[NativeDefault::Bool(false)],
        _ => &[],
    }
}

/// Index of a native by its module's *leaf* segment + name — e.g. leaf `"console"`, name
/// `"println"`. Used by the interpreter and compiler, which (unlike the transpiler) track variable
/// scope and resolve a member call `q.m(..)` locals-first: a qualifier `q` is only leaf-looked-up
/// once it is known *not* to be a bound variable, and the checker has already enforced that `q` was
/// imported and the native exists. Unambiguous while every stdlib leaf is distinct (Waves 1–2);
/// leaf collisions with user packages are resolved by import aliasing (design O-D, deferred).
pub fn index_of_by_leaf(leaf: &str, name: &str) -> Option<usize> {
    registry()
        .iter()
        .position(|n| n.name == name && n.module.rsplit('.').next() == Some(leaf))
}

/// Import-aware qualified-native resolution shared by the interpreter and the bytecode compiler
/// (the transpiler applies the same rule inline over its own import map). Resolves `q.name(...)`:
///
/// 1. through the program's import map (leaf or `as`-alias → dotted module) — required since
///    DEC-277: the `Core.Native.*` raw-native modules are imported UNDER AN ALIAS by the friendly
///    preludes (`import Core.Native.Database as NativeDatabase;`), which pure leaf-matching cannot
///    see;
/// 2. else via the legacy leaf convention — kept for checker-SYNTHESIZED qualified calls that
///    carry no import item (UFCS / bare-fn-import / as-cast rewrites emit `<leaf>.name(...)`) —
///    EXCEPT that a `Core.Native.*` module is never leaf-matched: its leaf equals a prelude
///    class name (`Core.Native.Uri` vs `class Uri`, DEC-277/278), so an import-less
///    `Uri.parse(...)` must keep resolving as the class static the checker blessed, and reaching
///    the natives without an import binding would be "in the wind" anyway. (A user class merely
///    NAMED like a non-Native module leaf must NOT block this fallback — the checker resolves a
///    member-imported bare call to `<leaf>.name(...)` regardless of such a class; guarding on
///    declared class names here made the VM reject what the checker accepted.)
pub fn index_of_qualified(imports: &HashMap<String, String>, q: &str, name: &str) -> Option<usize> {
    if let Some(idx) = imports.get(q).and_then(|m| index_of(m, name)) {
        return Some(idx);
    }
    index_of_by_leaf(q, name).filter(|&idx| !registry()[idx].module.starts_with("Core.Native."))
}

/// Build the active import map (leaf qualifier → full dotted module path) from a program's items:
/// `import Core.Output;` binds the call-site qualifier `console` to module `Core.Console`. Carried
/// by the checker (import-required + shadowing enforcement) and the transpiler (which has no
/// variable-scope tracking to tell a qualifier from a value).
pub fn import_map(items: &[Item]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for item in items {
        if let Item::Import { path, alias, .. } = item {
            // The bound qualifier is the alias when present (`import a.b as c;` ⇒ `c`), else the
            // path's last segment (M5 S2c). A terminal `import type …;` binds a *type* name, not a
            // call qualifier, so it is excluded from this (call-site) map.
            let qualifier = alias.clone().or_else(|| path.last().cloned());
            if let Some(q) = qualifier {
                map.insert(q, path.join("."));
            }
            // Import-redesign S2: a member-import of a multi-type injected Core module
            // (`import Core.Time.Instant`, `import Core.Http.Router`, `import Core.Decimal.RoundingMode`)
            // also makes that MODULE's native qualifier resolvable (`Time.`, `Http.`, `Decimal.`), so the
            // injected prelude's own internal module-native calls type-check — e.g. `Instant.now()` calls
            // `Time.nowMilliseconds()`, hidden from the user. User code that writes such a qualifier
            // WITHOUT a whole-module import is rejected by the pre-injection enforcement pass (S2 stage C),
            // so this implicit binding never lets user code reach the module natives "for free".
            if alias.is_none()
                && path.len() == 3
                && path[0] == "Core"
                && matches!(path[1].as_str(), "Http" | "Time" | "Decimal")
            {
                map.entry(path[1].clone())
                    .or_insert_with(|| format!("Core.{}", path[1]));
            }
        }
    }
    map
}

#[cfg(test)]
mod tests;
