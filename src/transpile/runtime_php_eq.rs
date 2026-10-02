//! `==` / `!=` emission and its PHP runtime helper `__phorj_eq` (DEC-557).
//!
//! PHP's loose `==` juggles numeric strings (`"10" == "10.0"` is `true`), while Phorj's `==` on a
//! `string` is exact byte equality (`Value::eq_val`, `src/value/core_impl.rs`) — so a bare `==` in the
//! transpiled PHP made the PHP leg disagree with the VM and the tree-walker (Invariant 1) on any two
//! numeric-looking strings, silently (Invariant 14, tier 3).
//!
//! The emission is chosen from the operands' statically resolved [`OpKind`]s, in this order:
//! 1. **A kind that contains a `decimal`** — at any depth, through an optional, a generic instantiation's
//!    type arguments, a class/enum's fields or payloads, and every subtype of an interface- or base-typed
//!    operand (`kind_has_decimal`) — keeps PHP's loose `==`. A decimal is a PHP *string* carrier and
//!    Phorj's decimal equality is numeric and scale-insensitive (`1.50d == 1.5d`), which loose `==`
//!    reproduces and a strict compare would break. Residual, disclosed in KNOWN_ISSUES: a value that
//!    mixes `string` and `decimal` keeps the loose string behaviour.
//! 2. **Either side a `string`** → PHP `===` / `!==`, with a non-literal operand cast to `(string)`: PHP
//!    turns an integer-like map KEY into an int while its static kind stays `string`
//!    (KNOWN_ISSUES STRING-EQ-MAP-KEY).
//! 3. **Both sides `int`/`float`/`bool`** → `===` / `!==`. Native equality has no cross-type scalar case
//!    (`Int(1) == Float(1.0)` is `false`, and a union `int | float` can hold either), so PHP's loose
//!    `1 == 1.0` / `1 == true` would disagree; `===` keeps NaN unequal and `-0.0 == 0.0` equal.
//! 4. **Anything else** (`Other` — the bare type parameter of an erased generic body, an unresolved
//!    call/field/index —, lists, maps, tuples, class/enum instances) → `__phorj_eq`, which mirrors `eq_val`
//!    structurally: two strings (or a string and an int key) exactly, a `null` only to `null`, a closure
//!    to nothing (`eq_val` says `false` even for `f == f`), scalars strictly, maps order-independently,
//!    instances field by field with a cycle guard. It does NOT mirror `eq_val` for a `Set` (a PHP list, so
//!    membership equality becomes order-sensitive: KNOWN_ISSUES SET-EQ-ORDER). Inside an erased generic
//!    BODY a `decimal` is indistinguishable from a string, so it compares scale-sensitively on the PHP leg
//!    (developer ruling DEC-557, 2026-10-02; KNOWN_ISSUES STRING-EQ-DECIMAL-ERASED).

use super::*;

/// The helper body, emitted through `self.line` at the runtime block's indentation. `$seen` makes the
/// instance walk cycle-safe, like `eq_val`'s visited-pair set (an unguarded recursion would overflow).
const HELPER: &str = r#"function __phorj_eq($a, $b) {
    static $seen = [];
    if ($a === null || $b === null) { return $a === $b; }
    if (is_string($a) || is_string($b)) {
        if ((is_string($a) || is_int($a)) && (is_string($b) || is_int($b))) { return (string)$a === (string)$b; }
        return $a === $b;
    }
    if ($a instanceof \Closure || $b instanceof \Closure) { return false; }
    if (is_array($a) && is_array($b)) {
        if (count($a) !== count($b)) { return false; }
        foreach ($a as $k => $v) {
            if (!array_key_exists($k, $b) || !__phorj_eq($v, $b[$k])) { return false; }
        }
        return true;
    }
    if (is_object($a) && is_object($b)) {
        if (get_class($a) !== get_class($b)) { return false; }
        $pair = spl_object_id($a) . ':' . spl_object_id($b);
        if (isset($seen[$pair])) { return true; }
        $seen[$pair] = true;
        try { return __phorj_eq((array)$a, (array)$b); } finally { unset($seen[$pair]); }
    }
    return $a === $b;
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
            OpKind::Tuple(ks) | OpKind::Wrapped(ks) => {
                ks.iter().any(|k| self.kind_has_decimal(k, seen))
            }
            OpKind::Class(name) => {
                if seen.contains(name) {
                    return false;
                }
                seen.push(name.clone());
                // An ENUM's payload kinds live in `variant_field_kinds`, not `class_field_kinds`: without
                // this arm an enum with a `decimal` payload looked decimal-free and took the strict helper.
                if self.enums.contains(name)
                    && self.variant_field_kinds.iter().any(|((e, _), ks)| {
                        e == name && ks.iter().any(|k| self.kind_has_decimal(k, seen))
                    })
                {
                    return true;
                }
                // An interface- or base-typed operand can hold ANY subtype, so a decimal in any
                // implementer or subclass counts (the checker allows `Shape s1 == Shape s2`).
                if self.class_subtypes.get(name).is_some_and(|subs| {
                    subs.iter()
                        .any(|c| self.kind_has_decimal(&OpKind::Class(c.clone()), seen))
                }) {
                    return true;
                }
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
        } else if lk == OpKind::Str || rk == OpKind::Str || (scalar(&lk) && scalar(&rk)) {
            Some(strict)
        } else {
            None
        };
        let (pl, pr) = (
            Self::paren_if_compound(lhs, l.clone()),
            Self::paren_if_compound(rhs, r.clone()),
        );
        // PHP arrays turn an integer-like string KEY into an int, so a key read back from
        // `Map.keys` / a `for (string k in m)` is `int(10)` while its static kind is `string`
        // (KNOWN_ISSUES "Map key coercion"). The old loose `10 == "10"` hid that; a bare `===` would
        // expose it. A non-literal string operand is therefore cast, which is free for a real string.
        let norm = |e: &Expr, code: String| {
            if op == Some(strict) && !matches!(e, Expr::Str(..)) && self.expr_kind(e) == OpKind::Str
            {
                format!("(string){code}")
            } else {
                code
            }
        };
        let (pl, pr) = (norm(lhs, pl), norm(rhs, pr));
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
