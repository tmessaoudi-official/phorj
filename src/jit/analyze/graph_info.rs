//! The per-graph FIXPOINT STATE for the unboxed path (Invariant 13 split out of `analyze/mod.rs`).
//!
//! One cohesive unit: everything the cross-function fixpoint accumulates about a call graph —
//! per-function return kinds, call-site parameter overrides (`param_over`), method receiver classes
//! and instance field signatures — plus [`UbGraphInfo::param_kinds`], which merges those recordings
//! with the compiler-stamped checker facts into a function's FINAL param kinds.
//!
//! Split because the module had reached its frozen size baseline and this is the natural seam: the
//! rest of `analyze` WALKS bytecode, this HOLDS what the walk learned across functions. Bodies moved
//! verbatim.

use super::*;

/// Per-graph cross-function facts for the unboxed path, produced by [`resolve_unboxed_graph`]'s
/// fixpoint: each function's return KIND (`None` until computed — callers assume `Int`) and,
/// for method bodies, the receiver class injected as param 0 (`this`). Both are read by the
/// analyze pass AND by `build_body_unboxed` (which re-runs analyze on the stable facts).
#[derive(Clone, Debug, Default)]
pub(in crate::jit) struct UbGraphInfo {
    pub(in crate::jit) ret_kinds: Vec<Option<Kind>>,
    pub(in crate::jit) this_inst: Vec<Option<usize>>,
    /// Per-CLASS field kinds (layout-slot order NOT — ctor push order; every `MakeInstance`
    /// site of a class must agree, ownership-normalized). `Int` fields are raw words; `Str`
    /// fields are handle words the instance OWNS (released with it, runtime-bit-gated).
    pub(in crate::jit) field_kinds: Vec<Option<Vec<Kind>>>,
    /// Per-FUNCTION param-kind overrides recorded from CALL SITES that pass handle args
    /// (a string argument MOVES into the callee — normalized `Str(Owned)`); all sites must
    /// agree. `None` = no override (usage-proven kinds apply); `Unknown` cells = no override
    /// for that one param.
    pub(in crate::jit) param_over: Vec<Option<Vec<Kind>>>,
    /// The graph-wide THROWN class (v1: a single throwable class per graph, else fallback) —
    /// types every catch pad's incoming value.
    pub(in crate::jit) thrown_class: Option<usize>,
    /// The graph's ENTRY function index. The `str_params` seed (declared-`string` params →
    /// `Str(Borrowed)`) applies ROOT-ONLY: seeding an internal callee would clobber its
    /// call-site-proven `Str(Owned)` args (the moved-in ownership → a leak). Internal callees
    /// receive their str kinds from `param_over` (call-site facts); only the root, whose args
    /// arrive marshalled by `run_unboxed`, is seeded here. [DEC-333 R3-comp-F1 / R2-B6]
    pub(in crate::jit) entry_idx: usize,
    /// The `enum_descs` base index of the injected canonical `Core.Json` ADT, iff the program
    /// stamped [`BytecodeProgram::canonical_json`] (three-conjunct: `injected` + name `Json` +
    /// the 7-variant prelude shape). `None` = no canonical Json in the program (a user look-alike
    /// `enum Json` never sets it). The Json-ADT op arms read THIS to map a `MakeEnum`/`MatchTag`
    /// descriptor index to its relative tag 0..6 (Null..Object) — never by sniffing `enum_descs`
    /// shape. [DEC-333 R2-safety-F5 / R4-corr-2]
    #[allow(dead_code)] // read by the 5b Json-ADT arms; kept dead until they land.
    pub(in crate::jit) canonical_json: Option<u32>,
}

