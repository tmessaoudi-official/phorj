//! Scope, position, and outline helpers for LSP v2 (locals/params resolution, completion, document
//! symbols). Pure front-end queries over the parsed AST + raw buffer — **off the byte-identity spine**
//! (no backend is touched), so they carry no interp/VM/PHP parity risk.
//!
//! The enclosing callable for a cursor is found by *source ordering*, not span containment: top-level
//! items (and a class's members) are emitted in source order with ascending spans, so item `i` owns the
//! byte range `[item[i].start .. item[i+1].start)`. That is robust without precise end-spans (a decl's
//! `Span` covers only its keyword/name).

use crate::ast::{ClassMember, Item, Program, Stmt, Type};
use crate::token::Span;

/// Convert a byte `offset` into 0-based `(line, character)`, counting `character` in Unicode scalars
/// (the inverse of [`super::symbols::offset_at`]). Past-EOF clamps to the final position.
pub fn position_at(text: &str, offset: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, c) in text.char_indices() {
        if i >= offset {
            return (line, col);
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// The `Span` of a top-level item (its keyword/name position).
pub fn item_span(item: &Item) -> Span {
    match item {
        Item::Import { span, .. } | Item::TypeAlias { span, .. } | Item::Test { span, .. } => *span,
        Item::Function(f) => f.span,
        Item::Enum(e) => e.span,
        Item::Class(c) => c.span,
        Item::Interface(i) => i.span,
        Item::Trait(t) => t.span,
    }
}

/// The span of a class member (its keyword/name position).
fn member_span(m: &ClassMember) -> Span {
    match m {
        ClassMember::Field { span, .. }
        | ClassMember::Constructor { span, .. }
        | ClassMember::Hook { span, .. } => *span,
        ClassMember::Method(f) => f.span,
    }
}

/// Collect every local binding `(name, decl span)` introduced anywhere in `body`, recursing nested
/// blocks. Source order; the caller filters by position for shadowing/scoping. Covers the
/// statement-level binders: `var`/`Type x =`, `for` var, `if (var x = …)`, `catch`, and destructuring.
/// Lambda-parameter and match-pattern binders (expr-nested) are a documented v2.1 deferral.
pub fn collect_bindings(body: &[Stmt], out: &mut Vec<(String, Span)>) {
    for s in body {
        match s {
            Stmt::VarDecl { name, span, .. } => out.push((name.clone(), *span)),
            Stmt::For {
                name,
                val,
                body,
                span,
                ..
            } => {
                out.push((name.clone(), *span));
                if let Some((_, vname)) = val {
                    out.push((vname.clone(), *span));
                }
                collect_bindings(body, out);
            }
            Stmt::If {
                bind,
                then_block,
                else_block,
                span,
                ..
            } => {
                if let Some(n) = bind {
                    out.push((n.clone(), *span));
                }
                collect_bindings(then_block, out);
                if let Some(e) = else_block {
                    collect_bindings(e, out);
                }
            }
            Stmt::While { body, .. } => collect_bindings(body, out),
            Stmt::CFor {
                init, step, body, ..
            } => {
                if let Some(i) = init {
                    collect_bindings(std::slice::from_ref(&**i), out);
                }
                if let Some(st) = step {
                    collect_bindings(std::slice::from_ref(&**st), out);
                }
                collect_bindings(body, out);
            }
            Stmt::Block(b, _) => collect_bindings(b, out),
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                collect_bindings(body, out);
                for c in catches {
                    out.push((c.name.clone(), c.span));
                    collect_bindings(&c.body, out);
                }
                if let Some(f) = finally_block {
                    collect_bindings(f, out);
                }
            }
            Stmt::Destructure { pat, .. } => {
                for (n, sp) in pat.binders() {
                    out.push((n, sp));
                }
            }
            _ => {}
        }
    }
}

/// Every local binding visible in the callable that encloses `offset` — its parameters plus all
/// statement-level binders in its body. Empty if the cursor is not inside a function / method /
/// constructor / test body.
pub fn enclosing_bindings(program: &Program, offset: usize) -> Vec<(String, Span)> {
    let Some(item) = enclosing_item(program, offset) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Span)> = Vec::new();
    match item {
        Item::Function(f) => {
            for p in &f.params {
                out.push((p.name.clone(), p.span));
            }
            collect_bindings(&f.body, &mut out);
        }
        Item::Test { body, .. } => collect_bindings(body, &mut out),
        Item::Class(c) => {
            if let Some(m) = enclosing_member(&c.members, offset) {
                match m {
                    ClassMember::Method(f) => {
                        for p in &f.params {
                            out.push((p.name.clone(), p.span));
                        }
                        collect_bindings(&f.body, &mut out);
                    }
                    ClassMember::Constructor { params, body, .. } => {
                        for p in params {
                            out.push((p.name.clone(), p.span));
                        }
                        collect_bindings(body, &mut out);
                    }
                    ClassMember::Hook { set, .. } => {
                        if let Some((p, body)) = set {
                            out.push((p.name.clone(), p.span));
                            collect_bindings(body, &mut out);
                        }
                    }
                    ClassMember::Field { .. } => {}
                }
            }
        }
        _ => {}
    }
    out
}

