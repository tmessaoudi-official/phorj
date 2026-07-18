use super::*;
use crate::value::{HKey, Value};
use std::cmp::Ordering;

/// Natural total order over the scalar element types, matching the PHP `__phorj_sort` comparator
/// byte-for-byte: ints/floats/bools numerically (Rust `cmp`/`total_cmp` ≡ PHP `<=>`), strings
/// lexicographically by byte (Rust `String` Ord ≡ PHP `strcmp` — NOT PHP's numeric-string-juggling
/// `<=>`). A homogeneous typed list never mixes arms; a stray mix is treated as equal (total, no panic).
pub(super) fn natural_cmp(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.total_cmp(y),
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        _ => Ordering::Equal,
    }
}

/// `List.sort(List<T>) -> List<T>` — a new list in natural ascending order. Rust `sort_by` is stable
/// (≡ PHP 8.0+ `usort`); returns a fresh list (Phorj lists are immutable).
pub(super) fn list_sort(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs)] => {
            let mut ys = (**xs).clone();
            ys.sort_by(natural_cmp);
            Ok(Value::List(std::rc::Rc::new(ys)))
        }
        _ => Err("List.sort expects (List<T>)".into()),
    }
}

/// `List.sortWith(List<T>, (T, T) -> int) -> List<T>` — a new list ordered by the comparator (negative
/// ⇒ a before b, like PHP `usort`). The comparator runs on the calling backend via the re-entrant
/// invoker; a fault (or a non-int result) is captured and propagated rather than panicking the sort.
pub(super) fn list_sort_with(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            let mut ys = (**xs).clone();
            let mut err: Option<String> = None;
            ys.sort_by(|a, b| {
                if err.is_some() {
                    return Ordering::Equal;
                }
                match call(f, vec![a.clone(), b.clone()]) {
                    Ok(Value::Int(n)) => n.cmp(&0),
                    Ok(_) => {
                        err = Some("List.sortWith comparator must return int".into());
                        Ordering::Equal
                    }
                    Err(e) => {
                        err = Some(e);
                        Ordering::Equal
                    }
                }
            });
            match err {
                Some(e) => Err(e),
                None => Ok(Value::List(std::rc::Rc::new(ys))),
            }
        }
        _ => Err("List.sortWith expects (List<T>, (T, T) -> int)".into()),
    }
}

