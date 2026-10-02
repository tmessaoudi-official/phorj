//! Transpiler — kind inference for call results and reads off a generic instantiation.
//!
//! Split out of `kinds.rs` (Invariant 13). Generic type parameters are erased before the transpiler
//! runs, so a generic function's declared return kind is `Other`; DEC-557 needs the CONCRETE kind at the
//! call site, because a `decimal` hidden behind `Other` would take the string-exact `__phorj_eq`.

use super::*;
use crate::types::Ty;

/// Peel one optional: `Wrapped([k])` (a `T?`) → `k`. A generic instantiation (head `Class`, then its
/// type arguments) and a multi-member union keep their `Wrapped` form.
pub(super) fn unwrap_optional(k: OpKind) -> OpKind {
    match k {
        OpKind::Wrapped(mut v) if v.len() == 1 => v.remove(0),
        other => other,
    }
}

/// Bind a native signature's type parameters from the argument kinds (`List<T>` ↔ `List(k)`, …).
fn bind(ty: &Ty, k: &OpKind, out: &mut HashMap<String, OpKind>) {
    match (ty, k) {
        (Ty::Param(n), k) => {
            out.entry(n.clone()).or_insert_with(|| k.clone());
        }
        (Ty::List(t), OpKind::List(e)) => bind(t, e, out),
        (Ty::Map(tk, tv), OpKind::Map(k, v)) => {
            bind(tk, k, out);
            bind(tv, v, out);
        }
        (Ty::Optional(t), k) => bind(t, &unwrap_optional(k.clone()), out),
        _ => {}
    }
}

/// A native's return type with its type parameters replaced by what `bind` found.
fn subst(ty: &Ty, b: &HashMap<String, OpKind>) -> OpKind {
    match ty {
        Ty::Param(n) => b.get(n).cloned().unwrap_or(OpKind::Other),
        Ty::List(e) => OpKind::List(Box::new(subst(e, b))),
        Ty::Map(k, v) => OpKind::Map(Box::new(subst(k, b)), Box::new(subst(v, b))),
        Ty::Optional(e) => OpKind::Wrapped(vec![subst(e, b)]),
        other => opkind_of_ty(other),
    }
}

impl Transpiler {
    /// The kind of a read (`field` / `method` result) off a generic instantiation `Wrapped([Class(c),
    /// args…])`: the member's own kind, else — its type being an erased parameter — the type arguments,
    /// but only when one carries a `decimal` (anything else stays `Other`, the string-exact helper).
    pub(super) fn generic_member_kind(
        &self,
        recv: &OpKind,
        member: impl Fn(&str) -> OpKind,
    ) -> OpKind {
        let OpKind::Wrapped(parts) = recv else {
            return OpKind::Other;
        };
        let Some((OpKind::Class(c), args)) = parts.split_first() else {
            return OpKind::Other;
        };
        match member(c) {
            OpKind::Other
                if args
                    .iter()
                    .any(|a| self.kind_has_decimal(a, &mut Vec::new())) =>
            {
                OpKind::Wrapped(args.to_vec())
            }
            k => k,
        }
    }

    /// The [`OpKind`] of a call result (T6c): a constructor yields its class; a variant constructor its
    /// enum; a free function its declared return kind (a generic one echoes the argument its return
    /// names); a native `Leaf.fn(...)` its signature with type parameters bound from the arguments; a
    /// method call the method's return kind on the receiver's class.
    pub(super) fn call_kind(&self, callee: &Expr, args: &[Expr]) -> OpKind {
        let echo = |i: Option<&usize>| i.and_then(|i| args.get(*i)).map(|a| self.expr_kind(a));
        match callee {
            Expr::Ident(name, _) if self.classes.contains(name) => OpKind::Class(name.clone()),
            // A bare variant constructor `Exact(1.5d)` is a value of its enum (DEC-557).
            Expr::Ident(name, _) if self.variant_owner.contains_key(name) => {
                OpKind::Class(self.variant_owner[name].clone())
            }
            Expr::Ident(name, _) => match self.fn_ret_kinds.get(name) {
                Some(OpKind::Other) | None => {
                    echo(self.fn_echo_param.get(name)).unwrap_or(OpKind::Other)
                }
                Some(k) => k.clone(),
            },
            Expr::Member {
                object,
                name,
                safe: false,
                ..
            } => {
                // T6d: a native call `Leaf.fn(...)` (Leaf an imported module qualifier).
                if let Expr::Ident(leaf, _) = &**object {
                    if let Some(idx) = self
                        .imports
                        .get(leaf)
                        .and_then(|m| crate::native::index_of(m, name))
                    {
                        let nf = &crate::native::registry()[idx];
                        let mut b = HashMap::new();
                        for (p, a) in nf.params.iter().zip(args) {
                            bind(p, &self.expr_kind(a), &mut b);
                        }
                        return subst(&nf.ret, &b);
                    }
                    // A qualified variant constructor `Enum.Variant(...)` is a value of that enum.
                    if self.enums.contains(leaf)
                        && self
                            .variant_field_kinds
                            .contains_key(&(leaf.clone(), name.clone()))
                    {
                        return OpKind::Class(leaf.clone());
                    }
                }
                // Otherwise a method call on a value — resolve its receiver's class.
                let recv = self.expr_kind(object);
                let method = |c: &str| match self.lookup_method_ret_kind(c, name) {
                    OpKind::Other => {
                        echo(self.method_echo_param.get(&(c.to_string(), name.clone())))
                            .unwrap_or(OpKind::Other)
                    }
                    k => k,
                };
                match &recv {
                    OpKind::Class(c) => method(c),
                    OpKind::Wrapped(_) => self.generic_member_kind(&recv, method),
                    _ => OpKind::Other,
                }
            }
            _ => OpKind::Other,
        }
    }
}
