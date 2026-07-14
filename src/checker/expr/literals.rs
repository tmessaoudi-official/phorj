//! Expression checking — literals, interpolation, html, collections, index/range,
//! if-expr, lambdas.

use super::*;

impl Checker {
    pub(in crate::checker) fn check_str(&mut self, parts: &[crate::ast::StrPart]) -> Ty {
        use crate::ast::StrPart;
        for part in parts {
            if let StrPart::Expr(e) = part {
                let t = self.check_expr(e);
                let ok = matches!(
                    t,
                    Ty::Int | Ty::Float | Ty::Decimal | Ty::Bool | Ty::String | Ty::Error
                );
                if !ok {
                    let sp = Self::expr_span(e);
                    self.err(sp, format!("type `{t}` cannot be interpolated into a string (only primitives auto-stringify in M1)"));
                }
            }
        }
        Ty::String
    }

    /// Check an `html"…"` literal (core.html Wave 3) and record its type-directed desugaring.
    ///
    /// Each literal chunk becomes `html.raw(chunk)` (author markup is trusted); each `{e}` hole is
    /// resolved **by `e`'s type**: an `Html` value embeds as-is (already safe — lets you nest
    /// builders / other `html"…"`); a `string` is wrapped in `html.text(e)` (auto-escaped — the safe
    /// default for raw data); an `int`/`float`/`bool` is stringified then escaped; anything else is a
    /// clean `E-HTML-HOLE`. The default hole behavior is **escape** — injecting trusted markup
    /// requires writing `{html.raw(x)}` explicitly (unsafe is long, safe is short). The pieces are
    /// concatenated with `html.concat([…])`; the whole tree uses only Wave-1/2 natives, which are
    /// already byte-identical across the three backends, so parity is inherited, not re-proved.
    ///
    /// The replacement is stored by the literal's `Span.start` and applied by [`resolve_html`] after
    /// checking — `check` itself never mutates the AST (it borrows it). Returns [`Ty::Html`].
    pub(in crate::checker) fn check_html(
        &mut self,
        parts: &[crate::ast::StrPart],
        span: Span,
    ) -> Ty {
        use crate::ast::{Expr, StrPart};
        // `html"…"` desugars to `<leaf>.raw/.text/.concat` calls, so the program must import
        // core.html. Resolve whatever leaf maps to it (robust to `import core.html as h;`).
        let leaf = self
            .imports
            .iter()
            .find(|(_, full)| full.as_str() == "Core.Html")
            .map(|(leaf, _)| leaf.clone());
        let leaf = match leaf {
            Some(l) => l,
            None => {
                return self.err_coded(
                    span,
                    "`html\"…\"` requires the Core.Html module",
                    "E-HTML-IMPORT",
                    Some("add `import Core.Html;` (or `import Core.Html as h;`)".into()),
                );
            }
        };
        // Build `<leaf>.<name>(args)` as a plain `Member`-headed call (resolved like any namespaced
        // native by the backends, via the import map). All synthetic nodes carry the literal's span.
        let call = |name: &str, args: Vec<Expr>| -> Expr {
            Expr::Call {
                callee: Box::new(Expr::Member {
                    object: Box::new(Expr::Ident(leaf.clone(), span)),
                    name: name.to_string(),
                    safe: false,
                    sep: crate::ast::MemberSep::Dot,
                    span,
                }),
                args,
                span,
            }
        };
        let str_lit = |s: &str| Expr::Str(vec![StrPart::Literal(s.to_string())], span);

        let mut elems: Vec<Expr> = Vec::with_capacity(parts.len());
        for part in parts {
            match part {
                StrPart::Literal(chunk) => elems.push(call("raw", vec![str_lit(chunk)])),
                StrPart::Expr(e) => {
                    let t = self.check_expr(e);
                    match t {
                        // already an Html fragment — embed verbatim (no double-escape).
                        Ty::Html => elems.push((**e).clone()),
                        // raw text — escape it (the safe default).
                        Ty::String => elems.push(call("text", vec![(**e).clone()])),
                        // primitives stringify (via a one-hole string interp) then escape, for
                        // uniformity — numbers carry no markup but go through the same wall.
                        Ty::Int | Ty::Float | Ty::Bool => {
                            let stringified =
                                Expr::Str(vec![StrPart::Expr(Box::new((**e).clone()))], span);
                            elems.push(call("text", vec![stringified]));
                        }
                        // a poisoned hole already reported its own error; keep going without piling
                        // on, and emit *something* well-typed so the replacement stays buildable.
                        Ty::Error => elems.push(call("text", vec![str_lit("")])),
                        other => {
                            self.err_coded(
                                Self::expr_span(e),
                                format!(
                                    "cannot interpolate `{other}` into html; render it to a string or Html first"
                                ),
                                "E-HTML-HOLE",
                                Some(
                                    "wrap it with `Html.text(…)`/`Html.raw(…)`, or build it with the html builders"
                                        .into(),
                                ),
                            );
                            elems.push(call("text", vec![str_lit("")]));
                        }
                    }
                }
            }
        }

        let replacement = call("concat", vec![Expr::List(elems, span)]);
        self.html_resolutions.insert(span.start, replacement);
        Ty::Html
    }