pub(super) fn list_reverse(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs)] => {
            let mut v = (**xs).clone();
            v.reverse();
            Ok(Value::List(std::rc::Rc::new(v)))
        }
        _ => Err("List.reverse expects (List<T>)".into()),
    }
}
/// `zip(a, b) -> List<(A, B)>` (DEC-288) — pair up elements positionally, length = `min(|a|, |b|)`
/// (the extra tail of the longer list is dropped). Each pair is an erased 2-tuple (a runtime 2-list),
/// ready for `for ((x, y) in List.zip(a, b))`.
pub(super) fn list_zip(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(a), Value::List(b)] => {
            let n = a.len().min(b.len());
            let out: Vec<Value> = (0..n)
                .map(|i| Value::List(std::rc::Rc::new(vec![a[i].clone(), b[i].clone()])))
                .collect();
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.zip expects (List<A>, List<B>)".into()),
    }
}
/// `partition(xs, pred) -> (List<T>, List<T>)` (DEC-288) — split into `(matching, non-matching)`,
/// each preserving order. Returns an erased 2-tuple (a runtime 2-list of the two sublists), destructured
/// as `var (yes, no) = List.partition(xs, pred)`.
pub(super) fn list_partition(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            let mut yes = Vec::new();
            let mut no = Vec::new();
            for x in xs.iter() {
                match call(f, vec![x.clone()])? {
                    Value::Bool(true) => yes.push(x.clone()),
                    Value::Bool(false) => no.push(x.clone()),
                    other => {
                        return Err(format!(
                            "List.partition predicate must return bool, got {}",
                            other.type_name()
                        ))
                    }
                }
            }
            Ok(Value::List(std::rc::Rc::new(vec![
                Value::List(std::rc::Rc::new(yes)),
                Value::List(std::rc::Rc::new(no)),
            ])))
        }
        _ => Err("List.partition expects (List<T>, (T) -> bool)".into()),
    }
}
/// `enumerate(xs) -> Map<int, T>` — pair each element with its 0-based index, ready for the
/// two-binding `for (int i, T x in List.enumerate(xs))` form (B1). Insertion-ordered, so iteration
/// is index order. A PHP list array is already 0-keyed, so this erases to `array_values` (identity).
pub(super) fn list_enumerate(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs)] => {
            let pairs: Vec<(crate::value::HKey, Value)> = xs
                .iter()
                .enumerate()
                .map(|(i, v)| (crate::value::HKey::Int(i as i64), v.clone()))
                .collect();
            Ok(Value::Map(std::rc::Rc::new(pairs)))
        }
        _ => Err("List.enumerate expects (List<T>)".into()),
    }
}
/// `fill(value, count) -> List<T>` — a list of `count` copies of `value` (PHP `array_fill(0, …)`;
/// cf. JS `Array(n).fill(v)`, Dart `List.filled`). `count == 0` is the empty list; a negative count
/// faults cleanly (PHP `array_fill` `ValueError`, EV-7 — never an over-large alloc from `n as usize`).
/// Generic: the element type is inferred from `value` at the call site. Named `fill` (not `repeat`) so
/// its leaf does not collide with `Text.repeat` under UFCS (a generic-subject native matches every
/// receiver, so a shared leaf would make `x.repeat(n)` ambiguous).
pub(super) fn list_fill(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [value, Value::Int(n)] => {
            if *n < 0 {
                return Err("List.fill count must be >= 0".into());
            }
            Ok(Value::List(std::rc::Rc::new(vec![
                value.clone();
                *n as usize
            ])))
        }
        _ => Err("List.fill expects (T, int)".into()),
    }
}
pub(super) fn list_length(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        // Generic over the element type — the count of any list, byte-identical to PHP `count`.
        [Value::List(xs)] => Ok(Value::Int(xs.len() as i64)),
        _ => Err("List.length expects (List<T>)".into()),
    }
}
pub(super) fn list_contains(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs), needle] => Ok(Value::Bool(xs.iter().any(|x| x.eq_val(needle)))),
        _ => Err("List.contains expects (List<T>, T)".into()),
    }
}
/// `slice(List<T>, int, int) -> List<T>` — a sub-list, mirroring PHP `array_slice($xs, offset, len)`
/// EXACTLY (so the erasure is the bare builtin): a negative `offset`/`len` counts from the end, an
/// out-of-range slice clamps to empty. Returns a fresh (re-indexed) list.
pub(super) fn list_slice(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs), Value::Int(offset), Value::Int(length)] => {
            let n = xs.len() as i64;
            // PHP `array_slice` offset/length normalization, replicated for byte-identity.
            let start = if *offset < 0 {
                (n + *offset).max(0)
            } else {
                (*offset).min(n)
            };
            let end = if *length < 0 {
                (n + *length).max(start)
            } else {
                (start + *length).min(n)
            };
            let out: Vec<Value> = xs[start as usize..end as usize].to_vec();
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.slice expects (List<T>, int, int)".into()),
    }
}
// `take`/`drop` clamp `n` to `[0, len]` (n<0 ⇒ 0, n>len ⇒ len), so they never fault. PHP
// `array_slice` (which reindexes by default) reproduces both with `max(0, n)` (a negative `n` must be
// clamped, else array_slice would count from the end).
pub(super) fn list_take(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs), Value::Int(n)] => {
            let k = (*n).clamp(0, xs.len() as i64) as usize;
            Ok(Value::List(std::rc::Rc::new(xs[..k].to_vec())))
        }
        _ => Err("List.take expects (List<T>, int)".into()),
    }
}
pub(super) fn list_drop(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs), Value::Int(n)] => {
            let k = (*n).clamp(0, xs.len() as i64) as usize;
            Ok(Value::List(std::rc::Rc::new(xs[k..].to_vec())))
        }
        _ => Err("List.drop expects (List<T>, int)".into()),
    }
}
// `chunk(List<T>, int) -> List<List<T>>` — split into consecutive groups of `size`; the last group
// may be shorter. `size < 1` is a programmer error (charter: fault, not `T?`) — byte-identical
// `"List.chunk size must be at least 1"` on both backends; PHP `array_chunk` likewise throws on
// size < 1 (a fault-domain case, excluded from the example oracle). The Ok path mirrors PHP
// `array_chunk` (re-indexed groups), so an empty list yields `[]`.
pub(super) fn list_chunk(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs), Value::Int(n)] => {
            if *n < 1 {
                return Err("List.chunk size must be at least 1".into());
            }
            let size = *n as usize;
            let groups: Vec<Value> = xs
                .chunks(size)
                .map(|g| Value::List(std::rc::Rc::new(g.to_vec())))
                .collect();
            Ok(Value::List(std::rc::Rc::new(groups)))
        }
        _ => Err("List.chunk expects (List<T>, int)".into()),
    }
}
/// `indexOf(List<T>, T) -> int?` — the index of the first element equal to the needle (structural
/// `eq_val`, like `contains`), else `null`. Erases to a gated `__phorj_index_of` (PHP `array_search`
/// returns `false` on miss, mapped to `null`).
pub(super) fn list_index_of(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs), needle] => Ok(xs
            .iter()
            .position(|x| x.eq_val(needle))
            .map_or(Value::Null, |i| Value::Int(i as i64))),
        _ => Err("List.indexOf expects (List<T>, T)".into()),
    }
}
/// `lastIndexOf(List<T>, T) -> int?` — the index of the LAST element equal to the needle (structural
/// `eq_val`, like `indexOf`/`contains`), else `null`. The symmetric companion to `indexOf` (mirrors
/// `Core.Text.lastIndexOf`). Erases to a gated `__phorj_last_index_of` (PHP `array_keys($xs, $needle,
/// true)` → last key, or `null` when none match).
pub(super) fn list_last_index_of(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs), needle] => Ok(xs
            .iter()
            .rposition(|x| x.eq_val(needle))
            .map_or(Value::Null, |i| Value::Int(i as i64))),
        _ => Err("List.lastIndexOf expects (List<T>, T)".into()),
    }
}
/// `concat(List<T>, List<T>) -> List<T>` — the two lists joined (PHP `array_merge`, which re-indexes
/// sequential lists). A fresh list; both inputs are untouched (immutability).
pub(super) fn list_concat(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(a), Value::List(b)] => {
            let mut out = (**a).clone();
            out.extend(b.iter().cloned());
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.concat expects (List<T>, List<T>)".into()),
    }
}

