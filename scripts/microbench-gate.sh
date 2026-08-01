#!/usr/bin/env bash
# microbench-gate.sh — the G-8 mandate RATCHET gate (pre-push lane, docker).
#
# Consumes `microbench.sh --json` (per-feature phorj-VM vs release-PHP+JIT) and gates against
# bench/micro-baseline.json. It BLOCKS a push ONLY on the two ROBUST, load-insensitive signals:
#   - OUTPUT-IDENTITY break (identical == false — VM and release-php disagree; a correctness bug, and
#     bench micros are NOT in the differential, so this is their only parity check).
#   - WIN->LOSS FLIP: a feature whose baseline ratio (php_ns/vm_ns) was a WIN (>= 1 — the VM beat php)
#     now LOSES (< 1). This IS the G-8 ratchet: once the VM beats release-php+JIT on a feature, it must
#     keep beating it. (The ratchet is ARMED: bench/micro-baseline.json records real WINs — e.g. floatarith ~4.2, closurecall ~2.1 — that a flip would block.)
#
# Per-feature ratio deltas are REPORTED, NOT blocked on: absolute microbench ns/ratio is too noisy to
# gate on a shared dev machine — empirically 3-4x swings at load average ~7, with NO code change. The
# robust VM-perf-regression gate is scripts/perf-gate.sh (same-process tree÷VM ratio: both backends
# share the CPU so load cancels — load-immune, unlike native-VM-vs-docker-php here). The two gates are
# complementary: perf-gate = "the VM didn't slow down"; this = "we didn't lose a feature we'd won" + parity.
#
# Usage:  microbench-gate.sh           gate the current tree (exit 1 on a flip/identity break)
#         microbench-gate.sh --emit    (re)write bench/micro-baseline.json from a fresh best-of-K run
# Env:    MICROBENCH_GATE_JSON=<file>  use that microbench-JSON instead of running the harness
#                                      (docker-free, deterministic — for tests); microbench.sh's own
#                                      (PHG_BIN, MICROBENCH_RUNS, MICROBENCH_PHP_IMAGE) otherwise.
# Requires docker + the release binary (unless the JSON seam is set). Either absent => SKIP with a
# warning (a push is never wedged by missing infra). Exit 0 pass/skip, 1 regression, 2 setup error.
set -eEuo pipefail
export LC_ALL=C

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASELINE="${MICROBENCH_BASELINE:-$ROOT/bench/micro-baseline.json}"
EMIT=0
[[ "${1:-}" == "--emit" ]] && EMIT=1

# A WIN->LOSS "flip" blocks only when the new ratio drops below this band. A parity baseline (~1.0 —
# floatmul/floatloop) wobbling within [FLIP_EPSILON, 1.0) is box noise on this shared machine (±3-5%
# php-side swings with no code change), not a regression — MASTER-PLAN §0 gate-infra ruling, option (a),
# 2026-07-13. A genuine loss (new ratio < FLIP_EPSILON) still fails. Override via MICROBENCH_FLIP_EPSILON.
FLIP_EPSILON="${MICROBENCH_FLIP_EPSILON:-0.95}"

# An OWED loss may not silently DEEPEN. `--emit` records every feature that loses at emit time into
# `_owed` (derived, never hand-maintained — see the emit block), and this is the band within which an
# owed ratio may drift before the gate blocks. Generous on purpose: these are absolute native-vs-php
# ratios on a shared box, so a tight band would wedge pushes on noise. A 25% deepening is not noise.
OWED_EPSILON="${MICROBENCH_OWED_EPSILON:-0.75}"

# A WIN->LOSS flip must also be a meaningful drop RELATIVE to the feature's own baseline. See the
# comment at the flip check: an absolute band alone cannot tell a near-parity wobble from a regression.
RELATIVE_DROP="${MICROBENCH_RELATIVE_DROP:-0.85}"

