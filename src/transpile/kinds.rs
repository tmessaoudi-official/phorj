//! Transpiler — PHP-side kind inference (`OpKind` of locals/methods/fields/exprs).

use super::*;

impl Transpiler {
    /// Record a local/param/loop-var's scalar [`OpKind`] in the current scope (T6). Only called where
    /// the declared type is statically known; names without a kind resolve to `Other` (helper path).
    pub(super) fn declare_kind(&mut self, name: &str, kind: OpKind) {
        if kind != OpKind::Other {
            if let Some(s) = self.local_kinds.last_mut() {
                s.insert(name.to_string(), kind);
            }
        }
    }
    /// Resolve a name's [`OpKind`] from the innermost scope outward; `Other` if unknown.
    pub(super) fn local_kind(&self, name: &str) -> OpKind {
        self.local_kinds
            .iter()
            .rev()
            .find_map(|s| s.get(name).cloned())
            .unwrap_or(OpKind::Other)
    }

    /// The return [`OpKind`] of `class.method` — own method else walk `extends` parents (T6c).
    pub(super) fn lookup_method_ret_kind(&self, class: &str, method: &str) -> OpKind {
        if let Some(k) = self
            .method_ret_kinds
            .get(&(class.to_string(), method.to_string()))
        {
            return k.clone();
        }
        if let Some(parents) = self.class_parents.get(class) {
            for p in parents {
                let k = self.lookup_method_ret_kind(p, method);
                if k != OpKind::Other {
                    return k;
                }
            }
        }
        OpKind::Other
    }

    /// The [`OpKind`] of `class.field` — the field's own kind, else walk `extends` parents (T6b).
    /// `Other` if the class/field is unknown (→ helper fallback).
    pub(super) fn lookup_field_kind(&self, class: &str, field: &str) -> OpKind {
        if let Some(k) = self.class_field_kinds.get(class).and_then(|m| m.get(field)) {
            return k.clone();
        }
        if let Some(parents) = self.class_parents.get(class) {
            for p in parents {
                let k = self.lookup_field_kind(p, field);
                if k != OpKind::Other {
                    return k;
                }
            }
        }
        OpKind::Other
    }

