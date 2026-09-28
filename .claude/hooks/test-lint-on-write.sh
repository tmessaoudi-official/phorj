#!/usr/bin/env bash
# Guard for lint-on-write.sh — the PostToolUse(Edit|Write) advisory.
#
# The contract: it WARNS and it ALWAYS exits 0. Blocking a write would be a permission deny by
# another name, which project CLAUDE.md § "Claude config in this repo" forbids outright (developer
# ruling 2026-08-06 — a web session has no terminal to recover in). So "exit 0" is asserted on every
# path here, including the paths where the hook has something to complain about, and including the
# malformed-input paths where a `set -e` script would abort.
#
# Run: bash .claude/hooks/test-lint-on-write.sh
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$HERE/lint-on-write.sh"
PASS=0; FAIL=0
ok()  { printf '  ok   — %s\n' "$1"; PASS=$((PASS+1)); }
bad() { printf '  FAIL — %s\n' "$1"; FAIL=$((FAIL+1)); }

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/src" "$TMP/scripts" "$TMP/tests" "$TMP/.claude/hooks"
: > "$TMP/scripts/size-baseline.txt"
# The hook sources the GLOBAL ~/.claude/hooks/log-helpers.sh (global-is-reference ruling,
# 2026-08-18 — the repo copy is gone). This test runs on the developer's machine where the global
# copy exists, so the observability assertion below exercises the REAL log_obs (honouring the
# OBS_LOG override). On a machine without ~/.claude the hook's no-op fallback fires, and that ONE
# assertion is reported as SKIP (counted apart, never as a pass) — the other 40-odd need nothing global,
# which is why scripts/git-hooks/pre-push can run this suite (panel 2026-09-28 round 3, R3-2).
HAVE_LOG_HELPERS=0; [[ -f "$HOME/.claude/hooks/log-helpers.sh" ]] && HAVE_LOG_HELPERS=1
SKIP=0

# Run the hook with a sandbox project root, feeding a PostToolUse-shaped payload on stdin.
# Returns "rc|stderr" so a single call can assert both halves of the contract.
run() {
  local path="$1" out rc
  out="$(printf '{"tool_input":{"file_path":"%s"}}' "$path" \
        | CLAUDE_PROJECT_DIR="$TMP" OBS_LOG="$TMP/obs.log" bash "$SCRIPT" 2>&1 >/dev/null)"; rc=$?
  printf '%s|%s' "$rc" "$out"
}
rc_of()  { printf '%s' "${1%%|*}"; }
err_of() { printf '%s' "${1#*|}"; }

# Section 4b calls this; it was never defined (panel 2026-09-28, correctness F2 / completeness F6 —
# 'command not found' on every run since origin). Clears the grandfather baseline between sections.
reset_sandbox() { : > "$TMP/scripts/size-baseline.txt"; }

gen() { python3 -c "import sys; n=int(sys.argv[2]); open(sys.argv[1],'w').write(''.join('// line %d\n'%i for i in range(n)))" "$1" "$2"; }

echo "lint-on-write.sh — warn-only advisory contract"

# ── 1. ALWAYS exit 0, including on the paths that have something to say ─────────────
gen "$TMP/src/huge.rs" 700
r="$(run "$TMP/src/huge.rs")"
[[ "$(rc_of "$r")" == 0 ]] && ok "exit 0 even when it has a hard-cap complaint" \
                           || bad "exit $(rc_of "$r") on a hard-cap breach — a blocking hook is a deny"
# Converse: it must actually have complained, or the assertion above is vacuous.
[[ "$(err_of "$r")" == *"over the 500 HARD cap"* ]] && ok "reports the hard-cap breach on stderr" \
                                                    || bad "silent on a 700-line file: '$(err_of "$r")'"

# ── 2. Soft cap is advisory and distinct from the hard cap ─────────────────────────
gen "$TMP/src/mid.rs" 350
r="$(run "$TMP/src/mid.rs")"
[[ "$(err_of "$r")" == *"over the 300 soft cap"* ]] && ok "reports the soft cap at 350 lines" \
                                                    || bad "no soft-cap warning at 350: '$(err_of "$r")'"
[[ "$(err_of "$r")" == *"HARD cap"* ]] && bad "called a 350-line file a hard-cap breach" \
                                       || ok "does not confuse the soft cap with the hard cap"

