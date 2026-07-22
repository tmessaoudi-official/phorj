//! DEC-329.3 — the variant descriptor index (M-Decomp split from `mod.rs`, Invariant 13).

use super::*;

/// Per-variant metadata gathered in the pre-pass: its index into the `enum_descs` table (for
/// `MakeEnum`/`MatchTag`) and the class-aware type of each payload field (so a payload binding —
/// including a class-typed one — resolves through `ctype`). Decision P4-2.
pub(super) struct VariantMeta {
    pub(super) index: usize,
    pub(super) field_tags: Vec<CTy>,
}

/// DEC-329.3: the variant lookup, keyed BOTH ways. `by_enum` is the precise (enum → variant →
/// meta) index the canonical qualified forms resolve through (`qualify_variants` rewrites every
/// construction/pattern to carry its owning enum, so a variant name shared by two enums picks the
/// RIGHT descriptor). `owner` maps a bare variant name to its declaring enum (last declaration
/// wins — exactly the pre-329.3 single-map behavior) and serves only the documented fallbacks: a
/// qualification miss (impossible for checked programs) and the duck-typed `?`
/// ([`Compiler::compile_propagate`], which tests by variant NAME per the `Result`-shaped contract).
#[derive(Default)]
pub(super) struct VariantIndex {
    by_enum: HashMap<String, HashMap<String, VariantMeta>>,
    owner: HashMap<String, String>,
}

impl VariantIndex {
    pub(super) fn insert(&mut self, enum_name: &str, variant: &str, meta: VariantMeta) {
        self.by_enum
            .entry(enum_name.to_string())
            .or_default()
            .insert(variant.to_string(), meta);
        self.owner
            .insert(variant.to_string(), enum_name.to_string());
    }
    /// Resolve a variant: precise when the canonical qualifier is present, bare-owner fallback
    /// otherwise.
    pub(super) fn get(&self, enum_qualifier: Option<&str>, name: &str) -> Option<&VariantMeta> {
        let en = match enum_qualifier {
            Some(en) => en,
            None => self.owner.get(name)?,
        };
        self.by_enum.get(en)?.get(name)
    }
    /// Whether `name` is a declared variant of ANY enum (the bare-name membership tests).
    pub(super) fn contains_bare(&self, name: &str) -> bool {
        self.owner.contains_key(name)
    }
}
