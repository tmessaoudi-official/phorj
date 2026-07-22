# Phorj Invariants

The load-bearing rules that keep Phorj correct. Each is non-obvious, easy to break with a
plausible-looking change, and enforced somewhere concrete. Read this before touching the backends,
the value kernels, or the `Op` set. (Companion to `docs/ARCHITECTURE.md` for the layout, and the
frozen design records in `docs/specs/`.)

## 1. Backend parity is the spine — `run` ≡ the VM, byte-identical
The tree-walking interpreter (`phg run`) and the bytecode VM (`phg run`) must produce
**identical stdout *and* identical failure behaviour** for every program. This is the project's
central correctness contract.
- **Enforced by** `tests/differential.rs`: `agree(src)` compares the `Ok` output; `agree_err(src)`
  compares *failures* by semantic `FaultKind` (matched on the fault **body** substring —
  `"division by zero"`, `"integer overflow"`, … — not the raw rendered string, because the CLI
  adds per-stage prefixes and the VM adds a source-line prefix the interpreter lacks).
- **Why it bites:** the original `Op::Neg` P0 (negating `i64::MIN`) hid in the gap that existed
  before `agree_err` — the Ok-only oracle never saw divergent *crashes*.
- **Third surface (M2.5 `phg build`):** a standalone binary runs its **embedded source** through
  `cli::cmd_the VM leg` at startup (the self-detect hook in `src/main.rs`), so its output MUST equal
  `phg run <file>`. **Enforced by** `tests/build.rs::built_binary_matches_the VM leg`. The startup
  hook must keep dispatching through `cmd_the VM leg` (never `cmd_run`) and must not transform the source
  before execution — otherwise the distribution layer silently drifts off the spine while the
  differential suite (which never builds a binary) stays green.
  - **Cross-targets (Phase 2):** the surface now spans cross-built binaries. The stub-cache key is the
    **FNV-1a-64 of the running phg binary's bytes**, so a rebuilt phorj ⇒ cache miss ⇒ fresh stub —
    a stale stub can never embed your source into an *old* VM. Cross-parity is gated by
    `cross_musl_binary_matches_the VM leg` (native exec) and `cross_windows_section_round_trips` (PE section
    round-trip). The object-file section readers (ELF/PE/Mach-O/fat) honor **EV-7**: every offset uses
    checked arithmetic, and malformed/adversarial images return `None`, never a panic or OOB read.

## 2. The interpreter is the reference oracle
When the VM and the interpreter disagree, the **interpreter is right by definition** — it is the
older, simpler implementation and the semantics of record. New VM behaviour is validated *against*
the interpreter, never the reverse.

## 3. Arithmetic & comparison are single-sourced in `value.rs`
The checked integer kernels (`int_add/sub/mul/div/rem/neg → Result<i64, String>`), the float
kernels (`float_*`), and `compare_ord` live **once**, in `src/value.rs`. Both backends call them.
- **Never** re-inline `checked_*` / `partial_cmp` / a fault string in `interpreter.rs` or `vm.rs` —
  that re-opens the dual-implementation drift that caused the `Op::Neg` P0.
- The three canonical fault bodies (`FAULT_DIV_ZERO`, `FAULT_MOD_ZERO`, `FAULT_INT_OVERFLOW`) are
  `pub const` in `value.rs`; the `agree_err` oracle classifies on these exact bodies, so changing a
  body string is a parity-affecting change.

**`int` is a fixed 64-bit signed integer (`i64`), pinned by design.** Unlike PHP's `int`, whose width
is platform-dependent (32-bit on a 32-bit build, 64-bit elsewhere), Phorj's `int` is **always** `i64`
on every target — the backends all carry `Value::Int(i64)` and the transpile floor (PHP 8.5) is 64-bit
in practice, so the width never varies. `int` arithmetic that would leave the `i64` range is a **checked
fault** (`FAULT_INT_OVERFLOW`, EV-7 — never a silent wrap, unlike PHP which auto-promotes an overflowing
`int` to `float`). `float` is IEEE-754 double (`f64`). `decimal` is an exact i128-carried fixed-point
value (M-NUM S1). Conversions between these are **explicit** natives (`Core.Conversion`) — there is no
implicit coercion.

