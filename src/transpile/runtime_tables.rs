//! PHP transpiler — injected-`Json` helper hierarchy + the `Core.Reflection` table.

use super::*;

impl Transpiler {
    /// The `Core.Json` recursive helpers (each gated by its `uses_json_*` flag). They walk the injected
    /// `Json` enum's PHP class hierarchy — mangled variant classes `Null_`/`Bool_`/`Int_`/`Float_` and
    /// bare `Str`/`Arr`/`Obj` (the reserved-name mangle from this slice's prerequisite). Encoding
    /// mirrors the Rust `native::json` kernels byte-for-byte: a string scalar uses native
    /// `json_encode` (authoritative escaping); a float uses `__phorj_float` (positional shortest
    /// round-trip — NOT json's scientific notation, so it matches `run`/`runvm`); structure is
    /// hand-walked. Decoding delegates to native `json_decode` (objects → `stdClass` so `{}` ≠ `[]`),
    /// returning `null` (Phorj `None`) on any parse error, then rebuilds the enum hierarchy.
    pub(super) fn emit_json_helpers(&mut self) {
        // The injected `Json` enum is a `package Main` type, so its PHP variant classes live in
        // `\Main\` in a multi-package (namespaced) program but in the global namespace in a flat one.
        // These runtime helpers are emitted in the nameless global block, so a bare `instanceof Obj`
        // would resolve to `\Obj` (global) and never match the real `\Main\Obj` — every `instanceof`
        // would fall through to the object branch (the multi-package core.json bug). Qualify the
        // variant class references with `\Main\` when namespaced; empty (bare) when flat.
        let jp = if self.namespaced { "\\Main\\" } else { "" };
        if self.uses_json_encode {
            self.line("function __phorj_json_encode($j) {");
            self.indent += 1;
            self.line(&format!(
                "if ($j instanceof {jp}Null_) {{ return \"null\"; }}"
            ));
            self.line(&format!(
                "if ($j instanceof {jp}Bool_) {{ return $j->value ? \"true\" : \"false\"; }}"
            ));
            self.line(&format!(
                "if ($j instanceof {jp}Int_) {{ return (string)$j->value; }}"
            ));
            self.line(&format!(
                "if ($j instanceof {jp}Float_) {{ return __phorj_float($j->value); }}"
            ));
            self.line(&format!(
                "if ($j instanceof {jp}String_) {{ return json_encode($j->value); }}"
            ));
            self.line(&format!("if ($j instanceof {jp}Array_) {{"));
            self.indent += 1;
            self.line("$parts = [];");
            self.line("foreach ($j->items as $x) { $parts[] = __phorj_json_encode($x); }");
            self.line("return \"[\" . implode(\",\", $parts) . \"]\";");
            self.indent -= 1;
            self.line("}");
            self.line("$parts = [];");
            self.line(
                "foreach ($j->entries as $k => $v) { $parts[] = json_encode((string)$k) . \":\" . __phorj_json_encode($v); }",
            );
            self.line("return \"{\" . implode(\",\", $parts) . \"}\";");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_json_pretty {
            self.line(
                "function __phorj_json_encode_pretty($j) { return __phorj_json_pretty($j, 0); }",
            );
            self.line("function __phorj_json_pretty($j, $indent) {");
            self.indent += 1;
            self.line(&format!(
                "if ($j instanceof {jp}Array_ && count($j->items) > 0) {{"
            ));
            self.indent += 1;
            self.line("$pad = str_repeat(\" \", $indent + 4);");
            self.line("$parts = [];");
            self.line(
                "foreach ($j->items as $x) { $parts[] = $pad . __phorj_json_pretty($x, $indent + 4); }",
            );
            self.line(
                "return \"[\\n\" . implode(\",\\n\", $parts) . \"\\n\" . str_repeat(\" \", $indent) . \"]\";",
            );
            self.indent -= 1;
            self.line("}");
            self.line(&format!(
                "if ($j instanceof {jp}Object_ && count($j->entries) > 0) {{"
            ));
            self.indent += 1;
            self.line("$pad = str_repeat(\" \", $indent + 4);");
            self.line("$parts = [];");
            self.line(
                "foreach ($j->entries as $k => $v) { $parts[] = $pad . json_encode((string)$k) . \": \" . __phorj_json_pretty($v, $indent + 4); }",
            );
            self.line(
                "return \"{\\n\" . implode(\",\\n\", $parts) . \"\\n\" . str_repeat(\" \", $indent) . \"}\";",
            );
            self.indent -= 1;
            self.line("}");
            self.line("return __phorj_json_encode($j);");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_json_decode {
            self.line("function __phorj_json_decode($s) {");
            self.indent += 1;
            self.line("$d = json_decode($s);");
            self.line("if (json_last_error() !== JSON_ERROR_NONE) { return null; }");
            self.line("return __phorj_json_build($d);");
            self.indent -= 1;
            self.line("}");
            self.line("function __phorj_json_build($d) {");
            self.indent += 1;
            self.line(&format!("if (is_null($d)) {{ return new {jp}Null_(); }}"));
            self.line(&format!("if (is_bool($d)) {{ return new {jp}Bool_($d); }}"));
            self.line(&format!("if (is_int($d)) {{ return new {jp}Int_($d); }}"));
            self.line(&format!(
                "if (is_float($d)) {{ return new {jp}Float_($d); }}"
            ));
            self.line(&format!(
                "if (is_string($d)) {{ return new {jp}String_($d); }}"
            ));
            self.line("if (is_array($d)) {");
            self.indent += 1;
            self.line("$items = [];");
            self.line("foreach ($d as $x) { $items[] = __phorj_json_build($x); }");
            self.line(&format!("return new {jp}Array_($items);"));
            self.indent -= 1;
            self.line("}");
            self.line("$entries = [];");
            self.line(
                "foreach (get_object_vars($d) as $k => $v) { $entries[(string)$k] = __phorj_json_build($v); }",
            );
            self.line(&format!("return new {jp}Object_($entries);"));
            self.indent -= 1;
            self.line("}");
        }
        // NDJSON (JSON Lines). `parse_lines` reuses `__phorj_json_build` (gated via uses_json_decode);
        // `stringify_lines` reuses `__phorj_json_encode` (uses_json_encode). Split/join + the PHP
        // `trim()` default set match the Rust `json_parse_lines`/`json_stringify_lines` exactly.
        if self.uses_json_parse_lines {
            self.line("function __phorj_json_parse_lines($s) {");
            self.indent += 1;
            self.line("$out = [];");
            self.line("foreach (explode(\"\\n\", $s) as $line) {");
            self.indent += 1;
            self.line("$t = trim($line);");
            self.line("if ($t === \"\") { continue; }");
            self.line("$d = json_decode($t);");
            self.line("if (json_last_error() !== JSON_ERROR_NONE) { return null; }");
            self.line("$out[] = __phorj_json_build($d);");
            self.indent -= 1;
            self.line("}");
            self.line("return $out;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_json_stringify_lines {
            self.line("function __phorj_json_stringify_lines($xs) {");
            self.indent += 1;
            self.line("$parts = [];");
            self.line("foreach ($xs as $x) { $parts[] = __phorj_json_encode($x); }");
            self.line("return implode(\"\\n\", $parts);");
            self.indent -= 1;
            self.line("}");
        }
        // Core.Ini — a hand-rolled simple INI parser matching `ext::ini::natives::ini_parse` line-for-line
        // (NOT PHP `parse_ini_string`, whose type-coercion Phorj deliberately rejects). PHP `trim()`'s
        // default set matches the Rust `trim_matches`; overwriting an existing key keeps its position
        // (PHP array semantics == `build_map`). Returns a PHP array = the `Map<string,string>` value.
        if self.uses_ini_parse {
            self.line("function __phorj_ini_parse($s) {");
            self.indent += 1;
            self.line("$out = [];");
            self.line("$section = \"\";");
            self.line("foreach (explode(\"\\n\", $s) as $line) {");
            self.indent += 1;
            self.line("$t = trim($line);");
            self.line("if ($t === \"\" || $t[0] === \";\" || $t[0] === \"#\") { continue; }");
            self.line("if ($t[0] === \"[\" && substr($t, -1) === \"]\") { $section = trim(substr($t, 1, -1)); continue; }");
            self.line("$eq = strpos($t, \"=\");");
            self.line("if ($eq === false) { continue; }");
            self.line("$key = trim(substr($t, 0, $eq));");
            self.line("$val = trim(substr($t, $eq + 1));");
            self.line("$full = $section === \"\" ? $key : $section . \".\" . $key;");
            self.line("$out[$full] = $val;");
            self.indent -= 1;
            self.line("}");
            self.line("return $out;");
            self.indent -= 1;
            self.line("}");
        }
        // `Core.Option` combinators (Wave B B-2a) — over the injected `Some`/`None` PHP classes (no
        // builtin analog). The receiver is a param, so it is bound once (no double-eval of the call-site
        // argument expression). `map`/`filter` re-wrap; `andThen`'s `$f` itself returns an Option.
        if self.uses_option_map {
            self.line("function __phorj_option_map($o, $f) {");
            self.indent += 1;
            self.line("return $o instanceof Some ? new Some($f($o->value)) : $o;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_option_and_then {
            self.line("function __phorj_option_and_then($o, $f) {");
            self.indent += 1;
            self.line("return $o instanceof Some ? $f($o->value) : $o;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_option_filter {
            self.line("function __phorj_option_filter($o, $f) {");
            self.indent += 1;
            self.line("return ($o instanceof Some && $f($o->value)) ? $o : new None();");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_option_get_or_else {
            self.line("function __phorj_option_get_or_else($o, $d) {");
            self.indent += 1;
            self.line("return $o instanceof Some ? $o->value : $d;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_option_of_nullable {
            self.line("function __phorj_option_of_nullable($v) {");
            self.indent += 1;
            self.line("return $v === null ? new None() : new Some($v);");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_option_to_nullable {
            self.line("function __phorj_option_to_nullable($o) {");
            self.indent += 1;
            self.line("return $o instanceof Some ? $o->value : null;");
            self.indent -= 1;
            self.line("}");
        }
        // `Core.Result` combinators (Wave B B-2b, DEC-185) over the injected `Success`/`Failure` PHP
        // classes (`Success->value`, `Failure->error`). The receiver is a param (bound once, no
        // double-eval). `map`/`mapErr` re-wrap the touched arm and pass the other through unchanged;
        // `andThen`/`orElse` bind (the `$f` itself returns a Result). `toOption` bridges to the Option
        // injection's `Some`/`None`. `isSuccess`/`isFailure` are emitted inline at the call site.
        if self.uses_result_map {
            self.line("function __phorj_result_map($r, $f) {");
            self.indent += 1;
            self.line("return $r instanceof Success ? new Success($f($r->value)) : $r;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_result_map_err {
            self.line("function __phorj_result_map_err($r, $f) {");
            self.indent += 1;
            self.line("return $r instanceof Failure ? new Failure($f($r->error)) : $r;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_result_and_then {
            self.line("function __phorj_result_and_then($r, $f) {");
            self.indent += 1;
            self.line("return $r instanceof Success ? $f($r->value) : $r;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_result_get_or_else {
            self.line("function __phorj_result_get_or_else($r, $d) {");
            self.indent += 1;
            self.line("return $r instanceof Success ? $r->value : $d;");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_result_or_else {
            self.line("function __phorj_result_or_else($r, $f) {");
            self.indent += 1;
            self.line("return $r instanceof Success ? $r : $f($r->error);");
            self.indent -= 1;
            self.line("}");
        }
        if self.uses_result_to_option {
            self.line("function __phorj_result_to_option($r) {");
            self.indent += 1;
            self.line("return $r instanceof Success ? new Some($r->value) : new None();");
            self.indent -= 1;
            self.line("}");
        }
    }

    /// Emit `__phorj_reflect_of($v, $kind)` + its static table, built from the SAME `ClassTables` the
    /// Rust backends read — so `Reflect.interfaces`/`parents`/… are byte-identical by construction
    /// (no reliance on PHP's `class_implements`/`get_class_methods` with their own semantics). A
    /// non-object → `[]`; an unknown class / kind → `[]` (matching the Rust `unwrap_or_default`).
    pub(super) fn emit_reflect_table(&mut self) {
        // The union of every class that appears in any table, in sorted (BTreeMap) order.
        let mut classes: std::collections::BTreeSet<&String> = std::collections::BTreeSet::new();
        for m in [
            &self.class_tables.interfaces,
            &self.class_tables.parents,
            &self.class_tables.methods,
            &self.class_tables.fields,
        ] {
            classes.extend(m.keys());
        }
        let php_list = |names: &[String]| -> String {
            let items: Vec<String> = names
                .iter()
                .map(|n| format!("'{}'", php_escape(n)))
                .collect();
            format!("[{}]", items.join(", "))
        };
        // Build every entry string up front (immutable borrow of `class_tables`), then emit (which
        // borrows `self` mutably via `line`) — avoids a borrow conflict.
        let empty = Vec::new();
        let entries: Vec<String> = classes
            .iter()
            .map(|c| {
                format!(
                    "'{}' => ['interfaces' => {}, 'parents' => {}, 'methods' => {}, 'fields' => {}],",
                    php_escape(c),
                    php_list(self.class_tables.interfaces.get(*c).unwrap_or(&empty)),
                    php_list(self.class_tables.parents.get(*c).unwrap_or(&empty)),
                    php_list(self.class_tables.methods.get(*c).unwrap_or(&empty)),
                    php_list(self.class_tables.fields.get(*c).unwrap_or(&empty)),
                )
            })
            .collect();
        self.line("function __phorj_reflect_of($v, $kind) {");
        self.indent += 1;
        self.line("if (!is_object($v)) { return []; }");
        self.line("static $t = [");
        self.indent += 1;
        for e in entries {
            self.line(&e);
        }
        self.indent -= 1;
        self.line("];");
        self.line("return $t[get_class($v)][$kind] ?? [];");
        self.indent -= 1;
        self.line("}");
    }
}
