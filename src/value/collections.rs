//! Collection kernels (single-sourced, run ≡ runvm): map/set building + dedup, indexing,
//! COW element sets, ranges, and the canonical ordering compare.

use super::*;

/// The element sequence a single-binding `for (x in iter)` walks (B1 iteration protocol),
/// single-sourced so the interpreter and the VM (`Op::IterElems`) materialize the SAME ordered list
/// ⇒ byte-identical iteration. A `List` passes through; a `Set` yields its insertion-ordered elements
/// as values (matching the PHP-array `foreach` the transpiler emits). Any other value is a clean fault.
///
/// # Errors
/// `Err` with a `"cannot iterate over <type>"` body when `v` is not an iterable collection.
pub fn iter_elements(v: &Value) -> Result<Vec<Value>, String> {
    match v {
        Value::List(items) => Ok((**items).clone()),
        Value::Set(elems) => Ok(elems.iter().map(HKey::to_value).collect()),
        // B1: a `string` iterates its characters as 1-char strings (ASCII-domain like the rest of the
        // String stdlib — Unicode scalars; the transpiler emits PHP `str_split`, byte-identical for
        // ASCII). An empty string yields no elements (matches PHP 8 `str_split("")` == []).
        Value::Str(s) => Ok(s
            .chars()
            .map(|c| Value::Str(PhStr::new(c.encode_utf8(&mut [0u8; 4]))))
            .collect()),
        // B1: a `Map<K, V>` iterates as `[key, value]` 2-element lists in insertion order — the
        // two-binding `for (k, v in map)` form destructures each pair (the VM indexes [0]/[1], the
        // interpreter unpacks below). Single-sourced so run≡runvm.
        Value::Map(entries) => Ok(entries
            .iter()
            .map(|(k, v)| Value::List(Rc::new(vec![k.to_value(), v.clone()])))
            .collect()),
        other => Err(format!("cannot iterate over {}", other.type_name())),
    }
}

/// Build an **insertion-ordered** map from evaluated `(key, value)` pairs, matching PHP literal
/// semantics: a duplicate key keeps its **first position** but takes the **last value**
/// (`["a" => 1, "a" => 2]` ⇒ `["a" => 2]`, position of the first `"a"`). Single-sourced so the
/// interpreter (`Expr::Map`) and the VM (`Op::MakeMap`) dedup identically — `run ≡ runvm` (and a
/// non-`HKey` key, checker-unreachable, faults cleanly rather than panicking, EV-7).
pub fn build_map(pairs: Vec<(Value, Value)>) -> Result<Vec<(HKey, Value)>, String> {
    let mut out: Vec<(HKey, Value)> = Vec::with_capacity(pairs.len());
    for (k, v) in pairs {
        let key =
            HKey::from_value(&k).ok_or_else(|| format!("invalid map key: {}", k.type_name()))?;
        if let Some(slot) = out.iter_mut().find(|(ek, _)| *ek == key) {
            slot.1 = v; // existing key: keep first position, take last value (PHP semantics)
        } else {
            out.push((key, v));
        }
    }
    Ok(out)
}

/// Build an **insertion-ordered, deduplicated** set from evaluated element values, keeping each
/// element's **first occurrence** and discarding later duplicates (`Set.of([1, 2, 1]) ⇒ {1, 2}`,
/// in that order) — the same first-seen-order discipline as [`build_map`]'s keys. Single-sourced so
/// the interpreter and the VM dedup identically (`run ≡ runvm`); a non-`HKey` element
/// (checker-unreachable, the checker constrains a `Set<T>` element to the hashable subset) faults
/// cleanly rather than panicking (EV-7).
pub fn build_set(elems: Vec<Value>) -> Result<Vec<HKey>, String> {
    let mut out: Vec<HKey> = Vec::with_capacity(elems.len());
    for e in elems {
        let key = HKey::from_value(&e)
            .ok_or_else(|| format!("invalid set element: {}", e.type_name()))?;
        if !out.contains(&key) {
            out.push(key);
        }
    }
    Ok(out)
}

/// Look a key up in an insertion-ordered map. A missing key is a clean fault (`"map key not found"`),
/// byte-identical across both backends — the differential harness excludes fault cases, and the
/// present-key path is byte-identical to PHP `$m[$k]`. A non-`HKey` index is checker-unreachable
/// (`m[k]` types `k` against the map's key type) but handled defensively (EV-7).
pub fn map_index(map: &[(HKey, Value)], index: &Value) -> Result<Value, String> {
    let key =
        HKey::from_value(index).ok_or_else(|| format!("invalid map key: {}", index.type_name()))?;
    map.iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.clone())
        .ok_or_else(|| "map key not found".to_string())
}

/// Set `list[idx] = v` in place with bounds-checking (M-mut.5). The caller owns the copy-on-write
/// (`Rc::make_mut` before calling), so this mutates a uniquely-owned `Vec`. An out-of-range index
/// faults identically to a read (`"list index out of range"`, `FaultKind::IndexOob`) — note this
/// diverges from PHP, which would *extend* the array; examples only set in-bounds (KNOWN_ISSUES).
pub fn list_set(list: &mut [Value], idx: i64, v: Value) -> Result<(), String> {
    let i = usize::try_from(idx)
        .ok()
        .filter(|i| *i < list.len())
        .ok_or_else(|| "list index out of range".to_string())?;
    list[i] = v;
    Ok(())
}

