//! Bytecode compiler — constructor + method body compilation.

use super::*;

/// Compile one class's synthetic constructor `<Class>::new` (decision P4-4). Layout: ctor params
/// occupy slots `0..nparams`; the prologue loads the promoted params and `MakeInstance` builds the
/// instance into slot `nparams`; the body runs for side effects with the instance live; the
/// epilogue loads and returns that instance. The body's own `return`s are redirected to the
/// epilogue (never an `Op::Return`), so — exactly like the interpreter — a ctor body cannot change
/// the result: the promoted instance is always returned.
#[allow(clippy::too_many_arguments)]
pub(super) fn compile_constructor<'a>(
    program: &Program,
    c: &ClassDecl,
    desc_idx: usize,
    fns: &'a HashMap<String, FnMeta>,
    arities: &'a [usize],
    variants: &'a HashMap<String, VariantMeta>,
    enum_descs: &'a [EnumDesc],
    classes: &'a HashMap<String, usize>,
    statics_index: &'a HashMap<(String, String), (usize, CTy)>,
    consts_index: &'a HashMap<(String, String), (Value, CTy)>,
    class_descs: &'a [ClassDesc],
    names_index: &'a HashMap<String, usize>,
    field_tags: &'a HashMap<String, CTy>,
    class_field_ctys: &'a HashMap<String, HashMap<String, CTy>>,
    method_rets: &'a HashMap<(String, String), CTy>,
    method_generic_ret_from_param: &'a HashMap<(String, String), usize>,
    reified_operands: &'a HashMap<usize, CTy>,
    methods: &'a HashMap<(String, String), usize>,
    method_overloads: &'a HashMap<(String, String), usize>,
    parent_parents: &'a std::collections::BTreeMap<String, Vec<String>>,
    base_fn_idx: usize,
) -> Result<(Function, Vec<Function>), String> {
    // M-RT S6c.2: a no-own-ctor class compiles its inherited constructor *plan* — for single
    // inheritance the parent's, for multiple inheritance every parent's, in `extends` order. The
    // synthetic function stays `<Class>::new`; `MakeInstance(desc_idx)` builds a *this*-class instance
    // populated with every promoted param across the plan, then each plan entry's body runs in turn.
    let plan = crate::ast::ctor_plan(program, &c.name);
    let all_params: Vec<&CtorParam> = plan.iter().flat_map(|(p, _)| p.iter()).collect();
    let line = c.span.line;
    let mut comp = Compiler::new(
        fns,
        arities,
        variants,
        enum_descs,
        classes,
        statics_index,
        consts_index,
        class_descs,
        names_index,
        field_tags,
        class_field_ctys,
        method_rets,
        method_generic_ret_from_param,
        reified_operands,
        methods,
        method_overloads,
        base_fn_idx,
    );
    comp.cur_class = Some(c.name.clone()); // `this` resolves to this class (ctype)
    comp.parent_parents = Some(parent_parents); // M-RT super/parent resolution in a ctor body
    for p in &all_params {
        comp.add_local(&p.name, resolve_cty(&p.ty));
    }
    comp.height = comp.locals.len();
    // Prologue: load every promoted param across the plan, in slot order, then build the instance.
    // `MakeInstance` pops exactly those values (matching `class_descs[desc_idx].fields`, built from the
    // same flattened plan), so the order lines up.
    for (slot, p) in all_params.iter().enumerate() {
        if is_promoted(p) {
            comp.emit(Op::GetLocal(slot), line);
        }
    }
    comp.emit(Op::MakeInstance(desc_idx), line);
    let inst_slot = comp.add_local("$this", CTy::Other);
    comp.this_slot = Some(inst_slot); // a ctor body may reference `this` / bare fields
                                      // Feature B: expression field initializers run after promotion and before the ctor body, in
                                      // declaration order (base-first across ancestors). Each lowers to `this.field = <init>`:
                                      // load the instance, compile the initializer (with `this` live, so it reads an earlier sibling),
                                      // then `SetField`. The shared `ast::field_initializers` list keeps all three backends in lockstep.
    for (fname, init) in crate::ast::field_initializers(program, &c.name) {
        comp.emit(Op::GetLocal(inst_slot), line); // [this]
        comp.expr(&init)?; // [this, value]
        let idx = comp.field_name_index(&fname)?;
        comp.emit(Op::SetField(idx), line); // mutate in place, pop both
    }
    // Each plan entry's body runs in sequence on the one instance. An early `return` ends *that*
    // body only (matching the interpreter's separate per-parent call) — so each gets its own
    // return-jump set, patched to the end of that body, never skipping a later parent's init.
    for (_, body) in &plan {
        comp.ctor_return_jumps = Some(Vec::new());
        for s in body {
            comp.stmt(s)?;
        }
        let jumps = comp.ctor_return_jumps.take().unwrap_or_default();
        for j in jumps {
            comp.patch_jump(j);
        }
    }
    // Epilogue: load and return the constructed instance (a ctor body cannot change the result).
    comp.emit(Op::GetLocal(inst_slot), line);
    comp.emit(Op::Return, line);
    Ok((
        Function {
            name: format!("{}::new", c.name),
            arity: all_params.len(),
            n_captures: 0,    // constructors are never closures
            unchecked: false, // `#[UncheckedOverflow]` is free-function-only (parser rejects it on methods/ctors)
            chunk: comp.chunk,
        },
        comp.extra_functions,
    ))
}

