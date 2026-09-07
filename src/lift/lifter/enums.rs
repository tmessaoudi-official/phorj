//! PHP-lift — enums (scout forcing function, lane L1, 2026-09-07).
//!
//! Three things a PHP enum does that the lifter got wrong or refused outright, all of them on the
//! path to scout's `Rent/Core/Tenure.php` and so to the tenure-classifier depth target:
//!
//! 1. **`Tenure::PLAI` is an enum CASE, not a class constant** — and PHP spells the two identically,
//!    so telling them apart needs to know which names are enums. It used to lift as `Tenure::PLAI`,
//!    which is not phorj syntax at all: the draft "lifted" and then failed `phg check` with
//!    `E-UNKNOWN-IDENT`. A payload-less variant is CONSTRUCTED, like everything else (Invariant 12,
//!    mandatory `new`) — `new PLAI()`.
//! 2. **`self::CASE` inside an enum method** refused with *"`self` outside a class body"*, because
//!    `parse_enum` never set `current_class`. Fixed in the parser; the rule is R-7's, unchanged.
//! 3. **A PHP enum METHOD has no phorj form** — phorj enums carry no methods, by design
//!    (`MASTER-PLAN.md:2413`), and UFCS is the substitute. **DEC-509**: a method lowers to a FREE
//!    FUNCTION taking the enum as its first parameter, and UFCS resolves `$t->isExcluded()` to
//!    `t.isExcluded()` with the call site unchanged. Verified byte-identical on interpreter, VM and
//!    transpiled PHP before the ruling was taken.
//!
//! The registry is thread-local for the same reason `LIFTED_NATIVE_MODULES` is: the lifter is
//! stateless free functions and a lift runs on one thread. It is cleared at the start of every
//! `lift`, so runs never leak into each other, and the PROJECT lift seeds it with every enum in the
//! tree before any file is lifted — a `Tenure::PLAI` in one file naming an enum declared in another
//! is the common case, not the exception.

use super::*;

