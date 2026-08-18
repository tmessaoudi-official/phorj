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
# OBS_LOG override). On a machine without ~/.claude the hook's no-op fallback fires and the
# assertion fails LOUDLY — that is the signal this test needs the global install, not a defect.
[[ -f "$HOME/.claude/hooks/log-helpers.sh" ]] \
  || echo "WARN: no global log-helpers.sh — the observability assertion will fail" >&2

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
# size-gate.sh:47 is `lines <= HARD`, so at exactly 500 a grandfathered file raises stale=1 and it
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
[[ -z "$sout" ]] && ok "writes nothing to stdout (advisories belong on stderr)" \
                 || bad "wrote to stdout: '$sout'"

# Same for the .phg leg: a file the formatter rejects must warn, not block.
printf 'package Main;\nfunction main(  : {{{\n' > "$TMP/src/bad.phg"
rc=0
printf '{"tool_input":{"file_path":"%s"}}' "$TMP/src/bad.phg" \
  | CLAUDE_PROJECT_DIR="$TMP" OBS_LOG="$TMP/obs.log" bash "$SCRIPT" >/dev/null 2>&1 || rc=$?
[[ "$rc" == 0 ]] && ok "exit 0 when phg format --check rejects a .phg" \
                 || bad "exit $rc on an unformattable .phg — this hook would BLOCK a write"

# ── 9. It logs state-worthy events (global Rule 13 observability) ─────────────────
[[ -s "$TMP/obs.log" ]] && ok "wrote observability lines to \$OBS_LOG" \
                        || bad "logged nothing despite several reportable events"

echo
echo "$PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
