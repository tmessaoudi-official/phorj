#!/usr/bin/env bash
# microbench.sh — per-feature phorj-VM vs release-PHP+JIT (total self-timed ns), the G-8 harness.
#
# For each feature `bench/micro/<name>.phg` (with a hand-authored idiomatic `bench/micro/<name>.php`):
#   - phorj: `phg run <name>.phg`  (the VM) — best-of-K on this host.
#   - PHP:   `<name>.php` under `docker run php:8.5-cli` with opcache+JIT (a REAL release php — the
#            local builds are all ZTS DEBUG, JIT off, so they are NOT a valid baseline).
# Each micro self-times (warmup call + timed call) and prints `name<TAB>total_ns<TAB>checksum` — TOTAL
# self-timed nanoseconds, NOT ns/op: the old integer per-op (`d / iters`) floored sub-2ns/op workloads
# to `1`, collapsing distinct timings to a meaningless 1.00× tie (it masked intadd's true verdict). The
# ratio is scale-invariant (iters cancels — both legs run the same count), so total-ns gives full
# resolution. The checksum defeats dead-code elimination AND gates output-identity (VM and PHP must
# agree before a timing is trusted). Ratio = php_ns / vm_ns; WIN means the VM is faster (the G-8 bar).
#
# SAMPLING (P-2c hardening — the fibrec phantom-flip postmortem): samples are INTERLEAVED per
# feature (phg then php within each of K rounds) and BOTH sides are PINNED to one core
# (taskset / docker --cpuset-cpus). The old batched phases (all phg, then all php in one
# container) manufactured a 5.4x phantom WIN->LOSS flip on fibrec with NO code change under
# ambient load (the JIT was measured intact at 35x over the VM at the same moment):
# interleaving cancels load drift within a pair, pinning cancels scheduler-migration noise —
# the same discipline as the hand measurements. The php side runs in ONE long-lived pinned
# container via `docker exec` per sample (launch/exec overhead never enters the self-timed ns).
#
# Env: PHG_BIN (default target/release/phg), MICROBENCH_RUNS (K, default 3),
#      MICROBENCH_PHP_IMAGE (default php:8.5-cli),
#      MICROBENCH_CPU (pin core, default: last core). Flags: --json. Positional args run
#      ONLY those micros (one-by-one measurement): `microbench.sh jsonround dbwork`.
set -eEuo pipefail
export LC_ALL=C

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${PHG_BIN:-$ROOT/target/release/phg}"
MICRO="$ROOT/bench/micro"
K="${MICROBENCH_RUNS:-3}"
PHP_IMAGE="${MICROBENCH_PHP_IMAGE:-php:8.5-cli}"
JIT_FLAGS="-dopcache.enable_cli=1 -dopcache.jit_buffer_size=128M -dopcache.jit=tracing"
JSON=0
# Positional args = run ONLY these micros (e.g. `microbench.sh jsonround dbwork`); --json composes.
ONLY=()
for arg in "$@"; do
  case "$arg" in
    --json) JSON=1 ;;
    -*) echo "microbench: unknown flag $arg" >&2; exit 2 ;;
    *) ONLY+=("$arg") ;;
  esac
done

command -v docker >/dev/null 2>&1 || {
  echo "microbench: docker required (real release-php baseline)" >&2
  exit 2
}
[[ -x "$BIN" ]] || {
  echo "microbench: binary not found at $BIN — run: cargo build --release" >&2
  exit 2
}

