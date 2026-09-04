//! The transpiler's once-per-file runtime-helper emission gates. Each `uses_*` flag is set when its
//! helper is first emitted (the established gated-helper pattern) and read by the runtime-helper
//! emitters to define that helper exactly once per file. Split out of the `Transpiler` struct
//! (M-Decomp) to keep `transpile/mod.rs` and the struct itself under the file-size cap.
//!
//! `#[derive(Default)]` reproduces the old field-by-field `false` initialization verbatim (every flag
//! starts unset), so emitted PHP is byte-identical. Every field is `pub(in crate::transpile)` so the
//! emitter methods — spread across the sibling modules — read and set them exactly as before, now via
//! `self.gates.uses_*`.

#[derive(Default)]
pub(in crate::transpile) struct HelperGates {
    /// Set when `/`, `%`, an interpolation, or a range is emitted — each defines a once-per-file
    /// runtime helper (M7) that reproduces Phorj's type-driven semantics under PHP's looser rules:
    /// `__phorj_div` (int `/` ⇒ `intdiv`), `__phorj_rem` (float `%` ⇒ `fmod`), `__phorj_str`
    /// (bool ⇒ `"true"/"false"`), `__phorj_range` (empty/reversed ⇒ `[]`, never descending).
    pub(in crate::transpile) uses_div: bool,
    pub(in crate::transpile) uses_rem: bool,
    /// `__phorj_add` — `+` overloaded for string concat (`is_string` ⇒ `.`, else `+`).
    pub(in crate::transpile) uses_add: bool,
    pub(in crate::transpile) uses_str: bool,
    /// Set when an interpolation hole is statically a `float` and emits `__phorj_float` directly
    /// (T6) — so the shortest-round-trip float formatter is defined even when `__phorj_str` (its
    /// usual host) is never emitted because every other hole's kind was resolved natively.
    pub(in crate::transpile) uses_float: bool,
    pub(in crate::transpile) uses_range: bool,
    /// Set when `Reflection.kind(x)` is emitted — defines the `__phorj_kind` runtime helper once per
    /// file. A native's `php` closure can't set a `uses_*` flag (it has no `&mut self`), so
    /// `emit_member_call` special-cases this one native to set the flag before emitting (the
    /// established gated-helper pattern). The helper reproduces the coarse, erasure-stable type tag.
    pub(in crate::transpile) uses_reflect_kind: bool,
    /// Set when `Reflection.className(x)` is emitted — defines the `__phorj_class_name` helper once per
    /// file (single-evaluates its argument; excludes closures). Same gated-helper rationale as
    /// `uses_reflect_kind`.
    pub(in crate::transpile) uses_reflect_class_name: bool,
    /// Set when a `Core.Reflection.interfaces`/`parents`/… call is emitted — defines the
    /// `__phorj_reflect_of($v, $kind)` helper + its static table once per file.
    pub(in crate::transpile) uses_reflect_tables: bool,
    /// Set when `Core.Json.stringify`/`stringifyPretty`/`parse` is emitted — each defines its
    /// `__phorj_json_*` recursive helper once per file (gated-helper pattern, set in
    /// `emit_member_call`); walks the injected `Json` enum's scoped PHP variant classes; floats
    /// route via `__phorj_float` (implies `uses_float`).
    pub(in crate::transpile) uses_json_encode: bool,
    /// Per `Core.Native.Http` native emitted (DEC-331 s2) — gates the `__phorj_http_*` family.
    pub(in crate::transpile) uses_http: bool,
    pub(in crate::transpile) uses_json_pretty: bool,
    pub(in crate::transpile) uses_json_decode: bool,
    pub(in crate::transpile) uses_json_parse_lines: bool,
    pub(in crate::transpile) uses_json_stringify_lines: bool,
    pub(in crate::transpile) uses_ini_parse: bool,
    /// Set per `Core.Option` combinator/conversion emitted (Wave B B-2a) — each defines its gated
    /// `__phorj_option_*` helper once per file, operating over the injected `Some`/`None` PHP classes
    /// (no PHP builtin analog). The higher-order ones take the transpiled closure as a PHP callable;
    /// all bind the receiver to a param first, so an argument expression is never evaluated twice.
    pub(in crate::transpile) uses_option_map: bool,
    pub(in crate::transpile) uses_option_and_then: bool,
    pub(in crate::transpile) uses_option_filter: bool,
    pub(in crate::transpile) uses_option_get_or_else: bool,
    pub(in crate::transpile) uses_option_of_nullable: bool,
    pub(in crate::transpile) uses_option_to_nullable: bool,
    // `Core.Result` combinators (B-2b, DEC-185); `isSuccess`/`isFailure` inline `instanceof`.
    pub(in crate::transpile) uses_result_map: bool,
    pub(in crate::transpile) uses_result_map_err: bool,
    pub(in crate::transpile) uses_result_and_then: bool,
    pub(in crate::transpile) uses_result_get_or_else: bool,
    pub(in crate::transpile) uses_result_or_else: bool,
    pub(in crate::transpile) uses_result_to_option: bool,
    /// Set when `Core.Text.parseInt` is emitted — defines `__phorj_parse_int` once per file,
    /// mirroring Rust's `i64::from_str` (sign, base-10, i64 range, no surrounding whitespace);
    /// `null` otherwise — incl. i64 overflow, which PHP's `(int)` cast would silently clamp.
    pub(in crate::transpile) uses_text_parse_int: bool,
    /// Set when `Core.List.sort`/`sortWith` is emitted — the matching `__phorj_sort*` helper, once
    /// per file; copies before `usort` (immutability); `sort` = a `<=>`/`strcmp` type-dispatched
    /// comparator (byte order, not PHP numeric-string — Rust's natural order), `sortWith` = closure.
    pub(in crate::transpile) uses_list_sort: bool,
    pub(in crate::transpile) uses_list_sort_with: bool,
    /// Set when `Core.List.takeWhile` / `dropWhile` is emitted — gates the matching
    /// `__phorj_take_while` / `__phorj_drop_while` helper (a `foreach` + early `break`/`continue` that
    /// binds the list once, matching the native's stop-at-first-failing-element prefix/suffix).
    pub(in crate::transpile) uses_list_take_while: bool,
    pub(in crate::transpile) uses_list_drop_while: bool,
    /// Set when `Core.List.groupBy` is emitted — gates `__phorj_group_by` (a `foreach` auto-vivifying
    /// `$out[$f($x)][] = $x` in first-seen key order → the `Map<U, List<T>>` grouping).
    pub(in crate::transpile) uses_list_group_by: bool,
    /// Set when `Output.capture(fn)` is emitted (DEC-220-S3) — gates the once-per-file
    /// `__phorj_capture($fn){ ob_start(); $fn(); return ob_get_clean(); }` helper.
    pub(in crate::transpile) uses_capture: bool,
    /// Set when the matching `Core.List` breadth op is emitted — each defines a `__phorj_*` helper
    /// once per file (List breadth slice). They exist instead of inlining PHP `min`/`max`/`array_unique`
    /// because those juggle numeric strings, diverging from the Rust backends' byte-order;
    /// `find`/`any`/`all` short-circuit (`foreach` + early `return`) like the Rust kernels.
    pub(in crate::transpile) uses_list_unique: bool,
    pub(in crate::transpile) uses_list_difference: bool,
    pub(in crate::transpile) uses_list_intersection: bool,
    pub(in crate::transpile) uses_list_min: bool,
    pub(in crate::transpile) uses_list_max: bool,
    pub(in crate::transpile) uses_list_min_by: bool,
    pub(in crate::transpile) uses_list_max_by: bool,
    pub(in crate::transpile) uses_list_find: bool,
    pub(in crate::transpile) uses_list_any: bool,
    pub(in crate::transpile) uses_list_none: bool,
    pub(in crate::transpile) uses_list_all: bool,
    /// Set when `Core.Map.set` / `remove` is emitted — defines the matching `__phorj_map_set` /
    /// `__phorj_map_remove` helper once per file. Both produce a NEW map (Phorj maps are immutable);
    /// PHP arrays are COW value types, so the helper's by-value `$m` is already a copy.
    pub(in crate::transpile) uses_map_set: bool,
    pub(in crate::transpile) uses_map_remove: bool,
    /// Set when `Core.List.indexOf` is emitted — defines `__phorj_index_of`, which maps PHP
    /// `array_search`'s `false`-on-miss to `null` (the `int?` return).
    pub(in crate::transpile) uses_list_index_of: bool,
    /// Set when `Core.List.lastIndexOf` is emitted — defines `__phorj_last_index_of`, the LAST-match
    /// companion to `__phorj_index_of` (PHP `array_keys($xs, $needle, true)` → last key, or `null`).
    pub(in crate::transpile) uses_list_last_index_of: bool,
    /// Set when `Core.Text.indexOf` is emitted — defines `__phorj_text_index_of`, mapping PHP
    /// `strpos`'s `false`-on-miss to `null` (the `int?` return).
    pub(in crate::transpile) uses_text_index_of: bool,
    /// Set when `Core.String.reverse` is emitted — defines `__phorj_text_reverse`, reversing by
    /// Unicode code point (matching Rust `str::chars().rev()`) instead of PHP `strrev`'s byte
    /// reversal, which mangles multibyte text (UA-1.2).
    pub(in crate::transpile) uses_text_reverse: bool,
    /// Set when `Core.String.trim`/`trimStart`/`trimEnd` is emitted — defines the `__phorj_text_trim*`
    /// helpers that strip Rust's Unicode White_Space set (via PCRE `/u`), NOT PHP's ASCII-ish
    /// `trim`/`ltrim`/`rtrim` (which miss U+00A0/U+3000/… and mishandle form-feed vs NUL) — UA-1.1.
    pub(in crate::transpile) uses_text_trim: bool,
    pub(in crate::transpile) uses_text_trim_start: bool,
    pub(in crate::transpile) uses_text_trim_end: bool,
    /// Set when `Core.Text.parseFloat` is emitted — defines `__phorj_parse_float`, which gates the
    /// float grammar (strict / permissive, rejecting inf/nan) then casts, mirroring the Rust kernel.
    pub(in crate::transpile) uses_text_parse_float: bool,
    /// Set when `Core.String.chunk` is emitted — defines `__phorj_str_chunk`, which splits by CODE
    /// POINTS (not PHP str_split's bytes — no broken multibyte) via `preg_split('//u')` + `array_chunk`
    /// (the latter throws `ValueError` on n<1, matching the Rust native's fault), then `implode`s each.
    pub(in crate::transpile) uses_text_chunk: bool,
    /// Set when a `decimal` `+`/`-`/`*` (or `Decimal.of`) is emitted — each defines its BCMath
    /// `__phorj_dec_*` helper once per file (M-NUM S1). The helpers derive operand scales at runtime,
    /// compute the result scale (add/sub = max, mul = sum), call `bcadd`/`bcsub`/`bcmul`, then
    /// bounds-check the result against i128 range and `throw` the same `decimal overflow` fault as the
    /// Rust kernels — so the PHP leg matches interp/VM byte-for-byte (incl. the overflow fault).
    pub(in crate::transpile) uses_dec_add: bool,
    pub(in crate::transpile) uses_dec_sub: bool,
    pub(in crate::transpile) uses_dec_mul: bool,
    /// Set when bare `decimal % decimal` is emitted — defines `__phorj_dec_rem` (`bcmod` at
    /// `max(scales)`; a zero divisor throws, matching the Rust `decimal_rem` fault).
    pub(in crate::transpile) uses_dec_rem: bool,
    /// Set when bare `decimal / decimal` is emitted — defines `__phorj_dec_div_exact` (bcdiv +
    /// exactness check + trailing-zero strip; non-terminating / zero divisor throws, matching the
    /// Rust `decimal_div_exact` fault boundary byte-for-byte).
    pub(in crate::transpile) uses_dec_div_exact: bool,
    /// Set when `Process.run`/`runWith` are emitted (DEC-472) — defines `__phorj_proc_run`, which
    /// calls `proc_open` with an ARRAY so no shell parses the argv, matching the native leg's
    /// no-shell guarantee.
    pub(in crate::transpile) uses_proc_run: bool,
    /// Set when `String.wordWrap` is emitted — defines `__phorj_wordwrap`, the CODEPOINT algorithm.
    /// PHP's own `wordwrap` is byte-oriented and can emit invalid UTF-8, which a phorj `string`
    /// cannot hold, so the helper exists to keep all three legs identical rather than to work around
    /// a missing function.
    pub(in crate::transpile) uses_wordwrap: bool,
    /// Set when `Time.sleep` is emitted (DEC-487) — defines `__phorj_sleep`, which mirrors the
    /// native's three properties: a NO-OP under a frozen clock (so a frozen example costs nothing on
    /// this leg either), immediate return for a non-positive duration, else `usleep`. PHP cannot
    /// poll for SIGINT without `pcntl`, so the interruptibility half is native-only and disclosed.
    pub(in crate::transpile) uses_sleep: bool,
    /// Set when `String.foldAccents` is emitted (DEC-468) — defines `__phorj_fold_accents`. The
    /// table is FORMATTED FROM `crate::fold_accents::FOLD` at emit time, so the native leg and the
    /// PHP leg read one source; `iconv(...,'ASCII//TRANSLIT',...)` is not an option (ini extension,
    /// and locale-dependent, so not byte-identical).
    pub(in crate::transpile) uses_fold_accents: bool,
    /// Set when `Encoding.decode`/`encode` are emitted (DEC-468/DEC-494) — defines
    /// `__phorj_cs_name` / `__phorj_cs_decode` / `__phorj_cs_encode`. The charset tables are
    /// FORMATTED FROM the Rust consts in `ext::encoding::charset` at emit time, so the PHP leg and
    /// the native leg cannot drift; `mb_convert_encoding`/`iconv` are forbidden ini extensions,
    /// which is why the codec is hand-rolled rather than delegated.
    pub(in crate::transpile) uses_charset: bool,
    /// Set when `Decimal.of(s)` is emitted — defines `__phorj_dec_of`, validating the literal grammar
    /// (a tier-1 PCRE — NOT mbstring) + i128 range, returning the normalized decimal string or `null`.
    pub(in crate::transpile) uses_dec_of: bool,
    /// Set when `Decimal.div`/`Decimal.round` are emitted (M-NUM S2) — define `__phorj_dec_div` /
    /// `__phorj_dec_round`, replicating the Rust `round_div` kernel via BCMath (verified vs Rust
    /// i128 `/`/`%`), switching on `RoundingMode`'s PHP form; both gate `__phorj_round_div`.
    pub(in crate::transpile) uses_dec_div: bool,
    pub(in crate::transpile) uses_dec_round: bool,
    /// Set when `Convert.toInt(float)` is emitted (M-NUM S3) — defines `__phorj_float_to_int`,
    /// returning `null` on NaN/±∞/out-of-i64-range else the truncated int, with the edge-safe float
    /// bounds that agree with Rust `value::float_to_int` (avoids PHP's `(int)NAN == 0`).
    pub(in crate::transpile) uses_float_to_int: bool,
    /// Set when `Convert.decimalToInt(decimal)` is emitted (M-NUM S3) — defines `__phorj_dec_to_int`,
    /// truncating the carrier string toward zero (split before the dot) and range-checking i64, else
    /// `null`. Mirrors Rust `value::decimal_to_int`.
    pub(in crate::transpile) uses_dec_to_int: bool,
    /// Set when `Convert.floatToIntExact(float)` is emitted (M4 as-matrix `float as int`) — defines
    /// `__phorj_float_to_int_exact`: the integral-or-null kernel (`3.0→3`, `3.9→null`). Mirrors Rust
    /// `value::float_to_int_exact`.
    pub(in crate::transpile) uses_float_to_int_exact: bool,
    /// Set when `Convert.truncate(float)` is emitted (fault-parity pass 2026-07-05) — defines
    /// `__phorj_trunc`: truncate toward zero, FAULT on NaN/±∞/out-of-i64-range (the raw `(int)` cast
    /// diverged — Rust saturates, PHP wraps). Mirrors Rust `convert_truncate` (`value::float_to_int`).
    pub(in crate::transpile) uses_trunc: bool,
    /// Set when `Convert.round(float)` is emitted — defines `__phorj_round`: round half-away-from-zero
    /// (PHP `round()` default ≡ Rust `f.round()`), FAULT on NaN/±∞/out-of-i64-range. Mirrors
    /// `convert_round`.
    pub(in crate::transpile) uses_round: bool,
    /// Set when `Convert.decimalToIntExact(decimal)` is emitted (M4 as-matrix `decimal as int`) —
    /// defines `__phorj_dec_to_int_exact`: integral-or-null over the carrier string. Mirrors Rust
    /// `value::decimal_to_int_exact`.
    pub(in crate::transpile) uses_dec_to_int_exact: bool,
    /// Set when `Math.gcd(int, int)` is emitted (M-NUM S4) — defines `__phorj_gcd` (Euclid over the
    /// magnitudes), since gmp is absent under `php -n`. Mirrors the Rust `math_gcd` native body.
    pub(in crate::transpile) uses_math_gcd: bool,
    /// Set when `Math.clamp(int, int, int)` is emitted (UA-1.7) — defines `__phorj_clamp`, which
    /// faults on `lo > hi` (a caller bug) to match the native; the inline `max(min())` could not.
    pub(in crate::transpile) uses_math_clamp: bool,
    /// Set when `String.format(spec, args)` is emitted (W3-5/DEC-199) — defines `__phorj_format`, the
    /// PHP mirror of the strict `%`-sprintf renderer (`text_format`): `%s`→`__phorj_str`, `%d`→int-or-
    /// fault, `%%`→`%`, any other directive / count mismatch → a fault, byte-for-byte the same as the
    /// interpreter and VM.
    pub(in crate::transpile) uses_string_format: bool,
    /// DEC-238: a `Core.Native.Debug.render` call was emitted → emit the `__phorj_debug_render` twin
    /// (+ the enum-variant table it needs to render transpiled enums as `Ty.Variant(...)`).
    pub(in crate::transpile) uses_debug_render: bool,
    /// DEC-255: a READ-context index (`xs[i]` / `m[k]`) was emitted → emit the `__phorj_index` helper
    /// that THROWS on an out-of-range / missing key (PHP's bare `$o[$k]` silently returns null+Warning,
    /// where phorj faults — a byte-identity break in the fault direction the helper closes).
    pub(in crate::transpile) uses_index: bool,
    /// DEC-255: an int `+`/`-`/`*`/unary-neg was emitted → emit the `__phorj_checked_*` helpers that
    /// THROW on integer overflow (bare PHP int arithmetic silently promotes to float, where phorj
    /// faults). Only int-int arithmetic wraps; a float operand yields a legitimate float (no fault).
    pub(in crate::transpile) uses_checked_arith: bool,
    /// DEC-255: a native whose int result PHP silently promotes to float on overflow was emitted
    /// (`Math.abs` at `i64::MIN`, `Math.integerPower` overflow/neg-exp, `List.sum` overflow) → emit
    /// `__phorj_checked_int($r)` which THROWS when the wrapped result promoted, matching phorj's fault.
    pub(in crate::transpile) uses_checked_int: bool,
    /// Set when `Math.lcm(int, int)` is emitted (M4) — defines `__phorj_lcm` (`x/gcd*y` over the
    /// magnitudes, inlining Euclid so it needs no `__phorj_gcd`). Mirrors the Rust `math_lcm` native.
    pub(in crate::transpile) uses_math_lcm: bool,
    /// Set when `Math.numberFormat(float, int)` is emitted (M-NUM S4) — defines
    /// `__phorj_number_format`, assembling the grouped string byte-for-byte like `value::number_format`
    /// (so the PHP leg never relies on PHP's own `number_format` and its `-0`/locale quirks).
    pub(in crate::transpile) uses_math_number_format: bool,
    /// `Core.Random` emitted → the `__phorj_rng_*` helpers: hand-rolled xorshift64 byte-identical to
    /// the Rust kernel (masked `>>` = logical shift; `GOLDEN` = signed reinterpretation).
    pub(in crate::transpile) uses_rng: bool,
    /// `Core.Native.Uri` emitted (DEC-240) → the `__phorj_uri*` helpers over PHP 8.5's `Uri\Rfc3986`,
    /// mapping `InvalidUriException` to the `<<E>>`-sentinels the `Uri` prelude classifies.
    pub(in crate::transpile) uses_uri: bool,
    /// `Core.Regex` emitted (Fork A) → the `__phorj_regex_*` helpers (+ `__phorj_regex_delim`): the
    /// injected `Regex` holds the bare pattern, each helper builds a collision-free `~…~u` PCRE form
    /// for the matching `preg_*`. Byte-identical to the `regex`-crate backends on the regular subset;
    /// `\d\w\s` Unicode-vs-ASCII is the one documented edge (KNOWN_ISSUES) — examples keep ASCII.
    pub(in crate::transpile) uses_regex: bool,
    /// `Core.Time` emitted (M-TIME) → the `__phorj_now_*` freezable-clock helpers matching the Rust
    /// kernel (`src/native/time.rs`); frozen = byte-identical, unfrozen = documented non-gated.
    pub(in crate::transpile) uses_clock: bool,
    /// `Core.Log`/`Core.Native.Log` emitted (DEC-317) → the `__phorj_log_*` helpers (`log_php.rs`).
    pub(in crate::transpile) uses_log: bool,
    /// `Core.Native.FileSystem` emitted (DEC-313) → the `__phorj_fs_*` helpers (`fs_php.rs`).
    pub(in crate::transpile) uses_fs: bool,
    /// DEC-340 — the program touches `Core.Native.Database`, so the `__phorj_db_*` SAVEPOINT helpers
    /// must be emitted. PDO's `beginTransaction()` does not nest, so phorj's nesting `begin()` and its
    /// entry-depth auto-rollback cannot be expressed without them.
    pub(in crate::transpile) uses_db: bool,
}
