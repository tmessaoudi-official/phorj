//! `rt_u_*` helper SYMBOL registration for the Cranelift JITBuilder (M-Decomp from
//! `compile.rs`, Invariant 13): the single place a new runtime helper is exposed to codegen —
//! keep in lockstep with `UbHelperIds`/`UbHelperRefs` and the `declare` list in `compile.rs`.

use super::*;

/// Register every unboxed runtime helper symbol on the JIT builder (called iff `uses_handles`).
pub(in crate::jit) fn register_ub_symbols(builder: &mut JITBuilder) {
    builder.symbol("rt_u_list_new", rt_u_list_new as *const u8);
    builder.symbol("rt_u_list_push", rt_u_list_push as *const u8);
    builder.symbol("rt_u_list_seal", rt_u_list_seal as *const u8);
    builder.symbol("rt_u_index", rt_u_index as *const u8);
    builder.symbol("rt_u_concat", rt_u_concat as *const u8);
    builder.symbol("rt_u_str_len", rt_u_str_len as *const u8);
    builder.symbol("rt_u_free", rt_u_free as *const u8);
    builder.symbol("rt_u_map_push_pair", rt_u_map_push_pair as *const u8);
    builder.symbol("rt_u_map_seal", rt_u_map_seal as *const u8);
    builder.symbol("rt_u_set_seal", rt_u_set_seal as *const u8);
    builder.symbol("rt_u_map_get", rt_u_map_get as *const u8);
    builder.symbol("rt_u_map_has", rt_u_map_has as *const u8);
    builder.symbol("rt_u_list_push_int", rt_u_list_push_int as *const u8);
    builder.symbol("rt_u_index_int", rt_u_index_int as *const u8);
    builder.symbol("rt_u_int_to_str", rt_u_int_to_str as *const u8);
    builder.symbol("rt_u_concat_mix", rt_u_concat_mix as *const u8);
    builder.symbol("rt_u_acc_append", rt_u_acc_append as *const u8);
    builder.symbol("rt_u_list_len", rt_u_list_len as *const u8);
    builder.symbol("rt_u_list_acc_append", rt_u_list_acc_append as *const u8);
    builder.symbol("rt_u_map_builder_set", rt_u_map_builder_set as *const u8);
    builder.symbol("rt_u_map_builder_seed", rt_u_map_builder_seed as *const u8);
    builder.symbol("rt_u_list_acc_reseed", rt_u_list_acc_reseed as *const u8);
    builder.symbol("rt_u_list_builder_new", rt_u_list_builder_new as *const u8);
    builder.symbol(
        "rt_u_list_append_clone",
        rt_u_list_append_clone as *const u8,
    );
    builder.symbol("rt_u_native2", rt_u_native2 as *const u8);
    builder.symbol("rt_u_str_eq", rt_u_str_eq as *const u8);
    builder.symbol("rt_u_clone_value", rt_u_clone_value as *const u8);
    builder.symbol("rt_u_list_append_dyn", rt_u_list_append_dyn as *const u8);
    builder.symbol(
        "rt_u_str_list_acc_append",
        rt_u_str_list_acc_append as *const u8,
    );
    builder.symbol("rt_u_map_keys", rt_u_map_keys as *const u8);
    builder.symbol("rt_u_map_values", rt_u_map_values as *const u8);
    builder.symbol("rt_u_map_merge", rt_u_map_merge as *const u8);
    builder.symbol("rt_u_map_size", rt_u_map_size as *const u8);
}