features=()
for f in "$MICRO"/*.phg; do
  name="$(basename "$f" .phg)"
  [[ -f "$MICRO/$name.php" ]] && features+=("$name")
done
[[ ${#features[@]} -gt 0 ]] || {
  echo "microbench: no paired *.phg/*.php micros in $MICRO" >&2
  exit 2
}

# One-by-one runs: keep only the requested micros (exact names), loudly rejecting typos.
if [[ ${#ONLY[@]} -gt 0 ]]; then
  filtered=()
  for want in "${ONLY[@]}"; do
    found=0
    for name in "${features[@]}"; do
      [[ "$name" == "$want" ]] && { filtered+=("$name"); found=1; }
    done
    [[ $found -eq 1 ]] || {
      echo "microbench: no micro named \`$want\` (have: ${features[*]})" >&2
      exit 2
    }
  done
  features=("${filtered[@]}")
fi

# Phases 1+2 — INTERLEAVED, PINNED sampling (see the header): one long-lived pinned php
# container; per feature, K rounds of (pinned phg sample, pinned php sample), best-of-K each.
CPU="${MICROBENCH_CPU:-$(($(nproc) - 1))}"
CONTAINER="$(docker run -d --rm --cpuset-cpus="$CPU" -v "$MICRO:/w:ro" "$PHP_IMAGE" sleep infinity)"
cleanup_container() { docker rm -f "$CONTAINER" >/dev/null 2>&1 || true; }
trap cleanup_container EXIT

declare -A vm_ns vm_sum php_ns php_sum
for name in "${features[@]}"; do
  vbest=""
  vcs=""
  pbest=""
  pcs=""
  for ((k = 0; k < K; k++)); do
    line="$(taskset -c "$CPU" "$BIN" run "$MICRO/$name.phg")"
    ns="$(printf '%s' "$line" | cut -f2)"
    vcs="$(printf '%s' "$line" | cut -f3)"
    if [[ -z "$vbest" || "$ns" -lt "$vbest" ]]; then vbest="$ns"; fi
    # shellcheck disable=SC2086 # JIT_FLAGS is a deliberate word-split flag list
    pline="$(docker exec "$CONTAINER" php $JIT_FLAGS "/w/$name.php" 2>/dev/null)"
    pns="$(printf '%s' "$pline" | cut -f2)"
    pcs="$(printf '%s' "$pline" | cut -f3)"
    if [[ -z "$pbest" || "$pns" -lt "$pbest" ]]; then pbest="$pns"; fi
  done
  vm_ns[$name]="$vbest"
  vm_sum[$name]="$vcs"
  php_ns[$name]="$pbest"
  php_sum[$name]="$pcs"
done
cleanup_container
trap - EXIT

# Phase 3 — join, output-identity gate, report.
if [[ "$JSON" == 1 ]]; then
  printf '['
  first=1
  for name in "${features[@]}"; do
    v="${vm_ns[$name]:-0}"
    p="${php_ns[$name]:-0}"
    vs="${vm_sum[$name]:-x}"
    ps="${php_sum[$name]:-y}"
    ok=$([[ "$vs" == "$ps" ]] && echo true || echo false)
    ratio="$(awk -v v="$v" -v p="$p" 'BEGIN{if(v>0)printf "%.3f",p/v; else print 0}')"
    [[ $first == 1 ]] || printf ','
    first=0
    printf '{"feature":"%s","vm_ns":%s,"php_ns":%s,"ratio":%s,"identical":%s}' "$name" "$v" "$p" "$ratio" "$ok"
  done
  printf ']\n'
  exit 0
fi

printf '%-16s %12s %12s %9s  %s\n' feature "VM ns" "php+JIT ns" ratio verdict
printf '%-16s %12s %12s %9s  %s\n' "----" "----" "----" "----" "----"
for name in "${features[@]}"; do
  v="${vm_ns[$name]:-?}"
  p="${php_ns[$name]:-?}"
  vs="${vm_sum[$name]:-x}"
  ps="${php_sum[$name]:-y}"
  if [[ "$vs" != "$ps" ]]; then
    printf '%-16s %12s %12s %9s  CHECKSUM MISMATCH (vm=%s php=%s)\n' "$name" "$v" "$p" "-" "$vs" "$ps"
    continue
  fi
  ratio="$(awk -v v="$v" -v p="$p" 'BEGIN{if(v>0)printf "%.2f",p/v; else print "inf"}')"
  verdict="$(awk -v v="$v" -v p="$p" 'BEGIN{print (v<p)?"WIN":"LOSS"}')"
  printf '%-16s %12s %12s %8sx  %s\n' "$name" "$v" "$p" "$ratio" "$verdict"
done
