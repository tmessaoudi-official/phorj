//! The front-end pipeline chokepoints (`check_and_expand` / reified variants — Invariants
//! 5+6) and the `cmd_*` entry points every backend rides through.

use super::*;

/// Run a pipeline closure on a worker thread with a large (256 MB) stack. The tokenizer is iterative,
/// but the parser, checker, compiler, and tree-walking interpreter all recurse on the native stack
/// in proportion to expression/call nesting. A generous, *known* stack makes the explicit depth
/// limits (`limits::MAX_NEST_DEPTH`, `limits::MAX_CALL_DEPTH`) — not Rust's ambient frame budget —
/// the thing that bounds recursion, so adversarial-but-bounded input faults cleanly instead of
/// aborting, identically whether called from the CLI's main thread or a 2 MB test thread.
pub(super) fn on_deep_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(s, f)
            .expect("spawn pipeline worker thread")
            .join()
            .expect("pipeline worker thread panicked")
    })
}

/// lex + parse, rendering the stage error to a single line. Every stage now returns a unified
/// [`crate::diagnostic::Diagnostic`] that renders itself (stage prefix + position), so the CLI
/// just calls `to_string()` rather than hand-formatting per stage.
pub(super) fn lex_parse(src: &str) -> Result<Program, String> {
    let tokens = lex(src).map_err(|e| e.render(src))?;
    Parser::new(tokens)
        .parse_program()
        .map_err(|e| e.render(src))
}

/// Public lex + parse of a single source string into an **unchecked** `Program` (no type-check, no
/// alias/generic expansion). Exposes the private [`lex_parse`] for callers that want to run the
/// type-checker themselves and surface its diagnostics without aborting — e.g. the WASM playground,
/// which feeds the parsed program to [`check_json_program`] to render errors *and* warnings rather
/// than the fatal first-error string [`parse_checked`] produces. A syntax error still returns `Err`.
pub fn parse_program(src: &str) -> Result<Program, String> {
    lex_parse(src)
}

pub fn check_and_expand(prog: &Program, diag_src: &str) -> Result<Program, String> {
    check_and_expand_reified(prog, diag_src).map(|(p, _)| p)
}

