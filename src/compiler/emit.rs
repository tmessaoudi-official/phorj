//! Compiler plumbing — per-function state, op emission, jumps, scopes, locals.

use super::*;

impl<'a> Compiler<'a> {
    /// A fresh compiler for one function body, sharing the program-level tables (function/variant/
    /// class indices, descriptor tables, name pool). Locals/chunk/height start empty; the caller
    /// seeds params and (for constructors) toggles `ctor_return_jumps`.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::compiler) fn new(
        fns: &'a HashMap<String, FnMeta>,
        arities: &'a [usize],
        variants: &'a VariantIndex,
        enum_descs: &'a [EnumDesc],
        classes: &'a HashMap<String, usize>,
        imports: &'a HashMap<String, String>,
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
        base_fn_idx: usize,
    ) -> Self {
        Compiler {
            chunk: Chunk::new(),
            locals: Vec::new(),
            scope_depth: 0,
            fns,
            arities,
            extra_functions: Vec::new(),
            base_fn_idx,
            lambda_n_captures: Vec::new(),
            variants,
            enum_descs,
            classes,
            imports,
            statics_index,
            consts_index,
            class_descs,
            names_index,
            this_slot: None,
            field_tags,
            class_field_ctys,
            method_rets,
            method_generic_ret_from_param,
            reified_operands,
            methods,
            method_overloads,
            cur_class: None,
            parent_parents: None,
            match_bindings: Vec::new(),
            height: 0,
            ctor_return_jumps: None,
            loop_frames: Vec::new(),
            finally_stack: Vec::new(),
        }
    }

    pub(in crate::compiler) fn emit(&mut self, op: Op, line: u32) {
        // Maintain the operand-stack height (saturating: control flow after a `Return`/`MatchFail`
        // is dead code whose height is never read). Branch merges reset `height` explicitly.
        let eff = self.stack_effect(&op);
        self.height = self.height.saturating_add_signed(eff);
        self.chunk.emit(op, line);
    }

    /// Net operand-stack delta of one op (`pushes - pops`). Only consumed by `match` (to spill its
    /// scrutinee to the right slot); kept exhaustive so a new op can't silently skew the height.
    pub(in crate::compiler) fn stack_effect(&self, op: &Op) -> isize {
        match op {
            Op::Const(_) | Op::GetLocal(_) => 1,
            Op::AddI | Op::SubI | Op::MulI | Op::DivI | Op::RemI => -1,
            Op::AddF | Op::SubF | Op::MulF | Op::DivF | Op::RemF => -1,
            // Decimal `+ - *` pop two, push one (M-NUM S1); exact `%` (`RemD`) + exact-or-fault `/`
            // (`DivD`) too (2026-06-27).
            Op::AddD | Op::SubD | Op::MulD | Op::RemD | Op::DivD => -1,
            // Bitwise binaries pop two, push one (primitives P2).
            Op::BitAnd | Op::BitOr | Op::BitXor | Op::Shl | Op::Shr => -1,
            Op::Eq | Op::Ne | Op::Lt | Op::Gt | Op::Le | Op::Ge => -1,
            Op::Pop | Op::SetLocal(_) | Op::JumpIfFalse(_) | Op::Index | Op::MakeRange(_) => -1,
            // SetIndex pops (container, index, value) and pushes the new container: net -2.
            Op::SetIndex => -2,
            // SetIndexLocal pops (index, value) and mutates the local slot in place — pushes nothing.
            Op::SetIndexLocal(_) => -2,
            // SetPathLocal pops `depth` indices + the value; mutates the local slot in place — no push.
            Op::SetPathLocal(_, depth) => -(*depth as isize + 1),
            // BitNot is unary (pop one, push one) like Neg/Not.
            Op::Neg | Op::Not | Op::BitNot | Op::Len | Op::IterElems | Op::Jump(_) => 0,
            Op::MatchTag(_) | Op::MatchTagName(_) | Op::GetEnumField(_) => 0, // pop one, push one
            // DEC-302: `EnumValue` pops the enum, pushes its backing; `EnumFrom` pops the arg,
            // pushes the matched variant (or null). Both net 0.
            Op::EnumValue | Op::EnumFrom(..) => 0,
            Op::Concat(n) | Op::MakeList(n) => 1 - *n as isize,
            Op::MakeMap(n) => 1 - 2 * *n as isize, // pops 2n (key+value pairs), pushes the map
            // Pops `argc` args, pushes the native's return value (the old `Print` + `Const(Unit)`
            // pair collapses into one op, net delta unchanged).
            Op::CallNative(_, argc) => 1 - *argc as isize,
            Op::Call(idx) => 1 - self.arities[*idx] as isize,
            // Pops `argc` args, dispatches to one overload, pushes its single return value.
            Op::CallOverload(_, argc) => 1 - *argc as isize,
            // Statics-B: like `CallOverload` but the compiler pushed a dummy receiver below the args,
            // and the selected static body's arity pops it too — so this pops `argc + 1`, pushes 1.
            Op::CallStaticOverload(_, argc) => -(*argc as isize),
            Op::MakeEnum(idx) => 1 - self.enum_descs[*idx].arity as isize,
            Op::MakeInstance(idx) => 1 - self.class_descs[*idx].fields.len() as isize,
            Op::GetField(_) => 0,   // pop instance, push field value
            Op::SetField(_) => -2,  // pop instance + value, push nothing (statement)
            Op::GetStatic(_) => 1,  // push the static's value
            Op::SetStatic(_) => -1, // pop the value into the static slot
            Op::IsInstance(_) => 0, // pop value, push bool
            // Pops the receiver + `argc` args, pushes one result.
            Op::CallMethod(_, argc) => -(*argc as isize),
            // `parent`/super dispatch (M-RT): pops `this` + `argc` args, pushes the result → net -argc.
            Op::CallParent(_, argc) => -(*argc as isize),
            // Terminal (end/redirect the frame): height afterward is dead code, never read.
            Op::Return | Op::Fault(_) => 0,
            // MakeClosure(idx): pops `n_captures` capture values, pushes one `Value::Closure`.
            // Lambdas compiled by THIS compiler occupy [base, base+lambda_n_captures.len()) in the
            // trailing lambda block. Any other index is a named-function reference (never a closure
            // → 0 captures), including a forward-referenced one (its index is below `base`).
            Op::MakeClosure(idx) => {
                let lo = self.base_fn_idx;
                let n = if *idx >= lo && *idx < lo + self.lambda_n_captures.len() {
                    self.lambda_n_captures[idx - lo]
                } else {
                    0 // named function ref — no captures
                };
                1 - n as isize
            }
            // CallValue(argc): pops `argc` args + 1 closure, pushes 1 result → 1 - argc.
            Op::CallValue(argc) => 1 - *argc as isize,
            // M-faults 2b: `Throw` pops the exception value; the handler ops are pure bookkeeping.
            // The catch landing pad's pushed value (+1) is modeled by setting `self.height` directly
            // at the landing pad (like a `match` scrutinee), not via a stack effect here.
            Op::Throw => -1,
            Op::PushHandler(_) | Op::PopHandler => 0,
            // Green-thread ops (M6 W4). `Spawn` pops the call's result, pushes a `Task` (0).
            // `ChannelNew` pushes a fresh channel (+1). `ChannelSend` pops the value + channel and
            // pushes the void `Unit` (net -1). `ChannelRecv` pops the channel, pushes the value (0).
            // `Join` pops the task, pushes its result (0).
            Op::Spawn | Op::ChannelRecv | Op::Join => 0,
            Op::ChannelNew => 1,
            Op::ChannelSend => -1,
            // `SpawnCall(_, argc)` pops the `argc` args (the call did not run before it) and pushes one
            // `Task` — net `1 - argc`.
            Op::SpawnCall(_, argc) => 1 - *argc as isize,
        }
    }

    pub(in crate::compiler) fn emit_const(&mut self, v: Value, line: u32) {
        let k = self.chunk.add_const(v);
        self.emit(Op::Const(k), line);
    }

    pub(in crate::compiler) fn here(&self) -> usize {
        self.chunk.code.len()
    }

    /// Emit a jump placeholder (target 0); returns its code index for `patch_jump`.
    pub(in crate::compiler) fn emit_jump(&mut self, op: Op, line: u32) -> usize {
        let idx = self.here();
        self.emit(op, line);
        idx
    }

    /// Patch a previously-emitted forward jump to point at the current code position.
    pub(in crate::compiler) fn patch_jump(&mut self, idx: usize) {
        let target = self.here();
        self.patch_jump_to(idx, target);
    }

    /// Patch a previously-emitted jump to an explicit absolute target — used for `continue`
    /// back-edges (a known earlier position) where the target is not `here()` (M-mut.3).
    pub(in crate::compiler) fn patch_jump_to(&mut self, idx: usize, target: usize) {
        self.chunk.code[idx] = match self.chunk.code[idx] {
            Op::Jump(_) => Op::Jump(target),
            Op::JumpIfFalse(_) => Op::JumpIfFalse(target),
            // A `try`'s handler target is patched to its catch landing pad (M-faults 2b).
            Op::PushHandler(_) => Op::PushHandler(target),
            ref other => unreachable!("patch_jump on {other:?}"),
        };
    }

    pub(in crate::compiler) fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    pub(in crate::compiler) fn end_scope(&mut self, line: u32) {
        self.scope_depth -= 1;
        while matches!(self.locals.last(), Some(l) if l.depth > self.scope_depth) {
            self.emit(Op::Pop, line);
            self.locals.pop();
        }
    }

    pub(in crate::compiler) fn add_local(&mut self, name: &str, ty: CTy) -> usize {
        self.locals.push(Local {
            name: name.to_string(),
            ty,
            depth: self.scope_depth,
        });
        self.locals.len() - 1
    }

    pub(in crate::compiler) fn resolve_local(&self, name: &str) -> Option<usize> {
        self.locals.iter().rposition(|l| l.name == name)
    }
}
