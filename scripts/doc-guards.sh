#!/usr/bin/env bash
# DEC-362 — the three mechanical documentation guards, plus the diagnostic-code extension.
# Documentation rot was measured as the project's dominant defect class (60+ dangling `src/` refs,
# 13 DEC ids with no register row, cursors pinning orphanable bare SHAs). Prose cannot enforce this;
# these can. Wired into `pre-push`.
#
#   G1  every `src/….rs` path named in tracked markdown exists                  [baselined]
#   G2  every `DEC-nnn` mentioned in markdown has a row in the register         [HARD, no baseline]
#   G3  a commit SHA in docs/plans carries a ref or a subject, never bare       [baselined]
#   G4  every diagnostic code named in the register exists in `src/`            [baselined]
#
# WHY G1/G3/G4 are baselined and G2 is not: the first three have real pre-existing volume that would
# make the guard un-landable as a hard failure, so they follow the `size-baseline.txt` precedent —
# freeze today's set, fail only on NEW violations, burn the baseline down over time. G2 had only three
# violations, so they were FIXED instead and the guard is hard from day one.
#
# G4's baseline doubles as a useful artifact in its own right: the list of diagnostic codes the
# decision register PROMISES but `src/` does not yet implement (queued features like DEC-360's
# `W-UNUSED-*` family, DEC-370's `E-TRANSPILE-PARALLEL-NO-PHP`). A code leaving that list without
# appearing in `src/` is the phantom class this guard exists to catch (`E-RETIRED-FORIN`, the dead
# `E-MULTIPLE-MAIN`, Invariant 14's non-existent `--sequential-concurrency` flag — all found by hand).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
REGISTER="docs/research/full-audit/raw/C-decisions.md"
BASELINE="scripts/doc-guards-baseline.txt"
WRITE_BASELINE="${DOC_GUARDS_WRITE_BASELINE:-0}"

fails=0
new_baseline="$(mktemp)"
trap 'rm -f "$new_baseline"' EXIT
known() { grep -qxF "$1" "$BASELINE" 2>/dev/null; }

# Tracked markdown only: an untracked scratch file must never be able to fail a push.
mapfile -t MD < <(git ls-files '*.md')

# ── G1 — markdown `src/….rs` references must exist ───────────────────────────────────────────────
g1=0
while read -r ref; do
  [[ -z "$ref" ]] && continue
  [[ -f "$ref" ]] && continue
  echo "G1:$ref" >>"$new_baseline"
  if ! known "G1:$ref"; then
    echo "[doc-guards] G1 FAIL: markdown names a non-existent path: $ref" >&2
    g1=$((g1 + 1))
  fi
done < <(grep -ohE '\bsrc/[a-zA-Z0-9_/]+\.rs' "${MD[@]}" 2>/dev/null | sort -u)

# ── G2 — every DEC id mentioned must have a register row (HARD) ───────────────────────────────────
g2=0
while read -r dec; do
  [[ -z "$dec" ]] && continue
  grep -qE "(\| *$dec *\||^#+ .*$dec\b|\*\*$dec\b)" "$REGISTER" && continue
  echo "[doc-guards] G2 FAIL: $dec is referenced in markdown but has no row in $REGISTER" >&2
  g2=$((g2 + 1))
done < <(grep -ohE '\bDEC-[0-9]{3}\b' "${MD[@]}" 2>/dev/null | sort -u)

# ── G3 — a SHA in docs/plans must carry a ref or a subject ────────────────────────────────────────
# "Bare" = the line names a 7-40 hex token but gives no `origin/…` ref and no backtick-quoted subject
# of >=8 chars, so a future reader cannot recover what the commit was if it is ever orphaned.
g3=0
while IFS= read -r hit; do
  [[ -z "$hit" ]] && continue
  file="${hit%%:*}"; rest="${hit#*:}"; line="${rest%%:*}"; text="${rest#*:}"
  grep -qE 'origin/|`[^`]{8,}`' <<<"$text" && continue
  key="G3:$file:$(grep -ohE '\b[0-9a-f]{7,40}\b' <<<"$text" | head -1)"
  echo "$key" >>"$new_baseline"
  if ! known "$key"; then
    echo "[doc-guards] G3 FAIL: $file:$line pins a bare SHA with no ref or subject" >&2
    g3=$((g3 + 1))
  fi
done < <(grep -nE '\b[0-9a-f]{7,40}\b' docs/plans/*.md 2>/dev/null || true)

# ── G4 — diagnostic codes named in the register must exist in `src/` ─────────────────────────────
# Prefix fragments (`E-TRANSPILE-`) and bare stems (`E-TRANSPILE`) are extraction artifacts, not
# claims, so they are dropped rather than baselined — otherwise the baseline teaches noise.
g4=0
while read -r code; do
  [[ -z "$code" ]] && continue
  [[ "$code" == *- ]] && continue
  [[ "$code" =~ ^[EW]-[A-Z]+$ ]] && continue
  grep -rqF "\"$code\"" src/ 2>/dev/null && continue
  echo "G4:$code" >>"$new_baseline"
  if ! known "G4:$code"; then
    echo "[doc-guards] G4 FAIL: $REGISTER names $code but no such code exists in src/" >&2
    g4=$((g4 + 1))
  fi
done < <(grep -ohE '\b[EW]-[A-Z][A-Z0-9-]{2,}' "$REGISTER" | sort -u)

if [[ "$WRITE_BASELINE" == "1" ]]; then
  sort -u "$new_baseline" >"$BASELINE"
  echo "[doc-guards] baseline REWRITTEN: $(wc -l <"$BASELINE") entry(ies) in $BASELINE"
  exit 0
fi

fails=$((g1 + g2 + g3 + g4))
if ((fails)); then
  echo "[doc-guards] FAILED — G1=$g1 G2=$g2 G3=$g3 G4=$g4 new violation(s)." >&2
  echo "  A path/DEC/SHA/code that does not exist is doc rot: fix the reference, or implement the" >&2
  echo "  thing it names. Deliberately adding to the frozen set needs DOC_GUARDS_WRITE_BASELINE=1" >&2
  echo "  and a reason in the commit message." >&2
  exit 1
fi
echo "[doc-guards] OK (G1/G2/G3/G4; baseline: $(wc -l <"$BASELINE" 2>/dev/null || echo 0) frozen)"