    /// A general tagged template `tag"…literal{hole}…"` (DEC-212, both modes). The tag is resolved and
    /// the template desugared to a plain call the backends already lower (stored keyed by span, applied
    /// by `resolve_html`), so no backend sees the node — the same erased-sugar discipline as `html"…"`.
    ///
    /// - **Function mode** — `tag` is a (non-overloaded) free function: desugar to
    ///   `tag([literal chunks], [hole exprs])` (JS-style; the handler owns rendering/escaping). The
    ///   result type is the function's return type; a wrong-shaped handler is a normal call type error.
    /// - **Protocol mode** — `tag` is an imported module or a type providing `raw`/`text`/`concat`:
    ///   desugar to `tag.concat([tag.raw(lit), tag.text(hole), …])` (escape-by-default kernel, like
    ///   `html`). Holes are stringified then `text()`-escaped, so a protocol tag takes primitive holes.
    pub(in crate::checker) fn check_tagged_template(
        &mut self,
        tag: &str,
        parts: &[crate::ast::StrPart],
        span: Span,
    ) -> Ty {
        use crate::ast::{Expr, StrPart};
        let str_lit = |s: &str| Expr::Str(vec![StrPart::Literal(s.to_string())], span);

        // FUNCTION MODE: a non-overloaded free function named `tag`.
        if self.funcs.get(tag).is_some_and(|sigs| sigs.len() == 1) {
            let literals: Vec<Expr> = parts
                .iter()
                .filter_map(|p| match p {
                    StrPart::Literal(s) => Some(str_lit(s)),
                    StrPart::Expr(_) => None,
                })
                .collect();
            let holes: Vec<Expr> = parts
                .iter()
                .filter_map(|p| match p {
                    StrPart::Expr(e) => Some((**e).clone()),
                    StrPart::Literal(_) => None,
                })
                .collect();
            let replacement = Expr::Call {
                callee: Box::new(Expr::Ident(tag.to_string(), span)),
                args: vec![Expr::List(literals, span), Expr::List(holes, span)],
                span,
            };
            let ty = self.check_expr(&replacement);
            self.html_resolutions.insert(span.start, replacement);
            return ty;
        }

        // PROTOCOL MODE: an imported module or a type providing raw/text/concat.
        if self.imports.contains_key(tag) || self.classes.contains_key(tag) {
            let call = |name: &str, args: Vec<Expr>| -> Expr {
                Expr::Call {
                    callee: Box::new(Expr::Member {
                        object: Box::new(Expr::Ident(tag.to_string(), span)),
                        name: name.to_string(),
                        safe: false,
                        sep: crate::ast::MemberSep::Dot,
                        span,
                    }),
                    args,
                    span,
                }
            };
            let mut elems: Vec<Expr> = Vec::with_capacity(parts.len());
            for part in parts {
                match part {
                    StrPart::Literal(chunk) => elems.push(call("raw", vec![str_lit(chunk)])),
                    StrPart::Expr(e) => {
                        // Stringify the hole (primitives auto-stringify via a one-hole interp), then
                        // escape via `text()` — the safe default. A non-stringifiable hole errors there.
                        let stringified =
                            Expr::Str(vec![StrPart::Expr(Box::new((**e).clone()))], span);
                        elems.push(call("text", vec![stringified]));
                    }
                }
            }
            let replacement = call("concat", vec![Expr::List(elems, span)]);
            let ty = self.check_expr(&replacement);
            self.html_resolutions.insert(span.start, replacement);
            return ty;
        }

        self.err_coded(
            span,
            format!("unknown template tag `{tag}`"),
            "E-UNKNOWN-TAG",
            Some(
                "a template tag must be a function `(List<string>, List<H>) -> R` (function mode) or a module/type providing `raw`/`text`/`concat` (protocol mode)"
                    .into(),
            ),
        )
    }