thread_local! {
    /// Every enum in scope for the file being lifted right now, mapped to the method names that
    /// DEC-509 lowers to free functions. The methods matter at call sites: `Tenure::fallback()` is a
    /// call to a lowered function, while `Tenure::from(…)` is the backed-enum builtin and must keep
    /// naming its enum. Only a method the enum DECLARES is lowered.
    static ENUM_NAMES: std::cell::RefCell<std::collections::BTreeMap<String, HashSet<String>>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
    /// Enums declared ELSEWHERE in the project. The directory lift fills this once, before any file
    /// is lifted, because `Tenure::PLAI` in one file naming an enum declared in another is the
    /// common case; a single-file lift leaves it empty and sees only its own.
    static ENUM_BASELINE: std::cell::RefCell<std::collections::BTreeMap<String, HashSet<String>>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
    /// The receiver parameter a lowered enum method's `$this` resolves to, while that body is being
    /// lifted. `None` everywhere else, so an ordinary method's `$this` is untouched.
    static ENUM_RECEIVER: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The project lift's whole-tree seed. Replaces the baseline; pass an empty set to clear it, which
/// is what keeps a directory lift from leaking into a later single-file lift on the same thread.
pub(in crate::lift) fn set_project_enum_names(names: EnumSymbols) {
    ENUM_BASELINE.with(|e| *e.borrow_mut() = names);
}

/// Begin one file: the visible set is the project baseline plus this file's own enums. Called at the
/// top of every `lift`, so no run can inherit a previous file's private names.
pub(in crate::lift) fn begin_file(own: EnumSymbols) {
    let mut names = ENUM_BASELINE.with(|e| e.borrow().clone());
    names.extend(own);
    ENUM_NAMES.with(|e| *e.borrow_mut() = names);
}

/// Enum name → the method names it declares, for one parsed PHP file. Unqualified: the lifter
/// compares against the name as WRITTEN at the use site, which is what `use` statements have
/// already resolved to.
pub(in crate::lift) type EnumSymbols = std::collections::BTreeMap<String, HashSet<String>>;

pub(in crate::lift) fn enum_names_of(prog: &php::PhpProgram) -> EnumSymbols {
    prog.items
        .iter()
        .filter_map(|i| match i {
            php::PhpItem::Enum(e) => Some((
                e.name.clone(),
                e.methods.iter().map(|m| m.name.clone()).collect(),
            )),
            _ => None,
        })
        .collect()
}

/// A qualified `\App\Domain\Tenure::PLAI` arrives with its root marker already dropped by the
/// parser's name resolution, but a namespaced write can still carry segments — compare on the LEAF,
/// which is what the phorj draft will name.
fn leaf_of(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}

pub(super) fn is_enum(name: &str) -> bool {
    ENUM_NAMES.with(|e| e.borrow().contains_key(leaf_of(name)))
}

/// Does `class::method` name a method the enum DECLARES — i.e. one DEC-509 lowered to a free
/// function? `Tenure::fallback()` does; `Tenure::from("X")` does not (it is the backed-enum builtin,
/// which still names its enum), and neither does anything on an ordinary class.
pub(super) fn is_lowered_method(class: &str, method: &str) -> bool {
    ENUM_NAMES.with(|e| {
        e.borrow()
            .get(leaf_of(class))
            .is_some_and(|ms| ms.contains(method))
    })
}

/// The name `$this` lifts to right now, if a lowered enum-method body is being lifted.
pub(super) fn receiver() -> Option<String> {
    ENUM_RECEIVER.with(|r| r.borrow().clone())
}

fn with_receiver<T>(name: &str, f: impl FnOnce() -> T) -> T {
    ENUM_RECEIVER.with(|r| *r.borrow_mut() = Some(name.to_string()));
    let out = f();
    ENUM_RECEIVER.with(|r| *r.borrow_mut() = None);
    out
}

/// `Tenure` → `tenure`. Deterministic and readable; a name is never invented beyond lowering the
/// first character, and a clash with a real parameter is REFUSED rather than worked around.
fn receiver_name(enum_name: &str) -> String {
    let mut cs = enum_name.chars();
    match cs.next() {
        Some(c) => c.to_lowercase().collect::<String>() + cs.as_str(),
        None => String::from("it"),
    }
}

/// DEC-509 — lower every method of `e` to a free function.
///
/// An INSTANCE method gains a leading receiver parameter typed as the enum, and its `$this` becomes
/// that parameter. A STATIC method has no `$this`, so it lowers to a plain free function of the same
/// arity — PHP's `Tenure::fromLabel($s)` and phorj's `fromLabel(s)` mean the same thing.
///
/// `lifted_names` accumulates across the whole file so a second enum declaring the same method name
/// is caught: two free functions of one name in one package do not compile, and the lifter does not
/// invent a disambiguating name (DEC-166). It refuses and says which name collided.
pub(super) fn lower_methods(
    l: &mut Lifter,
    e: &php::PhpEnum,
    lifted_names: &mut HashSet<String>,
) -> Result<Vec<FunctionDecl>, String> {
    let mut out = Vec::new();
    for m in &e.methods {
        if m.body.is_none() {
            return Err(format!(
                "lift: enum `{}` method `{}` has no body (Tier-2)",
                e.name, m.name
            ));
        }
        if !lifted_names.insert(m.name.clone()) {
            return Err(format!(
                "lift: enum method `{}` lowers to a free function whose name is already taken in \
                 this file — two enums declaring `{}` would emit two functions of one name \
                 (DEC-509). Rename one in the PHP and re-run.",
                m.name, m.name
            ));
        }
        let recv = receiver_name(&e.name);
        let mut params = Vec::new();
        if !m.is_static {
            if m.params.iter().any(|p| p.name == recv) {
                return Err(format!(
                    "lift: enum `{}` method `{}` already has a parameter named `{}`, which is the \
                     receiver name the lowering needs (DEC-509). Rename that parameter in the PHP.",
                    e.name, m.name, recv
                ));
            }
            params.push(php::PhpParam {
                ty: Some(php::PhpType::Named(e.name.clone())),
                name: recv.clone(),
                default: None,
                promotion: None,
                is_readonly: false,
            });
        }
        params.extend(m.params.iter().cloned());
        let f = php::PhpFunction {
            attrs: Vec::new(),
            name: m.name.clone(),
            params,
            ret: m.ret.clone(),
            body: m.body.clone().unwrap_or_default(),
            line: m.line,
        };
        let lifted = if m.is_static {
            l.lift_function(&f)?
        } else {
            with_receiver(&recv, || l.lift_function(&f))?
        };
        out.push(lifted);
    }
    Ok(out)
}
