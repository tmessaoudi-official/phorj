//! UNBOXED higher-order List verticals — the `hofpipe` inline loops (`List.map`/`List.count`
//! with a STATIC lambda). Split out of `verticals.rs` (M-Decomp, Inv 13) so the fold family
//! (`sumBy`/reduce) has room to grow its accumulator modes here without regrowing the
//! grandfathered sibling. Shared emit state arrives via [`Ec`]; helper deps
//! (`ub_pop`/`ub_push`/`list_append_acc`/`emit_call_to`/`emit_release`) resolve through
//! `use super::*` exactly as in `verticals.rs`.

use super::*;

/// Which List higher-order op the shared `arm_list_hof` loop is emitting. All four walk the
/// input identically (one direct call per element); they differ only in the accumulator:
/// `Map` seeds/extends an ACL int-list builder; `Count` sums the raw 0/1 predicate result with a
/// WRAPPING `iadd` (bounded by list length — cannot overflow); `Sum` sums the int projection with
/// a CHECKED `sadd_overflow` (an overflow → code-5 VM redo → `list_sum_by`'s exact fault);
/// `Filter` appends the ORIGINAL element to the ACL builder iff the 0/1 predicate result is
/// nonzero (the listfilter flip — a conditional `list_append_acc`, survivor order = input order).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum ListHof {
    Map,
    Count,
    Sum,
    Filter,
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
    let builds_list = matches!(hof, ListHof::Map | ListHof::Filter);
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
    let (addr0, count, stride) = ub_list_walk_setup(b, ec, lv);
    // Output seed: map/filter → a fresh ACL builder; count/sum → a zero register.
    let acc0 = if builds_list {
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
        ListHof::Filter => {
            // Conditional append of the ORIGINAL element: a 0/1 predicate word branches around
            // the builder push; both edges merge with the (possibly extended) builder handle.
            // `list_append_acc` returns the SAME record handle it received (in-place push), so
            // threading its result through the merge is exact.
            let keep_blk = b.create_block();
            let joinb = b.create_block();
            b.append_block_param(joinb, types::I64);
            b.ins().brif(rv, keep_blk, &[], joinb, &[acc.into()]);
            b.switch_to_block(keep_blk);
            let acc_kept = list_append_acc(b, ec, h, acc, elem)?;
            b.ins().jump(joinb, &[acc_kept.into()]);
            b.switch_to_block(joinb);
            b.block_params(joinb)[0]
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
        if builds_list {
            Kind::IntList(Own::Owned)
        } else {
            Kind::Int
        },
    )
}

/// Resolve an `IntList` handle `lv` to a uniform `(addr0, count, stride)` walk and leave the builder
/// positioned in the merged `setup` block: a FLAT handle → `(buf + base<<6, count-bits, 64)`; an ACL
/// builder record → `(ptr, len>>3, 8)`; a boxed list → code 5 (VM redo, the disclosed v1 gap). Shared
/// by every hofpipe arm (`arm_list_hof`'s map/count/sum + `arm_list_reduce`).
fn ub_list_walk_setup(
    b: &mut FunctionBuilder,
    ec: &Ec,
    lv: ClValue,
) -> (ClValue, ClValue, ClValue) {
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
    (
        b.block_params(setup)[0],
        b.block_params(setup)[1],
        b.block_params(setup)[2],
    )
}

