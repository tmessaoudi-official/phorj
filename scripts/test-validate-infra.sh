#!/usr/bin/env bash
# Test suite for validate-infra.sh (DEC-388) — the Rule-7 infra Coverage gate.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOL="$HERE/validate-infra.sh"
PASS=0; FAIL=0
ok()  { printf '  ok   — %s\n' "$1"; PASS=$((PASS + 1)); }
bad() { printf '  FAIL — %s\n' "$1"; FAIL=$((FAIL + 1)); }

TMP=$(mktemp -d /tmp/test-validate-infra-XXXXXX)
trap 'rm -rf "$TMP"' EXIT

mkrepo() { # $1 = dir ; creates a minimal repo with tracked infra files
  local r="$1"
  mkdir -p "$r"/{scripts,.github/workflows,.claude}
  printf '[package]\nname = "phorj"\n' > "$r/Cargo.toml"
  mkdir -p "$r/docs/plans" && printf '# cursor\n' > "$r/docs/plans/SLICE-STATE.md"
  printf '#!/usr/bin/env bash\nset -euo pipefail\necho fine\n' > "$r/scripts/good.sh"
  printf 'name: ci\non: [push]\njobs:\n  a:\n    runs-on: ubuntu-latest\n' > "$r/.github/workflows/ci.yml"
  printf '{"permissions":{"allow":[]}}\n' > "$r/.claude/settings.json"
  ( cd "$r" && git init -q . && git add -A && git -c user.email=t@t -c user.name=t commit -qm init ) 2>/dev/null
}

echo "== validate-infra.sh =="
if [[ ! -f "$TOOL" ]]; then bad "tool exists at $TOOL"; printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"; exit 1; fi
ok "tool exists"
bash -n "$TOOL" && ok "bash -n parses" || bad "bash -n parses"

# ── 1. A clean tree passes, exit 0, and emits the Rule-7 evidence table ──────────────────
R="$TMP/clean"; mkrepo "$R"
out=$(cd "$R" && bash "$TOOL" 2>&1); rc=$?
[[ $rc -eq 0 ]] && ok "exit 0 on a clean tree" || bad "exit 0 on a clean tree (got $rc: $out)"
grep -qiE 'coverage' <<<"$out" && ok "emits a Coverage evidence section (Rule 7)" || bad "emits a Coverage evidence section"
grep -qF 'scripts/good.sh' <<<"$out" && ok "checks tracked shell scripts" || bad "checks tracked shell scripts"
grep -qF '.github/workflows/ci.yml' <<<"$out" && ok "checks workflow YAML" || bad "checks workflow YAML"
grep -qF '.claude/settings.json' <<<"$out" && ok "checks tracked JSON" || bad "checks tracked JSON"

# ── 2. A shell syntax error FAILS the gate (this is the whole point) ─────────────────────
R="$TMP/badsh"; mkrepo "$R"
printf '#!/usr/bin/env bash\nif [ 1 -eq 1 ]; then\n  echo unterminated\n' > "$R/scripts/broken.sh"
( cd "$R" && git add -A && git -c user.email=t@t -c user.name=t commit -qm x ) 2>/dev/null
out=$(cd "$R" && bash "$TOOL" 2>&1); rc=$?
[[ $rc -ne 0 ]] && ok "non-zero exit on a shell syntax error" || bad "non-zero exit on a shell syntax error"
grep -qiE 'fail' <<<"$out" && ok "names the failing shell file" || bad "names the failing shell file"

# ── 2b. A pinned php-oracle path FAILS — and a pin in a NON-hook script fails too ────────
# The check shipped 2026-08-20 with no test and, in this harness's fixture, scanned ZERO files while
# printing a pass — the probe-that-cannot-fail shape. These cases are the guard against it going
# dark again: one red per scanned surface, one green proving comments are not flagged, and an
# explicit assertion that the scan inspected a non-zero number of files.
# Assembled, never written literally: this file is itself scanned by the check under test, and a
# literal pin here would (correctly) fail the gate. The FIXTURE gets the full literal; the harness
# source does not.
PINPATH="/stack/tools/phpbrew/php/php-8.5.${PINPATCH:-8}/bin/php"
R="$TMP/pinhook"; mkrepo "$R"
mkdir -p "$R/scripts/git-hooks"
printf '#!/usr/bin/env bash\nexport PHORJ_PHP=%s\n' "$PINPATH" > "$R/scripts/git-hooks/pre-push"
( cd "$R" && git add -A && git -c user.email=t@t -c user.name=t commit -qm x ) 2>/dev/null
out=$(cd "$R" && bash "$TOOL" 2>&1); rc=$?
[[ $rc -ne 0 ]] && ok "non-zero exit on a pinned php path in a git hook" || bad "non-zero exit on a pinned php path in a git hook (got $rc)"
grep -qF 'pinned php path' <<<"$out" && ok "names the pin" || bad "names the pin"

R="$TMP/pinscript"; mkrepo "$R"
printf '#!/usr/bin/env bash\nPHP=%s\necho "$PHP"\n' "$PINPATH" > "$R/scripts/perf-thing.sh"
( cd "$R" && git add -A && git -c user.email=t@t -c user.name=t commit -qm x ) 2>/dev/null
out=$(cd "$R" && bash "$TOOL" 2>&1); rc=$?
[[ $rc -ne 0 ]] && ok "non-zero exit on a pinned php path in an ordinary script" || bad "non-zero exit on a pinned php path in an ordinary script (got $rc)"

