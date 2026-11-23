#!/usr/bin/env bash
# Behaviour tests for scripts/owed-legs.sh (the durable OWED list, DEC-597) and scripts/microbench-leg.sh (pre-push's
# microbench leg: the environment policy around microbench-gate.sh, DEC-597/598). Run by scripts/git-hooks/pre-push.
#   - record is idempotent per leg (the oldest commit is kept), the line passes doc-guards G3, clear turns OPEN into
#     CLEARED, list prints only OPEN entries, has-committed reads HEAD;
#   - check-staged, through a REAL pathspec commit with the hook installed, refuses a deleted OPEN entry and a new entry
#     left out of the commit, and accepts both once staged;
#   - the leg: an identity break blocks in every environment; a flip blocks on the PC and is recorded OWED in a cloud
#     session; every cloud pass is recorded; a SKIP or an unconfirmed "verdict OWED" pass is recorded; a JSON-seam pass
#     never clears; only a real PC pass clears; new or RECOVERED benches open `microbench-baseline`.
# The real gate is driven through its JSON seam (so the wording the leg keys on is the gate's own); a fake gate stands
# in only for the SKIP, OWED and real-pass outputs. Throwaway repos; no network, no timing, nothing written outside $TMP.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(git -C "$HERE" rev-parse --show-toplevel)"
PASS=0; FAIL=0
ok()  { printf '  ok   — %s\n' "$1"; PASS=$((PASS + 1)); }
bad() { printf '  FAIL — %s\n' "$1"; FAIL=$((FAIL + 1)); }
check() { if eval "$2"; then ok "$1"; else bad "$1 [$2]"; fi; }
TMP=$(mktemp -d /tmp/test-owed-legs-XXXXXX)
trap 'rm -rf "$TMP"' EXIT
# A hook or a cloud session hands git variables down; the throwaway repos below must see only themselves.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_CONFIG_COUNT GIT_CONFIG_PARAMETERS CLAUDE_CODE_REMOTE MICROBENCH_GATE_JSON
for v in "${!GIT_CONFIG_KEY_@}" "${!GIT_CONFIG_VALUE_@}"; do unset "$v"; done
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL="$TMP/gitconfig"
printf '[user]\n\tname = t\n\temail = t@example.invalid\n[commit]\n\tgpgsign = false\n' >"$GIT_CONFIG_GLOBAL"
echo "owed-legs — $ROOT/scripts/owed-legs.sh, $ROOT/scripts/microbench-leg.sh"

# A throwaway repo shaped like phorj: the two scripts, the gate, a SLICE-STATE with a defect queue, a pre-commit hook.
R="$TMP/repo"; mkdir -p "$R/scripts" "$R/docs/plans" "$R/hooks"
cp "$ROOT/scripts/owed-legs.sh" "$R/scripts/"; [[ -f "$ROOT/scripts/microbench-leg.sh" ]] && cp "$ROOT/scripts/microbench-leg.sh" "$R/scripts/"
cp "$ROOT/scripts/microbench-gate.sh" "$R/scripts/"
printf '# Slice state\n\nintro\n\n## DEFECT QUEUE (DEC-578)\n\n- one\n' >"$R/docs/plans/SLICE-STATE.md"
printf '#!/usr/bin/env bash\nexec bash scripts/owed-legs.sh check-staged\n' >"$R/hooks/pre-commit"; chmod +x "$R/hooks/pre-commit"
git -C "$R" init -q; git -C "$R" add -A; git -C "$R" commit -q -m "chore: the first commit of the fixture"
git -C "$R" config core.hooksPath hooks
git -C "$R" config phorj.runner host   # the leg runs its body here; the dk re-exec has its own case (section 3)
O="$R/docs/plans/SLICE-STATE.md"
owed() { (cd "$R" && bash scripts/owed-legs.sh "$@"); }

# ── 1. record / clear / list / has-committed ─────────────────────────────────────────────────────────────────
out="$(owed record microbench pc 'load 3.27 > 2.5 after 90s')"
check "record adds one OPEN line inside the block, before the defect queue" \
  '[[ $(grep -c "^- OPEN microbench since " "$O") == 1 ]] && awk "/owed-legs -->/{a=NR} /^## DEFECT QUEUE/{b=NR} END{exit !(a && a<b)}" "$O"'
check "the line names the commit, its subject in backticks, the date, the env and the reason" \
  'grep -qE "^- OPEN microbench since [0-9a-f]{8} \`chore: the first commit of the fixture\` \([0-9]{4}-[0-9]{2}-[0-9]{2}, pc\): load 3.27" "$O"'
