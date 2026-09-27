//! DEC-515 — what the lifter knows about the **shape of a variable**, and for how long.
//!
//! DEC-504 seeded this from PARAMETERS only: a `@param array{bp: int} $row` makes `$row['bp']` lift
//! to `row.bp`. That reaches the direct reads and stops there. The census of 2026-09-10 (plan §L4b)
//! found the bulk of the corpus's rewritable reads one level further out — the collection is
//! declared, and every read happens on a `foreach` binder that carries no declaration of its own:
//!
//! ```php
//! /** @param list<array{bp: int}> $rows */
//! foreach ($rows as $row) { $n += $row['bp']; }   // `$row` IS a (bp: int)
//! ```
//!
//! So there are TWO facts to keep, not one: what a variable's own named-tuple fields are
//! ([`TUPLE_FIELDS`]), and what the fields of a variable's ELEMENTS are ([`ELEM_FIELDS`]) — the
//! second is what a binder inherits. Both are read-only derivations of a type the PROGRAM declared;
//! nothing here infers a shape from a call's return type or from the elements of a literal, which is
//! the DEC-166 line and the reason ~31 of the corpus's binder reads are deliberately out of reach.
//!
//! **Scoping is the correctness property, not a nicety.** A binder is block-scoped and these maps
//! are flat, so a registration that is never removed rewrites a LATER variable of the same name
//! against an EARLIER shape — silently, producing a draft that reads plausibly and means something
//! else. `Rent/Store/Store.php` alone has five distinct `$row` scopes with five different shapes.
//! [`enter_binder`] therefore returns the displaced bindings and [`leave_binder`] puts them back,
//! and the pair is exercised by `a_nested_binder_rebinding_one_name_restores_the_outer_shape`.
//!
//! A closure body is the same kind of scope (row 5d): [`enter_closure`] snapshots both maps and
//! [`leave_closure`] restores them, so a `@var` registered inside a closure never answers outside it.

use super::*;

#[path = "shapes_closure.rs"]
mod closure;
#[path = "shapes_props.rs"]
mod props;
pub(super) use closure::{enter_closure, leave_closure, swap_scope, ClosureScope};
#[cfg(feature = "regex")]
pub(super) use props::current_class;
pub(super) use props::{begin_file_fns, declare_returned_builders, enter_class_props};

