//! Bytecode compiler — the checker's reified side-tables ([`crate::checker::Reified`]) converted to
//! compile types once, at `compile_with`. `ctype` falls back to `operands` where its own resolution is
//! unknown (S2.1-broad), and `collect_bindings` to `binding` for a `match` binding whose payload type
//! the compiler erased (GENERIC-PAYLOAD-OPERAND).

use super::*;

/// The compile-type view of [`crate::checker::Reified`]; empty on the plain `compile` path.
#[derive(Default)]
pub(in crate::compiler) struct ReifiedCty {
    operands: HashMap<usize, CTy>,
    binds: HashMap<usize, (String, CTy)>,
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
        }
    }

    pub(in crate::compiler) fn is_empty(&self) -> bool {
        self.operands.is_empty()
    }

    /// The reified operand type of the `Call`/`Member`/`Index` starting at `span_start`.
    pub(in crate::compiler) fn get(&self, span_start: &usize) -> Option<&CTy> {
        self.operands.get(span_start)
    }

    /// The checker's type for the `match` binding `name` declared at `span_start`; the name must
    /// match, so a binding the checker never saw (a desugared one) cannot pick up another's type.
    pub(in crate::compiler) fn binding(&self, span_start: usize, name: &str) -> Option<CTy> {
        let (n, ty) = self.binds.get(&span_start)?;
        (n == name).then(|| ty.clone())
    }
}
