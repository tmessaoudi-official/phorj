//! `Core.List` native registrations (kernels live in list.rs).

use super::list::*;
use super::*;
use crate::types::Ty;

pub(crate) fn list_natives() -> Vec<NativeFn> {
    let t = || Ty::Param("T".into());
    let u = || Ty::Param("U".into());
    // `minBy`/`maxBy`'s selector result var: like `map`'s `U`, but it appears ONLY in the selector's
    // return position and never in the native's result (`Optional(T)`) — the checker binds it in `θ`
    // and simply never substitutes it (unify recurses into `Ty::Function` returns; overloads.rs).
    let k = || Ty::Param("K".into());
    let list = |e: Ty| Ty::List(Box::new(e));
    vec![
        NativeFn {
            module: "Core.List",
            name: "reverse",
            params: vec![Ty::List(Box::new(t()))],
            ret: Ty::List(Box::new(t())),
            pure: true,
            eval: NativeEval::Pure(list_reverse),
            // array_reverse re-indexes a list (sequential keys) — byte-identical to the Rust Vec.
            lift_from: &["array_reverse"],
            php: |a| format!("array_reverse({})", parg(a, 0)),
        },
        // `zip(a, b) -> List<(A, B)>` (DEC-288) — positional pairs, length = min(|a|, |b|).
        NativeFn {
            module: "Core.List",
            name: "zip",
            params: vec![list(t()), list(u())],
            ret: list(Ty::Tuple(vec![t(), u()])),
            pure: true,
            eval: NativeEval::Pure(list_zip),
            // An IIFE binds both args ONCE (no double-eval) and truncates to the shorter length —
            // `array_map(null, …)` would pad the shorter with null (length = max), so it can't be used.
            lift_from: &[],
            php: |a| {
                format!(
                    "(function($__za, $__zb) {{ $__zn = min(count($__za), count($__zb)); $__zr = []; for ($__zi = 0; $__zi < $__zn; $__zi++) {{ $__zr[] = [$__za[$__zi], $__zb[$__zi]]; }} return $__zr; }})({}, {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        // `enumerate(xs) -> Map<int, T>` — index→element pairs for `for (int i, T x in …)` (B1).
        NativeFn {
            module: "Core.List",
            name: "enumerate",
            params: vec![list(t())],
            ret: Ty::Map(Box::new(Ty::Int), Box::new(t())),
            pure: true,
            eval: NativeEval::Pure(list_enumerate),
            // A PHP list is already 0-keyed; array_values guarantees sequential int keys.
            lift_from: &["array_values"],
            php: |a| format!("array_values({})", parg(a, 0)),
        },
        // `fill(value, count) -> List<T>` — `count` copies of `value` (PHP `array_fill`, value last).
        NativeFn {
            module: "Core.List",
            name: "fill",
            params: vec![t(), Ty::Int],
            ret: list(t()),
            pure: true,
            eval: NativeEval::Pure(list_fill),
            lift_from: &[],
            php: |a| format!("array_fill(0, {}, {})", parg(a, 1), parg(a, 0)),
        },
        NativeFn {
            module: "Core.List",
            name: "length",
            params: vec![Ty::List(Box::new(t()))],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(list_length),
            lift_from: &["count"],
            php: |a| format!("count({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.List",
            name: "sum",
            params: vec![Ty::List(Box::new(Ty::Int))],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(list_sum),
            lift_from: &["array_sum"],
            php: |a| format!("array_sum({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.List",
            name: "product",
            params: vec![Ty::List(Box::new(Ty::Int))],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(list_product),
            // PHP `array_product` (empty → 1); checked-overflow faults, PHP promotes to float — the
            // `array_sum` caveat, examples stay in i64 range.
            lift_from: &["array_product"],
            php: |a| format!("array_product({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.List",
            name: "contains",
            params: vec![list(t()), t()],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(list_contains),
            // strict `in_array` (=== ) matches Phorj's value equality for scalars + nested
            // lists/maps; arg order is (needle, haystack) — the reverse of `contains(list, value)`.
            // (A list of class instances would differ: PHP `===` is identity, Phorj is structural —
            // KNOWN_ISSUES; scalar/collection element lists are byte-identical.)
            lift_from: &[],
            php: |a| format!("in_array({}, {}, true)", parg(a, 1), parg(a, 0)),
        },
        NativeFn {
            module: "Core.List",
            name: "map",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(u()), Vec::new()),
            ],
            ret: list(u()),
            pure: true,
            eval: NativeEval::HigherOrder(list_map),
            // array_map(callable, array) — note the order is swapped vs Phorj's map(list, f).
            lift_from: &[],
            php: |a| format!("array_map({}, {})", parg(a, 1), parg(a, 0)),
        },
        NativeFn {
            module: "Core.List",
            name: "filter",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(Ty::Bool), Vec::new()),
            ],
            ret: list(t()),
            pure: true,
            eval: NativeEval::HigherOrder(list_filter),
            // array_filter preserves original keys; array_values re-indexes to a sequential list.
            lift_from: &[],
            php: |a| format!("array_values(array_filter({}, {}))", parg(a, 0), parg(a, 1)),
        },
        // `partition(xs, pred) -> (List<T>, List<T>)` (DEC-288) — (matching, non-matching).
        NativeFn {
            module: "Core.List",
            name: "partition",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(Ty::Bool), Vec::new()),
            ],
            ret: Ty::Tuple(vec![list(t()), list(t())]),
            pure: true,
            eval: NativeEval::HigherOrder(list_partition),
            // An IIFE binds the list + predicate ONCE, splits in one pass, and returns the erased
            // 2-tuple `[matching, non-matching]` — both re-indexed sequentially (`[]` append).
            lift_from: &[],
            php: |a| {
                format!(
                    "(function($__pl, $__pf) {{ $__py = []; $__pn = []; foreach ($__pl as $__px) {{ if ($__pf($__px)) {{ $__py[] = $__px; }} else {{ $__pn[] = $__px; }} }} return [$__py, $__pn]; }})({}, {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.List",
            name: "flatMap",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(list(u())), Vec::new()),
            ],
            ret: list(u()),
            pure: true,
            eval: NativeEval::HigherOrder(list_flat_map),
            // map each element to a list, then concatenate. `[]` seeds array_merge so an empty input
            // (or all-empty results) yields `[]`, never array_merge()'s no-argument error; the spread
            // re-indexes exactly like the native's sequential extend.
            lift_from: &[],
            php: |a| {
                format!(
                    "array_merge([], ...array_map({}, {}))",
                    parg(a, 1),
                    parg(a, 0)
                )
            },
        },
        NativeFn {
            module: "Core.List",
            name: "takeWhile",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(Ty::Bool), Vec::new()),
            ],
            ret: list(t()),
            pure: true,
            eval: NativeEval::HigherOrder(list_take_while),
            // Gated `__phorj_take_while` (binds the list once; a `foreach` + early `break` matches the
            // native's stop-at-first-false — an inline expression would re-evaluate the list arg).
            lift_from: &[],
            php: |a| format!("__phorj_take_while({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.List",
            name: "dropWhile",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(Ty::Bool), Vec::new()),
            ],
            ret: list(t()),
            pure: true,
            eval: NativeEval::HigherOrder(list_drop_while),
            lift_from: &[],
            php: |a| format!("__phorj_drop_while({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.List",
            name: "groupBy",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(u()), Vec::new()),
            ],
            ret: Ty::Map(Box::new(u()), Box::new(list(t()))),
            pure: true,
            eval: NativeEval::HigherOrder(list_group_by),
            // Gated `__phorj_group_by`: `$out[$f($x)][] = $x` auto-vivifies groups in first-seen key
            // order (≡ the native's first-seen Vec), matching the Map<U,List<T>> representation.
            lift_from: &[],
            php: |a| format!("__phorj_group_by({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.List",
            name: "reduce",
            params: vec![
                list(t()),
                u(),
                Ty::Function(vec![u(), t()], Box::new(u()), Vec::new()),
            ],
            ret: u(),
            pure: true,
            eval: NativeEval::HigherOrder(list_reduce),
            // array_reduce(array, callback, initial) — initial is Phorj's 2nd arg, fn its 3rd.
            lift_from: &[],
            php: |a| {
                format!(
                    "array_reduce({}, {}, {})",
                    parg(a, 0),
                    parg(a, 2),
                    parg(a, 1)
                )
            },
        },
        // `sort(List<T>) -> List<T>` — natural ascending (PHP `sort`, but byte-stable + string-byte
        // order). Gated `__phorj_sort` helper (a `<=>`/`strcmp` type-dispatched `usort` over a copy).
        NativeFn {
            module: "Core.List",
            name: "sort",
            params: vec![list(t())],
            ret: list(t()),
            pure: true,
            eval: NativeEval::Pure(list_sort),
            lift_from: &[],
            php: |a| format!("__phorj_sort({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.List",
            name: "sortDescending",
            params: vec![list(t())],
            ret: list(t()),
            pure: true,
            eval: NativeEval::Pure(list_sort_descending),
            // sort-then-reverse (reuses `__phorj_sort`) so equal-element order matches the Rust kernel.
            lift_from: &[],
            php: |a| format!("array_reverse(__phorj_sort({}))", parg(a, 0)),
        },
        // `sortWith(List<T>, (T, T) -> int) -> List<T>` — comparator (PHP `usort`), higher-order.
        NativeFn {
            module: "Core.List",
            name: "sortWith",
            params: vec![
                list(t()),
                Ty::Function(vec![t(), t()], Box::new(Ty::Int), Vec::new()),
            ],
            ret: list(t()),
            pure: true,
            eval: NativeEval::HigherOrder(list_sort_with),
            lift_from: &[],
            php: |a| format!("__phorj_sort_with({}, {})", parg(a, 0), parg(a, 1)),
        },
        // `slice(List<T>, int, int) -> List<T>` — PHP `array_slice` (offset, length; negatives count
        // from the end; out-of-range clamps to empty).
        NativeFn {
            module: "Core.List",
            name: "slice",
            params: vec![list(t()), Ty::Int, Ty::Int],
            ret: list(t()),
            pure: true,
            eval: NativeEval::Pure(list_slice),
            lift_from: &[],
            php: |a| {
                format!(
                    "array_slice({}, {}, {})",
                    parg(a, 0),
                    parg(a, 1),
                    parg(a, 2)
                )
            },
        },
        NativeFn {
            module: "Core.List",
            name: "take",
            params: vec![list(t()), Ty::Int],
            ret: list(t()),
            pure: true,
            eval: NativeEval::Pure(list_take),
            lift_from: &[],
            php: |a| format!("array_slice({}, 0, max(0, {}))", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.List",
            name: "drop",
            params: vec![list(t()), Ty::Int],
            ret: list(t()),
            pure: true,
            eval: NativeEval::Pure(list_drop),
            lift_from: &[],
            php: |a| format!("array_slice({}, max(0, {}))", parg(a, 0), parg(a, 1)),
        },
        // `chunk(List<T>, int) -> List<List<T>>` — consecutive groups of `size` (last may be shorter).
        // PHP `array_chunk` (re-indexed); `size < 1` faults on both backends (charter §3).
        NativeFn {
            module: "Core.List",
            name: "chunk",
            params: vec![list(t()), Ty::Int],
            ret: list(list(t())),
            pure: true,
            eval: NativeEval::Pure(list_chunk),
            lift_from: &["array_chunk"],
            php: |a| format!("array_chunk({}, {})", parg(a, 0), parg(a, 1)),
        },
        // `indexOf(List<T>, T) -> int?` — gated `__phorj_index_of` (PHP `array_search` strict → null).
        NativeFn {
            module: "Core.List",
            name: "indexOf",
            params: vec![list(t()), t()],
            ret: Ty::Optional(Box::new(Ty::Int)),
            pure: true,
            eval: NativeEval::Pure(list_index_of),
            lift_from: &[],
            php: |a| format!("__phorj_index_of({}, {})", parg(a, 0), parg(a, 1)),
        },
        // `lastIndexOf(List<T>, T) -> int?` — gated `__phorj_last_index_of` (PHP `array_keys` strict →
        // last key, or null). The symmetric companion to `indexOf`.
        NativeFn {
            module: "Core.List",
            name: "lastIndexOf",
            params: vec![list(t()), t()],
            ret: Ty::Optional(Box::new(Ty::Int)),
            pure: true,
            eval: NativeEval::Pure(list_last_index_of),
            lift_from: &[],
            php: |a| format!("__phorj_last_index_of({}, {})", parg(a, 0), parg(a, 1)),
        },
        // `concat(List<T>, List<T>) -> List<T>` — PHP `array_merge` (re-indexes sequential lists).
        NativeFn {
            module: "Core.List",
            name: "concat",
            params: vec![list(t()), list(t())],
            ret: list(t()),
            pure: true,
            eval: NativeEval::Pure(list_concat),
            lift_from: &["array_merge"],
            php: |a| format!("array_merge({}, {})", parg(a, 0), parg(a, 1)),
        },
        // `append(List<T>, T) -> List<T>` — a new list with the element added at the end (COW, O(n));
        // for hot loops prefer `List.fill` + index-set (O(1)/write) or `List.map(range, fn)`.
        NativeFn {
            module: "Core.List",
            name: "append",
            params: vec![list(t()), t()],
            ret: list(t()),
            pure: true,
            eval: NativeEval::Pure(list_append),
            lift_from: &[],
            php: |a| format!("array_merge({}, [{}])", parg(a, 0), parg(a, 1)),
        },
        // `first(List<T>) -> T?` / `last(List<T>) -> T?` — head/tail or null for an empty list.
        NativeFn {
            module: "Core.List",
            name: "first",
            params: vec![list(t())],
            ret: Ty::Optional(Box::new(t())),
            pure: true,
            eval: NativeEval::Pure(list_first),
            lift_from: &[],
            php: |a| format!("({}[0] ?? null)", parg(a, 0)),
        },
        NativeFn {
            module: "Core.List",
            name: "last",
            params: vec![list(t())],
            ret: Ty::Optional(Box::new(t())),
            pure: true,
            eval: NativeEval::Pure(list_last),
            lift_from: &[],
            php: |a| format!("({0}[count({0}) - 1] ?? null)", parg(a, 0)),
        },
        // `unique(List<T>) -> List<T>` — dedupe, keeping first occurrence + order. Value-equality
        // (Phorj structural ≡ the `__phorj_unique` helper's strict `in_array`); NOT PHP's
        // `array_unique` (which stringifies / juggles numeric strings — a parity break for `List<string>`).
        NativeFn {
            module: "Core.List",
            name: "unique",
            params: vec![list(t())],
            ret: list(t()),
            pure: true,
            eval: NativeEval::Pure(list_unique),
            lift_from: &[],
            php: |a| format!("__phorj_unique({})", parg(a, 0)),
        },
        // Set-style ops on Lists (FN-ARR long-tail) — typed-strict (better than PHP array_diff/intersect
        // string coercion); filter semantics (keep a's order + dups), compose with `unique` for a set.
        NativeFn {
            module: "Core.List",
            name: "difference",
            params: vec![list(t()), list(t())],
            ret: list(t()),
            pure: true,
            eval: NativeEval::Pure(list_difference),
            lift_from: &[],
            php: |a| format!("__phorj_list_difference({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.List",
            name: "intersection",
            params: vec![list(t()), list(t())],
            ret: list(t()),
            pure: true,
            eval: NativeEval::Pure(list_intersection),
            lift_from: &[],
            php: |a| format!("__phorj_list_intersection({}, {})", parg(a, 0), parg(a, 1)),
        },
        // `min(List<T>) -> T?` / `max(List<T>) -> T?` — null for an empty list. Uses the `natural_cmp`
        // byte-order (strings via `strcmp`, not PHP's numeric-string-juggling `min`/`max`), so the
        // `__phorj_min`/`_max` helpers match the Rust backends exactly.
        NativeFn {
            module: "Core.List",
            name: "min",
            params: vec![list(t())],
            ret: Ty::Optional(Box::new(t())),
            pure: true,
            eval: NativeEval::Pure(list_min),
            lift_from: &[],
            php: |a| format!("__phorj_min({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.List",
            name: "max",
            params: vec![list(t())],
            ret: Ty::Optional(Box::new(t())),
            pure: true,
            eval: NativeEval::Pure(list_max),
            lift_from: &[],
            php: |a| format!("__phorj_max({})", parg(a, 0)),
        },
        // `find(List<T>, (T) -> bool) -> T?` — the first element satisfying the predicate, or null.
        // `any` / `all` — short-circuiting existential / universal quantifiers. All three
        // SHORT-CIRCUIT identically on every backend (the `__phorj_find/any/all` helpers `foreach`
        // + early-`return`), so a side-effecting predicate produces byte-identical stdout.
        NativeFn {
            module: "Core.List",
            name: "find",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(Ty::Bool), Vec::new()),
            ],
            ret: Ty::Optional(Box::new(t())),
            pure: true,
            eval: NativeEval::HigherOrder(list_find),
            lift_from: &[],
            php: |a| format!("__phorj_find({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.List",
            name: "any",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(Ty::Bool), Vec::new()),
            ],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::HigherOrder(list_any),
            lift_from: &[],
            php: |a| format!("__phorj_any({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.List",
            name: "all",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(Ty::Bool), Vec::new()),
            ],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::HigherOrder(list_all),
            lift_from: &[],
            php: |a| format!("__phorj_all({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.List",
            name: "none",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(Ty::Bool), Vec::new()),
            ],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::HigherOrder(list_none),
            lift_from: &[],
            php: |a| format!("__phorj_none({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.List",
            name: "isEmpty",
            params: vec![list(t())],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(list_is_empty),
            lift_from: &[],
            php: |a| format!("count({}) === 0", parg(a, 0)),
        },
        NativeFn {
            module: "Core.List",
            name: "flatten",
            params: vec![list(list(t()))],
            ret: list(t()),
            pure: true,
            eval: NativeEval::Pure(list_flatten),
            // `array_merge(...$xss)` concatenates + re-indexes; `...[]` ⇒ `array_merge()` ⇒ `[]`.
            lift_from: &[],
            php: |a| format!("array_merge(...{})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.List",
            name: "count",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(Ty::Bool), Vec::new()),
            ],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::HigherOrder(list_count),
            // array_filter keeps the predicate-true elements; count them.
            lift_from: &[],
            php: |a| format!("count(array_filter({}, {}))", parg(a, 0), parg(a, 1)),
        },
        // `sumBy(List<T>, (T) -> int) -> int` — the sum of the projection over every element (empty →
        // 0). The projection sibling of `sum`/`product`/`count`; single type-var like `find` (fn ret +
        // native ret are both concrete `int`). Checked-add (overflow faults, EV-7 — the `sum` caveat),
        // non-int projection faults. Erases to `array_sum(array_map($fn, $xs))` (order-preserving).
        NativeFn {
            module: "Core.List",
            name: "sumBy",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(Ty::Int), Vec::new()),
            ],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::HigherOrder(list_sum_by),
            lift_from: &[],
            php: |a| format!("array_sum(array_map({}, {}))", parg(a, 1), parg(a, 0)),
        },
        // `minBy(List<T>, (T) -> K) -> T?` / `maxBy(List<T>, (T) -> K) -> T?` — the element whose
        // selector value is minimal / maximal (Kotlin `minByOrNull`/`maxByOrNull`), null for an empty
        // list. Selector RESULTS compared with the `min`/`max` byte-order (`natural_cmp` ≡ the PHP
        // `strcmp`/`<=>` dispatch); FIRST-wins on ties (the `__phorj_min_by`/`_max_by` helpers keep the
        // first via a strict `<`/`>` + first-seen flag — parity-affecting, since distinct elements can
        // share a key). The selector result var `K` appears only in the arg (see the `k` closure above).
        NativeFn {
            module: "Core.List",
            name: "minBy",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(k()), Vec::new()),
            ],
            ret: Ty::Optional(Box::new(t())),
            pure: true,
            eval: NativeEval::HigherOrder(list_min_by),
            lift_from: &[],
            php: |a| format!("__phorj_min_by({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.List",
            name: "maxBy",
            params: vec![
                list(t()),
                Ty::Function(vec![t()], Box::new(k()), Vec::new()),
            ],
            ret: Ty::Optional(Box::new(t())),
            pure: true,
            eval: NativeEval::HigherOrder(list_max_by),
            lift_from: &[],
            php: |a| format!("__phorj_max_by({}, {})", parg(a, 0), parg(a, 1)),
        },
    ]
}

