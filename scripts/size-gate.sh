#!/usr/bin/env bash
# Phorj file-size gate (Invariant 13 — ratified 2026-07-02, amended 2026-07-16 DEC-262).
# Soft cap 300 lines / hard cap 500 lines per source file, "everything organized/structured/
# decoupled into clear many files".
#
# RATCHET semantics (Invariant 13: "applies to new code immediately, to existing files as
# M-Decomp reaches them"): the 90 files already over 500 at gate-introduction are grandfathered in
# scripts/size-baseline.txt at their then-current line count. The gate enforces:
#   * a NON-grandfathered file may not exceed the 500 HARD cap        -> FAIL (new breach)
#   * a grandfathered file may not GROW beyond its baseline count     -> FAIL (must only shrink)
#   * a file over the 300 SOFT cap that is not grandfathered          -> WARN (advisory)
# So existing debt is frozen and can only burn down; no new or growing breach is allowed.
#
# When a grandfathered file is split below 500, drop its row from scripts/size-baseline.txt so the
# ratchet tightens (the gate WARNs when a baseline row is now comfortably under, as a reminder).
#
# Usage: bash scripts/size-gate.sh        (exit 1 on any FAIL, 0 otherwise; WARN never fails)
set -euo pipefail

_root="$(git rev-parse --show-toplevel 2>/dev/null || echo .)"
cd "$_root"

SOFT=300
HARD=500
BASELINE="scripts/size-baseline.txt"

# Load grandfather ceilings: file -> baseline line count.
declare -A ceiling=()
if [[ -f "$BASELINE" ]]; then
  while IFS=$'\t' read -r cnt path; do
    [[ -n "${path:-}" ]] && ceiling["$path"]="$cnt"
  done < "$BASELINE"
fi

fails=0
warns=0
stale=0

while IFS= read -r -d '' f; do
  lines=$(wc -l < "$f")
  rel="${f#./}"
  if [[ -n "${ceiling[$rel]:-}" ]]; then
    cap="${ceiling[$rel]}"
    if (( lines > cap )); then
      echo "FAIL (grandfathered file grew): $rel = $lines > baseline $cap — split it, do not grow it"
      fails=$((fails+1))
    elif (( lines <= HARD )); then
      echo "note (grandfathered file now under hard cap — drop from $BASELINE): $rel = $lines"
      stale=$((stale+1))
    fi
  else
    if (( lines > HARD )); then
      echo "FAIL (new hard-cap breach >$HARD): $rel = $lines — split by cohesion (M-Decomp)"
      fails=$((fails+1))
    elif (( lines > SOFT )); then
      echo "warn (soft cap >$SOFT): $rel = $lines"
      warns=$((warns+1))
    fi
  fi
done < <(find src -name '*.rs' -print0)

echo "[size-gate] grandfathered=${#ceiling[@]} fails=$fails warns=$warns stale=$stale"
if (( fails > 0 )); then
  echo "[size-gate] FAILED — $fails file(s) breach Invariant 13 (300 soft / 500 hard)."
  exit 1
fi
echo "[size-gate] OK (no new or growing hard-cap breach)"
