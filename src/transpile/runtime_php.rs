//! PHP transpiler — the once-per-file `__phorj_*` runtime helper templates (each gated by
//! its `uses_*` flag), mirroring the Rust value kernels byte-for-byte.

use super::*;

impl Transpiler {
    /// The once-per-file runtime helpers (each gated by its `uses_*` flag). In flat mode they are
    /// top-level globals; in namespaced mode they are emitted inside the nameless block, so their
    /// fully-qualified names are `\__phorj_*` (which the call sites emit via the `bs` prefix). Each
    /// mirrors a Phorj value kernel / `as_display` so the PHP leg matches `run`/`runvm` byte-for-byte.
    pub(super) fn emit_runtime_helpers(&mut self) {
        if self.uses_div {
            // Phorj `/`: int/int truncates toward zero (`intdiv`); float/float is real division.
            self.line("function __phorj_div($a, $b) {");
            self.indent += 1;
            self.line("return (is_int($a) && is_int($b)) ? intdiv($a, $b) : $a / $b;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_rem {
            // Phorj `%`: int/int integer modulo; float/float `fmod` (sign of dividend, like Rust `%`).
            // A zero divisor *throws* (Phorj faults on any division by zero): PHP `$a % 0` already
            // throws, but `fmod($a, 0.0)` would return `NAN`, so guard `$b == 0` first to agree.
            self.line("function __phorj_rem($a, $b) {");
            self.indent += 1;
            self.line("if ($b == 0) { throw new \\DivisionByZeroError(\"Modulo by zero\"); }");
            self.line("return (is_int($a) && is_int($b)) ? $a % $b : fmod($a, $b);");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_add {
            // Phorj `+` is overloaded: `string + string` concatenates, numbers add. The checker
            // guarantees both operands share a type, so `is_string($a)` selects the branch exactly
            // (PHP's `+` would TypeError on strings; `.` is its concat operator).
            self.line("function __phorj_add($a, $b) {");
            self.indent += 1;
            self.line("return is_string($a) ? $a . $b : $a + $b;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_str {
            // Mirror Value::as_display: bool ⇒ "true"/"false"; float ⇒ Rust `{}` formatting (via
            // __phorj_float); everything else PHP string cast. A naked `(string)$float` uses PHP's
            // `precision=14` and switches to scientific notation for large/small magnitudes — both
            // diverge from the Rust backends, which print the shortest round-trip, always positional.
            self.line("function __phorj_str($v) {");
            self.indent += 1;
            self.line("if (is_bool($v)) { return $v ? \"true\" : \"false\"; }");
            self.line("if (is_float($v)) { return __phorj_float($v); }");
            self.line("return (string)$v;");
            self.indent -= 1;
            self.line("}");
        }
        // `__phorj_float` is needed by `__phorj_str` AND directly by a statically-float interpolation
        // hole (T6) — so it is emitted whenever either is in play, independent of the `__phorj_str`
        // dispatch helper above.
        if self.uses_str || self.uses_float || self.uses_json_encode || self.uses_math_number_format
        {
            // Reproduce Rust's `f64` Display exactly (EV-6): the shortest decimal that round-trips to
            // the same double, in positional notation (never scientific, for any magnitude), with an
            // integer-valued float rendered without a trailing `.0`. The `%.{p}e` loop finds the
            // minimal precision that round-trips (Ryū/Grisu shortest is unique); the mantissa digits
            // are then placed positionally. Only tier-1 PHP functions, so it is correct under `php -n`.
            self.line("function __phorj_float($v) {");
            self.indent += 1;
            self.line("if (is_nan($v)) { return \"NaN\"; }");
            self.line("if (is_infinite($v)) { return $v < 0 ? \"-inf\" : \"inf\"; }");
            self.line("if ($v == 0.0) { return (fdiv(1.0, $v) < 0) ? \"-0\" : \"0\"; }");
            self.line("$neg = $v < 0;");
            self.line("$a = $neg ? -$v : $v;");
            self.line("$repr = sprintf(\"%.16e\", $a);");
            self.line("for ($p = 0; $p <= 16; $p++) {");
            self.indent += 1;
            self.line("$cand = sprintf(\"%.{$p}e\", $a);");
            self.line("if ((float)$cand === $a) { $repr = $cand; break; }");
            self.indent -= 1;
            self.line("}");
            self.line("$epos = strpos($repr, \"e\");");
            self.line("$exp = (int)substr($repr, $epos + 1);");
            self.line("$mant = str_replace(\".\", \"\", substr($repr, 0, $epos));");
            self.line("$mant = rtrim($mant, \"0\");");
            self.line("if ($mant === \"\") { $mant = \"0\"; }");
            self.line("$ndig = strlen($mant);");
            self.line("if ($exp >= $ndig - 1) {");
            self.indent += 1;
            self.line("$s = $mant . str_repeat(\"0\", $exp - ($ndig - 1));");
            self.indent -= 1;
            self.line("} elseif ($exp >= 0) {");
            self.indent += 1;
            self.line("$s = substr($mant, 0, $exp + 1) . \".\" . substr($mant, $exp + 1);");
            self.indent -= 1;
            self.line("} else {");
            self.indent += 1;
            self.line("$s = \"0.\" . str_repeat(\"0\", -$exp - 1) . $mant;");
            self.indent -= 1;
            self.line("}");
            self.line("return $neg ? \"-\" . $s : $s;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_range {
            // Phorj range: empty when start > hi; never descends (PHP `range()` descends — QW-13).
            self.line("function __phorj_range($a, $b, $inclusive) {");
            self.indent += 1;
            self.line("$hi = $inclusive ? $b : $b - 1;");
            self.line("return ($a <= $hi) ? range($a, $hi) : [];");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_reflect_kind {
            // `Reflect.kind` — the coarse, erasure-stable type tag, mirroring the Rust `reflect_kind`
            // arm exactly. Order is load-bearing: a PHP closure is BOTH `is_callable` and
            // `is_object`, so `is_callable` is tested first (Phorj closures ⇒ "callable", instances
            // and enum variants ⇒ "object"). Only tier-1 functions, so it is correct under `php -n`.
            self.line("function __phorj_kind($v) {");
            self.indent += 1;
            self.line("if (is_callable($v)) { return \"callable\"; }");
            self.line("if (is_object($v)) { return \"object\"; }");
            self.line("if (is_array($v)) { return \"array\"; }");
            self.line("if (is_int($v)) { return \"int\"; }");
            self.line("if (is_float($v)) { return \"float\"; }");
            self.line("if (is_bool($v)) { return \"bool\"; }");
            self.line("if (is_string($v)) { return \"string\"; }");
            self.line("return \"null\";");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_reflect_class_name {
            // `Reflect.className` — runtime class name for an object, else null. Mirrors the Rust
            // `reflect_class_name` arm: a closure is is_object in PHP but reports as not-a-class
            // (null) on both sides, so it is excluded. Single-evaluates `$v`. Tier-1 only (`php -n`).
            self.line("function __phorj_class_name($v) {");
            self.indent += 1;
            self.line("if (is_object($v) && !($v instanceof \\Closure)) { return get_class($v); }");
            self.line("return null;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_reflect_tables {
            self.emit_reflect_table();
        }
        self.emit_json_helpers();
        if self.uses_text_parse_int {
            // Mirror Rust's `i64::from_str`: `^[+-]?[0-9]+$`, in i64 range, no surrounding whitespace.
            // PHP's `(int)` clamps on overflow (≠ Rust's None), so detect overflow by re-deriving the
            // magnitude digits from the cast value and comparing to the input's (sign + leading zeros
            // stripped) — a mismatch means it clamped. Tier-1 only (PCRE), correct under `php -n`.
            self.line("function __phorj_parse_int($s) {");
            self.indent += 1;
            self.line("if (preg_match('/^[+-]?[0-9]+$/', $s) !== 1) { return null; }");
            self.line("$n = (int)$s;");
            self.line("$neg = ($s[0] === '-');");
            self.line("$digits = ltrim(ltrim($s, '+-'), '0');");
            self.line("if ($digits === '') { $digits = '0'; }");
            self.line("if ((string)($neg ? -$n : $n) !== $digits) { return null; }");
            self.line("return $n;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_text_parse_float {
            // Mirror the Rust `valid_float` grammar (strict / permissive), rejecting inf/nan, then cast.
            // PCRE only (tier-1, correct under `php -n`); `(float)` matches `f64::from_str` for the
            // accepted grammar (typical decimals; extreme-precision divergence is documented).
            self.line("function __phorj_parse_float($s, $permissive) {");
            self.indent += 1;
            self.line("$re = $permissive");
            self.line("    ? '/^[+-]?(?:[0-9]+\\.?[0-9]*|\\.[0-9]+)(?:[eE][+-]?[0-9]+)?$/'");
            self.line("    : '/^[+-]?[0-9]+(?:\\.[0-9]+)?(?:[eE][+-]?[0-9]+)?$/';");
            self.line("return preg_match($re, $s) === 1 ? (float)$s : null;");
            self.indent -= 1;
            self.line("}");
        }
        // --- Decimal (BCMath) helpers (M-NUM S1). Each mirrors the Rust `value::decimal_*` kernel:
        // derive operand scales from the strings, compute the result scale (add/sub = max, mul = sum),
        // call the matching `bc*` with that scale, then bounds-check the result's unscaled magnitude
        // against i128 range and `throw` the same `decimal overflow` body the Rust backends fault with
        // (the `agree_err` oracle classifies by body substring). BCMath is tier-1 (works under `php -n`).
        if self.uses_dec_add
            || self.uses_dec_sub
            || self.uses_dec_mul
            || self.uses_dec_rem
            || self.uses_dec_div_exact
            || self.uses_dec_div
            || self.uses_dec_round
        {
            // Scale of a BCMath decimal string = digits after the dot (0 if none). Matches the Rust
            // kernel deriving scale from `(unscaled, scale)`; a `bc*` result is always normalized.
            self.line("function __phorj_dec_scale($x) {");
            self.indent += 1;
            self.line("$p = strpos($x, '.');");
            self.line("return $p === false ? 0 : strlen($x) - $p - 1;");
            self.indent -= 1;
            self.line("}");
            // Fault if the result's unscaled magnitude leaves signed-i128 range, byte-identically to
            // the Rust `checked_*` overflow. The unscaled magnitude is the result digits with the dot
            // and sign removed; compared against i128::MAX (2^127 - 1) via `bccomp` (string-exact).
            self.line("function __phorj_dec_check($r) {");
            self.indent += 1;
            self.line("$digits = str_replace(['-', '.'], '', $r);");
            self.line("$digits = ltrim($digits, '0');");
            self.line("if ($digits === '') { $digits = '0'; }");
            self.line(
                "if (bccomp($digits, '170141183460469231731687303715887105727', 0) > 0) { \
                 throw new \\RuntimeException('decimal overflow'); }",
            );
            self.line("return $r;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_dec_add {
            self.line("function __phorj_dec_add($a, $b) {");
            self.indent += 1;
            self.line("$s = max(__phorj_dec_scale($a), __phorj_dec_scale($b));");
            self.line("return __phorj_dec_check(bcadd($a, $b, $s));");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_dec_sub {
            self.line("function __phorj_dec_sub($a, $b) {");
            self.indent += 1;
            self.line("$s = max(__phorj_dec_scale($a), __phorj_dec_scale($b));");
            self.line("return __phorj_dec_check(bcsub($a, $b, $s));");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_dec_rem {
            // Exact decimal remainder (bare `%`): `bcmod` at `max(scales)`; a zero divisor throws,
            // matching the Rust `decimal_rem` fault ("any division by zero throws").
            self.line("function __phorj_dec_rem($a, $b) {");
            self.indent += 1;
            self.line("$s = max(__phorj_dec_scale($a), __phorj_dec_scale($b));");
            self.line(
                "if (bccomp($b, '0', $s) === 0) { throw new \\DivisionByZeroError('decimal modulo by zero'); }",
            );
            self.line("return __phorj_dec_check(bcmod($a, $b, $s));");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_dec_div_exact {
            // Exact-or-fault bare `decimal /`: divide at high precision, verify the quotient is exact
            // (bcmul back == dividend), strip trailing zeros to the canonical minimal form (matching
            // the Rust `decimal_div_exact` result), then i128-bound-check. A non-terminating quotient
            // fails the exactness check and throws; a zero divisor throws. Byte-identical to the Rust
            // kernel's fault boundary + minimal output.
            self.line("function __phorj_dec_div_exact($a, $b) {");
            self.indent += 1;
            self.line("$sb = __phorj_dec_scale($b);");
            self.line(
                "if (bccomp($b, '0', $sb) === 0) { throw new \\DivisionByZeroError('decimal division by zero'); }",
            );
            self.line("$prec = __phorj_dec_scale($a) + $sb + 80;");
            self.line("$q = bcdiv($a, $b, $prec);");
            self.line(
                "if (bccomp(bcmul($q, $b, $prec * 2), $a, $prec) !== 0) { throw new \\RuntimeException('decimal division is not exact'); }",
            );
            self.line(
                "if (strpos($q, '.') !== false) { $q = rtrim($q, '0'); $q = rtrim($q, '.'); }",
            );
            self.line("if ($q === '' || $q === '-' || $q === '-0') { $q = '0'; }");
            self.line("return __phorj_dec_check($q);");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_dec_mul {
            self.line("function __phorj_dec_mul($a, $b) {");
            self.indent += 1;
            self.line("$s = __phorj_dec_scale($a) + __phorj_dec_scale($b);");
            self.line("return __phorj_dec_check(bcmul($a, $b, $s));");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_dec_of {
            // `Decimal.of(s) -> decimal?`: validate the literal grammar (optional sign, digits with an
            // optional single fractional part — `12`, `12.34`, `.5`; NO exponent/underscore/whitespace)
            // with a PCRE, then bounds-check the i128 range; return the normalized string or null.
            // Mirrors the Rust `value::decimal_of` exactly. The string is already its own decimal form
            // (no `bc*` normalization needed — Phorj preserves trailing zeros as scale).
            self.line("function __phorj_dec_of($s) {");
            self.indent += 1;
            self.line("if (preg_match('/^[+-]?(?:[0-9]+(?:\\.[0-9]+)?|\\.[0-9]+)$/', $s) !== 1) { return null; }");
            self.line("$digits = ltrim(str_replace(['-', '+', '.'], '', $s), '0');");
            self.line("if ($digits === '') { $digits = '0'; }");
            self.line("if (bccomp($digits, '170141183460469231731687303715887105727', 0) > 0) { return null; }");
            // Normalize a leading `+` away (Phorj's render has no `+`); keep the scale (trailing zeros).
            self.line("return ltrim($s, '+');");
            self.indent -= 1;
            self.line("}");
        }
        // --- Decimal division + rounding (M-NUM S2). Replicate the Rust `value::round_div` kernel via
        // BCMath integer arithmetic on the *unscaled* integer strings (`bcdiv`/`bcmod` truncate toward
        // zero / take the dividend's sign — verified identical to Rust i128 `/`/`%`), so every rounding
        // mode matches `run`/`runvm` byte-for-byte. The `RoundingMode` enum value arrives as a PHP
        // object (`new HalfUp()` ⇒ an instance of the injected global class `HalfUp`); the helper reads
        // its short class name and switches on it, exactly as the Rust native reads `Value::Enum.variant`.
        if self.uses_dec_div || self.uses_dec_round {
            // Unscaled integer-string of a decimal string: drop the dot. `"19.99"`→`"1999"`,
            // `"-2.5"`→`"-25"`, `"100"`→`"100"`. Matches `(unscaled, _)` in the Rust `(unscaled, scale)`.
            self.line("function __phorj_dec_unscaled($x) {");
            self.indent += 1;
            self.line("return str_replace('.', '', $x);");
            self.indent -= 1;
            self.line("}");
            // Short (namespace-free) class name of the RoundingMode value — `HalfUp`, `Floor`, …
            self.line("function __phorj_round_mode($mode) {");
            self.indent += 1;
            self.line("$c = get_class($mode);");
            self.line("$p = strrpos($c, '\\\\');");
            self.line("return $p === false ? $c : substr($c, $p + 1);");
            self.indent -= 1;
            self.line("}");
            // round_div(n, d, mode) on integer strings — the verbatim Rust kernel. `n`/`d` are signed
            // integer strings; the caller guarantees `d != 0`. Returns the rounded integer string.
            self.line("function __phorj_round_div($n, $d, $mode) {");
            self.indent += 1;
            // 1. Normalise the divisor sign so d > 0 (quotient sign unchanged).
            self.line(
                "if (bccomp($d, '0', 0) < 0) { $n = bcmul($n, '-1', 0); $d = bcmul($d, '-1', 0); }",
            );
            // 2. Truncating quotient + dividend-signed remainder.
            self.line("$q = bcdiv($n, $d, 0);");
            self.line("$rem = bcmod($n, $d);");
            self.line("if (bccomp($rem, '0', 0) === 0) { return $q; }");
            // s = sign of the dividend.
            self.line("$s = bccomp($n, '0', 0) > 0 ? '1' : '-1';");
            // half-comparison without doubling: |rem| vs d - |rem| (both >= 0).
            self.line("$absRem = ltrim($rem, '-');");
            self.line("$comp = bcsub($d, $absRem, 0);");
            self.line("$cmp = bccomp($absRem, $comp, 0);"); // -1/0/1
            self.line("$mode = __phorj_round_mode($mode);");
            self.line("switch ($mode) {");
            self.indent += 1;
            self.line("case 'Down': return $q;");
            self.line("case 'Up': return bcadd($q, $s, 0);");
            self.line("case 'Ceiling': return bccomp($n, '0', 0) > 0 ? bcadd($q, '1', 0) : $q;");
            self.line("case 'Floor': return bccomp($n, '0', 0) < 0 ? bcadd($q, '-1', 0) : $q;");
            self.line("case 'HalfUp': return $cmp >= 0 ? bcadd($q, $s, 0) : $q;");
            self.line("case 'HalfDown': return $cmp > 0 ? bcadd($q, $s, 0) : $q;");
            self.line("case 'HalfEven':");
            self.indent += 1;
            self.line("if ($cmp > 0) { return bcadd($q, $s, 0); }");
            self.line("if ($cmp < 0) { return $q; }");
            // exact tie → round to even: bump only if q is currently odd.
            self.line("return bccomp(bcmod($q, '2'), '0', 0) !== 0 ? bcadd($q, $s, 0) : $q;");
            self.indent -= 1;
            self.line("default: throw new \\RuntimeException('unknown RoundingMode');");
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("}");
            // Format a (bounds-checked) unscaled integer string at `scale` fractional digits — the
            // BCMath-padding form, matching the Rust `value::fmt_decimal` (never `-0`).
            self.line("function __phorj_dec_fmt($u, $scale) {");
            self.indent += 1;
            self.line("__phorj_dec_check($u);"); // i128 range guard (same overflow fault)
            self.line("$neg = bccomp($u, '0', 0) < 0;");
            self.line("$digits = ltrim($u, '-');");
            self.line("if ($scale === 0) { $body = $digits; }");
            self.line("else {");
            self.indent += 1;
            self.line("$digits = str_pad($digits, $scale + 1, '0', STR_PAD_LEFT);");
            self.line("$dot = strlen($digits) - $scale;");
            self.line("$body = substr($digits, 0, $dot) . '.' . substr($digits, $dot);");
            self.indent -= 1;
            self.line("}");
            self.line("return ($neg && bccomp($u, '0', 0) !== 0) ? '-' . $body : $body;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_dec_div {
            // `Decimal.div(a, b, scale, mode)`: N = au*10^(sb+scale), D = bu*10^sa; round_div(N,D);
            // format at `scale`. scale<0 / b==0 throw the same bodies as the Rust kernel.
            self.line("function __phorj_dec_div($a, $b, $scale, $mode) {");
            self.indent += 1;
            self.line(
                "if ($scale < 0) { throw new \\RuntimeException('decimal scale out of range'); }",
            );
            self.line("$sa = __phorj_dec_scale($a); $sb = __phorj_dec_scale($b);");
            self.line("$au = __phorj_dec_unscaled($a); $bu = __phorj_dec_unscaled($b);");
            self.line("if (bccomp($bu, '0', 0) === 0) { throw new \\RuntimeException('decimal division by zero'); }");
            self.line("$N = bcmul($au, bcpow('10', (string)($sb + $scale), 0), 0);");
            self.line("$D = bcmul($bu, bcpow('10', (string)$sa, 0), 0);");
            self.line("$u = __phorj_round_div($N, $D, $mode);");
            self.line("return __phorj_dec_fmt($u, $scale);");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_dec_round {
            // `Decimal.round(d, scale, mode)`: up-scale is exact (u*10^Δ), down-scale rounds via
            // round_div(u, 10^Δ). scale<0 throws.
            self.line("function __phorj_dec_round($d, $scale, $mode) {");
            self.indent += 1;
            self.line(
                "if ($scale < 0) { throw new \\RuntimeException('decimal scale out of range'); }",
            );
            self.line("$sd = __phorj_dec_scale($d);");
            self.line("$u = __phorj_dec_unscaled($d);");
            self.line("if ($scale >= $sd) {");
            self.indent += 1;
            self.line("$r = bcmul($u, bcpow('10', (string)($scale - $sd), 0), 0);");
            self.indent -= 1;
            self.line("} else {");
            self.indent += 1;
            self.line("$divisor = bcpow('10', (string)($sd - $scale), 0);");
            self.line("$r = __phorj_round_div($u, $divisor, $mode);");
            self.indent -= 1;
            self.line("}");
            self.line("return __phorj_dec_fmt($r, $scale);");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_float_to_int {
            // `Convert.toInt($f) -> int?`: null on NaN/±∞/out-of-i64-range, else truncate toward zero.
            // The upper bound is the EXCLUSIVE `9.2233720368547758E18` (i64::MAX is not exactly f64-
            // representable); the lower bound is the exact i64::MIN as f64. Matches `value::float_to_int`,
            // and avoids PHP's surprising `(int)NAN == 0`.
            self.line("function __phorj_float_to_int($f) {");
            self.indent += 1;
            // `$t` is the truncate-toward-zero of `$f` (Rust `f64::trunc`): floor for >=0, ceil for <0.
            self.line("if (!is_finite($f)) { return null; }");
            self.line("$t = ($f < 0) ? ceil($f) : floor($f);");
            self.line(
                "return ($t >= -9.2233720368547758E18 && $t < 9.2233720368547758E18) ? (int)$t : null;",
            );
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_trunc {
            // `Convert.truncate($f) -> int`: truncate toward zero, FAULT on NaN/±∞/out-of-i64-range
            // (fault-parity pass — the raw `(int)` cast diverged: Rust saturates, PHP wraps + warns).
            // Same bounds as `__phorj_float_to_int`; throws instead of returning null. Mirrors the Rust
            // `convert_truncate`; the fault text need not match Phorj's (a fault is never a byte-identity
            // example — Invariant 9), only that both legs fault.
            self.line("function __phorj_trunc($f) {");
            self.indent += 1;
            self.line(
                "if (!is_finite($f)) { throw new \\RuntimeException(\"Conversion.truncate: float is out of int range\"); }",
            );
            self.line("$t = ($f < 0) ? ceil($f) : floor($f);");
            self.line(
                "if ($t >= -9.2233720368547758E18 && $t < 9.2233720368547758E18) { return (int)$t; }",
            );
            self.line(
                "throw new \\RuntimeException(\"Conversion.truncate: float is out of int range\");",
            );
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_round {
            // `Convert.round($f) -> int`: round half-away-from-zero (PHP `round()` default ≡ Rust
            // `f.round()`), then range-check the ROUNDED value; FAULT on NaN/±∞/out-of-i64-range.
            // Mirrors the Rust `convert_round`.
            self.line("function __phorj_round($f) {");
            self.indent += 1;
            self.line(
                "if (!is_finite($f)) { throw new \\RuntimeException(\"Conversion.round: float is out of int range\"); }",
            );
            self.line("$r = round($f);");
            self.line(
                "if ($r >= -9.2233720368547758E18 && $r < 9.2233720368547758E18) { return (int)$r; }",
            );
            self.line(
                "throw new \\RuntimeException(\"Conversion.round: float is out of int range\");",
            );
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_dec_to_int {
            // `Convert.decimalToInt($s) -> int?`: the carrier string's integer part (before the dot),
            // truncated toward zero, or null if outside i64 range. Mirrors `value::decimal_to_int`
            // (i128 `unscaled / 10^scale`). Uses `bccomp` against the i64 bounds (BCMath is loaded for
            // decimals already). `(int)"123"` is exact for in-range integer strings.
            self.line("function __phorj_dec_to_int($s) {");
            self.indent += 1;
            self.line("$dot = strpos($s, '.');");
            self.line("$int = $dot === false ? $s : substr($s, 0, $dot);");
            self.line("if ($int === '' || $int === '-') { $int = '0'; }");
            self.line(
                "if (bccomp($int, '9223372036854775807', 0) > 0 || bccomp($int, '-9223372036854775808', 0) < 0) { return null; }",
            );
            self.line("return (int)$int;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_float_to_int_exact {
            // `Convert.floatToIntExact($f) -> int?` (M4 `float as int`): integral-or-null, never a
            // silent truncate. Mirrors `value::float_to_int_exact` (`fmod==0` then the finite+range
            // guard of `__phorj_float_to_int`). `fmod(-3.0,1.0)` is `-0.0` (== 0.0 in PHP), so a
            // negative integral passes; `(int)$f` is exact for an integral in-range float.
            self.line("function __phorj_float_to_int_exact($f) {");
            self.indent += 1;
            self.line("if (!is_finite($f) || fmod($f, 1.0) != 0.0) { return null; }");
            self.line(
                "return ($f >= -9.2233720368547758E18 && $f < 9.2233720368547758E18) ? (int)$f : null;",
            );
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_dec_to_int_exact {
            // `Convert.decimalToIntExact($s) -> int?` (M4 `decimal as int`): integral-or-null. The
            // carrier always renders exactly `scale` fractional digits, so a non-zero fraction
            // (after stripping trailing zeros) means non-integral → null. Mirrors
            // `value::decimal_to_int_exact` (`unscaled % 10^scale != 0`).
            self.line("function __phorj_dec_to_int_exact($s) {");
            self.indent += 1;
            self.line("$dot = strpos($s, '.');");
            self.line("if ($dot !== false) {");
            self.indent += 1;
            self.line("if (rtrim(substr($s, $dot + 1), '0') !== '') { return null; }");
            self.line("$int = substr($s, 0, $dot);");
            self.indent -= 1;
            self.line("} else { $int = $s; }");
            self.line("if ($int === '' || $int === '-') { $int = '0'; }");
            self.line(
                "if (bccomp($int, '9223372036854775807', 0) > 0 || bccomp($int, '-9223372036854775808', 0) < 0) { return null; }",
            );
            self.line("return (int)$int;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_math_gcd {
            // `Math.gcd` — Euclid over the magnitudes (gmp is absent under `php -n`). Mirrors the Rust
            // `math_gcd` native body for every in-range input (the `i64::MIN` magnitude edge faults in
            // Phorj, never reached by a byte-identity example).
            self.line("function __phorj_gcd($a, $b) {");
            self.indent += 1;
            self.line("if ($a < 0) { $a = -$a; }");
            self.line("if ($b < 0) { $b = -$b; }");
            // DEC-255: negating `PHP_INT_MIN` promotes to float — the sole input that overflows the
            // native's `u64`→`i64` result (`gcd(i64::MIN, …) = 2^63`), which faults in phorj. Throw to
            // match; every in-range input stays int and is unaffected.
            self.line(
                "if (is_float($a) || is_float($b)) { throw new \\OverflowException('integer overflow'); }",
            );
            self.line("while ($b != 0) { $t = $b; $b = $a % $b; $a = $t; }");
            self.line("return $a;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_math_clamp {
            // `Math.clamp` — faults on `lo > hi` to match the native (UA-1.7); the fault text need
            // not match Phorj's (a fault is never a byte-identity example — Invariant 9), only that
            // both legs fault. Otherwise `max($lo, min($v, $hi))`, exactly the old inline form.
            self.line("function __phorj_clamp($v, $lo, $hi) {");
            self.indent += 1;
            self.line(
                "if ($lo > $hi) { throw new \\RuntimeException(\"Math.clamp: min ($lo) must not exceed max ($hi)\"); }",
            );
            self.line("return max($lo, min($v, $hi));");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_index {
            // DEC-255: a READ `xs[i]` / `m[k]` faults in phorj on an out-of-range index / missing key.
            // Bare PHP `$o[$k]` silently returns null + a Warning (exit 0) — this helper THROWS instead,
            // so the transpiled program faults identically (non-zero exit). One helper covers List and
            // Map: PHP represents both as arrays, so `array_key_exists` catches an OOB int index AND a
            // missing string key. In-bounds/present reads return the same value → stdout unchanged.
            self.line("function __phorj_index($o, $k) {");
            self.indent += 1;
            self.line("if (!is_array($o) || !array_key_exists($k, $o)) {");
            self.indent += 1;
            self.line(
                "throw new \\OutOfRangeException('index or key not found: ' . (is_int($k) ? (string) $k : \"'\" . $k . \"'\"));",
            );
            self.indent -= 1;
            self.line("}");
            self.line("return $o[$k];");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_checked_arith {
            // DEC-255: phorj int arithmetic is checked (overflow faults). Bare PHP int `+`/`-`/`*`/neg
            // silently PROMOTES to float on overflow (exit 0), so these helpers detect that promotion
            // (`is_float` of an int-int result ⇒ overflow) and THROW, matching phorj's fault. In-range
            // results stay int and pass through unchanged → stdout byte-identical.
            for (name, expr) in [
                ("__phorj_checked_add($a, $b)", "$a + $b"),
                ("__phorj_checked_sub($a, $b)", "$a - $b"),
                ("__phorj_checked_mul($a, $b)", "$a * $b"),
                ("__phorj_checked_neg($a)", "-$a"),
            ] {
                self.line(&format!("function {name} {{"));
                self.indent += 1;
                self.line(&format!("$r = {expr};"));
                self.line(
                    "if (is_float($r)) { throw new \\OverflowException('integer overflow'); }",
                );
                self.line("return $r;");
                self.indent -= 1;
                self.line("}");
            }
        }
        if self.uses_checked_int {
            // DEC-255: `Math.abs`/`Math.integerPower`/`List.sum` return an int in phorj and FAULT on
            // overflow; the equivalent PHP builtins (`abs`/`pow`/`array_sum`) silently PROMOTE to float
            // instead of erroring. This helper receives the builtin's result and THROWS on the promotion
            // (`is_float` of a would-be-int result ⇒ overflow), matching phorj. In-range results are int
            // and pass through unchanged → stdout byte-identical.
            self.line("function __phorj_checked_int($r) {");
            self.indent += 1;
            self.line("if (is_float($r)) { throw new \\OverflowException('integer overflow'); }");
            self.line("return $r;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_debug_render {
            // DEC-238: the PHP TWIN of `native/debug.rs::render` — must mirror the pinned v1 format
            // byte-for-byte on the DETECTABLE domain (null/bool/int/float/string/list/map/instance/
            // enum/closure). ERASED-SHAPE DISCLOSURE (KNOWN_ISSUES): phorj Set/decimal/bytes erase to
            // PHP array/string/string, so their dumps render as the erased shape on this leg — the
            // differential catches any example that hits this (loudly), so no divergence can ship
            // silently. Instance fields are ksort'ed (the ClassLayout sorted order); enum payload
            // props keep declaration order (positional payloads).
            // A FUNCTION (not a `const`): helpers are emitted after the `main();` call, and PHP
            // hoists functions but executes `const` statements positionally — a const here would be
            // undefined while `main` runs.
            let rows: Vec<String> = self
                .debug_enum_rows
                .iter()
                .map(|(cls, en, var)| format!("'{cls}' => ['{en}', '{var}']"))
                .collect();
            self.line(&format!(
                "function __phorj_debug_enums() {{ return [{}]; }}",
                rows.join(", ")
            ));
            self.line("function __phorj_debug_quote($s) {");
            self.indent += 1;
            self.line("return '\"' . strtr($s, [\"\\\\\" => '\\\\\\\\', '\"' => '\\\\\"', \"\\n\" => '\\\\n', \"\\r\" => '\\\\r', \"\\t\" => '\\\\t']) . '\"';");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_debug_wrap($open, $close, $parts, $indent) {");
            self.indent += 1;
            self.line("if (count($parts) === 0) { return $open . $close; }");
            self.line("$spacey = substr($open, -1) === '{';");
            self.line("$inline = $open . ($spacey ? ' ' : '') . implode(', ', $parts) . ($spacey ? ' ' : '') . $close;");
            self.line("if (strlen($inline) <= 60 && strpos($inline, \"\\n\") === false) { return $inline; }");
            self.line(
                "$pad = str_repeat('    ', $indent + 1); $end = str_repeat('    ', $indent);",
            );
            self.line("$body = array_map(function ($p) use ($pad) { return $pad . $p; }, $parts);");
            self.line(
                "return $open . \"\\n\" . implode(\"\\n\", $body) . \"\\n\" . $end . $close;",
            );
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_debug_render($v, $indent = 0, $seen = []) {");
            self.indent += 1;
            self.line("if ($v === null) { return 'null'; }");
            self.line("if (is_bool($v)) { return $v ? 'true' : 'false'; }");
            self.line("if (is_int($v) || is_float($v)) { return __phorj_str($v); }");
            self.line("if (is_string($v)) { return __phorj_debug_quote($v); }");
            self.line("if (is_array($v)) {");
            self.indent += 1;
            self.line("$parts = [];");
            self.line("if (array_is_list($v)) {");
            self.indent += 1;
            self.line(
                "foreach ($v as $e) { $parts[] = __phorj_debug_render($e, $indent + 1, $seen); }",
            );
            self.line("return __phorj_debug_wrap('[', ']', $parts, $indent);");
            self.indent -= 1;
            self.line("}");
            self.line("foreach ($v as $k => $e) { $parts[] = (is_string($k) ? __phorj_debug_quote($k) : (string) $k) . ' => ' . __phorj_debug_render($e, $indent + 1, $seen); }");
            self.line("return __phorj_debug_wrap('{', '}', $parts, $indent);");
            self.indent -= 1;
            self.line("}");
            self.line("if ($v instanceof \\Closure) { return '<function>'; }");
            self.line("if (is_object($v)) {");
            self.indent += 1;

            self.line("if (in_array($v, $seen, true)) { return '*RECURSION*'; }");
            self.line("$seen[] = $v;");
            self.line("$cls = get_class($v);");
            // DEC-263: redact a `Secret<T>` before descending — byte-identical to the Rust surfaces'
            // `Secret(<redacted>)`. The Rust side keys on the (always-bare) Phorj class name "Secret";
            // on the PHP leg `get_class` returns the FQN, which is global `Secret` in single-package mode
            // but `Main\Secret` under multi-package (namespaced) emission — so match the TRAILING segment
            // (`\Secret`), like the `#[\SensitiveParameter]` attribute keys on the Phorj name. Over-redaction
            // (any `Ns\Secret`) is security-safe; `MySecret` etc. do not match.
            self.line(&format!(
                "if ($cls === 'Secret' || substr($cls, -7) === '\\\\Secret') {{ return '{}'; }}",
                crate::value::SECRET_REDACTED
            ));
            self.line("$enums = __phorj_debug_enums();");
            self.line("if (isset($enums[$cls])) {");
            self.indent += 1;
            self.line("[$en, $var] = $enums[$cls];");
            self.line("$parts = [];");
            self.line("foreach (get_object_vars($v) as $p) { $parts[] = __phorj_debug_render($p, $indent + 1, $seen); }");
            self.line("return count($parts) === 0 ? ($en . '.' . $var) : ($en . '.' . $var . '(' . implode(', ', $parts) . ')');");
            self.indent -= 1;
            self.line("}");
            self.line("$vars = get_object_vars($v); ksort($vars);");
            self.line("$parts = [];");
            self.line("foreach ($vars as $k => $p) { $parts[] = $k . ': ' . __phorj_debug_render($p, $indent + 1, $seen); }");
            self.line("if (count($parts) === 0) { return $cls . ' {}'; }");
            self.line("return __phorj_debug_wrap($cls . ' {', '}', $parts, $indent);");
            self.indent -= 1;
            self.line("}");
            self.line("return '<' . gettype($v) . '>';");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_string_format {
            // `String.format` (W3-5/DEC-199) — PHP mirror of the strict `%`-sprintf renderer
            // `text_format`. Each directive's raw text is captured and DELEGATED to PHP's own `sprintf`
            // (so flag/width/precision + `%f` rounding are canonical PHP), with the value chosen to keep
            // phorj semantics: `%s`→`__phorj_str($v)` (interpolation kernel — a bool is "true", not
            // sprintf's "1"), `%d`→int-or-fault, `%f`→int|float-or-fault. Precision on `%s`/`%d`, an
            // unknown conversion, a dangling `%`, and too-few/too-many values all FAULT (a fault is never
            // a byte-identity example — Invariant 9 — only that both legs fault). Byte scan matches the
            // interpreter's char scan (literal runs verbatim, directive bytes ASCII).
            self.line("function __phorj_format($spec, $args) {");
            self.indent += 1;
            self.line("$out = ''; $ai = 0; $i = 0; $n = strlen($spec); $c = count($args);");
            // Positional (`%N$`) bookkeeping (slice 4b) — mirrors the Rust renderer's strict semantics.
            self.line("$sawSeq = false; $sawPos = false; $used = array();");
            self.line("while ($i < $n) {");
            self.indent += 1;
            self.line("$ch = $spec[$i]; $i++;");
            self.line("if ($ch !== '%') { $out .= $ch; continue; }");
            self.line("if ($i < $n && $spec[$i] === '%') { $out .= '%'; $i++; continue; }");
            // Optional `[argnum$]` prefix: a digit run followed by `$` (else those digits are flags/width).
            self.line("$argIdx = -1;");
            self.line("$dj = $i; while ($dj < $n && strpos('0123456789', $spec[$dj]) !== false) { $dj++; }");
            self.line("if ($dj > $i && $dj < $n && $spec[$dj] === '$') { $argIdx = (int)substr($spec, $i, $dj - $i); if ($argIdx < 1) { throw new \\RuntimeException('String.format: positional index must be >= 1'); } $i = $dj + 1; }");
            // The directive body (flags/width/prec/conv) starts AFTER the argnum — `$dir` excludes it so
            // it is a plain single-value directive for `sprintf`.
            self.line("$start = $i;");
            self.line("while ($i < $n && strpos('-0+', $spec[$i]) !== false) { $i++; }");
            // Digit scan via `strpos` into a digit string (like the flag scan above), NOT `ctype_digit`:
            // the ctype extension is not guaranteed under the hermetic `php -n` oracle (it is shared in
            // some builds), and the transpile floor is tier-1 core functions only (extension policy).
            self.line("while ($i < $n && strpos('0123456789', $spec[$i]) !== false) { $i++; }");
            self.line("$hasPrec = false;");
            self.line("if ($i < $n && $spec[$i] === '.') { $hasPrec = true; $i++; while ($i < $n && strpos('0123456789', $spec[$i]) !== false) { $i++; } }");
            self.line(
                "if ($i >= $n) { throw new \\RuntimeException('String.format: dangling %'); }",
            );
            self.line("$conv = $spec[$i]; $i++;");
            self.line("$dir = '%' . substr($spec, $start, $i - $start);");
            self.line("if ($argIdx >= 1) { $sawPos = true; $idx = $argIdx - 1; } else { $sawSeq = true; $idx = $ai; $ai++; }");
            self.line("if ($idx >= $c) { throw new \\RuntimeException('String.format: not enough values'); }");
            self.line("$used[$idx] = true; $v = $args[$idx];");
            self.line("if ($conv === 's') {");
            self.indent += 1;
            // Precision on `%s` (slice 4a) = truncate to N chars, NEVER splitting a UTF-8 char (developer-
            // ruled). We char-truncate here rather than let `sprintf`'s byte-based `%.Ns` split a char, so
            // run≡runvm≡this-helper agree; then delegate width/flags to `sprintf` (the precision is a no-op
            // on the already-≤N-byte string). Manual scan keeps to tier-1 functions (hermetic `php -n`).
            self.line("$s = __phorj_str($v);");
            self.line("if ($hasPrec) {");
            self.indent += 1;
            self.line(
                "$dot = strpos($dir, '.'); $p = $dot === false ? 0 : (int)substr($dir, $dot + 1);",
            );
            self.line("if ($p < strlen($s)) { $cut = $p; while ($cut > 0 && (ord($s[$cut]) & 0xC0) === 0x80) { $cut--; } $s = substr($s, 0, $cut); }");
            self.indent -= 1;
            self.line("}");
            self.line("$out .= sprintf($dir, $s);");
            self.indent -= 1;
            self.line("} elseif ($conv === 'd') {");
            self.indent += 1;
            self.line("if ($hasPrec) { throw new \\RuntimeException('String.format: precision on %d not supported'); }");
            self.line("if (!is_int($v)) { throw new \\RuntimeException('String.format: %d expects an int'); }");
            self.line("$out .= sprintf($dir, $v);");
            self.indent -= 1;
            // Float conversions: `%f`, scientific `%e`/`%E` (slice 3b), shortest-repr `%g`/`%G` (slice
            // 3c) — int|float or fault, precision allowed, delegate the raw directive to PHP's own
            // `sprintf` (canonical rounding + PHP's min-1-digit signed exponent and `%g` branch/strip
            // rules, all of which the Rust renderer reproduces byte-for-byte).
            self.line("} elseif (strpos('feEgG', $conv) !== false) {");
            self.indent += 1;
            self.line("if (!is_int($v) && !is_float($v)) { throw new \\RuntimeException(\"String.format: %$conv expects a number\"); }");
            self.line("$out .= sprintf($dir, (float)$v);");
            self.indent -= 1;
            // Integer-radix conversions (slice 3a): int-or-fault, no precision, delegate the raw directive
            // to PHP `sprintf` (native `%x`/`%X`/`%o`/`%b`, 64-bit unsigned — matches the interpreter's
            // `n as u64`). `strpos` membership test keeps to tier-1 functions (hermetic `php -n`).
            self.line("} elseif (strpos('xXob', $conv) !== false) {");
            self.indent += 1;
            self.line("if ($hasPrec) { throw new \\RuntimeException('String.format: precision on integer-radix conversions not supported'); }");
            self.line("if (!is_int($v)) { throw new \\RuntimeException(\"String.format: %$conv expects an int\"); }");
            self.line("$out .= sprintf($dir, $v);");
            self.indent -= 1;
            self.line("} else { throw new \\RuntimeException(\"String.format: unsupported directive %$conv\"); }");
            self.indent -= 1;
            self.line("}");
            // Strict post-checks (mirror the Rust renderer): no mixing, every value referenced.
            self.line("if ($sawPos && $sawSeq) { throw new \\RuntimeException('String.format: cannot mix positional and sequential directives'); }");
            self.line("for ($k = 0; $k < $c; $k++) { if (empty($used[$k])) { throw new \\RuntimeException('String.format: value not referenced'); } }");
            self.line("return $out;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_math_lcm {
            // `Math.lcm` — `|a|/gcd*|b|` over the magnitudes, inlining Euclid (so it needs no
            // `__phorj_gcd`). Mirrors the Rust `math_lcm` native for every in-range input; `lcm(_, 0)=0`.
            self.line("function __phorj_lcm($a, $b) {");
            self.indent += 1;
            self.line("if ($a === 0 || $b === 0) { return 0; }");
            self.line("if ($a < 0) { $a = -$a; }");
            self.line("if ($b < 0) { $b = -$b; }");
            // DEC-255: negating `PHP_INT_MIN` promotes to float; so does the final product when the lcm
            // exceeds `i64::MAX`. Both fault in the native (`checked_mul` + `i64::try_from`). Throw on the
            // promotion; in-range inputs stay int → byte-identical.
            self.line(
                "if (is_float($a) || is_float($b)) { throw new \\OverflowException('integer overflow'); }",
            );
            self.line("$x = $a; $y = $b;");
            self.line("while ($y != 0) { $t = $y; $y = $x % $y; $x = $t; }");
            self.line("$r = intdiv($a, $x) * $b;");
            self.line("if (is_float($r)) { throw new \\OverflowException('integer overflow'); }");
            self.line("return $r;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_math_number_format {
            // `Math.numberFormat($v, $d)` — digit-string rounding, mirroring `value::number_format`
            // byte-for-byte: round the *shortest-round-trip* decimal string (`__phorj_float`, identical
            // to Rust's `{}` Display) half-away-from-zero by carry — NOT `round($v * 10^$d)` — so the
            // `.5`-boundary divergence is gone (both legs round the intended decimal). Then group by
            // threes and join with `.`. Single-sourced here (NOT PHP's `number_format`).
            self.line("function __phorj_number_format($v, $d) {");
            self.indent += 1;
            self.line("if ($d < 0) { $d = 0; }");
            self.line("if (!is_finite($v)) { return __phorj_float($v); }");
            self.line("$s = __phorj_float($v);");
            self.line("$neg = ($s[0] ?? '') === '-';");
            self.line("if ($neg) { $s = substr($s, 1); }");
            self.line("$dot = strpos($s, '.');");
            self.line("$int = $dot === false ? $s : substr($s, 0, $dot);");
            self.line("$frac = $dot === false ? '' : substr($s, $dot + 1);");
            self.line("$intd = str_split($int);");
            self.line("$fracd = strlen($frac) > 0 ? str_split($frac) : [];");
            self.line("$round_up = isset($fracd[$d]) && ord($fracd[$d]) >= ord('5');");
            self.line("$fracd = array_slice($fracd, 0, $d);");
            self.line("while (count($fracd) < $d) { $fracd[] = '0'; }");
            self.line("if ($round_up) {");
            self.indent += 1;
            self.line("$carry = 1;");
            self.line("for ($i = count($fracd) - 1; $i >= 0 && $carry; $i--) {");
            self.indent += 1;
            self.line("$x = (ord($fracd[$i]) - 48) + $carry; $fracd[$i] = chr(48 + $x % 10); $carry = intdiv($x, 10);");
            self.indent -= 1;
            self.line("}");
            self.line("for ($i = count($intd) - 1; $i >= 0 && $carry; $i--) {");
            self.indent += 1;
            self.line("$x = (ord($intd[$i]) - 48) + $carry; $intd[$i] = chr(48 + $x % 10); $carry = intdiv($x, 10);");
            self.indent -= 1;
            self.line("}");
            self.line("if ($carry) { array_unshift($intd, chr(48 + $carry)); }");
            self.indent -= 1;
            self.line("}");
            self.line("while (count($intd) > 1 && $intd[0] === '0') { array_shift($intd); }");
            self.line(
                "$all_zero = !in_array(true, array_map(fn($c) => $c !== '0', array_merge($intd, $fracd)), true);",
            );
            self.line("$out = ($neg && !$all_zero) ? '-' : '';");
            self.line("$n = count($intd);");
            self.line("for ($i = 0; $i < $n; $i++) {");
            self.indent += 1;
            self.line("if ($i > 0 && ($n - $i) % 3 === 0) { $out .= ','; }");
            self.line("$out .= $intd[$i];");
            self.indent -= 1;
            self.line("}");
            self.line("if ($d > 0) { $out .= '.' . implode('', $fracd); }");
            self.line("return $out;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_uri {
            // `Core.UriModule` (DEC-240) — thin wrappers over PHP 8.5's always-on `Uri\Rfc3986\Uri`
            // (the transpile twin; the Rust kernel is pinned to it byte-for-byte). Fallible
            // operations catch `Uri\InvalidUriException` into the same `<<E>>`-sentinel messages
            // the Rust natives produce; the injected `Uri` prelude classifies them into the typed
            // `UriError` taxonomy. `__phorj_uri` rebuilds the twin object from a stored raw form,
            // which is valid by construction (only parse/withers mint one) — it never throws.
            self.line("function __phorj_uri($raw) {");
            self.indent += 1;
            self.line("return new \\Uri\\Rfc3986\\Uri($raw);");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_uri_parse($s) {");
            self.indent += 1;
            self.line("try { new \\Uri\\Rfc3986\\Uri($s); return $s; }");
            self.line(
                "catch (\\Uri\\InvalidUriException $e) { return '<<E>>' . $e->getMessage(); }",
            );
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_uri_with($raw, $method, $v) {");
            self.indent += 1;
            self.line("try { return __phorj_uri($raw)->$method($v)->toRawString(); }");
            self.line(
                "catch (\\Uri\\InvalidUriException $e) { return '<<E>>' . $e->getMessage(); }",
            );
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_uri_resolve($raw, $r) {");
            self.indent += 1;
            self.line("try { return __phorj_uri($raw)->resolve($r)->toRawString(); }");
            self.line(
                "catch (\\Uri\\InvalidUriException $e) { return '<<E>>' . $e->getMessage(); }",
            );
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_rng {
            // `Core.Random` — the SAME xorshift64 as the Rust kernel (`src/native/random.rs`), so a
            // seeded sequence is byte-identical across all backends. State persists in a by-reference
            // function-static (no global statement needed). `GOLDEN` is the signed-i64 reinterpretation
            // of `0x9E3779B97F4A7C15` (the unsigned literal exceeds PHP_INT_MAX → would parse as float).
            // PHP `>>` is arithmetic, so the `>> 7` masks the 7 sign-extended top bits to emulate Rust's
            // logical `u64 >>`. `next()` masks the high bit (`& PHP_INT_MAX`) for a non-negative i64.
            self.line("function &__phorj_rng_state() {");
            self.indent += 1;
            self.line("static $s = -7046029254386353131;");
            self.line("return $s;");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_rng_step() {");
            self.indent += 1;
            self.line("$r = &__phorj_rng_state();");
            self.line("$x = $r;");
            self.line("$x ^= ($x << 13);");
            self.line("$x ^= (($x >> 7) & 0x01FFFFFFFFFFFFFF);");
            self.line("$x ^= ($x << 17);");
            self.line("$r = $x;");
            self.line("return $x;");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_rng_seed($seed) {");
            self.indent += 1;
            self.line("$r = &__phorj_rng_state();");
            self.line("$r = $seed ^ (-7046029254386353131);");
            self.line("if ($r === 0) { $r = -7046029254386353131; }");
            self.line("return null;");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_rng_next() {");
            self.indent += 1;
            self.line("return __phorj_rng_step() & PHP_INT_MAX;");
            self.indent -= 1;
            self.line("}");
            // `nextFloat`: top 53 bits of the step output / 2^53 → a dyadic `[0.0, 1.0)` fraction,
            // exact in IEEE-754 on both backends (both operands exactly representable).
            self.line("function __phorj_rng_next_float() {");
            self.indent += 1;
            self.line("return ((__phorj_rng_step() & PHP_INT_MAX) >> 10) / 9007199254740992.0;");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_rng_int_between($lo, $hi) {");
            self.indent += 1;
            self.line("$span = $hi - $lo + 1;");
            self.line("return $lo + ((__phorj_rng_step() & PHP_INT_MAX) % $span);");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_clock {
            // `Core.Time` — a freezable process-global clock matching the Rust kernel
            // (`src/native/time.rs`). The frozen value persists in by-reference function-statics (no
            // global statement). `nowMillis()` returns the frozen value when set, else `floor` of
            // `microtime(true)*1000` (integer epoch-millis, matching `SystemTime` truncation). A frozen
            // program is byte-identical across all backends; an unfrozen one reads the wall clock.
            self.line("function &__phorj_now_frozen() {");
            self.indent += 1;
            self.line("static $f = null;");
            self.line("return $f;");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_now_freeze($ms) {");
            self.indent += 1;
            self.line("$f = &__phorj_now_frozen();");
            self.line("$f = $ms;");
            self.line("return null;");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_now_unfreeze() {");
            self.indent += 1;
            self.line("$f = &__phorj_now_frozen();");
            self.line("$f = null;");
            self.line("return null;");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_now_millis() {");
            self.indent += 1;
            self.line("$f = &__phorj_now_frozen();");
            self.line("if ($f !== null) { return $f; }");
            self.line("return (int)(microtime(true) * 1000);");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_regex {
            // `Core.Regex` (Fork A) — the injected `Regex` holds the BARE pattern; `__phorj_regex_delim`
            // wraps it in a collision-free PCRE delimiter + the `u` (Unicode) modifier, matching the
            // `regex`-crate backends on the regular subset. `\d\w\s` are Unicode in the crate and ASCII
            // in PCRE-without-UCP — the one documented edge (KNOWN_ISSUES); shipped examples use ASCII
            // subjects so the byte-identity gate holds. PCRE is PHP core (present under `php -n`).
            self.line("function __phorj_regex_delim($pattern) {");
            self.indent += 1;
            self.line("foreach (['~', '#', '%', '@', '!', '`'] as $d) {");
            self.indent += 1;
            self.line("if (strpos($pattern, $d) === false) { return $d . $pattern . $d . 'u'; }");
            self.indent -= 1;
            self.line("}");
            self.line("return '~' . str_replace('~', '\\\\~', $pattern) . '~u';");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_regex_matches($re, $s) {");
            self.indent += 1;
            self.line("return preg_match(__phorj_regex_delim($re->pattern), $s) === 1;");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_regex_find($re, $s) {");
            self.indent += 1;
            self.line("return preg_match(__phorj_regex_delim($re->pattern), $s, $m) === 1 ? $m[0] : null;");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_regex_find_all($re, $s) {");
            self.indent += 1;
            self.line("preg_match_all(__phorj_regex_delim($re->pattern), $s, $m);");
            self.line("return $m[0];");
            self.indent -= 1;
            self.line("}");
            // Named captures only (the API), in group-index order — matches the crate's
            // `capture_names()` order and a matched-only filter (`is_string` drops numbered keys).
            self.line("function __phorj_regex_find_groups($re, $s) {");
            self.indent += 1;
            self.line(
                "if (preg_match(__phorj_regex_delim($re->pattern), $s, $m) !== 1) { return null; }",
            );
            self.line("$out = [];");
            self.line("foreach ($m as $k => $v) { if (is_string($k)) { $out[$k] = $v; } }");
            self.line("return $out;");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_regex_replace($re, $s, $repl) {");
            self.indent += 1;
            self.line("return preg_replace(__phorj_regex_delim($re->pattern), $repl, $s);");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_regex_split($re, $s) {");
            self.indent += 1;
            self.line("return preg_split(__phorj_regex_delim($re->pattern), $s);");
            self.indent -= 1;
            self.line("}");
        }
        // `Output.capture(fn)` (DEC-220-S3): run the closure with output buffering on and return the
        // captured bytes. `ob_start`/`ob_get_clean` are the exact PHP analogue of the backends'
        // `out.split_off(start)` — the closure's `echo` (from `Output.*`) lands in the buffer, not the
        // page. Byte-identical for the happy path (a printing, returning closure).
        if self.uses_capture {
            self.line("function __phorj_capture($fn) {");
            self.indent += 1;
            self.line("ob_start();");
            self.line("$fn();");
            self.line("return ob_get_clean();");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_list_sort {
            // Natural ascending over a COPY (Phorj lists are immutable). String by byte (`strcmp`,
            // ≡ Rust `String` Ord) — PHP's `<=>` would juggle numeric strings; ints/floats/bools via
            // `<=>` (≡ Rust numeric). `usort` is stable on PHP 8.0+ (≡ Rust `sort_by`).
            self.line("function __phorj_sort($xs) {");
            self.indent += 1;
            self.line("$ys = $xs;");
            self.line("usort($ys, function($a, $b) { return is_string($a) ? strcmp($a, $b) : ($a <=> $b); });");
            self.line("return $ys;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_list_sort_with {
            // Comparator sort over a COPY; the user closure returns the `<=>`-style int directly.
            self.line("function __phorj_sort_with($xs, $cmp) {");
            self.indent += 1;
            self.line("$ys = $xs;");
            self.line("usort($ys, $cmp);");
            self.line("return $ys;");
            self.indent -= 1;
            self.line("}");
        }
        // `List.unique` — first-occurrence-order dedupe by strict equality (≡ Phorj value-equality;
        // NOT `array_unique`, which stringifies).
        if self.uses_list_unique {
            self.line("function __phorj_unique($xs) {");
            self.indent += 1;
            self.line("$out = [];");
            self.line("foreach ($xs as $x) { if (!in_array($x, $out, true)) { $out[] = $x; } }");
            self.line("return $out;");
            self.indent -= 1;
            self.line("}");
        }
        // `List.min` / `List.max` — byte-order compare (string via `strcmp`, NOT PHP `min`/`max`'s
        // numeric-string juggling), null for an empty list. Same `cmp` as `__phorj_sort`.
        if self.uses_list_min {
            self.line("function __phorj_min($xs) {");
            self.indent += 1;
            self.line("if (!count($xs)) { return null; }");
            self.line("$m = $xs[0];");
            self.line("foreach ($xs as $x) { if ((is_string($x) ? strcmp($x, $m) : ($x <=> $m)) < 0) { $m = $x; } }");
            self.line("return $m;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_list_max {
            self.line("function __phorj_max($xs) {");
            self.indent += 1;
            self.line("if (!count($xs)) { return null; }");
            self.line("$m = $xs[0];");
            self.line("foreach ($xs as $x) { if ((is_string($x) ? strcmp($x, $m) : ($x <=> $m)) > 0) { $m = $x; } }");
            self.line("return $m;");
            self.indent -= 1;
            self.line("}");
        }
        // `List.find` / `any` / `all` — SHORT-CIRCUITING (`foreach` + early `return`), so a
        // side-effecting predicate runs on exactly the same prefix as the Rust backends.
        if self.uses_list_find {
            self.line("function __phorj_find($xs, $p) {");
            self.indent += 1;
            self.line("foreach ($xs as $x) { if ($p($x)) { return $x; } }");
            self.line("return null;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_list_any {
            self.line("function __phorj_any($xs, $p) {");
            self.indent += 1;
            self.line("foreach ($xs as $x) { if ($p($x)) { return true; } }");
            self.line("return false;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_list_all {
            self.line("function __phorj_all($xs, $p) {");
            self.indent += 1;
            self.line("foreach ($xs as $x) { if (!$p($x)) { return false; } }");
            self.line("return true;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_map_set {
            // A NEW map (Phorj maps are immutable). `$m` is passed by value, and PHP arrays are
            // copy-on-write, so assigning into it produces a fresh array — the caller's is untouched.
            self.line("function __phorj_map_set($m, $k, $v) {");
            self.indent += 1;
            self.line("$m[$k] = $v;");
            self.line("return $m;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_map_remove {
            self.line("function __phorj_map_remove($m, $k) {");
            self.indent += 1;
            self.line("unset($m[$k]);");
            self.line("return $m;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_list_index_of {
            // PHP `array_search($needle, $xs, true)` returns the int key or `false`; map `false` to
            // `null` for the `int?` return (strict `===` matches Phorj's `eq_val` for scalars).
            self.line("function __phorj_index_of($xs, $needle) {");
            self.indent += 1;
            self.line("$i = array_search($needle, $xs, true);");
            self.line("return $i === false ? null : $i;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_list_last_index_of {
            // PHP `array_keys($xs, $needle, true)` returns every strict-matching key; the last one is
            // the last index (or `null` when none match) — the LAST-match companion to `__phorj_index_of`.
            self.line("function __phorj_last_index_of($xs, $needle) {");
            self.indent += 1;
            self.line("$ks = array_keys($xs, $needle, true);");
            self.line("return empty($ks) ? null : end($ks);");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_text_index_of {
            // PHP `strpos` returns the byte offset or `false` (note: 0 is a valid offset); map only
            // `false` to `null` for the `int?` return.
            self.line("function __phorj_text_index_of($s, $needle) {");
            self.indent += 1;
            self.line("$i = strpos($s, $needle);");
            self.line("return $i === false ? null : $i;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_text_reverse {
            // Reverse by Unicode code point to match Rust `str::chars().rev()` — NOT `strrev`, whose
            // byte reversal corrupts multibyte text (UA-1.2). `preg_split('//u')` yields code points
            // without mbstring (absent under `php -n`); empty string → empty array → "".
            self.line("function __phorj_text_reverse($s) {");
            self.indent += 1;
            self.line(
                "return implode('', array_reverse(preg_split('//u', $s, -1, PREG_SPLIT_NO_EMPTY)));",
            );
            self.indent -= 1;
            self.line("}");
        }
        // `trim`/`trimStart`/`trimEnd` strip Rust's Unicode White_Space set (`char::is_whitespace`) —
        // NOT PHP's `trim`/`ltrim`/`rtrim`, whose default set is ASCII-ish and both misses the
        // multibyte spaces (U+00A0/U+2028/U+3000/…) AND differs even in ASCII (Rust strips form-feed
        // U+000C but not NUL; PHP is the reverse). The class below is exactly that set (verified
        // byte-identical to `str::trim` across the multibyte + form-feed edges). UA-1.1.
        const WS: &str = r"[\x{09}-\x{0D}\x{20}\x{85}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}]";
        if self.uses_text_trim {
            self.line("function __phorj_text_trim($s) {");
            self.indent += 1;
            self.line(&format!("return preg_replace('/^{WS}+|{WS}+$/u', '', $s);"));
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_text_trim_start {
            self.line("function __phorj_text_trim_start($s) {");
            self.indent += 1;
            self.line(&format!("return preg_replace('/^{WS}+/u', '', $s);"));
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_text_trim_end {
            self.line("function __phorj_text_trim_end($s) {");
            self.indent += 1;
            self.line(&format!("return preg_replace('/{WS}+$/u', '', $s);"));
            self.indent -= 1;
            self.line("}");
        }
    }
}
