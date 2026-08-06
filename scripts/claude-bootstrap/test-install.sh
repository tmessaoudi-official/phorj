#!/usr/bin/env bash
# Test suite for install.sh — the SessionStart bootstrap.
#
# The contract being pinned (developer ruling, 2026-08-06, ported from rent-watch b7867a4):
# **the repo is always the truth.** install.sh copies the three framework docs from the
# repo into ~/.claude UNCONDITIONALLY, every session, regardless of timestamps.
#
# The previous `cp -u` behaviour was timestamp-dependent and therefore nondeterministic,
# and its own header claim ("a hand-edited (newer) ~/.claude file is never clobbered") was
# FALSE: `cp -u` copies when the SOURCE is newer, and a fresh `git clone` stamps every file
# with the clone time — so in the container it clobbered anyway, while after a hand-edit of
# the target it silently did nothing and the repo stopped being the truth. Neither outcome
# was chosen; both depended on mtimes nobody was tracking.
#
# The one thing unconditional copying must NOT do is destroy a pre-existing global framework
# with no way back — so a file that was there before this hook ever ran is snapshotted once
# to <name>.pre-bootstrap.bak, and that snapshot is never touched again.
#
# Run: bash scripts/claude-bootstrap/test-install.sh
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$HERE/install.sh"
PASS=0; FAIL=0

