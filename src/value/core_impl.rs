//! `impl Value` — type names, display, truthiness — and AST-literal conversion.

use super::*;

/// If `v` is a lazy `Json` node (DEC-294), materialize one level to a `Value::Enum`; otherwise return
/// it unchanged. Always compiled (the VM's `MatchTag`/`GetEnumField` call it unconditionally): with the
/// `json` feature off there is no `JsonLazy` variant, so this is the identity.
#[cfg(feature = "json")]
pub fn materialize_if_lazy(v: Value) -> Value {
    crate::ext::json::materialize_if_lazy(v)
}
#[cfg(not(feature = "json"))]
#[inline]
pub fn materialize_if_lazy(v: Value) -> Value {
    v
}

impl Value {
    /// Short name for diagnostics. Composite types fold to a constant so the
    /// return can stay `&'static str`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Decimal { .. } => "decimal",
            Value::Bool(_) => "bool",
            Value::Str(_) => "string",
            Value::Bytes(_) => "bytes",
            Value::Unit => "unit",
            Value::Null => "null",
            Value::List(_) => "list",
            Value::Map(_) => "map",
            Value::Set(_) => "set",
            Value::Instance(_) => "instance",
            Value::Enum(_) => "enum",
            #[cfg(feature = "json")]
            Value::JsonLazy(_) => "enum", // a lazy Json node IS an enum (DEC-294) — no materialize needed
            Value::Closure(_) => "function",
            Value::Channel(..) => "channel",
            Value::Task(_) => "task",
            Value::Db(h) => h.kind(),
        }
    }

    /// Render a *primitive* value for interpolation / `println`. `None` for a
    /// composite value (the caller turns that into a runtime `Diagnostic`). Floats use
    /// Rust `{}` formatting (EV-6): `12.0` -> `"12"`.
    pub fn as_display(&self) -> Option<String> {
        match self {
            Value::Int(n) => Some(n.to_string()),
            Value::Float(x) => Some(format!("{x}")),
            // A decimal renders with exactly `scale` fractional digits (BCMath pads), never `-0`.
            // The same `fmt_decimal` both backends use, and the emitted PHP string already carries
            // this form (a BCMath result string `(string)`s identically) — so interpolating a decimal
            // is byte-identical across run/runvm/PHP (M-NUM S1).
            Value::Decimal { unscaled, scale } => Some(fmt_decimal(*unscaled, *scale)),
            Value::Bool(b) => Some(b.to_string()),
            Value::Str(s) => Some(s.as_str().to_string()),
            Value::Unit => Some("unit".to_string()),
            // Functions cannot be displayed (the checker forbids interpolating a function
            // value; this arm is only reached through the fallback `_ => None` path — EV-7).
            Value::Closure(_) => None,
            _ => None,
        }
    }

    /// Structural value equality for `==` / `!=`. Cycle-safe (F4): instances became shared-mutable
    /// handles in M-mut.6, so `a.next = b; b.next = a` can form a reference cycle. An unguarded
    /// recursion on such a cycle would overflow the native stack — and at *different* depths per
    /// backend, breaking `agree_err`. The `visited` pair set short-circuits a re-encountered
    /// `(a, b)` pair to `true` (co-inductive bisimulation, the standard correct cyclic equality), so
    /// `==` always terminates deterministically. PHP `==` is likewise cycle-protected.
    pub fn eq_val(&self, other: &Value) -> bool {
        self.eq_val_rec(other, &mut Vec::new())
    }

    /// Recursive worker for [`eq_val`]. `visited` records instance-pointer pairs currently being
    /// compared; only the `Instance` arm consults/extends it (lists/maps/sets/enums are acyclic value
    /// types — a cycle can only thread through an instance handle). Not popping memoizes equal pairs
    /// too, which is sound: a *false* pair short-circuits the whole comparison (every `&&`/`.all()`
    /// propagates the `false` up), so a stale-`true` memo for a false pair is never observed.
    #[allow(clippy::float_cmp)] // intentional: language-level float equality
    fn eq_val_rec(
        &self,
        other: &Value,
        visited: &mut Vec<(*const Instance, *const Instance)>,
    ) -> bool {
        // A lazy Json node (DEC-294) compares as the enum it materializes to — one level here, then
        // recurse (children materialize as they are compared). Keeps `==` byte-identical to eager Json.
        #[cfg(feature = "json")]
        {
            if let Value::JsonLazy(l) = self {
                return crate::ext::json::materialize_lazy(l).eq_val_rec(other, visited);
            }
            if let Value::JsonLazy(l) = other {
                return self.eq_val_rec(&crate::ext::json::materialize_lazy(l), visited);
            }
        }
        use Value::*;
        match (self, other) {
            (Int(a), Int(b)) => a == b,
            (Float(a), Float(b)) => a == b,
            // Decimal equality is **numeric, scale-insensitive** (`1.50d == 1.5d`): align both to the
            // max scale and compare unscaled. Mixed `decimal`/`int` widens the int to scale 0 (so
            // `2d == 2` matches `decimal == int` operator typing). An alignment overflow can't occur
            // here — it would only change two unequal values, never make equal values compare wrong:
            // when alignment overflows, the magnitudes differ by ≥10^Δ at i128 scale, so they are not
            // equal, and `decimal_cmp` returns `None` ⇒ `false`, which is correct.
            (Decimal { .. } | Int(_), Decimal { .. } | Int(_))
                if matches!((self, other), (Decimal { .. }, _) | (_, Decimal { .. })) =>
            {
                decimal_cmp(self, other) == Some(Ordering::Equal)
            }
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Bytes(a), Bytes(b)) => a == b,
            (Unit, Unit) => true,
            (List(a), List(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b.iter())
                        .all(|(x, y)| x.eq_val_rec(y, visited))
            }
            // Maps compare **order-independently** (insertion order is part of iteration, not of
            // identity): same key set with `eq_val` values. This matches PHP associative `==`.
            (Map(a), Map(b)) => {
                a.len() == b.len()
                    && a.iter().all(|(k, v)| {
                        b.iter()
                            .find(|(bk, _)| bk == k)
                            .is_some_and(|(_, bv)| v.eq_val_rec(bv, visited))
                    })
            }
            // Sets compare **order-independently** (insertion order is iteration, not identity):
            // same cardinality and same membership. Both are deduped by `build_set`, so a one-way
            // containment check at equal length suffices.
            (Set(a), Set(b)) => a.len() == b.len() && a.iter().all(|k| b.contains(k)),
            (Enum(a), Enum(b)) => {
                a.ty == b.ty
                    && a.variant == b.variant
                    && a.payload.len() == b.payload.len()
                    && a.payload
                        .iter()
                        .zip(b.payload.iter())
                        .all(|(x, y)| x.eq_val_rec(y, visited))
            }
            (Instance(a), Instance(b)) => {
                let pair = (Rc::as_ptr(a), Rc::as_ptr(b));
                if visited.contains(&pair) {
                    return true; // already comparing this pair (a cycle) → assume equal
                }
                visited.push(pair);
                if a.class != b.class {
                    return false;
                }
                // Same class ⇒ same shared `layout` ⇒ identical slot order, so compare slot-aligned.
                // An unset slot (`None`) is equal only to another unset slot — byte-identical to the
                // pre-S1b key-set comparison (a `None` slot was an absent HashMap key there).
                let fa = a.fields.borrow();
                let fb = b.fields.borrow();
                fa.len() == fb.len()
                    && fa.iter().zip(fb.iter()).all(|(x, y)| match (x, y) {
                        (Some(xv), Some(yv)) => xv.eq_val_rec(yv, visited),
                        (None, None) => true,
                        _ => false,
                    })
            }
            (Null, Null) => true,
            // Functions are not comparable — the checker forbids `==`/`!=` on function
            // types; this arm is a defensive fallback (EV-7, well-typed programs never reach it).
            (Closure(_), _) | (_, Closure(_)) => false,
            _ => false,
        }
    }
}