/// `unique` — first-occurrence-order dedupe by Phorj value-equality.
fn list_unique(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs)] => {
            let mut out: Vec<Value> = Vec::new();
            for x in xs.iter() {
                if !out.iter().any(|y| y.eq_val(x)) {
                    out.push(x.clone());
                }
            }
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.unique expects (List<T>)".into()),
    }
}

/// `difference(a, b)` — a's elements NOT present in b, preserving a's order + duplicates, by STRICT
/// value equality (`eq_val`, typed — NOT PHP `array_diff`'s string coercion; better than PHP).
fn list_difference(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(a), Value::List(b)] => {
            let out: Vec<Value> = a
                .iter()
                .filter(|x| !b.iter().any(|y| y.eq_val(x)))
                .cloned()
                .collect();
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.difference expects (List<T>, List<T>)".into()),
    }
}

/// `intersection(a, b)` — a's elements ALSO present in b, preserving a's order + duplicates, strict.
fn list_intersection(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(a), Value::List(b)] => {
            let out: Vec<Value> = a
                .iter()
                .filter(|x| b.iter().any(|y| y.eq_val(x)))
                .cloned()
                .collect();
            Ok(Value::List(std::rc::Rc::new(out)))
        }
        _ => Err("List.intersection expects (List<T>, List<T>)".into()),
    }
}

