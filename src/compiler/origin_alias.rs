//! Inherited method-table entries (M-RT S6/S6b; property hooks since DEC-572, plan row 4e), split out
//! of `program.rs`. An inherited `(class, name)` is a table ALIAS to the fn index its declaring class
//! already registered: no function is compiled twice, and the VM, the JIT and `ctype` all read the
//! same `methods` / `method_rets` entries a declared member would have.

use super::*;

/// The per-`(class, method)` tables an alias copies, borrowed from `compile_program`.
pub(super) struct MethodTables<'a> {
    pub(super) methods: &'a mut HashMap<(String, String), usize>,
    pub(super) rets: &'a mut HashMap<(String, String), CTy>,
    pub(super) overloads: &'a mut HashMap<(String, String), usize>,
    pub(super) generic_ret_from_param: &'a mut HashMap<(String, String), usize>,
}

impl MethodTables<'_> {
    /// Alias every `(class, name) -> (origin class, origin name)` of a shared origins table
    /// (`ast::class_method_origins` or `ast::class_hook_origins`) whose origin is registered. A class's
    /// own entry (or one already aliased) wins; an origin with no registered fn (a stored field that
    /// shadows a hook, DEC-582) adds nothing, so that member stays a field access.
    pub(super) fn alias(
        &mut self,
        origins: &std::collections::BTreeMap<(String, String), (String, String)>,
    ) {
        for ((cname, name), (oc, om)) in origins {
            let key = (cname.clone(), name.clone());
            if self.methods.contains_key(&key) {
                continue; // own / already-registered entry wins
            }
            let anc_key = (oc.clone(), om.clone());
            let Some(idx) = self.methods.get(&anc_key).copied() else {
                continue;
            };
            self.methods.insert(key.clone(), idx);
            if let Some(rty) = self.rets.get(&anc_key).cloned() {
                self.rets.insert(key.clone(), rty);
            }
            if let Some(set_id) = self.overloads.get(&anc_key).copied() {
                self.overloads.insert(key.clone(), set_id);
            }
            if let Some(i) = self.generic_ret_from_param.get(&anc_key).copied() {
                self.generic_ret_from_param.insert(key, i);
            }
        }
    }
}
