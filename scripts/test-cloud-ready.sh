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

# ── 2. CLAUDE.md: PC-only git rules are scoped, the cloud paragraph names the contract (CR1) ────────────────
# A rule written for the PC ("push with plain git push", "commits direct to master") reads as binding in a cloud
# session, where the kit's push guard refuses a bare `git push` and master is never pushed. Bullets are joined with
# their indented continuation lines, so a rule split across lines is judged whole.
while IFS='|' read -r verdict msg; do
  case "$verdict" in ok) ok "$msg" ;; FAIL) bad "$msg" ;; *) bad "unexpected checker output: $verdict|$msg" ;; esac
done < <(python3 - "$ROOT/CLAUDE.md" <<'PY' 2>&1
import sys, re
try:
    lines = open(sys.argv[1]).read().split("\n")
    units, cur = [], None   # a unit = a bullet with its continuation lines, or a prose paragraph
    for l in lines:
        if l.startswith("- ") or not l.strip() or l.startswith("#"):
            if cur: units.append(cur)
            cur = l if l.strip() and not l.startswith("#") else None
        else:
            cur = (cur + " " + l.strip()) if cur else l
    if cur: units.append(cur)
    def check(desc, pred, needle):
        hits = [u for u in units if needle in u]
        if not hits:
            print(f"FAIL|{desc}: no unit contains {needle!r}")
        for u in hits:
            print(("ok" if pred(u) else "FAIL") + f"|{desc}: {u[:90]!r}")
    # the scope must sit NEXT to the rule (120 chars either side), not anywhere in a long paragraph
    def near(needle):
        def pred(u):
            i = u.index(needle)
            return "developer's machine" in u[max(0, i - 120): i + len(needle) + 120]
        return pred
    check("the plain-push rule is scoped to the developer's machine", near("plain `git push`"), "plain `git push`")
    check("the direct-to-master line is scoped to the developer's machine", near("commits direct to"), "commits direct to")
    cloud = [u for u in units if "CLOUD SESSIONS NEVER TOUCH" in u]
    print(("ok" if len(cloud) == 1 else "FAIL") + f"|exactly one cloud-session paragraph ({len(cloud)})")
    for need in ("claude-code-setup", ".claude/cloud-setup.sh", "claude/**", "cloud/**", "pre-push"):
        print(("ok" if cloud and need in cloud[0] else "FAIL") + f"|the cloud paragraph names {need!r}")
except Exception as e:
    print(f"FAIL|CLAUDE.md check crashed: {type(e).__name__}: {e}")
PY
)

# ── 3. Wiring: a suite no gate runs goes dark unnoticed (pre-push records three instances) ──────────────────
for suite in test-cloud-ready test-cloud-setup; do
  if grep -qE "^bash \"\\\$_gate_root/scripts/$suite\\.sh\"\$" "$ROOT/scripts/git-hooks/pre-push"; then
    ok "pre-push runs scripts/$suite.sh"
  else
    bad "pre-push does not run scripts/$suite.sh"
  fi
done

# ── 4. The toolchain hook is TRACKED executable (CR3) ───────────────────────────────────────────────────────
# The kit runs <workspace>/.claude/cloud-setup.sh only when it is an executable regular file on a FRESH clone, so the
# mode git records is what counts — and core.fileMode=false on the PC means a local chmod never reaches the index.
mode="$(git -C "$ROOT" ls-files -s .claude/cloud-setup.sh | cut -c1-6)"
[[ "$mode" == 100755 ]] && ok ".claude/cloud-setup.sh is tracked as 100755" \
                        || bad ".claude/cloud-setup.sh is tracked as '${mode:-untracked}', not 100755 (git update-index --chmod=+x)"

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 && $PASS -gt 0 ]]
