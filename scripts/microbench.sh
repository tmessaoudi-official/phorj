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

# PHP baseline source: docker (default, the CI/dev box) OR a LOCAL php binary. Set MICROBENCH_PHP_BIN
# to a real release php WITH opcache/JIT to run without docker (e.g. a container that has no docker or
# where image pulls are blocked); MICROBENCH_PHP_OPCACHE points at its opcache.so when JIT ships as a
# shared zend_extension (a from-source CLI build). The local path pins the same core as the phg side.
# Engine/JIT matrix knobs: MICROBENCH_PHG_ARGS = extra `phg run` args (e.g. `--no-jit` for the
# plain VM, `--tree-walker` for the reference interpreter) and MICROBENCH_PHP_JIT=0 = drop the
# opcache/JIT flags (a plain interpreted php) — together they produce the interpreter-vs-
# interpreter matrix. Defaults preserve the G-8 harness (VM+JIT vs php+JIT) exactly.
PHG_ARGS="${MICROBENCH_PHG_ARGS:-}"
[[ "${MICROBENCH_PHP_JIT:-1}" == "0" ]] && JIT_FLAGS=""

LOCAL_PHP="${MICROBENCH_PHP_BIN:-}"
# MICROBENCH_DOCKER_BOTH=1 (DEC-333e, dev's fairness ruling): run BOTH legs inside the SAME
# php docker container — `docker cp` the phg binary in and `docker exec` both, so kernel,
# cgroup CPU set, libc, and scheduler pressure are identical for the two legs (the close-margin
# protocol; host-vs-container asymmetry measured up to 2.7x swings on the php leg alone).
# ⚠ Shipped UNTESTED in the authoring container (docker blocked there) — validate with one run.
# Requires docker mode (mutually exclusive with MICROBENCH_PHP_BIN) and a glibc-compatible phg
# (the debian-based php image runs a host glibc build; a musl/static build always works).
DOCKER_BOTH="${MICROBENCH_DOCKER_BOTH:-0}"
if [[ "$DOCKER_BOTH" == "1" && -n "$LOCAL_PHP" ]]; then
  echo "microbench: MICROBENCH_DOCKER_BOTH=1 needs docker mode — unset MICROBENCH_PHP_BIN" >&2
  exit 2
fi
OPCACHE_ARG=""
[[ -n "$LOCAL_PHP" && -n "${MICROBENCH_PHP_OPCACHE:-}" ]] && OPCACHE_ARG="-dzend_extension=${MICROBENCH_PHP_OPCACHE}"
if [[ -z "$LOCAL_PHP" ]]; then
  command -v docker >/dev/null 2>&1 || {
    echo "microbench: docker required (real release-php baseline) — or set MICROBENCH_PHP_BIN to a local php+JIT" >&2
    exit 2
  }
elif [[ ! -x "$LOCAL_PHP" ]]; then
  echo "microbench: MICROBENCH_PHP_BIN='$LOCAL_PHP' is not executable" >&2
  exit 2
fi
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
CONTAINER=""
cleanup_container() { [[ -n "$CONTAINER" ]] && docker rm -f "$CONTAINER" >/dev/null 2>&1 || true; }
if [[ -z "$LOCAL_PHP" ]]; then
  CONTAINER="$(docker run -d --rm --cpuset-cpus="$CPU" -v "$MICRO:/w:ro" "$PHP_IMAGE" sleep infinity)"
  trap cleanup_container EXIT
  # DOCKER_BOTH: the phg leg runs inside the same pinned container as the php leg.
  [[ "$DOCKER_BOTH" == "1" ]] && docker cp "$BIN" "$CONTAINER:/phg"
fi

declare -A vm_ns vm_sum php_ns php_sum
for name in "${features[@]}"; do
  vbest=""
  vcs=""
  pbest=""
  pcs=""
  for ((k = 0; k < K; k++)); do
    # shellcheck disable=SC2086 # PHG_ARGS is a deliberate word-split flag list
    if [[ "$DOCKER_BOTH" == "1" ]]; then
      line="$(docker exec "$CONTAINER" /phg run $PHG_ARGS "/w/$name.phg")"
    else
      line="$(taskset -c "$CPU" "$BIN" run $PHG_ARGS "$MICRO/$name.phg")"
    fi
    ns="$(printf '%s' "$line" | cut -f2)"
    vcs="$(printf '%s' "$line" | cut -f3)"
    if [[ -z "$vbest" || "$ns" -lt "$vbest" ]]; then vbest="$ns"; fi
    # shellcheck disable=SC2086 # JIT_FLAGS / OPCACHE_ARG are deliberate word-split flag lists
    if [[ -n "$LOCAL_PHP" ]]; then
      pline="$(taskset -c "$CPU" "$LOCAL_PHP" $OPCACHE_ARG $JIT_FLAGS "$MICRO/$name.php" 2>/dev/null)"
    else
      pline="$(docker exec "$CONTAINER" php $JIT_FLAGS "/w/$name.php" 2>/dev/null)"
    fi
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
