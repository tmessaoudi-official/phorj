//! Type-parameter substitution builders for generic class/enum member access.

use super::*;

impl Checker {
    /// Build the substitution mapping a generic class's type parameters to a concrete instance's type
    /// arguments — `{T → int}` for a `Box<int>` receiver (M-RT generics-all). empty (the identity
    /// substitution) for a non-generic class or any non-class name, so member/method access on a
    /// non-generic type is unchanged. `zip` tolerates an arity mismatch defensively.
    pub(in crate::checker) fn class_subst(&self, cls: &str, cargs: &[Ty]) -> HashMap<String, Ty> {
        match self.classes.get(cls) {
            Some(info) => info
                .type_params
                .iter()
                .cloned()
                .zip(cargs.iter().cloned())
                .collect(),
            // DEC-257 generic interfaces: an interface-typed receiver (`Producer<int> p`)
            // substitutes the INTERFACE's type parameters, so `p.produce()` types as `int`,
            // not the raw `T` from the flattened signature.
            None => self
                .interfaces
                .get(cls)
                .map(|i| {
                    i.type_params
                        .iter()
                        .cloned()
                        .zip(cargs.iter().cloned())
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// The substitution mapping a generic enum's type parameters to a scrutinee's type arguments
    /// (`Option<int>` ⇒ `{T → int}`), so a `match` binds a variant payload at the concrete type
    /// (`Some(n)` ⇒ `n: int`). empty for a non-generic enum, so it is the identity in the common case
    /// (M-RT generic enums). Mirror of [`class_subst`].
    pub(in crate::checker) fn enum_subst(
        &self,
        enum_name: &str,
        eargs: &[Ty],
    ) -> HashMap<String, Ty> {
        match self.enums.get(enum_name) {
            Some(info) => info
                .type_params
                .iter()
                .cloned()
                .zip(eargs.iter().cloned())
                .collect(),
            None => HashMap::new(),
        }
    }
}
