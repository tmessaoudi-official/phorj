//! THE LADDER RULE's transpile gate (Invariant 14, tier 2).
//!
//! One table, one walk: the Core modules whose semantics have no faithful byte-identical PHP
//! mapping, and the `E-TRANSPILE-<FEATURE>` each one refuses with. Split out of `pipeline.rs`
//! when adding the `Core.Native.Http` row pushed that file past its Invariant-13 size baseline —
//! the gate is a cohesive unit with a single job, which is what the split rule asks for.
//!
//! IT RUNS PRE-EXPANSION, and every row depends on that. **FOUR** callers refuse before preludes are
//! injected — `transpile_program` (the loader path, `main.rs` dispatches `transpile` here),
//! `build_php` (the DEC-320 adoption lever), `transpile_source` (the single-source chokepoint behind
//! `cmd_transpile` AND the playground's `transpile_json`), and `benchmark --vs-php` (which emits and
//! EXECUTES PHP, so it is a transpile path; panel round 3 C7/F1) — so a row keyed on
//! a module the INJECTED preludes themselves import — as
//! `Core.Native.Http` is — rejects only the user's own import. Move this call after expansion and
//! the `Core.Native.Http` row alone would reject every `import Core.Http;` program in the corpus.

use crate::ast::{Item, Program};

