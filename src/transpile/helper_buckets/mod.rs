//! **THE `__phorj_*` helper classification registry** (DEC-377 — Wave 2.2).
//!
//! The developer's rule: *a `__phorj_*` helper may exist ONLY when PHP cannot do natively what phorj
//! does.* DEC-377 sharpened it into three buckets, and the audit it demanded was OWED for four days —
//! *"nobody currently knows which bucket each is in, which is the same unverified-claims pattern this
//! whole agenda has been fixing."* This file is that audit, plus the ratchet that stops it decaying
//! again (DEC-356's inventory decayed 17→26 while it sat unbuilt; this one cannot).
//!
//! **The buckets.**
//! 1. **Semantic necessity** — PHP genuinely differs, so the naive native call would be WRONG.
//! 2. **No single-expression equivalent** — PHP has the pieces but not in one expression (a `try`/`catch`
//!    or a temporary is needed). DEC-377 requires the reason STATED per helper; families are annotated
//!    below.
//! 3. **Convenience/DRY only** — must be INLINED.
//!
//! ## The headline finding: bucket 3 is EMPTY
//!
//! DEC-412's heuristic first pass listed **17 bucket-3 candidates** to inline. Reading all 17 refutes
//! every one, and the two findings attached to that list are both wrong:
//!
//! - **`uri_parse` / `uri_resolve` / `uri_with` are NOT "pure waste".** The register suspected they
//!   reimplement PHP 8.5's URI extension. They *use* it — `new \Uri\Rfc3986\Uri($raw)`. What they add
//!   is the exception→sentinel bridge phorj's `Result` surface needs, and that needs `try`/`catch`, which
//!   **is not an expression in PHP** [Verified under php-8.5.8: `$x = try {…} catch {…};` is a parse
//!   error, and `@` does not suppress an exception]. Bucket 2, justified.
//! - **The `text_*` group is NOT "ASCII-oriented and inlinable".** It is the opposite: these exist
//!   *because* PHP's native calls are byte-oriented and therefore wrong. [Verified under php-8.5.8:
//!   `trim()` leaves U+00A0/U+2009 untouched where the helper's `/u` class strips them; `strrev("héllo")`
//!   produces mojibake where the codepoint-wise form does not.] Bucket 1.
//! - **`__phorj_trim` does not exist.** Zero `function __phorj_trim(` definitions — the list of 17
//!   contains a phantom, an artifact of prefix-matching `__phorj_trim_start`.
//!
//! So DEC-377's "must be INLINED" action applies to **nothing**. The deliverable is the classification
//! and the ratchet, not a code change.
//!
//! ## The count was wrong three times
//!
//! DEC-377 said **168**; DEC-412 corrected to **149 real**; the audited figure was **165**, and it is
//! **187** as of DEC-472 (DEC-348 added six lock helpers, DEC-347 two streaming-lines helpers,
//! DEC-494/496/487/489/472 the charset, accent-fold, sleep, word-wrap and process rows). The number moves whenever a
//! helper is added, which is fine; what must never drift again is the number stated here versus the
//! source, and the ratchet below is what ties them together. Both earlier numbers came from grepping `__phorj_` and subtracting guessed
//! artifacts; this one enumerates `function &?__phorj_x(` definitions plus the checked-arith codegen
//! table, and is asserted by the test below. `__phorj_unwrap` appears in comments but was inlined at
//! M3 S2.5 and is not a helper.
//!
//! ## Bucket 1 — semantic necessity (71)
//!
//! `checked_*` PHP overflows int→float instead of faulting · `dec_*` bcmath fixed-point, PHP has no
//! decimal · `float_*` `round_*` `trunc` `div` `rem` `add` PHP's rounding/division differ at the edges ·
//! `class_name` DEC-329.3 enum-variant naming · `json_*` PHP's `json_encode` differs on key order and
//! float rendering · `option_*` `result_*` `none` PHP has no such types · `text_*` `str_*` `index_*`
//! PHP's string calls are BYTE-oriented (verified above) · `debug_*` phorj's debug rendering has no PHP
//! analog and its escape table is parity-affecting · `reflect_*` `kind_*` `capture_*` `parse_*` no
//! native equivalent · `sleep` PHP's `sleep`/`usleep` take whole seconds or microseconds and cannot
//! express a frozen clock, which is what makes a sleeping program deterministic (DEC-487/497) ·
//! `wordwrap` PHP's `wordwrap` breaks on BYTES and mid-grapheme, and differs on an over-long word
//! (DEC-489) · `proc_run` PHP's `exec`/`shell_exec`/`system` all take a SHELL STRING, exactly the
//! form DEC-472 refuses; only `proc_open`'s array form passes argv without a shell.
//!
//!   `__phorj_add` `__phorj_capture` `__phorj_checked_add` `__phorj_checked_int`
//!   `__phorj_checked_mul` `__phorj_checked_neg` `__phorj_checked_sub` `__phorj_class_name`
//!   `__phorj_debug_enums` `__phorj_debug_quote` `__phorj_debug_render` `__phorj_debug_wrap`
//!   `__phorj_dec_add` `__phorj_dec_check` `__phorj_dec_div` `__phorj_dec_div_exact`
//!   `__phorj_dec_fmt` `__phorj_dec_mul` `__phorj_dec_of` `__phorj_dec_rem`
//!   `__phorj_dec_round` `__phorj_dec_scale` `__phorj_dec_sub` `__phorj_dec_to_int`
//!   `__phorj_dec_to_int_exact` `__phorj_dec_unscaled` `__phorj_div` `__phorj_float`
//!   `__phorj_float_to_int` `__phorj_float_to_int_exact` `__phorj_index` `__phorj_index_of`
//!   `__phorj_json_build` `__phorj_json_decode` `__phorj_json_encode` `__phorj_json_encode_pretty`
//!   `__phorj_json_parse_lines` `__phorj_json_pretty` `__phorj_json_stringify_lines` `__phorj_kind`
//!   `__phorj_none` `__phorj_option_and_then` `__phorj_option_filter` `__phorj_option_get_or_else`
//!   `__phorj_option_map` `__phorj_option_of_nullable` `__phorj_option_to_nullable` `__phorj_parse_float`
//!   `__phorj_parse_int` `__phorj_proc_run` `__phorj_reflect_of` `__phorj_rem`
//!   `__phorj_result_and_then` `__phorj_result_get_or_else` `__phorj_result_map` `__phorj_result_map_err`
//!   `__phorj_result_or_else` `__phorj_result_to_option` `__phorj_round` `__phorj_round_div`
//!   `__phorj_round_mode` `__phorj_sleep` `__phorj_str` `__phorj_str_chunk`
//!   `__phorj_text_index_of` `__phorj_text_reverse` `__phorj_text_trim` `__phorj_text_trim_end`
//!   `__phorj_text_trim_start` `__phorj_trunc` `__phorj_wordwrap`
//!
//! ## Bucket 2 — no single-expression equivalent (116)
//!
//! Reason stated per family, as DEC-377 requires: `fs_*` `http_*` `db_*` `uri_*` need `try`/`catch` to
//! turn an exception into a value, and `try` is not an expression in PHP · `regex_*` `log_*` need a
//! temporary for the match/handler array · `rng_*` `now_*` `db_depths` return BY REFERENCE
//! (`function &…`) to hold mutable global state · `sort` `min` `max` `all` `any` `find` `drop_*` `take_*`
//! `last` `unique` `group_*` `list_*` `map_*` PHP's equivalents differ in short-circuit or
//! evaluation-order semantics · `range` `number_format` `format` `clamp` `gcd` `lcm` `ini_*` `init_*`
//! are multi-statement · `cs_*` `fold_accents` are TABLE-DRIVEN loops: `mb_convert_encoding` and
//! `iconv` are ini extensions the transpile rules forbid, so DEC-494/496 hand-roll both legs from
//! one table and the loop is not an expression.
//!
//!   `__phorj_all` `__phorj_any` `__phorj_clamp` `__phorj_cs_decode`
//!   `__phorj_cs_encode` `__phorj_cs_name` `__phorj_db_begin` `__phorj_db_classify`
//!   `__phorj_db_commit` `__phorj_db_depths` `__phorj_db_msg_is_unique` `__phorj_db_rollback`
//!   `__phorj_db_rollback_all` `__phorj_db_set_depth` `__phorj_db_try` `__phorj_db_try_unit`
//!   `__phorj_db_tx_depth` `__phorj_db_unwind_to` `__phorj_drop_while` `__phorj_find`
//!   `__phorj_fold_accents` `__phorj_format` `__phorj_fs_copy` `__phorj_fs_create_dir`
//!   `__phorj_fs_delete` `__phorj_fs_err` `__phorj_fs_for_each_line` `__phorj_fs_list_dir`
//!   `__phorj_fs_lock_acquire` `__phorj_fs_lock_open` `__phorj_fs_lock_release` `__phorj_fs_lock_store`
//!   `__phorj_fs_lock_try_acquire` `__phorj_fs_locks` `__phorj_fs_move` `__phorj_fs_put`
//!   `__phorj_fs_read_bytes` `__phorj_fs_read_lines_chunk` `__phorj_fs_read_text` `__phorj_fs_remove_dir`
//!   `__phorj_fs_remove_dir_all` `__phorj_fs_rmrf` `__phorj_fs_size` `__phorj_fs_split_lines`
//!   `__phorj_fs_walk` `__phorj_fs_walk_into` `__phorj_gcd` `__phorj_group_by`
//!   `__phorj_http_boundary_of` `__phorj_http_cookie_pairs` `__phorj_http_decode_path` `__phorj_http_header_pairs`
//!   `__phorj_http_json_parse` `__phorj_http_multipart_fields` `__phorj_http_parse_multipart` `__phorj_http_parse_query`
//!   `__phorj_http_parse_request` `__phorj_http_pct_decode` `__phorj_http_read_spill` `__phorj_http_stash_body`
//!   `__phorj_http_trim` `__phorj_ini_parse` `__phorj_init_statics` `__phorj_last_index_of`
//!   `__phorj_lcm` `__phorj_list_difference` `__phorj_list_intersection` `__phorj_log_configure`
//!   `__phorj_log_emit` `__phorj_log_fmt` `__phorj_log_json_escape` `__phorj_log_ord`
//!   `__phorj_log_rotate` `__phorj_log_write` `__phorj_map_remove` `__phorj_map_set`
//!   `__phorj_max` `__phorj_max_by` `__phorj_min` `__phorj_min_by`
//!   `__phorj_now_freeze` `__phorj_now_frozen` `__phorj_now_millis` `__phorj_now_unfreeze`
//!   `__phorj_number_format` `__phorj_range` `__phorj_regex_check` `__phorj_regex_compile`
//!   `__phorj_regex_delim` `__phorj_regex_expand` `__phorj_regex_find` `__phorj_regex_find_all`
//!   `__phorj_regex_find_all_groups` `__phorj_regex_find_groups` `__phorj_regex_linear_unsupported` `__phorj_regex_matches`
//!   `__phorj_regex_pcre_divergent` `__phorj_regex_quote_meta` `__phorj_regex_replace` `__phorj_regex_replace_callback`
//!   `__phorj_regex_split` `__phorj_regex_validated` `__phorj_rng_int_between` `__phorj_rng_next`
//!   `__phorj_rng_next_float` `__phorj_rng_seed` `__phorj_rng_state` `__phorj_rng_step`
//!   `__phorj_sort` `__phorj_sort_with` `__phorj_take_while` `__phorj_unique`
//!   `__phorj_uri` `__phorj_uri_parse` `__phorj_uri_resolve` `__phorj_uri_with`