# Annotate `[noisy]` when the VM leg's slowest sample exceeds its fastest by this much. Changes NO
# verdict — information only, and free (the harness already takes the K samples). phorj's short
# high-IPC loops carry a 25-40% per-iteration variance here, so best-of-K lands ABOVE the true minimum
# and the ratio reads PESSIMISTIC; 15% sits above php's observed 2-5%. Full rationale: DEC-430/430.1.
#
# ⚠ ONE-WAY EVIDENCE. Over K=3 the spread is a DETECTOR, not a measurement — three draws miss tails
# (`listcontains` read +1% then +59% minutes apart). A marker means "distrust this row"; its ABSENCE
# means only "these three agreed", never "solid". Reading it as a certificate is worse than no marker.
NOISE_PCT="${MICROBENCH_NOISE_PCT:-15}"

# Flags for the one-shot "does this php actually JIT?" probe (the local-baseline gate below).
JIT_PROBE="-dopcache.enable_cli=1 -dopcache.jit_buffer_size=8M -dopcache.jit=tracing"

command -v jq >/dev/null 2>&1 || {
  echo "microbench-gate: jq is required" >&2
  exit 2
}

# Acquire the measurement JSON: the testing seam bypasses docker/binary/harness entirely.
if [[ -n "${MICROBENCH_GATE_JSON:-}" ]]; then
  [[ -f "$MICROBENCH_GATE_JSON" ]] || {
    echo "microbench-gate: MICROBENCH_GATE_JSON=$MICROBENCH_GATE_JSON not found" >&2
    exit 2
  }
  json="$(cat "$MICROBENCH_GATE_JSON")"