/// THE LADDER RULE (MASTER-PLAN G-rules; first applications: concurrency, `Core.Database`, `Core.Mail`):
/// a native-only Core module — one whose semantics have no faithful PHP byte-identity mapping (live
/// DB I/O, SMTP delivery) — HARD-ERRORS on transpile with a module-specific `E-TRANSPILE-<FEATURE>`
/// code. Never a silent degrade, and never the wall of prelude-internal errors the check would
/// otherwise produce. New native-only module = one row here.
pub(super) fn reject_native_only_transpile(prog: &Program) -> Result<(), String> {
    const NATIVE_ONLY: &[(&[&str], &str, &str)] = &[
        (
            &["Core", "Database"],
            "E-TRANSPILE-DB",
            "`Core.Database` is native-only: live database I/O cannot be byte-identical across the phorj drivers and PHP PDO, so transpiling it is refused rather than silently diverging (THE LADDER RULE). Run this program with `phg run`.",
        ),
        (
            &["Core", "SessionModule"],
            "E-TRANSPILE-SESSION",
            "`Core.SessionModule` is native-only PERMANENTLY (DEC-313): entropy-random session ids (observable via Session.id()), a wall-clock idle TTL, and the persistent in-process store make it not byte-identically transpilable to PHP's per-request $_SESSION model (THE LADDER RULE: refusing beats silent divergence). Run session programs with `phg run` / `phg serve`.",
        ),
        (
            &["Core", "HttpClientModule"],
            "E-TRANSPILE-HTTPCLIENT",
            "`Core.HttpClientModule` is native-only: live network I/O cannot be byte-identical across the phorj client and a PHP mapping, so transpiling it is refused rather than silently diverging (THE LADDER RULE). A faithful curl-mapping is a recorded future lift. Run this program with `phg run`.",
        ),
        (
            &["Core", "Mail"],
            "E-TRANSPILE-MAIL",
            "`Core.Mail` is native-only (DEC-223): PHP's mail() has no SMTP auth, no TLS, and is header-injection-prone — any mapping would silently drop auth/TLS/attachments (THE LADDER RULE forbids the downgrade). Run this program with `phg run`.",
        ),
        // DEC-277: the RAW `Core.Native.*` twins of the native-only modules above. Importing the
        // raw natives directly must not bypass the ladder gate — several of their `php` emitters
        // are placeholders (e.g. `Core.Native.Database` close/transaction), so a transpile would
        // silently diverge instead of refusing. (`Core.Native.Uri`/`Core.Native.Debug` stay
        // transpilable — their emitters are real twins.)
        (
            &["Core", "Native", "Database"],
            "E-TRANSPILE-DB",
            "`Core.Native.Database` (the raw natives under `Core.Database`) is native-only: live database I/O cannot be byte-identical across the phorj drivers and PHP PDO (THE LADDER RULE). Run this program with `phg run`.",
        ),
        (
            &["Core", "Native", "Session"],
            "E-TRANSPILE-SESSION",
            "`Core.Native.Session` (the raw natives under `Core.SessionModule`) is native-only PERMANENTLY (DEC-313, same grounds as the module). Run session programs with `phg run` / `phg serve`.",
        ),
        (
            &["Core", "Native", "HttpClient"],
            "E-TRANSPILE-HTTPCLIENT",
            "`Core.Native.HttpClient` (the raw natives under `Core.HttpClientModule`) is native-only: live network I/O cannot be byte-identical (THE LADDER RULE). Run this program with `phg run`.",
        ),
        (
            &["Core", "Native", "Mail"],
            "E-TRANSPILE-MAIL",
            "`Core.Native.Mail` (the raw natives under `Core.Mail`) is native-only (DEC-223, THE LADDER RULE). Run this program with `phg run`.",
        ),
        // The raw twin of `Core.Http`, and the hole the call-keyed `E-TRANSPILE-SERVE` in
        // `transpile/call.rs` left open: that arm reads `Http.serve` and nothing else, so
        // registering a handler through `Core.Native.Http.registerServe` instead walked past it.
        // `phg transpile` then exited 0 emitting `__phorj_http_register_serve(...)` — a helper NO
        // family defines — and the PHP leg fatalled at runtime while the native legs ran clean.
        // That is Invariant 1's spine broken by a transpile the toolchain called a success, which
        // tier 2 of THE LADDER RULE forbids: refuse at transpile time, never diverge at run time.
        // The `E-IMPORT-NATIVE-MEMBER` hint recommends this very spelling, so the bypass was on the
        // endorsed path rather than an obscure corner.
        //
        // SAFE AS A MODULE ROW ONLY because this gate runs PRE-EXPANSION — `cmd_transpile` refuses
        // the `lex_parse` output and `transpile_program` refuses before `check_and_expand`. The
        // injected HTTP preludes import this module themselves and `Http.serve`'s body CALLS
        // `registerServe`, so the same row applied AFTER injection would reject every
        // `import Core.Http;` program, i.e. the whole `examples/web/*` corpus this refusal exists
        // to protect. `an_ordinary_core_http_program_still_transpiles_after_the_module_keyed_refusal`
        // (tests/serve.rs) pins that ordering; move the gate later and it goes red.
        //
        // Whole-module, like its four siblings above: the friendly `Core.Http` surface is the
        // supported way to reach these natives, and a member-keyed gate would have to re-derive
        // the user's import alias to know what `X.registerServe` means.
        (
            &["Core", "Native", "Http"],
            "E-TRANSPILE-SERVE",
            "`Core.Native.Http` (the raw natives under `Core.Http`) is native-only: it registers a serve handler, and PHP is served BY a web server rather than being one, so no faithful idiomatic mapping exists (THE LADDER RULE, Invariant 14 tier 2). Importing the raw module must not bypass the `Http.serve` refusal. Run this program with `phg serve`.",
        ),
    ];
    // The by-name containment arm that used to sit here (a user call to `NativeHttp.registerServe`
    // through the LEAKED prelude alias) was removed with DEC-459: the prelude's alias is now isolated
    // under `NativeHttp#prelude`, so `NativeHttp` in user code is an unknown identifier and there is
    // nothing to contain (`tests/prelude_isolation.rs`, `tests/serve.rs`).
    for it in &prog.items {
        let Item::Import { path, span, .. } = it else {
            continue;
        };
        for (module, code, why) in NATIVE_ONLY {
            if path.len() >= module.len() && path.iter().zip(module.iter()).all(|(a, b)| a == b) {
                let m = module.join(".");
                return Err(format!(
                    "transpile error at {}:{}: cannot transpile a program importing `{m}`\n  [{code}]\n  hint: {why}",
                    span.line, span.col
                ));
            }
        }
    }
    Ok(())
}
