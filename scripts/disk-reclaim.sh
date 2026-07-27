#!/usr/bin/env bash
# Reclaim disk in the ephemeral phorj container — DEC-388 (the /cleanup insight from the bundle audit,
# retargeted at the problem this repo actually has).
#
# WHY: build artefacts are the disk crisis here, not Claude state. Measured 2026-07-27: `target/` was
# 22 GB with the root filesystem 88% full and 4.8 GB free, and SLICE-STATE records "No space left on
# device" having surfaced as SPURIOUS BUILD REDS — a red test run that is really a full disk is the
# most expensive failure mode we have, because it looks like a code regression.
#
# SAFETY, in order of importance:
#   1. DRY RUN IS THE DEFAULT. Nothing is deleted without --yes.
#   2. It refuses to run anywhere that is not the phorj repo root (marker files, checked below).
#   3. Every candidate path is confined to $REPO/target or $REPO/var/claude, verified after
#      resolution — not merely by how the string was written.
#   4. `var/phorj-app` is NEVER touched at any tier. It is the developer's live real-world comparison
#      app (DEC-259, Invariant 18) and is explicitly never-propose-deleting.
#   5. Nothing git-tracked is ever a candidate: every tier lives under gitignored paths.
#
# Everything it removes is rebuildable by `cargo build`. Usage:
#   scripts/disk-reclaim.sh                    # dry run, cache tier
#   scripts/disk-reclaim.sh --yes              # remove incremental caches (safest, keeps binaries)
#   scripts/disk-reclaim.sh --tier=debug --yes # + all of target/debug
#   scripts/disk-reclaim.sh --tier=all --yes   # + target/release, and prune old var/claude reports
set -uo pipefail

TIER=cache
APPLY=0
for arg in "$@"; do
  case "$arg" in
    --yes|-y)      APPLY=1 ;;
    --tier=*)      TIER="${arg#--tier=}" ;;
    -h|--help)
      sed -n '2,26p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *)
      printf 'disk-reclaim: unknown argument %s (see --help)\n' "$arg" >&2
      exit 2 ;;
  esac
done

case "$TIER" in
  cache|debug|all) ;;
  *) printf 'disk-reclaim: unknown --tier=%s (want: cache | debug | all)\n' "$TIER" >&2; exit 2 ;;
esac

# ── Guard 2: are we actually in the phorj repo? ────────────────────────────────────────────────
REPO="$(pwd)"
if ! git -C "$REPO" rev-parse --show-toplevel >/dev/null 2>&1; then
  printf 'disk-reclaim: %s is not a git work tree — refusing to delete anything.\n' "$REPO" >&2
  exit 1
fi
REPO="$(git -C "$REPO" rev-parse --show-toplevel)"
if [[ ! -f "$REPO/Cargo.toml" || ! -f "$REPO/docs/plans/SLICE-STATE.md" ]]; then
  printf 'disk-reclaim: %s does not look like the phorj repo (want Cargo.toml + docs/plans/SLICE-STATE.md)\n' "$REPO" >&2
  printf '             refusing to delete anything.\n' >&2
  exit 1
fi

NEVER="$REPO/var/phorj-app"          # Guard 4 — DEC-259

human() { du -sh "$1" 2>/dev/null | cut -f1; }
bytes() { du -sk "$1" 2>/dev/null | cut -f1; }   # KiB, portable enough for a report

# ── Build the candidate list for the requested tier ────────────────────────────────────────────
CANDIDATES=()
add() { [[ -e "$1" ]] && CANDIDATES+=("$1"); }

# cache tier: pure rebuildable caches, binaries preserved
while IFS= read -r d; do add "$d"; done < <(find "$REPO/target" -maxdepth 2 -type d -name incremental 2>/dev/null)
add "$REPO/target/tmp"
add "$REPO/target/.rustc_info.json"
add "$REPO/target/package"
add "$REPO/target/doc"

if [[ "$TIER" == "debug" || "$TIER" == "all" ]]; then
  add "$REPO/target/debug"
fi
if [[ "$TIER" == "all" ]]; then
  add "$REPO/target/release"
  # Prune handoff/report archives but keep the newest of each — they are the compaction safety net.
  for sub in handoff sleuth inspections reports forge; do
    d="$REPO/var/claude/$sub"
    [[ -d "$d" ]] || continue
    while IFS= read -r f; do add "$f"; done < <(ls -1t "$d"/*.md 2>/dev/null | tail -n +3)
  done
fi

# ── Guard 3 + 4: confine every candidate, and drop anything under the protected app ────────────
SAFE=()
for c in "${CANDIDATES[@]:-}"; do
  [[ -n "$c" ]] || continue
  real="$(cd "$(dirname "$c")" 2>/dev/null && pwd)/$(basename "$c")" || continue
  case "$real" in
    "$NEVER"|"$NEVER"/*)
      printf 'disk-reclaim: REFUSING %s — var/phorj-app is never deletable (DEC-259)\n' "$real" >&2
      continue ;;
    "$REPO"/target|"$REPO"/target/*|"$REPO"/var/claude/*)
      SAFE+=("$real") ;;
    *)
      printf 'disk-reclaim: REFUSING %s — outside target/ and var/claude/\n' "$real" >&2
      continue ;;
  esac
done

# ── Report ─────────────────────────────────────────────────────────────────────────────────────
printf '=== disk-reclaim (tier=%s, %s) ===\n\n' "$TIER" \
  "$([[ $APPLY -eq 1 ]] && echo 'APPLYING' || echo 'DRY RUN — nothing will be deleted')"

printf 'Filesystem before:\n'
df -h "$REPO" | tail -1 | awk '{printf "  %s used, %s avail (%s full)\n", $3, $4, $5}'
printf '  target/ total: %s\n\n' "$(human "$REPO/target")"

if [[ ${#SAFE[@]} -eq 0 ]]; then
  printf 'Nothing to reclaim at tier=%s.\n' "$TIER"
  exit 0
fi

TOTAL_KB=0
printf 'Candidates:\n'
for p in "${SAFE[@]}"; do
  kb=$(bytes "$p"); kb=${kb:-0}
  TOTAL_KB=$((TOTAL_KB + kb))
  printf '  %-8s %s\n' "$(human "$p")" "${p#"$REPO"/}"
done
printf '\nWould free approximately: %s MiB\n' "$((TOTAL_KB / 1024))"

if [[ $APPLY -eq 0 ]]; then
  printf '\nDRY RUN — nothing was deleted. Re-run with --yes to apply.\n'
  printf 'Everything above is rebuildable with `cargo build`.\n'
  exit 0
fi

# ── Apply ──────────────────────────────────────────────────────────────────────────────────────
printf '\nRemoving:\n'
for p in "${SAFE[@]}"; do
  if rm -rf "$p"; then
    printf '  removed %s\n' "${p#"$REPO"/}"
  else
    printf '  FAILED  %s\n' "${p#"$REPO"/}" >&2
  fi
done

printf '\nFilesystem after:\n'
df -h "$REPO" | tail -1 | awk '{printf "  %s used, %s avail (%s full)\n", $3, $4, $5}'
printf '  target/ total: %s\n' "$(human "$REPO/target" || echo 0)"
printf '\nDone. Next `cargo build` will repopulate what it needs.\n'
exit 0
