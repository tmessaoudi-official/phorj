#!/usr/bin/env bash
# microbench-leg.sh - pre-push's microbench leg: microbench-gate.sh measures and judges; this script applies the
# environment's policy and keeps the OWED list (scripts/owed-legs.sh) honest (DEC-597, DEC-598; DEC-365's NO-HIDDEN-LOSS).
#
#   - An OUTPUT-IDENTITY break blocks everywhere: it is a correctness signal, not timing (DEC-598: P0).
#   - On the PC, a confirmed WIN->LOSS flip blocks (DEC-598: a P1 regression), as the gate always did.
#   - In a claude.ai cloud session (CLAUDE_CODE_REMOTE=true) the gate measures regardless of load, so the identity half
#     runs whenever a PHP comparator resolves; ratios from a shared VM are never a verdict (two cloud runs flipped two
#     different benches, 2026-11-23), so every cloud run PRINTS what is owed, writes no entry and never holds the push
#     for a ratio or for infra (an identity break, a malformed baseline and a phg that fails in the harness still do).
#   - On the PC, a SKIP (load, no comparator, source mismatch, no binary) or a pass that carries an unconfirmed
#     "verdict OWED" leaves `microbench` OWED; only a real PC pass clears it (a JSON-seam pass is not a measurement).
#   - On the PC, a bench missing from the baseline, or RECOVERED, leaves `microbench-baseline` OWED until a pass shows neither
#     (the ratchet does not protect it until someone re-emits on a quiet PC: DEC-431.1, DEC-365).
#
#   scripts/microbench-leg.sh            # what pre-push runs
#   scripts/microbench-leg.sh --verify   # the step that clears `microbench`: release build first, a long settle; run
#                                        # it on the PC when the box is quiet (DEC-598: first work). It re-executes
#                                        # through scripts/dk itself. `microbench-baseline` is cleared by a quiet-PC
#                                        # `scripts/microbench-gate.sh --emit` (D3), then a pass.
# Exit: 0 = nothing blocks (the OWED list says what is still owed), 1 = regression, 2 = setup error.
# Seams (tests): MICROBENCH_GATE_SCRIPT, plus the gate's own (MICROBENCH_GATE_JSON, MICROBENCH_BASELINE).
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# MICROBENCH_ONLY is this leg's own (the confirming re-run below): an inherited one would make the main run partial,
# and a partial pass must never clear debt (6C 2026-11-23).
unset MICROBENCH_ONLY
# Like the hooks: on a PC host, run in the toolchain image (never a host cargo or php, CLAUDE.md); pre-push already
# runs inside it, and a host-runner checkout (a cloud session) runs here.
if [[ "${PHORJ_IN_DOCKER:-}" != 1 && "$(git -C "$root" config --get phorj.runner || true)" != host ]]; then
  exec "$root/scripts/dk" bash "$root/scripts/microbench-leg.sh" "$@"
fi
GATE="${MICROBENCH_GATE_SCRIPT:-$root/scripts/microbench-gate.sh}"
env=pc; [[ "${CLAUDE_CODE_REMOTE:-}" == true ]] && env=cloud
# A cloud session writes no entry and is never refused for one (DEC-597 refinement, 2026-11-23): it PRINTS what is
# owed, and the developer's push after landing runs this leg on the landed tree and records what it cannot judge.
owed() {
  if [[ "$env" == cloud && "$1" == record ]]; then
    echo "microbench-leg: OWED (cloud, not recorded: the PC push after landing judges it) $2: $4"; return 0
  fi
  (cd "$root" && bash scripts/owed-legs.sh "$@")
}
# A cloud run measures anyway: past the load guard, and past a php SOURCE MISMATCH (a cloud host without docker measures
# on its own php8.5, not the baseline's `docker php:8.5-cli`), since output identity needs no baseline. Its ratios are
# never a verdict; everything below reports them OWED (printed in the cloud, recorded on the PC: owed() above).
if [[ "$env" == cloud ]]; then export MICROBENCH_MAX_LOAD=1000 MICROBENCH_IDENTITY_ONLY=1; fi
if [[ "${1:-}" == --verify ]]; then
  [[ "$env" == pc ]] || { echo "microbench-leg: --verify clears debt and runs on the PC only (DEC-598)" >&2; exit 2; }
  (cd "$root" && cargo build --release) || exit 2   # measure HEAD, never a stale binary
  export MICROBENCH_SETTLE_SECS="${MICROBENCH_SETTLE_SECS:-900}"
fi

