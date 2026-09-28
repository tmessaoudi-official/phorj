#!/usr/bin/env bash
# PostToolUse(Edit|Write) advisory — phorj write-time lint.
#
# WARN-ONLY, ALWAYS exit 0. This is not negotiable and it is not laziness: project CLAUDE.md
# § "Claude config in this repo" records the developer's 2026-08-06 ruling that there are NO
# permission denies in this environment, because a web session has no terminal to recover in — and a
# PostToolUse hook that blocks a write is a deny by another name. So this script reports and gets out
# of the way. The tiered git hooks (`scripts/git-hooks/{pre-commit,pre-push}`) remain the enforcement
# tier; this one only shortens the feedback loop from "at commit" to "at write".
#
# Three checks, dispatched by extension:
#   *.rs   -> rustfmt --check     (the fmt tier of the gate, one file early)
#   *.phg  -> phg format --check  (the `phg format --check examples selftest` tier, one file early)
#   any    -> Invariant 13 size advisory (soft 300 / hard 500, and grandfathered-must-not-GROW)
#
# The size advisory is the one with real teeth, and it is here because of a specific failure: on
# 2026-08-06 a file was pushed back under its baseline three times by SHAVING COMMENTS before the
# author did the right thing and split it. `scripts/size-gate.sh` catches the breach at push time,
# which is already too late to influence the design — by then the feature is written and the cheap
# move is to shave. Told at write time, the cheap move is to split, which is what Invariant 13 asks
# for ("split-as-you-go is the DEFAULT: a feature that would push a file past the soft cap STARTS by
# splitting it").
#
# Guard: `test-lint-on-write.sh` beside this file (its count is in its last line) — run by
# scripts/git-hooks/pre-push; run it yourself after any edit here.
#
# `set -uo pipefail` deliberately omits `-e`, but note that adding `-e` would be behaviour-NEUTRAL
# today, not a fix and not a break: every fallible command below is already explicitly guarded
# (`|| true`, `2>/dev/null || echo`, or sitting inside an `if !`, which suspends errexit), and every
# `(( … ))` is a condition rather than a statement. Verified by sabotage on 2026-08-06 — flipping to
# `-euo` left all 18 assertions green. The guards are the safety, not the flag; do not remove one on
# the theory that the other covers it.
set -uo pipefail

SOFT=300
HARD=500

root="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
# log_obs comes from the GLOBAL install (global-is-reference ruling, 2026-08-18 — no repo copy);
# absent it, the no-op stub keeps the hook alive on machines without ~/.claude (e.g. CI).
# shellcheck source=/dev/null
. "$HOME/.claude/hooks/log-helpers.sh" 2>/dev/null || log_obs() { :; }

payload="$(cat 2>/dev/null || true)"
file="$(printf '%s' "$payload" | jq -r '.tool_input.file_path // empty' 2>/dev/null || true)"

# Nothing to say about a non-file tool call, an unreadable payload, or a file already gone.
[[ -n "$file" && -f "$file" ]] || exit 0

# A warning must reach the MODEL, not only the transcript. With exit 0 a PostToolUse hook's stderr
# never reaches it; the only channel that does is stdout JSON hookSpecificOutput.additionalContext
# (measured 2026-09-28 with random markers, 5 variants x 2 models — ~/.claude review-remediation
# row 33). So every warn() also collects its line, and ONE JSON object is printed on exit (the harness
# parses stdout only when it is a single JSON document). Still warn-only: the exit code stays 0.
_ctx=""
warn() { printf 'lint-on-write: %s\n' "$1" >&2; _ctx+="lint-on-write: $1"$'\n'; }
_emit_ctx() {
  [[ -n "$_ctx" ]] && jq -n --arg m "${_ctx%$'\n'}" \
    '{hookSpecificOutput: {hookEventName: "PostToolUse", additionalContext: $m}}'
  # `return 0`: with no warnings the `[[ -n ]] &&` above is false; the trap's status never overrides the
  # script's own `exit 0` (measured 2026-09-28: `f(){ false; }; trap f EXIT; exit 0` → rc 0), so this only
  # keeps the function's status honest — it masks no failure (panel 2026-09-28, safety F3).
  return 0
}
trap _emit_ctx EXIT

case "$file" in
  *.rs)
    if command -v rustfmt >/dev/null 2>&1; then
      if ! out="$(rustfmt --edition 2021 --check "$file" 2>&1)"; then
        [[ -n "$out" ]] && warn "rustfmt would reformat ${file#"$root"/} — run \`cargo fmt\` before committing"
        log_obs INFO lint-on-write "rustfmt --check dirty: $file"
      fi
    fi
    ;;
  *.phg)
    phg=""
    for cand in "$root/target/release/phg" "$root/target/debug/phg"; do
      [[ -x "$cand" ]] && { phg="$cand"; break; }
    done
    if [[ -n "$phg" ]]; then
      if ! "$phg" format --check "$file" >/dev/null 2>&1; then
        warn "phg format --check fails on ${file#"$root"/} — run \`phg format\` before committing"
        log_obs INFO lint-on-write "phg format --check dirty: $file"
      fi
    fi
    ;;
esac