/// Set `map[key] = v` (M-mut.5): update in place if `key` is present, else append — insertion-ordered
/// like PHP `$m[$k] = $v`, preserving the `Rc<Vec<(HKey, Value)>>` order invariant (R1). The caller
/// owns the COW. A non-`HKey` key is checker-unreachable (EV-7).
pub fn map_set(map: &mut Vec<(HKey, Value)>, key: &Value, v: Value) -> Result<(), String> {
    let k = HKey::from_value(key).ok_or_else(|| format!("invalid map key: {}", key.type_name()))?;
    if let Some(slot) = map.iter_mut().find(|(ek, _)| *ek == k) {
        slot.1 = v;
    } else {
        map.push((k, v));
    }
    Ok(())
}

/// Set a nested element `container[i0][i1]…[ik] = v` in place (Spec nested-value-index-assign). COW is
/// applied `Rc::make_mut` **root-to-leaf**: after the outer `make_mut` the inner `Rc` is uniquely held,
/// so descent mutates in place; a genuinely-shared level still copies (value semantics preserved). The
/// caller owns the outermost COW (it mutates `container` in its slot). `indices` is non-empty; a
/// single index is the flat `xs[i]=e` case. A bad index/key faults exactly like a read (`"list index
/// out of range"` / `"map key not found"`). **Single-sourced so `run ≡ runvm`** — both backends call it.
pub fn set_nested(container: &mut Value, indices: &[Value], v: Value) -> Result<(), String> {
    let (idx, rest) = indices
        .split_first()
        .expect("set_nested requires at least one index");
    match container {
        Value::List(xs) => {
            let list = std::rc::Rc::make_mut(xs);
            let i = match idx {
                Value::Int(n) => *n,
                other => return Err(format!("expected int index, found {}", other.type_name())),
            };
            let i = usize::try_from(i)
                .ok()
                .filter(|i| *i < list.len())
                .ok_or_else(|| "list index out of range".to_string())?;
            if rest.is_empty() {
                list[i] = v;
                Ok(())
            } else {
                set_nested(&mut list[i], rest, v)
            }
        }
        Value::Map(m) => {
            let map = std::rc::Rc::make_mut(m);
            if rest.is_empty() {
                map_set(map, idx, v)
            } else {
                let k = HKey::from_value(idx)
                    .ok_or_else(|| format!("invalid map key: {}", idx.type_name()))?;
                let slot = map
                    .iter_mut()
                    .find(|(ek, _)| *ek == k)
                    .map(|(_, val)| val)
                    .ok_or_else(|| "map key not found".to_string())?;
                set_nested(slot, rest, v)
            }
        }
        other => Err(format!("cannot index-assign {}", other.type_name())),
    }
}

/// Maximum number of elements a range literal may materialize before faulting (P1-#9). An unbounded
/// `0..n` would otherwise allocate an arbitrarily large `Vec` and abort the process (OOM, exit 101)
/// instead of producing a clean, byte-identical fault on both backends (EV-7). ~10M × 16 B ≈ 160 MB
/// ceiling — generous for any realistic program, well below uncontrolled OOM. Tunable.
pub const MAX_RANGE_LEN: i64 = 10_000_000;

/// Materialize an integer range exactly as both backends do, with a shared size guard (P1-#9). `hi`
/// is the inclusive upper bound: `end` for `..=`, `end - 1` for `..`. An empty/reversed range
/// (`start > hi`) yields `[]`. A range wider than [`MAX_RANGE_LEN`] faults `"range too large"` rather
/// than OOM-aborting. All arithmetic is checked (EV-7): `end - 1` underflow (exclusive `..i64::MIN`)
/// and `hi - start` overflow both resolve without panicking. Single-sourced so `run`/`runvm` fault
/// identically (the differential harness classifies the body substring as `RangeTooLarge`).
pub fn build_range(start: i64, end: i64, inclusive: bool) -> Result<Vec<Value>, String> {
    let hi = if inclusive {
        end
    } else {
        match end.checked_sub(1) {
            Some(h) => h,
            None => return Ok(Vec::new()), // exclusive `start..i64::MIN` — always empty
        }
    };
    if start > hi {
        return Ok(Vec::new());
    }
    let span = hi.checked_sub(start).ok_or("range too large")?;
    if span >= MAX_RANGE_LEN {
        return Err("range too large".to_string());
    }
    Ok((start..=hi).map(Value::Int).collect())
}

/// Ordering probe for `< > <= >=`. `Ok(None)` is the NaN case (every ordered comparison of NaN is
/// `false`); `Err` is a non-comparable operand pairing. The op→bool projection stays backend-local
/// (the op enums differ); only the ordering and the comparability fault are shared.
pub fn compare_ord(a: &Value, b: &Value) -> Result<Option<Ordering>, String> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Ok(x.partial_cmp(y)),
        (Value::Float(x), Value::Float(y)) => Ok(x.partial_cmp(y)),
        // Decimal ordering is numeric + scale-insensitive (mixed `decimal`/`int` allowed): the checker
        // guarantees `< > <= >=` operands match (decimal⊕decimal or decimal⊕int), so this only fires
        // for those. `decimal_cmp` returns `None` only on an (unreachable-for-equal-values) alignment
        // overflow, which projects to `false` like NaN — sound, since equality is `Some(Equal)` only.
        (Value::Decimal { .. }, Value::Decimal { .. } | Value::Int(_))
        | (Value::Int(_), Value::Decimal { .. }) => Ok(decimal_cmp(a, b)),
        _ => Err(format!(
            "cannot compare {} and {}",
            a.type_name(),
            b.type_name()
        )),
    }
}
