//! `==` / `!=` emission and its PHP runtime helper `__phorj_eq` (DEC-557).
//!
//! PHP's loose `==` juggles numeric strings (`"10" == "10.0"` is `true`), while Phorj's `==` on a
//! `string` is exact byte equality (`Value::eq_val`, `src/value/core_impl.rs`) — so a bare `==` in the
//! transpiled PHP made the PHP leg disagree with the VM and the tree-walker (Invariant 1) on any two
//! numeric-looking strings, silently (Invariant 14, tier 3).
//!
//! The emission is chosen from the operands' statically resolved [`OpKind`]s, in this order:
//! 1. **A kind that contains a `decimal`** keeps PHP's loose `==`. A decimal is a PHP *string* carrier
//!    and Phorj's decimal equality is numeric and scale-insensitive (`1.50d == 1.5d`), which loose `==`
//!    reproduces and a strict compare would break. Known residual, disclosed in KNOWN_ISSUES: a value
//!    that mixes `string` and `decimal` in one container/class keeps the loose string behaviour.
//! 2. **Either side a `string`** → PHP `===` / `!==`.
//! 3. **Both sides `int`/`float`/`bool`** → bare `==` / `!=` (same type, so identical to `===`).
//! 4. **Anything else** (`Other` — an erased generic `T`, an unresolved call/field/index —, lists, maps,
//!    tuples, class/enum instances) → `__phorj_eq`, which mirrors `eq_val` structurally and compares any
//!    two strings exactly. Under erasure a `decimal` flowing through an UNKNOWN-kind operand therefore
//!    compares scale-sensitively on the PHP leg (developer ruling DEC-557, 2026-10-02).

use super::*;

/// The helper body, emitted through `self.line` at the runtime block's indentation. `$seen` makes the
/// instance walk cycle-safe, like `eq_val`'s visited-pair set (an unguarded recursion would overflow).
const HELPER: &str = r#"function __phorj_eq($a, $b) {
    static $seen = [];
    if ($a === null || $b === null || is_string($a) || is_string($b)) { return $a === $b; }
    if (is_array($a) && is_array($b)) {
        if (count($a) !== count($b)) { return false; }
        foreach ($a as $k => $v) {
            if (!array_key_exists($k, $b) || !__phorj_eq($v, $b[$k])) { return false; }
        }
        return true;
    }
    if (is_object($a) && is_object($b)) {
        if (get_class($a) !== get_class($b)) { return false; }
        foreach ($seen as $p) { if ($p[0] === $a && $p[1] === $b) { return true; } }
        $seen[] = [$a, $b];
        try { return __phorj_eq((array)$a, (array)$b); } finally { array_pop($seen); }
    }
    return $a == $b;
}"#;

impl Transpiler {
    /// Emit the `__phorj_eq` helper (called from `emit_runtime_helpers` under `uses_eq`).
    pub(super) fn emit_eq_helper(&mut self) {
        for l in HELPER.lines() {
            self.line(l);
        }
    }

    /// Does a kind carry a `decimal` anywhere inside it? `seen` guards a self-referential class.
    fn kind_has_decimal(&self, k: &OpKind, seen: &mut Vec<String>) -> bool {
        match k {
            OpKind::Decimal => true,
            OpKind::List(e) => self.kind_has_decimal(e, seen),
            OpKind::Map(a, b) => self.kind_has_decimal(a, seen) || self.kind_has_decimal(b, seen),
            OpKind::Tuple(ks) => ks.iter().any(|k| self.kind_has_decimal(k, seen)),
            OpKind::Class(name) => {
                if seen.contains(name) {
                    return false;
                }
                seen.push(name.clone());
                let own = self
                    .class_field_kinds
                    .get(name)
                    .is_some_and(|m| m.values().any(|k| self.kind_has_decimal(k, seen)));
                own || self.class_parents.get(name).is_some_and(|ps| {
                    ps.iter()
                        .any(|p| self.kind_has_decimal(&OpKind::Class(p.clone()), seen))
                })
            }
            _ => false,
        }
    }

    /// The PHP text for `lhs == rhs` / `lhs != rhs` (`negate`), given the already-emitted operand code.
    pub(super) fn emit_eq(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
        l: String,
        r: String,
        negate: bool,
    ) -> String {
        let (lk, rk) = (self.expr_kind(lhs), self.expr_kind(rhs));
        let scalar = |k: &OpKind| matches!(k, OpKind::Int | OpKind::Float | OpKind::Bool);
        let (loose, strict) = if negate { ("!=", "!==") } else { ("==", "===") };
        let op = if self.kind_has_decimal(&lk, &mut Vec::new())
            || self.kind_has_decimal(&rk, &mut Vec::new())
        {
            Some(loose)
        } else if lk == OpKind::Str || rk == OpKind::Str {
            Some(strict)
        } else if scalar(&lk) && scalar(&rk) {
            Some(loose)
        } else {
            None
        };
        let (pl, pr) = (
            Self::paren_if_compound(lhs, l.clone()),
            Self::paren_if_compound(rhs, r.clone()),
        );
        match op {
            Some(op) => format!("{pl} {op} {pr}"),
            None => {
                self.gates.uses_eq = true;
                let bs = if self.namespaced { "\\" } else { "" };
                let bang = if negate { "!" } else { "" };
                format!("{bang}{bs}__phorj_eq({l}, {r})")
            }
        }
    }
}