else
  # PHP SOURCE, resolved in order: an explicit MICROBENCH_PHP_BIN, then docker, then the stack's
  # oracle php. Docker stays the cross-box reference, but its absence used to mean the ratchet simply
  # never ran — and in this container it is always unusable, so the gate was dark on every push for
  # weeks (DEC-423).
  #
  # ⚠ "Docker is usable" means the DAEMON answers, not that the client is installed. The dev container
  # ships the binary with no daemon behind it, so a `command -v docker` test passes and the run then
  # dies on connect. The first arming attempt got this wrong in exactly that way — the fallback was
  # gated on the binary being ABSENT, so it never fired here and the very next push still printed
  # "docker daemon unreachable — SKIP". Both conditions are folded into one probe below.
  if [[ -z "${MICROBENCH_PHP_BIN:-}" ]] && ! docker version >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    [[ -f "$ROOT/scripts/toolchain.env" ]] && source "$ROOT/scripts/toolchain.env"
    # PROBE the JIT, never assume it: a php without opcache, or with JIT off, is not a valid G-8
    # baseline and silently using one would understate every loss (the mb_strlen lesson, DEC-423).
    if [[ -n "${PHORJ_PHP:-}" && -x "${PHORJ_PHP:-}" ]] && "$PHORJ_PHP" $JIT_PROBE -r \
        'exit((opcache_get_status(false)["jit"]["on"] ?? false) ? 0 : 1);' >/dev/null 2>&1; then
      export MICROBENCH_PHP_BIN="$PHORJ_PHP"
      echo "microbench-gate: docker unusable — using the local release php+JIT ($PHORJ_PHP)" >&2
    else
      echo "microbench-gate: docker unusable and no local php+JIT — SKIP the G-8 gate (infra, not a" >&2
      echo "  regression). The verdict is OWED: re-run where a real release php+JIT is reachable" >&2
      echo "  before making any perf claim (DEC-365 no-hidden-loss)." >&2
      exit 0
    fi
  fi
  BIN="${PHG_BIN:-$ROOT/target/release/phg}"
  if [[ ! -x "$BIN" ]]; then
    echo "microbench-gate: release binary $BIN absent — SKIP (run: cargo build --release; infra, not a regression)" >&2
    exit 0
  fi
  # Load guard: this gate compares native-VM vs docker-php ABSOLUTE ratios, which swing 3-4x on this
  # shared box under load (the pinned core is not isolated via isolcpus). Blocking a push on a sample
  # taken under load yields FALSE WIN->LOSS flips — verified 2026-07-13: the collection micros read
  # 1.1-1.7 (WIN) at load <2 but 0.2-0.5 (LOSS) at load ~7, with NO code change. The load-IMMUNE
  # VM-regression gate is perf-gate.sh (same-process tree/VM ratio); THIS ratchet needs a quiet box
  # (MASTER-PLAN §0: "MUST re-run microbench-gate on a QUIET box"). So SKIP (never block) when the
  # 1-min load exceeds MICROBENCH_MAX_LOAD — a push is never wedged by an unmeasurable-under-load box.
  _maxload="${MICROBENCH_MAX_LOAD:-2.5}"
  # SETTLE, then skip. The load guard is right — measuring absolute native-vs-php ratios on a loaded
  # box manufactures false flips — but in the pre-push lane the load is caused by the lane ITSELF: the
  # full test suite, two clippy passes and a release build run immediately before this. So the guard
  # tripped essentially every time (measured 2.78 right after `cargo build --release`), and a gate that
  # always skips is the same disease as a gate that never runs, one layer down.
  #
  # The load is TRANSIENT — those processes have already exited and the 1-minute average is just
  # decaying — so wait for it rather than giving up on it. Bounded (default 90 s, polled every 5 s):
  # if it settles we measure, if it does not we skip exactly as before. This is not a retry loop around
  # a flaky operation; it is waiting for a known, self-inflicted, self-clearing condition.
  _settle="${MICROBENCH_SETTLE_SECS:-90}"
  _waited=0
  _load1="$(cut -d' ' -f1 /proc/loadavg 2>/dev/null || echo 0)"
  while awk -v l="$_load1" -v m="$_maxload" 'BEGIN{exit (l>m)?0:1}' && [[ "$_waited" -lt "$_settle" ]]; do
    [[ "$_waited" == 0 ]] && echo "microbench-gate: 1-min load $_load1 > $_maxload (the pre-push lane's own build) — waiting up to ${_settle}s for it to settle" >&2
    sleep 5
    _waited=$((_waited + 5))
    _load1="$(cut -d' ' -f1 /proc/loadavg 2>/dev/null || echo 0)"
  done
  if awk -v l="$_load1" -v m="$_maxload" 'BEGIN{exit (l>m)?0:1}'; then
    echo "microbench-gate: 1-min load $_load1 still > $_maxload after ${_settle}s — SKIP the G-8 ratchet (box too loaded to measure absolute VM-vs-php ratios reliably; perf-gate.sh still gates VM regressions). Not a regression; the verdict is OWED." >&2
    exit 0
  fi
  [[ "$_waited" -gt 0 ]] && echo "microbench-gate: load settled to $_load1 after ${_waited}s — measuring" >&2
  json="$(bash "$ROOT/scripts/microbench.sh" --json)" || {
    echo "microbench-gate: harness run failed" >&2
    exit 2
  }
fi