# ── 3. A small file is SILENT — a hook that warns always is a hook that is ignored ─
gen "$TMP/src/small.rs" 42
r="$(run "$TMP/src/small.rs")"
[[ -z "$(err_of "$r")" ]] && ok "silent on a 42-line file" \
                          || bad "noise on a small file: '$(err_of "$r")'"
sout_of() { printf '{"tool_input":{"file_path":"%s"}}' "$1" \
            | CLAUDE_PROJECT_DIR="$TMP" OBS_LOG="$TMP/obs.log" bash "$SCRIPT" 2>/dev/null; }
[[ -z "$(sout_of "$TMP/src/small.rs")" ]] && ok "silent on stdout too for a 42-line file" \
                                        || bad "stdout noise on a small file: '$(sout_of "$TMP/src/small.rs")'"

# ── 3b. A warning must reach the MODEL, not just the transcript ─────────────────────
# With exit 0 a PostToolUse hook's stderr never reaches the model; the only channel that does is
# stdout JSON hookSpecificOutput.additionalContext (measured 2026-09-28 with random markers, 5 variants
# x 2 models — ~/.claude review-remediation row 33). A warn-only hook the model cannot see shortens no
# feedback loop at all, which is the whole argument of the hook's header.
sout="$(sout_of "$TMP/src/huge.rs")"
ctx="$(printf '%s' "$sout" | jq -r '.hookSpecificOutput.additionalContext // empty' 2>/dev/null)"
[[ "$ctx" == *"HARD cap"* ]] && ok "the hard-cap warning reaches the model (additionalContext)" \
                             || bad "no additionalContext for the hard-cap breach; stdout='${sout:0:80}'"
[[ "$(printf '%s' "$sout" | jq -r '.hookSpecificOutput.hookEventName // empty' 2>/dev/null)" == PostToolUse ]] \
  && ok "hookEventName is PostToolUse" || bad "hookEventName missing from stdout JSON"

# ── 3c. TWO warnings at once → still ONE JSON document carrying both (panel 2026-09-28) ─
# The harness parses stdout only when it is a single JSON document; the accumulate-then-emit-once
# trap is what guarantees that. Every other case fires ONE warn(), so a "simplification" printing a
# JSON line per warn() passed 25/25 (completeness F1, correctness F1) — and with rustfmt present the
# rustfmt leg had no guard of its own (safety F2). An unformatted file over the hard cap fires both.
{ printf 'fn   main( ){ }\n'; python3 -c "print(''.join('// line %d\n'%i for i in range(700)),end='')"; } > "$TMP/src/two.rs"
# A stub rustfmt that always reports a diff makes the rustfmt leg fire on EVERY machine: with the
# real one absent, only one warn() fired and a JSON-per-warn mutant stayed green (round 2, N8/R2-8).
mkdir -p "$TMP/stubbin"
printf '#!/bin/sh\necho "Diff in $4"\nexit 1\n' > "$TMP/stubbin/rustfmt"; chmod +x "$TMP/stubbin/rustfmt"
sout2="$(PATH="$TMP/stubbin:$PATH" sout_of "$TMP/src/two.rs")"
[[ "$(printf '%s' "$sout2" | jq -s 'length' 2>/dev/null)" == 1 ]] && ok "two warnings → exactly ONE JSON document on stdout" \
                                                                 || bad "stdout is not one JSON document: '${sout2:0:120}'"
ctx2="$(printf '%s' "$sout2" | jq -r '.hookSpecificOutput.additionalContext // empty' 2>/dev/null)"
[[ "$ctx2" == *"HARD cap"* ]] && ok "the hard-cap warning is in that one document" || bad "hard-cap line missing: '${ctx2:0:120}'"
[[ "$ctx2" == *"rustfmt"* ]] && ok "the rustfmt warning is in that one document too" \
                             || bad "rustfmt warning missing from additionalContext: '${ctx2:0:120}'"
rm -rf "$TMP/src/two.rs" "$TMP/stubbin"

