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
#   * a malformed baseline row: no TAB, empty path, a TAB in the path,
#     a count that is not a plain decimal (≤ 9 digits), a duplicate path,
#     or a NUL byte anywhere in the file                                 -> FAIL (malformed row)
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
# A row is DATA: `(( lines > cap ))` evaluates array subscripts, so an unvalidated count `PWD[$(cmd)]`
# ran cmd at every push, and `abc` died on set -u (panel 2026-09-28, safety O1; lint test section 4d).
# A malformed row FAILS the gate — the ratchet's own data being wrong is not something to skip past.
# is_count: ASCII decimal digits only, checked against an explicit LIST — a `[0-9]` RANGE is
# locale-dependent (under en_US.UTF-8 it matched ٥, ² and ~620 other codepoints, which then made
# `(( ))` error inside an `if` = a false condition = the gate PASSED). The value is then forced to base
# 10: a leading 0 is octal to bash (0600 → 384; 08 → error → pass). Panel 2026-09-28 round 2
# (safety R2-1/R2-2, correctness N1, completeness N4); tests: scripts/test-size-gate.sh.
# ≤ 9 digits: a longer count wraps in bash's intmax (2^64+100000 reads as 100000), which failed OPEN.
is_count() { case "$1" in '' | *[!0123456789]*) return 1 ;; esac; ((${#1} <= 9)); }
malformed=0
# Readable first: the NUL probe below reads the file through a redirect, and a failed redirect would
# otherwise be reported as "contains a NUL byte" — a cause nobody would find (panel round 5).
if [[ -f "$BASELINE" && ! -r "$BASELINE" ]]; then
  printf 'FAIL (cannot read %s — check its permissions)\n' "$BASELINE"
  exit 1
fi
if [[ -f "$BASELINE" ]]; then
  # bash `read` DROPS a NUL byte, so `9<NUL>00` would load as 900 — a non-digit byte past is_count — and
  # a NUL turns `git diff` of this file into "Binary files differ", hiding which ceiling moved (panel
  # 2026-09-28 round 4, safety N2). A baseline holding a NUL anywhere is malformed.
  if ! tr -d '\000' < "$BASELINE" | cmp -s - "$BASELINE"; then
    printf 'FAIL (malformed %s: it contains a NUL byte)\n' "$BASELINE"
    malformed=$((malformed + 1))
  fi
  while IFS= read -r row || [[ -n "$row" ]]; do
    [[ -n "$row" ]] || continue
    cnt="${row%%$'\t'*}"
    path="${row#*$'\t'}"
    # Malformed: no TAB, an empty path, a path holding a TAB (a 3rd field — the hook's split would read
    # it differently), a count that is not a plain decimal, or a path already seen. A duplicate is how an
    # appended row could silently raise a reviewed ceiling (this loop is last-row-wins).
    if [[ "$row" != *$'\t'* || -z "$path" || "$path" == *$'\t'* ]] || ! is_count "$cnt" \
       || [[ -n "${ceiling[$path]+set}" ]]; then
      # %q: the row is attacker-controlled free text — never echo it raw (escape sequences, newlines).
      printf 'FAIL (malformed %s row, need <count><TAB><path>, a plain decimal count, one row per path): %q\n' "$BASELINE" "$row"
      malformed=$((malformed + 1))
      continue
    fi
    ceiling["$path"]=$((10#$cnt))
  done < "$BASELINE"
fi

fails=$malformed
warns=0
stale=0

while IFS= read -r -d '' f; do
  lines=$(wc -l < "$f")
  rel="${f#./}"
  if [[ -n "${ceiling[$rel]:-}" ]]; then
    cap="${ceiling[$rel]}"
    if (( lines > cap )); then
      # %q on every path: a committed FILENAME is free text too (an escape sequence would reach the
      # terminal and, when a session pushes, the model's context).
      printf 'FAIL (grandfathered file grew): %q = %s > baseline %s — split it, do not grow it\n' "$rel" "$lines" "$cap"
      fails=$((fails+1))
    elif (( lines <= HARD )); then
      printf 'note (grandfathered file now under hard cap — drop from %s): %q = %s\n' "$BASELINE" "$rel" "$lines"
      stale=$((stale+1))
    fi
  else
    if (( lines > HARD )); then
      printf 'FAIL (new hard-cap breach >%s): %q = %s — split by cohesion (M-Decomp)\n' "$HARD" "$rel" "$lines"
      fails=$((fails+1))
    elif (( lines > SOFT )); then
      printf 'warn (soft cap >%s): %q = %s\n' "$SOFT" "$rel" "$lines"
      warns=$((warns+1))
    fi
  fi
done < <(find src -name '*.rs' -print0)

echo "[size-gate] grandfathered=${#ceiling[@]} fails=$fails warns=$warns stale=$stale"
if (( fails > 0 )); then
  echo "[size-gate] FAILED — $fails failure(s): file-size breaches of Invariant 13 (300 soft / 500 hard) and/or malformed $BASELINE rows."
  exit 1
fi
echo "[size-gate] OK (no new or growing hard-cap breach)"
