//! Interpreter — the `Interp` engine core: program collection, static inits, call frames,
//! statement execution loop, fault dumps, debug pausing.

use super::*;

impl<'c> Interp<'c> {
    pub(super) fn collect(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::Function(f) => {
                    // M-RT overloading: a same-named function joins an overload set (the checker has
                    // already validated legality). Declaration order is preserved.
                    self.funcs
                        .entry(f.name.clone())
                        .or_default()
                        .push(f.clone());
                }
                Item::Enum(e) => {
                    for v in &e.variants {
                        self.variants
                            .insert(v.name.clone(), (e.name.clone(), v.fields.len()));
                    }
                }
                Item::Class(c) => {
                    // Seed `static` field storage once at load from the literal-const initializer
                    // (M-mut.7) — the same `const_literal` kernel the VM's `static_inits` uses (F3).
                    for m in &c.members {
                        if let crate::ast::ClassMember::Field {
                            modifiers,
                            name,
                            init,
                            ..
                        } = m
                        {
                            if modifiers.contains(&crate::ast::Modifier::Static) {
                                let v = init
                                    .as_ref()
                                    .and_then(crate::value::const_literal)
                                    .unwrap_or(Value::Unit);
                                self.statics.insert((c.name.clone(), name.clone()), v);
                            }
                        }
                    }
                    self.classes.insert(c.name.clone(), c.clone());
                }
                // Interfaces have no runtime instances; they contribute only to the
                // `class_implements` table built below (used by `instanceof`).
                Item::Interface(_) => {}
                // M-RT S8: a trait is registered as a synthetic class so `call_method` can resolve a
                // trait-supplied method body — the shared `class_method_origins` maps a using class's
                // method to its `(trait, m)` origin, and the body is looked up via `self.classes`. A
                // trait is never instantiated and never enters the subtype table, so this entry only
                // ever serves method-body lookup.
                Item::Trait(t) => {
                    self.classes.insert(
                        t.name.clone(),
                        crate::ast::ClassDecl {
                            vis: crate::ast::Visibility::Public,
                            attrs: Vec::new(), // synthetic trait→class carries no attributes
                            name: t.name.clone(),
                            type_params: Vec::new(),
                            type_param_bounds: Vec::new(),
                            extends: Vec::new(),
                            implements: Vec::new(),
                            implements_args: Vec::new(),
                            open: false,
                            is_abstract: true,
                            sealed: false,
                            resolutions: Vec::new(),
                            uses: Vec::new(),
                            members: t.members.clone(),
                            foreign: false,
                            span: t.span,
                        },
                    );
                }
                Item::Import { .. } => {}
                // Aliases are expanded out of the AST before any backend runs (checker::
                // expand_aliases); this arm only satisfies the exhaustive match.
                Item::TypeAlias { .. } => {}
                // M-Test: `test` items declare no callable symbol; the `phg test` runner executes
                // each test body directly (M-Test T3). Nothing to hoist here.
                Item::Test { .. } => {}
            }
        }
        // M-RT S8: seed a `use`d trait's `static` field as a per-using-class copy (PHP `use` semantics)
        // — keyed `(class, field)`, mirroring the compiler's per-using-class slot.
        for it in &program.items {
            let Item::Class(c) = it else { continue };
            for u in &c.uses {
                for t in &program.items {
                    let Item::Trait(td) = t else { continue };
                    if td.name != u.name {
                        continue;
                    }
                    for m in &td.members {
                        if let crate::ast::ClassMember::Field {
                            modifiers,
                            name,
                            init,
                            ..
                        } = m
                        {
                            if modifiers.contains(&crate::ast::Modifier::Static) {
                                let v = init
                                    .as_ref()
                                    .and_then(crate::value::const_literal)
                                    .unwrap_or(Value::Unit);
                                self.statics.insert((c.name.clone(), name.clone()), v);
                            }
                        }
                    }
                }
            }
        }
        // The single shared runtime subtype oracle (M-RT S6c.3): parent classes AND interfaces, so
        // `instanceof`/match-patterns/overload-subtyping see a class ancestor too. Same algorithm as the
        // VM (the BytecodeProgram builds the identical table), no divergence.
        self.class_implements = crate::ast::instanceof_table(program);
        self.class_tables = crate::native::ClassTables::from_program(program);
        // M-perf S1b: the shared `name → slot` layout per class — same source as the compiler/VM, so
        // both backends allocate slot-aligned instances. Stored as one `Rc` per class, cloned onto
        // every instance of that class in `construct`.
        self.layouts = crate::ast::class_field_layout(program)
            .into_iter()
            .map(|(class, names)| (class, crate::value::ClassLayout::new(names)))
            .collect();
        // The single shared method-dispatch table (M-RT S6b): `call_method` resolves `(class, name)`
        // to its `(declaring_class, method)` — the same table the compiler pre-flattens into the VM's
        // method table, so multi-parent / resolution-clause / renamed dispatch can never diverge. The
        // conflict list is checker-only (E-MI-CONFLICT); a clean program reaches here conflict-free.
        let (origins, _conflicts) = crate::ast::class_method_origins(program);
        self.method_origins = origins;
        // M-RT super/parent: cache the inheritance tables for lexical `parent` resolution (shared with
        // the checker + compiler via `ast::resolve_parent_method`).
        self.parent_parents = crate::ast::class_parents(program);
        self.parent_mro = crate::ast::class_mro(program);
        // Class constants (Feature A): the shared table already flattens inheritance + traits. Drop the
        // declared `Type` — the interpreter inlines only the literal `Value` at each `ClassName.NAME`.
        self.consts = crate::ast::class_consts(program)
            .into_iter()
            .map(|(key, (v, _ty))| (key, v))
            .collect();
        // Expression field initializers (Feature B): the shared ordered list per class.
        for it in &program.items {
            if let Item::Class(c) = it {
                self.field_inits.insert(
                    c.name.clone(),
                    crate::ast::field_initializers(program, &c.name),
                );
            }
        }
    }

    /// Feature B-static: evaluate every **non-literal** static-field initializer once, in declaration
    /// order across classes, BEFORE `main` — overwriting the `Unit` placeholder `collect` seeded.
    /// Evaluated with no `this` (statics are class-level); a later static may read an earlier one
    /// (already stored). Literal statics are already seeded by `collect`, so they are skipped here —
    /// matching the VM, which keeps literals in `static_inits` and emits a `SetStatic` prelude only for
    /// the non-literals. Runs after `collect`, so every function/static is available.
    pub(super) fn eval_static_inits(&mut self, program: &Program) -> R<()> {
        for item in &program.items {
            let Item::Class(c) = item else { continue };
            for m in &c.members {
                if let ClassMember::Field {
                    modifiers,
                    name,
                    init: Some(e),
                    ..
                } = m
                {
                    if modifiers.contains(&Modifier::Static)
                        && !modifiers.contains(&Modifier::Const)
                        && crate::value::const_literal(e).is_none()
                    {
                        let saved_frame = std::mem::replace(&mut self.frame, CallScopes::new());
                        let saved_this = self.this.take();
                        let v = self.eval(e);
                        self.frame = saved_frame;
                        self.this = saved_this;
                        let v = v?;
                        self.statics.insert((c.name.clone(), name.clone()), v);
                    }
                }
            }
        }
        Ok(())
    }

    /// Run a callable body in a fresh frame: bind `args` to `names` in the base
    /// scope, set `this`, execute, restore caller state. A `Return` becomes the
    /// value; falling off the end yields `Unit`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_call(
        &mut self,
        fn_name: &str,
        names: &[String],
        body: &[Stmt],
        args: Vec<Value>,
        this: Option<Value>,
        lexical_class: Option<&str>,
        // `#[UncheckedOverflow]`: does the callee's body wrap int arithmetic? Derived from the callee's attributes
        // by the caller (free functions only; always `false` for methods/constructors). Saved/restored
        // like `cur_class` so nested calls into a checked function re-enable checking.
        unchecked: bool,
    ) -> R<Value> {
        // Mirror the VM's frame cap: past the shared limit, fault cleanly instead of letting
        // native recursion abort the process. Checked before incrementing, so the guard path
        // leaves `depth` untouched and every non-guard exit below decrements exactly once.
        if self.depth >= crate::limits::MAX_CALL_DEPTH {
            return rt("stack overflow");
        }
        self.depth += 1;
        // Push a trace frame (line is filled in by `exec_stmt` as the body runs). Popped only on the
        // success arms below — an error leaves it on `trace_stack` for the top-level snapshot.
        self.trace_stack.push(crate::diagnostic::Frame {
            function: fn_name.to_string(),
            file: None,
            line: 0,
            col: 0,
        });
        let saved_frame = std::mem::replace(&mut self.frame, CallScopes::new());
        let saved_this = std::mem::replace(&mut self.this, this);
        // M-RT super/parent: the running body's lexical class (the declaring class, for `parent`
        // resolution). `None` for a free function. Saved/restored exactly like `this`.
        let saved_class = std::mem::replace(&mut self.cur_class, lexical_class.map(str::to_string));
        let saved_unchecked = std::mem::replace(&mut self.cur_unchecked, unchecked);
        for (n, a) in names.iter().zip(args) {
            self.frame.declare(n, a);
        }
        let mut result = self.exec_stmts(body);
        // M-DX S3: this is the innermost `run_call` to observe a surfacing fault, so `self.frame`
        // still holds the *faulting* frame's live locals (they are discarded on the next line). If a
        // dump is enabled and not already captured (the `is_none` guard makes the deepest frame win as
        // the fault unwinds outward), snapshot the locals into the diagnostic. Side-channel only.
        //
        // The actual capture lives in a `#[cold] #[inline(never)]` helper: `run_call` is the hot
        // recursive frame, and inlining the snapshot/render temporaries (a `BTreeMap`, string builders,
        // the recursive value renderer) here would reserve their stack in *every* frame — enough,
        // times `MAX_CALL_DEPTH`, to overflow the 256 MB worker before the depth guard fires.
        if let Err(Signal::Runtime(diag)) = &mut result {
            if diag.dump.is_none() && crate::dump::should_dump() {
                self.capture_fault_dump(diag);
            }
        }
        self.frame = saved_frame;
        self.this = saved_this;
        self.cur_class = saved_class;
        self.cur_unchecked = saved_unchecked;
        self.depth -= 1;
        match result {
            Ok(()) => {
                self.trace_stack.pop();
                Ok(Value::Unit)
            }
            Err(Signal::Return(v)) => {
                self.trace_stack.pop();
                Ok(v)
            }
            Err(other) => Err(other),
        }
    }

    /// Snapshot the live trace stack as ordered frames (innermost → outermost) for a fault diagnostic.
    pub(super) fn snapshot_frames(&self) -> Vec<crate::diagnostic::Frame> {
        self.trace_stack.iter().rev().cloned().collect()
    }

    /// Capture the faulting frame's locals into `diag`'s value-dump (M-DX S3). Kept `#[cold]` +
    /// `#[inline(never)]` so its stack-heavy temporaries never bloat the hot recursive `run_call`
    /// frame (see the call site). Reached at most once per fault (guarded by `dump.is_none()`).
    #[cold]
    #[inline(never)]
    pub(super) fn capture_fault_dump(&self, diag: &mut Diagnostic) {
        let locals = self.frame.snapshot_locals();
        diag.dump = Some(Box::new(crate::dump::format_locals(&locals)));
    }

    /// Run one interactive-debugger pause (M-DX S5). `#[cold]` + `#[inline(never)]` so the stack-heavy
    /// snapshot/frontend machinery never bloats the hot recursive `exec_stmt` frame (same discipline as
    /// [`Self::capture_fault_dump`]). A `quit` from the frontend detaches the session (the program then
    /// runs to completion).
    #[cold]
    #[inline(never)]
    pub(super) fn debug_pause(&mut self, line: u32) {
        let locals = self.frame.snapshot_locals();
        let frames = self.snapshot_frames();
        let depth = self.depth;
        let quit = if let Some(session) = &mut self.debug {
            session.pause(line, depth, locals, frames)
        } else {
            false
        };
        if quit {
            self.debug = None;
        }
    }

    pub(super) fn exec_stmts(&mut self, stmts: &[Stmt]) -> R<()> {
        for s in stmts {
            self.exec_stmt(s)?;
        }
        Ok(())
    }

    pub(super) fn exec_scoped(&mut self, stmts: &[Stmt]) -> R<()> {
        self.frame.push_scope();
        let r = self.exec_stmts(stmts);
        self.frame.pop_scope();
        r
    }
}