# ── 4. THE REGRESSION THIS HOOK EXISTS FOR: grandfathered growth ───────────────────
# scripts/size-gate.sh catches this at push. By then the cheap fix is to shave comments,
# which happened three times on 2026-08-06. Told at write time, the cheap fix is to split.
printf '600\tsrc/grand.rs\n' > "$TMP/scripts/size-baseline.txt"
gen "$TMP/src/grand.rs" 640
r="$(run "$TMP/src/grand.rs")"
[[ "$(rc_of "$r")" == 0 ]] || bad "exit $(rc_of "$r") on grandfathered growth"
if [[ "$(err_of "$r")" == *"grandfathered at 600"*"now 640"* ]]; then
  ok "reports growth of a grandfathered file (600 -> 640)"
else
  bad "missed grandfathered growth: '$(err_of "$r")'"
fi
# Converse: a grandfathered file that did NOT grow must not be reported as growing.
gen "$TMP/src/grand.rs" 590
r="$(run "$TMP/src/grand.rs")"
[[ "$(err_of "$r")" == *"do not grow it"* ]] && bad "reported growth for a file that shrank" \
                                             || ok "no growth warning when a grandfathered file shrinks"
# ...and once it is under the hard cap, it should say to drop the baseline row (ratchet tightening).
gen "$TMP/src/grand.rs" 480
r="$(run "$TMP/src/grand.rs")"
[[ "$(err_of "$r")" == *"drop its row"* ]] && ok "asks for the baseline row to be dropped below 500" \
                                           || bad "no ratchet-tightening hint at 480: '$(err_of "$r")'"

# ── 4b. Scope matches the AUTHORITY exactly: src/ only ────────────────────────────
# scripts/size-gate.sh scans `find src -name '*.rs'`. The first draft of the hook also covered
# tests/ and playground/src/ and claimed those "FAIL size-gate.sh at push" — false on 7 real files,
# tests/differential.rs (4945 lines) among them. No assertion covered those arms, which is why it
# shipped. These two do.
reset_sandbox
mkdir -p "$TMP/tests" "$TMP/playground/src"
gen "$TMP/tests/big.rs" 700
r="$(run "$TMP/tests/big.rs")"
[[ -z "$(err_of "$r")" ]] && ok "silent on tests/*.rs (size-gate.sh does not scan it)" \
                          || bad "warned about tests/*.rs: '$(err_of "$r")'"
gen "$TMP/playground/src/big.rs" 700
r="$(run "$TMP/playground/src/big.rs")"
[[ -z "$(err_of "$r")" ]] && ok "silent on playground/src/*.rs (out of the gate's scope)" \
                          || bad "warned about playground/src/*.rs: '$(err_of "$r")'"

# ── 4c. The EXACTLY-500 boundary agrees with size-gate.sh ─────────────────────────
# size-gate.sh's stale check is `lines <= HARD`, so at exactly 500 a grandfathered file raises stale=1 and it
# asks for the baseline row to be dropped. The first draft used `<` and was silent there.
printf '600\tsrc/edge.rs\n' > "$TMP/scripts/size-baseline.txt"
gen "$TMP/src/edge.rs" 500
r="$(run "$TMP/src/edge.rs")"
[[ "$(err_of "$r")" == *"drop its row"* ]] && ok "asks to drop the baseline row at EXACTLY 500" \
                                          || bad "silent at exactly 500 — disagrees with size-gate.sh: '$(err_of "$r")'"
gen "$TMP/src/edge.rs" 501
r="$(run "$TMP/src/edge.rs")"
[[ "$(err_of "$r")" == *"do not grow it"* ]] && bad "called 501 growth against a 600 baseline" \
                                             || ok "501 under a 600 baseline is not reported as growth"
: > "$TMP/scripts/size-baseline.txt"

# ── 4d. A baseline row is DATA, never code (panel 2026-09-28: safety O1/O2) ───────
# `(( lines > baseline ))` evaluates array subscripts, so a tracked row `PWD[$(cmd)]` ran cmd on the
# next Edit — and again at push through size-gate.sh (reproduced by the safety lens). A non-numeric
# row also broke "ALWAYS exit 0" (set -u → rc 1). Both consumers must treat the field as a number.
printf 'PWD[$(touch${IFS}%s/pwned)]\tsrc/evil.rs\nabc\tsrc/abc.rs\n' "$TMP" > "$TMP/scripts/size-baseline.txt"
gen "$TMP/src/evil.rs" 10
r="$(run "$TMP/src/evil.rs")"
[[ ! -e "$TMP/pwned" ]] && ok "hook: a baseline row is never executed as code" \
                        || bad "hook: a baseline row EXECUTED a command"
