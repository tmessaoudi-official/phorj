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
for f in scripts/toolchain.env scripts/git-hooks/*; do
  [[ -n "$f" && -f "$f" ]] || continue
  PIN_N=$((PIN_N + 1))
  hits=$(grep -vE '^[[:space:]]*#' "$f" | grep -nE 'phpbrew/php/php-[0-9]+\.[0-9]+\.[0-9]+' || true)
  if [[ -z "$hits" ]]; then
    PIN_OK=$((PIN_OK + 1)); say "  PASS $f"
  else
    say "  FAIL $f: pinned php path — $(head -1 <<<"$hits")"
    record_fail "php-pin FAIL $f: $(head -1 <<<"$hits")"
  fi
done

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
for tool in shellcheck yamllint hadolint; do
  if command -v "$tool" >/dev/null 2>&1; then
    say "  $tool present"
  else
    say "  SKIPPED: $tool absent from PATH — its checks did NOT run"
  fi
done

# ── The Rule-7 Coverage evidence row ───────────────────────────────────────────────────────────
say ""
say "## Coverage Evidence (Rule 6 / Rule 7 — infra TDD)"
say ""
say "| Check | Result |"
say "|---|---|"
say "| bash -n | ${SH_OK}/${SH_N} pass |"
say "| no pinned php oracle | ${PIN_OK}/${PIN_N} pass |"
say "| YAML parse | ${YML_OK}/${YML_N} pass |"
say "| JSON parse | ${JSON_OK}/${JSON_N} pass |"
say "| shellcheck / yamllint / hadolint | SKIPPED — absent from PATH |"
say ""

if [[ $FAILURES -gt 0 ]]; then
  printf 'validate-infra: %d FAILURE(S)\n' "$FAILURES" >&2
  for l in "${FAILED_LINES[@]}"; do printf '  %s\n' "$l" >&2; done
  exit 1
fi

if [[ $QUIET -eq 1 ]]; then
  printf 'validate-infra: OK (%d sh, %d yaml, %d json)\n' "$SH_N" "$YML_N" "$JSON_N"
else
  say "validate-infra: OK — ${SH_N} shell, ${YML_N} YAML, ${JSON_N} JSON checked."
fi
exit 0
