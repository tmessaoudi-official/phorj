#!/usr/bin/env bash
# microbench-leg.sh - pre-push's microbench leg: microbench-gate.sh measures and judges; this script applies the
# environment's policy and keeps the OWED list (scripts/owed-legs.sh) honest (DEC-597, DEC-598; DEC-365's NO-HIDDEN-LOSS).
#
#   - An OUTPUT-IDENTITY break blocks everywhere: it is a correctness signal, not timing (DEC-598: P0).
#   - On the PC, a confirmed WIN->LOSS flip blocks (DEC-598: a P1 regression), as the gate always did.
#   - In a claude.ai cloud session (CLAUDE_CODE_REMOTE=true) the gate measures regardless of load, so the identity half
#     runs whenever a PHP comparator resolves; ratios from a shared VM are never a verdict, so every cloud run, flip or
#     not, leaves `microbench` OWED for a quiet PC (two cloud runs flipped two different benches, 2026-11-23).
#   - A SKIP (load, no comparator, source mismatch, no binary) or a pass that carries an unconfirmed "verdict OWED"
#     leaves `microbench` OWED; only a real pass on the PC clears it (a JSON-seam pass is not a measurement).
#   - A bench missing from the baseline, or RECOVERED, leaves `microbench-baseline` OWED until a PC pass shows neither
#     (the ratchet does not protect it until someone re-emits on a quiet PC: DEC-431.1, DEC-365).
#
#   scripts/microbench-leg.sh            # what pre-push runs
#   scripts/microbench-leg.sh --verify   # the step that clears the debt: release build first, a long settle; run it
#                                        # on the PC, through scripts/dk, when the box is quiet (DEC-598: first work)
# Exit: 0 = nothing blocks (the OWED list says what is still owed), 1 = regression, 2 = setup error.
# Seams (tests): MICROBENCH_GATE_SCRIPT, plus the gate's own (MICROBENCH_GATE_JSON, MICROBENCH_BASELINE).
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATE="${MICROBENCH_GATE_SCRIPT:-$root/scripts/microbench-gate.sh}"
owed() { (cd "$root" && bash scripts/owed-legs.sh "$@"); }
env=pc; [[ "${CLAUDE_CODE_REMOTE:-}" == true ]] && env=cloud
if [[ "$env" == cloud ]]; then export MICROBENCH_MAX_LOAD=1000; fi   # measure anyway: the ratios are not trusted there
if [[ "${1:-}" == --verify ]]; then
  [[ "$env" == pc ]] || { echo "microbench-leg: --verify clears debt and runs on the PC only (DEC-598)" >&2; exit 2; }
  (cd "$root" && cargo build --release) || exit 2   # measure HEAD, never a stale binary
  export MICROBENCH_SETTLE_SECS="${MICROBENCH_SETTLE_SECS:-900}"
fi

log="$(mktemp)"; trap 'rm -f -- "$log"' EXIT
bash "$GATE" 2>&1 | tee "$log"; rc=${PIPESTATUS[0]}
[[ "$rc" == 2 ]] && exit 2
grep -qE '^ *FAIL [a-z0-9_]+: output-identity break' "$log" && exit 1   # the bench's own line, not the summary

reason() { grep -m1 -E "$1" "$log" | sed -E 's/^ *(microbench-gate: )?//'; }
if [[ "$rc" != 0 ]]; then
  [[ "$env" == cloud ]] || exit 1
  owed record microbench cloud "unconfirmed timing flips on a shared VM: $(grep -E '^ *FAIL [a-z0-9_]+:' "$log" | sed -E 's/^ *FAIL ([a-z0-9_]+):.*/\1/' | sort -u | tr '\n' ' ')"
  exit 0
fi
if ! grep -q 'PASS (ratchet' "$log"; then
  owed record microbench "$env" "$(reason 'SKIP|OWED|absent' || true)"
  exit 0
fi
if grep -q 'verdict OWED' "$log"; then
  owed record microbench "$env" "$(reason 'verdict OWED')"
elif [[ "$env" == cloud ]]; then
  owed record microbench cloud "ratios measured only on a shared VM (identity verified there)"
elif [[ -z "${MICROBENCH_GATE_JSON:-}" ]]; then
  owed clear microbench "microbench-gate PASS"
fi
new="$(grep -E '^ *(note [a-z0-9_]+: not in baseline|RECOVERED [a-z0-9_]+)' "$log" | sed -E 's/^ *(note|RECOVERED) ([a-z0-9_]+).*/\2/' | sort -u | tr '\n' ' ')"
if [[ -n "$new" ]]; then
  owed record microbench-baseline "$env" "not protected by the ratchet until re-emitted on a quiet PC: $new"
elif [[ "$env" == pc && -z "${MICROBENCH_GATE_JSON:-}" ]]; then
  owed clear microbench-baseline "microbench-gate PASS with every bench in the baseline"
fi
exit 0