/// `min`/`max` — the smallest/largest by `natural_cmp`, or `Null` for an empty list.
fn list_min(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs)] => Ok(xs
            .iter()
            .min_by(|a, b| natural_cmp(a, b))
            .cloned()
            .unwrap_or(Value::Null)),
        _ => Err("List.min expects (List<T>)".into()),
    }
}
fn list_max(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs)] => Ok(xs
            .iter()
            .max_by(|a, b| natural_cmp(a, b))
            .cloned()
            .unwrap_or(Value::Null)),
        _ => Err("List.max expects (List<T>)".into()),
    }
}

/// Run a `(T) -> bool` predicate over the list, short-circuiting. A non-bool result is a clean fault
/// (matches `filter`). `find` returns the first matching element (or `Null`); `any`/`all` the verdict.
fn list_pred(call: &mut ClosureInvoker, f: &Value, x: &Value) -> Result<bool, String> {
    match call(f, vec![x.clone()])? {
        Value::Bool(b) => Ok(b),
        other => Err(format!(
            "List predicate must return bool, got {}",
            other.type_name()
        )),
    }
}
fn list_find(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            for x in xs.iter() {
                if list_pred(call, f, x)? {
                    return Ok(x.clone());
                }
            }
            Ok(Value::Null)
        }
        _ => Err("List.find expects (List<T>, (T) -> bool)".into()),
    }
}
fn list_any(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            for x in xs.iter() {
                if list_pred(call, f, x)? {
                    return Ok(Value::Bool(true));
                }
            }
            Ok(Value::Bool(false))
        }
        _ => Err("List.any expects (List<T>, (T) -> bool)".into()),
    }
}
fn list_all(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            for x in xs.iter() {
                if !list_pred(call, f, x)? {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        }
        _ => Err("List.all expects (List<T>, (T) -> bool)".into()),
    }
}
/// `none(List<T>, (T) -> bool) -> bool` — the companion to `any`/`all`: true iff NO element satisfies
/// the predicate (`none` ≡ `!any`). Short-circuits at the first match, so a side-effecting predicate
/// is byte-identical on both backends (gated `__phorj_none`).
fn list_none(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::List(xs), f] => {
            for x in xs.iter() {
                if list_pred(call, f, x)? {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        }
        _ => Err("List.none expects (List<T>, (T) -> bool)".into()),
    }
}

// ---- Core.Map -----------------------------------------------------------------------------------
// Map query natives, all generic over the key/value types (`keys(Map<K,V>) -> List<K>`). They read
// the insertion-ordered `Value::Map` rep (a `Vec<(HKey, Value)>`, not a `HashMap` — risk R1), so
// `keys`/`values` are byte-identical with PHP's order-preserving `array_keys`/`array_values`. KEY
// COERCION CAVEAT (KNOWN_ISSUES): PHP arrays coerce integer-like string keys and bools to int keys,
// so a `keys()` over such a map renders differently under PHP than on the Rust backends; examples use
// plain (non-numeric) string keys, which PHP keeps verbatim. The interp↔VM spine is always identical.

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
