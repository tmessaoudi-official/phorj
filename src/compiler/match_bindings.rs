//! Bytecode compiler — `match`-arm bindings (split out of `matches.rs` for R9-K1). One walker of a
//! `Pattern` collects the bindings an arm introduces; `compile_match` registers them while it compiles
//! the arm, and `ctype` types an arm's body under them through `binding_overlay` (it takes `&self`).

use super::*;

impl Compiler<'_> {
    /// Register (emitting no code) every binding introduced by `pat`, so the arm body can
    /// re-extract them. `cur_ty` is the class-aware type of the value `pat` matches (for `ctype`) —
    /// a class-typed payload keeps its class, so `binding.field` resolves (Wave 4).
    pub(super) fn register_bindings(
        &mut self,
        pat: &Pattern,
        m_slot: usize,
        path: &[PathSeg],
        cur_ty: CTy,
    ) -> Result<(), String> {
        let mut found = Vec::new();
        self.collect_bindings(pat, m_slot, path, cur_ty, &mut found)?;
        self.match_bindings.extend(found);
        Ok(())
    }

    /// `ctype` of a `match` value (R9-K1). Each non-throwing arm (DEC-532) is typed under its OWN
    /// pattern bindings — `compile_match` registers them only while it compiles that arm, so without
    /// the overlay `A(v) => v` typed as `undefined variable` and `match … + 1` failed on the VM alone.
    /// The first such arm decides (the checker gives every arm one shared type), except that arms of
    /// incompatible types — `Kid` and `Par`, which the checker unifies to the wider class — give
    /// `Other`, never one arm's guess: an exact-class `with` or hook must not be compiled for a value
    /// of another class.
    pub(in crate::compiler) fn ctype_match(
        &self,
        scrutinee: &Expr,
        arms: &[MatchArm],
    ) -> Result<CTy, String> {
        let scrut = self.ctype(scrutinee).unwrap_or(CTy::Other); // as `compile_match` types it
        join_arm_ctys(
            arms.iter()
                .filter(|a| !matches!(a.body, Expr::Throw { .. }))
                .map(|arm| self.ctype_arm(arm, &scrut)),
        )
    }

    /// `ctype` of one arm's body with the arm's bindings in the overlay. The bindings are collected
    /// BEFORE the push, so nothing can return between the push and the truncate.
    fn ctype_arm(&self, arm: &MatchArm, scrut: &CTy) -> Result<CTy, String> {
        let mut found = Vec::new();
        self.collect_bindings(&arm.pattern, 0, &[], scrut.clone(), &mut found)?;
        let depth = self.binding_overlay.borrow().len();
        let named = found.into_iter().map(|b| (b.name, b.ty));
        self.binding_overlay.borrow_mut().extend(named);
        let r = self.ctype(&arm.body);
        self.binding_overlay.borrow_mut().truncate(depth);
        r
    }

    /// The `CTy` of the `match`-arm binding `name`: the overlay `ctype_match` installs while it types
    /// an arm (innermost), then the bindings `compile_match` registered while compiling one.
    pub(in crate::compiler) fn binding_cty(&self, name: &str) -> Option<CTy> {
        let overlay = self.binding_overlay.borrow();
        if let Some((_, t)) = overlay.iter().rev().find(|(n, _)| n == name) {
            return Some(t.clone());
        }
        let b = self.match_bindings.iter().rev().find(|b| b.name == name);
        b.map(|b| b.ty.clone())
    }

    /// The bindings `pat` introduces, each with the class-aware type of the value it binds: the one
    /// walker behind compiling an arm (`register_bindings`) and typing one (`ctype_arm`).
    fn collect_bindings(
        &self,
        pat: &Pattern,
        m_slot: usize,
        path: &[PathSeg],
        cur_ty: CTy,
        out: &mut Vec<MatchBinding>,
    ) -> Result<(), String> {
        match pat {
            Pattern::Binding { name, .. } => out.push(MatchBinding {
                name: name.clone(),
                match_slot: m_slot,
                path: path.to_vec(),
                ty: cur_ty,
            }),
            Pattern::Variant {
                name,
                fields,
                enum_qualifier,
                ..
            } => {
                let field_tags = self
                    .variants
                    .get(enum_qualifier.as_deref(), name)
                    .ok_or_else(|| format!("unknown variant `{name}`"))?
                    .field_tags
                    .clone();
                for (i, fp) in fields.iter().enumerate() {
                    let mut sub = path.to_vec();
                    sub.push(PathSeg::Enum(i));
                    let ty = field_tags.get(i).cloned().unwrap_or(CTy::Other);
                    self.collect_bindings(fp, m_slot, &sub, ty, out)?;
                }
            }
            // S5.2 struct pattern: each field binds (or sub-binds) at `path + Field(field)`. The
            // field's CTy comes from the program-wide class-field table so a struct-bound int is a
            // first-class arithmetic operand on the VM (`Point { x } => x + 1`), the operand trap.
            Pattern::Struct {
                type_name, fields, ..
            } => {
                for fp in fields {
                    let idx = self.field_name_index(&fp.field)?;
                    let mut sub = path.to_vec();
                    sub.push(PathSeg::Field(idx));
                    let ty = self
                        .class_field_ctys
                        .get(type_name)
                        .and_then(|m| m.get(&fp.field))
                        .cloned()
                        .unwrap_or(CTy::Other);
                    self.collect_bindings(&fp.pat, m_slot, &sub, ty, out)?;
                }
            }
            // M-RT S4 type pattern: bind the matched value (the whole sub-value at `path`) as the
            // narrowed class, so `binding.field` resolves (the Wave-4 class-aware operand path).
            Pattern::Type {
                type_name,
                binding: Some(name),
                ..
            } => out.push(MatchBinding {
                name: name.clone(),
                match_slot: m_slot,
                path: path.to_vec(),
                // A PRIMITIVE type-pattern (`int i =>`) binds a first-class arithmetic operand, so its
                // `CTy` must be `Int`/`Float`/`Str`/… — not `Class("int")`, which read as a non-numeric
                // operand and made `match x { int i => i * 2 }` compile-fail on the VM while the
                // interpreter/PHP ran it (a shipped interp ≡ VM divergence — the CTy-operand trap).
                ty: cty_of_type_name(type_name),
            }),
            _ => {} // wildcard / literals / `Type _` bind nothing
        }
        Ok(())
    }
}

