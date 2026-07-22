//! `Core.String` native registrations (kernels live in text.rs).

use super::text::*;
use super::text_format::text_format;
use super::*;
use crate::types::Ty;

pub(crate) fn text_natives() -> Vec<NativeFn> {
    let s = || Ty::String;
    vec![
        // W3-5 / DEC-199: `String.format(spec, args)` — PHP-style `%` sprintf. The checker
        // special-cases it (`check_string_format`) for arg-type validation + a compile-time
        // `E-FORMAT-UNSUPPORTED` gate on a literal spec, but (unlike the desugared `Reflect.typeName`)
        // it is a REAL runtime native: `text_format` renders it and `__phorj_format` is the PHP mirror.
        NativeFn {
            module: "Core.String",
            name: "format",
            params: vec![s(), Ty::List(Box::new(s()))],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_format),
            lift_from: &[],
            php: |a| format!("__phorj_format({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.String",
            name: "isEmpty",
            params: vec![s()],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(text_is_empty),
            lift_from: &[],
            php: |a| format!("({}) === ''", parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "trimStart",
            params: vec![s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_trim_start),
            lift_from: &[],
            php: |a| format!("__phorj_text_trim_start({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "trimEnd",
            params: vec![s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_trim_end),
            lift_from: &[],
            php: |a| format!("__phorj_text_trim_end({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "count",
            params: vec![s(), s()],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(text_count),
            lift_from: &["substr_count"],
            php: |a| format!("substr_count({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.String",
            name: "length",
            params: vec![s()],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(text_len),
            lift_from: &["strlen"],
            php: |a| format!("strlen({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "upperCase",
            params: vec![s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_upper),
            lift_from: &["strtoupper"],
            php: |a| format!("strtoupper({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "lowerCase",
            params: vec![s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_lower),
            lift_from: &["strtolower"],
            php: |a| format!("strtolower({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "trim",
            params: vec![s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_trim),
            lift_from: &[],
            php: |a| format!("__phorj_text_trim({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "contains",
            params: vec![s(), s()],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(text_contains),
            lift_from: &["str_contains"],
            php: |a| format!("str_contains({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.String",
            name: "reverse",
            params: vec![s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_reverse),
            // Erases to `__phorj_text_reverse` (code-point reversal), NOT `strrev` (byte reversal
            // mangles multibyte) — UA-1.2. Rust already reverses by `chars()`.
            lift_from: &[],
            php: |a| format!("__phorj_text_reverse({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "equalsIgnoreCase",
            params: vec![s(), s()],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(text_equals_ignore_case),
            lift_from: &[],
            php: |a| format!("strcasecmp({}, {}) === 0", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.String",
            name: "containsIgnoreCase",
            params: vec![s(), s()],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(text_contains_ignore_case),
            lift_from: &[],
            php: |a| format!("stripos({}, {}) !== false", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.String",
            name: "split",
            params: vec![s(), s()],
            ret: Ty::List(Box::new(Ty::String)),
            pure: true,
            eval: NativeEval::Pure(text_split),
            // PHP `explode(separator, string)` — separator first. `explode("")` throws (empty
            // delimiter), matching the Rust empty-separator fault; non-empty splits agree.
            lift_from: &[],
            php: |a| format!("explode({}, {})", parg(a, 1), parg(a, 0)),
        },
        // `characters(string) -> List<string>` — each Unicode code point (parallels `lines`). The named
        // way to split into chars now that `split(s, "")` faults. Code-point-safe via PCRE `/u`.
        NativeFn {
            module: "Core.String",
            name: "characters",
            params: vec![s()],
            ret: Ty::List(Box::new(Ty::String)),
            pure: true,
            eval: NativeEval::Pure(text_characters),
            lift_from: &[],
            php: |a| format!("preg_split('//u', {}, -1, PREG_SPLIT_NO_EMPTY)", parg(a, 0)),
        },
        // `chunk(string, int) -> List<string>` — consecutive pieces of N code points (last shorter);
        // the string twin of `List.chunk`. Code-point-based (NOT PHP str_split bytes — no broken
        // multibyte); `size < 1` faults; empty string → []. Gated `__phorj_str_chunk`.
        NativeFn {
            module: "Core.String",
            name: "chunk",
            params: vec![s(), Ty::Int],
            ret: Ty::List(Box::new(Ty::String)),
            pure: true,
            eval: NativeEval::Pure(text_chunk),
            lift_from: &[],
            php: |a| format!("__phorj_str_chunk({}, {})", parg(a, 0), parg(a, 1)),
        },
        // `capitalize(string) -> string` — ASCII `ucfirst` (Tier-1, byte-identical).
        NativeFn {
            module: "Core.String",
            name: "capitalize",
            params: vec![s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_capitalize),
            lift_from: &["ucfirst"],
            php: |a| format!("ucfirst({})", parg(a, 0)),
        },
        // `capitalizeWords(string) -> string` — PHP `ucwords` (ASCII, first letter of each word).
        NativeFn {
            module: "Core.String",
            name: "capitalizeWords",
            params: vec![s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_capitalize_words),
            lift_from: &["ucwords"],
            php: |a| format!("ucwords({})", parg(a, 0)),
        },
        // `translate(string, from, to) -> string` — PHP `strtr($s, $from, $to)` (byte char-map).
        NativeFn {
            module: "Core.String",
            name: "translate",
            params: vec![s(), s(), s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_translate),
            lift_from: &["strtr"],
            php: |a| format!("strtr({}, {}, {})", parg(a, 0), parg(a, 1), parg(a, 2)),
        },
        // `lines(string) -> List<string>` — split on `\n` (charter §2 subject-first; Tier-1).
        NativeFn {
            module: "Core.String",
            name: "lines",
            params: vec![s()],
            ret: Ty::List(Box::new(Ty::String)),
            pure: true,
            eval: NativeEval::Pure(text_lines),
            lift_from: &[],
            php: |a| format!("explode(\"\\n\", {})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "splitOnce",
            params: vec![s(), s()],
            ret: Ty::List(Box::new(Ty::String)),
            pure: true,
            eval: NativeEval::Pure(text_split_once),
            // PHP `explode(separator, string, 2)` — separator first; the limit-2 yields [head, tail].
            lift_from: &[],
            php: |a| format!("explode({}, {}, 2)", parg(a, 1), parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "join",
            params: vec![Ty::List(Box::new(Ty::String)), s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_join),
            // PHP `implode(glue, array)` — glue first.
            lift_from: &[],
            php: |a| format!("implode({}, {})", parg(a, 1), parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "replace",
            params: vec![s(), s(), s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_replace),
            // PHP `str_replace(search, replace, subject)`.
            lift_from: &[],
            php: |a| {
                format!(
                    "str_replace({}, {}, {})",
                    parg(a, 1),
                    parg(a, 2),
                    parg(a, 0)
                )
            },
        },
        NativeFn {
            module: "Core.String",
            name: "startsWith",
            params: vec![s(), s()],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(text_starts_with),
            lift_from: &["str_starts_with"],
            php: |a| format!("str_starts_with({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.String",
            name: "endsWith",
            params: vec![s(), s()],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(text_ends_with),
            lift_from: &["str_ends_with"],
            php: |a| format!("str_ends_with({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.String",
            name: "codepointLength",
            params: vec![s()],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(text_codepoint_length),
            // PCRE is always built into PHP — `/us` counts codepoints exactly.
            lift_from: &[],
            php: |a| format!("preg_match_all('/./us', {})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "codepoints",
            params: vec![s()],
            ret: Ty::List(Box::new(Ty::Int)),
            pure: true,
            eval: NativeEval::Pure(text_codepoints),
            // Pure-PHP UTF-8 scalar decode (no mbstring): split codepoints via PCRE, decode bytes.
            lift_from: &[],
            php: |a| {
                format!(
                    "array_map(function($c) {{ $b = array_values(unpack('C*', $c)); $n = count($b); if ($n === 1) {{ return $b[0]; }} if ($n === 2) {{ return (($b[0] & 0x1F) << 6) | ($b[1] & 0x3F); }} if ($n === 3) {{ return (($b[0] & 0x0F) << 12) | (($b[1] & 0x3F) << 6) | ($b[2] & 0x3F); }} return (($b[0] & 0x07) << 18) | (($b[1] & 0x3F) << 12) | (($b[2] & 0x3F) << 6) | ($b[3] & 0x3F); }}, preg_split('//u', {}, -1, PREG_SPLIT_NO_EMPTY))",
                    parg(a, 0)
                )
            },
        },
        // DEC-256 native-only tier (per-function ladder: E-TRANSPILE-UNICODE when CALLED).
        NativeFn {
            module: "Core.String",
            name: "unicodeUpper",
            params: vec![s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_unicode_upper),
            lift_from: &[],
            php: |_| "__PHORJ_NATIVE_ONLY_UNICODE__".to_string(),
        },
        NativeFn {
            module: "Core.String",
            name: "unicodeLower",
            params: vec![s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_unicode_lower),
            lift_from: &[],
            php: |_| "__PHORJ_NATIVE_ONLY_UNICODE__".to_string(),
        },
        #[cfg(feature = "unicode")]
        NativeFn {
            module: "Core.String",
            name: "graphemeLength",
            params: vec![s()],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(text_grapheme_length),
            lift_from: &[],
            php: |_| "__PHORJ_NATIVE_ONLY_UNICODE__".to_string(),
        },
        #[cfg(feature = "unicode")]
        NativeFn {
            module: "Core.String",
            name: "graphemes",
            params: vec![s()],
            ret: Ty::List(Box::new(Ty::String)),
            pure: true,
            eval: NativeEval::Pure(text_graphemes),
            lift_from: &[],
            php: |_| "__PHORJ_NATIVE_ONLY_UNICODE__".to_string(),
        },
        NativeFn {
            module: "Core.String",
            name: "levenshtein",
            params: vec![s(), s()],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(text_levenshtein),
            lift_from: &["levenshtein"],
            php: |a| format!("levenshtein({}, {})", parg(a, 0), parg(a, 1)),
        },
        // DEC-243: PHP-parity `similar_text()` count; the by-reference `$percent` twin is the
        // separate VALUE-returning `similarTextPercent` below (no by-ref params in Phorj).
        NativeFn {
            module: "Core.String",
            name: "similarText",
            params: vec![s(), s()],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(text_similar),
            lift_from: &["similar_text"],
            php: |a| format!("similar_text({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.String",
            name: "similarTextPercent",
            params: vec![s(), s()],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(text_similar_percent),
            // PHP exposes the percent only via the by-ref third arg — an IIFE keeps this a pure
            // Tier-1 expression (no gated helper needed; META-7 trade disclosed in the register).
            lift_from: &[],
            php: |a| {
                format!(
                    "(function($a, $b) {{ $p = 0.0; if ($a !== '' || $b !== '') {{ similar_text($a, $b, $p); }} return $p; }})({}, {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.String",
            name: "repeat",
            params: vec![s(), Ty::Int],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_repeat),
            lift_from: &["str_repeat"],
            php: |a| format!("str_repeat({}, {})", parg(a, 0), parg(a, 1)),
        },
        // `parseInt(string) -> int?` — None on a non-integer (the first optional-return native). PHP
        // erases to the gated `__phorj_parse_int` helper (set in `transpile/call.rs`), which mirrors
        // Rust's `i64::from_str` exactly: optional sign, base-10 digits (leading zeros OK), in i64
        // range, no surrounding whitespace; anything else is `null` (Phorj `None`).
        NativeFn {
            module: "Core.String",
            name: "parseInt",
            params: vec![s()],
            ret: Ty::Optional(Box::new(Ty::Int)),
            pure: true,
            eval: NativeEval::Pure(text_parse_int),
            lift_from: &[],
            php: |a| format!("__phorj_parse_int({})", parg(a, 0)),
        },
        // `parseBool(string) -> bool?` (M4 `string as bool`) — strict `"true"`/`"false"` only; never
        // PHP truthiness. Arrow-IIFE PHP carrier = single-eval of the operand.
        NativeFn {
            module: "Core.String",
            name: "parseBool",
            params: vec![s()],
            ret: Ty::Optional(Box::new(Ty::Bool)),
            pure: true,
            eval: NativeEval::Pure(text_parse_bool),
            lift_from: &[],
            php: |a| {
                format!(
                    "(fn($__b) => $__b === 'true' ? true : ($__b === 'false' ? false : null))({})",
                    parg(a, 0)
                )
            },
        },
        // `parseFloat(string, bool permissive = false) -> float?` — the motivating native for M4
        // default parameters (the `permissive` flag defaults to strict). Rejects inf/nan in both
        // modes; permissive also accepts a lone leading/trailing dot. Gated `__phorj_parse_float`.
        NativeFn {
            module: "Core.String",
            name: "parseFloat",
            params: vec![s(), Ty::Bool],
            ret: Ty::Optional(Box::new(Ty::Float)),
            pure: true,
            eval: NativeEval::Pure(text_parse_float),
            lift_from: &[],
            php: |a| format!("__phorj_parse_float({}, {})", parg(a, 0), parg(a, 1)),
        },
        // `padLeft`/`padRight(string, int, string) -> string` — PHP `str_pad` (byte-based, no mbstring).
        NativeFn {
            module: "Core.String",
            name: "padLeft",
            params: vec![s(), Ty::Int, s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_pad_left),
            lift_from: &[],
            php: |a| {
                format!(
                    "str_pad({}, {}, {}, STR_PAD_LEFT)",
                    parg(a, 0),
                    parg(a, 1),
                    parg(a, 2)
                )
            },
        },
        NativeFn {
            module: "Core.String",
            name: "padRight",
            params: vec![s(), Ty::Int, s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_pad_right),
            // STR_PAD_RIGHT is the default, but pass it explicitly for symmetry/legibility.
            lift_from: &[],
            php: |a| {
                format!(
                    "str_pad({}, {}, {}, STR_PAD_RIGHT)",
                    parg(a, 0),
                    parg(a, 1),
                    parg(a, 2)
                )
            },
        },
        // `indexOf(string, string) -> int?` — gated `__phorj_text_index_of` (PHP `strpos` → null).
        NativeFn {
            module: "Core.String",
            name: "indexOf",
            params: vec![s(), s()],
            ret: Ty::Optional(Box::new(Ty::Int)),
            pure: true,
            eval: NativeEval::Pure(text_index_of),
            lift_from: &[],
            php: |a| format!("__phorj_text_index_of({}, {})", parg(a, 0), parg(a, 1)),
        },
        // `lastIndexOf(string, string) -> int?` — the last occurrence (PHP `strrpos` → null via a
        // single-eval arrow-IIFE, like `parseBool`; no helper-file edit).
        NativeFn {
            module: "Core.String",
            name: "lastIndexOf",
            params: vec![s(), s()],
            ret: Ty::Optional(Box::new(Ty::Int)),
            pure: true,
            eval: NativeEval::Pure(text_last_index_of),
            lift_from: &[],
            php: |a| {
                format!(
                    "(fn($__h, $__n) => ($__p = strrpos($__h, $__n)) === false ? null : $__p)({}, {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        // `removePrefix`/`removeSuffix(string, string) -> string` — drop an affix if present (PHP
        // `str_starts_with`/`str_ends_with` + `substr`, single-eval arrow-IIFE).
        NativeFn {
            module: "Core.String",
            name: "removePrefix",
            params: vec![s(), s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_remove_prefix),
            lift_from: &[],
            php: |a| {
                format!(
                    "(fn($__s, $__p) => str_starts_with($__s, $__p) ? substr($__s, strlen($__p)) : $__s)({}, {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.String",
            name: "removeSuffix",
            params: vec![s(), s()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_remove_suffix),
            lift_from: &[],
            php: |a| {
                format!(
                    "(fn($__s, $__p) => str_ends_with($__s, $__p) ? substr($__s, 0, strlen($__s) - strlen($__p)) : $__s)({}, {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        // `substring(string, int, int) -> string` — PHP `substr` (byte-indexed; negatives from end).
        NativeFn {
            module: "Core.String",
            name: "substring",
            params: vec![s(), Ty::Int, Ty::Int],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(text_substring),
            lift_from: &["substr"],
            php: |a| format!("substr({}, {}, {})", parg(a, 0), parg(a, 1), parg(a, 2)),
        },
    ]
}

/// `Text.parseInt` — parse a base-10 integer, or `None` (`Value::Null`) when the whole string is not
/// a valid `i64`. Delegates to Rust's `i64::from_str` (the single source of truth; the PHP helper is
/// written to match it byte-for-byte).
fn text_parse_int(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(s.parse::<i64>().map_or(Value::Null, Value::Int)),
        _ => Err("String.parseInt expects (string)".into()),
    }
}

/// `parseBool(string) -> bool?` (M4 as-matrix S3, the `string as bool` kernel) — **strict**: only the
/// literals `"true"`/`"false"` parse; anything else (incl. `"1"`, `"yes"`, `""`) is `null`. Phorj
/// deliberately does NOT inherit PHP's `(bool)"0" == false` / `(bool)"false" == true` truthiness — the
/// #1 string-cast footgun. The PHP carrier is an arrow-IIFE matching this exactly.
fn text_parse_bool(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(match s.as_str() {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => Value::Null,
        }),
        _ => Err("String.parseBool expects (string)".into()),
    }
}

// ---- Core.File ----------------------------------------------------------------------------------
// Filesystem natives (std::fs ↔ PHP file builtins, D-L9). `read` returns `string?` — `null` on any
// failure (missing file, permission, non-UTF-8) — exercising S2 null-safety (`??` / `if (var x =
// read(p))`). DETERMINISM: a file *read* is byte-identical across backends iff every backend reads
// the same bytes, so file examples read a **committed fixture**; `write` is a non-deterministic side
// effect and is excluded from the byte-identity-gated example set (it is unit-tested with a temp
// file). The run↔runvm spine shares the same `eval`, so it is always identical regardless.

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
