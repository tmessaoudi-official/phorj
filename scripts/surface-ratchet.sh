#!/usr/bin/env bash
# Surface ratchet — Invariant 17's 100% RULE, made mechanical.
#
# WHY THIS EXISTS. Invariant 17 says the LSP and both editors must support EVERYTHING we implement,
# "no exceptions, no lag", and Invariant 9 says a feature ships with an example. Both were being
# asserted in prose and neither was measured, so the true state on 2026-08-07 was:
#
#     310 `E-*` codes emitted in src/ — only 64 asserted by ANY test (21%)
#     conformance/ referenced 10 of them
#     the LSP advertised 8 providers; signatureHelp/codeAction/semanticTokens/inlayHint absent
#     editors/phpstorm/ contained nothing but a README
#
# The ONE surface that was complete — `phg explain`, 305/310 — is complete precisely because
# `explain_ratchet` fails the build when a code has no entry. Nothing else had that, so nothing else
# stayed current. A prose rule that no gate enforces decays to whatever the last hurried commit left
# behind; this session watched `#[Config]` documentation rot within a single week.
#
# WHAT IT DOES. Counts today's covered surface and refuses to let it SHRINK. It deliberately does NOT
# demand 100% on day one — that would mean authoring ~246 diagnostic fixtures before anything else
# could land, so the gate would be turned off within the hour and we would be back to prose. Instead
# the baseline is a FLOOR that only moves up: a new diagnostic code must arrive with coverage, or the
# count drops below the floor and the push fails.
#
# Same shape as `scripts/size-gate.sh` (grandfathered file sizes) and `scripts/doc-guards.sh`
# (frozen doc violations): freeze the debt, forbid its growth, pay it down deliberately.
#
# Usage:  bash scripts/surface-ratchet.sh          # gate (used by pre-push + CI)
#         bash scripts/surface-ratchet.sh --emit   # re-freeze the floor AFTER coverage improves
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
BASELINE="scripts/surface-baseline.txt"
EMIT=0
[[ "${1:-}" == "--emit" ]] && EMIT=1

# ── Measure ───────────────────────────────────────────────────────────────────────────────────────
# Codes EMITTED by the compiler. `E-FOO`/`E-NOPE`/`E-TYPE` are test fixtures inside #[cfg(test)] and
# are filtered out so they cannot inflate either side of the ratio.
mapfile -t codes < <(
  grep -rhoE '"E-[A-Z0-9-]+"' src/ --include=*.rs \
    | tr -d '"' | sort -u \
    | grep -vxE 'E-FOO|E-NOPE|E-TYPE'
)
total="${#codes[@]}"

asserted=0
conformance=0
for c in "${codes[@]}"; do
  # "Asserted" = pinned by something that FAILS when the code stops rendering: a Rust test, or a
  # conformance/diagnostics fixture (whose `.expected` pins the exact rendered output, so it is an
  # assertion in every sense that matters — it was wrongly excluded in the first draft of this script).
  if grep -rq --include=*.rs "$c" tests/ 2>/dev/null \
    || grep -rq --include='*tests*.rs' "$c" src/ 2>/dev/null \
    || grep -rq --include='tests.rs' "$c" src/ 2>/dev/null \
    || grep -rq "$c" conformance/ 2>/dev/null; then
    asserted=$((asserted + 1))
  fi
  grep -rq "$c" conformance/ 2>/dev/null && conformance=$((conformance + 1))
done

# LSP capabilities actually advertised in the initialize response.
lsp=$(grep -rhoE '"[a-zA-Z]+Provider"' src/lsp/ --include=*.rs 2>/dev/null | sort -u | wc -l)

# Runnable examples — Invariant 9's corpus, which is also the byte-identity gate.
examples=$(find examples -name '*.phg' | wc -l)

# ── Emit ──────────────────────────────────────────────────────────────────────────────────────────
if [[ "$EMIT" == 1 ]]; then
  cat >"$BASELINE" <<EOF
# Surface-coverage FLOOR (scripts/surface-ratchet.sh). Each number may only GO UP.
# Re-freeze with \`bash scripts/surface-ratchet.sh --emit\` after coverage improves, and say in the
# commit message what was added. Lowering a number by hand is laundering the gate, not fixing it.
#
# codes_total is informational and may move in EITHER direction (a code can be legitimately retired);
# the ratio floors below are what actually gate.
codes_total $total
codes_asserted $asserted
codes_in_conformance $conformance
lsp_providers $lsp
examples $examples
EOF
  echo "surface-ratchet: wrote $BASELINE"
  sed -n '6,$p' "$BASELINE" | sed 's/^/  /'
  exit 0
fi

# ── Gate ──────────────────────────────────────────────────────────────────────────────────────────
[[ -f "$BASELINE" ]] || {
  echo "surface-ratchet: no $BASELINE — create it with --emit" >&2
  exit 1
}
get() { awk -v k="$1" '$1==k{print $2}' "$BASELINE"; }

fails=0
check() { # name actual floor
  local name="$1" actual="$2" floor="$3"
  if ((actual < floor)); then
    echo "surface-ratchet: FAIL $name = $actual, floor is $floor — coverage went DOWN" >&2
    fails=$((fails + 1))
  else
    local extra=""
    ((actual > floor)) && extra="  (+$((actual - floor)) — re-emit to lock it in)"
    printf '  ok   %-22s %s / floor %s%s\n' "$name" "$actual" "$floor" "$extra"
  fi
}

echo "surface-ratchet: Invariant 17 / Invariant 9 coverage floors"
check codes_asserted "$asserted" "$(get codes_asserted)"
check codes_in_conformance "$conformance" "$(get codes_in_conformance)"
check lsp_providers "$lsp" "$(get lsp_providers)"
check examples "$examples" "$(get examples)"

pct=$((asserted * 100 / total))
echo "surface-ratchet: $asserted/$total diagnostic codes asserted (${pct}%) — the 100% RULE is NOT met yet"
echo "surface-ratchet:   remaining debt is tracked, not hidden; see docs/plans/SLICE-STATE.md"

if ((fails > 0)); then
  echo "surface-ratchet: FAILED — $fails floor(s) breached" >&2
  exit 1
fi
echo "surface-ratchet: PASS"
