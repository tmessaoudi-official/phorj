//! # JIT backend (Cranelift) — scaffold
//!
//! **Status: SCAFFOLD ONLY — no codegen yet.** This module is the in-tree home of the future
//! Cranelift JIT backend, the ruled path to the G-8 perf mandate (the bytecode VM is ~28× slower than
//! release-php+JIT on hot numeric loops; only native codegen closes that — see
//! `docs/plans/perf-wave.plan.md` and `docs/research/jit-aot-design-exploration.md`).
//!
//! ## Why in-tree (not a separate crate)
//!
//! The JIT is a 4th backend intimately coupled to `Op`/`Value`/chunk (invariants #3/#4/#6), all of
//! which live in this single `phorj` lib crate; the CLI dispatch (`cli::mod`) and the
//! bench/disassemble/playground compile paths are lib code too. A separate `phorj-jit` crate would
//! force those internals `pub` and create a `phorj → phorj-jit → phorj` dependency cycle whose
//! cleanest fix is a vtable in the very hot path the JIT exists to eliminate. So the JIT lives here.
//!
//! ## The `unsafe` discipline (when codegen lands)
//!
//! A JIT's call path (`finalize → transmute(buf → fn ptr) → call`) is `unsafe` in phorj's own code —
//! this will be phorj's FIRST first-party `unsafe` (the four existing external deps confine their
//! unsafe to third-party crates). Per the ruling (`docs/specs/UNIFIED-SPEC.md`
//! §"External dependency policy", 2026-07-06 amendment): the crate root drops from
//! `#![forbid(unsafe_code)]` to `#![deny(unsafe_code)]` and this module — and ONLY this module — carries
//! an audited `#![allow(unsafe_code)]` island. `deny` (unlike `forbid`) permits that scoped `allow`, and
//! CI (`.github/workflows/ci.yml`, the `unsafe-island` gate) fails the build if an
//! `#[allow(unsafe_code)]`/`#![allow(unsafe_code)]` escape hatch appears anywhere outside `src/jit/`,
//! machine-enforcing "first-party unsafe lives only in the JIT."
//!
//! The `cranelift-jit` dependency (dependency-policy domain #7, `jit` feature, non-wasm) and the
//! `forbid → deny` root change both land WITH the first codegen slice — not in this scaffold, which
//! keeps the crate `#![forbid(unsafe_code)]` and unsafe-free.