if [[ "$EMIT" == 1 ]]; then
  # `_owed` is DERIVED, never hand-maintained: every feature losing at emit time is recorded here with
  # the ratio it lost by. That is what stops `--emit` from laundering a loss (DEC-365 no-hidden-loss,
  # and the developer ruling of 2026-08-01 that the 9 known losses be frozen as OWED rather than
  # written in as the new normal). The gate then reports every owed feature on EVERY run and BLOCKS if
  # one deepens — so a loss can be carried, but never quietly, and never further.
  jq '{
    "_comment": "G-8 mandate ratchet baseline (scripts/microbench-gate.sh). Per-feature php/vm ratio + output-identity vs release-php+JIT. The gate BLOCKS on identity breaks, WIN->LOSS flips (ratio crossing 1.0 downward), and any _owed loss DEEPENING past MICROBENCH_OWED_EPSILON. It does NOT block on ratio magnitude (too noisy on a shared machine; perf-gate.sh is the robust VM-regression gate). RATCHET: re-emit after a fix lands a WIN so the flip check protects it.",
    "_owed_comment": "DERIVED at --emit from every feature with ratio < 1.0: the losses we are CARRYING, each with the ratio it lost by. Reported loudly every run and blocked from deepening. A feature leaves this list by being FIXED and re-emitted, never by being edited out.",
    "_baseline_php": "'"${MICROBENCH_PHP_BIN:-docker php:8.5-cli}"'",
    _owed: (map(select(.ratio < 1.0) | { (.feature): { ratio: .ratio } }) | add // {}),
    features: (map({ (.feature): { ratio: .ratio, identical: .identical } }) | add)
  }' <<<"$json" >"$BASELINE"
  echo "microbench-gate: wrote baseline -> $BASELINE ($(jq '.features | length' "$BASELINE") features, $(jq '._owed | length' "$BASELINE") OWED loss(es))"
  jq -r '._owed | to_entries[] | "  OWED \(.key): ratio \(.value.ratio) — carried, must not deepen"' "$BASELINE"
  exit 0
fi

[[ -f "$BASELINE" ]] || {
  echo "microbench-gate: no baseline at $BASELINE — run: bash scripts/microbench-gate.sh --emit" >&2
  exit 2
}

