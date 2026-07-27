#!/usr/bin/env bash
# Test suite for disk-reclaim.sh (DEC-388 / Q-J5).
# This script DELETES things, so the tests concentrate on the guards: dry-run by default, refuses to
# run outside the phorj repo, never touches var/phorj-app or anything git-tracked.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOL="$HERE/disk-reclaim.sh"
PASS=0; FAIL=0
ok()  { printf '  ok   — %s\n' "$1"; PASS=$((PASS + 1)); }
bad() { printf '  FAIL — %s\n' "$1"; FAIL=$((FAIL + 1)); }

TMP=$(mktemp -d /tmp/test-disk-reclaim-XXXXXX)
trap 'rm -rf "$TMP"' EXIT

# ── A fake phorj repo: the marker files disk-reclaim.sh keys on, plus bait it must never touch ──
mkrepo() {
  local r="$1"
  mkdir -p "$r"/{src,examples,docs/plans,target/debug/incremental,target/release,var/phorj-app,var/claude/handoff}
  printf '[package]\nname = "phorj"\n' > "$r/Cargo.toml"
  printf '# cursor\n'                  > "$r/docs/plans/SLICE-STATE.md"
  printf 'fn main() {}\n'              > "$r/src/main.rs"
  head -c 4096 /dev/urandom            > "$r/target/debug/incremental/blob.bin"
  head -c 4096 /dev/urandom            > "$r/target/debug/binary"
  head -c 4096 /dev/urandom            > "$r/target/release/binary"
  printf 'the developer live comparison app — DEC-259 says never delete\n' > "$r/var/phorj-app/app.phg"
  printf 'old handoff\n'               > "$r/var/claude/handoff/handoff-old.md"
  ( cd "$r" && git init -q . && git add -A && git -c user.email=t@t -c user.name=t commit -qm init ) 2>/dev/null
}

echo "== disk-reclaim.sh =="
if [[ ! -f "$TOOL" ]]; then bad "tool exists at $TOOL"; printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"; exit 1; fi
ok "tool exists"
bash -n "$TOOL" && ok "bash -n parses" || bad "bash -n parses"

# ── 1. DRY RUN IS THE DEFAULT: reports, deletes nothing ──────────────────────────────────
R="$TMP/repo1"; mkrepo "$R"
out=$(cd "$R" && bash "$TOOL" 2>&1); rc=$?
[[ $rc -eq 0 ]] && ok "exit 0 on a dry run" || bad "exit 0 on a dry run (got $rc)"
[[ -f "$R/target/debug/incremental/blob.bin" ]] && ok "dry run deleted NOTHING" || bad "dry run deleted files"
grep -qiE 'dry.?run' <<<"$out" && ok "dry run says so out loud" || bad "dry run says so out loud"
grep -qiE 'would (free|remove|delete)' <<<"$out" && ok "reports what it would free" || bad "reports what it would free"

# ── 2. --yes on the default tier removes caches only ─────────────────────────────────────
R="$TMP/repo2"; mkrepo "$R"
out=$(cd "$R" && bash "$TOOL" --yes 2>&1); rc=$?
[[ $rc -eq 0 ]] && ok "exit 0 with --yes" || bad "exit 0 with --yes (got $rc)"
[[ ! -d "$R/target/debug/incremental" ]] && ok "cache tier removes target/*/incremental" || bad "cache tier removes target/*/incremental"
[[ -f "$R/target/debug/binary" ]] && ok "cache tier leaves target/debug/binary alone" || bad "cache tier leaves target/debug/binary alone"
[[ -f "$R/target/release/binary" ]] && ok "cache tier leaves target/release alone" || bad "cache tier leaves target/release alone"

# ── 3. THE BAIT: var/phorj-app must survive every tier (DEC-259) ─────────────────────────
R="$TMP/repo3"; mkrepo "$R"
( cd "$R" && bash "$TOOL" --tier=all --yes >/dev/null 2>&1 )
[[ -f "$R/var/phorj-app/app.phg" ]] && ok "var/phorj-app survives --tier=all (DEC-259)" || bad "var/phorj-app was DELETED — DEC-259 violated"
[[ -f "$R/src/main.rs" ]] && ok "src/ survives --tier=all" || bad "src/ was touched"
[[ -f "$R/Cargo.toml" ]] && ok "tracked files survive --tier=all" || bad "tracked files were touched"
[[ ! -d "$R/target/release" ]] && ok "--tier=all does remove target/release" || bad "--tier=all should remove target/release"

# ── 4. Refuses to run outside the phorj repo (no marker files) ───────────────────────────
NOPE="$TMP/notarepo"; mkdir -p "$NOPE/target/debug"; head -c 512 /dev/urandom > "$NOPE/target/debug/x"
out=$(cd "$NOPE" && bash "$TOOL" --yes 2>&1); rc=$?
[[ $rc -ne 0 ]] && ok "non-zero exit outside the phorj repo" || bad "non-zero exit outside the phorj repo (got $rc)"
[[ -f "$NOPE/target/debug/x" ]] && ok "deleted nothing outside the phorj repo" || bad "DELETED FILES outside the phorj repo"

# ── 5. An unknown tier is an error, not a silent default ─────────────────────────────────
R="$TMP/repo5"; mkrepo "$R"
out=$(cd "$R" && bash "$TOOL" --tier=bogus --yes 2>&1); rc=$?
[[ $rc -ne 0 ]] && ok "unknown --tier is an error" || bad "unknown --tier silently accepted"
[[ -f "$R/target/debug/binary" ]] && ok "unknown --tier deleted nothing" || bad "unknown --tier deleted files"

# ── 6. It reports disk state so the operator can judge ───────────────────────────────────
R="$TMP/repo6"; mkrepo "$R"
out=$(cd "$R" && bash "$TOOL" 2>&1)
grep -qE '[0-9]+%|Avail|avail' <<<"$out" && ok "reports filesystem headroom" || bad "reports filesystem headroom"

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 ]]