thread_local! {
    /// DEC-504 — the NAMED-TUPLE fields of each variable in the function being lifted, keyed by
    /// variable name. Read by the `Index` arm so `$row['bp']` lifts to `row.bp` rather than to a
    /// string index a tuple cannot answer.
    ///
    /// Thread-local because the lifter is stateless free functions, so a fact learned in the
    /// signature has no other route to the body. FUNCTION-scoped — cleared at the start of every
    /// function lift, because `$row` in one function is not `$row` in the next.
    static TUPLE_FIELDS: std::cell::RefCell<std::collections::HashMap<String, Vec<String>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// DEC-515 — the named-tuple fields of each variable's ELEMENTS: what a `foreach` binder over
    /// that variable inherits. Same lifetime and same clearing point as [`TUPLE_FIELDS`].
    static ELEM_FIELDS: std::cell::RefCell<std::collections::HashMap<String, Vec<String>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// DEC-538 / row 4h — the variables DECLARED as a collection (parameter type, `@param`,
    /// `@var`): the leaf of the type (`List` / `Map`) and whether the declaration was nullable. A
    /// spread of a map is refused by name (PHP's string-key spread has map semantics); a strict
    /// `=== []` on a non-null one lifts to `List.isEmpty` / `Map.isEmpty` (`identity.rs`). Same
    /// lifetime, clearing point and closure snapshot as the two shape maps.
    static COLL_VARS: std::cell::RefCell<std::collections::HashMap<String, (&'static str, bool)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// The collection leaf of a declared type and whether it is nullable (seeing through one
/// `Optional`, as the shape tests do). Only the two a PHP `array` lifts to (`mappings.rs`).
fn coll_type(ty: &Type) -> Option<(&'static str, bool)> {
    let (ty, nullable) = match ty {
        Type::Optional { inner, .. } => (inner.as_ref(), true),
        other => (other, false),
    };
    match ty {
        Type::Named { name, .. } if name == "List" => Some(("List", nullable)),
        Type::Named { name, .. } if name == "Map" => Some(("Map", nullable)),
        _ => None,
    }
}

/// Whether `var` was declared a `Map`, nullable or not, in the function being lifted (DEC-538).
pub(super) fn is_map_var(var: &str) -> bool {
    COLL_VARS.with(|m| matches!(m.borrow().get(var), Some(("Map", _))))
}

/// The leaf of `var`'s declared collection type, when it was declared a NON-null `List` or `Map`
/// in the function being lifted (row 4h).
pub(super) fn coll_leaf(var: &str) -> Option<&'static str> {
    COLL_VARS.with(|m| match m.borrow().get(var) {
        Some((leaf, false)) => Some(*leaf),
        _ => None,
    })
}

/// The field labels of a named-tuple type, seeing through one `Optional`.
///
/// A POSITIONAL tuple contributes nothing — there are no field names to read by — and neither does
/// any other type. The `Optional` unwrap is what makes `@var array{…}|null $seen` behave like the
/// shape it is: PHP's `?array` and the docblock union describe one variable.
pub(super) fn tuple_labels(ty: &Type) -> Option<Vec<String>> {
    let ty = match ty {
        Type::Optional { inner, .. } => inner.as_ref(),
        other => other,
    };
    match ty {
        Type::Tuple(_, Some(labels), _) => Some(labels.iter().map(|l| l.name.clone()).collect()),
        _ => None,
    }
}

/// The field labels of a declared collection's ELEMENT — what a `foreach` binder over it inherits.
///
/// `list<array{…}>` and `array<T>` lift to `List<T>`, `array<K, V>` to `Map<K, V>` (`mappings.rs`),
/// so the element is the sole argument of a `List` and the SECOND of a `Map`. It is the VALUE that
/// binds: `foreach ($m as $k => $v)` gives `$v` the labels and `$k` the key type, never the reverse.
pub(super) fn element_labels(ty: &Type) -> Option<Vec<String>> {
    let ty = match ty {
        Type::Optional { inner, .. } => inner.as_ref(),
        other => other,
    };
    let Type::Named { name, args, .. } = ty else {
        return None;
    };
    match (name.as_str(), args.len()) {
        ("List", 1) => tuple_labels(&args[0]),
        ("Map", 2) => tuple_labels(&args[1]),
        _ => None,
    }
}

/// Record what every parameter declares, replacing the previous function's maps (DEC-504/DEC-515).
///
/// Both maps are cleared together: they have the same function scope, and clearing only one would
/// let a previous function's element shape answer for a name this one never declared.
/// Takes `(name, type)` pairs so a constructor's parameters reset the maps too (row 4p).
pub(super) fn set_tuple_fields<'a>(params: impl IntoIterator<Item = (&'a str, &'a Type)>) {
    let params: Vec<(&str, &Type)> = params.into_iter().collect();
    TUPLE_FIELDS.with(|t| {
        ELEM_FIELDS.with(|e| {
            let (mut t, mut e) = (t.borrow_mut(), e.borrow_mut());
            t.clear();
            e.clear();
            COLL_VARS.with(|m| {
                let mut m = m.borrow_mut();
                m.clear();
                m.extend(
                    params
                        .iter()
                        .filter_map(|(n, ty)| coll_type(ty).map(|c| (n.to_string(), c))),
                );
            });
            for (n, ty) in &params {
                if let Some(labels) = tuple_labels(ty) {
                    t.insert(n.to_string(), labels);
                }
                if let Some(labels) = element_labels(ty) {
                    e.insert(n.to_string(), labels);
                }
            }
        });
    });
}

/// Whether `var` is a named tuple with a field called `field` — the test that turns `$row['bp']`
/// into `row.bp`. A key the shape does NOT declare answers `false` and stays an index read: the
/// lifter invents no field, and the checker is the right place for that mistake to surface.
pub(super) fn is_tuple_field(var: &str, field: &str) -> bool {
    TUPLE_FIELDS.with(|m| {
        m.borrow()
            .get(var)
            .is_some_and(|fs| fs.iter().any(|f| f == field))
    })
}

/// The bindings a `foreach` binder displaced, so they can be put back when its body closes.
///
/// Both maps are saved, not just the tuple one: a binder shadows a name completely, so a variable
/// that WAS a keyed collection must stop answering as one inside a loop that rebinds its name.
pub(super) struct BinderScope {
    binder: String,
    tuple: Option<Vec<String>>,
    elem: Option<Vec<String>>,
}