fails=0
wins=0
owed=0
# Timing-based blocks are CONFIRMED before they block. An output-identity break is not timing and
# blocks immediately; a flip or an owed deepening is re-measured first — see the confirmation pass.
suspects=()
suspect_kind=()
noisy_n=0
# `noise` is the suffix appended to whatever this feature's line turns out to be. Computed once, here,
# so every branch below (owed / recovered / flip / wobble / ok) carries it without repeating the math.
while IFS=$'\t' read -r feat ratio identical vm_ns vm_worst; do
  [[ -n "$feat" ]] || continue
  # A JSON without the spread fields (older run / test fixture) yields the literal "null" — "not
  # measured", never a zero to divide by nor a 0% that would read as CLEAN. The `!= "null"` halves are
  # DEFENSIVE, not load-bearing (verified by deletion: bash coerces the bare word to 0 and `-gt 0`
  # rejects it); kept because that coercion is obscure and a future field may not coerce.
  noise=""
  if [[ "$vm_ns" != "null" && "$vm_worst" != "null" && "${vm_ns:-0}" -gt 0 && "${vm_worst:-0}" -gt 0 ]]; then
    _sp="$(awk -v b="$vm_ns" -v w="$vm_worst" 'BEGIN{printf "%.0f", (w/b-1)*100}')"
    if [[ "$_sp" -ge "$NOISE_PCT" ]]; then
      # One-sided on purpose (`+N%`, never `±N%`): best-of-K is an upper bound on the real VM time,
      # so the truth is at or BELOW what is printed. Short here; explained once in the summary.
      noise="  [noisy: VM spread +${_sp}%]"
      noisy_n=$((noisy_n + 1))
    fi
  fi
  if [[ "$identical" != "true" ]]; then
    echo "  FAIL $feat: output-identity break (VM vs PHP checksum differ) — a correctness bug, not a timing"
    fails=$((fails + 1))
    continue
  fi
  b_ratio="$(jq -r --arg f "$feat" '.features[$f].ratio // empty' "$BASELINE")"
  owed_ratio="$(jq -r --arg f "$feat" '._owed[$f].ratio // empty' "$BASELINE")"
  win_now="$(awk -v r="$ratio" 'BEGIN{print (r>=1.0)?"WIN":"loss"}')"
  [[ "$win_now" == "WIN" ]] && wins=$((wins + 1))
  if [[ -z "$b_ratio" ]]; then
    echo "  note $feat: not in baseline (new) — ratio=$ratio ($win_now); run --emit to snapshot it"
    continue
  fi
  # An OWED loss: carried deliberately, reported every run, blocked from deepening. Checked BEFORE the
  # flip logic because a feature cannot be both (the flip check needs a WIN baseline).
  if [[ -n "$owed_ratio" ]]; then
    owed=$((owed + 1))
    if [[ "$win_now" == "WIN" ]]; then
      echo "  RECOVERED $feat: owed at $owed_ratio, now $ratio (a WIN) — re-emit so the ratchet protects it$noise"
      continue
    fi
    if awk -v o="$owed_ratio" -v r="$ratio" -v eps="$OWED_EPSILON" 'BEGIN{exit (r < o*eps)?0:1}'; then
      echo "  susp $feat: an OWED loss may have DEEPENED — was $owed_ratio, now $ratio; re-measuring to confirm"
      suspects+=("$feat")
      suspect_kind+=("owed:$owed_ratio:$ratio")
      continue
    fi
    echo "  owed $feat: ratio $owed_ratio -> $ratio (still losing; carried, not laundered)$noise"
    continue
  fi
  # BLOCK: a feature we had WON now LOSES by MORE than the noise band (the G-8 ratchet).
  # A parity baseline (~1.0 — floatmul/floatloop) wobbling a fraction below 1.0 is box noise on this
  # shared machine (empirically ±3-5% php-side swings with NO code change; MASTER-PLAN §0 gate-infra
  # ruling), NOT a regression: only a drop below FLIP_EPSILON blocks. A genuine >5% loss still fails.
  # The band is RELATIVE to the baseline as well as absolute, because a feature that only JUST wins
  # cannot be distinguished from noise by an absolute threshold. `mapinsert` (baseline 1.012) tripping
  # at 0.940 is the live case that forced this — a 7% wobble on a shared box, not a regression, and it
  # would have wedged every push. A feature must now be BOTH below the absolute band AND clearly down
  # on its own baseline (`RELATIVE_DROP`). A strong WIN is unaffected: baseline 5.0 still blocks the
  # moment it drops under 0.95, because 5.0 * 0.85 is far above that and the absolute term binds.
  if awk -v br="$b_ratio" -v r="$ratio" -v eps="$FLIP_EPSILON" -v rel="$RELATIVE_DROP" \
     'BEGIN{ lim = (br*rel < eps) ? br*rel : eps; exit (br>=1.0 && r<lim)?0:1 }'; then
    echo "  susp $feat: possible WIN->LOSS flip — baseline $b_ratio (WIN) now $ratio; re-measuring to confirm"
    suspects+=("$feat")
    suspect_kind+=("flip:$b_ratio:$ratio")
    continue
  fi
  # Near-parity wobble: a WIN baseline now fractionally < 1.0 but within the FLIP_EPSILON band — box
  # noise on a parity baseline, reported not blocked (so a shared-machine push is never wedged by it).
  if awk -v br="$b_ratio" -v r="$ratio" 'BEGIN{exit (br>=1.0 && r<1.0)?0:1}'; then
    echo "  warn $feat: near-parity wobble — baseline $b_ratio now $ratio (within $FLIP_EPSILON noise band; not blocking)$noise"
    continue
  fi
  # REPORT (non-blocking): ratio movement vs baseline.
  echo "  ok   $feat: ratio $b_ratio -> $ratio ($win_now)$noise"
done < <(jq -r '.[] | [.feature, .ratio, .identical, (.vm_ns // null), (.vm_worst_ns // null)] | @tsv' <<<"$json")

