//! PHP lifter — expression lifting. The declaration leaf conversions live in `leaves.rs` and the
//! `match` cluster in `matches.rs` (split out under Invariant 13, lane L1c).

use super::*;
use crate::ast::LambdaBody;

// ── expressions (no scope state) ──

pub(super) fn lift_expr(e: &php::PhpExpr) -> Result<Expr, String> {
    Ok(match e {
        php::PhpExpr::Int(n) => Expr::Int(*n, SP),
        php::PhpExpr::Closure { params, ret, body } => Expr::Lambda {
            params: lift_params(params)?,
            ret: ret.as_ref().map(lift_type).transpose()?,
            throws: Vec::new(),
            body: LambdaBody::Expr(Box::new(lift_expr(body)?)),
            span: SP,
        },
        // DEC-511: PHP's `.` COERCES its operands to string; phorj's `+` refuses to ("no
        // coercion"), so lifting `.` to `+` produced a draft that lifted and then failed
        // `phg check` — ~207 sites in one real 120-file codebase. phorj already has the faithful
        // form: an interpolation stringifies each part exactly as PHP's `.` does. A CHAIN flattens
        // into ONE interpolation rather than a nest, which is what PHP's own evaluation produces.
        // A STRICT comparison against `null` is phorj's `is null` narrowing test, not an equality:
        // `$x === null` asks exactly "is this the null case of an optional", which `==` cannot
        // express here — the checker rejects `T? == null` as a cross-type comparison, so lifting it
        // as an equality produced a draft that lifted and then failed `phg check`. Both orderings
        // are handled, because Yoda style (`null === $x`) is common in real PHP.
        //
        // STRICT ONLY. PHP's LOOSE `$x == null` is also true for `0`, `""`, `[]` and `false`, so it
        // is NOT this test; it is left as a plain `==` and the checker then reports it, which is the
        // honest DEC-166 outcome — the lifter does not guess which of the five the author meant.
        php::PhpExpr::Binary {
            op: op @ (php::PhpBinOp::Identical | php::PhpBinOp::NotIdentical),
            left,
            right,
        } if matches!(left.as_ref(), php::PhpExpr::Null)
            || matches!(right.as_ref(), php::PhpExpr::Null) =>
        {
            let subject = if matches!(left.as_ref(), php::PhpExpr::Null) {
                right
            } else {
                left
            };
            let test = Expr::InstanceOf {
                value: Box::new(lift_expr(subject)?),
                type_name: "null".to_string(),
                span: SP,
            };
            match op {
                php::PhpBinOp::Identical => test,
                // phorj has no `is not null`: the negation is spelled `!(x is null)`.
                _ => Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(test),
                    span: SP,
                },
            }
        }
        php::PhpExpr::Binary {
            op: php::PhpBinOp::Concat,
            ..
        } => {
            let mut parts = Vec::new();
            flatten_concat(e, &mut parts)?;
            Expr::Str(parts, SP)
        }
        php::PhpExpr::BlockClosure { params, ret, body } => {
            // A phorj lambda captures enclosing locals BY VALUE, which is exactly what PHP's
            // by-value `use (…)` list asks for — so the list is dropped at the parser and the
            // captured names simply stay in scope here. A by-REFERENCE capture never reaches this
            // point: it is refused by name (DEC-506).
            let mut declared: HashSet<String> = HashSet::new();
            let lifted_params = lift_params(params)?;
            for p in &lifted_params {
                declared.insert(p.name.clone());
            }
            Expr::Lambda {
                params: lifted_params,
                ret: ret.as_ref().map(lift_type).transpose()?,
                throws: Vec::new(),
                body: LambdaBody::Block(Lifter {}.lift_block(body, &mut declared)?),
                span: SP,
            }
        }
        // A destructure is a STATEMENT in phorj (`var (a, b) = …`), so it is handled in
        // `lift_assign_like`. PHP also allows it as an expression (`f([$a, $b] = g())`), which has
        // no phorj form — refused by name rather than mis-lifted.
        php::PhpExpr::Destructure { .. } => {
            return Err(
                "lift: a destructuring assignment used as an EXPRESSION has no phorj form — \
                        phorj's `var (a, b) = …` is a statement (Tier-2)"
                    .into(),
            )
        }
        php::PhpExpr::Float(f) => Expr::Float(*f, SP),
        php::PhpExpr::Str(s) => Expr::Str(vec![StrPart::Literal(s.clone())], SP),
        php::PhpExpr::Interp(parts) => {
            let mut out = Vec::with_capacity(parts.len());
            for part in parts {
                out.push(match part {
                    php::PhpStrPart::Lit(s) => StrPart::Literal(s.clone()),
                    php::PhpStrPart::Expr(e) => StrPart::Expr(Box::new(lift_expr(e)?)),
                });
            }
            Expr::Str(out, SP)
        }
        php::PhpExpr::Bool(b) => Expr::Bool(*b, SP),
        php::PhpExpr::Null => Expr::Null(SP),
        // Inside a lowered enum method (DEC-509) `$this` IS the receiver parameter; everywhere else
        // it is an ordinary `this`.
        php::PhpExpr::Var(name) if name == "this" => match super::enums::receiver() {
            Some(recv) => Expr::Ident(recv, SP),
            None => Expr::This(SP),
        },
        php::PhpExpr::Var(name) | php::PhpExpr::Name(name) => Expr::Ident(name.clone(), SP),
        php::PhpExpr::Array(elems) => lift_array(elems)?,
        // LIFT-ATTR: `name: value` lifts 1:1 — phorj spells a named argument exactly the same way
        // (DEC-297), so nothing is reordered here. The checker normalizes named args into their
        // positional slots later, and rejects the positions phorj does not support yet, which keeps
        // that judgement in ONE place instead of duplicating it in the lifter.
        php::PhpExpr::NamedArg { name, value } => Expr::NamedArg {
            name: name.clone(),
            value: Box::new(lift_expr(value)?),
            span: SP,
        },
        php::PhpExpr::Unary { op, expr } => Expr::Unary {
            op: match op {
                php::PhpUnOp::Not => UnaryOp::Not,
                php::PhpUnOp::Neg => UnaryOp::Neg,
                php::PhpUnOp::BitNot => UnaryOp::BitNot,
            },
            expr: Box::new(lift_expr(expr)?),
            span: SP,
        },
        php::PhpExpr::Binary { op, left, right } => {
            let phorj_op = lift_binop(*op)?;
            // DEC-512, third clause. PHP orders arrays; Phorj orders TUPLES and refuses `List<T>`
            // (their arities are static and dynamic respectively, and PHP's count-first rule only
            // agrees with lexicographic order when the arity is fixed). So in an ORDERING operand
            // position specifically, a positional array LITERAL lifts to a tuple:
            //
            //     [$a->tier, $a->position] <=> [$b->tier, $b->position]
            //       ->  (a.tier, a.position) <=> (b.tier, b.position)
            //
            // This does not violate DEC-166 ("the lifter never guesses"): a literal's arity is
            // syntactically present in the source, so nothing is inferred or invented. It is also
            // narrow on purpose — both sides must be positional literals of the SAME arity. A
            // variable, a call result, or a length mismatch keeps lifting to a list and is then
            // refused by the checker with `E-ORDER-LIST`, which is the honest outcome: the lifter
            // cannot know a runtime array's arity, and quietly assuming one would be the guess
            // DEC-166 forbids.
            let ordering = matches!(
                phorj_op,
                BinaryOp::Spaceship | BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge
            );
            let (mut lhs, mut rhs) = (lift_expr(left)?, lift_expr(right)?);
            if ordering {
                if let (
                    php::PhpExpr::Array(le),
                    php::PhpExpr::Array(re),
                    Expr::List(li, _),
                    Expr::List(ri, _),
                ) = (&**left, &**right, &lhs, &rhs)
                {
                    // Defence in depth, and deliberately redundant TODAY: `lift_array` yields an
                    // `Expr::List` only when no element carries a key (all-keyed becomes a `Map`,
                    // mixed refuses as Tier-2), so the destructure above already implies this.
                    // Mutating this predicate away therefore does NOT turn the suite red — stated
                    // here so a reader does not mistake it for a tested guard. It stops being
                    // redundant the moment `lift_array` admits an ascending `[0 => a, 1 => b]` as a
                    // list, which is why it is kept rather than deleted.
                    let positional = |es: &[php::PhpArrayElem]| es.iter().all(|e| e.key.is_none());
                    if !li.is_empty() && li.len() == ri.len() && positional(le) && positional(re) {
                        lhs = Expr::Tuple(li.clone(), SP);
                        rhs = Expr::Tuple(ri.clone(), SP);
                    }
                }
            }
            Expr::Binary {
                op: phorj_op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span: SP,
            }
        }
        // C-46: PHP `value instanceof ClassName` → Phorj's existing `instanceof` (M-RT S1).
        php::PhpExpr::InstanceOf { value, class } => Expr::InstanceOf {
            value: Box::new(lift_expr(value)?),
            type_name: class.clone(),
            span: SP,
        },
        php::PhpExpr::Cast { ty, value } => Expr::Cast {
            value: Box::new(lift_expr(value)?),
            type_name: ty.clone(),
            span: SP,
        },
        php::PhpExpr::EmptyColl(ty) => new_coll(&lift_type(ty)?)?,
        php::PhpExpr::AppendSlot(_) => {
            return Err("lift: `$xs[]` is only meaningful as the target of `=`".into());
        }
        php::PhpExpr::Assign { .. }
        | php::PhpExpr::CompoundAssign { .. }
        | php::PhpExpr::IncDec { .. } => {
            return Err("lift: assignment / `++` / `--` as a sub-expression is Tier-2".into());
        }
        php::PhpExpr::Ternary { cond, then, els } => {
            let then = then
                .as_ref()
                .ok_or("lift: elvis `?:` is Tier-2 (use a full ternary)")?;
            Expr::If {
                cond: Box::new(lift_expr(cond)?),
                then_expr: Box::new(lift_expr(then)?),
                else_expr: Box::new(lift_expr(els)?),
                span: SP,
            }
        }
        php::PhpExpr::Call { callee, args } => {
            // DEC-312: a bare PHP builtin with a registered inverse lifts to its Core form
            // (`strlen($s)` → `String.length(s)`, one registry row for both directions). Arity must
            // match the native's signature — a mismatched call falls through to the plain unresolved
            // lift (loud later, never a wrong guess).
            if let php::PhpExpr::Name(n) = &**callee {
                if let Some(nat) = crate::native::lift_of(n) {
                    if nat.params.len() == args.len() {
                        super::record_native_module(nat.module);
                        let mut lifted = lift_exprs(args)?;
                        // DEC-326: the RECEIVER form is the canonical style — a subject-first
                        // native lifts to `subject.name(rest…)` (UFCS erases it back to the module
                        // call pre-backend); a zero-arg native keeps the module form.
                        let (object, rest) = if lifted.is_empty() {
                            let q = nat.module.rsplit('.').next().expect("dotted module");
                            (Expr::Ident(q.to_string(), SP), Vec::new())
                        } else {
                            let recv = lifted.remove(0);
                            (recv, lifted)
                        };
                        return Ok(Expr::Call {
                            callee: Box::new(Expr::Member {
                                object: Box::new(object),
                                name: nat.name.to_string(),
                                safe: false,
                                sep: crate::ast::MemberSep::Dot,
                                span: SP,
                            }),
                            args: rest,
                            type_args: Vec::new(),
                            span: SP,
                        });
                    }
                }
            }
            Expr::Call {
                callee: Box::new(lift_expr(callee)?),
                args: lift_exprs(args)?,
                type_args: Vec::new(),
                span: SP,
            }
        }
        php::PhpExpr::MethodCall {
            recv,
            name,
            args,
            nullsafe,
        } => Expr::Call {
            callee: Box::new(Expr::Member {
                object: Box::new(lift_expr(recv)?),
                name: name.clone(),
                safe: *nullsafe,
                // PHP instance call `->`/`?->` (DEC-207).
                sep: crate::ast::MemberSep::Dot,
                span: SP,
            }),
            args: lift_exprs(args)?,
            type_args: Vec::new(),
            span: SP,
        },
        php::PhpExpr::Member {
            recv,
            name,
            nullsafe,
        } => Expr::Member {
            object: Box::new(lift_expr(recv)?),
            name: name.clone(),
            safe: *nullsafe,
            // PHP instance property `->`/`?->` (DEC-207).
            sep: crate::ast::MemberSep::Dot,
            span: SP,
        },
        php::PhpExpr::StaticCall { class, name, args } => Expr::Call {
            // DEC-509: a STATIC method the enum declares was lowered to a free function, so the call
            // site is a plain call. `Tenure::from(…)`/`cases()`/`tryFrom(…)` are the backed-enum
            // BUILTINS — not declared by the enum, so they keep naming it. Found by RUNNING the
            // example: the draft lifted, checked, then died on a name that did not exist.
            callee: if super::enums::is_lowered_method(class, name) {
                Box::new(Expr::Ident(name.clone(), SP))
            } else {
                Box::new(static_member(class, name))
            },
            args: lift_exprs(args)?,
            type_args: Vec::new(),
            span: SP,
        },
        php::PhpExpr::ClassConst { class, name } | php::PhpExpr::StaticProp { class, name } => {
            // `Tenure::PLAI` is an enum CASE; `Limits::MAX` is a class constant. PHP spells them
            // identically, so the enum registry is what tells them apart (lane L1). A payload-less
            // variant is CONSTRUCTED — `new PLAI()` — because construction is mandatory in phorj
            // (Invariant 12); emitting `Tenure::PLAI` produced a draft that lifted and then failed
            // `phg check` with `E-UNKNOWN-IDENT`.
            if super::enums::is_enum(class) {
                Expr::New(
                    Box::new(Expr::Call {
                        callee: Box::new(Expr::Ident(name.clone(), SP)),
                        args: Vec::new(),
                        type_args: Vec::new(),
                        span: SP,
                    }),
                    SP,
                )
            } else {
                static_member(class, name)
            }
        }
        php::PhpExpr::Index { base, index } => Expr::Index {
            object: Box::new(lift_expr(base)?),
            index: Box::new(lift_expr(index)?),
            span: SP,
        },
        php::PhpExpr::New { class, args } => Expr::New(
            Box::new(Expr::Call {
                // Two rules in one call (`phorj_error_name`): a PHP builtin EXCEPTION maps onto phorj's
                // standard taxonomy (DEC-421 — `\RuntimeException` → `RuntimeError`), and anything else
                // just loses PHP's root marker, which phorj has no spelling for. Mapping only the CATCH
                // clause and not the `new` left `throw new RuntimeException(…)` naming a type that does
                // not exist, so the draft still failed to check.
                callee: Box::new(Expr::Ident(
                    crate::lift::lifter::exceptions::phorj_error_name(class),
                    SP,
                )),
                args: lift_exprs(args)?,
                type_args: Vec::new(),
                span: SP,
            }),
            SP,
        ),
        php::PhpExpr::Match { subject, arms } => lift_match(subject, arms)?,
    })
}