log="$(mktemp)"; trap 'rm -f -- "$log"' EXIT
bash "$GATE" 2>&1 | tee "$log"; rc=${PIPESTATUS[0]}
if [[ "$rc" == 2 ]]; then
  # In a cloud session a harness that cannot run (a docker pull, the host php) is infra: OWED, never a held push. A
  # setup error in the repo (a malformed baseline) still stops it, as on the PC, and so does a phg that fails on a
  # bench (scripts/microbench.sh names it: a phorj defect, never infra; cloud proof run 2026-11-23, F2).
  if [[ "$env" == cloud ]] && grep -q 'harness run failed' "$log" && ! grep -q 'malformed baseline' "$log" \
     && ! grep -q '^microbench: phg failed on ' "$log"; then
    owed record microbench cloud "the harness could not run in a cloud session: harness run failed"; exit 0
  fi
  exit 2
fi
grep -qE '^ *FAIL [a-z0-9_]+: output-identity break' "$log" && exit 1   # the bench's own line, not the summary

reason() { grep -m1 -E "$1" "$log" | sed -E 's/^ *(microbench-gate: )?//'; }
if [[ "$rc" != 0 ]]; then
  [[ "$env" == cloud ]] || exit "$rc"
  # Only timing verdicts become OWED: every FAIL line a flip or a deepening, and at least one. Anything else (a gate that
  # died, a FAIL of another kind) propagates, never read as noise.
  fails="$(grep -E '^ *FAIL [a-z0-9_]+:' "$log" || true)"
  timing="$(grep -E '^ *FAIL [a-z0-9_]+: (WIN->LOSS flip|an OWED loss DEEPENED)' <<<"$fails" || true)"
  [[ -n "$fails" && "$fails" == "$timing" ]] || exit "$rc"
  owed record microbench cloud "unconfirmed timing verdicts on a shared VM: $(sed -E 's/^ *FAIL ([a-z0-9_]+):.*/\1/' <<<"$timing" | sort -u | tr '\n' ' ')"
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
# A bench missing from the baseline is a fact; a RECOVERED line is one sample, so on the PC it is re-measured first
# (MICROBENCH_ONLY: the gate on those benches alone) and opens `microbench-baseline` only when the re-run agrees, as a
# flip is confirmed (developer ruling 2026-11-23: a near-parity bench, deepjson 0.975 <-> 1.269, flapped the entry
# open and shut on alternate pushes). A re-run that cannot measure leaves the entry as it was. In the cloud a
# RECOVERED line compares with a baseline measured on another comparator: an artefact, never counted.
names() { grep -E "^ *($1)" "$2" | sed -E 's/^ *(note|RECOVERED) ([a-z0-9_]+).*/\2/' | LC_ALL=C sort -u | tr '\n' ' '; }
new="$(names 'note [a-z0-9_]+: not in baseline' "$log")"
rec=""; unknown=0
if [[ "$env" == pc ]]; then
  rec="$(names 'RECOVERED [a-z0-9_]+' "$log")"
  if [[ -n "$rec" && -z "${MICROBENCH_GATE_JSON:-}" ]]; then
    echo "microbench-leg: re-measuring RECOVERED bench(es) to confirm: $rec"
    log2="$(mktemp)"; trap 'rm -f -- "$log" "$log2"' EXIT
    MICROBENCH_ONLY="${rec% }" bash "$GATE" 2>&1 | tee "$log2"
    grep -qE '^ *FAIL [a-z0-9_]+: output-identity break' "$log2" && exit 1
    if grep -q 'PASS (ratchet' "$log2"; then
      conf="$(names 'RECOVERED [a-z0-9_]+' "$log2")"
      noise="$(LC_ALL=C comm -23 <(tr ' ' '\n' <<<"$rec" | sed '/^$/d') <(tr ' ' '\n' <<<"$conf" | sed '/^$/d') | tr '\n' ' ')"
      [[ -z "$noise" ]] || echo "microbench-leg: RECOVERED not confirmed on re-measure (noise, not recorded): $noise"
      rec="$conf"
    else
      unknown=1; echo "microbench-leg: the confirming re-run could not measure; microbench-baseline is left as it was"
    fi
  fi
fi
new="$new$rec"
if [[ -n "${new// }" ]]; then
  owed record microbench-baseline "$env" "not protected by the ratchet until re-emitted on a quiet PC: $new"
elif [[ "$env" == pc && -z "${MICROBENCH_GATE_JSON:-}" && "$unknown" == 0 ]]; then
  owed clear microbench-baseline "microbench-gate PASS with every bench in the baseline"
fi
exit 0
