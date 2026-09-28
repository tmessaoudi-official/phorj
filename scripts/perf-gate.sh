#!/usr/bin/env bash
# perf-gate.sh — Lane 2 W1: fail on a gross VM *regression*.
#
# THIS IS A VM-HEALTH REGRESSION SIGNAL, NOT A PERFORMANCE-VS-PHP CLAIM. It gates on `vm_speedup`
# (tree-walk ÷ VM ratio) from `phg benchmark --json`: a machine-independent measure — both backends
# run on the same CPU in the same process, so a faster/slower host scales them together and the ratio
# stays put, whereas absolute nanoseconds swing wildly across machines and CI runners. It flags when
# the VM slows *relative to a fixed reference* (the tree-walker); it makes NO claim about beating PHP.
#
# The G-8 MANDATE ("the VM is faster than release-php+JIT, per feature") is gated SEPARATELY by
# scripts/microbench.sh (VM vs `docker run php:8.5-cli`, per-feature WIN-count) — that is the honest
# perf-vs-PHP bar. This gate and that one measure different things; keep both.
#
# Timing noise is ONE-directional: the scheduler, GC, and thermal throttling only ever *slow* a run,
# never speed it up past the true achievable time. (Empirically a run measured 3.27 against a ~22 true
# value.) So the least-perturbed estimator is best-of-N (max speedup = min time), which this script
# uses — not mean or min. The floor is generous (baseline * min_ratio) so the gate flags gross (~1.7x+)
# regressions, not micro-noise; baseline_vm_speedup sits below the observed best so a slower CI
# ratio-regime does not false-fail.
#
# Config lives in bench/baseline.json. Exit 0 = pass, 1 = regression, 2 = setup error (including a
# malformed baseline value, and a run with no measurable vm_speedup — behaviour suite:
# scripts/test-perf-gate.sh).
# Env: PHG_BIN (default target/release/phg), PERF_GATE_RUNS (default from baseline).
set -eEuo pipefail
# Force the C locale so awk's printf uses '.' as the decimal separator (a fr_FR locale emits '10,8000',
# which awk then re-parses as just 10 — a silent floor corruption). All numeric formatting here must be
# locale-independent.
export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN="${PHG_BIN:-$ROOT/target/release/phg}"
BASELINE="$ROOT/bench/baseline.json"

command -v jq >/dev/null 2>&1 || { echo "perf-gate: jq is required" >&2; exit 2; }
[[ -x "$BIN" ]] || { echo "perf-gate: binary not found at $BIN — run: cargo build --release" >&2; exit 2; }
[[ -f "$BASELINE" ]] || { echo "perf-gate: baseline not found at $BASELINE" >&2; exit 2; }

workload="$(jq -r '.workload' "$BASELINE")"
baseline_speedup="$(jq -r '.baseline_vm_speedup' "$BASELINE")"
min_ratio="$(jq -r '.min_ratio' "$BASELINE")"
runs="${PERF_GATE_RUNS:-$(jq -r '.runs' "$BASELINE")}"
# Every value above is DATA (bench/baseline.json is tracked, PERF_GATE_RUNS is env) and is checked before
# use: `(( ))` evaluates array subscripts, so runs="PWD[$(cmd)]" would RUN cmd; and awk reads a missing
# key (jq prints `null`) or text as 0 — floor 0.0000, which passes every run. An explicit digit list, not
# a [0-9] range (locale-dependent); 10# because a leading 0 is octal to bash. A bad value is a setup error.
# ≤ 9 digits: a longer count wraps in bash's intmax (2^64+100000 reads as 100000), which failed OPEN.
is_count() { case "$1" in '' | *[!0123456789]*) return 1 ;; esac; ((${#1} <= 9)); }
is_positive() { [[ "$1" =~ ^[0123456789]+(\.[0123456789]+)?$ ]] && awk -v x="$1" 'BEGIN{exit (x>0)?0:1}'; }
if ! is_count "$runs" || ((10#$runs == 0)); then
  printf 'perf-gate: runs must be a positive decimal count, got %q\n' "$runs" >&2
  exit 2
fi
runs=$((10#$runs))
for pair in "baseline_vm_speedup=$baseline_speedup" "min_ratio=$min_ratio"; do
  is_positive "${pair#*=}" || { printf 'perf-gate: %s must be a positive number in %s, got %q\n' "${pair%%=*}" "$BASELINE" "${pair#*=}" >&2; exit 2; }
done
floor="$(awk -v b="$baseline_speedup" -v r="$min_ratio" 'BEGIN{printf "%.4f", b*r}')"

printf 'perf-gate: workload=%q ' "$workload" # %q: a tracked string — never print control bytes raw
echo "baseline_speedup=$baseline_speedup min_ratio=$min_ratio floor=$floor runs=$runs"

best="0"
for ((i = 1; i <= runs; i++)); do
  s="$("$BIN" benchmark --json "$ROOT/$workload" | jq -r '.vm_speedup')"
  # The MEASURED value is data too: `phg benchmark --json` emits null when a leg measures 0 ns, and awk
  # compares a non-number to the floor AS A STRING ("null" >= "10.8" is true) — one such run used to
  # PASS the gate, even beside real regressions in best-of-N. Not a number = setup error, never a sample.
  if ! [[ "$s" =~ ^[0123456789]+(\.[0123456789]+)?([eE][-+]?[0123456789]+)?$ ]]; then
    printf 'perf-gate: run %s gave no measurable vm_speedup (%q) — setup error, not a sample\n' "$i" "$s" >&2
    exit 2
  fi
  echo "  run $i: vm_speedup=$s"
  best="$(awk -v a="$best" -v b="$s" 'BEGIN{print (b>a)?b:a}')"
done

echo "perf-gate: best-of-$runs vm_speedup=$best (floor=$floor)"
if awk -v m="$best" -v f="$floor" 'BEGIN{exit (m>=f)?0:1}'; then
  echo "perf-gate: PASS"
else
  pct="$(awk -v r="$min_ratio" 'BEGIN{printf "%.0f", (1-r)*100}')"
  echo "perf-gate: FAIL — best vm_speedup $best fell below floor $floor (a >${pct}% VM regression vs baseline $baseline_speedup)" >&2
  exit 1
fi
