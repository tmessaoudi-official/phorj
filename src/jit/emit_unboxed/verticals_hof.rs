//! UNBOXED higher-order List verticals — the `hofpipe` inline loops (`List.map`/`List.count`
//! with a STATIC lambda). Split out of `verticals.rs` (M-Decomp, Inv 13) so the fold family
//! (`sumBy`/reduce) has room to grow its accumulator modes here without regrowing the
//! grandfathered sibling. Shared emit state arrives via [`Ec`]; helper deps
//! (`ub_pop`/`ub_push`/`list_append_acc`/`emit_call_to`/`emit_release`) resolve through
//! `use super::*` exactly as in `verticals.rs`.

use super::*;

/// Which List higher-order op the shared `arm_list_hof` loop is emitting. All three walk the
/// input identically (one direct call per element); they differ only in the accumulator:
/// `Map` seeds/extends an ACL int-list builder; `Count` sums the raw 0/1 predicate result with a
/// WRAPPING `iadd` (bounded by list length — cannot overflow); `Sum` sums the int projection with
/// a CHECKED `sadd_overflow` (an overflow → code-5 VM redo → `list_sum_by`'s exact fault).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum ListHof {
    Map,
    Count,
    Sum,
}

/// `List.map` / `List.count` / `List.sumBy` with a STATIC lambda (the hofpipe vertical): ONE loop —
/// a uniform `(addr, stride)` walk over the input (flat list: 64-byte slots, raw i64 at
/// bytes 0..8; ACL builder: packed 8-byte i64s; a boxed list → code 5, redo on VM — the
/// disclosed v1 gap), a DIRECT call per element (`Fn`, or `FnCap1` with the capture word
/// PREPENDED — the VM's `[caps.., args..]` lambda frame), and: map → an ACL builder output
/// (inline cap-checked pushes via `list_append_acc`); count → a register sum of the 0/1
/// predicate results. No closure object, no VM re-entry, no per-element allocation.
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_list_hof(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    fn_refs: &[Option<FuncRef>],
    ctx: ClValue,
    depth: ClValue,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    info: &UbGraphInfo,
    hof: ListHof,
) -> Result<(), JitError> {
    let is_map = hof == ListHof::Map;
    let (fv, fk) = ub_pop(b, vars, fvars, kinds)?;
    let (f, has_cap) = match fk {
        Kind::Fn(f) => (f, false),
        Kind::FnCap1(f) => (f, true),
        other => {
            return Err(JitError::Unsupported(format!(
                "unboxed List HOF callee kind {other:?}"
            )))
        }
    };
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(lk, Kind::IntList(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed List HOF receiver kind {lk:?}"
        )));
    }
    // Representation dispatch → uniform (addr0, count, stride).
    let setup = b.create_block();
    b.append_block_param(setup, types::I64); // addr0
    b.append_block_param(setup, types::I64); // count
    b.append_block_param(setup, types::I64); // stride
    let flat_blk = b.create_block();
    let chk_acl = b.create_block();
    let acl_blk = b.create_block();
    let bad_blk = b.create_block();
    let flat_bit = b.ins().band_imm_s(lv, UB_TAG_FLAT);
    b.ins().brif(flat_bit, flat_blk, &[], chk_acl, &[]);
    b.switch_to_block(flat_blk);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let base = b.ins().band_imm_s(lv, UB_IDX_MASK);
    let boff = b.ins().ishl_imm_s(base, 6);
    let faddr = b.ins().iadd(buf, boff);
    let fcnt_raw = b.ins().ushr_imm_s(lv, 40);
    let fcnt = b.ins().band_imm_s(fcnt_raw, 0xFFFFF);
    let s64 = b.ins().iconst(types::I64, 64);
    b.ins()
        .jump(setup, &[faddr.into(), fcnt.into(), s64.into()]);
    b.switch_to_block(chk_acl);
    let acl_bit = b.ins().band_imm_s(lv, UB_TAG_ACL);
    b.ins().brif(acl_bit, acl_blk, &[], bad_blk, &[]);
    b.switch_to_block(acl_blk);
    let abase = b.ins().load(types::I64, ec.stable, ec.ctx, 40);
    let ridx = b.ins().band_imm_s(lv, UB_IDX_MASK);
    let roff = b.ins().imul_imm_s(ridx, 24);
    let prec = b.ins().iadd(abase, roff);
    let aptr = b.ins().load(types::I64, MemFlagsData::new(), prec, 0);
    let alenb = b.ins().load(types::I64, MemFlagsData::new(), prec, 8);
    let acnt = b.ins().ushr_imm_s(alenb, 3);
    let s8 = b.ins().iconst(types::I64, 8);
    b.ins().jump(setup, &[aptr.into(), acnt.into(), s8.into()]);
    // Boxed int list: code 5 — the VM redo runs the canonical higher-order native.
    b.switch_to_block(bad_blk);
    let always = b.ins().iconst(types::I64, 1);
    ec.fault_if(b, always, 5);
    let z = b.ins().iconst(types::I64, 0);
    b.ins().jump(setup, &[z.into(), z.into(), z.into()]); // unreachable terminator
    b.switch_to_block(setup);
    let addr0 = b.block_params(setup)[0];
    let count = b.block_params(setup)[1];
    let stride = b.block_params(setup)[2];
    // Output seed: map → a fresh ACL builder; count → a zero register.
    let acc0 = if is_map {
        let call = b.ins().call(h.list_builder_new, &[ec.ctx]);
        let oh = b.inst_results(call)[0];
        let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, oh, 0);
        ec.fault_if(b, bad, 5);
        oh
    } else {
        b.ins().iconst(types::I64, 0)
    };
    // The loop: header checks the remaining count (a 0-element list skips straight out).
    let header = b.create_block();
    b.append_block_param(header, types::I64); // addr
    b.append_block_param(header, types::I64); // remaining
    b.append_block_param(header, types::I64); // acc (out handle / running sum)
    let bodyb = b.create_block();
    let exitb = b.create_block();
    b.append_block_param(exitb, types::I64);
    b.ins()
        .jump(header, &[addr0.into(), count.into(), acc0.into()]);
    b.switch_to_block(header);
    let addr = b.block_params(header)[0];
    let rem = b.block_params(header)[1];
    let acc = b.block_params(header)[2];
    b.ins().brif(rem, bodyb, &[], exitb, &[acc.into()]);
    b.switch_to_block(bodyb);
    let elem = b.ins().load(types::I64, MemFlagsData::new(), addr, 0);
    let cargs = if has_cap { vec![fv, elem] } else { vec![elem] };
    emit_call_to(
        b,
        ec,
        fn_refs,
        ctx,
        depth,
        vars,
        fvars,
        kinds,
        f,
        cargs,
        info.ret_of(f),
        None,
    )?;
    let (rv, _rk) = ub_pop(b, vars, fvars, kinds)?;
    let acc1 = match hof {
        ListHof::Map => list_append_acc(b, ec, h, acc, rv)?,
        // Bool results are 0/1 i64 words — the wrapping sum IS the count (bounded, cannot overflow).
        ListHof::Count => b.ins().iadd(acc, rv),
        ListHof::Sum => {
            // CHECKED sum of the int projection: an overflow carry → code-5 VM redo, which re-runs
            // `list_sum_by` and renders its exact `"integer overflow in List.sumBy"` fault. The
            // no-overflow case (every shipped example / bench) stays a single `sadd_overflow`.
            let (sum, ovf) = b.ins().sadd_overflow(acc, rv);
            ec.fault_if(b, ovf, 5);
            sum
        }
    };
    let addr1 = b.ins().iadd(addr, stride);
    let rem1 = b.ins().iadd_imm_s(rem, -1);
    b.ins()
        .jump(header, &[addr1.into(), rem1.into(), acc1.into()]);
    b.switch_to_block(exitb);
    let res = b.block_params(exitb)[0];
    // A consumed OWNED input dies here (an ACL from a preceding map recycles its record).
    if lk.is_owned_handle() {
        emit_release(b, ec, h, lv);
    }
    ub_push(
        b,
        vars,
        fvars,
        kinds,
        res,
        if is_map {
            Kind::IntList(Own::Owned)
        } else {
            Kind::Int
        },
    )
}