/// Like [`check_and_expand`], but also returns the checker's **reified-operand side-table** (S2.1-broad):
/// `expr span.start -> resolved Ty` for `Call`/`Member`/`Index` results, fed to the VM compiler
/// ([`crate::compiler::compile_with`]) so a generic method result / field read specializes as the
/// arithmetic operand the checker proved. The interpreter paths use the map-dropping wrapper above.
#[allow(clippy::type_complexity)]
pub fn check_and_expand_reified(
    prog: &Program,
    diag_src: &str,
) -> Result<(Program, std::collections::HashMap<usize, crate::types::Ty>), String> {
    // Import-redesign S2 stage C: enforce injected-type import discipline on the RAW user program,
    // BEFORE any prelude injection or the S1 qualifier collapse — so the preludes' own bare internals
    // are never scanned and bare-vs-qualified is still distinguishable. A bare injected member type
    // (`Router`, `Duration`, …) or `#[Route]` used without a member-import is `E-INJECTED-TYPE-BARE`.
    let injected_violations = crate::checker::enforce_injected_discipline(prog);
    if !injected_violations.is_empty() {
        let lines: Vec<String> = injected_violations
            .iter()
            .map(|e| e.render(diag_src))
            .collect();
        return Err(lines.join("\n"));
    }
    // DEC-196 Q3: fault-intrinsic import discipline (`Core.Assert`/`Core.Abort`). On the RAW program
    // (bare-vs-qualified still distinguishable): validate that every intrinsic call is covered by the
    // matching import (`E-UNIMPORTED` otherwise) AND normalize the qualified form `Assert.assert(x)`
    // down to the bare intrinsic `assert(x)` every backend already lowers. A no-op unless an intrinsic
    // module is touched, so intrinsic-free programs are byte-for-byte unchanged.
    let intrinsic_rewritten = match crate::checker::resolve_intrinsic_imports(prog.clone()) {
        Ok(p) => p,
        Err(ds) => {
            let lines: Vec<String> = ds.iter().map(|e| e.render(diag_src)).collect();
            return Err(lines.join("\n"));
        }
    };
    let prog = &intrinsic_rewritten;
    // Feature-availability gate: an import of a Core module whose natives are compiled out of THIS
    // build (e.g. `Core.Db` under `--no-default-features`) is ONE clean `E-MODULE-UNAVAILABLE` — never
    // the wall of prelude-internal `E-UNKNOWN-IDENT`s the injection below would otherwise produce.
    if let Some(d) = super::preludes::unavailable_core_module(prog) {
        return Err(d.render(diag_src));
    }
    // UA-L2 (registry-unification): one fold over `CORE_MODULES` replaces the former eight chained
    // `inject_*_prelude` calls — a no-op for programs that import no injected Core module. Byte-identical
    // to the old chain (proven over the whole corpus at cutover); adding a Core module is now one row.
    let injected = inject_core_modules(prog);
    // M6 W2: lower `Http.autoRouter()` into explicit `Router` construction from the `#[Route]`-
    // annotated handlers — BEFORE the checker, so the generated registration type-checks like
    // hand-written code (a no-op unless `Core.Http` is imported). The `#[Route]` attrs survive for the
    // checker's validation pass, then are inert for the backends.
    let routed = crate::checker::desugar_auto_router(injected.into_owned());
    // Import-redesign S1: collapse qualified injected-type references (`Http.Router`, `Time.Duration`,
    // `Decimal.RoundingMode`) in type-annotation position down to their bare injected type — so both the
    // checker AND every backend see the plain `Router`/`Duration`/`RoundingMode` the preludes declare.
    // Runs after `desugar_auto_router` (its generated `Router` construction is bare already) and before
    // `check_resolutions`.
    let routed = crate::checker::collapse_injected_type_qualifiers(routed);
    // Wave B B-2c (DEC-186): resolve imported injected-enum variants (`import Core.Result.Success;` /
    // `… as X;` / grouped) to their qualified `Enum.Variant` form, so the proven qualified construction +
    // pattern paths handle them byte-identically. A no-op unless a variant import is present. Runs after
    // the qualifier collapse (its output is bare injected TYPE names, disjoint from variant heads) and
    // before `check_resolutions`.
    let routed = crate::checker::resolve_variant_imports(routed);
    // DI v1 (`docs/plans/di-attributes.plan.md`): expand `inject<T>()` composition roots into plain
    // `new` construction (a synthesized `__phorj_di_<T>()` factory per requested type). Pre-check, so
    // the generated graph type-checks like hand-written code and every backend sees explicit
    // construction (Inv-5). A no-op unless `inject` is used; compile errors (E-DI-*/E-INJECT-NO-TYPE)
    // surface here exactly like the other pre-check passes.
    let routed = match crate::checker::desugar_di(routed) {
        Ok(p) => p,
        Err(ds) => {
            let lines: Vec<String> = ds.iter().map(|e| e.render(diag_src)).collect();
            return Err(lines.join("\n"));
        }
    };
    // DEC-208 S2: lower the type-directed `Core.Db` hydration calls `stmt.queryInto()` /
    // `stmt.queryOneInto()` into plain construction via synthesized per-class helpers, drawing the row
    // class from the binding's declared type OR an explicit call-site turbofish (slice A wired —
    // turbofish wins; arity checked in the pass, `E-TYPE-ARG-COUNT`). Pre-check, so the generated
    // `new T(row.getX(..)?)` graph type-checks like hand-written code and both backends run the one
    // desugared AST (Inv-5; `run ≡ runvm` automatic). A no-op unless `Core.Db` is imported.
    let routed = match crate::checker::desugar_db(routed) {
        Ok(p) => p,
        Err(ds) => {
            let lines: Vec<String> = ds.iter().map(|e| e.render(diag_src)).collect();
            return Err(lines.join("\n"));
        }
    };
    let prog = &routed;
    match crate::checker::check_resolutions(prog) {
        Ok((warnings, html, ufcs, overload_renames, reified)) => {
            for w in &warnings {
                eprintln!("warning: {}", w.render(diag_src));
            }
            // De-alias types, erase `html"…"` literals into their `Html.concat([…])` kernel calls
            // (built by the checker, keyed by span), then erase generic type parameters — all three
            // are front-end sugar removed before any backend runs (M-RT S7 adds the last).
            // Feature C: `unwrap_new` strips the `Expr::New` construction wrapper after the type sugar
            // is gone, so every backend sees the plain construction `Call`. Slice 6: `rewrite_ufcs`
            // runs last, rewriting each resolved `x.f(a)` member call into the ordinary free/native
            // call `f(x, a)` the checker chose — by then the receiver/args are fully de-sugared.
            // Batch D: inject `= null` defaults for optional instance fields (after aliases are
            // expanded, so an aliased optional is already `Type::Optional`) — a front-end desugar so
            // every backend initializes them identically.
            // Slice C1: rename each return-overload member's *definition* to its mangled name (by decl
            // span); the resolved selector *call sites* were already merged into `ufcs` above and are
            // rewritten to the same mangled names by `rewrite_ufcs`. A no-op when no function is
            // return-overloaded (so single-overload programs stay byte-identical).
            // B1b: inline `parent.constructor(…)` LAST, so the cloned parent body is already fully
            // de-sugared (aliases/html/generics/new/UFCS/overload-renames all applied). A no-op unless
            // a constructor forwards to its parent — programs without it stay byte-identical.
            Ok((
                crate::checker::inline_parent_ctors(crate::checker::rename_overload_defs(
                    crate::checker::rewrite_ufcs(
                        crate::checker::unwrap_new(crate::checker::erase_generics(
                            crate::checker::resolve_html(
                                crate::checker::inject_optional_field_defaults(
                                    crate::checker::expand_aliases(prog),
                                ),
                                &html,
                            ),
                        )),
                        &ufcs,
                    ),
                    &overload_renames,
                )),
                reified,
            ))
        }
        Err(errs) => {
            let lines: Vec<String> = errs.iter().map(|e| e.render(diag_src)).collect();
            Err(lines.join("\n"))
        }
    }
}

