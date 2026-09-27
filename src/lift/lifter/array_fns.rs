//! Row 5f — PHP array functions whose Phorj form is a receiver-form `Core.List` call but which a
//! `lift_from` row cannot express: `array_map` swaps its arguments, and `usort` sorts its argument
//! BY REFERENCE and returns `bool`, so it only has a meaning as a statement. Every other shape
//! returns `None` and falls through to the plain unresolved call — loud at `phg check`, never a
//! guess. Stability needs no work: `sortWith` and PHP 8 `usort` are both stable, pinned by the
//! DEC-505 differential test.

use super::*;

/// `recv.name(arg)`, recording `Core.List` for the import pass. Called only after every operand has
/// lifted, so a failed lift never leaves an import behind.
fn list_receiver_call(recv: Expr, name: &str, arg: Expr) -> Expr {
    record_native_module("Core.List");
    Expr::Call {
        callee: Box::new(Expr::Member {
            object: Box::new(recv),
            name: name.to_string(),
            safe: false,
            sep: crate::ast::MemberSep::Dot,
            span: SP,
        }),
        args: vec![arg],
        type_args: Vec::new(),
        span: SP,
    }
}

/// `array_map($f, $xs)` → `xs.map(f)`. Exactly one array: PHP zips any further arrays, which
/// `map` cannot express.
pub(super) fn lift_array_map(
    callee: &php::PhpExpr,
    args: &[php::PhpExpr],
) -> Option<Result<Expr, String>> {
    let (php::PhpExpr::Name(n), [f, xs]) = (callee, args) else {
        return None;
    };
    // Row 4i: the receiver form evaluates `$xs` before `$f`, so the swap needs one inert side.
    if n != "array_map" || !super::reordered::reorderable(&[f, xs]) {
        return None;
    }
    Some((|| {
        let recv = lift_expr(xs)?;
        let f = lift_expr(f)?;
        Ok(list_receiver_call(recv, "map", f))
    })())
}

/// The by-reference builtins this lift lowers to a REASSIGNMENT of their first argument —
/// `usort` here (row 5f), `array_unshift` in `spread.rs` (DEC-538). A parameter passed there is
/// written, so it is copied (row 4q, DEC-548); a lowering added to the statement arm joins this list.
pub(in crate::lift::lifter) const REASSIGNING_BUILTINS: &[&str] = &["usort", "array_unshift"];

/// The statement `usort($xs, $cmp);` → `xs = xs.sortWith(cmp);`. The target must be a plain
/// variable: a field or element target would need a write this lift does not guess.
pub(in crate::lift::lifter) fn lift_usort_stmt(e: &php::PhpExpr) -> Option<Result<Stmt, String>> {
    let php::PhpExpr::Call { callee, args } = e else {
        return None;
    };
    let (php::PhpExpr::Name(n), [target @ php::PhpExpr::Var(_), cmp]) =
        (callee.as_ref(), args.as_slice())
    else {
        return None;
    };
    if n != "usort" {
        return None;
    }
    Some((|| {
        let recv = lift_expr(target)?;
        let cmp = lift_expr(cmp)?;
        Ok(Stmt::Assign {
            target: lift_expr(target)?,
            value: list_receiver_call(recv, "sortWith", cmp),
            span: SP,
        })
    })())
}