/// Compile one instance method as a function (decision P4-6). Layout: slot 0 is the receiver
/// (`this`), slots `1..=nparams` are the params; the body runs with `this` and the class's field
/// scope live; an implicit `Unit` return terminates it (P3-7). The frame is opened by
/// `Op::CallMethod`, which places the receiver at slot 0.
#[allow(clippy::too_many_arguments)]
pub(super) fn compile_method<'a>(
    class_name: &str,
    f: &FunctionDecl,
    fns: &'a HashMap<String, FnMeta>,
    arities: &'a [usize],
    variants: &'a HashMap<String, VariantMeta>,
    enum_descs: &'a [EnumDesc],
    classes: &'a HashMap<String, usize>,
    statics_index: &'a HashMap<(String, String), (usize, CTy)>,
    consts_index: &'a HashMap<(String, String), (Value, CTy)>,
    class_descs: &'a [ClassDesc],
    names_index: &'a HashMap<String, usize>,
    field_tags: &'a HashMap<String, CTy>,
    class_field_ctys: &'a HashMap<String, HashMap<String, CTy>>,
    method_rets: &'a HashMap<(String, String), CTy>,
    method_generic_ret_from_param: &'a HashMap<(String, String), usize>,
    reified_operands: &'a HashMap<usize, CTy>,
    methods: &'a HashMap<(String, String), usize>,
    method_overloads: &'a HashMap<(String, String), usize>,
    parent_parents: &'a std::collections::BTreeMap<String, Vec<String>>,
    base_fn_idx: usize,
    // Batch-1 D: the non-literal static-init prelude, non-empty only for a class-static `main` entry
    // (emitted before the body, exactly as a top-level `main` does it). `&[]` for every other method.
    static_prelude: &[(usize, &Expr)],
) -> Result<(Function, Vec<Function>), String> {
    let mut comp = Compiler::new(
        fns,
        arities,
        variants,
        enum_descs,
        classes,
        statics_index,
        consts_index,
        class_descs,
        names_index,
        field_tags,
        class_field_ctys,
        method_rets,
        method_generic_ret_from_param,
        reified_operands,
        methods,
        method_overloads,
        base_fn_idx,
    );
    comp.cur_class = Some(class_name.to_string()); // `this` resolves to this class (ctype)
    comp.parent_parents = Some(parent_parents); // M-RT super/parent resolution in a method body
    comp.add_local("$this", CTy::Other); // slot 0 = receiver
    for p in &f.params {
        comp.add_local(&p.name, resolve_cty(&p.ty));
    }
    comp.this_slot = Some(0);
    comp.height = comp.locals.len();
    let last_line = f.span.line;
    // Static-init prelude (class-static `main` entry only) — `<init>` then `SetStatic(slot)`, before
    // any user code; the statics are class-level so no `this`/locals are read.
    for (slot, init) in static_prelude {
        comp.expr(init)?;
        comp.emit(Op::SetStatic(*slot), last_line);
    }
    for s in &f.body {
        comp.stmt(s)?;
    }
    comp.emit_const(Value::Unit, last_line);
    comp.emit(Op::Return, last_line);
    Ok((
        Function {
            name: format!("{class_name}::{}", f.name),
            arity: 1 + f.params.len(),
            n_captures: 0,    // methods are never closures
            unchecked: false, // `#[UncheckedOverflow]` is free-function-only (parser rejects it on methods)
            chunk: comp.chunk,
        },
        comp.extra_functions,
    ))
}