/// `append(List<T>, T) -> List<T>` — a new list with `v` added at the end. Lists are immutable
/// (COW), so this returns a fresh list; for building in a hot loop prefer `List.fill` + index-set
/// (O(1) per write since M-DOGFOOD W8) or `List.map(range, fn)`. Erases to PHP `array_merge($xs, [$v])`.
pub(super) fn list_append(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs), v] => {
            let mut out = (**xs).clone();
            out.push(v.clone());
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.append expects (List<T>, T)".into()),
    }
}
/// `first(List<T>) -> T?` / `last(List<T>) -> T?` — the first/last element, or `null` for an empty
/// list. Erase inline to `($xs[0] ?? null)` / `($xs[count($xs) - 1] ?? null)`.
pub(super) fn list_first(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs)] => Ok(xs.first().cloned().unwrap_or(Value::Null)),
        _ => Err("List.first expects (List<T>)".into()),
    }
}
pub(super) fn list_last(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs)] => Ok(xs.last().cloned().unwrap_or(Value::Null)),
        _ => Err("List.last expects (List<T>)".into()),
    }
}

pub(super) fn list_sum(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs)] => {
            let mut acc: i64 = 0;
            for x in xs.iter() {
                match x {
                    // Checked: an overflowing sum faults cleanly (EV-7), like the int arithmetic
                    // kernels. PHP `array_sum` would instead promote to float on overflow — examples
                    // stay well within i64 range (caveat in KNOWN_ISSUES).
                    Value::Int(n) => {
                        acc = acc
                            .checked_add(*n)
                            .ok_or_else(|| "integer overflow in List.sum".to_string())?;
                    }
                    other => {
                        return Err(format!(
                            "List.sum expects List<int>, found element of type {}",
                            other.type_name()
                        ))
                    }
                }
            }
            Ok(Value::Int(acc))
        }
        _ => Err("List.sum expects (List<int>)".into()),
    }
}

