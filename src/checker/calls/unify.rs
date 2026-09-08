//! Type unification for generic calls — matching a declared parameter type against an argument
//! type to solve the call's type parameters.
//!
//! Split out of `overloads.rs` (Invariant 13): unification is the shared subroutine the overload
//! and generic-call paths both call, not part of either one's dispatch logic.

use super::*;

impl Checker {
    /// Structural unification of a declared type (possibly containing `Ty::Param`) against a concrete
    /// argument type, accumulating bindings in `θ`. Returns false on a mismatch. A parameter binds
    /// the first concrete type it meets; a later occurrence must be *consistent* (assignable either
    /// way, so subtyping is tolerated). A non-parameter position falls back to ordinary
    /// assignability. `Ty::Error` (poison) unifies with anything (M-RT S7).
    pub(in crate::checker) fn unify(
        &self,
        declared: &Ty,
        actual: &Ty,
        theta: &mut HashMap<String, Ty>,
    ) -> bool {
        if matches!(declared, Ty::Error) || matches!(actual, Ty::Error) {
            return true;
        }
        match (declared, actual) {
            (Ty::Param(p), a) => match theta.get(p) {
                None => {
                    theta.insert(p.clone(), a.clone());
                    true
                }
                Some(bound) => self.ty_assignable(a, bound) || self.ty_assignable(bound, a),
            },
            (Ty::List(d), Ty::List(a)) | (Ty::Set(d), Ty::Set(a)) => self.unify(d, a, theta),
            (Ty::Optional(d), Ty::Optional(a)) => self.unify(d, a, theta),
            // A non-null, non-optional argument against an `Optional(T)` parameter binds `T` from the
            // inner type — `Option.ofNullable(42)` infers `T = int` (an `int` IS assignable to `int?`,
            // so this just aligns `unify` with the existing `(other, Optional(t))` assignability rule).
            // A bare `null` is deliberately excluded: it cannot determine `T` (falls through to plain
            // assignability — `null` is assignable to any optional, but binds nothing).
            (Ty::Optional(d), a) if !matches!(a, Ty::Null) => self.unify(d, a, theta),
            (Ty::Map(dk, dv), Ty::Map(ak, av)) => {
                self.unify(dk, ak, theta) && self.unify(dv, av, theta)
            }
            // Unify params + ret only; the `throws` sets (DEC-222) are not a unification position (no
            // native carries a throwing function param, and a throws set has no positional arity to
            // bind against) — a type param appearing solely in a throws position is not inferred.
            (Ty::Function(dp, dr, _), Ty::Function(ap, ar, _)) => {
                dp.len() == ap.len()
                    && dp.iter().zip(ap).all(|(d, a)| self.unify(d, a, theta))
                    && self.unify(dr, ar, theta)
            }
            // Two generic class instances with the same head — unify their arguments so a generic
            // function over a generic class (`function unwrap<T>(Box<T> b) -> T`) binds `T` from a
            // `Box<int>` argument (M-RT generics-all). Different heads fall through to assignability.
            (Ty::Named(dn, da), Ty::Named(an, aa)) if dn == an && da.len() == aa.len() => {
                da.iter().zip(aa).all(|(d, a)| self.unify(d, a, theta))
            }
            // Same-arity tuples unify element-wise (DEC-288) — a generic `f<A, B>((A, B) p)` binds
            // `A`/`B` from an `(int, string)` argument.
            // DEC-504: labels are PART of the type, so a labelled tuple unifies only with an
            // identically-labelled one — `(a: int, b: int)` and `(c: int, d: int)` are distinct.
            (Ty::Tuple(dts, dl), Ty::Tuple(ats, al)) if dts.len() == ats.len() && dl == al => {
                dts.iter().zip(ats).all(|(d, a)| self.unify(d, a, theta))
            }
            // No type parameter at this position — ordinary assignability (actual → declared).
            (d, a) => self.ty_assignable(a, d),
        }
    }
}