## 4. Float display parity — `12.0` renders as `"12"`
`printLine`/interpolation render via `Value::as_display`, which formats floats with Rust `{}`
(`12.0 → "12"`, design rule EV-6). Both backends use the same method, so the transpiled PHP and
both runtimes agree. Don't introduce a second formatting path.

## 5. Adding an `Op` variant requires its match arm in the same commit
A new `Op` touches **three** match surfaces, and as of M9 **all three are exhaustive (no `_`
wildcard)** — so a missing arm is a *compile error*, never a silent hole:
- `vm::Vm::exec_op` (`src/vm.rs`) — the per-op execution semantics (irreducibly per-`Op`).
- `compiler::Compiler::stack_effect` (`src/compiler.rs`) — the net stack delta.
- `BytecodeProgram::validate` (`src/chunk.rs`) — the operand-index bounds check (EV-7). Until M9
  this carried a `_ => None` wildcard, so a new index-carrying `Op` could silently skip its bounds
  check; it now enumerates every variant (index-checked arms via `.then(|| …)` + one explicit
  no-index `=> None` arm), matching the other two. **Do not reintroduce a `_` wildcard here** — it
  is the forcing function that makes the bounds check un-skippable.

A new `Op` that carries a *pool* index (`Const`, `Call`, jumps, `MakeInstance`/`GetField`/`MatchTag`,
`MakeClosure`, …) must add its bounds arm to `validate`; one that carries only a count or local slot
goes in the no-index arm. Either way the compiler now refuses to build until you choose.

## 6. No crash on input (EV-7)
Malformed or adversarial `.phg` must exit 1 with a clean `Diagnostic`, **never** SIGABRT/panic.
- The whole pipeline runs on a 256 MB worker thread (`cli::on_deep_stack`) so the *explicit* depth
  limits — not Rust's ambient stack — bound recursion.
- Limits are centralised in `src/limits.rs`: `MAX_CALL_DEPTH`, `MAX_NEST_DEPTH` (parser),
  `MAX_EXPR_DEPTH` (checker). Both backends share `MAX_CALL_DEPTH` — a single limit, no
  divergence band.
- `BytecodeProgram::validate` turns would-be out-of-range panics into clean errors before the VM
  runs a single op.

