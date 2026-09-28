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
# The fake binary: appends one line per invocation; call N prints line N of $TMP/speedup (the last line
# once they run out) as the vm_speedup JSON value — or, for the literal line `{}`, an object with no key.
cat > "$TMP/phg" <<EOF
#!/usr/bin/env bash
echo x >> "$TMP/calls"
n=\$(wc -l < "$TMP/calls")
v=\$(sed -n "\${n}p" "$TMP/speedup"); [[ -n "\$v" ]] || v=\$(tail -n 1 "$TMP/speedup")
if [[ "\$v" == "{}" ]]; then echo '{}'; else printf '{"vm_speedup": %s}\n' "\$v"; fi
EOF
chmod +x "$TMP/phg"
# A stub `x` on PATH: the 9-char hostile count `1+_[$(x)]` sits UNDER the 9-digit cap, so only the
# digit-list check keeps it out (round 3: every longer payload was stopped by the cap alone).
mkdir -p "$TMP/bin"; printf '#!/bin/sh\ntouch "%s/pwned"\n' "$TMP" > "$TMP/bin/x"; chmod +x "$TMP/bin/x"

# base <baseline_vm_speedup> <min_ratio> <runs> [workload] — each argument is a raw JSON value.
base() {
  printf '{"workload": %s, "baseline_vm_speedup": %s, "min_ratio": %s, "runs": %s}\n' \
    "${4:-\"examples/w.phg\"}" "$1" "$2" "$3" > "$TMP/bench/baseline.json"
}
gate() { rm -f "$TMP/calls"; (cd "$TMP" && PATH="$TMP/bin:$PATH" PHG_BIN="$TMP/phg" bash scripts/perf-gate.sh 2>&1); }
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
for payload in 'PWD[$(touch${IFS}%s/pwned)]' '1+PWD[$(touch${IFS}%s/pwned)]' '1+_[$(x)]'; do
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
for pair in 'null|0.6' '"abc"|0.6' '18.0|null' '18.0|"x"' '0|0.6' '18.0|0' '"0x"|0.6' '18.0|"0.6x"'; do
  base "${pair%%|*}" "${pair#*|}" 3
  out="$(gate)"; rc=$?
  [[ $rc -eq 2 ]] && ok "baseline_vm_speedup/min_ratio = $pair is a setup error" || bad "$pair: rc=$rc ${out:0:160}"
done

# ── 6. The MEASURED value is data too: a null / missing / text vm_speedup is a setup error ──────────
# awk compares a non-number to the floor AS A STRING ("null" >= "10.8" is true), so one unmeasurable
# run (`phg benchmark --json` emits null when a leg measures 0 ns) used to PASS the gate — even inside
# best-of-N next to real regressions (round 3, safety R3-2).
base 18.0 0.6 3
printf '5\n5\n5\n' > "$TMP/speedup"
out="$(gate)"; rc=$?
[[ $rc -eq 1 ]] && ok "control: three 5x runs vs floor 10.8 are a regression" || bad "control 5,5,5: rc=$rc ${out:0:160}"
# "5x" / "x5" / "1e3junk" pin both regex anchors (round 4: an anchor-dropped check PASSED "5x").
for seq in 'null' '"fast"' '{}' '"NaN"' '5|null|5' 'null|5|5' '"5x"' '"x5"' '"1e3junk"'; do
  tr '|' '\n' <<<"$seq" > "$TMP/speedup"
  out="$(gate)"; rc=$?
  [[ $rc -eq 2 ]] && ok "measured vm_speedup $seq is a setup error, never a PASS" || bad "measured $seq: rc=$rc ${out:0:160}"
done

printf '%s\n' '"\u001b]0;T\u0007"' > "$TMP/speedup"
out="$(gate)"; rc=$?
[[ $rc -eq 2 && "$out" != *$'\033'* ]] && ok "a measured value with control bytes is a setup error, printed %q" || bad "measured ESC: rc=$rc ${out:0:160}"

# ── 7. The workload string is printed %q-quoted: raw control bytes never reach the terminal/model ──
echo 20 > "$TMP/speedup"; base 18.0 0.6 1 '"examples/w\u001b]0;T\u0007.phg"'
out="$(gate)"
[[ "$out" != *$'\033'* ]] && ok "an escape sequence in the workload is not printed raw" || bad "raw ESC in: ${out:0:160}"

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 ]]