// The higher-order `Core.List` ops (M-RT S7b-3). Each takes a `Value::Closure` argument and calls it
// once per element via the backend-supplied `call` invoker ([`ClosureInvoker`]) — so the one body
// runs on the interpreter *and* the VM (parity), and any fault the closure raises propagates as a
// plain `String` that both backends classify identically. The element type `T` (and `map`/`reduce`'s
// result type `U`) are inferred at the call site by the generic-native path; the registry's
// `Ty::Param` never reaches a backend (M-RT S7b). They erase to PHP's `array_map`/`array_filter`/
// `array_reduce` (D-L9). `filter` wraps `array_filter` in `array_values` to re-index the result to a
// sequential list (PHP's `array_filter` preserves the original keys), matching the Rust `Vec`.

pub(super) fn list_map(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs.iter() {
                out.push(call(f, vec![x.clone()])?);
            }
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.map expects (List<T>, (T) -> U)".into()),
    }
}
pub(super) fn list_filter(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            let mut out = Vec::new();
            for x in xs.iter() {
                match call(f, vec![x.clone()])? {
                    Value::Bool(true) => out.push(x.clone()),
                    Value::Bool(false) => {}
                    other => {
                        return Err(format!(
                            "List.filter predicate must return bool, got {}",
                            other.type_name()
                        ))
                    }
                }
            }
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.filter expects (List<T>, (T) -> bool)".into()),
    }
}
pub(super) fn list_reduce(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), init, f] => {
            let mut acc = init.clone();
            for x in xs.iter() {
                acc = call(f, vec![acc, x.clone()])?;
            }
            Ok(acc)
        }
        _ => Err("List.reduce expects (List<T>, U, (U, T) -> U)".into()),
    }
}

/// The `Core.List` registry entries (M-RT S7b). `reverse` is generic over the element type; `sum` is
/// concrete `List<int> -> int`; `map`/`filter`/`reduce` are the higher-order ops (S7b-3). All erase
/// to the PHP array builtin of the same shape (D-L9).
pub(super) fn list_is_empty(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs)] => Ok(Value::Bool(xs.is_empty())),
        _ => Err("List.isEmpty expects (List<T>)".into()),
    }
}

/// `List.flatten(List<List<T>>) -> List<T>` — concatenate the inner lists in order (PHP
/// `array_merge(...)`). A non-list element is a type error the checker prevents; defensively ignored.
pub(super) fn list_flatten(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs)] => {
            let mut out = Vec::new();
            for x in xs.iter() {
                if let Value::List(inner) = x {
                    out.extend(inner.iter().cloned());
                }
            }
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.flatten expects (List<List<T>>)".into()),
    }
}

/// `List.flatMap(List<T>, (T) -> List<U>) -> List<U>` — map each element to a list, then concatenate
/// the results (map + one-level flatten, the universal `flatMap`/`concatMap`). The mapper runs on the
/// calling backend via the re-entrant invoker; a non-list result is checker-unreachable (the callback's
/// declared return type is `List<U>`) but faults cleanly rather than panicking (EV-7).
pub(super) fn list_flat_map(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            let mut out = Vec::new();
            for x in xs.iter() {
                match call(f, vec![x.clone()])? {
                    Value::List(inner) => out.extend(inner.iter().cloned()),
                    other => {
                        return Err(format!(
                            "List.flatMap mapper must return List, got {}",
                            other.type_name()
                        ))
                    }
                }
            }
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.flatMap expects (List<T>, (T) -> List<U>)".into()),
    }
}

