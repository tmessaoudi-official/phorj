//! The compile-time operand-type (`CTy`) resolver + static/const/hook lookups. ⚠ CTy-
//! operand trap (Invariant 7): un-rejecting an operand expression form REQUIRES an arm here.

use super::*;

impl Compiler<'_> {
    /// DEC-302: the contiguous `enum_descs` range `(start, count)` for enum `name`, or `None` when
    /// `name` is not a declared enum. Variants are emitted contiguously per enum in the compiler
    /// pre-pass, so the range is a single scan (drives `cases()` inlining + `Op::EnumFrom`).
    pub(in crate::compiler) fn enum_desc_range(&self, name: &str) -> Option<(usize, usize)> {
        let start = self.enum_descs.iter().position(|d| d.ty.as_ref() == name)?;
        let count = self.enum_descs[start..]
            .iter()
            .take_while(|d| d.ty.as_ref() == name)
            .count();
        Some((start, count))
    }

    /// DEC-302: the operand `CTy` of a backed enum's `.value` (`Int`/`Str`), or `None` when `name`
    /// is not a backed enum. Lets `s.value` / `from(x).value` specialize as an operand (Invariant 7).
    pub(in crate::compiler) fn enum_backing_cty(&self, name: &str) -> Option<CTy> {
        self.enum_descs
            .iter()
            .find(|d| d.ty.as_ref() == name && d.backing.is_some())
            .and_then(|d| d.backing.as_ref())
            .map(|v| match v {
                Value::Int(_) => CTy::Int,
                Value::Str(_) => CTy::Str,
                _ => CTy::Other,
            })
    }

    /// DEC-302: `true` when `object` is a backed-enum-typed receiver (so `object.value` lowers to
    /// `Op::EnumValue`, not `GetField`). A backed enum's `CTy` is `CTy::Class(name)` (`resolve_cty`'s
    /// nominal fallback), disambiguated from a real class by `enum_backing_cty`.
    pub(in crate::compiler) fn is_backed_enum_receiver(&self, object: &Expr) -> bool {
        matches!(self.ctype(object), Ok(CTy::Class(n)) if self.enum_backing_cty(&n).is_some())
    }

    /// Infer whether an arithmetic operand is int- or float-typed, to pick the specialized op
    /// (decision P2-6). Only reached for operands of `+ - * / %`, which the checker guarantees are
    /// numeric. The numeric projection of `ctype` (M2 Wave 4): `ctype` resolves the operand's full
    /// class-aware type and `as_num` narrows it. The error wording matches the pre-Wave-4 paths (a
    /// checker-unreachable surface — no test depends on it — kept faithful regardless).
    pub(in crate::compiler) fn num_ty(&self, e: &Expr) -> Result<NumTy, String> {
        let cty = self.ctype(e)?;
        Self::as_num(&cty).ok_or_else(|| match e {
            Expr::Ident(name, _) => format!("`{name}` is not numeric"),
            Expr::Call { callee, .. } => match &**callee {
                Expr::Ident(name, _) => format!("`{name}` does not return a numeric type"),
                _ => format!("cannot infer numeric type of {e:?}"),
            },
            _ => format!("cannot infer numeric type of {e:?}"),
        })
    }

    /// Resolve an expression's class-aware type (M2 Wave 4), mirroring `expr`'s resolution order so
    /// a field read / method result / nested member / class-typed payload each resolve once,
    /// recursively. Generalizes the old per-arm `num_ty`: an `Ident` resolves through a `match`-arm
    /// binding, then a local, then a bare field of `this`; `This` is the current class; `Member`
    /// walks the object's class to the field's type; a `Call` resolves to a function/constructor or
    /// method return type. Anything it can name but isn't numeric/class collapses to `Other`; only a
    /// genuinely unresolvable operand errors (the same surface that errored pre-Wave-4).
    pub(in crate::compiler) fn ctype(&self, e: &Expr) -> Result<CTy, String> {
        // S2.1-broad reified-operand side-table — consulted as a **FALLBACK**, not first. The normal
        // class-aware resolution wins whenever it yields a concrete operand type, so a correctly-typed
        // field/method read is never overridden. Why fallback (not first): the map is keyed only by
        // `span.start`, and an injected prelude (parsed from its own 0-based source string, then
        // prepended) can share a `span.start` with a user expression — consulting it first let a user
        // entry hijack a prelude `this.field` read, a spurious "cannot infer numeric type" (the
        // `datetimes.phg` regression). Reified now applies ONLY when the normal path can't resolve the
        // operand (an erased generic — `box.get() + 1`, `box.value + 1`, a `List<T>`/`Map` return —
        // which collapses to `Other`), exactly what the side-table was built for, so the generic
        // specialization is unaffected. Empty on the run-family `compile` path ⇒ zero overhead.
        let normal = self.ctype_normal(e);
        if matches!(normal, Ok(ref c) if !matches!(c, CTy::Other)) {
            return normal;
        }
        if !self.reified_operands.is_empty() {
            let key = match e {
                Expr::Call { span, .. } | Expr::Member { span, .. } | Expr::Index { span, .. } => {
                    Some(span.start)
                }
                _ => None,
            };
            if let Some(cty) = key.and_then(|k| self.reified_operands.get(&k)) {
                return Ok(cty.clone());
            }
        }
        normal
    }

    /// The class-aware operand resolution proper; the reified-operand fallback in [`ctype`] wraps it.
    pub(in crate::compiler) fn ctype_normal(&self, e: &Expr) -> Result<CTy, String> {
        match e {
            Expr::Int(..) => Ok(CTy::Int),
            Expr::Float(..) => Ok(CTy::Float),
            Expr::Decimal { .. } => Ok(CTy::Decimal),
            // A string literal (incl. an interpolated one — both are `Expr::Str`) is `CTy::Str` so a
            // `"a" + s` concat lowers to `Op::Concat`; `bool`/`bytes` literals are non-operands.
            Expr::Str(..) => Ok(CTy::Str),
            Expr::Bool(..) | Expr::Bytes(..) => Ok(CTy::Other),
            // A list literal's element type comes from its first element (empty → `Other`), so an
            // index into it (`[1, 2, 3][0] + 1`) resolves as an operand (M3 S1.1).
            Expr::List(elems, _) => Ok(CTy::List(Box::new(
                elems
                    .first()
                    .and_then(|el| self.ctype(el).ok())
                    .unwrap_or(CTy::Other),
            ))),
            // `xs[i]` resolves to the list's element type (so `xs[0] + 1` specializes); a non-list
            // receiver collapses to `Other` (checker-unreachable as an arithmetic operand).
            Expr::Index { object, .. } => match self.ctype(object)? {
                CTy::List(elem) => Ok(*elem),
                CTy::Map(_, val) => Ok(*val), // `m[k]` resolves to the value type (M-RT S3)
                _ => Ok(CTy::Other),
            },
            // A map literal's key/value types come from its first pair (≥1, parser-guaranteed), so a
            // `var m = ["a" => 1]; m["a"] + 1` specializes the arithmetic (M-RT S3).
            Expr::Map(pairs, _) => {
                let (k0, v0) = &pairs[0];
                Ok(CTy::Map(
                    Box::new(self.ctype(k0).unwrap_or(CTy::Other)),
                    Box::new(self.ctype(v0).unwrap_or(CTy::Other)),
                ))
            }
            Expr::Ident(name, _) => {
                if let Some(b) = self.match_bindings.iter().rev().find(|b| b.name == *name) {
                    Ok(b.ty.clone())
                } else if let Some(s) = self.resolve_local(name) {
                    Ok(self.locals[s].ty.clone())
                } else if let Some(t) = self.field_tags.get(name) {
                    Ok(t.clone())
                } else if let Some(meta) = self.fns.get(name) {
                    // A bare named-function reference in value position (e.g. `var f = dbl`) is a
                    // function value, so `f(x)` dispatches through `CallValue` like a lambda local.
                    Ok(CTy::Fn {
                        params: meta.params.clone(),
                        ret: Box::new(meta.ret.clone()),
                    })
                } else {
                    Err(format!("undefined variable `{name}`"))
                }
            }
            Expr::This(_) => match &self.cur_class {
                Some(c) => Ok(CTy::Class(c.clone())),
                None => Err("`this` used outside a method".into()),
            },
            Expr::Member { object, name, .. } => {
                // A `const` class constant resolves to its declared operand `CTy` (Feature A) — checked
                // first (before statics and `ctype(object)`, which would reject the bare class name).
                if let Some(cty) = self.const_cty(object, name) {
                    return Ok(cty);
                }
                // Static read `ClassName.field` resolves to the static's declared `CTy` (M-mut.7) —
                // checked first, since `ctype(object)` would reject the bare class name.
                if let Some(cty) = self.static_cty(object, name) {
                    return Ok(cty);
                }
                let obj_cty = self.ctype(object);
                // DEC-302: `s.value` on a backed enum resolves to its backing operand type (`Int`/
                // `Str`), so `from(9).value + 1` specializes — without this the VM rejects what the
                // interpreter accepts (the CTy-operand trap, Invariant 7).
                if name == "value" {
                    if let Ok(CTy::Class(n)) = &obj_cty {
                        if let Some(bt) = self.enum_backing_cty(n) {
                            return Ok(bt);
                        }
                    }
                }
                // A property hook read `o.name` (M-mut.7b): its operand type is the `<name>$get`
                // method's return type. Resolved before the field path so `o.fahrenheit + 1.0`
                // specializes — without it the VM would reject what the interpreter accepts (the
                // documented CTy-operand trap).
                if let Ok(CTy::Class(cls)) = &obj_cty {
                    if let Some(cty) = self.method_rets.get(&(cls.clone(), format!("{name}$get"))) {
                        return Ok(cty.clone());
                    }
                }
                match obj_cty? {
                    CTy::Class(cls) => self
                        .class_field_ctys
                        .get(&cls)
                        .and_then(|fs| fs.get(name))
                        .cloned()
                        .ok_or_else(|| format!("no field `{name}` on `{cls}`")),
                    _ => Err(format!("cannot infer type of field `{name}`")),
                }
            }
            Expr::Call { callee, args, .. } => match &**callee {
                Expr::Ident(name, _) => {
                    if let Some(meta) = self.fns.get(name) {
                        // An erased generic whose result echoes argument `i` (`id<T>(T x) -> T`):
                        // recover the operand type from that argument so `id(7) + 1` specializes on
                        // the VM exactly as the interpreter evaluates it (S2.1). Falls back to the
                        // (erased → `Other`) declared return for any other shape.
                        if let Some(i) = meta.generic_ret_from_param {
                            if let Some(arg) = args.get(i) {
                                return self.ctype(arg);
                            }
                        }
                        Ok(meta.ret.clone())
                    } else if self.classes.contains_key(name) {
                        Ok(CTy::Class(name.clone())) // a constructor returns its instance
                    } else if self.variants.contains_key(name) {
                        Ok(CTy::Other) // an enum value: not numeric, not a class we track fields of
                    } else if let Some(slot) = self.resolve_local(name) {
                        // A function-value local (lambda): the call result is the lambda's ret type.
                        match &self.locals[slot].ty {
                            CTy::Fn { ret, .. } => Ok(*ret.clone()),
                            _ => Err(format!("cannot infer numeric type of {e:?}")),
                        }
                    } else {
                        Err(format!("cannot infer numeric type of {e:?}"))
                    }
                }
                // Native module-qualified call (`List.length(xs)`, `Text.parseInt(s)`, …): resolve to
                // the native's return operand type. Checked BEFORE `ctype(object)` — the qualifier is a
                // bare module name (`List`), not a value, so `ctype(Ident("List"))` would error. Mirror
                // the emit-path guard in `compiler/expr.rs`: a head that is not a local/match-binding,
                // resolvable via `index_of_qualified` (import-map-first, Native-excluded leaf
                // fallback — same rule as the emit path). Without this, `List.length(xs) - 1` compiles on
                // the interpreter but the VM rejects it ("undefined variable `List`") — a parity break.
                Expr::Member {
                    object,
                    name,
                    safe: false,
                    ..
                } if matches!(&**object, Expr::Ident(q, _)
                    if self.resolve_local(q).is_none() && self.resolve_binding(q).is_none()
                        && crate::native::index_of_qualified(self.imports, q, name).is_some()) =>
                {
                    let Expr::Ident(q, _) = &**object else {
                        unreachable!("guard above already matched `object` as `Expr::Ident`")
                    };
                    Ok(native_ret_cty(
                        crate::native::index_of_qualified(self.imports, q, name)
                            .expect("guard above already resolved this qualified native"),
                    ))
                }
                // DEC-302 enum static methods `Enum.from(x)` / `Enum.tryFrom(x)` / `Enum.cases()`: the
                // head is a bare enum name (not a value), so resolve directly. `from`/`tryFrom` yield
                // the enum type (`CTy::Class(enum)` — an optional carries its inner CTy, so
                // `tryFrom(x)!.value` specializes); `cases()` yields `List<enum>`. Without this,
                // `Enum.from(9).value + 1` gets `Other` and the VM rejects the operand (Invariant 7).
                Expr::Member {
                    object,
                    name,
                    safe: false,
                    ..
                } if matches!(&**object, Expr::Ident(en, _)
                    if self.resolve_local(en).is_none() && self.resolve_binding(en).is_none()
                        && self.enum_desc_range(en).is_some()
                        && matches!(name.as_str(), "cases" | "from" | "tryFrom")) =>
                {
                    let Expr::Ident(en, _) = &**object else {
                        unreachable!("guard above already matched `object` as `Expr::Ident`")
                    };
                    Ok(match name.as_str() {
                        "cases" => CTy::List(Box::new(CTy::Class(en.clone()))),
                        _ => CTy::Class(en.clone()), // from / tryFrom
                    })
                }
                // Static method call `ClassName.method(args)` (slice B0 / Statics): the head is a bare
                // class name, not a value, so `ctype(object)` would reject it — resolve directly via
                // `method_rets[(class, method)]`. Without this, `var f = Router.compose(...)` gets
                // `CTy::Other` and a later `f(x)` is rejected on the VM as "not a function" — a parity
                // break (the same CTy-operand/fn-value trap as the native arm above).
                Expr::Member {
                    object,
                    name,
                    safe: false,
                    ..
                } if matches!(&**object, Expr::Ident(cls, _)
                    if self.resolve_local(cls).is_none() && self.resolve_binding(cls).is_none()
                        && self.classes.contains_key(cls)) =>
                {
                    let Expr::Ident(cls, _) = &**object else {
                        unreachable!("guard above already matched `object` as `Expr::Ident`")
                    };
                    self.method_rets
                        .get(&(cls.clone(), name.clone()))
                        .cloned()
                        .ok_or_else(|| format!("no static method `{name}` on `{cls}`"))
                }
                // Method call: the return type is keyed on the receiver's runtime class.
                Expr::Member { object, name, .. } => match self.ctype(object)? {
                    CTy::Class(cls) => {
                        // S2.1 (methods): a generic method whose result echoes one of its own params
                        // (`pick<T>(T a, T b) -> T`) is erased to `Other` in `method_rets`; recover the
                        // operand type from the echoed argument so `u.pick(7, 8) + 1` specializes on the
                        // VM exactly as the interpreter evaluates it. Falls through to the (erased)
                        // declared return for any other shape.
                        if let Some(i) = self
                            .method_generic_ret_from_param
                            .get(&(cls.clone(), name.clone()))
                            .copied()
                        {
                            if let Some(arg) = args.get(i) {
                                return self.ctype(arg);
                            }
                        }
                        self.method_rets
                            .get(&(cls.clone(), name.clone()))
                            .cloned()
                            .ok_or_else(|| format!("no method `{name}` on `{cls}`"))
                    }
                    _ => Err(format!("cannot infer numeric type of {e:?}")),
                },
                _ => Err(format!("cannot infer numeric type of {e:?}")),
            },
            Expr::Unary { expr, .. } => self.ctype(expr),
            Expr::Binary { lhs, .. } => self.ctype(lhs),
            // `value instanceof C` is a `bool` — never an arithmetic operand, but a `var b = …`
            // initializer reads `ctype`, so resolve it to `Other` rather than erroring.
            Expr::InstanceOf { .. } => Ok(CTy::Other),
            // `inner!` unwraps `T?` to `T`; its operand type is the inner's (so `o! + 1` specializes
            // — `resolve_cty(Optional)` already yields the inner `CTy`). M3 S2.5.
            Expr::Force { inner, .. } => self.ctype(inner),
            // `expr?` unwraps a `Result<T, E>` to its `Ok` payload — generally an erased/unknown
            // operand type, so it is not a specialized arithmetic operand (M-faults 2a).
            Expr::Propagate { .. } => Ok(CTy::Other),
            // `obj with { … }` yields a fresh instance of `obj`'s class — same compile-type as `obj`.
            Expr::CloneWith { object, .. } => self.ctype(object),
            // A `match` value's type is its arms' shared type (checker-guaranteed); infer it from
            // the first arm's body so `var x = match … { … }` specializes like an explicit local.
            Expr::Match { arms, .. } => match arms.first() {
                Some(arm) => self.ctype(&arm.body),
                None => Ok(CTy::Other),
            },
            // A range materializes to `List<int>`, so its compile-type is `List(Int)` — carrying the
            // element type lets `(0..n)[i] + 1` (or a range bound to a `var`, then indexed) specialize.
            Expr::Range { .. } => Ok(CTy::List(Box::new(CTy::Int))),
            // Both `if` branches share a type (checker-guaranteed); infer it from the then-branch so
            // `var x = if (c) { 1 } else { 2 }` specializes arithmetic on `x` (like `Match`).
            Expr::If { then_expr, .. } => self.ctype(then_expr),
            // A lambda's compile-time type reflects its declared params and return type so that
            // a `var f = function(int x) => x + 1` local later resolves calls on `f` to `CallValue`.
            Expr::Lambda { params, ret, .. } => Ok(CTy::Fn {
                params: params.iter().map(|p| resolve_cty(&p.ty)).collect(),
                ret: Box::new(ret.as_ref().map_or(CTy::Other, resolve_cty)),
            }),
            // A `parent.m(…)` / `parent(A).m(…)` result resolves to the target method's return type
            // (M-RT super/parent) — keyed on the resolved declaring class — so a parent call used as an
            // arithmetic operand (`parent.combine(…) + 1`) specializes on the VM, matching the
            // interpreter (the documented CTy-operand parity trap).
            Expr::ParentCall {
                ancestor, method, ..
            } => Ok(self.parent_ret_cty(ancestor.as_deref(), method)),
            // `spawn <call>` is a `Task<T>` handle (M6 W4). Modeled as `CTy::Class("Task")` (the
            // reserved built-in) so `var t = spawn f(); t.join()` dispatches the `Op::Join` lowering —
            // without it the instance-method path would not recognize the receiver. (The payload type
            // `T` is not carried, so a `t.join()`/`ch.receive()` result is not specialized to `AddI`/etc.
            // when used directly as an arithmetic operand — it still runs correctly via the polymorphic
            // arithmetic path, only without the int/float fast op; byte-identity is unaffected.)
            Expr::Spawn { .. } => Ok(CTy::Class("Task".to_string())),
            other => Err(format!("cannot infer numeric type of {other:?}")),
        }
    }

    /// Numeric refinement of a `CTy` — the bridge from "what type the operand is" to "which
    /// specialized arithmetic op." `None` for non-numeric types (a defensive path: the checker
    /// already guarantees arithmetic operands are numeric).
    pub(in crate::compiler) fn as_num(ty: &CTy) -> Option<NumTy> {
        match ty {
            CTy::Int => Some(NumTy::Int),
            CTy::Float => Some(NumTy::Float),
            CTy::Decimal => Some(NumTy::Decimal),
            CTy::Str
            | CTy::Class(_)
            | CTy::Other
            | CTy::List(_)
            | CTy::Map(..)
            | CTy::Fn { .. } => None,
        }
    }

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
