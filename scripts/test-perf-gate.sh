#!/usr/bin/env bash
# Behaviour tests for scripts/perf-gate.sh — run by scripts/git-hooks/pre-push next to the other gates'
# own suites. No cargo build, no timing: PHG_BIN is a fake that prints a fixed vm_speedup and counts
# its invocations, so every case is deterministic and ~1s.
#
# Focus: bench/baseline.json (and PERF_GATE_RUNS) are DATA and must never be evaluated as code or read
# in a way that fails OPEN (panel 2026-09-28, round 2 — the size-gate hole reproduced here):
#   - `for ((i = 1; i <= runs; i++))` evaluates array subscripts → runs = "PWD[$(cmd)]" ran cmd;
#   - a leading 0 is octal to bash ("08" errored) — the count is forced to base 10;
#   - baseline_vm_speedup / min_ratio go through awk, where a missing key (jq prints `null`) or text
#     reads as 0 → floor 0.0000 → every run PASSES. A setup error (exit 2) instead.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PASS=0
FAIL=0
ok()  { printf '  ok   — %s\n' "$1"; PASS=$((PASS + 1)); }
bad() { printf '  FAIL — %s\n' "$1"; FAIL=$((FAIL + 1)); }

TMP=$(mktemp -d /tmp/test-perf-gate-XXXXXX)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/scripts" "$TMP/bench" "$TMP/examples"
cp "$HERE/perf-gate.sh" "$TMP/scripts/perf-gate.sh"
: > "$TMP/examples/w.phg"
# The fake binary: appends one line per invocation, prints the speedup held in $TMP/speedup.
cat > "$TMP/phg" <<EOF
#!/usr/bin/env bash
echo x >> "$TMP/calls"
printf '{"vm_speedup": %s}\n' "\$(cat "$TMP/speedup")"
EOF
chmod +x "$TMP/phg"

# base <baseline_vm_speedup> <min_ratio> <runs> — each argument is a raw JSON value.
base() {
  printf '{"workload": "examples/w.phg", "baseline_vm_speedup": %s, "min_ratio": %s, "runs": %s}\n' \
    "$1" "$2" "$3" > "$TMP/bench/baseline.json"
}
gate() { rm -f "$TMP/calls"; (cd "$TMP" && PHG_BIN="$TMP/phg" bash scripts/perf-gate.sh 2>&1); }
calls() { [[ -f "$TMP/calls" ]] && wc -l < "$TMP/calls" | tr -d ' ' || echo 0; }

echo "perf-gate.sh — baseline values are data"

# ── 1. Controls: the ordinary gate passes and fails (each later case must be able to differ) ──────
echo 20 > "$TMP/speedup"; base 18.0 0.6 3
out="$(gate)"; rc=$?
[[ $rc -eq 0 && "$(calls)" -eq 3 ]] && ok "20 vs floor 10.8 passes, 3 runs" || bad "control pass: rc=$rc calls=$(calls) ${out:0:160}"
echo 5 > "$TMP/speedup"
out="$(gate)"; rc=$?
[[ $rc -eq 1 ]] && ok "5 vs floor 10.8 is a regression (exit 1)" || bad "control fail: rc=$rc ${out:0:160}"

# ── 2. A leading zero is DECIMAL: "08" means 8 runs (octal 08 used to be an arithmetic error) ────
echo 20 > "$TMP/speedup"; base 18.0 0.6 '"08"'
out="$(gate)"; rc=$?
[[ $rc -eq 0 && "$(calls)" -eq 8 ]] && ok "runs \"08\" = 8 runs" || bad "runs 08: rc=$rc calls=$(calls) ${out:0:160}"

# ── 3. Hostile run counts are never executed, and are a setup error (exit 2) ───────────────────────
for payload in 'PWD[$(touch${IFS}%s/pwned)]' '1+PWD[$(touch${IFS}%s/pwned)]'; do
  # shellcheck disable=SC2059 # the payload IS the format, on purpose
  hostile="$(printf "$payload" "$TMP")"
  label="${payload%%\$*}…"
  base 18.0 0.6 "$(jq -n --arg v "$hostile" '$v')"
  rm -f "$TMP/pwned"; out="$(gate)"; rc=$?
  [[ ! -e "$TMP/pwned" ]] && ok "baseline runs '$label' is never executed" || bad "baseline runs '$label' EXECUTED a command"
  [[ $rc -eq 2 ]] && ok "baseline runs '$label' is a setup error" || bad "baseline runs '$label': rc=$rc ${out:0:160}"
  base 18.0 0.6 3
  rm -f "$TMP/pwned"; out="$(PERF_GATE_RUNS="$hostile" gate)"; rc=$?
  [[ ! -e "$TMP/pwned" ]] && ok "PERF_GATE_RUNS '$label' is never executed" || bad "PERF_GATE_RUNS '$label' EXECUTED a command"
  [[ $rc -eq 2 ]] && ok "PERF_GATE_RUNS '$label' is a setup error" || bad "PERF_GATE_RUNS '$label': rc=$rc ${out:0:160}"
done

# ── 4. Not-a-count runs are a setup error: zero runs would measure nothing ─────────────────────────
for r in 0 '"abc"' '"٣"' null '"-3"' '"18446744073709551621"'; do
  base 18.0 0.6 "$r"
  out="$(gate)"; rc=$?
  [[ $rc -eq 2 && "$(calls)" -eq 0 ]] && ok "runs $r is a setup error, nothing run" || bad "runs $r: rc=$rc calls=$(calls) ${out:0:160}"
done

# ── 5. A missing or non-numeric floor input must not zero the floor (that PASSES everything) ───────
echo 0.5 > "$TMP/speedup"   # a catastrophic regression: only a zeroed floor lets it pass
for pair in 'null|0.6' '"abc"|0.6' '18.0|null' '18.0|"x"' '0|0.6' '18.0|0'; do
  base "${pair%%|*}" "${pair#*|}" 3
  out="$(gate)"; rc=$?
  [[ $rc -eq 2 ]] && ok "baseline_vm_speedup/min_ratio = $pair is a setup error" || bad "$pair: rc=$rc ${out:0:160}"
done

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 ]]