# The real line number, not one from a comment-stripped stream.
R="$TMP/pinline"; mkrepo "$R"
{ printf '#!/usr/bin/env bash\n'; for _ in $(seq 8); do printf '# filler comment\n'; done
  printf 'PHP=%s\n' "$PINPATH"; } > "$R/scripts/deep.sh"
( cd "$R" && git add -A && git -c user.email=t@t -c user.name=t commit -qm x ) 2>/dev/null
out=$(cd "$R" && bash "$TOOL" 2>&1)
grep -qF 'deep.sh: pinned php path — 10:' <<<"$out" \
  && ok "reports the file's real line number (10), not the filtered-stream one" \
  || bad "reports the file's real line number; got: $(grep -F 'deep.sh' <<<"$out" | head -1)"

# A pin named only inside a COMMENT is documentation (toolchain.env's own root-cause narrative
# names the stale versions) and must not fail the gate.
R="$TMP/pincomment"; mkrepo "$R"
printf '#!/usr/bin/env bash\n# we used to pin %s here\necho ok\n' "$PINPATH" > "$R/scripts/documented.sh"
( cd "$R" && git add -A && git -c user.email=t@t -c user.name=t commit -qm x ) 2>/dev/null
out=$(cd "$R" && bash "$TOOL" 2>&1); rc=$?
[[ $rc -eq 0 ]] && ok "a pin mentioned in a comment does not fail the gate" || bad "a pin in a comment wrongly failed (got $rc)"
grep -qE '\| no pinned php oracle \| [0-9]+/[1-9][0-9]* clean \|' <<<"$out" \
  && ok "the pin scan inspected a non-zero number of files" \
  || bad "the pin scan inspected ZERO files (dark check): $(grep -F 'no pinned php oracle' <<<"$out")"

# ── 3. Malformed YAML FAILS ──────────────────────────────────────────────────────────────
R="$TMP/badyml"; mkrepo "$R"
printf 'name: ci\non: [push\njobs:\n  a: {{{\n' > "$R/.github/workflows/broken.yml"
( cd "$R" && git add -A && git -c user.email=t@t -c user.name=t commit -qm x ) 2>/dev/null
out=$(cd "$R" && bash "$TOOL" 2>&1); rc=$?
[[ $rc -ne 0 ]] && ok "non-zero exit on malformed YAML" || bad "non-zero exit on malformed YAML"

# ── 4. Malformed JSON FAILS — settings.json breaking silently is the real-world case ─────
R="$TMP/badjson"; mkrepo "$R"
printf '{"permissions": {"allow": [,]}}\n' > "$R/.claude/settings.json"
( cd "$R" && git add -A && git -c user.email=t@t -c user.name=t commit -qm x ) 2>/dev/null
out=$(cd "$R" && bash "$TOOL" 2>&1); rc=$?
[[ $rc -ne 0 ]] && ok "non-zero exit on malformed JSON" || bad "non-zero exit on malformed JSON"

# ── 5. Absent optional tools must SKIP LOUD, never be reported as a pass ─────────────────
R="$TMP/clean2"; mkrepo "$R"
out=$(cd "$R" && bash "$TOOL" 2>&1)
if command -v shellcheck >/dev/null 2>&1; then
  ok "shellcheck present — nothing to skip"
else
  grep -qiE 'skip.*shellcheck|shellcheck.*(absent|skip|not installed)' <<<"$out" \
    && ok "declares shellcheck SKIPPED out loud" || bad "silently omits shellcheck instead of skipping loud"
fi

# ── 6. --quiet suppresses the table but keeps the exit code (for hook use) ───────────────
R="$TMP/clean3"; mkrepo "$R"
out=$(cd "$R" && bash "$TOOL" --quiet 2>&1); rc=$?
[[ $rc -eq 0 ]] && ok "--quiet exits 0 on a clean tree" || bad "--quiet exits 0 on a clean tree"
[[ $(wc -l <<<"$out") -lt 6 ]] && ok "--quiet is actually quiet" || bad "--quiet printed $(wc -l <<<"$out") lines"
R="$TMP/badsh2"; mkrepo "$R"
printf '#!/usr/bin/env bash\nwhile true; do\n' > "$R/scripts/broken.sh"
( cd "$R" && git add -A && git -c user.email=t@t -c user.name=t commit -qm x ) 2>/dev/null
( cd "$R" && bash "$TOOL" --quiet >/dev/null 2>&1 ); rc=$?
[[ $rc -ne 0 ]] && ok "--quiet still fails on a broken file" || bad "--quiet swallowed a failure"

# ── 7. Read-only, checked BEHAVIOURALLY: the tree must be byte-identical afterwards ──────
# (A grep for redirects is the wrong test — `>/dev/null` is read-only. Compare the tree instead.)
R="$TMP/readonly"; mkrepo "$R"
before=$(cd "$R" && find . -path ./.git -prune -o -type f -exec sha256sum {} \; | sort | sha256sum)
before_status=$(cd "$R" && git status --porcelain)
( cd "$R" && bash "$TOOL" >/dev/null 2>&1 )
after=$(cd "$R" && find . -path ./.git -prune -o -type f -exec sha256sum {} \; | sort | sha256sum)
after_status=$(cd "$R" && git status --porcelain)
[[ "$before" == "$after" ]] && ok "tree is byte-identical after running (read-only)" \
                            || bad "tool MODIFIED the tree"
[[ "$before_status" == "$after_status" ]] && ok "nothing staged or newly untracked" \
                            || bad "tool changed git status"
# And explicitly: it must never invoke git write commands.
if grep -qE 'git (add|commit|push|checkout|reset)' "$TOOL"; then
  bad "tool invokes a git write command"
else
  ok "tool invokes no git write commands"
fi

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 ]]