rm -f "$TMP/pwned"
[[ "$(rc_of "$r")" == 0 ]] && ok "hook: exit 0 on a hostile baseline row" || bad "hook: exit $(rc_of "$r") on a hostile baseline row"
[[ "$(err_of "$r")" == *"malformed"* ]] && ok "hook: the hostile row is reported, not silently used" \
                                        || bad "hook: hostile row not reported: '$(err_of "$r")'"
gen "$TMP/src/abc.rs" 10
r="$(run "$TMP/src/abc.rs")"
[[ "$(rc_of "$r")" == 0 ]] && ok "hook: exit 0 on a non-numeric baseline row (was rc 1 under set -u)" \
                           || bad "hook: exit $(rc_of "$r") on a non-numeric baseline row"
# Round 2 (panel 2026-09-28): a DIGIT-prefixed hostile row (an anchor-less regex let it through), a
# non-ASCII digit (a `[0-9]` range matches ٥ under en_US.UTF-8), and a leading zero (octal to bash).
# size-gate.sh's own cases moved to scripts/test-size-gate.sh, which pre-push runs.
printf '1+PWD[$(touch${IFS}%s/pwned)]\tsrc/evil.rs\n' "$TMP" > "$TMP/scripts/size-baseline.txt"
r="$(run "$TMP/src/evil.rs")"
[[ ! -e "$TMP/pwned" ]] && ok "hook: a digit-prefixed hostile row is never executed" \
                        || bad "hook: a digit-prefixed row EXECUTED a command"
rm -f "$TMP/pwned"
[[ "$(err_of "$r")" == *"malformed"* ]] && ok "hook: the digit-prefixed row is reported malformed" \
                                        || bad "hook: digit-prefixed row not reported: '$(err_of "$r")'"
printf '٣\tsrc/evil.rs\n' > "$TMP/scripts/size-baseline.txt"
r="$(LC_ALL=en_US.UTF-8 run "$TMP/src/evil.rs")"
[[ "$(err_of "$r")" == *"malformed"* ]] && ok "hook: a non-ASCII digit is malformed under en_US.UTF-8" \
                                        || bad "hook: non-ASCII digit accepted: '$(err_of "$r")'"
printf '0600\tsrc/evil.rs\n' > "$TMP/scripts/size-baseline.txt"
gen "$TMP/src/evil.rs" 590
r="$(run "$TMP/src/evil.rs")"
[[ "$(err_of "$r")" == *"do not grow it"* ]] && bad "hook: 0600 read as octal (384) — reported growth at 590" \
                                             || ok "hook: 0600 means 600 (base 10) — no growth at 590"
# A count past bash's intmax WRAPS: 2^64+100000 read as 100000 hid a 700-line file. Capped at 9 digits.
printf '18446744073709651616\tsrc/evil.rs\n' > "$TMP/scripts/size-baseline.txt"
gen "$TMP/src/evil.rs" 700
r="$(run "$TMP/src/evil.rs")"
[[ "$(err_of "$r")" == *"malformed"* ]] && ok "hook: a 20-digit count is malformed, not wrapped" \
                                        || bad "hook: 20-digit count accepted (wrapped): '$(err_of "$r")'"
# Round 3 (panel 2026-09-28): a 9-char hostile row UNDER the length cap, so only the digit-list check keeps
# it out (a stub `x` on PATH marks execution); and the row SHAPES the gate FAILs must not be read another
# way here — an empty count, a duplicate path (gate: last row wins, this hook's old awk: first), a 3rd
# field (a TAB inside the path) — each is reported malformed, never honoured as a grandfather ceiling.
mkdir -p "$TMP/bin"; printf '#!/bin/sh\ntouch "%s/pwned"\n' "$TMP" > "$TMP/bin/x"; chmod +x "$TMP/bin/x"
printf '1+_[$(x)]\tsrc/evil.rs\n' > "$TMP/scripts/size-baseline.txt"
r="$(PATH="$TMP/bin:$PATH" run "$TMP/src/evil.rs")"
[[ ! -e "$TMP/pwned" ]] && ok "hook: a 9-char hostile row (under the cap) is never executed" \
                        || bad "hook: the 9-char row EXECUTED a command"
rm -f "$TMP/pwned"
[[ "$(err_of "$r")" == *"malformed"* ]] && ok "hook: the 9-char hostile row is reported malformed" \
                                        || bad "hook: 9-char row not reported: '$(err_of "$r")'"