    /// The source span of an expression (used to position errors precisely).
    pub(in crate::checker) fn expr_span(e: &crate::ast::Expr) -> Span {
        use crate::ast::Expr;
        match e {
            Expr::Int(_, s)
            | Expr::Float(_, s)
            | Expr::Bool(_, s)
            | Expr::Str(_, s)
            | Expr::Bytes(_, s)
            | Expr::Ident(_, s)
            | Expr::List(_, s)
            | Expr::Map(_, s) => *s,
            Expr::Null(s) | Expr::This(s) => *s,
            Expr::Decimal { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::InstanceOf { span, .. }
            | Expr::Cast { span, .. }
            | Expr::Call { span, .. }
            | Expr::Member { span, .. }
            | Expr::Index { span, .. }
            | Expr::Force { span, .. }
            | Expr::Propagate { span, .. }
            | Expr::Match { span, .. }
            | Expr::Range { span, .. }
            | Expr::If { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::CloneWith { span, .. }
            | Expr::OverloadSelect { span, .. }
            | Expr::ParentCall { span, .. }
            | Expr::New(_, span)
            | Expr::Spawn { span, .. }
            | Expr::Inject { span, .. }
            | Expr::NewColl { span, .. }
            | Expr::TaggedTemplate { span, .. }
            | Expr::Html(_, span) => *span,
        }
    }

    /// `new List<T>()` / `new Map<K,V>()` / `new Set<T>()` (DEC-214) — explicit empty-collection
    /// construction. Self-typed from the type arguments (no contextual inference), which are resolved
    /// through `resolve_type` (so casing + injected-import discipline + existence are all enforced on
    /// the args). Arity is checked against the kind. The pre-backend `rewrite_new_coll` pass then lowers
    /// this to an empty `List`/`Map`, so no backend needs the element type.
    pub(in crate::checker) fn check_new_coll(
        &mut self,
        kind: crate::ast::CollKind,
        args: &[crate::ast::Type],
        span: Span,
    ) -> Ty {
        use crate::ast::CollKind;
        if args.len() != kind.arity() {
            return self.err(
                span,
                format!(
                    "`new {}<…>()` expects {} type argument(s), got {}",
                    kind.name(),
                    kind.arity(),
                    args.len()
                ),
            );
        }
        let tys: Vec<Ty> = args.iter().map(|a| self.resolve_type(a)).collect();
        match kind {
            CollKind::List => Ty::List(Box::new(tys[0].clone())),
            CollKind::Map => Ty::Map(Box::new(tys[0].clone()), Box::new(tys[1].clone())),
        }
    }

    /// DEC-214 part-2: a bare empty `[]` literal is rejected everywhere — an empty collection must be
    /// CONSTRUCTED with mandatory `new` (`new List<T>()` / `new Map<K,V>()`), never inferred from
    /// context. Single-sourced so the three typing sites (`check_list`, `thread_literal_expected`, and
    /// the call-argument `check_arg`) emit one identical `E-EMPTY-LITERAL`. Returns `Ty::Error`.
    pub(in crate::checker) fn err_empty_literal(&mut self, span: Span) -> Ty {
        self.err_coded(
            span,
            "an empty collection needs its type",
            "E-EMPTY-LITERAL",
            Some("write `new List<T>()` or `new Map<K,V>()`".into()),
        )
    }

    pub(in crate::checker) fn check_list(&mut self, elems: &[crate::ast::Expr], span: Span) -> Ty {
        if elems.is_empty() {
            // DEC-214 part-2: a bare empty `[]` has no element type and is no longer contextually
            // inferred — it must be built with `new List<T>()` / `new Map<K,V>()`.
            return self.err_empty_literal(span);
        }
        let first = self.check_expr(&elems[0]);
        for e in &elems[1..] {
            let t = self.check_expr(e);
            if !self.ty_assignable(&t, &first) && !self.ty_assignable(&first, &t) {
                self.err(
                    span,
                    format!("list elements must share one type; found `{first}` and `{t}`"),
                );
            }
        }
        Ty::List(Box::new(first))
    }

    /// `[k => v, …]` (M-RT S3): infer the key type `K` and value type `V`, unifying across pairs
    /// (each must share one type, like list elements). The parser guarantees ≥1 pair (an empty `[]`
    /// is the empty *list*). Keys must be the hashable subset — `int`/`bool`/`string` — else
    /// `E-MAP-KEY` (a `float`/instance/list key has no `HKey`). Result: `Ty::Map(K, V)`.
    pub(in crate::checker) fn check_map(
        &mut self,
        pairs: &[(crate::ast::Expr, crate::ast::Expr)],
        span: Span,
    ) -> Ty {
        let (k0, v0) = &pairs[0];
        let key_ty = self.check_expr(k0);
        let val_ty = self.check_expr(v0);
        for (k, v) in &pairs[1..] {
            let kt = self.check_expr(k);
            if !self.ty_assignable(&kt, &key_ty) && !self.ty_assignable(&key_ty, &kt) {
                self.err(
                    span,
                    format!("map keys must share one type; found `{key_ty}` and `{kt}`"),
                );
            }
            let vt = self.check_expr(v);
            if !self.ty_assignable(&vt, &val_ty) && !self.ty_assignable(&val_ty, &vt) {
                self.err(
                    span,
                    format!("map values must share one type; found `{val_ty}` and `{vt}`"),
                );
            }
        }
        if !matches!(key_ty, Ty::Int | Ty::Bool | Ty::String | Ty::Error) {
            return self.err_coded(
                span,
                format!("map key type must be `int`, `bool`, or `string`, found `{key_ty}`"),
                "E-MAP-KEY",
                None,
            );
        }
        Ty::Map(Box::new(key_ty), Box::new(val_ty))
    }

    /// Static bounds check for a fixed-length list `[T; N]` (Phase 1 types slice). Only a *literal*
    /// index is known at compile time: a constant `< 0` or `>= len` is `E-FIXEDLIST-BOUNDS`. A
    /// non-literal index is left to the runtime bounds check (the same `Op::Index` as a list).
    pub(in crate::checker) fn fixedlist_static_bounds(
        &mut self,
        index: &crate::ast::Expr,
        len: usize,
        elem: &Ty,
        span: Span,
    ) {
        if let crate::ast::Expr::Int(k, _) = index {
            if *k < 0 || (*k as usize) >= len {
                self.err_coded(
                    span,
                    format!("index {k} is out of bounds for `[{elem}; {len}]`"),
                    "E-FIXEDLIST-BOUNDS",
                    Some(format!("valid indices are 0..{len}")),
                );
            }
        }
    }

    pub(in crate::checker) fn check_index(
        &mut self,
        object: &crate::ast::Expr,
        index: &crate::ast::Expr,
        span: Span,
    ) -> Ty {
        let obj = self.check_expr(object);
        let idx = self.check_expr(index);
        match obj {
            Ty::List(elem) => {
                if !self.ty_assignable(&idx, &Ty::Int) {
                    self.err(span, format!("list index must be `int`, found `{idx}`"));
                }
                *elem
            }
            // `pair[i]` on a `[T; N]`: like a list read, but a *literal* index is bounds-checked at
            // compile time (`pair[5]` on `[int; 2]` is `E-FIXEDLIST-BOUNDS`); a dynamic index falls
            // back to the runtime bounds check, same as a list (Phase 1 types slice).
            Ty::FixedList(elem, n) => {
                if !self.ty_assignable(&idx, &Ty::Int) {
                    self.err(span, format!("list index must be `int`, found `{idx}`"));
                }
                self.fixedlist_static_bounds(index, n, &elem, span);
                *elem
            }
            // `m[k]` (M-RT S3): the index must match the key type; the result is the value type. A
            // missing key faults at runtime (byte-identical present-key, like list-OOB the fault path
            // is excluded from differential gating).
            Ty::Map(k, v) => {
                if !self.ty_assignable(&idx, &k) {
                    self.err(span, format!("map index must be `{k}`, found `{idx}`"));
                }
                *v
            }
            Ty::Error => Ty::Error,
            other => self.err(span, format!("type `{other}` cannot be indexed")),
        }
    }

    /// `start..end` / `start..=end`: both bounds must be `int`; the range's type is `List<int>` (its
    /// only role this slice is `for … in`). A non-int bound is `E-RANGE-TYPE` (decision S1-R).
    pub(in crate::checker) fn check_range(
        &mut self,
        start: &crate::ast::Expr,
        end: &crate::ast::Expr,
        span: Span,
    ) -> Ty {
        let s = self.check_expr(start);
        let e = self.check_expr(end);
        let ok = |t: &Ty| matches!(t, Ty::Int | Ty::Error);
        if !ok(&s) || !ok(&e) {
            return self.err_coded(
                span,
                format!("range bounds must be `int`, found `{s}` and `{e}`"),
                "E-RANGE-TYPE",
                None,
            );
        }
        Ty::List(Box::new(Ty::Int))
    }

    /// Expression `if`: the condition must be `bool` and both arms must share one type `T`, which is
    /// the expression's type. (`else` is mandatory at the parser, so there is no missing-else case
    /// here.) Mirrors `check_match`'s arm-unification rule (M3 S1.3).
    pub(in crate::checker) fn check_if_expr(
        &mut self,
        cond: &crate::ast::Expr,
        then_e: &crate::ast::Expr,
        else_e: &crate::ast::Expr,
        span: Span,
    ) -> Ty {
        let c = self.check_expr(cond);
        if !self.ty_assignable(&c, &Ty::Bool) {
            self.err(span, format!("`if` condition must be `bool`, found `{c}`"));
        }
        let t = self.check_expr(then_e);
        let e = self.check_expr(else_e);
        if t != Ty::Error
            && e != Ty::Error
            && !self.ty_assignable(&e, &t)
            && !self.ty_assignable(&t, &e)
        {
            self.err(
                span,
                format!("`if` branches must share one type; found `{t}` and `{e}`"),
            );
        }
        if t == Ty::Error {
            e
        } else {
            t
        }
    }

    /// Type-check a lambda expression (M3 S3, Task 3). Returns `Ty::Function(params, ret, throws)`
    /// (DEC-222 — the lambda's declared checked-exception set; empty when no `throws` clause).
    ///
    /// Type-checks a lambda. A method-body lambda **may** capture `this` (Phase 1 closures slice): it
    /// is captured by value (the `Rc` instance handle, so mutations stay live), `this` types as the
    /// enclosing class via `cur_class`, and the two backends + PHP all bind the same receiver. The one
    /// place it stays rejected is a **field/static initializer** (`in_field_init`): the instance is
    /// only partially built when an initializer runs, so capturing the receiver is the F8 footgun.
    pub(in crate::checker) fn check_lambda(
        &mut self,
        params: &[crate::ast::Param],
        ret: &Option<crate::ast::Type>,
        throws: &[crate::ast::Type],
        body: &crate::ast::LambdaBody,
        span: Span,
    ) -> Ty {
        use crate::ast::LambdaBody;
        // A field-default lambda may not capture `this` (partially-built instance, F8).
        if self.in_field_init && crate::ast::lambda_uses_this(body) {
            self.err_coded(
                span,
                "a field-initializer lambda cannot capture `this` — the instance is not fully built yet",
                "E-LAMBDA-THIS",
                Some("move the closure into the constructor body, or capture a specific value (`var v = this.x;`) instead".into()),
            );
        }
        let param_tys: Vec<Ty> = params.iter().map(|p| self.resolve_type(&p.ty)).collect();
        // DEC-222: resolve + normalize the lambda's DECLARED throws (flatten unions, canonical-sort,
        // dedupe — like `resolve_type`'s function-type path), validate each is an `Error` subtype
        // (`E-THROW-TYPE`/`E-THROWS-TOO-BROAD`, shared with fn/ctor decls), and check the body with
        // these throws in context. Absent clause ⇒ empty set ⇒ a `throw`/`?` in the body still hits
        // `E-THROW-UNDECLARED`/`E-CALL-UNHANDLED` (a bare closure declares nothing, exactly like a
        // named function with no `throws`). No inference — a throwing lambda must declare its throws.
        let lambda_throws: Vec<Ty> = {
            let resolved: Vec<Ty> = throws.iter().map(|t| self.resolve_type(t)).collect();
            let mut es = Self::flatten_throws(resolved);
            es.sort_by_key(std::string::ToString::to_string);
            es.dedup();
            self.validate_throw_types(&es, span);
            es
        };
        // Save and replace the current return type (a lambda has its own return scope).
        let saved_ret = std::mem::replace(&mut self.cur_ret, Ty::Error);
        // A lambda is a separate callable: its body discharges against ITS OWN declared throws
        // (DEC-222), and it does not see the lexical `try` it is written inside (it may be invoked
        // elsewhere — e.g. passed to a native), so the enclosing `try` stack is cleared (M-faults 2b).
        let saved_throws = std::mem::replace(&mut self.cur_throws, lambda_throws.clone());
        let saved_try = std::mem::take(&mut self.try_catch_stack);
        let saved_main = std::mem::replace(&mut self.cur_is_main, false);
        self.push_scope();
        for p in params {
            let pty = self.resolve_type(&p.ty);
            self.declare(&p.name, pty, p.span);
        }
        let ret_ty = match body {
            LambdaBody::Expr(e) => {
                let inferred = self.check_expr(e);
                if let Some(rt) = ret {
                    let declared = self.resolve_type(rt);
                    if !self.ty_assignable(&inferred, &declared) {
                        self.err_assign(span, &inferred, &declared);
                    }
                    declared
                } else {
                    inferred
                }
            }
            LambdaBody::Block(stmts) => {
                // A2/F10: an explicit `-> T` annotation is required for statement-body lambdas.
                match ret {
                    Some(rt) => {
                        let declared = self.resolve_type(rt);
                        self.cur_ret = declared.clone();
                        // Batch F (finding #6): a statement-body lambda must return on all paths just
                        // like a free fn/method — falling off the end of a `-> int` lambda bound `unit`
                        // into an `int` slot. Route through `check_body` (W-UNREACHABLE) + enforce
                        // return totality.
                        self.check_body(stmts);
                        self.check_return_totality(&declared, stmts, span);
                        declared
                    }
                    None => self.err(
                        span,
                        "a statement-body lambda requires an explicit `-> T` return type",
                    ),
                }
            }
        };
        self.pop_scope();
        self.cur_ret = saved_ret;
        self.cur_throws = saved_throws;
        self.try_catch_stack = saved_try;
        self.cur_is_main = saved_main;
        Ty::Function(param_tys, Box::new(ret_ty), lambda_throws)
    }
}
