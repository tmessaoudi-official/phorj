//! Unboxed helper-REF import (M-Decomp from `emit_unboxed/mod.rs`, Invariant 13): resolve every
//! [`UbHelperIds`] `FuncId` into a per-function `FuncRef` — the single place a new helper joins
//! the emit-side table (keep in lockstep with `handles/symbols.rs` and the `compile.rs` declares).

use super::*;

/// Import every unboxed runtime helper into the function under construction.
pub(super) fn declare_ub_refs(
    m: &mut JITModule,
    ids: &UbHelperIds,
    f: &mut cranelift::codegen::ir::Function,
) -> UbHelperRefs {
    UbHelperRefs {
        list_new: m.declare_func_in_func(ids.list_new, f),
        list_push: m.declare_func_in_func(ids.list_push, f),
        list_seal: m.declare_func_in_func(ids.list_seal, f),
        index: m.declare_func_in_func(ids.index, f),
        concat: m.declare_func_in_func(ids.concat, f),
        str_len: m.declare_func_in_func(ids.str_len, f),
        free: m.declare_func_in_func(ids.free, f),
        map_push_pair: m.declare_func_in_func(ids.map_push_pair, f),
        map_seal: m.declare_func_in_func(ids.map_seal, f),
        set_seal: m.declare_func_in_func(ids.set_seal, f),
        map_get: m.declare_func_in_func(ids.map_get, f),
        map_has: m.declare_func_in_func(ids.map_has, f),
        list_push_int: m.declare_func_in_func(ids.list_push_int, f),
        index_int: m.declare_func_in_func(ids.index_int, f),
        int_to_str: m.declare_func_in_func(ids.int_to_str, f),
        concat_mix: m.declare_func_in_func(ids.concat_mix, f),
        acc_append: m.declare_func_in_func(ids.acc_append, f),
        list_len: m.declare_func_in_func(ids.list_len, f),
        list_acc_append: m.declare_func_in_func(ids.list_acc_append, f),
        map_builder_set: m.declare_func_in_func(ids.map_builder_set, f),
        map_builder_seed: m.declare_func_in_func(ids.map_builder_seed, f),
        list_acc_reseed: m.declare_func_in_func(ids.list_acc_reseed, f),
        list_builder_new: m.declare_func_in_func(ids.list_builder_new, f),
        list_append_clone: m.declare_func_in_func(ids.list_append_clone, f),
        native2: m.declare_func_in_func(ids.native2, f),
        str_eq: m.declare_func_in_func(ids.str_eq, f),
        clone_value: m.declare_func_in_func(ids.clone_value, f),
        list_append_dyn: m.declare_func_in_func(ids.list_append_dyn, f),
        str_list_acc_append: m.declare_func_in_func(ids.str_list_acc_append, f),
        map_keys: m.declare_func_in_func(ids.map_keys, f),
        map_values: m.declare_func_in_func(ids.map_values, f),
        map_merge: m.declare_func_in_func(ids.map_merge, f),
        map_size: m.declare_func_in_func(ids.map_size, f),
        map_ext_new: m.declare_func_in_func(ids.map_ext_new, f),
        map_ext_push: m.declare_func_in_func(ids.map_ext_push, f),
        str_contains: m.declare_func_in_func(ids.str_contains, f),
        validate: m.declare_func_in_func(ids.validate, f),
    }
}
