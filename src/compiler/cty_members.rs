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

    /// Emit the plain read of `.name` on the value on top of the stack: `GetField`, or `EnumValue` for
    /// a `.value` whose name no class declares as a field or hook — the checker accepted the read, so
    /// the receiver (one the compiler could not type: `m ?? …`, a `Result` binder) is a backed enum
    /// (plan row 4b, close panel round 3).
    pub(in crate::compiler) fn emit_field_read(
        &mut self,
        name: &str,
        line: u32,
    ) -> Result<(), String> {
        match self.field_name_index(name) {
            Ok(idx) => self.emit(Op::GetField(idx), line),
            Err(_) if name == "value" => self.emit(Op::EnumValue, line),
            Err(e) => return Err(e),
        }
        Ok(())
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

    /// The synthetic method name `<name>$get` when `object.name` is a hook decided at COMPILE time
    /// (M-mut.7b): `object`'s compile-type class is EXACT and registers `<name>$get`. `None` ⇒ a
    /// stored field (`GetField`), or a hook only the runtime class decides ([`Self::hook_dynamic`]) —
    /// which covers an extended class too, since a subclass does not inherit its parent's hook in the
    /// oracle (plan row 4b; KNOWN_ISSUES HOOK-INHERITANCE).
    pub(in crate::compiler) fn hook_get_method(&self, object: &Expr, name: &str) -> Option<String> {
        self.hook_static(object, name, "get")
    }

    /// The write twin of [`Self::hook_get_method`]: `<name>$set` decided at compile time.
    pub(in crate::compiler) fn hook_set_method(&self, object: &Expr, name: &str) -> Option<String> {
        self.hook_static(object, name, "set")
    }

    fn hook_static(&self, object: &Expr, name: &str, kind: &str) -> Option<String> {
        let m = format!("{name}${kind}");
        let Ok(CTy::Class(cls)) = self.ctype(object) else {
            return None;
        };
        (self.is_exact_class(&cls) && self.method_rets.contains_key(&(cls, m.clone()))).then_some(m)
    }

    /// Plan row 4b: the `(field, hook)` name indices when `object.name` must be decided at RUN time —
    /// some class hooks `name` (`<name>$<kind>`), yet the receiver's compile-time class is not exact
    /// ([`Self::hook_static`] ran first), so the runtime class decides. `None` ⇒ the plain
    /// `GetField` / `SetField` is exact.
    pub(in crate::compiler) fn hook_dynamic(
        &self,
        object: &Expr,
        name: &str,
        kind: &str,
    ) -> Option<(usize, usize)> {
        let m = format!("{name}${kind}");
        let hooked = match self.ctype(object) {
            // Only the classes a value typed `cls` can be: `cls` itself or a descendant (an exact
            // class with the hook took [`Self::hook_static`] already).
            Ok(CTy::Class(cls))
                if self
                    .class_parents_all
                    .is_some_and(|ps| ps.contains_key(&cls)) =>
            {
                self.hooked_at_or_below(&cls, &m)
            }
            // A trait (`this` in a trait body): any using class, or a subclass of one, may be the
            // receiver. An enum or other non-class type has no hook to find.
            Ok(CTy::Class(cls)) => self.classes.contains_key(&cls) && self.any_class_has(&m),
            _ => self.any_class_has(&m),
        };
        if !hooked {
            return None;
        }
        Some((
            self.field_name_index(name).ok()?,
            self.field_name_index(&m).ok()?,
        ))
    }

    /// Plan row 4b: `class` is EXACT when it is a class (not a trait, whose `this` is any using
    /// class) and no class in the program extends it, so a value typed as `class` is an instance of
    /// `class` itself and compile-time dispatch on it is sound.
    pub(in crate::compiler) fn is_exact_class(&self, class: &str) -> bool {
        self.class_parents_all.is_some_and(|m| {
            m.contains_key(class) && !m.values().any(|ps| ps.iter().any(|p| p == class))
        })
    }

    /// Whether `cls` or a class that transitively extends it declares the synthetic hook method
    /// `hook` (a `use`d trait's hook is registered under the using class).
    fn hooked_at_or_below(&self, cls: &str, hook: &str) -> bool {
        self.method_rets
            .keys()
            .any(|(c, m)| m == hook && self.extends_or_is(c, cls))
    }

    /// Whether class `c` is `ancestor` or reaches it through `extends`. Visited-guarded, so an
    /// `extends` cycle (a checker error) cannot loop the compiler.
    fn extends_or_is(&self, c: &str, ancestor: &str) -> bool {
        let mut todo = vec![c];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(k) = todo.pop() {
            if k == ancestor {
                return true;
            }
            if seen.insert(k) {
                if let Some(ps) = self.class_parents_all.and_then(|m| m.get(k)) {
                    todo.extend(ps.iter().map(String::as_str));
                }
            }
        }
        false
    }

    /// Whether any class in the program declares the synthetic hook method `hook`.
    fn any_class_has(&self, hook: &str) -> bool {
        self.method_rets.keys().any(|(_, m)| m == hook)
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