## 7. Errors are one `Diagnostic`; backend position-asymmetry is deliberate
All stages produce `diagnostic::Diagnostic { stage, message, line, col }`. `Display` has three
forms (`line==0` → no position; `col==0` → `at <line>` (VM runtime); else `at <line>:<col>`
(front-end)). Both backends now attach a source line to runtime faults (the VM via
`Chunk.lines[ip]`, the interpreter via its stack-trace frames) and they agree for ordinary faults.
**Known limitation (fault-line skew — W0-5 / H §5):** a fault raised *inside* a `"{…}"` string
interpolation is the one exception — `run` reports the true line, but the VM reports **line 1**
(stack-trace frames likewise), because `parser::split_interpolation` re-lexes the inner expression
with a fresh lexer that resets to line 1 and the VM has no scope IP ranges to recover the real line.
Message, `FaultKind`, and exit code still agree, so the body-substring oracle (#1), `agree_err`, and
the CLI differential all stay green — only the line diverges. Pinned by the `#[ignore]`d
`interpolation_fault_line_matches_between_backends` gate in `tests/differential.rs`; the fix needs
VM debug symbols (scope IP ranges) and is scheduled **W5-13**.

## 8. Determinism — sort `HashMap`-derived lists before rendering
Any user-facing list built from `HashMap`/`HashSet` iteration must be sorted before `join`, or the
output varies with the hash seed. Live example: the non-exhaustive-`match` error in `checker.rs`
sorts the missing-variant list. New diagnostics that enumerate map keys must do the same.

## 9. The AST is untyped; backends re-derive types
The checker validates **without annotating** the AST. Backends that need a type re-derive it
structurally from the declared `Type` annotations — the compiler via `compiler::CTy` + `ctype`/
`num_ty` (M2 Wave 4 made this class-aware: `CTy::Class(name)` carries an instance's class so
`obj.field`/`obj.m()`/class-typed payloads resolve). Don't assume a node carries a resolved type.

## 10. The quality gate is a compile-time + pre-commit invariant
`#![deny(unsafe_code)]` on both crate roots (relaxed from `forbid` for the JIT: its
finalize→transmute→fn-ptr `unsafe` is the sole first-party island, confined to `src/jit/` behind the
CI `unsafe-island` gate); `[lints] warnings = "deny"` + `clippy.all = "deny"` in `Cargo.toml` (so a
warning *fails the build*); the toolchain is pinned (`rust-toolchain.toml`) to keep that gate
reproducible. The tracked `scripts/git-hooks/pre-commit` runs `fmt --check` + `clippy -Dwarnings` +
`test`. Green means: `cargo test` + `cargo clippy --all-targets` + `cargo fmt --check` + `cargo build
--release`, all clean. As of 2026-07-09 `jit` is a **default feature**, so those bare commands include
native codegen; additionally verify the jit-off path with `cargo check --no-default-features` (with
`jit` off the only way to build, that path can otherwise bit-rot).

## 11. No perf change without a measured before/after
`phg benchmark <file>` (median-of-N, output-identity gated) is the baseline tool. Any
perf-motivated change (Copy-on-`Op`, deep-copy elimination, dispatch tweaks) must ship with a
before/after number from it — perf claims are **Verified**, not asserted.

## 12. Keyword-vs-import 3-way rule — "nothing in the wind" is about *symbols*, not *grammar*
Phorj's namespace decision ("everything namespaced, nothing in the wind") governs **symbols**
(functions and user/library types), never the **type grammar**. There are exactly three tiers, and
the boundary is not negotiable per-file:

1. **Built-in types — never imported (keyword-class).** They are part of the type grammar, like
   `if`/`for`. The named set is single-sourced in `is_builtin_type_name` (`src/checker/common.rs`):
   `int float bool string bytes decimal double i8..i64 u8..u64 void never empty`, the containers
   `List Map Set`, the markers/handles `Error Channel Task`, and `Html Attr`. The **structural**
   type forms are equally import-free: optional `T?`, function types `(A, B) => R`, unions `A | B`,
   intersections `A & B`, ranges `a..b`. You never write `import Core.Types.int` — importing a
   primitive would break familiarity-first (no mainstream language does it) and buy nothing.
2. **User / library types — `import Pkg.Path.Name [as Alias];`.** A `class`/`enum`/`interface`/`trait`
   defined in another package is reached through a single `import` whose path resolves to a type —
   the resolver then binds the **bare type name** (`import Acme.Geometry.Rect;` → `Rect`). The former
   `import type` keyword was **removed** in the 2026-07-03 unified-import model and no longer parses
   (see `docs/specs/UNIFIED-SPEC.md` §"Unified import and injected-type discipline"). No wildcard
   (PHP has no `use A\*`).
3. **Stdlib *functions* — `import Core.X;` then leaf-qualified calls (`X.fn(...)`).** e.g.
   `import Core.Output;` → `Output.printLine(...)`. Functions are always qualified; there is no bare
   global.

Corollary (rejected designs, M-DOGFOOD): **no forced import of primitives** and **no `Integer`/
`Float`/`Decimal` object wrappers** — method-on-primitive is UFCS (`n.abs()`), nullability is `T?`,
primitives-in-generics already work (`List<int>`), and `decimal` is already a primitive. A wrapper
tier would be Java autoboxing: redundant capability plus surprise.

## 13. JIT unboxed speculation — every faulting op sets sticky OR branches to code 5 (MUST-CHECK)
The unboxed JIT codegen (`build_body_unboxed`, `--features jit`) is **speculative**: int arithmetic
uses WRAPPING ops and records overflow in a sticky-flag Cranelift `Variable` instead of branching
per-op (the "ovf-spec" slice). Correctness rests on a coupling invariant, the same class as the
`Op`-variant and CTy-operand MUST-CHECKs:

- **Every faulting op in the unboxed subset must either OR its fault condition into `sticky` (for
  non-trapping ops — arith overflow, `ineg` of `MIN`) or branch to the shared fault-exit with code 5
  (for hardware-trapping ops — `sdiv`/`srem` zero and `MIN/-1`, and the `Op::Call` depth cap).** At
  every loop **back-edge** AND every `Return`, `sticky != 0` ⇒ exit code 5.
- Code 5 = "redo on VM": `run_unboxed` maps it to `JitRun::Fault(REDO_ON_VM)`; the b3b `Op::Call` hook
  (`src/vm/exec.rs`) re-runs the callee on the VM, whose per-op CHECKED arithmetic is the single
  source of fault truth (renders the exact, correctly-ordered fault + line). The unboxed path never
  emits a user-facing fault string itself.
- **Widening the subset to a new faulting op (shift, checked `as`, `pow`, float faults, …) that
  forgets this = a SILENT byte-identity P0**: a speculatively-wrapped success would mask a VM fault
  (return a wrong value where the VM faults). Add the sticky/branch AND a differential/end-to-end case
  in the SAME commit.
- **The back-edge guard is mandatory, not optional hardening.** Without it, speculation can extend a
  loop past the VM's fault point and even non-terminate where the VM faults (`while (i != 0) { i = i *
  3; }`: `3^k mod 2^64` is never 0 → infinite native loop vs a ~40-iter VM overflow fault). The guard
  bounds native to ≤1 partial iteration past the first overflow. Guard tests:
  `ovf_spec_*` in `src/jit/tests.rs` (end-to-end `cmd_run` vs `cmd_treewalk`).

**Float ops (slice v1).** Floats live in the unboxed `vars` as their f64 BITS (an i64); code `bitcast`s
I64↔F64 only at the float op. `AddF/SubF/MulF` are TOTAL (overflow → inf, never a fault) → NO sticky,
NO branch. `DivF` faults ONLY on a zero divisor (`value::float_div`: `b == 0.0`, incl. `-0.0`) →
`fcmp Equal b, 0.0` → the same code-5 redo (NaN/inf divisors do NOT fault: `fcmp Equal` is false for
NaN, matching `Ok(a/b)`). `RemF` is EXCLUDED (no native Cranelift `frem`; `fmod` libcall deferred).
Two soundness limits the untyped bytecode forces (v1): **(a) leaf-only** — a function with any float op
must have no `Op::Call` (the Call arm models a callee return as `Int`, so a float through a call would
mis-decode); **(b) comparisons need a known-non-float operand** — `icmp` is only valid on integer bits,
and a float param used only in a comparison is `Unknown` (bytecode-identical to an int one), so a
comparison is rejected unless ≥1 operand is a KNOWN `Int`/`Bool` (else `icmp` on f64 bits = silent
byte-identity bug). Both limits lift once param types are threaded into the bytecode. `Return` accepts
`Float` and records `Compiled.ret_kind` (asserted consistent across all reachable entry Returns) — the
sole signal telling `run_unboxed` to decode the i64 return as `Value::Float(from_bits)` vs `Value::Int`.

## 14. Standing rules from the 2026-07-16 full-reopen audit (delivery invariants 13/16/17/18 in CLAUDE.md)

Recorded here so this file stays the one-stop invariant read; the normative text lives in
`CLAUDE.md` (invariants 13, 16–18) and the rulings in `C-decisions.md` §2026-07-16.

- **File-size cap: soft 300 / hard 500 lines** per source file (DEC-262 — amends the old 800/1000).
  Split-as-you-go is the default; split by cohesion (M-Decomp), never by line count alone.
- **META-7 — cross-language scan + byte-identity-is-a-tool.** Before designing anything meant to
  beat PHP, survey how other languages solved it. Emitting a `__phorj_*` helper to keep the PHP leg
  identical is always an acceptable tool — but the trade is ALWAYS surfaced to the developer, never
  self-decided.
- **`phg check` ≡ LSP diagnostics** (DEC-252): same pipeline, never diverge. The LSP must see the
  same injected-prelude world `check` sees.
- **Transpile AND lift updated in the same change** as every language/stdlib feature — a feature
  that runs but doesn't transpile/lift (or vice versa) is not done. Editors both-same-change
  (DEC-181) unchanged.
- **Perf-bench doctrine** (DEC-259/267): everything with a PHP equivalent is benched against it
  (I/O via fixtures); real-application MACRO benches join the suite; WIN-OR-FLAG applies to all
  of it.