# ── Invariant 13 size advisory ─────────────────────────────────────────────────────────────────
# Scope must MATCH THE AUTHORITY EXACTLY. `scripts/size-gate.sh` scans `find src -name '*.rs'` and
# nothing else, so this advisory covers `src/**.rs` and nothing else.
#
# The first draft also covered `tests/*.rs` and `playground/src/*.rs` and told the author their file
# "FAILS scripts/size-gate.sh at push". That was simply false — the gate never looks there — and it was
# false on SEVEN real files, including `tests/differential.rs` (4945 lines), which Invariants 1 and 9
# make the single most-edited test file in the repo: every feature commit would have been greeted by a
# fabricated push-failure warning on the one file it is required to touch. A channel that cries wolf on
# every commit is a channel the author learns to ignore, which is precisely the failure this hook's
# header argues against. Found by all three DEC-268 lenses on the first run of the panel.
case "$file" in
  "$root"/src/*.rs) ;;
  *) exit 0 ;;
esac

rel="${file#"$root"/}"
lines="$(wc -l < "$file" 2>/dev/null || echo 0)"
# The row is DATA from a tracked file: `(( lines > baseline ))` evaluates array subscripts, so a row
# `PWD[$(cmd)]` ran cmd on the next Edit, and `abc` broke "ALWAYS exit 0" under set -u (panel
# 2026-09-28, safety O1/O2 — reproduced; test section 4d). Validate before any arithmetic, and do not
# echo the row's text into the model's context.
# Rows are read EXACTLY as size-gate.sh reads them (count = text before the FIRST tab, path = all
# after it), so the two consumers never disagree on what a row means (round 3, R3-3/R3-8): a row
# for this file is malformed when its count is empty or not a plain decimal, when the path appears in
# more than one row (the gate FAILs a duplicate), or when a 3rd field follows the path.
# is_count — the same check as size-gate.sh's: ASCII digits from an explicit LIST (a `[0-9]` range
# matched ٣ under en_US.UTF-8), at most 9 of them (a longer count wraps in bash's intmax: 2^64+100000
# reads as 100000); the value is then forced to base 10 (a leading 0 is octal: 0600 → 384).
is_count() { case "$1" in '' | *[!0123456789]*) return 1 ;; esac; ((${#1} <= 9)); }
# The path goes to awk through ENVIRON, not `-v` (which turns `\t` in a filename into a TAB and misses
# the row); `-r` as well as `-f`: an unreadable baseline is left alone (no false "malformed", no awk
# noise), as a missing one is. A NUL byte anywhere, or a row with no TAB, makes size-gate FAIL the
# push whatever file it names, so both are reported here too (panel 2026-09-28 round 4).
B="$root/scripts/size-baseline.txt"
rows=0 odd=0 notab=0 baseline=""
if [[ -f "$B" && -r "$B" ]]; then
  if ! tr -d '\000' < "$B" | cmp -s - "$B"; then
    warn "scripts/size-baseline.txt contains a NUL byte (malformed) — size-gate FAILs the push until it is removed; the size check is skipped"
    log_obs INFO lint-on-write "baseline holds a NUL byte"
    exit 0
  fi
  IFS=$'\t' read -r rows odd notab baseline < <(F="$rel" awk '
    { i = index($0, "\t"); if (!i) { if ($0 != "") t++; next }; p = substr($0, i + 1)
      if (p == "") { e++; next }
      if (p == ENVIRON["F"]) { n++; c = substr($0, 1, i - 1) } else if (index(p, ENVIRON["F"] "\t") == 1) b++ }
    END { printf "%d\t%d\t%d\t%s\n", n, b, t + e, c }' "$B")
fi
if [[ "$notab" != 0 ]]; then
  warn "scripts/size-baseline.txt has $notab row(s) with no TAB or an empty path (malformed) — size-gate FAILs the push until they read <count><TAB><path>"
  log_obs INFO lint-on-write "baseline rows without a TAB: $notab"
fi
if [[ "$rows" != 0 || "$odd" != 0 ]] && { [[ "$rows" != 1 || "$odd" != 0 ]] || ! is_count "$baseline"; }; then
  warn "scripts/size-baseline.txt has a malformed row for $rel (need ONE row <count><TAB><path> with a plain decimal count) — fix the row; the size check is skipped for this file"
  log_obs INFO lint-on-write "malformed baseline row: $rel"
  exit 0
fi
if [[ "$rows" == 1 ]]; then baseline=$((10#$baseline)); else baseline=""; fi

if [[ -n "$baseline" ]]; then
  # Grandfathered: the rule is it must not GROW. Shrinking below 500 means dropping the row.
  if (( lines > baseline )); then
    warn "$rel is grandfathered at $baseline lines and is now $lines — Invariant 13 says split it, do not grow it (and do not shave comments to squeeze back under)"
    log_obs INFO lint-on-write "grandfathered growth: $rel $baseline -> $lines"
  elif (( lines <= HARD )); then
    # `<=`, not `<`: size-gate.sh's `elif (( lines <= HARD ))` branch uses `lines <= HARD` and raises `stale=1` at EXACTLY 500, asking
    # for the row to be dropped. The first draft used `<` and went silent on precisely that boundary —
    # the one line where the push-time gate wants action.
    warn "$rel is now $lines lines, at or under the $HARD hard cap — drop its row from scripts/size-baseline.txt so the ratchet tightens"
  fi
elif (( lines > HARD )); then
  warn "$rel is $lines lines, over the $HARD HARD cap — this FAILS scripts/size-gate.sh at push; split by cohesion into a mod.rs + sub-files"
  log_obs INFO lint-on-write "hard-cap breach: $rel $lines"
elif (( lines > SOFT )); then
  warn "$rel is $lines lines, over the $SOFT soft cap — Invariant 13 wants the split to START now, not once it reaches $HARD"
fi

exit 0