/// lex + parse + type-check (the gate). Renders every type error, one per line.
pub(super) fn parse_checked(src: &str) -> Result<Program, String> {
    let prog = lex_parse(src)?;
    check_and_expand(&prog, src)
}

/// Like [`parse_checked`], but also returns the checker's **reified-operand side-table** so a
/// VM-running caller can [`compile_with`] it — the byte-identical path [`cmd_run`] uses. Without it,
/// a method-call/field-read result used as an arithmetic operand (`a.join() + b.join()`,
/// `box.get() + 1`) is rejected by the VM compiler (`ctype` falls through to `method_rets`) while the
/// interpreter accepts it — a `run ≠ runvm` divergence. Any inline-source path that builds a
/// `BytecodeProgram` (`disasm`, `bench`) MUST use this, not `parse_checked` + `compile`.
#[allow(clippy::type_complexity)]
pub(super) fn parse_checked_reified(
    src: &str,
) -> Result<(Program, std::collections::HashMap<usize, crate::types::Ty>), String> {
    let prog = lex_parse(src)?;
    check_and_expand_reified(&prog, src)
}

/// Public lex + parse + check of a single source string into a checked, alias-expanded `Program`.
/// Exposes the private [`parse_checked`] pipeline for callers that need a ready-to-run program from
/// inline source — e.g. `tests/serve.rs`, which builds a serve program then drives it through
/// [`crate::serve::serve`] over an in-memory transport.
pub fn parse_checked_program(src: &str) -> Result<Program, String> {
    parse_checked(src)
}

/// Like [`parse_checked_program`], but also returns the reified-operand side-table — so a caller (e.g.
/// `tests/serve.rs`) can build the VM serve factory ([`crate::serve::vm_factory`]) on the exact
/// byte-identical path the CLI uses.
#[allow(clippy::type_complexity)]
pub fn parse_checked_program_reified(
    src: &str,
) -> Result<(Program, std::collections::HashMap<usize, crate::types::Ty>), String> {
    parse_checked_reified(src)
}