check "record says to commit it" '[[ "$out" == *"commit it"* ]]'
sha1="$(git -C "$R" rev-parse --short=8 HEAD)"
git -C "$R" commit -q -m "docs: record the owed microbench entry" -- docs/plans/SLICE-STATE.md
owed record microbench cloud 'another reason' >/dev/null
check "record again: the oldest commit is kept, nothing changes" \
  '[[ $(grep -c "^- OPEN microbench " "$O") == 1 ]] && grep -q "since $sha1 " "$O" && git -C "$R" diff --quiet'
check "list prints the OPEN entry" '[[ "$(owed list)" == "OWED (DEC-597): - OPEN microbench since $sha1 "* ]]'
check "has-committed: yes for microbench, no for another leg" 'owed has-committed microbench && ! owed has-committed other'
check "a bad leg name is refused (exit 2)" '(owed record "Bad Leg" pc x >/dev/null 2>&1); [[ $? == 2 ]]'
# doc-guards G3: a SHA in docs/plans needs a backtick subject of >= 8 characters
check "the written line passes doc-guards G3 (a backtick subject of 8+ characters)" \
  'grep -E "^- OPEN " "$O" | grep -qE "\`[^\`]{8,}\`"'

# ── 2. check-staged, through real pathspec commits ──────────────────────────────────────────────────────────
printf 'x\n' >"$R/a.txt"; git -C "$R" add a.txt; git -C "$R" commit -q -m "feat: add a file to the fixture" -- a.txt
sed -i '/^- OPEN microbench /d' "$O"
printf 'y\n' >>"$R/a.txt"
git -C "$R" commit -q -m "docs: drop the entry by hand" -- a.txt docs/plans/SLICE-STATE.md 2>"$TMP/err"; rc=$?
check "a commit deleting an OPEN entry by hand is refused" '[[ $rc != 0 ]] && grep -q "only a gate" "$TMP/err"'
git -C "$R" checkout -q -- docs/plans/SLICE-STATE.md
owed record identity-x pc 'a new owed entry' >/dev/null
git -C "$R" commit -q -m "feat: a commit that leaves the new entry out" -- a.txt 2>"$TMP/err"; rc=$?
check "a commit that leaves a new entry in the working tree is refused" '[[ $rc != 0 ]] && grep -q "stage that file" "$TMP/err"'
git -C "$R" commit -q -m "feat: the same commit with the entry staged" -- a.txt docs/plans/SLICE-STATE.md 2>"$TMP/err"; rc=$?
check "...and accepted once the file is staged" '[[ $rc == 0 ]]'
out="$(owed clear identity-x 'a real pass')"
check "clear turns OPEN into CLEARED, keeps the since, drops the reason, names the pass" \
  'grep -qE "^- CLEARED identity-x since [0-9a-f]{8} \`[^\`]+\` \([0-9-]+, pc\) by a PASS at [0-9a-f]{8} \([0-9-]+, a real pass\)$" "$O" && ! grep -q "^- OPEN identity-x" "$O"'
git -C "$R" commit -q -m "docs: commit the clearance of the entry" -- docs/plans/SLICE-STATE.md 2>"$TMP/err"; rc=$?
check "a gate's clearance commits (OPEN -> CLEARED is not a deletion)" '[[ $rc == 0 ]]'
check "clear on a leg that is not open is a no-op" '[[ -z "$(owed clear nothing-open x)" ]] && git -C "$R" diff --quiet'

