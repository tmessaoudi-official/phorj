#!/usr/bin/env bash
# Behaviour tests for tools/cloud-kit (plan docs/plans/2026-10-26-cloud-kit.plan.md, DEC-570).
# Synthetic fixtures only — no real config or memory ever enters the repo. Every case runs against a fake
# HOME in a temp dir, so the machine running the tests is never touched. Needs: bash, python3, tar, age.
#     bash tools/cloud-kit/test-cloud-kit.sh
set -euo pipefail

KIT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
command -v age >/dev/null && command -v age-keygen >/dev/null \
  || { echo "test-cloud-kit: SKIP — age/age-keygen not installed (nothing was tested)" >&2; exit 0; }

T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT
pass=0; fail=0
ok()  { pass=$((pass + 1)); echo "  ok   $1"; }
bad() { fail=$((fail + 1)); echo "  FAIL $1" >&2; }

# ── fixtures ─────────────────────────────────────────────────────────────────────────────────────────
B="$T/bundle/claude-setup-global-20990101-000000/global"
mkdir -p "$B"/{hooks,bin/lib,skills/loop,skills/next,refs,rules,.claude/rules}
printf '# Global Reasoning Framework\nbody\n@RTK.md\n@RTK-local.md\n@THINKING.md\n' >"$B/CLAUDE.md"
echo thinking >"$B/THINKING.md"; echo blast >"$B/BLAST-RADIUS.md"
echo loop >"$B/skills/loop/SKILL.md"; echo next >"$B/skills/next/SKILL.md"
printf '#!/usr/bin/env bash\nexit 0\n' >"$B/hooks/session-start-banner.sh"
printf '#!/usr/bin/env bash\nexit 0\n' >"$B/hooks/ask-human-question-guard.sh"
printf '#!/usr/bin/env bash\nexit 0\n' >"$B/hooks/precompact-handoff.sh"
printf "HOME = R(os.environ.get('HOME'))\nHOST_DIRS = ['/etc', '/root', '/opt']\n" >"$B/hooks/ask-bash-firewall.sh"
printf '#!/usr/bin/env bash\necho fake-gates-bypass "$@"\n' >"$B/bin/gates-bypass.sh"
cat >"$B/settings.json.template" <<'JSON'
{"defaultMode":"plan","model":"opus","permissions":{"deny":["Bash(sudo)"]},
 "env":{"CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS":"1"},"statusLine":{"type":"command","command":"x"},
 "hooks":{
  "SessionStart":[{"matcher":"","hooks":[{"type":"command","command":"\"$HOME\"/.claude/hooks/session-remember/start.sh"},
                                          {"type":"command","command":"\"$HOME\"/.claude/hooks/session-start-banner.sh"}]}],
  "Stop":[{"matcher":"","hooks":[{"type":"command","command":"bash ~/.claude/hooks/ask-human-question-guard.sh"}]}],
  "PreCompact":[{"matcher":"","hooks":[{"type":"command","command":"\"$HOME\"/.claude/hooks/precompact-handoff.sh"}]}],
  "PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"rtk hook claude"}]},
                {"matcher":"Bash","hooks":[{"type":"command","command":"bash ~/.claude/hooks/ask-bash-firewall.sh"}]}]}}
JSON
(cd "$T/bundle" && tar -czf "$T/global.tar.gz" .)

M="$T/mem"; mkdir -p "$M/entries/phorj/memory" "$M/entries/phorj/expertise"
echo '{"version":1,"entries":[{"key":"phorj","name":"phorj","slug":"-stack-projects-phorj"}]}' >"$M/map.json"
echo '# index' >"$M/entries/phorj/memory/MEMORY.md"; echo 'a fact' >"$M/entries/phorj/memory/fact.md"
echo 'advisor() at every gate' >"$M/entries/phorj/certification-schedule"; echo repo >"$M/entries/phorj/plan-location"
echo '# expertise' >"$M/entries/phorj/expertise/index.md"
(cd "$M" && tar -czf "$T/memories.tar.gz" .)

