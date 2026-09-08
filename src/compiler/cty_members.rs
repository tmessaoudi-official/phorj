//! `impl Compiler` — MEMBER and PATH resolution: field-name indices, static/const slots, property
//! hooks, and the binding-path emitters.
//!
//! Split out of `cty.rs` under Invariant 13 (M-Decomp) when the `<=>` build (`eba09d4e`) pushed that
//! file past its size-baseline. The boundary is the real seam already present there: `cty.rs` answers
//! *what compile-time type is this expression* (the Invariant-7 CTy trap lives there and the warning
//! stays with it), while these answer *where does this member live* — pool index, static slot, const
//! value, hook method, binding path. Nothing here consults `CTy` to decide a type.

use super::*;

impl Compiler<'_> {
    /// Resolve a field/member name to its index in the program's `names` pool (for `GetField`). The
    /// pool is pre-built from every declared field name, so a checker-valid read always resolves;
    /// an unknown name would be a compiler bug.
    pub(in crate::compiler) fn field_name_index(&self, name: &str) -> Result<usize, String> {
        self.names_index
            .get(name)
            .copied()
            .ok_or_else(|| format!("unknown field `{name}`"))
    }

    /// Resolve a `ClassName.field` static-field access to its program-level static slot (M-mut.7).
    /// Returns `Some(idx)` only when `object` is a class *name* (not shadowed by a local) and `field`
    /// is one of its `static` fields — i.e. exactly the static-access shape the checker accepts.
    /// `None` ⇒ fall through to instance-field handling.
    pub(in crate::compiler) fn static_slot(&self, object: &Expr, field: &str) -> Option<usize> {
        if let Expr::Ident(name, _) = object {
            if self.resolve_local(name).is_none() && self.classes.contains_key(name) {
                return self
                    .statics_index
                    .get(&(name.clone(), field.to_string()))
                    .map(|&(idx, _)| idx);
            }
        }
        None
    }

    /// The `CTy` of a `ClassName.field` static access, or `None` if it is not a static (M-mut.7).
    /// Lets `ctype` treat a static as an arithmetic operand (`C.total + 1` specializes — without it
    /// the VM rejects what the interpreter accepts, the documented CTy-operand trap).
    pub(in crate::compiler) fn static_cty(&self, object: &Expr, field: &str) -> Option<CTy> {
        if let Expr::Ident(name, _) = object {
            if self.resolve_local(name).is_none() && self.classes.contains_key(name) {
                return self
                    .statics_index
                    .get(&(name.clone(), field.to_string()))
                    .map(|(_, cty)| cty.clone());
            }
        }
        None
    }

    /// The inlined literal `Value` of a `ClassName.NAME` class-constant access, or `None` if it is not
    /// a const (Feature A). Mirrors [`Self::static_slot`]; checked *before* it so a const access never
    /// looks for a (non-existent) static slot.
    pub(in crate::compiler) fn const_value(&self, object: &Expr, field: &str) -> Option<Value> {
        if let Expr::Ident(name, _) = object {
            if self.resolve_local(name).is_none() && self.classes.contains_key(name) {
                return self
                    .consts_index
                    .get(&(name.clone(), field.to_string()))
                    .map(|(v, _)| v.clone());
            }
        }
        None
    }

    /// The operand `CTy` of a `ClassName.NAME` class-constant access (Feature A) — lets `ctype` treat a
    /// const as an arithmetic operand (`Limits.MAX + 1` specializes), the same CTy-operand discipline
    /// as a static. Mirror of [`Self::static_cty`].
    pub(in crate::compiler) fn const_cty(&self, object: &Expr, field: &str) -> Option<CTy> {
        if let Expr::Ident(name, _) = object {
            if self.resolve_local(name).is_none() && self.classes.contains_key(name) {
                return self
                    .consts_index
                    .get(&(name.clone(), field.to_string()))
                    .map(|(_, cty)| cty.clone());
            }
        }
        None
    }

    /// The synthetic method name `<name>$get` if `object.name` is a readable property hook
    /// (M-mut.7b) — i.e. `object`'s compile-type is a class with a registered `<name>$get` method.
    /// `None` ⇒ `object.name` is a stored field (or not a hook), handled by `GetField`.
    pub(in crate::compiler) fn hook_get_method(&self, object: &Expr, name: &str) -> Option<String> {
        if let Ok(CTy::Class(cls)) = self.ctype(object) {
            let m = format!("{name}$get");
            if self.method_rets.contains_key(&(cls, m.clone())) {
                return Some(m);
            }
        }
        None
    }

    /// The synthetic method name `<name>$set` if `object.name` is a writable property hook
    /// (M-mut.7b). `None` ⇒ a stored field, handled by `SetField`.
    pub(in crate::compiler) fn hook_set_method(&self, object: &Expr, name: &str) -> Option<String> {
        if let Ok(CTy::Class(cls)) = self.ctype(object) {
            let m = format!("{name}$set");
            if self.method_rets.contains_key(&(cls, m.clone())) {
                return Some(m);
            }
        }
        None
    }

    /// Resolve a `match`-arm binding by name (innermost shadows). Returns the `$match` slot and the
    /// payload path to re-extract, cloned so the caller can emit without holding a borrow on `self`.
    pub(in crate::compiler) fn resolve_binding(&self, name: &str) -> Option<(usize, Vec<PathSeg>)> {
        self.match_bindings
            .iter()
            .rev()
            .find(|b| b.name == name)
            .map(|b| (b.match_slot, b.path.clone()))
    }

    /// Emit the per-step field loads of a binding `path` (the value to descend from is already on
    /// the stack). Each step is an enum-payload index or a named instance-field read.
    pub(in crate::compiler) fn emit_path(&mut self, path: &[PathSeg], line: u32) {
        for seg in path {
            match seg {
                PathSeg::Enum(i) => self.emit(Op::GetEnumField(*i), line),
                PathSeg::Field(idx) => self.emit(Op::GetField(*idx), line),
            }
        }
    }

    /// Push the sub-value of the `$match` scrutinee (slot `m_slot`) reached by `path`.
    pub(in crate::compiler) fn emit_load_path(
        &mut self,
        m_slot: usize,
        path: &[PathSeg],
        line: u32,
    ) {
        self.emit(Op::GetLocal(m_slot), line);
        self.emit_path(path, line);
    }
}
