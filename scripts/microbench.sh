#!/usr/bin/env bash
# microbench.sh — per-feature phorj-VM vs release-PHP+JIT (ns/op), the G-8 measurement harness.
#
# For each feature `bench/micro/<name>.phg` (with a hand-authored idiomatic `bench/micro/<name>.php`):
#   - phorj: `phg run <name>.phg`  (the VM) — best-of-K on this host.
#   - PHP:   `<name>.php` under `docker run php:8.5-cli` with opcache+JIT (a REAL release php — the
#            local builds are all ZTS DEBUG, JIT off, so they are NOT a valid baseline).
# Each micro self-times (warmup call + timed call) and prints `name<TAB>ns_per_op<TAB>checksum`; the
# checksum defeats dead-code elimination AND gates output-identity (VM and PHP must agree before a
# timing is trusted). Ratio = php_ns / vm_ns; WIN means the VM is faster (the G-8 bar).
#
# Env: PHG_BIN (default target/release/phg), MICROBENCH_RUNS (K, default 3),
#      MICROBENCH_PHP_IMAGE (default php:8.5-cli). Flags: --json.
set -eEuo pipefail
export LC_ALL=C

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${PHG_BIN:-$ROOT/target/release/phg}"
MICRO="$ROOT/bench/micro"
K="${MICROBENCH_RUNS:-3}"
PHP_IMAGE="${MICROBENCH_PHP_IMAGE:-php:8.5-cli}"
JIT_FLAGS="-dopcache.enable_cli=1 -dopcache.jit_buffer_size=128M -dopcache.jit=tracing"
JSON=0
[[ "${1:-}" == "--json" ]] && JSON=1

command -v docker >/dev/null 2>&1 || { echo "microbench: docker required (real release-php baseline)" >&2; exit 2; }
[[ -x "$BIN" ]] || { echo "microbench: binary not found at $BIN — run: cargo build --release" >&2; exit 2; }

features=()
for f in "$MICRO"/*.phg; do
  name="$(basename "$f" .phg)"
  [[ -f "$MICRO/$name.php" ]] && features+=("$name")
done
[[ ${#features[@]} -gt 0 ]] || { echo "microbench: no paired *.phg/*.php micros in $MICRO" >&2; exit 2; }

# Phase 1 — phorj VM, best-of-K per feature (on this host).
declare -A vm_ns vm_sum
for name in "${features[@]}"; do
  best=""; cs=""
  for ((k = 0; k < K; k++)); do
    line="$("$BIN" run "$MICRO/$name.phg")"
    ns="$(printf '%s' "$line" | cut -f2)"
    cs="$(printf '%s' "$line" | cut -f3)"
    if [[ -z "$best" || "$ns" -lt "$best" ]]; then best="$ns"; fi
  done
  vm_ns[$name]="$best"
  vm_sum[$name]="$cs"
done

# Phase 2 — release PHP+JIT, best-of-K, ALL micros in ONE container launch (container start is slow).
php_out="$(docker run --rm -v "$MICRO:/w:ro" "$PHP_IMAGE" sh -c '
  K='"$K"'
  for f in /w/*.php; do
    name=$(basename "$f" .php)
    best=""; cs=""
    i=0
    while [ "$i" -lt "$K" ]; do
      line=$(php '"$JIT_FLAGS"' "$f")
      ns=$(printf "%s" "$line" | cut -f2)
      cs=$(printf "%s" "$line" | cut -f3)
      if [ -z "$best" ] || [ "$ns" -lt "$best" ]; then best="$ns"; fi
      i=$((i + 1))
    done
    printf "%s %s %s\n" "$name" "$best" "$cs"
  done
' 2>/dev/null)"

declare -A php_ns php_sum
while read -r name ns cs; do
  [[ -n "$name" ]] || continue
  php_ns[$name]="$ns"
  php_sum[$name]="$cs"
done <<<"$php_out"

# Phase 3 — join, output-identity gate, report.
if [[ "$JSON" == 1 ]]; then
  printf '['
  first=1
  for name in "${features[@]}"; do
    v="${vm_ns[$name]:-0}"; p="${php_ns[$name]:-0}"; vs="${vm_sum[$name]:-x}"; ps="${php_sum[$name]:-y}"
    ok=$([[ "$vs" == "$ps" ]] && echo true || echo false)
    ratio="$(awk -v v="$v" -v p="$p" 'BEGIN{if(v>0)printf "%.3f",p/v; else print 0}')"
    [[ $first == 1 ]] || printf ','
    first=0
    printf '{"feature":"%s","vm_ns":%s,"php_ns":%s,"ratio":%s,"identical":%s}' "$name" "$v" "$p" "$ratio" "$ok"
  done
  printf ']\n'
  exit 0
fi

printf '%-16s %12s %12s %9s  %s\n' feature "VM ns/op" "php+JIT" ratio verdict
printf '%-16s %12s %12s %9s  %s\n' "----" "----" "----" "----" "----"
for name in "${features[@]}"; do
  v="${vm_ns[$name]:-?}"; p="${php_ns[$name]:-?}"; vs="${vm_sum[$name]:-x}"; ps="${php_sum[$name]:-y}"
  if [[ "$vs" != "$ps" ]]; then
    printf '%-16s %12s %12s %9s  CHECKSUM MISMATCH (vm=%s php=%s)\n' "$name" "$v" "$p" "-" "$vs" "$ps"
    continue
  fi
  ratio="$(awk -v v="$v" -v p="$p" 'BEGIN{if(v>0)printf "%.2f",p/v; else print "inf"}')"
  verdict="$(awk -v v="$v" -v p="$p" 'BEGIN{print (v<p)?"WIN":"LOSS"}')"
  printf '%-16s %12s %12s %8sx  %s\n' "$name" "$v" "$p" "$ratio" "$verdict"
done