age-keygen -o "$T/key.txt" 2>/dev/null
RECIP="$(age-keygen -y "$T/key.txt")"
KEY="$(grep '^AGE-SECRET-KEY-' "$T/key.txt")"
mkdir -p "$T/ccr"
SLUG="$(printf '%s' "$T/projects/phorj" | tr / -)"   # the slug Claude Code derives for that project path

fresh_home() { rm -rf "$T/home"; mkdir -p "$T/home/.claude"; echo '{"advisorModel":"fable"}' >"$T/home/.claude/settings.json"; }
snapshot() { (cd "$T/home" && find . -type f -exec sha256sum {} + | sort); }
# A cloud-like environment: everything the installer checks, pointed at the fakes.
cloud() { env -i PATH="$PATH" HOME="$T/home" CLAUDE_CODE_REMOTE=true CLOUD_KIT_CCR_DIR="$T/ccr" \
            PHORJ_CLOUD_KIT_KEY="$KEY" CLOUD_KIT_PROJECTS_ROOT="$T/projects" "$@"; }

echo "test-cloud-kit"

# ── 1. refusal: a PC-like environment (no cloud markers at all) touches nothing ──────────────────────
fresh_home; before="$(snapshot)"
if env -i PATH="$PATH" HOME="$T/home" bash "$KIT/install.sh" --from-plain "$T/global.tar.gz" "$T/memories.tar.gz" >"$T/out" 2>&1; then
  bad "PC-like env: install succeeded"
else
  rc=$?; [[ $rc -eq 3 ]] && grep -q 'not a claude.ai cloud session' "$T/out" && [[ "$(snapshot)" == "$before" ]] \
    && ok "PC-like env: refused (exit 3), HOME untouched" || bad "PC-like env: wrong refusal (rc=$rc) or HOME changed"
fi

# ── 2. refusal: CLAUDE_CODE_REMOTE alone is not enough (no proxy dir) ────────────────────────────────
fresh_home; before="$(snapshot)"
if env -i PATH="$PATH" HOME="$T/home" CLAUDE_CODE_REMOTE=true CLOUD_KIT_CCR_DIR="$T/no-such-dir" PHORJ_CLOUD_KIT_KEY="$KEY" \
     bash "$KIT/install.sh" --from-plain "$T/global.tar.gz" "$T/memories.tar.gz" >"$T/out" 2>&1; then
  bad "no proxy dir: install succeeded"
else
  [[ "$(snapshot)" == "$before" ]] && grep -q 'not a claude.ai cloud session' "$T/out" \
    && ok "no proxy dir: refused, HOME untouched" || bad "no proxy dir: wrong refusal or HOME changed"
fi

# ── 3. refusal: encrypted mode without the key ───────────────────────────────────────────────────────
bash "$KIT/pack.sh" "$RECIP" "$T/global.tar.gz" "$T/memories.tar.gz" "$T/kit.tar.age" >/dev/null
fresh_home; before="$(snapshot)"
if env -i PATH="$PATH" HOME="$T/home" CLAUDE_CODE_REMOTE=true CLOUD_KIT_CCR_DIR="$T/ccr" \
     bash "$KIT/install.sh" --kit "$T/kit.tar.age" >"$T/out" 2>&1; then
  bad "no key: install succeeded"
else
  [[ "$(snapshot)" == "$before" ]] && grep -q 'PHORJ_CLOUD_KIT_KEY' "$T/out" \
    && ok "no key: refused, HOME untouched" || bad "no key: wrong refusal or HOME changed"
fi

# ── 4. the blob is ciphertext: no fixture string survives in it ──────────────────────────────────────
if grep -qa -e 'a fact' -e 'Global Reasoning Framework' "$T/kit.tar.age"; then bad "blob leaks plaintext"
else ok "blob holds no plaintext fixture string"; fi