# ── 3. the microbench leg ───────────────────────────────────────────────────────────────────────────────────
B="$ROOT/bench/micro-baseline.json"
jq -r '[.features | to_entries[] | {feature: .key, vm_ns: 1000, php_ns: 1000, ratio: .value.ratio, identical: true}]' "$B" >"$TMP/clean.json"
win="$(jq -r '[.features | to_entries[] | select(.value.ratio >= 2)] | sort_by(.value.ratio) | last | .key' "$B")"
jq --arg f "$win" 'map(if .feature == $f then .ratio = 0.3 else . end)' "$TMP/clean.json" >"$TMP/flip.json"
jq --arg f "$win" 'map(if .feature == $f then .identical = false else . end)' "$TMP/clean.json" >"$TMP/ident.json"
jq '. + [{feature: "brandnew", vm_ns: 1000, php_ns: 1000, ratio: 0.5, identical: true}]' "$TMP/clean.json" >"$TMP/new.json"
# A fake gate for the outputs the seam cannot produce: prints $FAKE_OUT, exits $FAKE_RC.
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$FAKE_OUT"\necho "knobs: settle=${MICROBENCH_SETTLE_SECS:-} maxload=${MICROBENCH_MAX_LOAD:-} idonly=${MICROBENCH_IDENTITY_ONLY:-}"\nexit "${FAKE_RC:-0}"\n' >"$TMP/fakegate"; chmod +x "$TMP/fakegate"
reset_owed() { git -C "$R" checkout -q -- docs/plans/SLICE-STATE.md; sed -i '/^- \(OPEN\|CLEARED\) /d' "$O"; }
leg() { (cd "$R" && MICROBENCH_BASELINE="$B" bash scripts/microbench-leg.sh "$@" >"$TMP/legout" 2>&1); }
seam() { local j="$1"; shift; MICROBENCH_GATE_JSON="$j" leg "$@"; }
fake() { MICROBENCH_GATE_SCRIPT="$TMP/fakegate" leg "$@"; }