/// `run`: lex -> parse -> check (gate) -> interpret -> captured stdout.
/// M8.5 interop: refuse to *execute* a program that uses foreign `declare` symbols. The Rust backends
/// (interpreter/VM) have no PHP runtime, so a foreign call cannot run — the program is PHP-target-only.
/// `check`/`transpile` work fully; only `run`/`runvm` hit this one clean pre-flight gate (no per-call
/// fault machinery in the execution paths). A single scan after type-checking, before any backend.
pub(super) fn foreign_runtime_gate(prog: &Program) -> Result<(), String> {
    use crate::ast::Item;
    let has_foreign = prog.items.iter().any(|it| {
        matches!(it, Item::Function(f) if f.foreign) || matches!(it, Item::Class(c) if c.foreign)
    });
    if has_foreign {
        return Err(
            "error[E-FOREIGN-RUNTIME]: this program declares foreign PHP symbols (`declare`), \
             which require the PHP runtime to execute. The Rust backends (run/runvm) have no PHP \
             runtime — transpile it instead: `phg transpile <file> > out.php && php out.php`.\n"
                .to_string(),
        );
    }
    Ok(())
}

/// Check + de-sugar a program for the interactive debugger (M-DX S5): the same `check_and_expand`
/// the run backends use, plus the foreign-runtime gate (the debugger is interpreter-only, so a
/// `declare`d foreign-PHP program can't be stepped). Shared by the REPL and DAP frontends.
pub fn check_and_expand_for_debug(prog: &Program, diag_src: &str) -> Result<Program, String> {
    let checked = check_and_expand(prog, diag_src)?;
    foreign_runtime_gate(&checked)?;
    Ok(checked)
}

pub fn cmd_treewalk(src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        let prog = parse_checked(src)?;
        foreign_runtime_gate(&prog)?;
        // S4.3 cutover: a program that uses `spawn` runs on the cooperative green-thread driver (real
        // task interleaving); every other program stays on the unchanged synchronous interpreter. wasm
        // (and a `--no-default-features` build without `green`) keeps the eager path — the cfg gate
        // makes the cooperative driver absent there. Byte-identical to `runvm` via the shared scheduler.
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&prog) {
            return crate::interpreter::run_cooperative_interp(&prog)
                .map(|(out, _exit)| out)
                .map_err(|e| e.to_string());
        }
        interpret(&prog).map_err(|e| e.to_string())
    })
}

/// `runvm`: lex -> parse -> check (gate) -> compile to bytecode -> VM -> captured stdout.
/// The bytecode backend; must produce byte-identical output to `cmd_treewalk` (differential).
/// Build a `Vm` for `program`, attaching a fresh JIT hot-function cache when the `jit` feature is on
/// (b3b — wire `phg run` to the JIT). A fresh cache per program run keeps the compile-once cache
/// correctly scoped to the one program's bytecode. On a non-jit build this is exactly `Vm::new`.
pub(super) fn vm_for(program: &BytecodeProgram) -> Vm<'_> {
    #[cfg(feature = "jit")]
    {
        // JIT by default; `phg run --no-jit` (crate::vm::set_jit_enabled(false)) falls back to the
        // pure VM without a rebuild — the byte-identical oracle path, an escape hatch for a suspected
        // JIT issue.
        if crate::vm::jit_enabled() {
            Vm::new(program).with_jit(std::rc::Rc::new(std::cell::RefCell::new(
                crate::vm::JitCache::new(),
            )))
        } else {
            Vm::new(program)
        }
    }
    #[cfg(not(feature = "jit"))]
    {
        Vm::new(program)
    }
}

pub fn cmd_run(src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        let parsed = lex_parse(src)?;
        let (prog, reified) = check_and_expand_reified(&parsed, src)?;
        foreign_runtime_gate(&prog)?;
        let program = compile_with(&prog, &reified).map_err(|e| e.to_string())?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&prog) {
            return crate::vm::run_cooperative_vm(&program)
                .map(|(out, _exit)| out)
                .map_err(|e| e.to_string());
        }
        vm_for(&program).run().map_err(|e| e.to_string())
    })
}

/// Like [`cmd_treewalk`], but also returns `main`'s exit code (Batch-1 B). The string source path
/// (`-e`/stdin and standalone built binaries); the project-loader path is [`treewalk_program_exit`].
pub fn cmd_treewalk_exit(src: &str) -> Result<(String, i64), String> {
    on_deep_stack(|| {
        let prog = parse_checked(src)?;
        foreign_runtime_gate(&prog)?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&prog) {
            return crate::interpreter::run_cooperative_interp(&prog).map_err(|e| e.to_string());
        }
        interpret_main(&prog).map_err(|e| e.to_string())
    })
}

