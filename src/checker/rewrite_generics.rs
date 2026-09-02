use super::*;

/// Erase generic type parameters from a checked program (M-RT S7). For every generic free function,
/// every type annotation that names one of *that function's* type parameters is rewritten to
/// `Type::Erased` and the parameter list is cleared, so the interpreter, compiler, and transpiler
/// all see an ordinary, type-variable-free function (PHP `mixed` at the boundary). This is the same
/// "compile-time-only, expanded out before any backend" discipline as `type` aliases and `html"…"`,
/// and it is what keeps generics zero-cost and byte-identical across the three backends: there is no
/// monomorphization, the type variables simply disappear after checking. Type parameters are scoped
/// to their own function, so only `Item::Function` items with a non-empty `type_params` are
/// rewritten; everything else is returned untouched (a program with no generics is byte-for-byte the
/// pre-S7 AST). Runs after a successful [`check`], so the `T`-bearing types it erases were already
/// validated.
#[path = "rewrite_generics_walk.rs"]
mod walk;
use walk::*;

pub fn erase_generics(program: Program) -> Program {
    use crate::ast::{ClassDecl, ClassMember, FunctionDecl, Item, Type};
    use std::collections::HashSet;

    type Params<'a> = HashSet<&'a str>;

    fn member_is_generic(m: &ClassMember) -> bool {
        matches!(m, ClassMember::Method(f) if !f.type_params.is_empty())
    }

    /// `Some(i)` when `f`'s return type is *exactly* a bare own type parameter (`-> T`, no args) and
    /// parameter `i` is annotated with that same parameter — `id<T>(T x) -> T` ⇒ 0,
    /// `firstOr<T>(List<T>, T) -> T` ⇒ 1, `applyTwice<T>(T, (T)->T) -> T` ⇒ 0. `None` otherwise (a
    /// container/concrete return, or no parameter directly carries the returned parameter). Computed
    /// from the *pre-erasure* signature; consumed only by the VM compiler's `ctype` (S2.1).
    fn generic_ret_echo_param(f: &FunctionDecl) -> Option<usize> {
        let ret_name = match f.ret.as_ref()? {
            Type::Named { name, args, .. } if args.is_empty() && f.type_params.contains(name) => {
                name
            }
            _ => return None,
        };
        f.params.iter().position(|p| {
            matches!(&p.ty, Type::Named { name, args, .. } if args.is_empty() && name == ret_name)
        })
    }

    /// Erase type parameters across a member list — shared by the class arm and the trait arm (CD-31).
    /// `outer` is the declaring type's own parameter set (a class's `<T>`); a trait declares none, so it
    /// passes an empty set and only each method's own `<U>` is erased.
    ///
    /// Extracted rather than copied: a trait's members are the identical `Vec<ClassMember>`, and the
    /// missing trait arm was an Invariant-1 spine break — the transpiler emitted the un-erased parameter
    /// as a PHP type (`function echoBack(U $x): U`) and the PHP leg died where both native legs ran.
    fn erase_members(members: Vec<ClassMember>, outer: &[&str]) -> Vec<ClassMember> {
        members
            .into_iter()
            .map(|m| match m {
                ClassMember::Method(f) => {
                    // erase the class's params *and* this method's own
                    let mut set: Params = outer.iter().copied().collect();
                    for tp in &f.type_params {
                        set.insert(tp.as_str());
                    }
                    ClassMember::Method(FunctionDecl {
                        modifiers: f.modifiers.clone(),
                        attrs: f.attrs.clone(),
                        vis: f.vis,
                        name: f.name.clone(),
                        type_params: Vec::new(), // erased
                        type_param_bounds: Vec::new(),
                        params: f.params.iter().map(|p| rparam(p, &set)).collect(),
                        ret: f.ret.as_ref().map(|t| rty(t, &set)),
                        throws: f.throws.iter().map(|t| rty(t, &set)).collect(),
                        body: f.body.iter().map(|s| rstmt(s, &set)).collect(),
                        foreign: f.foreign,
                        // S2.1 (methods): recover, before erasing the method's `<T>`, which
                        // argument the result echoes — so the VM compiler can specialize
                        // `u.pick(7, 8) + 1` exactly as the interpreter evaluates it. Computed
                        // from the pre-erasure signature (`generic_ret_echo_param` keys on the
                        // method's own `type_params`, so it never fires for a class-`T` return).
                        generic_ret_from_param: generic_ret_echo_param(&f),
                        span: f.span,
                    })
                }
                ClassMember::Field {
                    modifiers,
                    ty,
                    name,
                    init,
                    span,
                } => {
                    let set: Params = outer.iter().copied().collect();
                    ClassMember::Field {
                        modifiers,
                        ty: rty(&ty, &set),
                        name,
                        init: init.as_ref().map(|e| rexpr(e, &set)),
                        span,
                    }
                }
                ClassMember::Constructor {
                    modifiers,
                    params,
                    throws,
                    body,
                    span,
                } => {
                    let set: Params = outer.iter().copied().collect();
                    ClassMember::Constructor {
                        modifiers,
                        params: params.iter().map(|p| rctorparam(p, &set)).collect(),
                        // Erase the class type params from the ctor's `throws` types, like the fn path.
                        throws: throws.iter().map(|t| rty(t, &set)).collect(),
                        body: body.iter().map(|s| rstmt(s, &set)).collect(),
                        span,
                    }
                }
                // A property hook (M-mut.7b): erase the class params from its type, get
                // expression, and set parameter+block (a hook declares no `<T>` of its own).
                ClassMember::Hook {
                    ty,
                    name,
                    get,
                    set: setter,
                    span,
                } => {
                    let set: Params = outer.iter().copied().collect();
                    ClassMember::Hook {
                        ty: rty(&ty, &set),
                        name,
                        get: get.as_ref().map(|e| rexpr(e, &set)),
                        set: setter.as_ref().map(|(p, b)| {
                            (rparam(p, &set), b.iter().map(|s| rstmt(s, &set)).collect())
                        }),
                        span,
                    }
                }
            })
            .collect()
    }

    let Program {
        package,
        items,
        span,
    } = program;
    let items = items
        .into_iter()
        .map(|item| match item {
            Item::Function(f) if !f.type_params.is_empty() => {
                let params: Params = f.type_params.iter().map(String::as_str).collect();
                // Recover, before erasing the type parameters, which argument (if any) the result
                // echoes — so the VM compiler can later specialize `id(7) + 1` (S2.1).
                let generic_ret_from_param = generic_ret_echo_param(&f);
                Item::Function(FunctionDecl {
                    modifiers: f.modifiers.clone(),
                    attrs: f.attrs.clone(),
                    vis: f.vis,
                    name: f.name.clone(),
                    type_params: Vec::new(), // erased
                    type_param_bounds: Vec::new(),
                    params: f.params.iter().map(|p| rparam(p, &params)).collect(),
                    ret: f.ret.as_ref().map(|t| rty(t, &params)),
                    throws: f.throws.iter().map(|t| rty(t, &params)).collect(),
                    body: f.body.iter().map(|s| rstmt(s, &params)).collect(),
                    foreign: f.foreign,
                    generic_ret_from_param,
                    span: f.span,
                })
            }
            // A generic class (class-level `<T>`) and/or a class with a generic method (M-RT
            // generics-all): erase the class's type parameters across *every* member (field types,
            // constructor parameters, method signatures + bodies) and each generic method's own
            // `<U>`, then clear all type-parameter lists. The class's `<T>`-typed members become PHP
            // `mixed`; the class declaration itself stays (just non-generic). A class with neither
            // class-level params nor a generic method is returned untouched by the `other` arm, so a
            // non-generic program is byte-for-byte the pre-generics AST.
            Item::Class(c)
                if !c.type_params.is_empty() || c.members.iter().any(member_is_generic) =>
            {
                let class_params: Vec<&str> = c.type_params.iter().map(String::as_str).collect();
                let members = erase_members(c.members, &class_params);
                Item::Class(ClassDecl {
                    vis: c.vis,
                    attrs: c.attrs,
                    name: c.name,
                    type_params: Vec::new(), // erased
                    type_param_bounds: Vec::new(),
                    extends: c.extends,
                    // Interface type ARGUMENTS are erased with the rest of the generic machinery
                    // (DEC-257) — the backends only ever read the interface names.
                    implements_args: vec![Vec::new(); c.implements.len()],
                    implements: c.implements,
                    open: c.open,
                    is_abstract: c.is_abstract,
                    sealed: c.sealed,
                    resolutions: c.resolutions,
                    uses: c.uses,
                    members,
                    foreign: c.foreign,
                    span: c.span,
                })
            }
            // A generic enum (`Option<T>`/`Result<T, E>`, M-RT generic enums): erase the enum's type
            // parameters across every variant's field types (a `T` payload becomes PHP `mixed`) and
            // clear the parameter list, so the backends see an ordinary, type-variable-free enum.
            // Same "expanded out before any backend" discipline as a generic class.
            Item::Enum(e) if !e.type_params.is_empty() => {
                let params: Params = e.type_params.iter().map(String::as_str).collect();
                Item::Enum(crate::ast::EnumDecl {
                    vis: e.vis,
                    name: e.name,
                    type_params: Vec::new(), // erased
                    type_param_bounds: Vec::new(),
                    backing_type: e.backing_type, // None here (a generic enum is never backed)
                    variants: e
                        .variants
                        .into_iter()
                        .map(|v| crate::ast::EnumVariant {
                            name: v.name,
                            fields: v.fields.iter().map(|p| rparam(p, &params)).collect(),
                            backing_value: v.backing_value,
                            span: v.span,
                        })
                        .collect(),
                    injected: e.injected,
                    span: e.span,
                })
            }
            // A generic interface (`Iterator<T>`, DEC-257): erase the interface's type parameters
            // across every method signature (a `T` return/param becomes `Type::Erased`) and clear
            // the parameter list — same discipline as classes/enums; the transpiler emits an
            // ordinary PHP interface from the result.
            Item::Interface(i) if !i.type_params.is_empty() => {
                let params: Params = i.type_params.iter().map(String::as_str).collect();
                Item::Interface(crate::ast::InterfaceDecl {
                    vis: i.vis,
                    name: i.name,
                    type_params: Vec::new(), // erased
                    extends: i.extends,
                    methods: i
                        .methods
                        .into_iter()
                        .map(|m| crate::ast::FunctionDecl {
                            params: m.params.iter().map(|p| rparam(p, &params)).collect(),
                            ret: m.ret.as_ref().map(|t| rty(t, &params)),
                            throws: m.throws.iter().map(|t| rty(t, &params)).collect(),
                            ..m
                        })
                        .collect(),
                    sealed: i.sealed,
                    injected: i.injected,
                    span: i.span,
                })
            }
            // CD-31 — this arm was MISSING and it was an Invariant-1 spine break. A trait declares
            // no type parameters of its own, but its METHODS may (`function echoBack<U>(U x) -> U`),
            // and a trait's members flatten into the using class. Falling through left `U` un-erased:
            // both native legs ran (they ignore the stray `Ty::Param`) while the transpiler emitted
            // `function echoBack(U $x): U` and PHP died with `TypeError: must be of type U` — the
            // three legs disagreeing, which is exactly what Invariant 1 forbids.
            Item::Trait(t) if t.members.iter().any(member_is_generic) => {
                Item::Trait(crate::ast::TraitDecl {
                    name: t.name,
                    members: erase_members(t.members, &[]),
                    span: t.span,
                })
            }
            // Everything with nothing to erase: a non-generic function/class/trait, an enum or
            // interface without type parameters, a test body (which declares no items), and the two
            // genuine leaves. Named rather than swept into `other => other` so that a new `Item`
            // form — or a new position where a type parameter can be written — has to be ruled on
            // here instead of silently reaching a backend un-erased (CD-31).
            it @ (Item::Function(..)
            | Item::Class(..)
            | Item::Trait(..)
            | Item::Enum(..)
            | Item::Interface(..)
            | Item::Test { .. }
            | crate::item_leaves!()) => it,
        })
        .collect();
    Program {
        package,
        items,
        span,
    }
}
