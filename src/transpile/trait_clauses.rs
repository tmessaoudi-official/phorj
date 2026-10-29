//! PHP transpiler — the `use` block clauses of a multi-parent class and of a class composing traits:
//! `insteadof`/`as` resolution for trait-method collisions (M-RT S6b, Wave 1.3). Split from
//! `classes_synth.rs` (Invariant 13).

use super::*;

impl Transpiler {
    /// The `insteadof`/`as` clauses for a multi-parent class's `use` block (M-RT S6b). A method name
    /// supplied by ≥2 direct parents with **distinct origins** is a real PHP trait collision needing
    /// `insteadof` (a diamond shared base — same origin through both arms — is skipped, PHP auto-merges
    /// it). The winner is the parent named by a `use P.m` clause, else the single parent left after
    /// `rename`/`exclude` remove the others; every other providing parent's trait is listed after
    /// `insteadof`. A class that overrides the method itself needs no clause (the class method wins). A
    /// `rename P.m as n` also emits `T<P>::m as n;`.
    pub(super) fn build_trait_clauses(&self, c: &ClassDecl, program: &Program) -> Vec<String> {
        use crate::ast::Resolution;
        let (origins, _conflicts) = crate::ast::class_method_origins(program);
        // method name -> [(direct parent, origin (declaring class, method))]
        let mut provides: std::collections::BTreeMap<String, Vec<(String, Origin)>> =
            std::collections::BTreeMap::new();
        for ((cls, name), origin) in &origins {
            if c.extends.contains(cls) {
                provides
                    .entry(name.clone())
                    .or_default()
                    .push((cls.clone(), origin.clone()));
            }
        }
        let own: std::collections::BTreeSet<&str> = c
            .members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Method(f) => Some(f.name.as_str()),
                _ => None,
            })
            .collect();
        let mut clauses = Vec::new();
        for (m, entries) in &provides {
            let distinct: std::collections::BTreeSet<&Origin> =
                entries.iter().map(|(_, o)| o).collect();
            if distinct.len() < 2 || own.contains(m.as_str()) {
                continue; // diamond auto-merge, single source, or overridden by the class itself
            }
            let providing: std::collections::BTreeSet<String> =
                entries.iter().map(|(p, _)| p.clone()).collect();
            // The winner: `use P.m` names it; otherwise the one parent left after rename/exclude.
            let used = c.resolutions.iter().find_map(|r| match r {
                Resolution::Use { parent, method, .. } if method == m => Some(parent.clone()),
                _ => None,
            });
            let removed: std::collections::BTreeSet<String> = c
                .resolutions
                .iter()
                .filter_map(|r| match r {
                    Resolution::Rename { parent, method, .. } if method == m => {
                        Some(parent.clone())
                    }
                    Resolution::Exclude { parent, method, .. } if method == m => {
                        Some(parent.clone())
                    }
                    _ => None,
                })
                .collect();
            let winner = used.or_else(|| providing.iter().find(|p| !removed.contains(*p)).cloned());
            if let Some(w) = winner {
                let losers: Vec<String> = providing
                    .iter()
                    .filter(|p| **p != w)
                    .map(|p| format!("T{p}"))
                    .collect();
                if !losers.is_empty() {
                    clauses.push(format!("T{w}::{m} insteadof {};", losers.join(", ")));
                }
            }
        }
        for r in &c.resolutions {
            if let Resolution::Rename {
                parent,
                method,
                as_name,
                ..
            } = r
            {
                clauses.push(format!("T{parent}::{method} as {as_name};"));
            }
        }
        clauses
    }

    /// The `insteadof`/`as` clauses for an explicit trait-composition (`use P; use Q;`) block when two
    /// composed traits supply the same method name (Wave 1.3). The Phorj-side resolution
    /// (`use P.m`/`rename`/`exclude`) is already validated by the checker; this lowers it to PHP. The
    /// trait-composition analogue of [`build_trait_clauses`] (which handles MI-decomposed parents and
    /// uses `T<parent>` names): here the providing sources are the directly-declared methods of each
    /// `use`d trait, named directly. A method the class overrides itself, or supplied by only one trait,
    /// needs no clause. (A collision via a trait's *own* nested `use` is not detected here — only direct
    /// declarations; that narrower case is caught by the PHP oracle if it ever arises.)
    pub(super) fn build_use_trait_clauses(&self, c: &ClassDecl, program: &Program) -> Vec<String> {
        use crate::ast::{Item, Resolution};
        // Directly-declared method names of a `use`d trait.
        let trait_methods = |name: &str| -> std::collections::BTreeSet<String> {
            program
                .items
                .iter()
                .find_map(|it| match it {
                    Item::Trait(t) if t.name == name => Some(
                        t.members
                            .iter()
                            .filter_map(|m| match m {
                                ClassMember::Method(f) => Some(f.name.clone()),
                                _ => None,
                            })
                            .collect(),
                    ),
                    _ => None,
                })
                .unwrap_or_default()
        };
        // method name -> set of composed traits supplying it directly.
        let mut provides: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
            std::collections::BTreeMap::new();
        for u in &c.uses {
            for m in trait_methods(&u.name) {
                provides.entry(m).or_default().insert(u.name.clone());
            }
        }
        let own: std::collections::BTreeSet<&str> = c
            .members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Method(f) => Some(f.name.as_str()),
                _ => None,
            })
            .collect();
        let mut clauses = Vec::new();
        for (m, traits) in &provides {
            if traits.len() < 2 || own.contains(m.as_str()) {
                continue; // single source, or the class overrides it (the class method wins)
            }
            // The winner: `use P.m` names it; else the one trait left after `rename`/`exclude`.
            let used = c.resolutions.iter().find_map(|r| match r {
                Resolution::Use { parent, method, .. } if method == m => Some(parent.clone()),
                _ => None,
            });
            let removed: std::collections::BTreeSet<String> = c
                .resolutions
                .iter()
                .filter_map(|r| match r {
                    Resolution::Rename { parent, method, .. } if method == m => {
                        Some(parent.clone())
                    }
                    Resolution::Exclude { parent, method, .. } if method == m => {
                        Some(parent.clone())
                    }
                    _ => None,
                })
                .collect();
            let winner = used.or_else(|| traits.iter().find(|p| !removed.contains(*p)).cloned());
            if let Some(w) = winner {
                let losers: Vec<String> = traits.iter().filter(|p| **p != w).cloned().collect();
                if !losers.is_empty() {
                    clauses.push(format!("{w}::{m} insteadof {};", losers.join(", ")));
                }
            }
        }
        for r in &c.resolutions {
            if let Resolution::Rename {
                parent,
                method,
                as_name,
                ..
            } = r
            {
                clauses.push(format!("{parent}::{method} as {as_name};"));
            }
        }
        clauses
    }
}