/// Like [`cmd_run`], but also returns `main`'s exit code (Batch-1 B).
pub fn cmd_run_exit(src: &str) -> Result<(String, i64), String> {
    on_deep_stack(|| {
        let parsed = lex_parse(src)?;
        let (prog, reified) = check_and_expand_reified(&parsed, src)?;
        foreign_runtime_gate(&prog)?;
        let program = compile_with(&prog, &reified).map_err(|e| e.to_string())?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&prog) {
            return crate::vm::run_cooperative_vm(&program).map_err(|e| e.to_string());
        }
        vm_for(&program).run_main().map_err(|e| e.to_string())
    })
}

/// `check`: lex -> parse -> check; report success or the type errors.
pub fn cmd_check(src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        parse_checked(src)?;
        Ok("OK (type-checks clean)\n".to_string())
    })
}

// --- Program-taking runners (M5 S2b) -----------------------------------------------------------
// The project loader (`crate::loader`) resolves a file path to a single, possibly multi-file-merged
// `Program`; these run/check/transpile it. They mirror the `cmd_*(&str)` pipelines exactly (same
// check -> de-alias -> backend), so a loose single-file program routed through `loader` produces
// byte-identical output. `diag_src` carries the source for error carets (`""` for a merged unit).

/// `run` on a loaded [`Unit`] (interpreter backend). A runtime fault is rendered **with its stack
/// trace** (error-handling slice 1): frames are attributed to files via the unit's `fn_files`, and the
/// caret is drawn against the innermost frame's source (project mode) or the single `diag_src` (loose).
pub fn treewalk_program(unit: &crate::loader::Unit) -> Result<String, String> {
    on_deep_stack(|| {
        let checked = check_and_expand(&unit.program, &unit.diag_src)?;
        foreign_runtime_gate(&checked)?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&checked) {
            return crate::interpreter::run_cooperative_interp(&checked)
                .map(|(out, _exit)| out)
                .map_err(|mut e| {
                    let src = unit.attribute_frames(&mut e);
                    e.render(&src)
                });
        }
        interpret(&checked).map_err(|mut e| {
            let src = unit.attribute_frames(&mut e);
            e.render(&src)
        })
    })
}

/// `runvm` on a loaded [`Unit`] (bytecode + VM backend). Same trace rendering as [`treewalk_program`].
pub fn run_program(unit: &crate::loader::Unit) -> Result<String, String> {
    on_deep_stack(|| {
        let (checked, reified) = check_and_expand_reified(&unit.program, &unit.diag_src)?;
        foreign_runtime_gate(&checked)?;
        let program = compile_with(&checked, &reified).map_err(|e| e.to_string())?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&checked) {
            return crate::vm::run_cooperative_vm(&program)
                .map(|(out, _exit)| out)
                .map_err(|mut e| {
                    let src = unit.attribute_frames(&mut e);
                    e.render(&src)
                });
        }
        vm_for(&program).run().map_err(|mut e| {
            let src = unit.attribute_frames(&mut e);
            e.render(&src)
        })
    })
}

/// Like [`treewalk_program`], but also returns `main`'s exit code (Batch-1 B). `phg run <file>` uses this
/// to set the process exit status; the stdout-only [`treewalk_program`] stays for the differential.
pub fn treewalk_program_exit(unit: &crate::loader::Unit) -> Result<(String, i64), String> {
    on_deep_stack(|| {
        let checked = check_and_expand(&unit.program, &unit.diag_src)?;
        foreign_runtime_gate(&checked)?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&checked) {
            return crate::interpreter::run_cooperative_interp(&checked).map_err(|mut e| {
                let src = unit.attribute_frames(&mut e);
                e.render(&src)
            });
        }
        interpret_main(&checked).map_err(|mut e| {
            let src = unit.attribute_frames(&mut e);
            e.render(&src)
        })
    })
}

