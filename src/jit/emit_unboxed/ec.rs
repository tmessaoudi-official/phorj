//! The shared emit-context [`Ec`] (M-Decomp from `emit_unboxed/mod.rs`, Invariant 13): the
//! Copy per-function state every op arm receives (ctx pointer, stable-header memflags, shared
//! fault-exit, speculation sticky) plus its fault/sticky/slot emission methods. Bodies moved
//! verbatim; arms keep reaching it via `use super::*`.

use super::*;

/// Copy-able per-function emit context shared by every op arm: the `UbCtx` pointer, the
/// stable-header memflags, and the (optional) shared fault-exit / speculation-sticky handles.
/// Replaces the closures the pre-decomposition monolith captured, so arms can live in sibling
/// files. All fields are Copy; methods take the builder explicitly.
#[derive(Clone, Copy)]
pub(super) struct Ec {
    /// The per-run [`UbCtx`] pointer (entry param 0; null for a pure-numeric graph).
    pub(super) ctx: ClValue,
    /// `notrap + can_move` flags for loads of RUN-INVARIANT `UbCtx` header fields (arena base @0,
    /// free-stack @8, capacity @32 — never stored; mutable `free_top` @16 / `bump` @24 keep defaults).
    pub(super) stable: MemFlagsData,
    /// The shared fault-exit block (`Some` iff `needs_fault_exit`).
    pub(super) fault_exit: Option<Block>,
    /// The speculation sticky Variable (`Some` iff `needs_sticky`).
    pub(super) sticky: Option<Variable>,
}

impl Ec {
    /// Emit "if `flag` (i8, nonzero) then fault with `code` else continue in a fresh block".
    /// Only ever called on a path that needs the fault-exit (div/rem/call/depth or a sticky
    /// redo), so `fault_exit` is guaranteed `Some` here (`needs_fault_exit`).
    pub(super) fn fault_if(&self, b: &mut FunctionBuilder, flag: ClValue, code: i64) {
        let fx = self
            .fault_exit
            .expect("fault_if requires a fault-exit block (needs_fault_exit)");
        let cv = b.ins().iconst(types::I64, code);
        let cont = b.create_block();
        b.ins().brif(flag, fx, &[cv.into()], cont, &[]);
        b.switch_to_block(cont);
    }

    /// ovf-spec: OR a boolean overflow `flag` (i8, 0/1 from `*_overflow` / an `is_min` compare)
    /// into the sticky Variable — no branch, so the hot no-overflow path costs only the OR.
    /// Zero-extends to i64. Only called for an UNPROVEN speculated op, so `sticky` is `Some` (`needs_sticky`).
    pub(super) fn accumulate_sticky(&self, b: &mut FunctionBuilder, flag: ClValue) {
        let sv = self
            .sticky
            .expect("accumulate_sticky requires the sticky var (needs_sticky)");
        let cur = b.use_var(sv);
        let ext = b.ins().uextend(types::I64, flag);
        let next = b.ins().bor(cur, ext);
        b.def_var(sv, next);
    }

    /// P-2a-inline: push an owned arena slot's index onto the inline free stack (caller has already
    /// established `v` is slot-tagged with OWNED set). 5 memory ops, no call.
    pub(super) fn slot_push(&self, b: &mut FunctionBuilder, v: ClValue) {
        let fsp = b.ins().load(types::I64, self.stable, self.ctx, 8);
        let ft = b.ins().load(types::I64, MemFlagsData::new(), self.ctx, 16);
        let slot = b.ins().band_imm_s(v, UB_IDX_MASK);
        let foff = b.ins().ishl_imm_s(ft, 2);
        let faddr = b.ins().iadd(fsp, foff);
        b.ins().istore32(MemFlagsData::new(), slot, faddr, 0);
        let ft1 = b.ins().iadd_imm_s(ft, 1);
        b.ins().store(MemFlagsData::new(), ft1, self.ctx, 16);
    }

    /// P-2a-inline: recycle a slot-tagged operand IFF its runtime OWNED bit is set (a flat-list element
    /// / pinned const is compile-time Owned but runtime-borrowed → no-op free). Only at known-slot sites.
    pub(super) fn slot_free_if_owned(&self, b: &mut FunctionBuilder, v: ClValue) {
        let owned_bit = b.ins().band_imm_s(v, UB_TAG_OWNED);
        let push_blk = b.create_block();
        let cont = b.create_block();
        b.ins().brif(owned_bit, push_blk, &[], cont, &[]);
        b.switch_to_block(push_blk);
        self.slot_push(b, v);
        b.ins().jump(cont, &[]);
        b.switch_to_block(cont);
    }

    /// Allocate a fresh arena SLOT inline (the P-2a-inline concat ladder, shared by `MakeInstance`):
    /// pop the inline free stack if non-empty, else bump — a full arena is code 5 (redo on VM;
    /// exhaustion is a fallback, never a user-visible fault). Returns the slot INDEX (untagged).
    pub(super) fn slot_alloc(&self, b: &mut FunctionBuilder) -> ClValue {
        let alloc_done = b.create_block();
        b.append_block_param(alloc_done, types::I64);
        let pop_blk = b.create_block();
        let bump_blk = b.create_block();
        let ft = b.ins().load(types::I64, MemFlagsData::new(), self.ctx, 16);
        b.ins().brif(ft, pop_blk, &[], bump_blk, &[]);
        b.switch_to_block(pop_blk);
        let ft1 = b.ins().iadd_imm_s(ft, -1);
        b.ins().store(MemFlagsData::new(), ft1, self.ctx, 16);
        let fsp = b.ins().load(types::I64, self.stable, self.ctx, 8);
        let foff = b.ins().ishl_imm_s(ft1, 2);
        let faddr = b.ins().iadd(fsp, foff);
        let popped = b.ins().uload32(MemFlagsData::new(), faddr, 0);
        b.ins().jump(alloc_done, &[popped.into()]);
        b.switch_to_block(bump_blk);
        let bp = b.ins().load(types::I64, MemFlagsData::new(), self.ctx, 24);
        let cap = b.ins().load(types::I64, self.stable, self.ctx, 32);
        let full = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, bp, cap);
        self.fault_if(b, full, 5);
        let bp1 = b.ins().iadd_imm_s(bp, 1);
        b.ins().store(MemFlagsData::new(), bp1, self.ctx, 24);
        b.ins().jump(alloc_done, &[bp.into()]);
        b.switch_to_block(alloc_done);
        b.block_params(alloc_done)[0]
    }
}
