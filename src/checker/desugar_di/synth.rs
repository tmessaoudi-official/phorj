//! DI v1 — synthetic factory construction: names, spans, the generated `phorjInject<T>`
//! items and their construction expressions.

use super::*;

// A camelCase synthetic factory name (free functions are `E-NAME-CASE`-checked, so the `__phorj_`
// transpile-helper convention can't be used here). Type names are PascalCase, so `phorjInject<Type>` is
// camelCase. Collision with a hand-written `phorjInject…` function is astronomically unlikely and
// disclosed (KNOWN_ISSUES); a dedicated reserved-prefix check is a later refinement.
pub(super) fn factory_name(t: &str) -> String {
    format!("phorjInject{t}")
}

pub(super) fn di_span() -> Span {
    Span {
        start: 0,
        len: 0,
        line: 1,
        col: 1,
    }
}

pub(super) fn err(message: String, span: Span) -> Diagnostic {
    Diagnostic::new(Stage::Type, message, span.line, span.col)
}

/// A local-variable name for a class instance inside a factory body (unique per class → sharing).
/// Class names are PascalCase, so `di<Class>` is camelCase (locals are convention-checked too).
pub(super) fn inst_var(class: &str) -> String {
    format!("di{class}")
}

/// Build `function phorjInject<T>(): T { var di<K> = <construct>; …; return <root>; }` by LET-FLOATING
/// the [`Built`] tree (slice 4b): every SHARED node is hoisted to a `var` (emitted once, in topological
/// deps-first order) and referenced by each dependent; every TRANSIENT node is inlined fresh at each use.
/// Construction kind (new-vs-provides) and sharing (shared-vs-transient) are orthogonal — `build_expr`
/// emits all four combinations. For an all-shared graph this is byte-identical to the pre-4b flat plan
/// (post-order hoist order matches), which is the regression guard for the shipped slices.
pub(super) fn synth_factory(requested: &str, root: &Built) -> Item {
    let sp = di_span();
    // Shared keys in topological (deps-first) order, deduped by first completion; `node_for` keeps the
    // first `Built` seen for each shared key (any occurrence has the same construct + deps).
    let mut shared_order: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut node_for: BTreeMap<String, &Built> = BTreeMap::new();
    pub(super) fn collect<'a>(
        node: &'a Built,
        order: &mut Vec<String>,
        seen: &mut BTreeSet<String>,
        node_for: &mut BTreeMap<String, &'a Built>,
    ) {
        // Descend through EVERY node (incl. transient) so a shared dep nested under a transient is still
        // hoisted; only SHARED nodes are collected as hoisted vars.
        for d in &node.deps {
            collect(d, order, seen, node_for);
        }
        if !node.transient && seen.insert(node.key.clone()) {
            node_for.insert(node.key.clone(), node);
            order.push(node.key.clone());
        }
    }
    collect(root, &mut shared_order, &mut seen, &mut node_for);

    let mut body: Vec<Stmt> = shared_order
        .iter()
        .map(|key| {
            let node = node_for[key];
            Stmt::VarDecl {
                ty: named_type(key, sp),
                name: inst_var(key),
                init: build_expr(node, sp),
                mutable: false,
                span: sp,
            }
        })
        .collect();
    // The root: a shared root is its hoisted var; a transient root is inlined.
    let ret_value = if root.transient {
        build_expr(root, sp)
    } else {
        Expr::Ident(inst_var(&root.key), sp)
    };
    body.push(Stmt::Return {
        value: Some(ret_value),
        span: sp,
    });
    Item::Function(crate::ast::FunctionDecl {
        modifiers: Vec::new(),
        attrs: Vec::new(),
        vis: crate::ast::Visibility::Public,
        name: factory_name(requested),
        type_params: Vec::new(),
        params: Vec::new(),
        // Return the REQUESTED type (`inject<Logger>()` returns `Logger`, built as its single impl —
        // assignable), so the call site types exactly as the user wrote.
        ret: Some(named_type(requested, sp)),
        throws: Vec::new(),
        body,
        foreign: false,
        generic_ret_from_param: None,
        span: sp,
    })
}

/// The construction expression for one node: a SHARED dependency resolves to its hoisted `var` ident
/// (`inst_var`), a TRANSIENT dependency is inlined by recursing (a fresh construction each time). The
/// construct kind chooses `new <class>(args)` or the static factory call `<owner>.<method>(args)`.
pub(super) fn build_expr(node: &Built, sp: Span) -> Expr {
    let args: Vec<Expr> = node
        .deps
        .iter()
        .map(|d| {
            if d.transient {
                build_expr(d, sp)
            } else {
                Expr::Ident(inst_var(&d.key), sp)
            }
        })
        .collect();
    match &node.construct {
        Construct::New(class) => Expr::New(
            Box::new(Expr::Call {
                callee: Box::new(Expr::Ident(class.clone(), sp)),
                args,
                span: sp,
            }),
            sp,
        ),
        // `<owner>.<method>(args)` — a static factory call (a `Member` callee on the owner class name,
        // exactly like `Color.of(…)`); NO `new`, so the provider fully controls construction.
        Construct::Provides(owner, method) => Expr::Call {
            callee: Box::new(Expr::Member {
                object: Box::new(Expr::Ident(owner.clone(), sp)),
                name: method.clone(),
                safe: false,
                sep: crate::ast::MemberSep::Dot,
                span: sp,
            }),
            args,
            span: sp,
        },
    }
}

pub(super) fn named_type(name: &str, span: Span) -> Type {
    Type::Named {
        name: name.to_string(),
        args: Vec::new(),
        span,
    }
}
