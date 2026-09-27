//! Rows 4p and 4p4 (2026-09-27) — three more DECLARATIONS a shape can come from, beside a parameter
//! and a `@var` local (DEC-504, DEC-515).
//!
//! * A PROPERTY. `foreach ($this->bands as $band)` over a property declared
//!   `array<string, array{max: int, …}>` binds `$band` to that element shape, so `$band['max']` lifts
//!   to `band.max`. Class-scoped: set when a class starts lifting and restored when it ends, and kept
//!   OUT of the per-function clear — a method does not redeclare its class's properties.
//! * The BUILDER local of a declared return. `$hits = []; …; return $hits;` is already constructed
//!   as the return type (Lane R-6, `seed.rs`); its element shape is registered before the body is
//!   lifted, so a keyed literal written into it (`$hits[$at] = ['tenure' => …]`) becomes the declared
//!   tuple the way a returned literal does. The detection mirrors that seeding exactly — a top-level
//!   `$x = []` and a top-level `return $x;` — so no local is shaped that is not also typed.
//! * A statically resolved CALLEE's declared return (row 4p4, DEC-550). `foreach ($this->hits() as
//!   $h)` under `@return list<array{tag: string}>` binds `$h` to `(tag: string)`. Resolved on the PHP
//!   side, where `rows()` and `$rows()` are still different things: `$this->m()`, `self::m()`,
//!   `static::m()` and `Own::m()` against the ENCLOSING class's methods, and a plain `f()` against the
//!   functions this FILE declares. Not resolved, so binding nothing: an inherited or `parent::`
//!   method, another object's method, and a function declared in another file.
//!
//! Every one reads a type the program WROTE; none looks at a callee's body or a literal's elements
//! (DEC-166).

use super::*;
use std::collections::HashMap;

/// What the class being lifted declares: its name, its properties' types, its methods' returns.
#[derive(Default)]
struct ClassShapes {
    name: String,
    props: HashMap<String, Type>,
    methods: HashMap<String, Type>,
}

thread_local! {
    static CLASS: std::cell::RefCell<ClassShapes> = std::cell::RefCell::new(ClassShapes::default());
    /// Function name → its declared return, for the file being lifted (row 4p4).
    static FN_RETS: std::cell::RefCell<HashMap<String, Type>> =
        std::cell::RefCell::new(HashMap::new());
}

/// The enclosing class's declarations, put back when this class's lift ends — on the error path too.
pub(in crate::lift::lifter) struct PropScope(Option<ClassShapes>);

impl Drop for PropScope {
    fn drop(&mut self) {
        if let Some(prev) = self.0.take() {
            CLASS.with(|m| *m.borrow_mut() = prev);
        }
    }
}

/// Record `c`'s instance properties — declared ones, and the constructor's promoted parameters — and
/// its methods' declared returns.
///
/// A type that does not lift is skipped, not reported: the member's own lift reports it and fails
/// the class, so the registry never answers for a class that lifts.
pub(in crate::lift::lifter) fn enter_class_props(c: &php::PhpClass) -> PropScope {
    let mut shapes = ClassShapes {
        name: c.name.clone(),
        ..ClassShapes::default()
    };
    for m in &c.members {
        match m {
            php::PhpMember::Prop {
                is_static: false,
                ty: Some(ty),
                name,
                ..
            } => {
                if let Ok(t) = lift_type(ty) {
                    shapes.props.insert(name.clone(), t);
                }
            }
            php::PhpMember::Method(me) => {
                if me.name == "__construct" {
                    for p in me.params.iter().filter(|p| p.promotion.is_some()) {
                        if let Some(Ok(t)) = p.ty.as_ref().map(lift_type) {
                            shapes.props.insert(p.name.clone(), t);
                        }
                    }
                }
                if let Some(Ok(t)) = me.ret.as_ref().map(lift_type) {
                    shapes.methods.insert(me.name.clone(), t);
                }
            }
            _ => {}
        }
    }
    PropScope(Some(
        CLASS.with(|m| std::mem::replace(&mut *m.borrow_mut(), shapes)),
    ))
}

/// Start one file's lift: the declared returns of the functions it declares (row 4p4).
pub(in crate::lift::lifter) fn begin_file_fns(prog: &php::PhpProgram) {
    let rets = prog
        .items
        .iter()
        .filter_map(|i| match i {
            php::PhpItem::Function(f) => Some((f.name.clone(), lift_type(f.ret.as_ref()?).ok()?)),
            _ => None,
        })
        .collect();
    FN_RETS.with(|m| *m.borrow_mut() = rets);
}

/// The field labels of property `name`'s ELEMENTS — what a binder over `$this->name` inherits.
pub(super) fn prop_elem_labels(name: &str) -> Option<Vec<String>> {
    CLASS.with(|m| m.borrow().props.get(name).and_then(element_labels))
}

/// The field labels of the ELEMENTS a statically resolved call returns (row 4p4, see the module doc).
pub(super) fn call_elem_labels(call: &php::PhpExpr) -> Option<Vec<String>> {
    match call {
        php::PhpExpr::MethodCall {
            recv,
            name,
            nullsafe: false,
            ..
        } if matches!(&**recv, php::PhpExpr::Var(v) if v == "this") => method_elem_labels(name),
        php::PhpExpr::StaticCall { class, name, .. } => {
            let own = CLASS.with(|m| {
                let own = &m.borrow().name;
                !own.is_empty()
                    && (class == "self"
                        || class == "static"
                        || class.rsplit('\\').next() == Some(own.as_str()))
            });
            if own {
                method_elem_labels(name)
            } else {
                None
            }
        }
        php::PhpExpr::Call { callee, .. } => match &**callee {
            php::PhpExpr::Name(f) => FN_RETS.with(|m| {
                m.borrow()
                    .get(f.trim_start_matches('\\'))
                    .and_then(element_labels)
            }),
            _ => None,
        },
        _ => None,
    }
}

fn method_elem_labels(name: &str) -> Option<Vec<String>> {
    CLASS.with(|m| m.borrow().methods.get(name).and_then(element_labels))
}

/// Register the element shape of each builder local `body` returns under `ret` (see the module doc).
pub(in crate::lift::lifter) fn declare_returned_builders(
    body: &[php::PhpStmt],
    ret: &Option<Type>,
) {
    let Some(labels) = ret.as_ref().and_then(element_labels) else {
        return;
    };
    let returned: Vec<&str> = body
        .iter()
        .filter_map(|s| match s {
            php::PhpStmt::Return(Some(php::PhpExpr::Var(n))) => Some(n.as_str()),
            _ => None,
        })
        .collect();
    for s in body {
        let php::PhpStmt::Expr(php::PhpExpr::Assign { target, value }) = s else {
            continue;
        };
        if let (php::PhpExpr::Var(n), php::PhpExpr::Array(items)) = (&**target, &**value) {
            if items.is_empty() && returned.contains(&n.as_str()) {
                ELEM_FIELDS.with(|e| e.borrow_mut().insert(n.clone(), labels.clone()));
            }
        }
    }
}
