//! Expression checking — unary/binary operators, `is`, casts.

use super::*;

impl Checker {
    pub(in crate::checker) fn check_unary(
        &mut self,
        op: crate::ast::UnaryOp,
        expr: &crate::ast::Expr,
        span: Span,
    ) -> Ty {
        use crate::ast::UnaryOp;
        let t = self.check_expr(expr);
        if t == Ty::Error {
            return Ty::Error;
        }
        match op {
            UnaryOp::Neg if t == Ty::Int || t == Ty::Float || t == Ty::Decimal => t,
            UnaryOp::Neg => self.err(
                span,
                format!("unary `-` requires int, float, or decimal, found `{t}`"),
            ),
            UnaryOp::Not if t == Ty::Bool => Ty::Bool,
            UnaryOp::Not => self.err(span, format!("unary `!` requires `bool`, found `{t}`")),
            UnaryOp::BitNot if t == Ty::Int => Ty::Int,
            UnaryOp::BitNot => self.err(span, format!("unary `~` requires `int`, found `{t}`")),
        }
    }

    pub(in crate::checker) fn check_binary(
        &mut self,
        op: crate::ast::BinaryOp,
        lhs: &crate::ast::Expr,
        rhs: &crate::ast::Expr,
        span: Span,
    ) -> Ty {
        use crate::ast::BinaryOp;
        let l = self.check_expr(lhs);
        let r = self.check_expr(rhs);
        if l == Ty::Error || r == Ty::Error {
            return match op {
                BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::Gt
                | BinaryOp::Le
                | BinaryOp::Ge
                | BinaryOp::And
                | BinaryOp::Or => Ty::Bool,
                _ => Ty::Error,
            };
        }
        match op {
            // `+` is overloaded for string concatenation (Phase 1 string slice): `string + string`
            // → `string`. It is type-directed with **no coercion** — `string + int` stays an error
            // (the typed system kills JS's `"1" + 1` footgun). Only `+` concatenates; `-`/`*`/`/`/`%`
            // remain numeric-only.
            BinaryOp::Add if l == Ty::String && r == Ty::String => Ty::String,
            // `decimal` arithmetic. All of `+ - * / %` over decimals yield `decimal`; `decimal ⊕ int`
            // (either order) widens the int and stays `decimal` — the one ergonomic coercion (qty
            // math). `/` is *exact-or-fault* at runtime (a non-terminating quotient like `1d/3d`
            // faults; use `Decimal.div(a, b, scale, mode)` for a rounded one) and `%` is the exact
            // remainder — both type as `decimal` here. A `decimal ⊕ float` mix is the bug this
            // primitive prevents (`E-DECIMAL-FLOAT-MIX`). Checked before the int/float arm so a decimal
            // operand never reaches the matching-int-or-float test.
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                if l == Ty::Decimal || r == Ty::Decimal =>
            {
                let other = if l == Ty::Decimal { &r } else { &l };
                match other {
                    Ty::Decimal | Ty::Int => Ty::Decimal,
                    Ty::Float => self.err_coded(
                        span,
                        "cannot mix `decimal` and `float` in arithmetic — they are distinct types"
                            .to_string(),
                        "E-DECIMAL-FLOAT-MIX",
                        Some(
                            "convert explicitly (a `float` literal `1.5` and a `decimal` `1.50d` are \
                             different); keep money math in `decimal`"
                                .into(),
                        ),
                    ),
                    _ => self.err(
                        span,
                        format!("`decimal` arithmetic requires a `decimal` or `int` operand, found `{l}` and `{r}`"),
                    ),
                }
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                if (l == Ty::Int && r == Ty::Int) || (l == Ty::Float && r == Ty::Float) {
                    l
                } else if op == BinaryOp::Add && (l == Ty::String || r == Ty::String) {
                    self.err(span, format!("`+` concatenates two `string`s or adds two numbers — no coercion; found `{l}` and `{r}`"))
                } else {
                    self.err(span, format!("arithmetic requires matching int or float operands, found `{l}` and `{r}`"))
                }
            }
            // `**` power is type-directed like the other arithmetic ops but never concatenates:
            // `int**int→int`, `float**float→float`, anything else is an error (no coercion).
            BinaryOp::Pow => {
                if (l == Ty::Int && r == Ty::Int) || (l == Ty::Float && r == Ty::Float) {
                    l
                } else {
                    self.err(
                        span,
                        format!(
                            "`**` requires matching int or float operands, found `{l}` and `{r}`"
                        ),
                    )
                }
            }
            BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
                // `decimal` compares against `decimal` or `int` (numeric, scale-insensitive); same
                // operand rule as decimal arithmetic, a `float` mix is `E-DECIMAL-FLOAT-MIX`.
                let dec_ok = (l == Ty::Decimal && (r == Ty::Decimal || r == Ty::Int))
                    || (r == Ty::Decimal && l == Ty::Int);
                if (l == Ty::Int && r == Ty::Int) || (l == Ty::Float && r == Ty::Float) || dec_ok {
                    Ty::Bool
                } else if l == Ty::Decimal || r == Ty::Decimal {
                    self.err_coded(
                        span,
                        format!(
                            "cannot compare `decimal` with `{}`",
                            if l == Ty::Decimal { &r } else { &l }
                        ),
                        "E-DECIMAL-FLOAT-MIX",
                        Some("compare a `decimal` only with another `decimal` or an `int`".into()),
                    );
                    Ty::Bool
                } else {
                    self.err(span, format!("comparison requires matching int or float operands, found `{l}` and `{r}`"));
                    Ty::Bool
                }
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                // `decimal == int` (either order) is numeric equality (the operator-level int-widen);
                // every other cross-type pairing still requires explicit conversion.
                let dec_int =
                    (l == Ty::Decimal && r == Ty::Int) || (l == Ty::Int && r == Ty::Decimal);
                if l != r && !dec_int {
                    self.err(
                        span,
                        format!(
                            "cross-type comparison requires explicit conversion (`{l}` vs `{r}`)"
                        ),
                    );
                }
                Ty::Bool
            }
            BinaryOp::And | BinaryOp::Or => {
                if l != Ty::Bool || r != Ty::Bool {
                    self.err(
                        span,
                        format!("`&&`/`||` require `bool` operands, found `{l}` and `{r}`"),
                    );
                }
                Ty::Bool
            }
            BinaryOp::Coalesce => {
                match &l {
                    Ty::Error => Ty::Error,
                    Ty::Null => r.clone(), // `null ?? b` is always `b`
                    Ty::Optional(inner) => {
                        let inner = (**inner).clone();
                        if self.ty_assignable(&r, &inner) {
                            inner // `a ?? b` yields the unwrapped `T` when the default is a `T`
                        } else {
                            if !self.ty_assignable(&r, &Ty::Optional(Box::new(inner.clone()))) {
                                self.err(
                                span,
                                format!("`??` default of type `{r}` is not compatible with `{inner}?`"),
                            );
                            }
                            Ty::Optional(Box::new(inner)) // both sides optional → stays `T?`
                        }
                    }
                    other => self.err(
                        span,
                        format!("left operand of `??` must be optional, found `{other}`"),
                    ),
                }
            }
            BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr => {
                if l == Ty::Int && r == Ty::Int {
                    Ty::Int
                } else {
                    self.err(
                        span,
                        format!("bitwise operators require `int` operands, found `{l}` and `{r}`"),
                    )
                }
            }
            BinaryOp::Pipe => unreachable!("`|>` is lowered to a call in the parser"),
        }
    }

    /// `value instanceof TypeName` (M-RT S1): a runtime type test that always yields `bool`. The
    /// right operand must name a known class **or interface** (M-RT S2); the left operand must be a
    /// class instance (a `Ty::Named`). The smart-cast that narrows the operand inside an `if`
    /// then-block lives in `check_stmt`'s `Stmt::If` arm (it needs the surrounding block), not here.
    pub(in crate::checker) fn check_instanceof(
        &mut self,
        value: &crate::ast::Expr,
        type_name: &str,
        span: Span,
    ) -> Ty {
        let v = self.check_expr(value);
        // Slice 3 (DEC-184): `is`/`instanceof` test a discriminable PRIMITIVE (`x is int`) as well as
        // a class/interface. A primitive RHS is a runtime `Value`-variant test (interpreter/VM) →
        // `is_int`/`is_float`/`is_string`/`is_bool`/`is_null` in the transpiled leg — byte-identical,
        // and it accepts any left operand (a primitive value flows in), so it returns before the
        // class-operand check below.
        if let Some(prim) = prim_pat_ty(type_name) {
            // Byte-identity guard (mirrors `match`'s E-MATCH-ERASED-AMBIG): a `string` test over a
            // union (or optional-union) that also holds a PHP-string-erased sibling (decimal/bytes/
            // html/attr) can't be told apart by the transpiled `is_string()`.
            if matches!(prim, Ty::String) {
                if let Some(members) = union_members_of(&v) {
                    if members
                        .iter()
                        .any(|m| !matches!(m, Ty::String) && erases_to_php_string(m))
                    {
                        self.err_coded(
                            span,
                            "type test `string` is ambiguous here — the operand's type also holds a type that erases to a PHP string (decimal/bytes/html/attr), so the transpiled PHP can't distinguish them".to_string(),
                            "E-MATCH-ERASED-AMBIG",
                            Some("split the type or test a more specific one — run/runvm could tell them apart, but the PHP leg cannot".into()),
                        );
                    }
                }
            }
            return Ty::Bool;
        }
        // decimal/bytes/html/attr erase to a PHP `string`, so a runtime test can't be byte-identical
        // (LADDER-forced reject, mirroring E-MATCH-TYPE-ERASED for `match` type-patterns).
        if matches!(type_name, "decimal" | "bytes" | "html" | "attr") {
            return self.err_coded(
                span,
                format!("type test `{type_name}` can't be runtime-discriminated — it erases to a PHP string; only int/float/string/bool/null and classes/interfaces can be tested"),
                "E-MATCH-TYPE-ERASED",
                Some("test its wrapping form, or use a class/interface".into()),
            );
        }
        // M-RT S8: a trait is reuse, not a type — it cannot be an `instanceof` target (it is collected
        // into `classes` for member lookup, so this explicit guard is needed before the class check).
        if self.traits.contains(type_name) {
            return self.err_coded(
                span,
                format!("`{type_name}` is a trait, not a type"),
                "E-INSTANCEOF-TYPE",
                Some("a trait is reuse, not a type; test against a class or interface".into()),
            );
        }
        if !self.classes.contains_key(type_name) && !self.interfaces.contains_key(type_name) {
            return self.err_coded(
                span,
                format!(
                    "type test requires a class, interface, or discriminable primitive (int/float/string/bool/null) on the right, found `{type_name}`"
                ),
                "E-INSTANCEOF-TYPE",
                Some("only a declared class/interface or a discriminable primitive can be tested with `is`/`instanceof`".into()),
            );
        }
        match &v {
            // A poisoned operand already reported its own error; still type the test as `bool`.
            Ty::Error => {}
            // A class instance — or a union (M-RT S4) / intersection (M-RT S5) of them — is the
            // meaningful left operand.
            Ty::Named(..) | Ty::Union(..) | Ty::Intersection(..) => {}
            other => {
                self.err_coded(
                    span,
                    format!("`instanceof` left operand must be a class instance, found `{other}`"),
                    "E-INSTANCEOF-TYPE",
                    Some("`instanceof` tests whether a class instance is of a given class".into()),
                );
            }
        }
        Ty::Bool
    }

    /// `value as TypeName` — the checked downcast (M4 casting axis 2). Validates the same operands as
    /// `instanceof` (a class/union/intersection value on the left; a class or interface name on the
    /// right) but types the result `TypeName?` (the cast yields the value when it really is a
    /// `TypeName` at runtime, else `null`). A *primitive* `as` (e.g. `x as int`) is rejected with a
    /// hint toward `Core.Conversion`/`parse*` — value conversion is a different axis from type assertion.
    pub(in crate::checker) fn check_cast(
        &mut self,
        value: &crate::ast::Expr,
        type_name: &str,
        span: Span,
    ) -> Ty {
        let v = self.check_expr(value);
        // M4 as-matrix: a primitive target is a value CONVERSION (or identity), not a class downcast.
        // `value as int/float/string/bool/decimal` is typed here (and conversions are rewritten to a
        // native call), so it never falls into the class/interface path below.
        if matches!(type_name, "int" | "float" | "string" | "bool" | "decimal") {
            return self.check_cast_primitive(value, &v, type_name, span);
        }
        // A trait is reuse, not a type — same guard as `instanceof`.
        if self.traits.contains(type_name) {
            return self.err_coded(
                span,
                format!("`{type_name}` is a trait, not a type"),
                "E-CAST-TYPE",
                Some("a trait is reuse, not a type; cast to a class or interface".into()),
            );
        }
        let is_class = self.classes.contains_key(type_name);
        if !is_class && !self.interfaces.contains_key(type_name) {
            // Tailor the hint for a primitive target: `as` is *assertion*, not conversion.
            let hint = if is_builtin_type_name(type_name) {
                "`as` is a checked downcast, not a value conversion — use `Core.Conversion` (e.g. \
                 `Conversion.toFloat`/`truncate`) or `Core.String.parseInt`/`parseFloat` to change a value's type"
            } else {
                "only a declared class or interface can be a cast target"
            };
            return self.err_coded(
                span,
                format!(
                    "`as` requires a class or interface name on the right, found `{type_name}`"
                ),
                "E-CAST-TYPE",
                Some(hint.into()),
            );
        }
        match &v {
            // A poisoned operand already reported; still type the cast as `TypeName?`.
            Ty::Error => {}
            // A class instance — or a union (S4) / intersection (S5) of them — is the meaningful left
            // operand (same surface as `instanceof`).
            Ty::Named(..) | Ty::Union(..) | Ty::Intersection(..) => {}
            other => {
                self.err_coded(
                    span,
                    format!("`as` left operand must be a class instance, found `{other}`"),
                    "E-CAST-TYPE",
                    Some(
                        "`as` downcasts a class instance to a more specific class or interface"
                            .into(),
                    ),
                );
            }
        }
        // Result is `TypeName?`. A generic class carries erased (poison) args — `instanceof`/`as` see
        // no runtime type arguments (`x as Box` ≡ `x as Box<…erased…>`), mirroring the narrow logic.
        let arity = self
            .classes
            .get(type_name)
            .map_or(0, |c| c.type_params.len());
        Ty::Optional(Box::new(Ty::Named(
            type_name.to_string(),
            vec![Ty::Error; arity],
        )))
    }

    /// `value as <primitive>` (M4 as-matrix, S1 — concrete-primitive sources). Types the result per
    /// the **Unified, fallibility-typed** model (lossless → total `T`, lossy/fallible → `T?`) and, for
    /// a real conversion, records a span-keyed rewrite to a leaf-qualified native call
    /// (`Conversion.toFloat(v)` / `String.parseInt(v)` …) that the backends resolve by `index_of_by_leaf`
    /// without an import. **Identity** (`T as T`) is total, fires `W-REDUNDANT-CAST`, and is NOT
    /// rewritten — the `Cast` node survives and each backend emits the value unchanged. Bool cells,
    /// `float as decimal`, `string as decimal`, and union/erased *assertion* sources land in later
    /// slices (rejected for now with a forward-looking hint). No new `Op`/`Value`.
    pub(in crate::checker) fn check_cast_primitive(
        &mut self,
        value: &crate::ast::Expr,
        v: &Ty,
        target: &str,
        span: Span,
    ) -> Ty {
        let target_ty = match target {
            "int" => Ty::Int,
            "float" => Ty::Float,
            "string" => Ty::String,
            "bool" => Ty::Bool,
            "decimal" => Ty::Decimal,
            _ => unreachable!("guarded by the caller"),
        };
        // Identity — already the target type. Total, no rewrite, redundant-cast lint.
        if *v == target_ty {
            self.warn_coded(
                span,
                format!("redundant cast: value is already `{target}`"),
                "W-REDUNDANT-CAST",
                Some("remove the `as` — the value already has this type".into()),
            );
            return target_ty;
        }
        // A poisoned operand already reported; type the cast as the (likely) target to avoid a cascade.
        if matches!(v, Ty::Error) {
            return target_ty;
        }
        // Union source (S2): the value is one-of, so `as P` is a runtime ASSERTION → `P?` (the value
        // when its runtime variant is `P`, else null) — except `as string`, which is the total
        // `toString` conversion (every value renders). `decimal` is deferred (its PHP carrier is a
        // string, indistinguishable from a `string` union member).
        if matches!(v, Ty::Union(_)) {
            let assert_cell: Option<&str> = match target {
                "int" => Some("asInt"),
                "float" => Some("asFloat"),
                "bool" => Some("asBool"),
                _ => None,
            };
            if let Some(name) = assert_cell {
                self.record_cast_call("Conversion", name, value, span);
                return Ty::Optional(Box::new(target_ty));
            }
            if target == "string" {
                self.record_cast_call("Conversion", "toString", value, span);
                return Ty::String;
            }
            // `as decimal` on a union — deferred (carrier conflation).
            self.err_coded(
                span,
                format!("`{v} as {target}` is not a supported conversion"),
                "E-CAST-TYPE",
                Some(
                    "`as decimal` on a union is deferred (decimal shares PHP's string carrier)"
                        .into(),
                ),
            );
            return Ty::Error;
        }
        // (source, target) → (qualifier leaf, native name, result type). `opt(t)` = `T?`.
        let opt = |t: Ty| Ty::Optional(Box::new(t));
        let cell: Option<(&str, &str, Ty)> = match (v, target) {
            (Ty::Int, "float") => Some(("Conversion", "toFloat", Ty::Float)),
            (Ty::Int, "decimal") => Some(("Conversion", "intToDecimal", Ty::Decimal)),
            (Ty::Float, "int") => Some(("Conversion", "floatToIntExact", opt(Ty::Int))),
            (Ty::Decimal, "int") => Some(("Conversion", "decimalToIntExact", opt(Ty::Int))),
            (Ty::Decimal, "float") => Some(("Conversion", "decimalToFloat", Ty::Float)),
            (Ty::String, "int") => Some(("String", "parseInt", opt(Ty::Int))),
            (Ty::String, "float") => Some(("String", "parseFloat", opt(Ty::Float))),
            // S4 decimal extras — float via the shortest-string parse; string via `Decimal.of`.
            (Ty::Float, "decimal") => Some(("Conversion", "floatToDecimal", opt(Ty::Decimal))),
            (Ty::String, "decimal") => Some(("Decimal", "of", opt(Ty::Decimal))),
            // S3 bool cells — total numeric↔bool (explicit `!= 0` / `1`/`0`), strict string parse.
            (Ty::Int, "bool") => Some(("Conversion", "intToBool", Ty::Bool)),
            (Ty::Float, "bool") => Some(("Conversion", "floatToBool", Ty::Bool)),
            (Ty::Decimal, "bool") => Some(("Conversion", "decimalToBool", Ty::Bool)),
            (Ty::Bool, "int") => Some(("Conversion", "boolToInt", Ty::Int)),
            (Ty::Bool, "float") => Some(("Conversion", "boolToFloat", Ty::Float)),
            (Ty::Bool, "decimal") => Some(("Conversion", "boolToDecimal", Ty::Decimal)),
            (Ty::String, "bool") => Some(("String", "parseBool", opt(Ty::Bool))),
            // any primitive → string is total (Convert.toString is generic).
            (Ty::Int | Ty::Float | Ty::Decimal | Ty::Bool, "string") => {
                Some(("Conversion", "toString", Ty::String))
            }
            _ => None,
        };
        match cell {
            Some((leaf, name, ret)) => {
                self.record_cast_call(leaf, name, value, span);
                ret
            }
            None => {
                // Deferred cell (bool, float→decimal, string→decimal) or an impossible pair.
                self.err_coded(
                    span,
                    format!("`{v} as {target}` is not a supported conversion"),
                    "E-CAST-TYPE",
                    Some(
                        "convert via `Core.Conversion` / `Core.String.parse*`; bool/decimal-from-float/string \
                         casts ship in a later slice"
                            .into(),
                    ),
                );
                Ty::Error
            }
        }
    }

    /// Record a primitive `as`-cast rewrite: `value as T` ⇒ `Leaf.name(value)` (a leaf-qualified
    /// native call the backends resolve by `index_of_by_leaf` without an import), keyed by the cast
    /// node's span. The synthetic call must be full-arity — the default-param fill pass does not see a
    /// post-check rewrite — so `String.parseFloat` (which takes `(string, bool permissive=false)`) gets
    /// its `false` (strict) default supplied explicitly.
    pub(in crate::checker) fn record_cast_call(
        &mut self,
        leaf: &str,
        name: &str,
        value: &crate::ast::Expr,
        span: Span,
    ) {
        use crate::ast::Expr;
        let callee = Expr::Member {
            object: Box::new(Expr::Ident(leaf.to_string(), span)),
            name: name.to_string(),
            safe: false,
            sep: crate::ast::MemberSep::Dot,
            span,
        };
        let mut call_args = vec![value.clone()];
        if name == "parseFloat" {
            call_args.push(Expr::Bool(false, span));
        }
        self.cast_resolutions.insert(
            span.start,
            Expr::Call {
                callee: Box::new(callee),
                args: call_args,
                span,
            },
        );
    }

    // ---- stubs replaced in later tasks ----
}