for shape in '\tsrc/evil.rs' '600\tsrc/evil.rs\n9999\tsrc/evil.rs' '9999\tsrc/evil.rs\tx' '600\tsrc/evil.rs\n9999\tsrc/evil.rs\tx' ' 600\tsrc/evil.rs' '600 \tsrc/evil.rs' '9\000%s00\tsrc/evil.rs'; do
  # shellcheck disable=SC2059 # the shape IS the format, on purpose (\t, \n)
  printf "$shape\\n" > "$TMP/scripts/size-baseline.txt"
  r="$(run "$TMP/src/evil.rs")"
  [[ "$(rc_of "$r")" == 0 && "$(err_of "$r")" == *"malformed"* ]] && ok "hook: row shape '$shape' is reported malformed (exit 0)" \
                                                                  || bad "hook: row shape '$shape': rc=$(rc_of "$r") '$(err_of "$r")'"
done
# Round 4: a row with NO TAB names no file the hook can match, but the gate FAILs the push on ANY such
# row — so the hook says so, whatever file is being edited (a 400-line file: no size rule fires).
gen "$TMP/src/mid4.rs" 400
printf '600 src/other.rs\n' > "$TMP/scripts/size-baseline.txt"
r="$(run "$TMP/src/mid4.rs")"
[[ "$(rc_of "$r")" == 0 && "$(err_of "$r")" == *"no TAB"* ]] && ok "hook: a tab-less row anywhere is reported (the gate will FAIL)" \
                                                            || bad "hook: tab-less row not reported: '$(err_of "$r")'"
# No baseline file (all debt paid), and an unreadable one: silence about rows, exit 0 — never a false
# "malformed" (round 4, correctness N-R4-1).
rm -f "$TMP/scripts/size-baseline.txt"
r="$(run "$TMP/src/mid4.rs")"
[[ "$(rc_of "$r")" == 0 && "$(err_of "$r")" != *"malformed"* && "$(err_of "$r")" != *"awk"* ]] && ok "hook: no baseline file → no row complaint" \
                                                                                              || bad "hook: missing baseline: '$(err_of "$r")'"
: > "$TMP/scripts/size-baseline.txt"; chmod 000 "$TMP/scripts/size-baseline.txt"
r="$(run "$TMP/src/mid4.rs")"
chmod 644 "$TMP/scripts/size-baseline.txt"
[[ "$(rc_of "$r")" == 0 && "$(err_of "$r")" != *"malformed"* && "$(err_of "$r")" != *"awk"* ]] && ok "hook: an unreadable baseline → no false 'malformed'" \
                                                                                              || bad "hook: unreadable baseline: '$(err_of "$r")'"
rm -f "$TMP/src/mid4.rs"
# A path holding a BACKSLASH is matched byte for byte, as the gate does: `awk -v` would turn `\t` into a
# TAB and miss the row (round 4, correctness N-R4-4 / safety N2). Grandfathered at 600, 550 lines: silent.
bs="src/a\\tb.rs"; gen "$TMP/$bs" 550
printf '600\t%s\n' "$bs" > "$TMP/scripts/size-baseline.txt"
r="$(jq -nc --arg p "$TMP/$bs" '{tool_input:{file_path:$p}}' | CLAUDE_PROJECT_DIR="$TMP" OBS_LOG="$TMP/obs.log" bash "$SCRIPT" 2>&1 >/dev/null)"
[[ "$r" != *"HARD cap"* && "$r" != *"malformed"* ]] && ok "hook: a backslash path matches its row (grandfathered, silent)" \
                                                  || bad "hook: backslash path missed its row: '$r'"
rm -f "$TMP/$bs"; reset_sandbox
# A space-separated row is NOT a grandfather row (the gate FAILs it): a 700-line file still hears the hard cap.
printf '9999 src/evil.rs\n' > "$TMP/scripts/size-baseline.txt"
r="$(run "$TMP/src/evil.rs")"
[[ "$(err_of "$r")" == *"HARD cap"* ]] && ok "hook: a space-separated row is not honoured (hard-cap warning)" \
                                       || bad "hook: space-separated row honoured: '$(err_of "$r")'"
rm -f "$TMP/src/evil.rs" "$TMP/src/abc.rs"; reset_sandbox

