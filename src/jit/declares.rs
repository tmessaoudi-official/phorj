//! The handle-op helper DECLARES (M-Decomp from `compile.rs`, Invariant 13): one `FuncId` per
//! `rt_u_*` runtime helper, declared into the fresh `JITModule` when the graph uses handles.
//! Keep the field/name/signature triple in lockstep with `handles/symbols.rs` (the pointer
//! registrations) and `emit_unboxed/refs.rs` (the per-function imports).

use super::*;

/// Declare every unboxed runtime helper import (`Linkage::Import` against the symbols the
/// `JITBuilder` registered) and return the id table. Bodies moved verbatim from `compile.rs`.
pub(super) fn declare_ub_helper_ids(
    module: &mut JITModule,
    ptr: Type,
) -> Result<UbHelperIds, JitError> {
    let declare = |m: &mut JITModule, name: &str, sig: &Signature| {
        m.declare_function(name, Linkage::Import, sig)
            .map_err(|e| JitError::Codegen(format!("declare {name}: {e}")))
    };

    let sig1 = make_sig(module, &[ptr], Some(types::I64));
    let sig2 = make_sig(module, &[ptr, types::I64], Some(types::I64));
    let sig3 = make_sig(module, &[ptr, types::I64, types::I64], Some(types::I64));
    let sig4 = make_sig(
        module,
        &[ptr, types::I64, types::I64, types::I64],
        Some(types::I64),
    );
    let sig_free = make_sig(module, &[ptr, types::I64], None);
    let sig5 = make_sig(
        module,
        &[ptr, types::I64, types::I64, types::I64, types::I64],
        Some(types::I64),
    );
    // Two-i64 return (value, code) — the same multi-return shape as the compiled
    // functions' own signatures (see [`UbMapGetRet`] for the ABI note).
    let mut sig_map_get = make_sig(
        module,
        &[ptr, types::I64, types::I64, types::I64],
        Some(types::I64),
    );
    sig_map_get.returns.push(AbiParam::new(types::I64));
    Ok(UbHelperIds {
        list_new: declare(module, "rt_u_list_new", &sig2)?,
        list_push: declare(module, "rt_u_list_push", &sig4)?,
        list_seal: declare(module, "rt_u_list_seal", &sig2)?,
        index: declare(module, "rt_u_index", &sig4)?,
        concat: declare(module, "rt_u_concat", &sig4)?,
        str_len: declare(module, "rt_u_str_len", &sig3)?,
        free: declare(module, "rt_u_free", &sig_free)?,
        map_push_pair: declare(module, "rt_u_map_push_pair", &sig5)?,
        map_seal: declare(module, "rt_u_map_seal", &sig2)?,
        set_seal: declare(module, "rt_u_set_seal", &sig2)?,
        map_get: declare(module, "rt_u_map_get", &sig_map_get)?,
        // Same two-i64 (present, code) return shape as `map_get`.
        map_has: declare(module, "rt_u_map_has", &sig_map_get)?,
        list_push_int: declare(module, "rt_u_list_push_int", &sig3)?,
        index_int: {
            let mut s = make_sig(
                module,
                &[ptr, types::I64, types::I64, types::I64],
                Some(types::I64),
            );
            s.returns.push(AbiParam::new(types::I64));
            declare(module, "rt_u_index_int", &s)?
        },
        int_to_str: declare(module, "rt_u_int_to_str", &sig2)?,
        concat_mix: {
            let s = make_sig(
                module,
                &[
                    ptr,
                    types::I64,
                    types::I64,
                    types::I64,
                    types::I64,
                    types::I64,
                    types::I64,
                    types::I64,
                    types::I64,
                    types::I64,
                ],
                Some(types::I64),
            );
            declare(module, "rt_u_concat_mix", &s)?
        },
        acc_append: declare(module, "rt_u_acc_append", &sig4)?,
        list_len: declare(module, "rt_u_list_len", &sig2)?,
        list_acc_append: declare(module, "rt_u_list_acc_append", &sig3)?,
        map_builder_set: declare(module, "rt_u_map_builder_set", &sig4)?,
        map_builder_seed: declare(module, "rt_u_map_builder_seed", &sig4)?,
        list_acc_reseed: declare(module, "rt_u_list_acc_reseed", &sig3)?,
        list_builder_new: declare(module, "rt_u_list_builder_new", &sig1)?,
        list_append_clone: declare(module, "rt_u_list_append_clone", &sig4)?,
        native2: {
            let mut s = make_sig(
                module,
                &[ptr, types::I64, types::I64, types::I64, types::I64],
                Some(types::I64),
            );
            s.returns.push(AbiParam::new(types::I64));
            declare(module, "rt_u_native2", &s)?
        },
        str_eq: declare(module, "rt_u_str_eq", &sig4)?,
        clone_value: declare(module, "rt_u_clone_value", &sig3)?,
        list_append_dyn: {
            let s = make_sig(
                module,
                &[ptr, types::I64, types::I64, types::I64, types::I64],
                Some(types::I64),
            );
            declare(module, "rt_u_list_append_dyn", &s)?
        },
        str_list_acc_append: {
            let s = make_sig(module, &[ptr, types::I64, types::I64], Some(types::I64));
            declare(module, "rt_u_str_list_acc_append", &s)?
        },
        map_keys: declare(module, "rt_u_map_keys", &sig3)?,
        map_values: declare(module, "rt_u_map_values", &sig3)?,
        map_merge: declare(module, "rt_u_map_merge", &sig4)?,
        map_size: declare(module, "rt_u_map_size", &sig3)?,
        map_ext_new: declare(module, "rt_u_map_ext_new", &sig2)?,
        map_ext_push: declare(module, "rt_u_map_ext_push", &sig4)?,
        str_contains: declare(module, "rt_u_str_contains", &sig4)?,
        validate: declare(module, "rt_u_validate", &sig4)?,
    })
}