    /// Statically resolve an expression's operand [`OpKind`] for native-operator selection (T6).
    /// Covers the scalar surface — literals, typed locals/params/loop-vars, nested arithmetic/unary,
    /// `instanceof` (bool), and `inner!` (the inner's kind). Field reads, indexing, method/function
    /// calls and `this` are deliberately `Other` (→ runtime helper), since pinning their types down
    /// would mean rebuilding the compiler's full type maps; the helper fallback keeps those correct.
    pub(super) fn expr_kind(&self, e: &Expr) -> OpKind {
        match e {
            Expr::Int(..) => OpKind::Int,
            Expr::Float(..) => OpKind::Float,
            Expr::Decimal { .. } => OpKind::Decimal,
            Expr::Str(..) => OpKind::Str,
            Expr::Bool(..) => OpKind::Bool,
            Expr::Ident(name, _) => {
                // T6d: a bare class-name ident (only ever the object of a static/const access
                // `ClassName::FIELD`) resolves to that class, so the enclosing `Member` arm can look
                // up the const/static field's kind. A real local shadows (checked first).
                let k = self.local_kind(name);
                if k == OpKind::Other && self.classes.contains(name) {
                    OpKind::Class(name.clone())
                } else {
                    k
                }
            }
            Expr::Unary { op, expr, .. } => match op {
                UnaryOp::Neg => self.expr_kind(expr),
                UnaryOp::Not => OpKind::Bool,
                UnaryOp::BitNot => OpKind::Int,
            },
            Expr::Binary { op, lhs, rhs, .. } => match op {
                // Arithmetic: result kind follows the operands (the checker guarantees they agree).
                // `+` over strings is concatenation → `Str`; otherwise numeric (Float dominates Int).
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                    let (l, r) = (self.expr_kind(lhs), self.expr_kind(rhs));
                    if matches!(op, BinaryOp::Add) && (l == OpKind::Str || r == OpKind::Str) {
                        OpKind::Str
                    } else if l == OpKind::Decimal || r == OpKind::Decimal {
                        // `decimal ⊕ {decimal,int}` stays decimal (M-NUM S1); the PHP carrier is a
                        // string, but the operand *kind* is `Decimal` so a nested `(a * b) + c`
                        // routes every level through the `__phorj_dec_*` helpers.
                        OpKind::Decimal
                    } else if l == OpKind::Float || r == OpKind::Float {
                        OpKind::Float
                    } else if l == OpKind::Int || r == OpKind::Int {
                        OpKind::Int
                    } else {
                        OpKind::Other
                    }
                }
                // Comparisons / logical / bitwise-on-bool produce a bool.
                BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::Le
                | BinaryOp::Gt
                | BinaryOp::Ge
                | BinaryOp::And
                | BinaryOp::Or => OpKind::Bool,
                // Bitwise ops are int-only (primitives P2) → an int operand for any enclosing `+`.
                BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
                | BinaryOp::Shl
                | BinaryOp::Shr => OpKind::Int,
                _ => OpKind::Other,
            },
            Expr::InstanceOf { .. } => OpKind::Bool,
            Expr::Force { inner, .. } => self.expr_kind(inner),
            // T6d: `xs[i]` → element kind; `m[k]` → value kind.
            Expr::Index { object, .. } => match self.expr_kind(object) {
                OpKind::List(elem) => *elem,
                OpKind::Map(_, val) => *val,
                _ => OpKind::Other,
            },
            // A list/map literal carries its element kind from the first item, so `[1,2,3][0]`
            // resolves (M3 S1.1 analog).
            Expr::List(items, _) => OpKind::List(Box::new(
                items.first().map_or(OpKind::Other, |e| self.expr_kind(e)),
            )),
            Expr::Map(pairs, _) => OpKind::Map(
                Box::new(
                    pairs
                        .first()
                        .map_or(OpKind::Other, |(k, _)| self.expr_kind(k)),
                ),
                Box::new(
                    pairs
                        .first()
                        .map_or(OpKind::Other, |(_, v)| self.expr_kind(v)),
                ),
            ),
            // T6b: `this` is the enclosing class; a field read resolves through the class tables.
            Expr::This(_) => self
                .cur_class
                .as_ref()
                .map_or(OpKind::Other, |c| OpKind::Class(c.clone())),
            // A field read `obj.f` (instance or `this`): resolve `obj`'s class, then look up `f`.
            // A safe read `obj?.f` is `T?` (an optional) → not a scalar operand → `Other`.
            Expr::Member {
                object,
                name,
                safe: false,
                ..
            } => match self.expr_kind(object) {
                OpKind::Class(c) => self.lookup_field_kind(&c, name),
                _ => OpKind::Other,
            },
            // A call result (T6c): a constructor `ClassName(...)` (Phorj `new` is unwrapped to a
            // `Call`) yields an instance of that class (so `mk().x` resolves); a free-function call
            // resolves to its declared return kind; a method call `obj.m(...)` resolves to the
            // method's return kind on `obj`'s class (+ inherited).
            Expr::Call { callee, .. } => match &**callee {
                Expr::Ident(name, _) if self.classes.contains(name) => OpKind::Class(name.clone()),
                Expr::Ident(name, _) => self
                    .fn_ret_kinds
                    .get(name)
                    .cloned()
                    .unwrap_or(OpKind::Other),
                Expr::Member {
                    object,
                    name,
                    safe: false,
                    ..
                } => {
                    // T6d: a native call `Leaf.fn(...)` (Leaf an imported module qualifier, e.g.
                    // `Text.upper`) resolves to the native's declared return kind (mirrors the
                    // import-driven native resolution in `emit_call`).
                    if let Expr::Ident(leaf, _) = &**object {
                        if let Some(module) = self.imports.get(leaf) {
                            if let Some(idx) = crate::native::index_of(module, name) {
                                return opkind_of_ty(&crate::native::registry()[idx].ret);
                            }
                        }
                    }
                    // Otherwise a method call on a value — resolve its receiver's class.
                    match self.expr_kind(object) {
                        OpKind::Class(c) => self.lookup_method_ret_kind(&c, name),
                        _ => OpKind::Other,
                    }
                }
                _ => OpKind::Other,
            },
            _ => OpKind::Other,
        }
    }

    /// The kind of an `/` or `%` result for native-operator selection (T6): `Float` if either operand
    /// is float, `Int` if either is int, else `Other` (→ runtime helper). The checker guarantees both
    /// operands share a numeric type, so resolving either suffices.
    pub(super) fn arith_kind(&self, lhs: &Expr, rhs: &Expr) -> OpKind {
        match (self.expr_kind(lhs), self.expr_kind(rhs)) {
            (OpKind::Float, _) | (_, OpKind::Float) => OpKind::Float,
            (OpKind::Int, _) | (_, OpKind::Int) => OpKind::Int,
            _ => OpKind::Other,
        }
    }
}
