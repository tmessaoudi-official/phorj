//! `Core.Map.union` (DEC-534, scout row 5q) — PHP's array union `$a + $b` as a named native, so `+`
//! stays arithmetic-only. Split out of `map.rs`, which is already past the soft cap (Invariant 13).
use super::*;
use crate::types::Ty;
use crate::value::Value;

/// `union(a, b) -> Map<K,V>` — `a`'s entries in order, then `b`'s entries whose key `a` lacks. On a
/// shared key the LEFT value wins — the mirror of [`Map.merge`](super::map), and exactly PHP `+`.
fn map_union(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Map(a), Value::Map(b)] => {
            // A linear key scan, the idiom `build_map` and `merge` use: `HKey` holds a `PhStr`, whose
            // cached hash is interior-mutable, so it is not hashed as a set key (clippy
            // `mutable_key_type`).
            let mut out = (**a).clone();
            out.extend(
                b.iter()
                    .filter(|(k, _)| !a.iter().any(|(ak, _)| ak == k))
                    .map(|(k, v)| (k.clone(), v.clone())),
            );
            Ok(Value::Map(std::rc::Rc::new(out)))
        }
        _ => Err("Map.union expects (Map<K, V>, Map<K, V>)".into()),
    }
}

pub(crate) fn map_union_natives() -> Vec<NativeFn> {
    let map = || {
        Ty::Map(
            Box::new(Ty::Param("K".into())),
            Box::new(Ty::Param("V".into())),
        )
    };
    vec![NativeFn {
        module: "Core.Map",
        name: "union",
        params: vec![map(), map()],
        ret: map(),
        pure: true,
        eval: NativeEval::Pure(map_union),
        // PHP `+` is an operator, not a function, so there is no `lift_from` name: the lifter maps
        // the operator itself, and only where it knows both operands are maps (DEC-534).
        lift_from: &[],
        // Ladder case 1: PHP's own `+` IS this operation (left wins, left order, then right's new keys,
        // int keys kept). Parenthesised so it composes inside any enclosing expression.
        php: |a| format!("({} + {})", parg(a, 0), parg(a, 1)),
    }]
}