ok()   { printf '  ok   — %s\n' "$1"; PASS=$((PASS+1)); }
bad()  { printf '  FAIL — %s\n' "$1"; FAIL=$((FAIL+1)); }
check(){ if [[ "$2" == "$3" ]]; then ok "$1"; else bad "$1 (want '$3', got '$2')"; fi; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Run install.sh with HOME and the project dir redirected into the sandbox.
run_install() {
  HOME="$TMP/home" CLAUDE_PROJECT_DIR="$TMP/proj" bash "$SCRIPT" >"$TMP/out" 2>"$TMP/err"
  printf '%s' $?
}

reset_sandbox() {
  rm -rf "$TMP/home" "$TMP/proj"
  mkdir -p "$TMP/home" "$TMP/proj"
}

echo "install.sh — repo-is-truth contract"

# ── 1. Cold install: nothing at ~/.claude yet ────────────────────────────────────
reset_sandbox
rc="$(run_install)"
check "exit 0 on a cold install" "$rc" "0"
for f in CLAUDE.md THINKING.md BLAST-RADIUS.md; do
  [[ -f "$TMP/home/.claude/$f" ]] && ok "installed $f" || bad "did not install $f"
done
if diff -q "$HERE/CLAUDE-global.md" "$TMP/home/.claude/CLAUDE.md" >/dev/null 2>&1; then
  ok "CLAUDE.md content matches the repo's CLAUDE-global.md"
else
  bad "CLAUDE.md content does not match the repo copy"
fi
[[ -d "$TMP/proj/var/claude" ]] && ok "pre-creates var/claude/ in the project dir" \
                                || bad "did not create var/claude/"

# ── 2. THE REGRESSION THIS SUITE EXISTS FOR ─────────────────────────────────────
# A NEWER file at the target must still be overwritten by the repo copy. Under the old
# `cp -u` this silently did nothing, so a stale global framework survived forever and
# the repo was NOT the truth.
reset_sandbox
mkdir -p "$TMP/home/.claude"
printf 'STALE CONTENT THAT MUST BE REPLACED\n' > "$TMP/home/.claude/CLAUDE.md"
touch -d '2099-01-01' "$TMP/home/.claude/CLAUDE.md"     # far newer than the repo copy
run_install >/dev/null
if grep -q 'STALE CONTENT' "$TMP/home/.claude/CLAUDE.md"; then
  bad "repo copy did NOT overwrite a newer target — the repo is not the truth"
else
  ok "repo copy overwrites a NEWER target (repo is the truth)"
fi

# Converse: the replacement is the real repo content, not an empty/truncated file.
if diff -q "$HERE/CLAUDE-global.md" "$TMP/home/.claude/CLAUDE.md" >/dev/null 2>&1; then
  ok "the overwritten file is byte-identical to the repo copy"
else
  bad "overwrote the target but the result differs from the repo copy"
fi

# ── 3. A pre-existing foreign framework is snapshotted exactly once ─────────────
reset_sandbox
mkdir -p "$TMP/home/.claude"
printf 'THE DEVELOPER OWN HAND-MAINTAINED FRAMEWORK\n' > "$TMP/home/.claude/CLAUDE.md"
run_install >/dev/null
BAK="$TMP/home/.claude/CLAUDE.md.pre-bootstrap.bak"
if [[ -f "$BAK" ]] && grep -q 'HAND-MAINTAINED' "$BAK"; then
  ok "snapshots a pre-existing foreign CLAUDE.md to .pre-bootstrap.bak"
else
  bad "did not snapshot the pre-existing CLAUDE.md"
fi

# Second run must NOT re-snapshot: by now the target is our own repo copy, and
# overwriting the backup with it would destroy the only surviving original.
run_install >/dev/null
if grep -q 'HAND-MAINTAINED' "$BAK"; then
  ok "the snapshot survives a second run (not overwritten by our own copy)"
else
  bad "second run clobbered the snapshot — the original is lost"
fi

# THE CASE THAT ACTUALLY EXERCISES THE `! -e "$backup"` GUARD, and the one that matters
# most in practice: the MULTI-REPO scenario. All five sibling repos ship this same hook,
# so opening rent-watch installs ITS CLAUDE-global.md over ours. On the next phorj
# session the target differs from our source again — and without the guard we would
# snapshot rent-watch's copy on top of the developer's irreplaceable original.
# Found by sabotage-verification upstream: removing the guard still passed the assertion
# above, because `cmp -s` alone short-circuits when the target happens to equal our source.
printf 'A SIBLING REPO CLAUDE-global.md (e.g. rent-watch)\n' > "$TMP/home/.claude/CLAUDE.md"
run_install >/dev/null
if grep -q 'HAND-MAINTAINED' "$BAK"; then
  ok "snapshot survives a sibling repo overwriting the target in between"
else
  bad "a sibling repo's copy replaced the original snapshot — the original is lost"
fi

# ── 4. No snapshot when there was nothing to lose ───────────────────────────────
reset_sandbox
run_install >/dev/null
if [[ -f "$TMP/home/.claude/CLAUDE.md.pre-bootstrap.bak" ]]; then
  bad "created a pointless .bak on a cold install"
else
  ok "no .bak on a cold install (nothing was at risk)"
fi

# ── 5. Idempotent: repeated runs are stable and quiet ──────────────────────────
reset_sandbox
run_install >/dev/null
sum1="$(cksum < "$TMP/home/.claude/CLAUDE.md")"
rc="$(run_install)"
sum2="$(cksum < "$TMP/home/.claude/CLAUDE.md")"
check "exit 0 on a repeat run" "$rc" "0"
check "content stable across runs" "$sum1" "$sum2"
# "quiet" was in this section's heading from the start but nothing asserted it, so a change that made
# the snapshot notice fire on EVERY run would have passed all 18 assertions. Now it cannot.
[[ ! -s "$TMP/err" ]] && ok "a repeat run is silent on stderr" \
                      || bad "repeat run was not quiet: $(tr '\n' ' ' < "$TMP/err")"

# ── 5b. FAIL CLOSED when the snapshot cannot be taken ──────────────────────────
# The original must never be destroyed just because we could not preserve it. A DANGLING SYMLINK at
# $backup is the cheapest reproducer: `-e` is false for it, so the old `! -e "$backup"` guard let the
# snapshot proceed, `cp -p` then failed ENOENT, `2>/dev/null` ate the error and `cp -f` destroyed the
# original with rc=0 and an empty stderr.
reset_sandbox
mkdir -p "$TMP/home/.claude"
printf 'THE ONLY COPY OF THE DEVELOPER FRAMEWORK\n' > "$TMP/home/.claude/CLAUDE.md"
ln -s "$TMP/nonexistent-dir/x" "$TMP/home/.claude/CLAUDE.md.pre-bootstrap.bak"
rc="$(run_install)"
check "exit 0 when the snapshot cannot be taken" "$rc" "0"
if grep -q 'ONLY COPY' "$TMP/home/.claude/CLAUDE.md" 2>/dev/null; then
  ok "original left UNTOUCHED when its snapshot fails (fail-closed)"
else
  bad "destroyed the original after a FAILED snapshot — the safety net failed open"
fi
[[ -s "$TMP/err" ]] && ok "says so loudly on stderr when it refuses" \
                    || bad "refused silently — indistinguishable from success"

# ── 5c. NEVER write through a symlinked target ─────────────────────────────────
# `cp -f` follows a symlinked destination. A dotfiles-managed ~/.claude/CLAUDE.md is an ordinary
# layout, and writing through it puts the repo copy in a git-tracked tree OUTSIDE ~/.claude, which the
# script's own scope note promises never happens.
reset_sandbox
mkdir -p "$TMP/home/.claude" "$TMP/dotfiles"
printf 'MY DOTFILES MASTER COPY\n' > "$TMP/dotfiles/claude-md"
ln -s "$TMP/dotfiles/claude-md" "$TMP/home/.claude/CLAUDE.md"
run_install >/dev/null
if grep -q 'MY DOTFILES MASTER COPY' "$TMP/dotfiles/claude-md"; then
  ok "external symlink target is NOT written through"
else
  bad "wrote through the symlink into $TMP/dotfiles — outside ~/.claude"
fi
[[ -L "$TMP/home/.claude/CLAUDE.md" ]] && bad "left the target a symlink — the repo copy did not land" \
                                       || ok "replaced the symlink with a real file in ~/.claude"
if diff -q "$HERE/CLAUDE-global.md" "$TMP/home/.claude/CLAUDE.md" >/dev/null 2>&1; then
  ok "and that real file is the repo copy"
else
  bad "target is not the repo copy after replacing a symlink"
fi

# ── 6. ONE-DIRECTIONAL: never copies anything OUT of ~/.claude into the repo ────
# ~/.claude.json holds the oauth account, userID and machineID, and this repo is PUBLIC.
reset_sandbox
printf '{"oauthAccount":"MUST-NEVER-BE-COPIED"}\n' > "$TMP/home/.claude.json"
mkdir -p "$TMP/home/.claude"
printf 'SECRET-SESSION-STATE\n' > "$TMP/home/.claude/.credentials.json"
run_install >/dev/null
if grep -rq 'MUST-NEVER-BE-COPIED\|SECRET-SESSION-STATE' "$TMP/proj" 2>/dev/null; then
  bad "copied data OUT of ~/.claude into the project tree"
else
  ok "nothing from ~/.claude was copied into the project tree"
fi
# Grep EXECUTABLE lines only. The header deliberately describes the copy-out block that
# must never return, so scanning the whole file matches its own warning — that false
# positive was the first thing this assertion caught upstream, in itself.
if sed 's/#.*//' "$SCRIPT" | grep -qE 'cp .*(-R|-r).*\.claude|claude-bundle|force-with-lease'; then
  bad "install.sh contains a copy-out / bundle-publish pattern in executable code"
else
  ok "no copy-out or bundle-publish pattern in executable code"
fi

# ── 7. Not fatal when var/claude cannot be created ─────────────────────────────
# The hook must never abort a SessionStart over a convenience mkdir. rent-watch's suite
# used `chmod 500` on the project dir to provoke this; that is VACUOUS when the tests run
# as root (root ignores the mode bits), which is exactly how they run in this container —
# verified: `chmod 500 dir && mkdir dir/x` succeeds as uid 0. A path whose parent is a
# regular FILE fails with ENOTDIR for every uid including root, so that is what we use.
reset_sandbox
printf 'not a directory\n' > "$TMP/proj-file"
rc="$(HOME="$TMP/home" CLAUDE_PROJECT_DIR="$TMP/proj-file" bash "$SCRIPT" >"$TMP/out" 2>"$TMP/err"; printf '%s' $?)"
check "exit 0 when var/claude cannot be created (ENOTDIR)" "$rc" "0"
# Converse: the three docs still landed. A non-fatal mkdir is only correct if the work
# that matters completed — an early `exit 0` would pass the line above and install nothing.
[[ -f "$TMP/home/.claude/CLAUDE.md" ]] && ok "docs still installed despite the mkdir failure" \
                                       || bad "mkdir failure prevented the doc install"

echo
echo "$PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