pub(super) fn lift_exprs(es: &[php::PhpExpr]) -> Result<Vec<Expr>, String> {
    es.iter().map(lift_expr).collect()
}

pub(super) fn lift_array(elems: &[php::PhpArrayElem]) -> Result<Expr, String> {
    if elems.is_empty() {
        return Ok(Expr::List(Vec::new(), SP));
    }
    let any_key = elems.iter().any(|e| e.key.is_some());
    let all_key = elems.iter().all(|e| e.key.is_some());
    if any_key && !all_key {
        return Err("lift: a mixed keyed/positional array is Tier-2".into());
    }
    if all_key {
        let mut pairs = Vec::new();
        for e in elems {
            pairs.push((lift_expr(e.key.as_ref().unwrap())?, lift_expr(&e.value)?));
        }
        Ok(Expr::Map(pairs, SP))
    } else {
        let items: Result<Vec<_>, _> = elems.iter().map(|e| lift_expr(&e.value)).collect();
        Ok(Expr::List(items?, SP))
    }
}

/// A string LITERAL contributes its text directly (so `'a' . 'b'` is the one literal `"ab"`, not two
/// spliced parts); anything else becomes an interpolated expression, which is where PHP's coercion
/// happens and where phorj's interpolation does exactly the same thing. A nested interpolation on
/// either side is spliced in rather than re-wrapped, so `"x$a" . $b` stays one flat string.
fn flatten_concat(e: &php::PhpExpr, out: &mut Vec<StrPart>) -> Result<(), String> {
    match e {
        php::PhpExpr::Binary {
            op: php::PhpBinOp::Concat,
            left,
            right,
        } => {
            flatten_concat(left, out)?;
            flatten_concat(right, out)?;
        }
        php::PhpExpr::Str(s) => out.push(StrPart::Literal(s.clone())),
        php::PhpExpr::Interp(parts) => {
            for p in parts {
                out.push(match p {
                    php::PhpStrPart::Lit(s) => StrPart::Literal(s.clone()),
                    php::PhpStrPart::Expr(inner) => StrPart::Expr(Box::new(lift_expr(inner)?)),
                });
            }
        }
        other => out.push(StrPart::Expr(Box::new(lift_expr(other)?))),
    }
    Ok(())
}