/// `List.takeWhile(List<T>, (T) -> bool) -> List<T>` — the longest PREFIX whose elements all satisfy
/// the predicate; stops (does not scan further) at the first element that fails. The predicate runs on
/// the calling backend via the re-entrant invoker; a non-bool result faults cleanly (EV-7).
pub(super) fn list_take_while(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            let mut out = Vec::new();
            for x in xs.iter() {
                match call(f, vec![x.clone()])? {
                    Value::Bool(true) => out.push(x.clone()),
                    Value::Bool(false) => break,
                    other => {
                        return Err(format!(
                            "List.takeWhile predicate must return bool, got {}",
                            other.type_name()
                        ))
                    }
                }
            }
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.takeWhile expects (List<T>, (T) -> bool)".into()),
    }
}

/// `List.dropWhile(List<T>, (T) -> bool) -> List<T>` — the SUFFIX after the longest prefix whose
/// elements all satisfy the predicate; the predicate stops running once the first failing element is
/// seen (that element and all after it are kept verbatim). A non-bool result faults cleanly (EV-7).
pub(super) fn list_drop_while(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            let mut out = Vec::new();
            let mut dropping = true;
            for x in xs.iter() {
                if dropping {
                    match call(f, vec![x.clone()])? {
                        Value::Bool(true) => continue,
                        Value::Bool(false) => dropping = false,
                        other => {
                            return Err(format!(
                                "List.dropWhile predicate must return bool, got {}",
                                other.type_name()
                            ))
                        }
                    }
                }
                out.push(x.clone());
            }
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.dropWhile expects (List<T>, (T) -> bool)".into()),
    }
}

/// `List.groupBy(List<T>, (T) -> U) -> Map<U, List<T>>` — partition the list into groups keyed by the
/// selector's result, preserving FIRST-SEEN key order and each group's element order (the universal
/// Kotlin/Swift/LINQ `groupBy`). The selector runs on the calling backend via the re-entrant invoker;
/// a non-hashable key faults cleanly (EV-7) — the checker constrains `U` to the hashable subset.
pub(super) fn list_group_by(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            let mut groups: Vec<(HKey, Vec<Value>)> = Vec::new();
            for x in xs.iter() {
                let kv = call(f, vec![x.clone()])?;
                let key = HKey::from_value(&kv).ok_or_else(|| {
                    format!("List.groupBy key must be hashable, got {}", kv.type_name())
                })?;
                if let Some(slot) = groups.iter_mut().find(|(gk, _)| *gk == key) {
                    slot.1.push(x.clone());
                } else {
                    groups.push((key, vec![x.clone()]));
                }
            }
            let entries: Vec<(HKey, Value)> = groups
                .into_iter()
                .map(|(k, vs)| (k, Value::List(std::rc::Rc::new(vs))))
                .collect();
            Ok(Value::Map(std::rc::Rc::new(entries)))
        }
        _ => Err("List.groupBy expects (List<T>, (T) -> U)".into()),
    }
}

/// `List.count(List<T>, (T) -> bool) -> int` — how many elements satisfy the predicate. The predicate
/// runs on the calling backend via the re-entrant invoker; a fault (or non-bool result) is propagated.
pub(super) fn list_count(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            let mut n: i64 = 0;
            for x in xs.iter() {
                match call(f, vec![x.clone()])? {
                    Value::Bool(true) => n += 1,
                    Value::Bool(false) => {}
                    _ => return Err("List.count predicate must return bool".into()),
                }
            }
            Ok(Value::Int(n))
        }
        _ => Err("List.count expects (List<T>, (T) -> bool)".into()),
    }
}