// --- Arithmetic & comparison kernels (single-sourced; both backends call these) ---
//
// The `Op::Neg` parity bug (M2 P3.5 Wave 0) was possible because integer arithmetic lived in two
// hand-kept-identical copies, one per backend. These kernels are the *one* implementation both the
// tree-walker (`interpreter::arith`/`eval_unary`/`compare`) and the VM (`vm.rs` arith arms +
// `compare`) dispatch into, so the two can no longer drift. They return the bare fault *body*
// (`String`); each backend wraps it in its own error type. Floats can't fault (NaN/inf are valid
// `f64`); only integer overflow and integer division/modulo by zero are faults. The op→bool / op→fn
// projection stays in each backend — their op enums (`BinaryOp` vs `Op`) differ, so only the
// arithmetic and the fault strings are shared, not the dispatch.

/// DEC-302 canonical fault body for `Enum.from(x)` when no variant carries the value `x`. Single-
/// sourced so the interpreter's `rt(..)` and the VM's `Op::EnumFrom` fault render byte-identically
/// (Invariant #4 — fault bodies are parity-affecting). The value is shown in its literal form (a
/// quoted string, a bare int); any other `Value` is checker-unreachable for a backed-enum arg.
pub fn enum_from_miss(enum_name: &str, value: &Value) -> String {
    let shown = match value {
        Value::Str(s) => format!("\"{}\"", s.as_str()),
        Value::Int(n) => n.to_string(),
        other => other.type_name().to_string(),
    };
    format!("no case of enum `{enum_name}` has value {shown}")
}

/// Evaluate a compile-time **literal-constant** expression to a `Value` — used to seed `static`
/// field storage once at program load (M-mut.7). Both backends call this (F3), so the interpreter's
/// `statics` map and the VM's `static_inits` table hold identical values. Returns `None` for anything
/// that is not a literal; the checker rejects a non-literal static initializer (`E-STATIC-INIT-CONST`),
/// so a `None` is checker-unreachable at load. Scalars + `null` + `bytes` only this slice — richer
/// constant expressions (arithmetic, collection literals) are deferred.
pub fn const_literal(e: &crate::ast::Expr) -> Option<Value> {
    use crate::ast::{Expr, StrPart};
    match e {
        Expr::Int(n, _) => Some(Value::Int(*n)),
        Expr::Float(f, _) => Some(Value::Float(*f)),
        Expr::Decimal {
            unscaled, scale, ..
        } => Some(Value::Decimal {
            unscaled: *unscaled,
            scale: *scale,
        }),
        Expr::Bool(b, _) => Some(Value::Bool(*b)),
        Expr::Null(_) => Some(Value::Null),
        // A plain string literal is a single `Literal` part; any interpolation makes it non-const.
        Expr::Str(parts, _) => match parts.as_slice() {
            [] => Some(Value::Str(PhStr::empty())),
            [StrPart::Literal(s)] => Some(Value::Str(PhStr::literal(s))),
            _ => None,
        },
        Expr::Bytes(b, _) => Some(Value::Bytes(Rc::new(b.clone()))),
        _ => None,
    }
}
