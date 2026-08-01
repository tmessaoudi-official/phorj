#!/usr/bin/env bash
# test-microbench-gate.sh — behaviour tests for the G-8 ratchet (scripts/microbench-gate.sh).
#
# **Why this exists.** The gate is the only thing standing between a perf regression and master, and it
# had NO tests — the `MICROBENCH_GATE_JSON` seam was built "for tests" and nothing used it. That is how
# it managed to be DARK for weeks without anyone noticing (DEC-423: docker is absent in the dev
# container, so the gate skipped on every push and printed OWED). A gate nobody tests is a gate nobody
# can trust is running.
#
# Every case below drives the real script through the JSON seam — no docker, no php, no timing, fully
# deterministic — and asserts the EXIT CODE plus the specific message. The five behaviours pinned are
# exactly the five the gate promises:
#
#   1. a clean run PASSES, and reports every OWED loss it is carrying;
#   2. an OWED loss that DEEPENS blocks (a carried loss may not quietly get worse);
#   3. a genuine WIN->LOSS flip blocks (the G-8 ratchet itself);
#   4. an OWED loss that RECOVERS passes, and says to re-emit so the ratchet starts protecting it;
#   5. an output-identity break blocks (a correctness bug, not a timing).
#
# Plus the one that is easy to get wrong: a NEAR-PARITY wobble must NOT block. `mapinsert` has a
# baseline of 1.012, and an absolute-only band flagged it at 0.940 — a 7% swing on a shared box. That
# would have wedged every push, which is why the flip band is relative to the baseline as well.
set -uo pipefail
export LC_ALL=C

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATE="$ROOT/scripts/microbench-gate.sh"
BASELINE="$ROOT/bench/micro-baseline.json"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

command -v jq >/dev/null 2>&1 || { echo "test-microbench-gate: jq required — SKIP" >&2; exit 0; }
[[ -f "$BASELINE" ]] || { echo "test-microbench-gate: no baseline — SKIP" >&2; exit 0; }

# Synthesize a run that MATCHES the baseline exactly: every feature at its recorded ratio, identical.
# Derived from the baseline itself, so the fixture can never drift out of sync with it.
jq -r '[.features | to_entries[] | {feature: .key, vm_ns: 1000, php_ns: 1000, ratio: .value.ratio, identical: true}]' \
  "$BASELINE" >"$TMP/clean.json"

fails=0
# `want_exit want_pattern label json` — run the gate on `json`, assert exit code and a message match.
check() {
  local want_exit="$1" want_pat="$2" label="$3" json="$4"
  local out rc
  out="$(MICROBENCH_GATE_JSON="$json" bash "$GATE" 2>&1)"
  rc=$?
  if [[ "$rc" != "$want_exit" ]]; then
    echo "  FAIL $label: exit $rc, want $want_exit"
    echo "$out" | sed 's/^/        /' | tail -5
    fails=$((fails + 1))
    return
  fi
  if ! grep -qE "$want_pat" <<<"$out"; then
    echo "  FAIL $label: no match for /$want_pat/"
    echo "$out" | sed 's/^/        /' | tail -5
    fails=$((fails + 1))
    return
  fi
  echo "  ok   $label"
}

# The first OWED feature in the baseline, whatever it happens to be — the tests must not hardcode a
# feature name that a future fix removes from the list.
owed_feat="$(jq -r '._owed | keys[0] // empty' "$BASELINE")"
owed_ratio="$(jq -r --arg f "$owed_feat" '._owed[$f].ratio' "$BASELINE")"
# The biggest WIN, for the flip test — likewise derived.
win_feat="$(jq -r '[.features | to_entries[] | select(.value.ratio >= 2)] | sort_by(.value.ratio) | last | .key // empty' "$BASELINE")"

echo "test-microbench-gate: owed=$owed_feat ($owed_ratio) win=$win_feat"

check 0 'PASS \(ratchet' "1 clean run passes" "$TMP/clean.json"

if [[ -n "$owed_feat" ]]; then
  check 0 "owed $owed_feat" "1b clean run REPORTS the owed loss" "$TMP/clean.json"

  # 2. Deepen it well past the 0.75 band.
  jq --arg f "$owed_feat" --argjson r "$(awk -v o="$owed_ratio" 'BEGIN{print o*0.4}')" \
    '(.[] | select(.feature==$f) | .ratio) = $r' "$TMP/clean.json" >"$TMP/deepen.json"
  check 1 "OWED loss DEEPENED" "2 a deepened owed loss blocks" "$TMP/deepen.json"

  # 4. Recover it to a clear WIN.
  jq --arg f "$owed_feat" '(.[] | select(.feature==$f) | .ratio) = 1.5' \
    "$TMP/clean.json" >"$TMP/recover.json"
  check 0 "RECOVERED $owed_feat" "4 a recovered owed loss passes with a re-emit note" "$TMP/recover.json"
fi

if [[ -n "$win_feat" ]]; then
  jq --arg f "$win_feat" '(.[] | select(.feature==$f) | .ratio) = 0.5' \
    "$TMP/clean.json" >"$TMP/flip.json"
  check 1 "WIN->LOSS flip" "3 a real win->loss flip blocks" "$TMP/flip.json"
fi

# 5. Identity break on any feature.
any_feat="$(jq -r '.[0].feature' "$TMP/clean.json")"
jq --arg f "$any_feat" '(.[] | select(.feature==$f) | .identical) = false' \
  "$TMP/clean.json" >"$TMP/ident.json"
check 1 "output-identity break" "5 an output-identity break blocks" "$TMP/ident.json"

# 6. A near-parity WIN wobbling below 1.0 must WARN, never block — the mapinsert case.
near="$(jq -r '[.features | to_entries[] | select(.value.ratio >= 1.0 and .value.ratio < 1.05)] | first | .key // empty' "$BASELINE")"
if [[ -n "$near" ]]; then
  jq --arg f "$near" '(.[] | select(.feature==$f) | .ratio) = 0.94' \
    "$TMP/clean.json" >"$TMP/wobble.json"
  check 0 "near-parity wobble" "6 a near-parity wobble warns, does not block" "$TMP/wobble.json"
else
  echo "  skip 6 near-parity wobble: no baseline feature in [1.0, 1.05)"
fi

if [[ "$fails" -gt 0 ]]; then
  echo "test-microbench-gate: FAIL — $fails case(s)" >&2
  exit 1
fi
echo "test-microbench-gate: OK"