# CONFIRMATION PASS. A timing-based verdict is re-measured before it blocks a push.
#
# Why: this gate compares ABSOLUTE native-vs-php ratios, which move with box load — that is why it has
# a load guard at all. Now that the ratchet actually runs (it was dark for weeks, DEC-423), a false
# block would wedge a push, and the pre-push lane is precisely where the box is busiest. Observed
# live: one run at load ~1.5 reported a blocking flip that did not reproduce at all on the next run.
#
# Re-measuring ONLY the flagged features is cheap (seconds, not the full 51) and it is the honest
# discriminator: a real regression reproduces, load noise does not. This does NOT weaken the ratchet —
# a confirmed flip still blocks — it removes the false ones. An output-identity break never comes here:
# it is a correctness signal, not a timing one, and blocks on sight.
if [[ ${#suspects[@]} -gt 0 && -z "${MICROBENCH_GATE_JSON:-}" ]]; then
  echo "microbench-gate: re-measuring ${#suspects[@]} suspect(s) to confirm: ${suspects[*]}"
  if confirm_json="$(bash "$ROOT/scripts/microbench.sh" --json "${suspects[@]}" 2>/dev/null)"; then
    while IFS=$'\t' read -r feat ratio; do
      [[ -n "$feat" ]] || continue
      b_ratio="$(jq -r --arg f "$feat" '.features[$f].ratio // empty' "$BASELINE")"
      owed_ratio="$(jq -r --arg f "$feat" '._owed[$f].ratio // empty' "$BASELINE")"
      if [[ -n "$owed_ratio" ]]; then
        if awk -v o="$owed_ratio" -v r="$ratio" -v eps="$OWED_EPSILON" 'BEGIN{exit (r < o*eps)?0:1}'; then
          echo "  FAIL $feat: an OWED loss DEEPENED — was $owed_ratio, confirmed at $ratio"
          fails=$((fails + 1))
        else
          echo "  ok   $feat: not confirmed on re-measure ($ratio) — load noise, not a regression"
        fi
        continue
      fi
      if awk -v br="$b_ratio" -v r="$ratio" -v eps="$FLIP_EPSILON" -v rel="$RELATIVE_DROP" \
         'BEGIN{ lim = (br*rel < eps) ? br*rel : eps; exit (br>=1.0 && r<lim)?0:1 }'; then
        echo "  FAIL $feat: WIN->LOSS flip — baseline $b_ratio (WIN), confirmed at $ratio"
        fails=$((fails + 1))
      else
        echo "  ok   $feat: not confirmed on re-measure ($ratio) — load noise, not a regression"
      fi
    done < <(jq -r '.[] | [.feature, .ratio] | @tsv' <<<"$confirm_json")
  else
    # Could not re-measure: report the suspects, do NOT block on an unconfirmed timing (DEC-365 —
    # unmeasurable is OWED, never a block and never a silent pass).
    echo "microbench-gate: re-measure FAILED — suspects unconfirmed and NOT blocked; verdict OWED for: ${suspects[*]}" >&2
  fi
elif [[ ${#suspects[@]} -gt 0 ]]; then
  # The JSON seam (tests) is deterministic by construction — there is no load noise to confirm away,
  # so a suspect IS the verdict. Same wording as the confirmed path, so the tests pin the real message.
  for i in "${!suspects[@]}"; do
    IFS=: read -r kind was now <<<"${suspect_kind[$i]}"
    if [[ "$kind" == "owed" ]]; then
      echo "  FAIL ${suspects[$i]}: an OWED loss DEEPENED — was $was, now $now"
    else
      echo "  FAIL ${suspects[$i]}: WIN->LOSS flip — baseline $was (WIN) now $now"
    fi
    fails=$((fails + 1))
  done
fi

echo "microbench-gate: $wins WIN / $(($(jq 'length' <<<"$json") - wins)) loss vs release-php+JIT; $owed OWED (carried); $fails blocking regression(s)"
# Say once what the per-line `[noisy]` markers mean. Silent when nothing was noisy.
if [[ "$noisy_n" -gt 0 ]]; then
  echo "microbench-gate: $noisy_n feature(s) measured with >${NOISE_PCT}% VM spread — their ratios are PESSIMISTIC"
  echo "  (best-of-K is an upper bound on the real VM time; raising MICROBENCH_RUNS tightens it. DEC-430)"
fi
if [[ "$fails" -gt 0 ]]; then
  echo "microbench-gate: FAIL — $fails regression(s) (WIN->LOSS flip or output-identity break)" >&2
  exit 1
fi
echo "microbench-gate: PASS (ratchet: no flips, all output-identical)"
