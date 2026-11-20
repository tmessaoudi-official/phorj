#!/usr/bin/env bash
# Infra syntax gate — produces the Coverage evidence row that global Rule 7 REQUIRES for any change
# to a Dockerfile, compose file, Makefile target, shell script or YAML/JSON config (DEC-388).
#
# WHY THIS IS NATIVE AND NOT THE BUNDLE'S /validate-infra SKILL: that skill is 212 lines built around
# `docker compose config`, hadolint and yamllint. Measured in this repo 2026-07-27: 0 Dockerfiles,
# 0 compose files, and yamllint/shellcheck/hadolint ALL absent from PATH — so four of its six steps
# would degrade to skips. What is actually here is 9 tracked shell scripts, 2 git hooks and 4 GitHub
# workflows. So: ~100 lines that check exactly that, and — the part a skill cannot do — WIRED INTO
# pre-push, which makes the evidence mechanical instead of something a session has to remember.
#
# Read-only. Exits non-zero if any file fails, so it can gate a push.
#
#   scripts/validate-infra.sh            # full report + Rule-7 Coverage table
#   scripts/validate-infra.sh --quiet    # one line unless something fails (hook mode)
set -uo pipefail

QUIET=0
for arg in "$@"; do
  case "$arg" in
    --quiet|-q) QUIET=1 ;;
    -h|--help)  sed -n '2,17p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) printf 'validate-infra: unknown argument %s\n' "$arg" >&2; exit 2 ;;
  esac
done

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT" || exit 2

say() { [[ $QUIET -eq 1 ]] || printf '%s\n' "$*"; }

FAILURES=0
SH_OK=0;   SH_N=0
YML_OK=0;  YML_N=0
JSON_OK=0; JSON_N=0
PIN_OK=0;  PIN_N=0
EXE_OK=0;  EXE_N=0
VER_OK=0;  VER_N=0
declare -a FAILED_LINES=()

record_fail() { FAILURES=$((FAILURES + 1)); FAILED_LINES+=("$1"); }

# ── Discover: tracked files + the git hooks (which are tracked under scripts/git-hooks) ────────
mapfile -t SH_FILES < <(git ls-files '*.sh' 2>/dev/null; ls -1 scripts/git-hooks/* 2>/dev/null)
mapfile -t YML_FILES < <(git ls-files '*.yml' '*.yaml' 2>/dev/null)
mapfile -t JSON_FILES < <(git ls-files '*.json' 2>/dev/null | grep -vE '^(playground/web/pkg|editors/.*/node_modules)' || true)

say "=== validate-infra (Rule 7 infra syntax gate) ==="
say ""

# ── Shell: bash -n on every one ────────────────────────────────────────────────────────────────
say "-- shell syntax (bash -n) --"
for f in "${SH_FILES[@]:-}"; do
  [[ -n "$f" && -f "$f" ]] || continue
  SH_N=$((SH_N + 1))
  if err=$(bash -n "$f" 2>&1); then
    SH_OK=$((SH_OK + 1)); say "  PASS $f"
  else
    say "  FAIL $f: $err"; record_fail "bash -n FAIL $f: $err"
  fi
done
[[ $SH_N -eq 0 ]] && say "  (no shell files found)"

# ── Executable bit, read from the INDEX (what a clone gets), not the disk ─────────────────────
# This repo sets core.fileMode=false, so staging a new file records it 100644 whatever its disk mode:
# scripts/dk landed that way (efa88579) while every hook and CI step runs `scripts/dk …` by name, so
# a fresh clone would have failed "Permission denied" and only this machine's disk +x hid it. A file
# run by its name is an extensionless file with a shebang (the git hooks, scripts/dk); a `.sh` file is
# run as `bash x.sh` and may stay 100644.
say ""
say "-- executable in the index (extensionless files with a shebang) --"
while IFS= read -r -d '' rec; do
  mode="${rec%% *}"; f="${rec#*$'\t'}"
  base="${f##*/}"
  [[ "$base" == *.* || ! -f "$f" ]] && continue
  [[ "$(head -c2 -- "$f" 2>/dev/null)" == '#!' ]] || continue
  EXE_N=$((EXE_N + 1))
  if [[ "$mode" == 100755 ]]; then
    EXE_OK=$((EXE_OK + 1)); say "  PASS $f"
  else
    say "  FAIL $f: tracked $mode, run by name (fix: git update-index --chmod=+x -- $f)"
    record_fail "exec FAIL $f: index mode $mode, needs 100755"
  fi
done < <(git ls-files -s -z 2>/dev/null)
[[ $EXE_N -eq 0 ]] && say "  (no extensionless shebang files found)"