if [[ ! -f "$R/scripts/microbench-leg.sh" ]]; then bad "scripts/microbench-leg.sh exists"
else
reset_owed; seam "$TMP/ident.json"; rc=$?
check "PC: an identity break blocks (exit 1)" '[[ $rc == 1 ]]'
reset_owed; CLAUDE_CODE_REMOTE=true seam "$TMP/ident.json"; rc=$?
check "cloud: an identity break blocks too (exit 1)" '[[ $rc == 1 ]]'
reset_owed; seam "$TMP/flip.json"; rc=$?
check "PC: a flip blocks (exit 1, DEC-598) and records nothing" '[[ $rc == 1 ]] && ! grep -q "^- OPEN " "$O"'
reset_owed; CLAUDE_CODE_REMOTE=true seam "$TMP/flip.json"; rc=$?
check "cloud: a flip does not block; microbench is OWED, naming the bench" '[[ $rc == 0 ]] && grep -qE "^- OPEN microbench .*cloud\).*$win" "$O"'
reset_owed; CLAUDE_CODE_REMOTE=true seam "$TMP/clean.json"; rc=$?
check "cloud: a clean pass is still OWED (ratios from a shared VM)" '[[ $rc == 0 ]] && grep -qE "^- OPEN microbench .*cloud\)" "$O"'
reset_owed; owed record microbench pc 'an older skip' >/dev/null; seam "$TMP/clean.json"; rc=$?
check "a JSON-seam pass never clears" '[[ $rc == 0 ]] && grep -q "^- OPEN microbench " "$O"'
reset_owed; FAKE_OUT='microbench-gate: 1-min load 3.27 still > 2.5 after 90s — SKIP the G-8 ratchet (box too loaded)' fake; rc=$?
check "PC: a SKIP records microbench OWED with the gate's reason" '[[ $rc == 0 ]] && grep -qE "^- OPEN microbench .*pc\): .*SKIP the G-8 ratchet" "$O"'
reset_owed; FAKE_OUT=$'microbench-gate: re-measure FAILED — suspects unconfirmed and NOT blocked; verdict OWED for: x\nmicrobench-gate: PASS (ratchet: no flips, all output-identical)' fake; rc=$?
check "PC: a pass with an unconfirmed 'verdict OWED' is recorded, not cleared" '[[ $rc == 0 ]] && grep -qE "^- OPEN microbench .*verdict OWED" "$O"'
reset_owed; owed record microbench pc 'an older skip' >/dev/null; FAKE_OUT='microbench-gate: PASS (ratchet: no flips, all output-identical)' fake; rc=$?
check "PC: a real pass clears microbench" '[[ $rc == 0 ]] && grep -q "^- CLEARED microbench " "$O" && ! grep -q "^- OPEN microbench" "$O"'
reset_owed; seam "$TMP/new.json"; rc=$?
check "a bench missing from the baseline opens microbench-baseline, naming it" '[[ $rc == 0 ]] && grep -qE "^- OPEN microbench-baseline .*brandnew" "$O"'
reset_owed; FAKE_OUT=$'  RECOVERED deepjson: owed at 0.9, now 1.1 (a WIN) — re-emit so the ratchet protects it\nmicrobench-gate: PASS (ratchet: no flips, all output-identical)' fake; rc=$?
check "a RECOVERED bench opens microbench-baseline" '[[ $rc == 0 ]] && grep -qE "^- OPEN microbench-baseline .*deepjson" "$O"'
reset_owed; CLAUDE_CODE_REMOTE=true FAKE_OUT=$'  RECOVERED deepjson: owed at 0.9, now 1.1 (a WIN) — re-emit so the ratchet protects it\nmicrobench-gate: PASS (ratchet: no flips, all output-identical)' fake; rc=$?
check "cloud: a RECOVERED line (a comparator artefact there) opens no microbench-baseline entry" '[[ $rc == 0 ]] && ! grep -q "^- OPEN microbench-baseline" "$O"'
reset_owed; FAKE_RC=2 FAKE_OUT='microbench-gate: SETUP ERROR — 1 malformed baseline ratio(s)' fake; rc=$?
check "a setup error stays exit 2 and records nothing" '[[ $rc == 2 ]] && ! grep -q "^- OPEN " "$O"'
reset_owed; CLAUDE_CODE_REMOTE=true FAKE_RC=1 FAKE_OUT=$'microbench-gate: harness died' fake; rc=$?
check "cloud: a gate exit 1 with no FAIL line propagates (not read as timing), records nothing" '[[ $rc == 1 ]] && ! grep -q "^- OPEN " "$O"'
reset_owed; CLAUDE_CODE_REMOTE=true FAKE_RC=1 FAKE_OUT=$'  FAIL a: WIN->LOSS flip — baseline 2 (WIN) now 0.3\n  FAIL b: something else entirely' fake; rc=$?
check "cloud: a FAIL that is not a flip or a deepening propagates" '[[ $rc == 1 ]] && ! grep -q "^- OPEN " "$O"'
reset_owed; CLAUDE_CODE_REMOTE=true FAKE_RC=1 FAKE_OUT=$'  FAIL a: WIN->LOSS flip — baseline 2 (WIN) now 0.3\n  FAIL b: an OWED loss DEEPENED — was 0.9, now 0.5' fake; rc=$?
check "cloud: flips and deepenings only: OWED naming both, exit 0" '[[ $rc == 0 ]] && grep -qE "^- OPEN microbench .*cloud\).*a b" "$O"'
reset_owed; CLAUDE_CODE_REMOTE=true FAKE_OUT='microbench-gate: PASS (ratchet: no flips, all output-identical)' fake
check "cloud: the leg lifts the load guard and asks for identity through a source mismatch" 'grep -q "maxload=1000 idonly=1" "$TMP/legout"'
FAKE_OUT='microbench-gate: PASS (ratchet: no flips, all output-identical)' fake
check "PC: the leg sets neither knob" 'grep -q "settle= maxload= idonly=$" "$TMP/legout"'
# --verify: a release build first (fake cargo, recorded), then the gate with a long settle; refused in the cloud.
mkdir -p "$TMP/vbin"; printf '#!/bin/sh\necho "cargo $*" >>"%s/cargo.calls"\n' "$TMP" >"$TMP/vbin/cargo"; chmod +x "$TMP/vbin/cargo"; : >"$TMP/cargo.calls"
reset_owed; PATH="$TMP/vbin:$PATH" FAKE_OUT='microbench-gate: PASS (ratchet: no flips, all output-identical)' fake --verify; rc=$?
check "--verify: cargo build --release first, then the gate with a 900 s settle" '[[ $rc == 0 ]] && grep -qx "cargo build --release" "$TMP/cargo.calls" && grep -q "settle=900" "$TMP/legout"'
: >"$TMP/cargo.calls"; CLAUDE_CODE_REMOTE=true PATH="$TMP/vbin:$PATH" fake --verify; rc=$?
check "--verify in a cloud session: refused (exit 2), nothing built" '[[ $rc == 2 && ! -s "$TMP/cargo.calls" ]]'
# Off the host runner the leg re-executes through scripts/dk, as the hooks do: never a host cargo (CLAUDE.md).
R2="$TMP/repo2"; mkdir -p "$R2/scripts"; cp "$ROOT/scripts/microbench-leg.sh" "$ROOT/scripts/dk" "$R2/scripts/"; cp "$ROOT/compose.yaml" "$R2/"
git -C "$R2" init -q; printf '#!/bin/sh\necho "docker $*" >>"%s/cargo.calls"\nexit 1\n' "$TMP" >"$TMP/vbin/docker"; chmod +x "$TMP/vbin/docker"
: >"$TMP/cargo.calls"; (cd "$R2" && env -u PHORJ_IN_DOCKER PATH="$TMP/vbin:$PATH" bash scripts/microbench-leg.sh --verify >"$TMP/legout" 2>&1); rc=$?
check "--verify on a PC host: re-executes through scripts/dk (docker is asked), never a host cargo" \
  '[[ $rc != 0 ]] && grep -q "^docker" "$TMP/cargo.calls" && ! grep -q "^cargo" "$TMP/cargo.calls"'