# ── 5. encrypted round-trip into a cloud-like HOME ───────────────────────────────────────────────────
fresh_home
if cloud bash "$KIT/install.sh" --kit "$T/kit.tar.age" >"$T/out" 2>&1; then ok "cloud env: install exit 0"
else bad "cloud env: install failed: $(tail -3 "$T/out")"; fi
H="$T/home/.claude"; P="$H/projects/$SLUG"
[[ -f "$H/THINKING.md" && -f "$H/skills/next/SKILL.md" ]] && ok "framework files installed" || bad "framework files missing"
[[ ! -e "$H/skills/loop" ]] && ok "bundle loop skill skipped (host provides /loop)" || bad "loop skill installed"
grep -q '^@RTK' "$H/CLAUDE.md" && bad "@RTK include kept" || ok "@RTK includes removed"
grep -q '^@THINKING.md' "$H/CLAUDE.md" && ok "other @includes kept" || bad "@THINKING include lost"
head -5 "$H/CLAUDE.md" | grep -q 'Cloud adaptation' && ok "cloud adaptation block prepended" || bad "no cloud block"
grep -q "if d != HOME" "$H/hooks/ask-bash-firewall.sh" && ok "firewall copy patched (HOME out of HOST_DIRS)" || bad "firewall not patched"
[[ "$(cat "$P/memory/fact.md" 2>/dev/null)" == "a fact" ]] && ok "memory under the cloud slug" || bad "memory not at $P"
[[ -f "$P/certification-schedule" && -f "$P/plan-location" && -f "$P/expertise/index.md" ]] \
  && ok "schedule, plan-location, expertise installed" || bad "project files missing"
[[ -s "$P/.cloud-kit-manifest" ]] && ok "memory manifest written" || bad "no manifest"
python3 - "$H/settings.json" <<'PY' && ok "settings: merged, harness key kept, no deny/defaultMode/model/env/statusLine" || bad "settings wrong"
import json, sys
s = json.load(open(sys.argv[1]))
assert s.get("advisorModel") == "fable", "harness key lost"
for k in ("defaultMode", "model", "env", "statusLine"): assert k not in s, k
assert not s.get("permissions", {}).get("deny"), "deny kept"
cmds = [h["command"] for ev in s["hooks"].values() for g in ev for h in g["hooks"]]
assert any("session-start-banner.sh" in c for c in cmds), "banner missing"
assert any("ask-human-question-guard.sh" in c for c in cmds), "guard missing"
assert any("ask-bash-firewall.sh" in c for c in cmds), "firewall not wired by default"
for skip in ("session-remember", "precompact-handoff", "rtk hook"):
    assert not any(skip in c for c in cmds), "skipped hook present: " + skip
PY

# ── 6. idempotent: a second run changes nothing ──────────────────────────────────────────────────────
before="$(snapshot)"
cloud bash "$KIT/install.sh" --kit "$T/kit.tar.age" >/dev/null 2>&1 || true
[[ "$(snapshot)" == "$before" ]] && ok "second install is a no-op" || bad "second install changed files"

# ── 7. --no-firewall leaves the firewall out ─────────────────────────────────────────────────────────
fresh_home; cloud bash "$KIT/install.sh" --kit "$T/kit.tar.age" --no-firewall >/dev/null 2>&1 || true
grep -q 'ask-bash-firewall.sh' "$H/settings.json" && bad "--no-firewall still wired it" || ok "--no-firewall leaves the firewall out"
fresh_home; cloud bash "$KIT/install.sh" --kit "$T/kit.tar.age" >/dev/null 2>&1

# ── 8. an unsafe archive (path escape) is refused before anything is written ─────────────────────────
python3 - "$T/evil.tar.gz" <<'PY'   # GNU tar strips a leading ../ on create, so build the escape by hand
import io, sys, tarfile
with tarfile.open(sys.argv[1], "w:gz") as t:
    data = b"pwned"; ti = tarfile.TarInfo("../escape"); ti.size = len(data); t.addfile(ti, io.BytesIO(data))
PY
fresh_home; before="$(snapshot)"
if cloud bash "$KIT/install.sh" --from-plain "$T/evil.tar.gz" "$T/memories.tar.gz" >"$T/out" 2>&1; then
  bad "unsafe archive accepted"