/// Like [`run_program`], but also returns `main`'s exit code (Batch-1 B).
pub fn run_program_exit(unit: &crate::loader::Unit) -> Result<(String, i64), String> {
    on_deep_stack(|| {
        let (checked, reified) = check_and_expand_reified(&unit.program, &unit.diag_src)?;
        foreign_runtime_gate(&checked)?;
        let program = compile_with(&checked, &reified).map_err(|e| e.to_string())?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&checked) {
            return crate::vm::run_cooperative_vm(&program).map_err(|mut e| {
                let src = unit.attribute_frames(&mut e);
                e.render(&src)
            });
        }
        vm_for(&program).run_main().map_err(|mut e| {
            let src = unit.attribute_frames(&mut e);
            e.render(&src)
        })
    })
}

/// `check` on an already-loaded program.
pub fn check_program(prog: &Program, diag_src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        check_and_expand(prog, diag_src)?;
        Ok("OK (type-checks clean)\n".to_string())
    })
}

/// `check --json` on an already-loaded program: machine-readable diagnostics for editor / LSP
/// integration (the seam `diagnostic.rs` calls out). Returns the JSON array (errors then warnings; see
/// [`crate::diagnostic::diagnostics_json`]) and whether any *error* was present, so the caller prints
/// the array to **stdout** and exits 0 (clean / warnings only) or 1 (errors) — `check`'s exit
/// semantics, but the array is always the output and nothing goes to stderr. Positions ride on each
/// diagnostic, so no `diag_src` is needed.
pub fn check_json_program(prog: &Program) -> (String, bool) {
    on_deep_stack(|| match crate::checker::check_resolutions(prog) {
        Ok((warnings, _html, _ufcs, _ovl, _reified)) => {
            (crate::diagnostic::diagnostics_json(&[], &warnings), false)
        }
        Err(errs) => (crate::diagnostic::diagnostics_json(&errs, &[]), true),
    })
}

/// `transpile` on an already-loaded program (emit PHP). Multi-namespace emission for a multi-package
/// project is S2c; S2b emits the existing flat form (correct for `package Main` / single-package).
/// THE LADDER RULE (MASTER-PLAN G-rules; first applications: concurrency, `Core.Db`, `Core.Mail`):
/// a native-only Core module — one whose semantics have no faithful PHP byte-identity mapping (live
/// DB I/O, SMTP delivery) — HARD-ERRORS on transpile with a module-specific `E-TRANSPILE-<FEATURE>`
/// code. Never a silent degrade, and never the wall of prelude-internal errors the check would
/// otherwise produce. New native-only module = one row here.
fn reject_native_only_transpile(prog: &Program) -> Result<(), String> {
    const NATIVE_ONLY: &[(&[&str], &str, &str)] = &[
        (
            &["Core", "Db"],
            "E-TRANSPILE-DB",
            "`Core.Db` is native-only: live database I/O cannot be byte-identical across the phorj drivers and PHP PDO, so transpiling it is refused rather than silently diverging (THE LADDER RULE). Run this program with `phg run` / `phg runvm`.",
        ),
        (
            &["Core", "Fs"],
            "E-TRANSPILE-FS",
            "`Core.Fs` is native-only for now: its typed FsError protocol has no PHP emitter yet (PHP has faithful filesystem functions, so a real mapping is a recorded future lift — refusing beats emitting a silently-diverging program, THE LADDER RULE). Run this program with `phg run`, or use the transpilable `Core.File` subset.",
        ),
        (
            &["Core", "HttpClient"],
            "E-TRANSPILE-HTTPCLIENT",
            "`Core.HttpClient` is native-only: live network I/O cannot be byte-identical across the phorj client and a PHP mapping, so transpiling it is refused rather than silently diverging (THE LADDER RULE). A faithful curl-mapping is a recorded future lift. Run this program with `phg run`.",
        ),
        (
            &["Core", "Mail"],
            "E-TRANSPILE-MAIL",
            "`Core.Mail` is native-only (DEC-223): PHP's mail() has no SMTP auth, no TLS, and is header-injection-prone — any mapping would silently drop auth/TLS/attachments (THE LADDER RULE forbids the downgrade). Run this program with `phg run`.",
        ),
    ];
    use crate::ast::Item;
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

pub fn transpile_program(prog: &Program, diag_src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        reject_native_only_transpile(prog)?;
        let checked = check_and_expand(prog, diag_src)?;
        crate::transpile::emit(&checked)
    })
}

