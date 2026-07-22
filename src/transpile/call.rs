//! PHP transpiler — call (M-Decomp W4). See mod.rs for the struct + core + entry points.

use super::*;

impl Transpiler {
    pub(super) fn emit_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<String, String> {
        if let Expr::Ident(name, _) = callee {
            // Fault intrinsics (M-faults 2a) → PHP exceptions (a `throw` expression, PHP 8.0+). The
            // fault text is single-sourced on `FaultMsg::message` so it reads identically to the
            // backends (panics aren't runnable examples, so this isn't oracle-compared, but it stays
            // valid, faithful PHP).
            use crate::chunk::FaultMsg;
            match name.as_str() {
                "panic" => {
                    let m = FaultMsg::Panic(lit_arg(args.first())).message();
                    return Ok(format!(
                        "throw new \\RuntimeException(\"{}\")",
                        php_escape(&m)
                    ));
                }
                "todo" => {
                    return Ok(format!(
                        "throw new \\RuntimeException(\"{}\")",
                        php_escape(&FaultMsg::Todo.message())
                    ));
                }
                "unreachable" => {
                    return Ok(format!(
                        "throw new \\LogicException(\"{}\")",
                        php_escape(&FaultMsg::Unreachable.message())
                    ));
                }
                "assert" => {
                    let c = self.emit_expr(&args[0])?;
                    let m = FaultMsg::Assert(lit_arg(args.get(1))).message();
                    return Ok(format!(
                        "({c} ? null : throw new \\RuntimeException(\"{}\"))",
                        php_escape(&m)
                    ));
                }
                _ => {}
            }
            let argv = self.emit_args(args)?;
            // Enum variant or class construction → `new`; mirrors the evaluator's dispatch. A
            // cross-package class name is mangled (FQN); a variant subclass lives in its enum's
            // namespace, so a cross-package variant is constructed fully-qualified too.
            if self.variants.contains(name) {
                return Ok(format!("new {}({argv})", self.variant_ref(name)));
            }
            // M8.5: construct a foreign PHP class as `new \Name(…)` (global).
            if self.foreign_classes.contains(name) {
                return Ok(format!("new \\{name}({argv})"));
            }
            if self.classes.contains(name) {
                return Ok(format!("new {}({argv})", php_type_ref(name)));
            }
            // A closure stored in a local variable (e.g. a `\Closure` parameter or a `var`-bound
            // lambda) must be called as `$f(…)` — PHP requires the `$` sigil on variable-call sites.
            if self.is_local(name) {
                return Ok(format!("${name}({argv})"));
            }
            // M8.5: a foreign `declare function` call → the global PHP form `\name(…)`, so it resolves
            // to the PHP builtin/library function even inside a namespace block.
            if self.foreign_fns.contains(name) {
                return Ok(format!("\\{name}({argv})"));
            }
            // A resolved cross-package call carries a mangled (`\`-bearing) name → emit it
            // fully-qualified (leading `\`). A bare name (same-`Main`-namespace call) stays bare.
            if self.namespaced && name.contains('\\') {
                return Ok(format!("\\{name}({argv})"));
            }
            return Ok(format!("{name}({argv})")); // free function
        }
        if let Expr::Member { .. } = callee {
            return self.emit_member_call(callee, args);
        }
        // A lambda literal OR any general expression that evaluates to a function value — `adder()(x)`
        // (call a returned closure), `fns[i](x)`, `(c ? f : g)(x)`. PHP invokes a callable value with
        // `(<expr>)(args)`. The checker has verified the callee is function-typed; mirrors the VM's
        // `CallValue` path and the interpreter, so all three backends agree.
        let f = self.emit_expr(callee)?;
        let argv = self.emit_args(args)?;
        Ok(format!("({f})({argv})"))
    }

    pub(super) fn emit_args(&mut self, args: &[Expr]) -> Result<String, String> {
        let parts: Result<Vec<_>, _> = args.iter().map(|a| self.emit_expr(a)).collect();
        Ok(parts?.join(", "))
    }