# ── 5. Scope: files the size gate does not govern produce no size noise ────────────
mkdir -p "$TMP/docs"; gen "$TMP/docs/big.md" 900
r="$(run "$TMP/docs/big.md")"
[[ -z "$(err_of "$r")" ]] && ok "no size advisory for a 900-line doc (out of the gate's scope)" \
                          || bad "size-warned an out-of-scope file: '$(err_of "$r")'"

# ── 6. Malformed / hostile input is non-fatal ─────────────────────────────────────
for label in "empty stdin" "not json" "no file_path" "nonexistent path"; do
  case "$label" in
    "empty stdin")      payload='' ;;
    "not json")         payload='<<<not json>>>' ;;
    "no file_path")     payload='{"tool_input":{}}' ;;
    "nonexistent path") payload='{"tool_input":{"file_path":"/nope/nope.rs"}}' ;;
  esac
  rc=0
  printf '%s' "$payload" | CLAUDE_PROJECT_DIR="$TMP" OBS_LOG="$TMP/obs.log" bash "$SCRIPT" >/dev/null 2>&1 || rc=$?
  [[ "$rc" == 0 ]] && ok "exit 0 on $label" || bad "exit $rc on $label"
done

# ── 7. A path containing spaces is handled ───────────────────────────────────────
mkdir -p "$TMP/src/with space"; gen "$TMP/src/with space/x.rs" 700
r="$(run "$TMP/src/with space/x.rs")"
[[ "$(rc_of "$r")" == 0 && "$(err_of "$r")" == *"HARD cap"* ]] \
  && ok "handles a path with a space" || bad "mishandled a spaced path: rc=$(rc_of "$r") '$(err_of "$r")'"

# ── 8. Exit 0 even when a SUB-TOOL errors, and nothing on stdout ─────────────────
# These pin the half of the contract that a `set -e` sabotage slipped past on 2026-08-06: the suite
# asserted exit 0 on paths where nothing failed, so it could not tell an advisory hook from a
# blocking one. Here rustfmt is handed a file it cannot parse, so it genuinely exits non-zero.
printf 'fn broken( { let x = ;;;\n' > "$TMP/src/bad.rs"
rc=0; sout=""
sout="$(printf '{"tool_input":{"file_path":"%s"}}' "$TMP/src/bad.rs" \
       | CLAUDE_PROJECT_DIR="$TMP" OBS_LOG="$TMP/obs.log" bash "$SCRIPT" 2>/dev/null)" || rc=$?
[[ "$rc" == 0 ]] && ok "exit 0 when rustfmt itself fails on unparseable Rust" \
                 || bad "exit $rc when rustfmt failed — this hook would BLOCK a write"
# stdout carries nothing but the one additionalContext JSON (plain text there never reaches the model
# and is not JSON the harness can parse). Empty is fine: rustfmt may be absent on this machine.
{ [[ -z "$sout" ]] || printf '%s' "$sout" | jq -e '.hookSpecificOutput.additionalContext' >/dev/null 2>&1; } \
  && ok "stdout is empty or the additionalContext JSON, never plain text" \
  || bad "plain text on stdout: '$sout'"

# Same for the .phg leg: a file the formatter rejects must warn, not block.
printf 'package Main;\nfunction main(  : {{{\n' > "$TMP/src/bad.phg"
rc=0
printf '{"tool_input":{"file_path":"%s"}}' "$TMP/src/bad.phg" \
  | CLAUDE_PROJECT_DIR="$TMP" OBS_LOG="$TMP/obs.log" bash "$SCRIPT" >/dev/null 2>&1 || rc=$?
[[ "$rc" == 0 ]] && ok "exit 0 when phg format --check rejects a .phg" \
                 || bad "exit $rc on an unformattable .phg — this hook would BLOCK a write"

# ── 9. It logs state-worthy events (global Rule 13 observability) ─────────────────
if (( HAVE_LOG_HELPERS )); then
  [[ -s "$TMP/obs.log" ]] && ok "wrote observability lines to \$OBS_LOG" \
                          || bad "logged nothing despite several reportable events"
else
  printf '  skip — observability: no global ~/.claude/hooks/log-helpers.sh on this machine\n'; SKIP=$((SKIP+1))
fi

echo
echo "$PASS passed, $FAIL failed, $SKIP skipped"
[[ "$FAIL" -eq 0 ]]