/// Every `__phorj_*` helper, with its DEC-377 bucket. Sorted; asserted complete by the test below.
///
/// `1` = semantic necessity · `2` = no single-expression equivalent · `3` = convenience/DRY (must be
/// inlined — **currently empty, and a new entry here should be inlined rather than recorded**).
/// `#[cfg(test)]`: the registry is DOCUMENTATION plus the ratchet's expected set — no production code
/// reads it, and gating it keeps `-D warnings` honest rather than reaching for `#[allow(dead_code)]`.
/// The audit prose itself is in this module's `//!` header, so it is present in every build.
#[cfg(test)]
const HELPER_BUCKETS: &[(&str, u8)] = &[
    ("__phorj_add", 1),
    ("__phorj_all", 2),
    ("__phorj_any", 2),
    ("__phorj_capture", 1),
    ("__phorj_checked_add", 1),
    // DEC-494 — charset transcoding. Bucket 2: PHP's only native answer is `mb_convert_encoding` /
    // `iconv`, both ini extensions the transpile rules forbid and both absent under the oracle's
    // `php -n`, so there is no native call to make — the codec is a loop over code points, which is
    // not an expression. The tables are formatted from `ext::encoding::charset`'s consts at emit
    // time, so this is one implementation rendered twice, not two implementations.
    ("__phorj_cs_decode", 2),
    ("__phorj_cs_encode", 2),
    ("__phorj_cs_name", 2),
    ("__phorj_checked_int", 1),
    ("__phorj_checked_mul", 1),
    ("__phorj_checked_neg", 1),
    ("__phorj_checked_sub", 1),
    ("__phorj_clamp", 2),
    ("__phorj_class_name", 1),
    ("__phorj_db_begin", 2),
    ("__phorj_db_classify", 2),
    ("__phorj_db_commit", 2),
    ("__phorj_db_depths", 2),
    ("__phorj_db_msg_is_unique", 2),
    ("__phorj_db_rollback", 2),
    ("__phorj_db_rollback_all", 2),
    ("__phorj_db_set_depth", 2),
    ("__phorj_db_try", 2),
    ("__phorj_db_try_unit", 2),
    ("__phorj_db_tx_depth", 2),
    ("__phorj_db_unwind_to", 2),
    ("__phorj_debug_enums", 1),
    ("__phorj_debug_quote", 1),
    ("__phorj_debug_render", 1),
    ("__phorj_debug_wrap", 1),
    ("__phorj_dec_add", 1),
    ("__phorj_dec_check", 1),
    ("__phorj_dec_div", 1),
    ("__phorj_dec_div_exact", 1),
    ("__phorj_dec_fmt", 1),
    ("__phorj_dec_mul", 1),
    ("__phorj_dec_of", 1),
    ("__phorj_dec_rem", 1),
    ("__phorj_dec_round", 1),
    ("__phorj_dec_scale", 1),
    ("__phorj_dec_sub", 1),
    ("__phorj_dec_to_int", 1),
    ("__phorj_dec_to_int_exact", 1),
    ("__phorj_dec_unscaled", 1),
    ("__phorj_div", 1),
    ("__phorj_drop_while", 2),
    ("__phorj_find", 2),
    ("__phorj_float", 1),
    // DEC-468 — accent folding. Bucket 2: `strtr` alone would do it, but the map is a 190-entry
    // literal generated from `crate::fold_accents::FOLD`, so the alternative is emitting 4 kB of
    // array literal at every call site. PHP's nearest native, `iconv(…,'ASCII//TRANSLIT',…)`, is an
    // ini extension AND locale-dependent, so it is neither tier-1 nor byte-identical.
    ("__phorj_fold_accents", 2),
    ("__phorj_float_to_int", 1),
    ("__phorj_float_to_int_exact", 1),
    ("__phorj_format", 2),
    ("__phorj_fs_copy", 2),
    ("__phorj_fs_create_dir", 2),
    ("__phorj_fs_delete", 2),
    ("__phorj_fs_err", 2),
    ("__phorj_fs_list_dir", 2),
    ("__phorj_fs_move", 2),
    ("__phorj_fs_put", 2),
    ("__phorj_fs_read_bytes", 2),
    // DEC-347 streaming lines. Bucket 2: PHP HAS every piece (`fopen`/`fseek`/`fread`/`fgets`), but the
    // helper is a LOOP plus a mid-line extension plus a UTF-8 gate plus the typed-error mapping — none
    // of which is an expression, and `fgets` alone would not give the offset-advance contract the
    // prelude depends on (terminators kept, chunk never ending mid-line).
    ("__phorj_fs_for_each_line", 2),
    ("__phorj_fs_read_lines_chunk", 2),
    // DEC-347's chunk splitter. Bucket 2: `explode` gives the pieces, but dropping the trailing empty
    // element and stripping a `\r` per line is a statement sequence, not an expression. It lives in a
    // helper for a PERFORMANCE reason too — doing it in the prelude was O(n²) via `List.append`'s
    // copy-per-call (58x slower than `fgets`), so both legs push it down to one pass.
    ("__phorj_fs_split_lines", 2),
    ("__phorj_fs_read_text", 2),
    // DEC-348 advisory locking (6). Bucket 2 throughout: PHP HAS `flock`, but a scoped lock needs the
    // open handle to OUTLIVE the acquiring expression — the OS lock dies with the descriptor — so the
    // hold cannot be one expression. `__phorj_fs_locks` is the by-reference handle table that keeps it
    // alive, `_store` mints the ticket, `_open` centralises the typed-error pre-checks (PHP exposes no
    // ErrorKind), and `_acquire`/`_try_acquire`/`_release` are the three call-site entry points. Ticket
    // semantics are pinned to `src/native/fs_lock.rs`: 1-based, with 0 meaning not-acquired.
    ("__phorj_fs_lock_acquire", 2),
    ("__phorj_fs_lock_open", 2),
    ("__phorj_fs_lock_release", 2),
    ("__phorj_fs_lock_store", 2),
    ("__phorj_fs_lock_try_acquire", 2),
    ("__phorj_fs_locks", 2),
    ("__phorj_fs_remove_dir", 2),
    ("__phorj_fs_remove_dir_all", 2),
    ("__phorj_fs_rmrf", 2),
    ("__phorj_fs_size", 2),
    ("__phorj_fs_walk", 2),
    ("__phorj_fs_walk_into", 2),
    ("__phorj_gcd", 2),
    ("__phorj_group_by", 2),
    ("__phorj_http_boundary_of", 2),
    ("__phorj_http_cookie_pairs", 2),
    ("__phorj_http_decode_path", 2),
    ("__phorj_http_header_pairs", 2),
    ("__phorj_http_json_parse", 2),
    ("__phorj_http_multipart_fields", 2),
    ("__phorj_http_parse_multipart", 2),
    ("__phorj_http_parse_query", 2),
    ("__phorj_http_parse_request", 2),
    ("__phorj_http_pct_decode", 2),
    ("__phorj_http_read_spill", 2),
    ("__phorj_http_stash_body", 2),
    ("__phorj_http_trim", 2),
    ("__phorj_index", 1),
    ("__phorj_index_of", 1),
    ("__phorj_ini_parse", 2),
    ("__phorj_init_statics", 2),
    ("__phorj_json_build", 1),
    ("__phorj_json_decode", 1),
    ("__phorj_json_encode", 1),
    ("__phorj_json_encode_pretty", 1),
    ("__phorj_json_parse_lines", 1),
    ("__phorj_json_pretty", 1),
    ("__phorj_json_stringify_lines", 1),
    ("__phorj_kind", 1),
    ("__phorj_last_index_of", 2),
    ("__phorj_lcm", 2),
    ("__phorj_list_difference", 2),
    ("__phorj_list_intersection", 2),
    ("__phorj_log_configure", 2),
    ("__phorj_log_emit", 2),
    ("__phorj_log_fmt", 2),
    ("__phorj_log_json_escape", 2),
    ("__phorj_log_ord", 2),
    ("__phorj_log_rotate", 2),
    ("__phorj_log_write", 2),
    ("__phorj_map_remove", 2),
    ("__phorj_map_set", 2),
    ("__phorj_max", 2),
    ("__phorj_max_by", 2),
    ("__phorj_min", 2),
    ("__phorj_min_by", 2),
    ("__phorj_none", 1),
    ("__phorj_now_freeze", 2),
    ("__phorj_now_frozen", 2),
    ("__phorj_now_millis", 2),
    ("__phorj_now_unfreeze", 2),
    ("__phorj_number_format", 2),
    ("__phorj_option_and_then", 1),
    ("__phorj_option_filter", 1),
    ("__phorj_option_get_or_else", 1),
    ("__phorj_option_map", 1),
    ("__phorj_option_of_nullable", 1),
    ("__phorj_option_to_nullable", 1),
    ("__phorj_parse_float", 1),
    ("__phorj_parse_int", 1),
    // DEC-472 — `Process.run`. Bucket 1 (semantic necessity): PHP's `exec`/`shell_exec`/`system`
    // all take a SHELL STRING, exactly the form the ruling refuses. Only `proc_open`'s ARRAY form
    // passes argv without a shell, and the timeout needs a poll loop that drains both pipes.
    ("__phorj_proc_run", 1),
    ("__phorj_range", 2),
    ("__phorj_reflect_of", 1),
    ("__phorj_regex_check", 2),
    ("__phorj_regex_compile", 2),
    ("__phorj_regex_delim", 2),
    ("__phorj_regex_expand", 2),
    ("__phorj_regex_find", 2),
    ("__phorj_regex_find_all", 2),
    ("__phorj_regex_find_all_groups", 2),
    ("__phorj_regex_find_groups", 2),
    ("__phorj_regex_linear_unsupported", 2),
    ("__phorj_regex_matches", 2),
    ("__phorj_regex_pcre_divergent", 2),
    ("__phorj_regex_quote_meta", 2),
    ("__phorj_regex_replace", 2),
    ("__phorj_regex_replace_callback", 2),
    ("__phorj_regex_split", 2),
    ("__phorj_regex_validated", 2),
    ("__phorj_rem", 1),
    ("__phorj_result_and_then", 1),
    ("__phorj_result_get_or_else", 1),
    ("__phorj_result_map", 1),
    ("__phorj_result_map_err", 1),
    ("__phorj_result_or_else", 1),
    ("__phorj_result_to_option", 1),
    ("__phorj_rng_int_between", 2),
    ("__phorj_rng_next", 2),
    ("__phorj_rng_next_float", 2),
    ("__phorj_rng_seed", 2),
    ("__phorj_rng_state", 2),
    ("__phorj_rng_step", 2),
    ("__phorj_round", 1),
    ("__phorj_round_div", 1),
    ("__phorj_round_mode", 1),
    // DEC-487 — `Time.sleep`. Bucket 1 (semantic necessity): the naive `usleep($ms*1000)` would be
    // WRONG under `Time.freeze`, where the native is a no-op, so a frozen example would sleep for
    // real on this leg only and stop being byte-identical.
    ("__phorj_sleep", 1),
    ("__phorj_sort", 2),
    ("__phorj_sort_with", 2),
    ("__phorj_str", 1),
    ("__phorj_str_chunk", 1),
    ("__phorj_take_while", 2),
    ("__phorj_text_index_of", 1),
    ("__phorj_text_reverse", 1),
    ("__phorj_text_trim", 1),
    ("__phorj_text_trim_end", 1),
    ("__phorj_text_trim_start", 1),
    ("__phorj_trunc", 1),
    // FN-STR — `String.wordWrap`. Bucket 1 (semantic necessity): PHP's own `wordwrap` is
    // byte-oriented and splits multi-byte characters into invalid UTF-8, so the naive native call
    // would be WRONG for a phorj `string`, not merely differently shaped.
    ("__phorj_wordwrap", 1),
    ("__phorj_unique", 2),
    ("__phorj_uri", 2),
    ("__phorj_uri_parse", 2),
    ("__phorj_uri_resolve", 2),
    ("__phorj_uri_with", 2),
];

#[cfg(test)]
mod tests;