/// The `CTy` of a value chosen among alternative arms — a `match`'s non-throwing arms (R9-K1) or an
/// un-narrowed `if`'s non-throwing branches (IF-CLASS-ARM-TYPE): the first arm's, unless two arms are
/// incompatible, or a class arm sits beside an unknown one, which give `Other`. The checker unifies
/// `Kid` and `Par` (or `Kid` and an erased generic call returning `Par`) to the wider class, so taking
/// one arm's class would compile an exact-class `with` or hook for a value of another class. An unknown
/// beside a scalar still votes for nothing (`1` beside `f()` keeps `Int`), and a first arm whose
/// `ctype` errors keeps the error, as before.
pub(in crate::compiler) fn join_arm_ctys(
    arms: impl IntoIterator<Item = Result<CTy, String>>,
) -> Result<CTy, String> {
    let mut first: Option<Result<CTy, String>> = None;
    let mut seen: Vec<CTy> = Vec::new();
    for t in arms {
        if let Ok(ty) = &t {
            let class_beside_unknown = |a: &CTy, b: &CTy| {
                matches!(
                    (a, b),
                    (CTy::Class(_), CTy::Other) | (CTy::Other, CTy::Class(_))
                )
            };
            if seen
                .iter()
                .any(|s| !cty_compatible(s, ty) || class_beside_unknown(s, ty))
            {
                return Ok(CTy::Other);
            }
            seen.push(ty.clone());
        }
        first.get_or_insert(t);
    }
    first.unwrap_or(Ok(CTy::Other))
}

/// Whether two arm types can describe the same value: equal, or unknown (`Other`) on one side at any
/// depth — an unknown votes for nothing, so `[1]` beside `[]` keeps `List(Int)`.
fn cty_compatible(a: &CTy, b: &CTy) -> bool {
    let all = |x: &[CTy], y: &[CTy]| {
        x.len() == y.len() && x.iter().zip(y).all(|(p, q)| cty_compatible(p, q))
    };
    match (a, b) {
        (CTy::Other, _) | (_, CTy::Other) => true,
        (CTy::List(x), CTy::List(y)) => cty_compatible(x, y),
        (CTy::Map(k1, v1), CTy::Map(k2, v2)) => cty_compatible(k1, k2) && cty_compatible(v1, v2),
        (CTy::Tuple(x), CTy::Tuple(y)) => all(x, y),
        (
            CTy::Fn {
                params: p1,
                ret: r1,
            },
            CTy::Fn {
                params: p2,
                ret: r2,
            },
        ) => all(p1, p2) && cty_compatible(r1, r2),
        _ => a == b,
    }
}
