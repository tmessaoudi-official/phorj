//! Bytecode compiler — the checker's reified side-tables ([`crate::checker::Reified`]) converted to
//! compile types once, at `compile_with`. `ctype` falls back to `operands` where its own resolution is
//! unknown (S2.1-broad), `collect_bindings` to `binding` for a `match` binding whose payload type
//! the compiler erased (GENERIC-PAYLOAD-OPERAND), and `ctype` to `propagate` for a `var x = f()?`
//! initializer (TRY-UNWRAP-OPERAND).

use super::*;

/// The compile-type view of [`crate::checker::Reified`]; empty on the plain `compile` path.
#[derive(Default)]
pub(in crate::compiler) struct ReifiedCty {
    operands: HashMap<usize, CTy>,
    binds: HashMap<usize, (String, CTy)>,
    propagates: HashMap<usize, CTy>,
}

impl ReifiedCty {
    /// Convert each entry to its operand `CTy`, dropping unknown ones, so a non-operand never
    /// overrides the compiler's own resolution.
    pub(in crate::compiler) fn from_checker(r: &crate::checker::Reified) -> Self {
        let known = |ty: &crate::types::Ty| Some(ty_to_cty(ty)).filter(|c| *c != CTy::Other);
        ReifiedCty {
            operands: r
                .operands
                .iter()
                .filter_map(|(&span, ty)| Some((span, known(ty)?)))
                .collect(),
            binds: r
                .pattern_binds
                .iter()
                .filter_map(|(&span, (name, ty))| Some((span, (name.clone(), known(ty)?))))
                .collect(),
            propagates: r
                .propagates
                .iter()
                .filter_map(|(&span, ty)| Some((span, known(ty)?)))
                .collect(),
        }
    }

    /// Guards `ctype`'s fallback, so it covers every table that fallback reads.
    pub(in crate::compiler) fn is_empty(&self) -> bool {
        self.operands.is_empty() && self.propagates.is_empty()
    }

    /// The reified operand type of the `Call`/`Member`/`Index` starting at `span_start`.
    pub(in crate::compiler) fn get(&self, span_start: &usize) -> Option<&CTy> {
        self.operands.get(span_start)
    }

    /// The unwrapped `Success` type of the Result-mode `?` at `span_start` (TRY-UNWRAP-OPERAND).
    pub(in crate::compiler) fn propagate(&self, span_start: usize) -> Option<CTy> {
        self.propagates.get(&span_start).cloned()
    }

    /// The checker's type for the `match` binding `name` declared at `span_start`; the name must
    /// match, so a binding the checker never saw (a desugared one) cannot pick up another's type.
    pub(in crate::compiler) fn binding(&self, span_start: usize, name: &str) -> Option<CTy> {
        let (n, ty) = self.binds.get(&span_start)?;
        (n == name).then(|| ty.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Ty;

    fn table(name: &str, ty: Ty) -> ReifiedCty {
        let mut r = crate::checker::Reified::default();
        r.pattern_binds.insert(7, (name.to_string(), ty));
        ReifiedCty::from_checker(&r)
    }

    #[test]
    fn a_binding_type_is_returned_only_for_its_own_name() {
        let t = table("v", Ty::Int);
        assert!(t.binding(7, "v") == Some(CTy::Int));
        assert!(t.binding(7, "w").is_none());
    }

    #[test]
    fn a_binding_type_is_returned_only_at_its_own_span() {
        assert!(table("v", Ty::Int).binding(8, "v").is_none());
    }

    #[test]
    fn a_table_holding_only_propagates_is_not_empty() {
        let mut r = crate::checker::Reified::default();
        r.propagates.insert(3, Ty::Int);
        let t = ReifiedCty::from_checker(&r);
        assert!(!t.is_empty());
        assert!(t.propagate(3) == Some(CTy::Int));
    }

    #[test]
    fn an_unknown_binding_type_is_dropped() {
        assert!(table("v", Ty::Error).binding(7, "v").is_none());
    }
}