# ── No hardcoded php-oracle pin in the gate scripts ────────────────────────────────────────────
# The oracle is resolved ONCE, by scripts/toolchain.env, which globs `php-8.5.*` newest-first and
# capability-checks bcmath. A literal `phpbrew/php/php-<x.y.z>/` anywhere else is a SECOND source of
# truth that goes stale the moment the stack bumps a patch version — and it fails WORSE than having
# no fallback at all: the gate then hands the test suite a path that does not exist, so the failure
# surfaces as an opaque `php required but not found` assert instead of toolchain.env's loud,
# actionable warning. That is not hypothetical: on 2026-08-20 a push failed exactly this way, with
# `pre-push` substituting a pinned `php-8.5.8` after the stack had moved to `php-8.5.9`. Third time
# this class has bitten the gate — hence a mechanical check rather than another careful comment.
# Comment lines are stripped first: toolchain.env's own root-cause narrative NAMES the stale
# versions in prose, and a check that red-fails on documentation fails for the wrong reason.
say ""
say "-- no hardcoded php-oracle pin (single source of truth: scripts/toolchain.env) --"
# Scope: EVERY tracked shell script and workflow, not just the two files that broke — a pin added
# tomorrow in scripts/perf-gate.sh or a CI workflow is the same defect, and a check that only looks
# where the bug already was cannot catch the next one. `grep -n` runs on the FILE (not on a filtered
# stream) so the reported line number is the real one; comment hits are dropped afterwards, because
# the root-cause narratives in toolchain.env legitimately NAME the stale versions.
# Known limitation, accepted: a path assembled from a variable (`php-$V/bin/php`) does not match.
# That is not a pin — it is a resolver — and matching it would flag toolchain.env's own glob.
mapfile -t PIN_FILES < <(printf '%s\n' "${SH_FILES[@]:-}" scripts/toolchain.env "${YML_FILES[@]:-}" | sort -u)
for f in "${PIN_FILES[@]:-}"; do
  [[ -n "$f" && -f "$f" && -r "$f" ]] || continue
  PIN_N=$((PIN_N + 1))
  hits=$(grep -nE 'phpbrew/php/php-[0-9]+\.[0-9]+\.[0-9]+' "$f" | grep -vE '^[0-9]+:[[:space:]]*#' || true)
  if [[ -z "$hits" ]]; then
    PIN_OK=$((PIN_OK + 1))
  else
    say "  FAIL $f: pinned php path — $(head -1 <<<"$hits")"
    record_fail "php-pin FAIL $f: $(head -1 <<<"$hits")"
  fi
done
say "  ${PIN_OK}/${PIN_N} clean"
# A scan that inspected nothing must never read as a pass — that is the shape the example-glob
# no-op wore for weeks. Zero files means the scan is broken (wrong cwd, renamed paths), not that
# the repo is clean.
if [[ $PIN_N -eq 0 ]]; then
  say "  FAIL: the pin scan inspected ZERO files — the check is dark, not clean"
  record_fail "php-pin FAIL: scan inspected 0 files"
fi

