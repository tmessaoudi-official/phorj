//! Row 4p (2026-09-27) — two more DECLARATIONS a shape can come from, beside a parameter and a
//! `@var` local (DEC-504, DEC-515).
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
//!
//! Both read a type the program WROTE; neither looks at a call's return or a literal's elements
//! (DEC-166).

use super::*;

thread_local! {
    /// Property name → its lifted type, for the class being lifted.
    static PROP_TYPES: std::cell::RefCell<std::collections::HashMap<String, Type>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// The enclosing class's properties, put back when this class's lift ends — on the error path too.
pub(in crate::lift::lifter) struct PropScope(Option<std::collections::HashMap<String, Type>>);

impl Drop for PropScope {
    fn drop(&mut self) {
        if let Some(prev) = self.0.take() {
            PROP_TYPES.with(|m| *m.borrow_mut() = prev);
        }
    }
}

/// Record `c`'s instance properties: declared ones, and the constructor's promoted parameters.
///
/// A type that does not lift is skipped, not reported: the member's own lift reports it and fails
/// the class, so the registry never answers for a class that lifts.
pub(in crate::lift::lifter) fn enter_class_props(c: &php::PhpClass) -> PropScope {
    let mut props = std::collections::HashMap::new();
    for m in &c.members {
        match m {
            php::PhpMember::Prop {
                is_static: false,
                ty: Some(ty),
                name,
                ..
            } => {
                if let Ok(t) = lift_type(ty) {
                    props.insert(name.clone(), t);
                }
            }
            php::PhpMember::Method(me) if me.name == "__construct" => {
                for p in me.params.iter().filter(|p| p.promotion.is_some()) {
                    if let Some(Ok(t)) = p.ty.as_ref().map(lift_type) {
                        props.insert(p.name.clone(), t);
                    }
                }
            }
            _ => {}
        }
    }
    PropScope(Some(
        PROP_TYPES.with(|m| std::mem::replace(&mut *m.borrow_mut(), props)),
    ))
}

/// The field labels of property `name`'s ELEMENTS — what a binder over `$this->name` inherits.
pub(super) fn prop_elem_labels(name: &str) -> Option<Vec<String>> {
    PROP_TYPES.with(|m| m.borrow().get(name).and_then(element_labels))
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