/// The local binding `name` resolves to at `offset`: the nearest *preceding* declaration of that name
/// in the enclosing callable (shadowing-correct for the common declare-above/use-below case). `None`
/// if no such local (the caller then tries top-level resolution).
pub fn local_definition(program: &Program, name: &str, offset: usize) -> Option<Span> {
    enclosing_bindings(program, offset)
        .into_iter()
        .filter(|(n, sp)| n == name && sp.start <= offset)
        .max_by_key(|(_, sp)| sp.start)
        .map(|(_, sp)| sp)
}

/// The top-level item whose source range `[start .. next_start)` contains `offset`.
pub fn enclosing_item(program: &Program, offset: usize) -> Option<&Item> {
    let items = &program.items;
    for (i, it) in items.iter().enumerate() {
        let start = item_span(it).start;
        let end = items.get(i + 1).map_or(usize::MAX, |n| item_span(n).start);
        if offset >= start && offset < end {
            return Some(it);
        }
    }
    None
}

/// The class member whose source range `[start .. next_start)` contains `offset` (members are
/// source-ordered, same heuristic as [`enclosing_item`]).
fn enclosing_member(members: &[ClassMember], offset: usize) -> Option<&ClassMember> {
    for (i, m) in members.iter().enumerate() {
        let start = member_span(m).start;
        let end = members
            .get(i + 1)
            .map_or(usize::MAX, |n| member_span(n).start);
        if offset >= start && offset < end {
            return Some(m);
        }
    }
    None
}

/// Resolve the declared class/type NAME of a receiver identifier at `offset`, for instance member
/// completion (`this.` / `myVar.`). `this` → the enclosing class; otherwise the declared `Type::Named`
/// head of a matching parameter, `var`, class field, or ctor-promoted param in scope. `None` when the
/// receiver is untyped / inferred (`var x = …`) / out of scope — completion then emits nothing (the
/// same conservative gate the module-member path uses; a wrong member list is worse than none).
pub(super) fn receiver_type_name(
    program: &Program,
    offset: usize,
    receiver: &str,
) -> Option<String> {
    let item = enclosing_item(program, offset)?;
    if receiver == "this" {
        return match item {
            Item::Class(c) => Some(c.name.clone()),
            _ => None,
        };
    }
    let ty = match item {
        Item::Function(f) => typed_binder_in_fn(f, receiver),
        Item::Class(c) => class_receiver_type(c, offset, receiver),
        _ => None,
    }?;
    named_head(ty)
}

/// The head name of a nominal type — `Type::Named` (unwrapping a top-level `T?`). `None` for
/// non-nominal types (union / intersection / function / inferred): there is no single class whose
/// members we could list.
fn named_head(ty: &Type) -> Option<String> {
    match ty {
        Type::Named { name, .. } => Some(name.clone()),
        Type::Optional { inner, .. } => named_head(inner),
        _ => None,
    }
}

/// A parameter or local `var` of a function naming `receiver` (declared type only — `var x = expr`
/// with inferred type yields `Type::Infer`, which `named_head` rejects).
fn typed_binder_in_fn<'a>(f: &'a crate::ast::FunctionDecl, receiver: &str) -> Option<&'a Type> {
    for p in &f.params {
        if p.name == receiver {
            return Some(&p.ty);
        }
    }
    typed_binder_in_stmts(&f.body, receiver)
}

/// A local `var <receiver>: T` declared anywhere in `stmts` (a completion hint — scope-precise
/// nearest-preceding isn't required to name a type).
fn typed_binder_in_stmts<'a>(stmts: &'a [Stmt], receiver: &str) -> Option<&'a Type> {
    for s in stmts {
        if let Stmt::VarDecl { ty, name, .. } = s {
            if name == receiver {
                return Some(ty);
            }
        }
    }
    None
}

/// The declared type of `receiver` seen from inside class `c`: an instance field or ctor-promoted
/// param (both are instance members), else a param / local of the enclosing method or constructor.
fn class_receiver_type<'a>(
    c: &'a crate::ast::ClassDecl,
    offset: usize,
    receiver: &str,
) -> Option<&'a Type> {
    for m in &c.members {
        match m {
            ClassMember::Field { name, ty, .. } if name == receiver => return Some(ty),
            ClassMember::Constructor { params, .. } => {
                for p in params {
                    if p.name == receiver {
                        return Some(&p.ty);
                    }
                }
            }
            _ => {}
        }
    }
    match enclosing_member(&c.members, offset)? {
        ClassMember::Method(f) => typed_binder_in_fn(f, receiver),
        ClassMember::Constructor { params, body, .. } => {
            for p in params {
                if p.name == receiver {
                    return Some(&p.ty);
                }
            }
            typed_binder_in_stmts(body, receiver)
        }
        _ => None,
    }
}
