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

use super::*;

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
pub(super) fn set_tuple_fields(params: &[Param]) {
    TUPLE_FIELDS.with(|t| {
        ELEM_FIELDS.with(|e| {
            let (mut t, mut e) = (t.borrow_mut(), e.borrow_mut());
            t.clear();
            e.clear();
            for p in params {
                if let Some(labels) = tuple_labels(&p.ty) {
                    t.insert(p.name.clone(), labels);
                }
                if let Some(labels) = element_labels(&p.ty) {
                    e.insert(p.name.clone(), labels);
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
/// The shape is looked up only when the iterated expression is a plain variable. Anything else —
/// `foreach ($this->rows as $r)`, `foreach (f() as $r)` — is left unbound: `$this->rows` is an
/// `Expr::Member` the read site cannot key on, and a call's return type is precisely what the
/// lifter refuses to infer (DEC-166).
///
/// An iterated variable with NO element shape still displaces: the binder must stop answering with
/// whatever it meant outside the loop, or `foreach ($ints as $row)` would keep an outer `$row`'s
/// fields alive over an element that has none.
pub(super) fn enter_binder(iter: &Expr, binder: &str) -> BinderScope {
    let labels = match iter {
        Expr::Ident(name, _) => ELEM_FIELDS.with(|m| m.borrow().get(name).cloned()),
        _ => None,
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