/// The ??-FUSED `List.maxBy`/`minBy` fold (the maxby/minby flips): the same inline
/// `(addr, stride)` walk, one direct selector call per element, and a FIRST-WINS strict
/// extreme fold — the running best is replaced only on a STRICTLY better key (`sgt` for max,
/// `slt` for min; parity-affecting tie-break, see `list_extreme_by`'s kernel doc) with the
/// first element forced in via `rem == count`. The nullable `T?` result is made TOTAL by the
/// fused `?? <int>` window: an empty list yields `default` (exactly what `null ?? default`
/// evaluates to), so the pushed kind is a plain `Int` — no optional Kind needed. A boxed list
/// → code 5 (VM redo).
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_list_extreme_by(
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
    is_max: bool,
    default: i64,
) -> Result<(), JitError> {
    let (fv, fk) = ub_pop(b, vars, fvars, kinds)?;
    let (f, has_cap) = match fk {
        Kind::Fn(f) => (f, false),
        Kind::FnCap1(f) => (f, true),
        other => {
            return Err(JitError::Unsupported(format!(
                "unboxed maxBy/minBy callee kind {other:?}"
            )))
        }
    };
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(lk, Kind::IntList(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed maxBy/minBy receiver kind {lk:?}"
        )));
    }
    let (addr0, count, stride) = ub_list_walk_setup(b, ec, lv);
    let header = b.create_block();
    b.append_block_param(header, types::I64); // addr
    b.append_block_param(header, types::I64); // remaining
    b.append_block_param(header, types::I64); // best key
    b.append_block_param(header, types::I64); // best element
    let bodyb = b.create_block();
    let exitb = b.create_block();
    b.append_block_param(exitb, types::I64);
    let zero = b.ins().iconst(types::I64, 0);
    b.ins().jump(
        header,
        &[addr0.into(), count.into(), zero.into(), zero.into()],
    );
    b.switch_to_block(header);
    let addr = b.block_params(header)[0];
    let rem = b.block_params(header)[1];
    let bkey = b.block_params(header)[2];
    let belem = b.block_params(header)[3];
    b.ins().brif(rem, bodyb, &[], exitb, &[belem.into()]);
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
    let (key, _kk) = ub_pop(b, vars, fvars, kinds)?;
    // Replace ONLY on a strictly better key; the FIRST element is forced in (its `bkey` seed
    // is dead) via `rem == count` — the selector still ran for it, exactly once.
    let cc = if is_max {
        IntCC::SignedGreaterThan
    } else {
        IntCC::SignedLessThan
    };
    let better = b.ins().icmp(cc, key, bkey);
    let first = b.ins().icmp(IntCC::Equal, rem, count);
    let take = b.ins().bor(better, first);
    let nkey = b.ins().select(take, key, bkey);
    let nelem = b.ins().select(take, elem, belem);
    let addr1 = b.ins().iadd(addr, stride);
    let rem1 = b.ins().iadd_imm_s(rem, -1);
    b.ins().jump(
        header,
        &[addr1.into(), rem1.into(), nkey.into(), nelem.into()],
    );
    b.switch_to_block(exitb);
    let best = b.block_params(exitb)[0];
    // Empty list → the fused `??` default (the `null ?? default` byte-identical result).
    let dflt = b.ins().iconst(types::I64, default);
    let nonempty = b.ins().icmp_imm_s(IntCC::NotEqual, count, 0);
    let res = b.ins().select(nonempty, best, dflt);
    if lk.is_owned_handle() {
        emit_release(b, ec, h, lv);
    }
    ub_push(b, vars, fvars, kinds, res, Kind::Int)
}

/// `List.reduce(xs, seed, f)` with a STATIC 2-arg lambda (the fold vertical): the same inline
/// `(addr, stride)` walk as `arm_list_hof`, but the accumulator is SEEDED from the `seed` operand and
/// each step calls `f(acc, elem)` — the running `acc` is PREPENDED to the element (an `FnCap1` capture
/// word goes first: `[cap, acc, elem]`). No fold-level overflow guard: any arithmetic lives inside the
/// user lambda, compiled with its own checked ops. Result kind = the seed's kind `U` (Int in this v1
/// subset — analyze fails closed otherwise). A boxed list → code 5 (VM redo).
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_list_reduce(
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
) -> Result<(), JitError> {
    // Push order (xs, seed, f) → pop f, then seed, then xs.
    let (fv, fk) = ub_pop(b, vars, fvars, kinds)?;
    let (f, has_cap) = match fk {
        Kind::Fn(f) => (f, false),
        Kind::FnCap1(f) => (f, true),
        other => {
            return Err(JitError::Unsupported(format!(
                "unboxed List.reduce callee kind {other:?}"
            )))
        }
    };
    let (seed, _sk) = ub_pop(b, vars, fvars, kinds)?; // U = Int (analyze-guaranteed)
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(lk, Kind::IntList(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed List.reduce receiver kind {lk:?}"
        )));
    }
    let (addr0, count, stride) = ub_list_walk_setup(b, ec, lv);
    // The loop: `acc` seeded from `seed`; a 0-element list returns the seed unchanged.
    let header = b.create_block();
    b.append_block_param(header, types::I64); // addr
    b.append_block_param(header, types::I64); // remaining
    b.append_block_param(header, types::I64); // acc
    let bodyb = b.create_block();
    let exitb = b.create_block();
    b.append_block_param(exitb, types::I64);
    b.ins()
        .jump(header, &[addr0.into(), count.into(), seed.into()]);
    b.switch_to_block(header);
    let addr = b.block_params(header)[0];
    let rem = b.block_params(header)[1];
    let acc = b.block_params(header)[2];
    b.ins().brif(rem, bodyb, &[], exitb, &[acc.into()]);
    b.switch_to_block(bodyb);
    let elem = b.ins().load(types::I64, MemFlagsData::new(), addr, 0);
    let cargs = if has_cap {
        vec![fv, acc, elem]
    } else {
        vec![acc, elem]
    };
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
    // New acc = the callback result (no fold-level arithmetic → no overflow guard here).
    let (rv, _rk) = ub_pop(b, vars, fvars, kinds)?;
    let addr1 = b.ins().iadd(addr, stride);
    let rem1 = b.ins().iadd_imm_s(rem, -1);
    b.ins()
        .jump(header, &[addr1.into(), rem1.into(), rv.into()]);
    b.switch_to_block(exitb);
    let res = b.block_params(exitb)[0];
    if lk.is_owned_handle() {
        emit_release(b, ec, h, lv);
    }
    ub_push(b, vars, fvars, kinds, res, Kind::Int)
}
