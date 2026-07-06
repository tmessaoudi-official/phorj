#!/usr/bin/env bash
# microbench-gate.sh — the G-8 mandate RATCHET gate (pre-push lane, docker).
#
# Consumes `microbench.sh --json` (per-feature phorj-VM vs release-PHP+JIT) and gates against
# bench/micro-baseline.json. It BLOCKS a push ONLY on the two ROBUST, load-insensitive signals:
#   - OUTPUT-IDENTITY break (identical == false — VM and release-php disagree; a correctness bug, and
#     bench micros are NOT in the differential, so this is their only parity check).
#   - WIN->LOSS FLIP: a feature whose baseline ratio (php_ns/vm_ns) was a WIN (>= 1 — the VM beat php)
#     now LOSES (< 1). This IS the G-8 ratchet: once the VM beats release-php+JIT on a feature, it must
#     keep beating it. (Today every feature LOSES, so this arms for when the JIT lands wins.)
#
# Per-feature ratio deltas are REPORTED, NOT blocked on: absolute microbench ns/ratio is too noisy to
# gate on a shared dev machine — empirically 3-4x swings at load average ~7, with NO code change. The
# robust VM-perf-regression gate is scripts/perf-gate.sh (same-process tree÷VM ratio: both backends
# share the CPU so load cancels — load-immune, unlike native-VM-vs-docker-php here). The two gates are
# complementary: perf-gate = "the VM didn't slow down"; this = "we didn't lose a feature we'd won" + parity.
#
# Usage:  microbench-gate.sh           gate the current tree (exit 1 on a flip/identity break)
#         microbench-gate.sh --emit    (re)write bench/micro-baseline.json from a fresh best-of-K run
# Env:    MICROBENCH_GATE_JSON=<file>  use that microbench-JSON instead of running the harness
#                                      (docker-free, deterministic — for tests); microbench.sh's own
#                                      (PHG_BIN, MICROBENCH_RUNS, MICROBENCH_PHP_IMAGE) otherwise.
# Requires docker + the release binary (unless the JSON seam is set). Either absent => SKIP with a
# warning (a push is never wedged by missing infra). Exit 0 pass/skip, 1 regression, 2 setup error.
set -eEuo pipefail
export LC_ALL=C

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASELINE="${MICROBENCH_BASELINE:-$ROOT/bench/micro-baseline.json}"
EMIT=0
[[ "${1:-}" == "--emit" ]] && EMIT=1

command -v jq >/dev/null 2>&1 || {
  echo "microbench-gate: jq is required" >&2
  exit 2
}

# Acquire the measurement JSON: the testing seam bypasses docker/binary/harness entirely.
if [[ -n "${MICROBENCH_GATE_JSON:-}" ]]; then
  [[ -f "$MICROBENCH_GATE_JSON" ]] || {
    echo "microbench-gate: MICROBENCH_GATE_JSON=$MICROBENCH_GATE_JSON not found" >&2
    exit 2
  }
  json="$(cat "$MICROBENCH_GATE_JSON")"
else
  if ! command -v docker >/dev/null 2>&1; then
    echo "microbench-gate: docker absent — SKIP the G-8 mandate gate (infra, not a regression)" >&2
    exit 0
  fi
  BIN="${PHG_BIN:-$ROOT/target/release/phg}"
  if [[ ! -x "$BIN" ]]; then
    echo "microbench-gate: release binary $BIN absent — SKIP (run: cargo build --release; infra, not a regression)" >&2
    exit 0
  fi
  json="$(bash "$ROOT/scripts/microbench.sh" --json)" || {
    echo "microbench-gate: harness run failed" >&2
    exit 2
  }
fi

if [[ "$EMIT" == 1 ]]; then
  jq '{
    "_comment": "G-8 mandate ratchet baseline (scripts/microbench-gate.sh). Per-feature php/vm ratio + output-identity vs release-php+JIT (docker php:8.5-cli). The gate BLOCKS on identity breaks and WIN->LOSS flips (ratio crossing 1.0 downward) — NOT on ratio magnitude (too noisy on a shared machine; perf-gate.sh is the robust VM-regression gate). RATCHET: re-emit after the JIT lands a WIN so the flip check protects it. ratio<1 = the VM still LOSES to php (the JIT is the lever).",
    features: (map({ (.feature): { ratio: .ratio, identical: .identical } }) | add)
  }' <<<"$json" >"$BASELINE"
  echo "microbench-gate: wrote baseline -> $BASELINE ($(jq '.features | length' "$BASELINE") features)"
  exit 0
fi

[[ -f "$BASELINE" ]] || {
  echo "microbench-gate: no baseline at $BASELINE — run: bash scripts/microbench-gate.sh --emit" >&2
  exit 2
}

fails=0
wins=0
while IFS=$'\t' read -r feat ratio identical; do
  [[ -n "$feat" ]] || continue
  if [[ "$identical" != "true" ]]; then
    echo "  FAIL $feat: output-identity break (VM vs PHP checksum differ) — a correctness bug, not a timing"
    fails=$((fails + 1))
    continue
  fi
  b_ratio="$(jq -r --arg f "$feat" '.features[$f].ratio // empty' "$BASELINE")"
  win_now="$(awk -v r="$ratio" 'BEGIN{print (r>=1.0)?"WIN":"loss"}')"
  [[ "$win_now" == "WIN" ]] && wins=$((wins + 1))
  if [[ -z "$b_ratio" ]]; then
    echo "  note $feat: not in baseline (new) — ratio=$ratio ($win_now); run --emit to snapshot it"
    continue
  fi
  # BLOCK: a feature we had WON now LOSES (the G-8 ratchet).
  if awk -v br="$b_ratio" -v r="$ratio" 'BEGIN{exit (br>=1.0 && r<1.0)?0:1}'; then
    echo "  FAIL $feat: WIN->LOSS flip — baseline ratio $b_ratio (WIN) now $ratio (LOSS): a G-8 mandate regression"
    fails=$((fails + 1))
    continue
  fi
  # REPORT (non-blocking): ratio movement vs baseline.
  echo "  ok   $feat: ratio $b_ratio -> $ratio ($win_now)"
done < <(jq -r '.[] | [.feature, .ratio, .identical] | @tsv' <<<"$json")

echo "microbench-gate: $wins WIN / $(($(jq 'length' <<<"$json") - wins)) loss vs release-php+JIT; $fails blocking regression(s)"
if [[ "$fails" -gt 0 ]]; then
  echo "microbench-gate: FAIL — $fails regression(s) (WIN->LOSS flip or output-identity break)" >&2
  exit 1
fi
echo "microbench-gate: PASS (ratchet: no flips, all output-identical)"