/// `serve` on an already-loaded program (M6 W4): type-check, build the request handler factory, then
/// run the blocking HTTP serve loop ([`crate::serve::serve_tcp`]) until the process is killed. Defaults
/// to the **bytecode VM** (faster than the tree-walker — measured ~2.3× lower end-to-end latency on a
/// representative handler; byte-identical via [`Vm::run_entry`] ≡ `call_named`); `tree_walker` selects
/// the interpreter oracle (`phg serve --tree-walker`, the
/// exact pre-VM behaviour). The single-threaded path runs on the 256 MB deep-stack worker (native-stack
/// headroom for re-entrant natives / the interpreter's deep recursion). Note: `--workers N` pool
/// threads are plain `std::thread::spawn` (default ~8 MB stack), not the deep-stack worker — the VM is
/// iterative so it is far less exposed than the tree-walker was, but a `--tree-walker` pool worker has
/// less headroom than the single-threaded path (pre-existing; unchanged by this slice).
pub fn serve_program(
    prog: &Program,
    diag_src: &str,
    addr: &str,
    timeout: Option<std::time::Duration>,
    profile: crate::profile::Profile,
    workers: usize,
    tree_walker: bool,
) -> Result<String, String> {
    on_deep_stack(|| {
        // Reified side-table is threaded into the VM compile (Invariant 6); the interp path ignores it.
        let (checked, reified) = check_and_expand_reified(prog, diag_src)?;
        let checked = std::sync::Arc::new(checked);
        let factory = if tree_walker {
            crate::serve::interp_factory(checked)
        } else {
            crate::serve::vm_factory(checked, std::sync::Arc::new(reified))
                .map_err(|e| e.to_string())?
        };
        crate::serve::serve_tcp(factory, addr, timeout, profile, workers)
            .map_err(|e| format!("serve: {e}"))?;
        Ok(String::new())
    })
}

/// Build a standalone executable for the host from `src`. `input_path` names the source (used to
/// derive the default output name); `out_path` overrides it. Validates the program first (never emits
/// a broken binary), then delegates to `bundle::cross::build_host`, which reuses this phg binary as
/// the stub and embeds `src` as a `.phorj` section. Returns a one-line success message.
pub fn cmd_build(
    input_path: &str,
    src: &str,
    out_path: Option<&str>,
    profile: crate::profile::Profile,
) -> Result<String, String> {
    cmd_check(src)?; // validate; emit nothing on failure
    let out = match out_path {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            let stem = std::path::Path::new(input_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| format!("cannot derive output name from {input_path}"))?;
            std::path::PathBuf::from(stem)
        }
    };
    crate::bundle::cross::build_host(src, &out, profile)
}

/// `parse`: lex -> parse; dump the AST.
pub fn cmd_parse(src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        let prog = lex_parse(src)?;
        Ok(format!("{prog:#?}\n"))
    })
}

/// `lex`: dump the token stream.
pub fn cmd_tokenize(src: &str) -> Result<String, String> {
    let tokens = lex(src).map_err(|e| e.to_string())?;
    let mut out = String::new();
    for t in tokens {
        out.push_str(&format!("{:?} @ {}:{}\n", t.kind, t.span.line, t.span.col));
    }
    Ok(out)
}

/// `lift`: read PHP source, emit a Phorj **draft** (the inverse of `transpile`). Best-effort and
/// review-required — the output is prefixed with a `// lifted (verify)` banner so the contract is
/// visible in the file itself. Anything outside the Tier-1 lift subset is a clear `lift …` error
/// (never a silent guess). No `on_deep_stack`: the lift parser has its own depth guard.
pub fn cmd_lift(src: &str) -> Result<String, String> {
    let phorj = crate::lift::lifter::lift_source(src)?;
    Ok(format!(
        "// lifted (verify) — a best-effort PHP->Phorj draft; review before trusting it.\n{phorj}"
    ))
}

/// `transpile`: lex -> parse -> native-only ladder gate -> check (gate) -> emit PHP source.
pub fn cmd_transpile(src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        let raw = lex_parse(src)?;
        reject_native_only_transpile(&raw)?;
        let prog = parse_checked(src)?;
        crate::transpile::emit(&prog)
    })
}

