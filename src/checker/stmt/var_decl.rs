//! Checking a `var`/typed local DECLARATION — the one statement that carries an initializer, an
//! optional annotation, and every inference rule that follows from their interaction.
//!
//! Split out of `stmt/core.rs` (Invariant 13): the arm had grown to ~180 lines of its own — the
//! `?`-propagation forms, the return-overload and `Channel<T>` sinks, literal expected-type
//! threading, fixed-length checking, `void`-capture, and the DEC-504 inferred-tuple recording —
//! while every other arm in `check_stmt` is a handful of lines or a call.

use super::*;

impl Checker {
    /// Check `[<ty>] <name> = <init>;` and declare the binding.
    pub(in crate::checker) fn check_var_decl(
        &mut self,
        ty: &crate::ast::Type,
        name: &str,
        init: &crate::ast::Expr,
        mutable: bool,
        span: Span,
    ) {
        // A let-initializer is the one position where Result-mode `?` propagation is allowed
        // (M-faults 2a): detect it here and type it via `check_propagate` (the unwrapped `Ok`
        // payload). Throws-mode `?` (a throwing call) is allowed in *any* position and tried
        // first; it returns the call's normal type and erases the node (`try_throws_propagate`).
        //
        // Resolve the declared type ONCE (non-`Infer`), reused by the literal expected-type
        // arms below and the final assignability check — a second `resolve_type(ty)` re-emits
        // any resolution error (e.g. `E-TYPE-ARG-COUNT` for `Map<int>`) a second time.
        let declared_once: Option<Ty> = match ty {
            crate::ast::Type::Infer(_) => None,
            _ => Some(self.resolve_type(ty)),
        };
        let actual = match init {
            crate::ast::Expr::Propagate { inner, span: psp } => {
                match self.try_throws_propagate(inner, *psp) {
                    Some(crate::checker::throws::PropagateOutcome::Throws(t)) => t,
                    // A call that throws nothing was already checked — hand its type to
                    // Result-mode without a duplicate check.
                    Some(crate::checker::throws::PropagateOutcome::Plain(t)) => {
                        self.check_propagate_typed(t, *psp)
                    }
                    None => self.check_propagate(inner, *psp),
                }
            }
            // C2 sink: a bare return-overloaded call binds to a concrete declared type without a
            // `<Type>` selector — the typed binding supplies the resolving context. (`var x = …`
            // is `Type::Infer`, has no context, and falls through to the `E-OVERLOAD-NO-CONTEXT`
            // path.)
            _ if !matches!(ty, crate::ast::Type::Infer(_))
                && self.is_return_overload_call(init) =>
            {
                let expected = self.resolve_type(ty);
                self.try_resolve_sink_overload(init, &expected)
                    .unwrap_or(Ty::Error)
            }
            // `Channel<T> ch = Channel.new();` (M6 W4): the static constructor has no argument
            // to infer `T` from, so it takes its element type from the binding's annotation
            // (the one construction site for a channel). Validate the call shape (0 args) here;
            // a non-`Channel` annotation falls through to the normal assign-mismatch path.
            _ if !matches!(ty, crate::ast::Type::Infer(_)) && Self::is_channel_new(init) => {
                if let crate::ast::Expr::Call {
                    args, span: csp, ..
                } = init
                {
                    if !args.is_empty() {
                        self.err_coded(
                            *csp,
                            "`Channel.new()` takes no arguments",
                            "E-CHANNEL-NEW-ARITY",
                            None,
                        );
                    }
                }
                let declared = self.resolve_type(ty);
                if matches!(&declared, Ty::Named(n, _) if n == "Channel") {
                    declared
                } else {
                    self.err_coded(
                        span,
                        format!("`Channel.new()` produces a `Channel<T>`, not `{declared}`"),
                        "E-CHANNEL-NEW-TYPE",
                        None,
                    )
                }
            }
            // A list literal with a `List<T>` annotation is checked against that expected
            // element type (M-DOGFOOD W0 + W3): each element must be assignable to `T`. This
            // (a) supplies the element type for an empty `[]` (the runtime value is an empty
            // `Value::List` regardless of `T`, so no backend change), and (b) lets a
            // heterogeneous list of subtypes upcast — e.g. `List<Shape> xs = [new Sq(), new
            // Tri()]` — which post-hoc element unification could not (a) do at all and (b)
            // `List` is invariant so `List<Sq>` is not assignable to `List<Shape>`. A
            // non-`List` annotation (e.g. a fixed-length `[T; N]`) falls through to the
            // normal path, which owns its own length/element checks.
            // A list/map literal with a `List<T>`/`Map<K,V>` annotation is checked against the
            // declared element/value types (UA-1.6 / DEC-178) via `thread_literal_expected`:
            // each member must be assignable to `T` / `K,V`, which (a) supplies the element type
            // for an empty `[]`, (b) lets a heterogeneous list of subtypes upcast
            // (`List<Shape> = [new Sq(), new Tri()]`), and (c) lets a union value/element
            // type-check (`Map<string, int | string> = ["a" => 1, "b" => "two"]`) — none of which
            // the bottom-up `check_list`/`check_map` inference can. A non-collection annotation
            // (e.g. `[T; N]`) returns `None` and falls through to the normal path below.
            crate::ast::Expr::List(..) | crate::ast::Expr::Map(..)
                if !matches!(ty, crate::ast::Type::Infer(_)) =>
            {
                let expected = declared_once.clone().unwrap_or(Ty::Error);
                self.thread_literal_expected(init, &expected)
                    .unwrap_or_else(|| self.check_expr(init))
            }
            _ => self.check_expr(init),
        };
        let declared = match ty {
            crate::ast::Type::Infer(infer_span) => {
                // `var` binds the initializer's type — but a bare `null` (type `Ty::Null`)
                // has no inferable element type and needs an explicit annotation, e.g.
                // `int? x = null;` (S0.2 / S2).
                if matches!(actual, Ty::Null) {
                    self.err_coded(
                        *infer_span,
                        "cannot infer a type from `null`",
                        "E-INFER-NULL",
                        Some("annotate the optional, e.g. `int? x = null;`".into()),
                    )
                } else {
                    actual.clone()
                }
            }
            _ => {
                // Reuse the single resolution from above (non-`Infer` ⇒ `Some`).
                let declared = declared_once.clone().unwrap_or(Ty::Error);
                // `[T; N] p = [e0, e1, …]`: a list literal carries a known length, so the
                // fixed-length is checked *here* (the literal is the one place the length is
                // statically known) — `List` itself is length-erased, so this is the only
                // path that introduces a `[T; N]` value (Phase 1 types slice).
                if let (Ty::FixedList(elem, n), crate::ast::Expr::List(elems, _)) =
                    (&declared, init)
                {
                    if elems.len() != *n {
                        self.err_coded(
                                    span,
                                    format!(
                                        "expected `[{elem}; {n}]` (length {n}), found a list literal of length {}",
                                        elems.len()
                                    ),
                                    "E-FIXEDLIST-LEN",
                                    None,
                                );
                    }
                    // Element-type compatibility (List is invariant, so this checks `elem`).
                    if !self.ty_assignable(&actual, &Ty::List(elem.clone())) {
                        self.err_assign(span, &actual, &declared);
                    }
                } else if !self.ty_assignable(&actual, &declared) {
                    self.err_assign(span, &actual, &declared);
                }
                declared
            }
        };
        // S0a: a `void` value is uncapturable. Binding one into a variable is an error —
        // *unless* the declared type is the holdable `empty` (`empty x = noop();`), the
        // explicit escape hatch (`void <: empty`). This catches both `var x = noop()`
        // (inferred `declared` = `Void`) and `void x = noop()` (declared = `Void`).
        let declared = if actual == Ty::Void && declared != Ty::Empty {
            self.err_coded(
                        span,
                        "a `void` value cannot be captured — the expression produces nothing",
                        "E-VOID-CAPTURE",
                        Some(
                            "drop the binding and call it as a statement; or, to hold the empty value, annotate it `empty` (e.g. `empty x = …;`)"
                                .into(),
                        ),
                    )
        } else {
            declared
        };
        // DEC-504 / Invariant 7: a `var` whose inferred type is a TUPLE records that type so
        // `materialize_inferred_types` writes it onto the declaration. Without an annotation
        // both backends fall back to the initializer, which by then has been ERASED to a
        // plain list — and a list's element type is position 0's, so `var t = (source: "z",
        // bp: 7); t.bp + 1` resolves `bp` as a string: the VM refuses to infer a numeric
        // type where the interpreter reads an int, and the PHP leg emits `.` for the `+`.
        // Tuples only: every other `var` is already served correctly by that fallback.
        if matches!(ty, crate::ast::Type::Infer(_)) && matches!(declared, Ty::Tuple(..)) {
            self.inferred_type_resolutions
                .insert(span.start, declared.clone());
        }
        self.declare_binding(name, declared, mutable, span);
    }
}