impl UbGraphInfo {
    pub(in crate::jit) fn new(
        n: usize,
        n_classes: usize,
        entry_idx: usize,
        canonical_json: Option<u32>,
    ) -> Self {
        Self {
            ret_kinds: vec![None; n],
            this_inst: vec![None; n],
            field_kinds: vec![None; n_classes],
            param_over: vec![None; n],
            thrown_class: None,
            entry_idx,
            canonical_json,
        }
    }
    /// The kind a `GetField` of ctor-push-position `j` on class `c` yields (`None` = the
    /// class's signature is not yet recorded — the fixpoint retries).
    pub(in crate::jit) fn field_kind(&self, c: usize, j: usize) -> Option<Kind> {
        self.field_kinds.get(c)?.as_ref()?.get(j).copied()
    }
    /// The kind a caller's stack receives from calling `callee` (`Int` until the fixpoint
    /// fills it — the pre-object behavior, so pure-int graphs converge in one pass).
    pub(in crate::jit) fn ret_of(&self, callee: usize) -> Kind {
        self.ret_kinds
            .get(callee)
            .copied()
            .flatten()
            .unwrap_or(Kind::Int)
    }
    /// Effective param kinds for `func_idx`: the usage-proven seeds, with a method body's
    /// slot 0 overridden to its receiver class (`this` arrives as a BORROWED instance handle)
    /// and declared scalar-union params seeded `Dyn` (`dyn_params` — the compiler-stamped
    /// checker fact; slot-aligned, so it applies LAST and wins over both usage proofs and
    /// call-site overrides: the declaration is ground truth, and Dyn is the superset every
    /// dynable site kind crosses into).
    pub(in crate::jit) fn param_kinds(
        &self,
        func_idx: usize,
        proven: &[Option<Kind>],
        arity: usize,
        dyn_params: &[bool],
        str_params: &[bool],
    ) -> Vec<Kind> {
        let mut pk: Vec<Kind> = (0..arity)
            .map(|s| proven.get(s).copied().flatten().unwrap_or(Kind::Unknown))
            .collect();
        // Call-site-recorded overrides (handle args) beat usage proofs (a str param feeding
        // MakeInstance is usage-proven "Int" by the conservative pre-pass — the override wins).
        if let Some(Some(over)) = self.param_over.get(func_idx) {
            for (s, k) in over.iter().enumerate() {
                if *k != Kind::Unknown {
                    if let Some(slot) = pk.get_mut(s) {
                        *slot = *k;
                    }
                }
            }
        }
        if let Some(c) = self.this_inst.get(func_idx).copied().flatten() {
            // L2b: `this` compiles OWNED only when the sig merge proved EVERY call site
            // passes an owned dying receiver (the fluent-chain shape — the site TRANSFERS
            // the word and the method's teardown releases the husk). Any borrowed site,
            // or no sig yet, keeps the safe borrowed compile.
            let own = match self.param_over.get(func_idx) {
                Some(Some(over)) => match over.first() {
                    Some(Kind::Inst(c2, Own::Owned)) if *c2 == c => Own::Owned,
                    _ => Own::Borrowed,
                },
                _ => Own::Borrowed,
            };
            if let Some(p0) = pk.get_mut(0) {
                *p0 = Kind::Inst(c, own);
            }
        }
        // W7: the declared-union seed — WITHOUT it, a mid-chain method that both takes and
        // consumes the union param deadlocks the fixpoint (its walk can't finish until the
        // param is Dyn; the param can't join to Dyn until later chain sites are reached).
        for (s, is_dyn) in dyn_params.iter().enumerate() {
            if *is_dyn {
                if let Some(slot) = pk.get_mut(s) {
                    *slot = Kind::Dyn;
                }
            }
        }
        // DEC-333 [R3-comp-F1 / R2-B6]: seed declared-`string` params of the ROOT function to
        // `Str(Borrowed)`. Root-only — `run_unboxed` marshals the entry's args into fresh
        // untagged ctx handles the body borrows (never releases); an internal callee gets its
        // str kinds from `param_over` (call-site ownership), so seeding it would demote a
        // proven `Str(Owned)` and leak the moved-in word. Guarded on `Unknown` so a call-site
        // or usage proof (never `Str` in practice for a declared-string param, but defensive)
        // is never overwritten — the fail-closed direction.
        if func_idx == self.entry_idx {
            for (s, is_str) in str_params.iter().enumerate() {
                if *is_str {
                    if let Some(slot) = pk.get_mut(s) {
                        if *slot == Kind::Unknown {
                            *slot = Kind::Str(Own::Borrowed);
                        }
                    }
                }
            }
        }
        pk
    }
}