    pub(super) fn emit_member_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
    ) -> Result<String, String> {
        if let Expr::Member {
            object, name, safe, ..
        } = callee
        {
            // Namespaced native call: `console.println(x)` → the native's PHP erasure (M3 Wave 1).
            // Resolved through the import map (the transpiler has no variable scope to tell a
            // qualifier from a value; the checker rejects a local shadowing an imported qualifier,
            // so a same-spelled value receiver is impossible).
            if !*safe {
                if let Expr::Ident(q, _) = &**object {
                    // Resolve the native: normally via the import map (no scope to tell a qualifier
                    // from a value). A primitive `as`-cast rewrite (M4 as-matrix) synthesizes an
                    // *un-imported* `Convert.*`/`Text.*` call; resolve those two leaves directly. Safe
                    // because (a) the checker rejects any user-written un-imported stdlib call
                    // (E-UNKNOWN-IDENT), and (b) we skip the fallback when `q` is a user class — so a
                    // user `Convert`/`Text` static call still wins.
                    let cast_leaf = matches!(q.as_str(), "Conversion" | "String" | "Decimal")
                        && !self.classes.contains(q);
                    let resolved = self
                        .imports
                        .get(q)
                        .and_then(|m| crate::native::index_of(m, name))
                        .or_else(|| {
                            cast_leaf
                                .then(|| crate::native::index_of_by_leaf(q, name))
                                .flatten()
                        });
                    if let Some(idx) = resolved {
                        let argv: Vec<String> = args
                            .iter()
                            .map(|a| self.emit_expr(a))
                            .collect::<Result<_, _>>()?;
                        // `Reflect.kind` emits the gated `__phorj_kind` helper; a native's `php`
                        // closure has no `&mut self` to set the flag, so set it here (the established
                        // gated-helper pattern — see `emit_runtime_helpers`).
                        let nat = &crate::native::registry()[idx];
                        // `Output.capture` erases to the gated `__phorj_capture` helper (DEC-220-S3).
                        if nat.module == "Core.Output" && nat.name == "capture" {
                            self.uses_capture = true;
                        }
                        if nat.module == "Core.Native.Debug" && nat.name == "render" {
                            self.uses_debug_render = true;
                            // Scalars render through the interpolation kernel twin.
                            self.uses_str = true;
                        }
                        if nat.module == "Core.String" && nat.name == "format" {
                            self.uses_string_format = true;
                            // `__phorj_format`'s `%s` stringifies via `__phorj_str` (the same kernel
                            // interpolation uses), so gate it in too.
                            self.uses_str = true;
                        }
                        if nat.module == "Core.Reflection" {
                            match nat.name {
                                "kind" => self.uses_reflect_kind = true,
                                "className" => self.uses_reflect_class_name = true,
                                "interfaces" | "parents" | "methods" | "fields" => {
                                    self.uses_reflect_tables = true;
                                }
                                _ => {}
                            }
                        }
                        if nat.module == "Core.Json" {
                            match nat.name {
                                // `stringifyPretty` reuses `__phorj_json_encode` for scalars/empties,
                                // so it gates both the pretty and the compact helper.
                                "stringify" => self.uses_json_encode = true,
                                "stringifyPretty" => {
                                    self.uses_json_pretty = true;
                                    self.uses_json_encode = true;
                                }
                                "parse" => self.uses_json_decode = true,
                                // NDJSON: parseLines reuses __phorj_json_build (decode); stringifyLines
                                // reuses __phorj_json_encode. Each gates its own line helper too.
                                "parseLines" => {
                                    self.uses_json_decode = true;
                                    self.uses_json_parse_lines = true;
                                }
                                "stringifyLines" => {
                                    self.uses_json_encode = true;
                                    self.uses_json_stringify_lines = true;
                                }
                                _ => {}
                            }
                        }
                        if nat.module == "Core.Ini" && nat.name == "parse" {
                            self.uses_ini_parse = true;
                        }
                        if nat.module == "Core.Option" {
                            match nat.name {
                                "map" => self.uses_option_map = true,
                                "andThen" => self.uses_option_and_then = true,
                                "filter" => self.uses_option_filter = true,
                                "getOrElse" => self.uses_option_get_or_else = true,
                                "ofNullable" => self.uses_option_of_nullable = true,
                                "toNullable" => self.uses_option_to_nullable = true,
                                _ => {}
                            }
                        }
                        if nat.module == "Core.Result" {
                            match nat.name {
                                "map" => self.uses_result_map = true,
                                "mapErr" => self.uses_result_map_err = true,
                                "andThen" => self.uses_result_and_then = true,
                                "getOrElse" => self.uses_result_get_or_else = true,
                                "orElse" => self.uses_result_or_else = true,
                                "toOption" => self.uses_result_to_option = true,
                                // isSuccess/isFailure emit an inline `instanceof` (no helper).
                                _ => {}
                            }
                        }
                        if nat.module == "Core.String" {
                            match nat.name {
                                "parseInt" => self.uses_text_parse_int = true,
                                "indexOf" => self.uses_text_index_of = true,
                                "reverse" => self.uses_text_reverse = true,
                                "trim" => self.uses_text_trim = true,
                                "trimStart" => self.uses_text_trim_start = true,
                                "trimEnd" => self.uses_text_trim_end = true,
                                "parseFloat" => self.uses_text_parse_float = true,
                                "chunk" => self.uses_text_chunk = true,
                                _ => {}
                            }
                        }
                        if nat.module == "Core.List" {
                            match nat.name {
                                "sort" | "sortDescending" => self.uses_list_sort = true,
                                "sortWith" => self.uses_list_sort_with = true,
                                "takeWhile" => self.uses_list_take_while = true,
                                "dropWhile" => self.uses_list_drop_while = true,
                                "groupBy" => self.uses_list_group_by = true,
                                "indexOf" => self.uses_list_index_of = true,
                                "lastIndexOf" => self.uses_list_last_index_of = true,
                                "unique" => self.uses_list_unique = true,
                                "difference" => self.uses_list_difference = true,
                                "intersection" => self.uses_list_intersection = true,
                                "min" => self.uses_list_min = true,
                                "max" => self.uses_list_max = true,
                                "minBy" => self.uses_list_min_by = true,
                                "maxBy" => self.uses_list_max_by = true,
                                "find" => self.uses_list_find = true,
                                "any" => self.uses_list_any = true,
                                "none" => self.uses_list_none = true,
                                "all" => self.uses_list_all = true,
                                _ => {}
                            }
                        }
                        if nat.module == "Core.Map" {
                            match nat.name {
                                "set" => self.uses_map_set = true,
                                "remove" => self.uses_map_remove = true,
                                _ => {}
                            }
                        }
                        // `Convert.*` gated helpers: `toString` reuses `__phorj_str`; `toInt` /
                        // `decimalToInt` each define their own edge-safe helper (M-NUM S3).
                        if nat.module == "Core.Conversion" {
                            match nat.name {
                                "toString" => self.uses_str = true,
                                "toInt" => self.uses_float_to_int = true,
                                "truncate" => self.uses_trunc = true,
                                "round" => self.uses_round = true,
                                "decimalToInt" => self.uses_dec_to_int = true,
                                "floatToIntExact" => self.uses_float_to_int_exact = true,
                                "decimalToIntExact" => self.uses_dec_to_int_exact = true,
                                // `float as decimal` reuses the float-display + decimal-parse helpers.
                                "floatToDecimal" => {
                                    self.uses_str = true;
                                    self.uses_dec_of = true;
                                }
                                _ => {}
                            }
                        }
                        // `Math.gcd`/`numberFormat` erase to gated `__phorj_*` helpers (M-NUM S4):
                        // gmp is absent under `php -n`, and `number_format` is single-sourced with the
                        // Rust kernel to dodge PHP's `-0`/locale quirks. The rest of `Core.Math` erases
                        // to a same-named PHP builtin (no helper).
                        if nat.module == "Core.Math" {
                            match nat.name {
                                "gcd" => self.uses_math_gcd = true,
                                "clamp" => self.uses_math_clamp = true,
                                "lcm" => self.uses_math_lcm = true,
                                "numberFormat" => self.uses_math_number_format = true,
                                _ => {}
                            }
                        }
                        // `Core.Random` erases to gated `__phorj_rng_*` helpers (2026-06-27): a
                        // hand-rolled xorshift64 byte-identical to the Rust kernel (so seeded output
                        // matches across all backends — Random is no longer quarantined).
                        if nat.module == "Core.Random" {
                            self.uses_rng = true;
                        }
                        // `Core.Native.Uri` erases to the gated `__phorj_uri*` helpers (DEC-240): thin
                        // wrappers over PHP 8.5's always-on `Uri\Rfc3986\Uri` — the extension IS
                        // the implementation on this leg; the Rust kernel is pinned to it. The
                        // DEC-279 percent-encoding rows (former `Core.Url`, now in this module) have
                        // self-contained `urlencode`-family emitters and need no helper block.
                        if nat.module == "Core.Native.Uri"
                            && !matches!(
                                nat.name,
                                "encodeForm" | "encodeComponent" | "decodeForm" | "decodeComponent"
                            )
                        {
                            self.uses_uri = true;
                        }
                        // `Core.Regex` erases to gated `__phorj_regex_*` helpers (Fork A, 2026-06-28):
                        // the injected `Regex` holds the bare pattern; the helpers build a
                        // collision-free `~…~u` PCRE form and delegate to `preg_*`.
                        if nat.module == "Core.Regex" {
                            self.uses_regex = true;
                        }
                        // `Core.Time` erases to gated `__phorj_now_*` helpers (M-TIME, 2026-06-28): a
                        // freezable process-global clock hand-rolled to match the Rust kernel, so a frozen
                        // program is byte-identical across all backends.
                        if nat.module == "Core.Time" {
                            self.uses_clock = true;
                        }
                        // `Core.Log` / `Core.Native.Log` (DEC-317 Log-v2) route through the gated
                        // `__phorj_log_*` helpers: a `$GLOBALS`-held channel config + an emit kernel
                        // hand-rolled to the same deterministic line/json contract as the Rust one.
                        if nat.module == "Core.Log" || nat.module == "Core.Native.Log" {
                            self.uses_log = true;
                        }
                        // `Decimal.*` erases to gated `__phorj_dec_*` helpers (M-NUM S1/S2).
                        if nat.module == "Core.Decimal" {
                            match nat.name {
                                "of" => self.uses_dec_of = true,
                                "divide" => self.uses_dec_div = true,
                                "round" => self.uses_dec_round = true,
                                _ => {}
                            }
                        }
                        let php = (nat.php)(&argv);
                        // DEC-256 per-function ladder: a native-only String function (Unicode
                        // case/graphemes — PHP needs forbidden ini extensions) hard-errors when
                        // actually CALLED; the module import alone stays transpilable.
                        if php.contains("__PHORJ_NATIVE_ONLY_UNICODE__") {
                            return Err(format!(
                                "E-TRANSPILE-UNICODE: `String.{}` is native-only — Unicode case/grapheme functions need PHP's mbstring/intl ini extensions, which the transpile rules forbid (THE LADDER RULE, DEC-256). Run this program with `phg run`; the codepoint tier (`String.codepointLength`/`codepoints`) transpiles.",
                                nat.name
                            ));
                        }
                        // Inside a namespace block a bare `strlen(...)` would resolve to
                        // `CurrentNs\strlen`; emit `\strlen(...)` for global-function natives (M5-8).
                        let php = if self.namespaced && looks_like_global_call(&php) {
                            format!("\\{php}")
                        } else {
                            php
                        };
                        // DEC-255: three natives return an int the underlying PHP builtin SILENTLY
                        // promotes to float on overflow (`Math.abs` at `i64::MIN`, `Math.integerPower`
                        // on overflow/negative-exponent, `List.sum` on overflow) — exactly where phorj
                        // faults. Wrap the (already namespace-resolved) builtin in `__phorj_checked_int`,
                        // which throws on the promotion (`is_float`), so the PHP leg faults in lockstep.
                        return Ok(
                            if matches!(
                                (nat.module, nat.name),
                                ("Core.Math", "abs")
                                    | ("Core.Math", "integerPower")
                                    | ("Core.List", "sum")
                            ) {
                                self.uses_checked_int = true;
                                let bs = if self.namespaced { "\\" } else { "" };
                                format!("{bs}__phorj_checked_int({php})")
                            } else {
                                php
                            },
                        );
                    }
                }
            }
            // DEC-302 enum static methods `Enum.cases()`/`from(x)`/`tryFrom(x)` → PHP
            // `Enum::method(args)` (representation B — the methods are emitted on the base class by
            // `emit_enum`). `php_type_ref` gives the same base-class reference `emit_enum` declared.
            if !*safe {
                if let Expr::Ident(en, _) = &**object {
                    if !self.is_local(en)
                        && self.enums.contains(en)
                        && matches!(name.as_str(), "cases" | "from" | "tryFrom")
                    {
                        let a = self.emit_args(args)?;
                        return Ok(format!("{}::{name}({a})", php_type_ref(en)));
                    }
                }
            }
            // Static method call `ClassName.method(args)` (slice B0) → PHP `Class::method(args)`.
            // The head is a class name (not a local), resolved after the native path (matching the
            // other backends' ordering); `php_type_ref` gives the same reference `new` uses (FQN in
            // namespaced mode).
            if !*safe {
                if let Expr::Ident(cls, _) = &**object {
                    // M8.5: a static call on a foreign class → `\Name::method(…)` (global).
                    if !self.is_local(cls) && self.foreign_classes.contains(cls) {
                        let a = self.emit_args(args)?;
                        return Ok(format!("\\{cls}::{name}({a})"));
                    }
                    if !self.is_local(cls) && self.classes.contains(cls) {
                        let a = self.emit_args(args)?;
                        return Ok(format!("{}::{name}({a})", php_type_ref(cls)));
                    }
                }
            }
            let o = self.emit_expr(object)?;
            let a = self.emit_args(args)?;
            let arrow = if *safe { "?->" } else { "->" };
            return Ok(format!("{o}{arrow}{name}({a})"));
        }
        Err("transpile error: bad member call".into())
    }
}