/// Bind `binder` to the element shape of the collection being iterated, for the duration of the
/// loop body. Returns what was displaced — pass it to [`leave_binder`], always.
///
/// The shape comes from a DECLARATION only (DEC-166): a variable's (a parameter, a `@var`, a builder
/// local), a `$this->prop` property's (row 4p), or a statically resolved callee's declared return
/// (row 4p4, DEC-550 — `php_iter` is the PHP-side iterable, where `rows()` and `$rows()` still
/// differ). Anything else — another object's property or method, an inherited method — is unbound.
///
/// An iterated variable with NO element shape still displaces: the binder must stop answering with
/// whatever it meant outside the loop, or `foreach ($ints as $row)` would keep an outer `$row`'s
/// fields alive over an element that has none.
pub(super) fn enter_binder(iter: &Expr, php_iter: &php::PhpExpr, binder: &str) -> BinderScope {
    let labels = match iter {
        Expr::Ident(name, _) => ELEM_FIELDS.with(|m| m.borrow().get(name).cloned()),
        // Row 4p: `$this->prop`, a property whose declared type is a collection of shapes.
        Expr::Member { object, name, .. } if matches!(**object, Expr::This(_)) => {
            props::prop_elem_labels(name)
        }
        // Row 4p4: a statically resolved call's declared return.
        _ => props::call_elem_labels(php_iter),
    };
    TUPLE_FIELDS.with(|t| {
        ELEM_FIELDS.with(|e| {
            let (mut t, mut e) = (t.borrow_mut(), e.borrow_mut());
            let scope = BinderScope {
                binder: binder.to_string(),
                tuple: t.remove(binder),
                elem: e.remove(binder),
            };
            if let Some(labels) = labels {
                t.insert(binder.to_string(), labels);
            }
            scope
        })
    })
}

/// Put back what [`enter_binder`] displaced. Called on BOTH exits from a loop body — including the
/// error path, because a lift that fails still leaves the thread-local maps behind it.
pub(super) fn leave_binder(scope: BinderScope) {
    let BinderScope {
        binder,
        tuple,
        elem,
    } = scope;
    TUPLE_FIELDS.with(|t| {
        ELEM_FIELDS.with(|e| {
            let (mut t, mut e) = (t.borrow_mut(), e.borrow_mut());
            match tuple {
                Some(labels) => t.insert(binder.clone(), labels),
                None => t.remove(&binder),
            };
            match elem {
                Some(labels) => e.insert(binder, labels),
                None => e.remove(&binder),
            };
        });
    });
}

/// Whether `var`'s ELEMENTS are named tuples with a field called `field` — the test that turns
/// `$rows[0]['name']` into `rows[0].name` (DEC-515, row 5d). Same "declared field only" rule as
/// [`is_tuple_field`].
pub(super) fn is_elem_field(var: &str, field: &str) -> bool {
    ELEM_FIELDS.with(|m| {
        m.borrow()
            .get(var)
            .is_some_and(|fs| fs.iter().any(|f| f == field))
    })
}

/// Record a LOCAL's declared type (`/** @var T $x */ $x = …;`, DEC-515 row 5d) the way
/// [`set_tuple_fields`] records a parameter's. Returns whether the type carries a shape at all — the
/// caller keeps the explicit type on the declaration only then, so a `@var int $n` changes nothing.
///
/// Function-scoped like everything here: PHP locals ARE function-scoped, so a registration made
/// inside a block answers for the same name after it, as the PHP variable does.
pub(super) fn declare_local_shape(name: &str, ty: &Type) -> bool {
    let (tuple, elem) = (tuple_labels(ty), element_labels(ty));
    let shaped = tuple.is_some() || elem.is_some();
    // Recorded beside the shapes, but it never changes the answer: the return value decides whether
    // the declared type is KEPT on the declaration (`statements.rs`), which is a shape question.
    // A redeclaration replaces the old answer: `@var string $x` must stop `$x` being a list.
    COLL_VARS.with(|m| match coll_type(ty) {
        Some(c) => m.borrow_mut().insert(name.to_string(), c),
        None => m.borrow_mut().remove(name),
    });
    TUPLE_FIELDS.with(|t| {
        ELEM_FIELDS.with(|e| {
            let (mut t, mut e) = (t.borrow_mut(), e.borrow_mut());
            if let Some(labels) = tuple {
                t.insert(name.to_string(), labels);
            }
            if let Some(labels) = elem {
                e.insert(name.to_string(), labels);
            }
        });
    });
    shaped
}

/// The fields `var` itself declares, and the fields of its elements.
pub(super) fn fields_of(var: &str) -> (Option<Vec<String>>, Option<Vec<String>>) {
    (
        TUPLE_FIELDS.with(|m| m.borrow().get(var).cloned()),
        ELEM_FIELDS.with(|m| m.borrow().get(var).cloned()),
    )
}