# The cloud path through the REAL gate (no seam): fake php, phg, harness and load, the real baseline's docker source
# (so a SOURCE MISMATCH, as on a cloud host without docker). An identity break blocks; a flip is OWED.
printf '#!/bin/sh\nexit 0\n' >"$TMP/fakephp"; cp "$TMP/fakephp" "$TMP/fakephg"
printf '#!/bin/sh\ncat "$HARNESS_JSON"\n' >"$TMP/harness"; chmod +x "$TMP/fakephp" "$TMP/fakephg" "$TMP/harness"
printf '9.99 9.99 9.99 1/100 1\n' >"$TMP/load-high"
real() { CLAUDE_CODE_REMOTE=true MICROBENCH_PHP_BIN="$TMP/fakephp" PHG_BIN="$TMP/fakephg" MICROBENCH_HARNESS="$TMP/harness" \
  HARNESS_JSON="$1" MICROBENCH_LOADAVG="$TMP/load-high" MICROBENCH_SETTLE_SECS=0 leg; }
reset_owed; real "$TMP/ident.json"; rc=$?
check "cloud, real gate, loaded, source mismatch: an identity break BLOCKS (exit 1)" '[[ $rc == 1 ]] && grep -q "output-identity break" "$TMP/legout"'
reset_owed; real "$TMP/flip.json"; rc=$?
check "cloud, real gate, loaded, source mismatch: a flip is OWED, exit 0" '[[ $rc == 0 ]] && grep -qE "^- OPEN microbench .*cloud\).*$win" "$O"'
fi

# ── 4. The cloud pre-push refusal: the OWED entry travels with the push (DEC-597) ───────────────────────────
H="$TMP/hookrepo"; mkdir -p "$H/scripts/git-hooks" "$H/docs/plans" "$TMP/hbin"
cp "$ROOT/scripts/git-hooks/pre-push" "$H/scripts/git-hooks/pre-push"; cp "$ROOT/scripts/owed-legs.sh" "$H/scripts/"
printf '# Slice state\n\n## DEFECT QUEUE\n' >"$H/docs/plans/SLICE-STATE.md"
git -C "$H" init -q; git -C "$H" add -A; git -C "$H" commit -q -m "chore: the hook fixture's first commit"; git -C "$H" config phorj.runner host
printf '#!/bin/sh\necho "$(basename "$0") $*" >>"%s/hook.calls"\nexit 0\n' "$TMP" >"$TMP/hbin/cargo"; cp "$TMP/hbin/cargo" "$TMP/hbin/docker"; chmod +x "$TMP/hbin/"*
hook() { : >"$TMP/hook.calls"; (cd "$H" && CLAUDE_CODE_REMOTE=true PATH="$TMP/hbin:$PATH" bash scripts/git-hooks/pre-push origin url </dev/null >"$TMP/hookout" 2>&1); }
hook; rc=$?
check "cloud pre-push, entry not committed: REFUSED (exit 1) before any cargo or docker call" \
  '[[ $rc == 1 ]] && grep -q "REFUSED (DEC-597)" "$TMP/hookout" && [[ ! -s "$TMP/hook.calls" ]]'
check "...and it wrote the cloud OWED entry for microbench" 'grep -qE "^- OPEN microbench since [0-9a-f]+ .*cloud\)" "$H/docs/plans/SLICE-STATE.md"'
git -C "$H" commit -q -m "docs: commit the owed microbench entry" -- docs/plans/SLICE-STATE.md
hook; rc=$?
check "cloud pre-push, entry committed: no refusal (it goes on to the gate)" '! grep -q "REFUSED" "$TMP/hookout"'
git -C "$H" reset -q --hard HEAD~1   # back to no committed entry: the PC must not refuse either
(cd "$H" && env -u CLAUDE_CODE_REMOTE PATH="$TMP/hbin:$PATH" bash scripts/git-hooks/pre-push origin url </dev/null >"$TMP/hookout" 2>&1)
check "PC pre-push (no CLAUDE_CODE_REMOTE): never refused for an owed entry" '! grep -q "REFUSED" "$TMP/hookout"'

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 && $PASS -gt 0 ]]