# ── Tool versions: docker/Dockerfile is the one place one is written ─────────────────────────
# A workflow that cannot run in the image restates a version as an env key of the same name (stub-registry.yml's
# ZIG_VERSION/ZIG_SHA256: it publishes, so it is never dispatched to try). Every such key must equal the ARG.
say ""
say "-- tool versions match docker/Dockerfile --"
if [[ -f docker/Dockerfile ]]; then
  while IFS='=' read -r name want; do
    for f in "${YML_FILES[@]:-}"; do
      # Workflows only: compose.yaml passes the same names as build-arg OVERRIDES (PHP_MINOR 8.6 for dev86).
      [[ -n "$f" && -f "$f" && "$f" == .github/workflows/* ]] || continue
      while IFS= read -r hit; do
        VER_N=$((VER_N + 1))
        got="$(sed -E 's/^[^:]*:[[:space:]]*//; s/^"//; s/"[[:space:]]*$//; s/[[:space:]]+$//' <<<"${hit#*:}")"
        if [[ "$got" == "$want" ]]; then
          VER_OK=$((VER_OK + 1)); say "  PASS $f $name"
        else
          say "  FAIL $f:${hit%%:*} $name is \"$got\", docker/Dockerfile says \"$want\""
          record_fail "version FAIL $f $name: $got != Dockerfile $want"
        fi
      done < <(grep -nE "^[[:space:]]*${name}:[[:space:]]" "$f" || true)
    done
  done < <(sed -nE 's/^ARG ([A-Z0-9_]+)=([^[:space:]]+)$/\1=\2/p' docker/Dockerfile)
  [[ $VER_N -eq 0 ]] && say "  (no workflow restates a Dockerfile version)"
else
  say "  (no docker/Dockerfile)"
fi

# ── YAML: parse with python3 (yamllint is absent here; a parse is the load-bearing check) ──────
say ""
say "-- YAML parse (python3 yaml.safe_load) --"
if command -v python3 >/dev/null 2>&1 && python3 -c 'import yaml' 2>/dev/null; then
  for f in "${YML_FILES[@]:-}"; do
    [[ -n "$f" && -f "$f" ]] || continue
    YML_N=$((YML_N + 1))
    if err=$(python3 -c 'import sys,yaml; list(yaml.safe_load_all(open(sys.argv[1])))' "$f" 2>&1); then
      YML_OK=$((YML_OK + 1)); say "  PASS $f"
    else
      say "  FAIL $f: $(head -2 <<<"$err" | tr '\n' ' ')"; record_fail "yaml FAIL $f"
    fi
  done
  [[ $YML_N -eq 0 ]] && say "  (no YAML files found)"
else
  say "  SKIPPED — python3 with pyyaml unavailable (LOUD SKIP: YAML was NOT validated)"
fi

# ── JSON: parse. settings.json breaking silently is the real-world failure this catches ────────
say ""
say "-- JSON parse --"
if command -v python3 >/dev/null 2>&1; then
  for f in "${JSON_FILES[@]:-}"; do
    [[ -n "$f" && -f "$f" ]] || continue
    JSON_N=$((JSON_N + 1))
    if err=$(python3 -c 'import sys,json; json.load(open(sys.argv[1]))' "$f" 2>&1); then
      JSON_OK=$((JSON_OK + 1)); say "  PASS $f"
    else
      say "  FAIL $f: $(tail -1 <<<"$err")"; record_fail "json FAIL $f"
    fi
  done
  [[ $JSON_N -eq 0 ]] && say "  (no JSON files found)"
else
  say "  SKIPPED — python3 unavailable (LOUD SKIP: JSON was NOT validated)"
fi

# ── Optional tools: absent means SKIPPED-OUT-LOUD, never a silent pass ─────────────────────────
say ""
say "-- optional linters --"
LINT_PRESENT=(); LINT_ABSENT=()
for tool in shellcheck yamllint hadolint; do
  if command -v "$tool" >/dev/null 2>&1; then
    LINT_PRESENT+=("$tool"); say "  $tool present — but NOT RUN by this gate (PostToolUse hooks lint on write)"
  else
    LINT_ABSENT+=("$tool"); say "  SKIPPED: $tool absent from PATH — its checks did NOT run"
  fi
done

# ── The Rule-7 Coverage evidence row ───────────────────────────────────────────────────────────
say ""
say "## Coverage Evidence (Rule 6 / Rule 7 — infra TDD)"
say ""
say "| Check | Result |"
say "|---|---|"
say "| bash -n | ${SH_OK}/${SH_N} pass |"
say "| no pinned php oracle | ${PIN_OK}/${PIN_N} clean |"
say "| executable in the index | ${EXE_OK}/${EXE_N} pass |"
say "| tool versions match docker/Dockerfile | ${VER_OK}/${VER_N} pass |"
say "| YAML parse | ${YML_OK}/${YML_N} pass |"
say "| JSON parse | ${JSON_OK}/${JSON_N} pass |"
# Report what is TRUE, computed — this row was a hardcoded "SKIPPED — absent from PATH" literal
# while all three tools were installed, i.e. the Rule-6 Coverage evidence this script exists to
# produce contained a false row (panel finding, 2026-08-20). Present-but-not-run is still not a
# check; it is stated as such rather than dressed up as one.
say "| shellcheck / yamllint / hadolint | present: ${LINT_PRESENT[*]:-none} (NOT RUN here) · absent: ${LINT_ABSENT[*]:-none} |"
say ""

if [[ $FAILURES -gt 0 ]]; then
  printf 'validate-infra: %d FAILURE(S)\n' "$FAILURES" >&2
  for l in "${FAILED_LINES[@]}"; do printf '  %s\n' "$l" >&2; done
  exit 1
fi

if [[ $QUIET -eq 1 ]]; then
  # PIN_N is in the quiet line on purpose: --quiet is the mode pre-push runs, and without the count
  # the hook cannot tell "3 files scanned" from "0 files scanned" — the dark-check state.
  printf 'validate-infra: OK (%d sh, %d yaml, %d json, %d php-pin)\n' "$SH_N" "$YML_N" "$JSON_N" "$PIN_N"
else
  say "validate-infra: OK — ${SH_N} shell, ${YML_N} YAML, ${JSON_N} JSON, ${PIN_N} php-pin checked."
fi
exit 0