else
  [[ "$(snapshot)" == "$before" ]] && grep -q 'unsafe' "$T/out" && ok "unsafe archive refused, HOME untouched" \
    || bad "unsafe archive: wrong refusal or HOME changed"
fi

# ── 9. memory delta: only new/changed files, slug renamed back for the PC, encrypted ─────────────────
fresh_home; cloud bash "$KIT/install.sh" --kit "$T/kit.tar.age" >/dev/null 2>&1
P="$T/home/.claude/projects/$SLUG"
echo 'changed fact' >"$P/memory/fact.md"; echo 'brand new' >"$P/memory/new.md"
mkdir -p "$T/deltas"
cloud env CLOUD_KIT_RECIPIENT="$RECIP" bash "$KIT/memory-delta.sh" --out "$T/deltas" >"$T/out" 2>&1 || true
d="$(ls "$T/deltas"/*.age 2>/dev/null | head -1)"
if [[ -n "$d" ]] && age -d -i "$T/key.txt" "$d" >"$T/delta.tar" 2>/dev/null; then
  list="$(tar -tf "$T/delta.tar" | sort | tr '\n' ' ')"
  [[ "$list" == *"-stack-projects-phorj/memory/fact.md"* && "$list" == *"-stack-projects-phorj/memory/new.md"* \
     && "$list" != *"MEMORY.md"* ]] && ok "delta: changed + new only, PC slug" || bad "delta contents wrong: $list"
else bad "no decryptable delta produced: $(tail -2 "$T/out")"; fi
rm -rf "$T/deltas"/*
cloud env CLOUD_KIT_RECIPIENT="$RECIP" bash "$KIT/memory-delta.sh" --out "$T/deltas" >"$T/out" 2>&1 || true
ls "$T/deltas"/*.age >/dev/null 2>&1 && bad "delta after re-baseline not empty" || ok "no delta when nothing changed since the last one"

# ── 10. arm.sh: refuses off-cloud, arms once, idempotent ─────────────────────────────────────────────
fresh_home; before="$(snapshot)"
if env -i PATH="$PATH" HOME="$T/home" CLOUD_KIT_CCR_DIR="$T/no-such-dir" bash "$KIT/arm.sh" >/dev/null 2>&1; then
  bad "arm.sh ran off-cloud"
else env -i PATH="$PATH" HOME="$T/home" CLOUD_KIT_CCR_DIR="$T/no-such-dir" bash "$KIT/arm.sh" >"$T/out" 2>&1 || true
  [[ "$(snapshot)" == "$before" ]] && grep -q 'not a claude.ai cloud' "$T/out" \
  && ok "arm.sh refuses off-cloud, HOME untouched" || bad "arm.sh wrong refusal or changed HOME off-cloud"; fi
env -i PATH="$PATH" HOME="$T/home" CLOUD_KIT_CCR_DIR="$T/ccr" bash "$KIT/arm.sh" >/dev/null 2>&1
env -i PATH="$PATH" HOME="$T/home" CLOUD_KIT_CCR_DIR="$T/ccr" bash "$KIT/arm.sh" >/dev/null 2>&1
n="$(grep -o 'tools/cloud-kit/install.sh' "$T/home/.claude/settings.json" | wc -l)"
[[ "$n" -eq 1 ]] && grep -q '"advisorModel": "fable"' "$T/home/.claude/settings.json" \
  && ok "arm.sh adds exactly one SessionStart hook, keeps harness keys" || bad "arm.sh hook count $n or keys lost"

# ── 11. the --hook mode never fails a session start, even off-cloud ──────────────────────────────────
fresh_home
fresh_home; before="$(snapshot)"
env -i PATH="$PATH" HOME="$T/home" bash "$KIT/install.sh" --hook >/dev/null 2>&1 && [[ "$(snapshot)" == "$before" ]] \
  && [[ ! -e "$T/home/.claude/logs" ]] && ok "--hook off-cloud: exit 0, no trace" || bad "--hook off-cloud failed or left a trace"

echo "test-cloud-kit: $pass passed, $fail failed"
[[ $fail -eq 0 ]]
