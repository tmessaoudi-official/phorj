#!/usr/bin/env bash
# Contract tests for phorj's cloud readiness (/cloud-ready, DEC-577) — run by scripts/git-hooks/pre-push next to
# the other gates' own suites. A claude.ai cloud session works on a `claude/...`, `cloud/...` or `ccr-...` branch
# and never pushes master; the developer lands that branch on the PC. These checks keep the repo-side half of that
# contract from drifting:
#   - CI runs the tests on every cloud branch pattern (otherwise a cloud branch gets NO gate: the PHP leg is CI-only
#     until the toolchain hook installs PHP);
#   - the two jobs that publish releases and move tags run in the `release` environment, whose deployment policy
#     (a GitHub setting, master + v* tags) refuses them from a cloud branch. A job `if:` is not that guard: a pushed
#     branch runs the workflow file as that branch has it.
# Reads tracked files only; writes nothing outside its own temp dir; no network.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${CLOUD_READY_ROOT:-$(git -C "$HERE" rev-parse --show-toplevel)}"
PASS=0
FAIL=0
ok()  { printf '  ok   — %s\n' "$1"; PASS=$((PASS + 1)); }
bad() { printf '  FAIL — %s\n' "$1"; FAIL=$((FAIL + 1)); }

echo "cloud-ready contract — $ROOT"

# ── 1. CI triggers and the publish jobs' environment (CR2) ──────────────────────────────────────────────────
# One python pass over the workflows; it prints `ok|…` / `FAIL|…` lines and nothing else.
while IFS='|' read -r verdict msg; do
  case "$verdict" in
    ok) ok "$msg" ;;
    FAIL) bad "$msg" ;;
    *) bad "unexpected checker output: $verdict|$msg" ;;
  esac
done < <(python3 - "$ROOT" <<'PY' 2>&1
import sys, os, yaml
root = sys.argv[1]
def load(p):
    with open(os.path.join(root, p)) as f:
        d = yaml.safe_load(f)
    # PyYAML reads the bare key `on` as the boolean True
    return d, (d.get("on") if "on" in d else d.get(True)) or {}
try:
    ci, on = load(".github/workflows/ci.yml")
    branches = ((on.get("push") or {}).get("branches")) or []
    for pat in ("master", "ccr-*", "claude/**", "cloud/**"):
        print(("ok" if pat in branches else "FAIL") + f"|ci.yml push triggers include '{pat}' (have {branches})")
    for wf, job in ((".github/workflows/release.yml", "publish-dev"),
                    (".github/workflows/stub-registry.yml", "primary-and-publish")):
        d, _ = load(wf)
        j = (d.get("jobs") or {}).get(job)
        if j is None:
            print(f"FAIL|{wf}: job '{job}' not found")
            continue
        env = j.get("environment")
        name = env.get("name") if isinstance(env, dict) else env
        print(("ok" if name == "release" else "FAIL") + f"|{wf} job '{job}' runs in environment 'release' (has {name!r})")
except Exception as e:
    print(f"FAIL|workflow check crashed: {type(e).__name__}: {e}")
PY
)

# ── 2. Wiring: a suite no gate runs goes dark unnoticed (pre-push records three instances) ──────────────────
if grep -qE '^bash "\$_gate_root/scripts/test-cloud-ready\.sh"$' "$ROOT/scripts/git-hooks/pre-push"; then
  ok "pre-push runs this suite"
else
  bad "pre-push does not run scripts/test-cloud-ready.sh"
fi

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 && $PASS -gt 0 ]]
