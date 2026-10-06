//! Transpiler — kind inference for call results and reads off a generic instantiation.
//!
//! Split out of `kinds.rs` (Invariant 13). Generic type parameters are erased before the transpiler
//! runs, so a generic function's declared return kind is `Other`; DEC-557 needs the CONCRETE kind at the
//! call site, because a `decimal` hidden behind `Other` would take the string-exact `__phorj_eq`.

use super::*;
use crate::ast::Type;
use crate::types::Ty;

/// Peel one optional: `Wrapped([k])` (a `T?`) → `k`. A generic instantiation (head `Class`, then its
/// type arguments) and a multi-member union keep their `Wrapped` form.
pub(super) fn unwrap_optional(k: OpKind) -> OpKind {
    match k {
        OpKind::Wrapped(mut v) if v.len() == 1 => v.remove(0),
        other => other,
    }
}

/// The kind of `a ?? b`: `a`'s kind with the optional peeled (else `b`'s). A SCALAR result stays
/// optional unless `b` is provably non-null. The checker types `a ?? b` as `T?` when `b` may be null (a
/// second optional, a `null` literal, a `T | null` union, an operand the transpiler cannot resolve such
/// as `o?.s`), and a peeled `string` sent `==` down the strict path, whose `(string)` cast turned a
/// runtime `null` into `""` on the PHP leg (audit F1, 2026-10-06). The optional keeps its inner kind, so
/// a decimal behind it keeps loose `==`; arithmetic, interpolation and iteration peel it again
/// ([`Transpiler::operand_kind`]). A structural kind (list, map, tuple, class) is never wrapped: it
/// never takes the strict path, and its index/bind/member consumers must keep seeing it (panel P0-1).
pub(super) fn coalesce_kind(lhs: OpKind, rhs: OpKind, rhs_is_null: bool) -> OpKind {
    let maybe_null = rhs_is_null
        || rhs == OpKind::Other
        || matches!(&rhs, OpKind::Wrapped(v) if v.len() <= 1 || v.contains(&OpKind::Other));
    let k = known(match unwrap_optional(lhs) {
        OpKind::Other => unwrap_optional(rhs),
        k => k,
    });
    let scalar = matches!(
        k,
        OpKind::Str | OpKind::Int | OpKind::Float | OpKind::Bool | OpKind::Decimal
    );
    if maybe_null && scalar {
        OpKind::Wrapped(vec![k])
    } else {
        k
    }
}

/// A class member's kind, with an erased type parameter (`T v`) kept distinguishable from any other
/// unresolved kind as `Wrapped([])` — only an ERASED member may borrow its type arguments from a
/// generic instantiation (`generic_member_kind`); a `Set` or closure member must not.
pub(super) fn member_kind(ty: &Type) -> OpKind {
    match ty {
        Type::Erased(_) => OpKind::Wrapped(Vec::new()),
        other => kind_of_type(other),
    }
}

/// `Wrapped([])` (an erased member read off a non-generic receiver) carries no information: `Other`.
pub(super) fn known(k: OpKind) -> OpKind {
    match k {
        OpKind::Wrapped(v) if v.is_empty() => OpKind::Other,
        k => k,
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
    /// The kind of an operand the checker proves NON-null — an arithmetic or negation operand, an
    /// interpolation hole, a `for` source — with one optional peeled. The only `Wrapped([k])` that can
    /// reach one is a `??` whose default the transpiler could not prove non-null ([`coalesce_kind`]),
    /// or a decimal read off a generic instantiation (`Box<decimal>.v` is `Wrapped([Decimal])`, see
    /// `generic_member_kind`), which this peel routes through the `__phorj_dec_*` helpers like any
    /// other decimal; for every other kind the checker admits in those positions it is the identity.
    pub(super) fn operand_kind(&self, e: &Expr) -> OpKind {
        unwrap_optional(self.expr_kind(e))
    }

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
            OpKind::Wrapped(v) if v.is_empty() => {
                if args
                    .iter()
                    .any(|a| self.kind_has_decimal(a, &mut Vec::new()))
                {
                    OpKind::Wrapped(args.to_vec())
                } else {
                    OpKind::Other
                }
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
                let method = |c: &str| {
                    let k = self.lookup_method_ret_kind(c, name);
                    if matches!(&k, OpKind::Wrapped(v) if v.is_empty()) || k == OpKind::Other {
                        let key = (c.to_string(), name.clone());
                        echo(self.method_echo_param.get(&key)).unwrap_or(k)
                    } else {
                        k
                    }
                };
                match &recv {
                    OpKind::Class(c) => known(method(c)),
                    OpKind::Wrapped(_) => self.generic_member_kind(&recv, method),
                    _ => OpKind::Other,
                }
            }
            _ => OpKind::Other,
        }
    }
}
