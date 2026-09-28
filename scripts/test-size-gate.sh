#!/usr/bin/env bash
# Behaviour tests for scripts/size-gate.sh — run by scripts/git-hooks/pre-push, next to the other
# gates' own suites (test-microbench-gate, test-validate-infra, test-surface-ratchet): a gate whose
# behaviour suite no gate runs goes dark unnoticed (pre-push records three instances).
#
# Focus: scripts/size-baseline.txt is TRACKED DATA and must never be evaluated as code or read in a
# way that fails OPEN (panel 2026-09-28: safety O1 / R2-1 / R2-2, completeness N2 / N4, correctness N1).
#   - `(( lines > cap ))` evaluates array subscripts → a row `PWD[$(cmd)]` ran cmd at every push;
#   - a validation regex without its `$` anchor let `1+PWD[$(cmd)]` through;
#   - a count past bash's intmax WRAPS (2^64+100000 read as 100000) — counts are capped at 9 digits;
#   - `[0-9]` is a locale RANGE (it matched ٥ and ² under en_US.UTF-8), and a leading 0 is octal to
#     bash — both made `(( ))` error inside an `if`, i.e. a false condition, i.e. the gate PASSED.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOL="$HERE/size-gate.sh"
PASS=0
FAIL=0
ok()  { printf '  ok   — %s\n' "$1"; PASS=$((PASS + 1)); }
bad() { printf '  FAIL — %s\n' "$1"; FAIL=$((FAIL + 1)); }

TMP=$(mktemp -d /tmp/test-size-gate-XXXXXX)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/src" "$TMP/scripts"
cp "$TOOL" "$TMP/scripts/size-gate.sh"
gen() { python3 -c "import sys; n=int(sys.argv[2]); open(sys.argv[1],'w').write(''.join('// line %d\n'%i for i in range(n)))" "$1" "$2"; }
# $TMP is not a git repo, so size-gate.sh's `git rev-parse` falls back to cwd.
gate() { (cd "$TMP" && bash scripts/size-gate.sh 2>&1); }
row() { printf '%s\t%s\n' "$1" "$2" > "$TMP/scripts/size-baseline.txt"; }

echo "size-gate.sh — baseline rows are data"

# ── 1. The ordinary ratchet still works (controls: each later case must be able to differ) ──────
gen "$TMP/src/g.rs" 590; row 600 src/g.rs
out="$(gate)"; rc=$?
[[ $rc -eq 0 ]] && ok "a grandfathered file under its baseline passes" || bad "rc=$rc on a clean baseline: ${out:0:160}"
gen "$TMP/src/g.rs" 601
out="$(gate)"; rc=$?
[[ $rc -ne 0 && "$out" == *"grew"* ]] && ok "growth past the baseline fails" || bad "rc=$rc, growth not caught: ${out:0:160}"

# ── 2. A leading zero is DECIMAL, never octal (0600 read as octal is 384 → a 590-line file would FAIL) ─
gen "$TMP/src/g.rs" 590; row 0600 src/g.rs
out="$(gate)"; rc=$?
[[ $rc -eq 0 ]] && ok "0600 means 600 (base 10): 590 lines pass" || bad "rc=$rc — 0600 read as octal? ${out:0:160}"
gen "$TMP/src/g.rs" 700; row 08 src/g.rs
out="$(gate)"; rc=$?
[[ $rc -ne 0 ]] && ok "08 means 8: a 700-line file FAILS (octal '08' used to error and pass)" || bad "rc=0 on 08 — failed OPEN: ${out:0:160}"

# ── 3. Hostile rows are never executed, and fail CLOSED ──────────────────────────────────────────
gen "$TMP/src/g.rs" 10
for payload in 'PWD[$(touch${IFS}%s/pwned)]' '1+PWD[$(touch${IFS}%s/pwned)]'; do
  # shellcheck disable=SC2059 # the payload IS the format, on purpose
  row "$(printf "$payload" "$TMP")" src/g.rs
  rm -f "$TMP/pwned"
  out="$(gate)"; rc=$?
  label="${payload%%\$*}…"
  [[ ! -e "$TMP/pwned" ]] && ok "row '$label' is never executed" || bad "row '$label' EXECUTED a command"
  [[ $rc -ne 0 && "$out" == *"malformed"* ]] && ok "row '$label' fails closed, named malformed" \
                                             || bad "row '$label': rc=$rc ${out:0:160}"
done

# ── 4. Not-a-number rows fail closed (never skipped, never an arithmetic error that reads as false) ─
gen "$TMP/src/g.rs" 700
for cnt in abc '٥' '²' '' '6 00' 18446744073709651616; do
  row "$cnt" src/g.rs
  out="$(LC_ALL=en_US.UTF-8 gate)"; rc=$?
  [[ $rc -ne 0 && "$out" == *"malformed"* ]] && ok "count '$cnt' fails closed under en_US.UTF-8" \
                                             || bad "count '$cnt': rc=$rc ${out:0:160}"
done
# A row with no TAB (e.g. space-separated) used to be skipped silently while the lint hook honoured it.
printf '600 src/g.rs\n' > "$TMP/scripts/size-baseline.txt"
out="$(gate)"; rc=$?
[[ $rc -ne 0 && "$out" == *"malformed"* ]] && ok "a row without a TAB fails closed" || bad "tab-less row: rc=$rc ${out:0:160}"

# ── 5. The FAIL line never prints a row's path raw (it is attacker-controlled free text) ─────────
printf 'abc\tsrc/x\033[31m.rs\n' > "$TMP/scripts/size-baseline.txt"
out="$(gate)"
[[ "$out" != *$'\033'* ]] && ok "an escape sequence in a path is not echoed raw" || bad "raw ESC echoed in: ${out:0:160}"

rm -f "$TMP/scripts/size-baseline.txt"
printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 ]]
