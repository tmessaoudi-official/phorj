#!/usr/bin/env bash
# owed-legs.sh - the durable list of gate legs that could not run where they were (DEC-597, DEC-598).
#
# DEC-365 ruled NO-HIDDEN-LOSS: an unmeasurable leg is recorded until a valid measurement clears it. Until 2026-11-23
# that record was only a line in a push log (51aad5f8 went out with the microbench skipped at load 6.36, and nothing
# kept it). The list lives in docs/plans/SLICE-STATE.md (Invariant 19: the one cursor file), between two markers:
#
#   - OPEN <leg> since <sha> `<subject>` (<date>, <env>): <reason>
#   - CLEARED <leg> since <sha> `<subject>` (<date>, <env>) by a PASS at <sha> (<date>, <evidence>)
#
# One OPEN entry per leg: the oldest unverified commit is kept, so a repeated skip changes nothing. Entries match on
# the LEG, never the sha (a cloud branch's shas rotate when the developer re-signs them onto master). A gate clears an
# entry only after a real pass; `check-staged` (run by pre-commit) refuses a commit that deletes an OPEN entry, or that
# leaves a new one in the working tree uncommitted. It cannot tell a CLEARED line typed by hand from a gate's.
#
#   owed-legs.sh record <leg> <env> <reason>   add the OPEN entry unless the leg already has one
#   owed-legs.sh clear <leg> <evidence>        OPEN -> CLEARED (no-op when the leg is not open)
#   owed-legs.sh list                          print the OPEN entries (silent when none)
#   owed-legs.sh has-committed <leg>           exit 0 when HEAD's file already holds an OPEN entry for <leg>
#   owed-legs.sh check-staged                  the pre-commit check described above
# PHORJ_OWED_FILE relocates the file (tests); the git subcommands always use the tracked path.
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
REL=docs/plans/SLICE-STATE.md
FILE="${PHORJ_OWED_FILE:-$root/$REL}"
BEGIN='<!-- owed-legs -->'
END='<!-- /owed-legs -->'
HEADING='## OWED GATE LEGS (DEC-597) — legs that could not run where they were; the FIRST work of a PC session that can run them (DEC-598)'

clean() { printf '%s' "$1" | tr -d '`|\r' | tr '\n\t' '  ' | cut -c1-"$2"; }
leg_ok() { [[ "$1" =~ ^[a-z0-9][a-z0-9-]*$ ]] || { echo "owed-legs: bad leg name '$1'" >&2; exit 2; }; }
open_line() { grep -m1 -E "^- OPEN $1 since " "$FILE" 2>/dev/null || true; }

# Create the block (before the defect queue, else at the end) when the file has none.
ensure_block() {
  grep -qxF "$BEGIN" "$FILE" && return 0
  local tmp; tmp="$(mktemp)"
  awk -v h="$HEADING" -v b="$BEGIN" -v e="$END" '
    !done && /^## DEFECT QUEUE/ { print h; print ""; print b; print e; print ""; done=1 }
    { print }
    END { if (!done) { print ""; print h; print ""; print b; print e } }' "$FILE" >"$tmp"
  cat "$tmp" >"$FILE"; rm -f -- "$tmp"
}

cmd="${1:-}"; shift || true
case "$cmd" in
  record)
    [[ $# -eq 3 ]] || { echo "usage: owed-legs.sh record <leg> <env> <reason>" >&2; exit 2; }
    leg_ok "$1"
    if [[ -n "$(open_line "$1")" ]]; then echo "owed-legs: $1 already OWED (kept: the oldest unverified commit)"; exit 0; fi
    ensure_block
    sha="$(git rev-parse --short=8 HEAD)"; subj="$(clean "$(git log -1 --format=%s HEAD)" 70)"
    (( ${#subj} >= 8 )) || subj="$subj (commit)"
    line="- OPEN $1 since $sha \`$subj\` ($(date -u +%F), $(clean "$2" 20)): $(clean "$3" 200)"
    tmp="$(mktemp)"
    awk -v e="$END" -v l="$line" '$0 == e { print l } { print }' "$FILE" >"$tmp"
    cat "$tmp" >"$FILE"; rm -f -- "$tmp"
    echo "owed-legs: OWED $1 recorded in $REL — commit it (DEC-597): $line"
    ;;
  clear)
    [[ $# -eq 2 ]] || { echo "usage: owed-legs.sh clear <leg> <evidence>" >&2; exit 2; }
    leg_ok "$1"
    old="$(open_line "$1")"; [[ -n "$old" ]] || exit 0
    new="$(sed -E 's/^- OPEN /- CLEARED /; s/ \(([0-9]{4}-[0-9]{2}-[0-9]{2}, [^)]*)\): .*$/ (\1)/' <<<"$old")"
    new="$new by a PASS at $(git rev-parse --short=8 HEAD) ($(date -u +%F), $(clean "$2" 60))"
    tmp="$(mktemp)"
    awk -v o="$old" -v n="$new" '$0 == o && !d { print n; d=1; next } { print }' "$FILE" >"$tmp"
    cat "$tmp" >"$FILE"; rm -f -- "$tmp"
    echo "owed-legs: $1 CLEARED in $REL — commit it: $new"
    ;;
  list)
    grep -E '^- OPEN ' "$FILE" 2>/dev/null | sed 's/^/OWED (DEC-597): /' || true
    ;;
  has-committed)
    leg_ok "${1:-}"
    git -C "$root" show "HEAD:$REL" 2>/dev/null | grep -qE "^- OPEN $1 since "
    ;;
  check-staged)
    head_f="$(git -C "$root" show "HEAD:$REL" 2>/dev/null || true)"
    idx_f="$(git -C "$root" show ":$REL" 2>/dev/null || true)"
    bad=0
    while IFS= read -r l; do
      [[ -n "$l" ]] || continue
      key="$(sed -E 's/^- OPEN ([a-z0-9-]+ since [0-9a-f]+) .*/\1/' <<<"$l")"
      grep -qE "^- (OPEN|CLEARED) ${key}( |$)" <<<"$idx_f" && continue
      echo "[pre-commit] REFUSED (DEC-597): an OPEN owed entry is deleted, and only a gate's pass may clear it: $l" >&2; bad=1
    done < <(grep -E '^- OPEN ' <<<"$head_f" || true)
    while IFS= read -r l; do
      [[ -n "$l" ]] || continue
      grep -qxF -e "$l" <<<"$idx_f" && continue
      echo "[pre-commit] REFUSED (DEC-597): $REL holds an OWED entry this commit leaves out — stage that file: $l" >&2; bad=1
    done < <(grep -E '^- (OPEN|CLEARED) ' "$root/$REL" 2>/dev/null || true)
    exit "$bad"
    ;;
  *) echo "usage: owed-legs.sh record|clear|list|has-committed|check-staged …" >&2; exit 2 ;;
esac