/// `disasm`: lex -> parse -> check (gate) -> compile -> dump the bytecode the VM will execute.
/// A read-only window onto the backend: per-function instruction listings and the program-level
/// descriptor tables. The op mnemonic is `Op`'s own `Debug`, *not* a hand-written match — so a new
/// `Op` variant appears here automatically with no second match surface to drift out of lockstep
/// (see memory `op-variant-match-coupling`); the per-op annotation is display-only with a `_`
/// fall-through, so an un-annotated new op simply shows no comment rather than failing to compile.
pub fn cmd_disassemble(src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        let (prog, reified) = parse_checked_reified(src)?;
        let program = compile_with(&prog, &reified).map_err(|e| e.to_string())?;
        Ok(disasm_program(&program))
    })
}

/// Resolve a human-readable annotation for an index-carrying op (the value a `Const` loads, the
/// callee of a `Call`, the field/method/variant/class a member op names). Display-only: the `_`
/// arm covers every op that needs no comment, so this never has to track the full `Op` set.
pub(super) fn annotate(op: &Op, chunk: &Chunk, p: &BytecodeProgram) -> Option<String> {
    match op {
        Op::Const(i) => chunk.consts.get(*i).map(|v| format!("{v:?}")),
        Op::Call(idx) => p
            .functions
            .get(*idx)
            .map(|f| format!("-> {}/{}", f.name, f.arity)),
        Op::GetField(i) => p.names.get(*i).map(|n| format!(".{n}")),
        Op::CallMethod(i, argc) => p.names.get(*i).map(|n| format!(".{n}(argc={argc})")),
        Op::CallNative(i, argc) => crate::native::registry()
            .get(*i)
            .map(|n| format!("-> {}.{}(argc={argc})", n.module, n.name)),
        Op::MakeEnum(i) | Op::MatchTag(i) => p
            .enum_descs
            .get(*i)
            .map(|d| format!("{}::{}", d.ty, d.variant)),
        Op::GetEnumField(i) => Some(format!("payload #{i}")),
        Op::MakeInstance(i) => p.class_descs.get(*i).map(|d| d.class.to_string()),
        _ => None,
    }
}

/// Format a whole [`BytecodeProgram`] as a disassembly listing. Descriptor tables are emitted only
/// when non-empty; the method table is sorted (HashMap iteration order is non-deterministic —
/// invariant #8) so the output is stable across runs.
pub(super) fn disasm_program(p: &BytecodeProgram) -> String {
    let mut out = format!(
        "phg disassemble — {} function(s), main = #{}\n",
        p.functions.len(),
        p.main
    );
    if !p.enum_descs.is_empty() {
        out.push_str("\nenum descriptors:\n");
        for (i, d) in p.enum_descs.iter().enumerate() {
            out.push_str(&format!("  #{i} {}::{}/{}\n", d.ty, d.variant, d.arity));
        }
    }
    if !p.class_descs.is_empty() {
        out.push_str("\nclass descriptors:\n");
        for (i, d) in p.class_descs.iter().enumerate() {
            out.push_str(&format!(
                "  #{i} {} {{ {} }}\n",
                d.class,
                d.fields.join(", ")
            ));
        }
    }
    if !p.methods.is_empty() {
        out.push_str("\nmethods:\n");
        let mut entries: Vec<_> = p.methods.iter().collect();
        entries.sort();
        for ((class, name), idx) in entries {
            out.push_str(&format!("  {class}::{name} -> #{idx}\n"));
        }
    }
    for (fi, f) in p.functions.iter().enumerate() {
        out.push_str(&format!("\nfn #{fi} {}/{}:\n", f.name, f.arity));
        for (ip, op) in f.chunk.code.iter().enumerate() {
            let line = f.chunk.lines.get(ip).copied().unwrap_or(0);
            match annotate(op, &f.chunk, p) {
                Some(a) => out.push_str(&format!("  {ip:>4}  L{line:<4} {op:?}  ; {a}\n")),
                None => out.push_str(&format!("  {ip:>4}  L{line:<4} {op:?}\n")),
            }
        }
    }
    out
}
