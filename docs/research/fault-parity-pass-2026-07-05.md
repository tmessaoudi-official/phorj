# Correct-lens fault-parity pass — 2026-07-05

Deferred from the B-2d rich-error audit / DEC-195. **Lens:** the byte-identity spine (Invariant 1)
requires that where a Phorj native faults, the transpiled PHP must *also* fault — a native that faults
in Phorj but SILENTLY SUCCEEDS in PHP (raw builtin returns a value instead of throwing) is a real
divergence needing a `__phorj_*` guard helper (the proven `__phorj_clamp`/`__phorj_gcd` pattern).

## Method

For each **reachable value-guard** native fault (excluding `expects (types)` signature guards — those
are checker-unreachable — and test-only faults), construct a minimal valid-typed fault-triggering
program, transpile it, run it under `php-8.5.8`, and compare **exit status**: non-zero PHP = consistent
(both fault; text need not match); zero PHP = a real "Phorj-faults / PHP-succeeds" divergence.

## Findings

### 1. Exit-status parity: the reachable value-guard set is CONSISTENT (no divergence)

PHP 8.5 is strict — it throws `ValueError` on the bad-value cases that Phorj faults on. Empirically
verified (rust_ec / php_ec, both non-zero = consistent):

| trigger | native | Rust | PHP 8.5 | verdict |
|---|---|---|---|---|
| `String.repeat("x", -1)` | `str_repeat` | fault | `ValueError` | consistent |
| `String.count("abc", "")` | `substr_count` | fault | `ValueError` | consistent |
| `String.padLeft/padRight("x",5,"")` | `str_pad` | fault | `ValueError` | consistent |
| `List.fill(0, -1)` (count<0) | `array_fill` | fault | `ValueError` | consistent |
| `List.chunk([…], 0)` | `array_chunk` | fault | `ValueError` | consistent (confirms B-2d) |
| `Hash.hkdf` len out of range | — | fault | `ValueError` | consistent (confirms B-2d) |

Conversion faults are guarded by construction: `toInt`→`int?` (null on out-of-range, single-sourced
with `value::float_to_int`), `floatToIntExact`/`decimalToIntExact`→`__phorj_*_exact` throwing helpers,
`toString`→`__phorj_str` (faults in kind). No coercing string-parse (`intval("abc")=0`-style) native
exists in `Core.Conversion`. So the exit-status lens finds **no divergence**.

### 2. OUTPUT divergence found — `Conversion.truncate` / `Conversion.round` on out-of-range floats

A different (but real) class: BOTH legs succeed, but produce DIFFERENT stdout.

- `Conversion.truncate(1.0e30)` → Rust `1e30 as i64` **saturates** = `9223372036854775807` (i64::MAX);
  PHP `(int)(1.0e30)` **wraps** = `5076964154930102272` (+ a "not representable" warning). **run ≠ php.**
- `Conversion.round(1.0e30)` → same class (`(int)round(...)`).
- NaN/±∞ input: consistent (both fault — Rust via the div-by-zero producing NaN then… actually
  `truncate(NaN)` faults on the VM path; PHP `(int)` of the NaN expression faults on the `0.0/0.0`).

Root cause: `truncate`/`round` are the "always returns `int`" conversions and emit a **raw** `(int)` /
`(int)round(...)` cast, which is lenient (wraps, warns) where Rust's `as i64` saturates. No example
uses an out-of-range input, so the differential never exercises it — a **latent** byte-identity gap.

Safe siblings already exist: `toInt` (→ `int?`, null on overflow) and `floatToIntExact` (faults). Only
the total `truncate`/`round` variants diverge.

## Disposition

Finding 1: no action (spine is sound for the exit-status lens on the reachable value-guard set).

Finding 2: the FIX is a user-visible semantic choice (what should `truncate`/`round` do on an
out-of-i64-range float?) → **surfaced to the developer (Invariant 15), not self-ruled.** Options:
fault (like `*Exact`); or emit a `__phorj_trunc`/`__phorj_round` helper that saturates identically to
Rust; or another rule. Once ruled, the fix is a `__phorj_*` guarded emit + a differential case with an
out-of-range input (`truncate(1e30) + 0`), same-commit.

## Output-parity sweep (2026-07-05, follow-up) — high-risk raw-builtin natives

Probed the highest-risk raw-builtin emits for OUTPUT divergence (both succeed, different stdout) and
FAULT divergence (one faults, one doesn't), comparing `run`/`runvm`/`php-8.5.8`:

| native | emit | edge input | verdict |
|---|---|---|---|
| `String.substring` | `substr` | start past end / len past end / negative start | **AGREE** (PHP `substr` clamps like Rust) |
| `Math.integerDivide` | `intdiv` | `i64::MIN / -1` (overflow) | **AGREE** (both fault: PHP `ArithmeticError`, Rust checked) |
| `Math.pow` | `pow` | `pow(0.0, -1.0)` | **AGREE on value** (`inf`); PHP adds a *deprecation warning* only — the known **UA-0.14** disclosure, not a stdout-value divergence |
| `Math.pow` | `pow` | `pow(-8.0, 0.5)` | **AGREE** (`NaN` both) |
| `String.split` | `explode` | `split(s, "")` empty separator | **DIVERGENCE** — Rust returns `["","a","b","c",""]`; PHP `explode("")` **faults** (ValueError). Phorj succeeds, PHP faults. **FIX → surfaced (Invariant 15).** |

Not exhaustive: this covered the highest-risk numeric/string raw-builtins. ~50 more raw-builtin emits
(array ops, libm math, hash, path, url) are lower-risk (type-safe homogeneous collections; shared
IEEE-754 / libc semantics) but NOT individually probed — the remaining systematic sweep is still a
fresh-context follow-up.
