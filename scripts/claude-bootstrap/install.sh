#!/usr/bin/env bash
# phorj Claude-container bootstrap — restores the developer's global reasoning framework into the
# EPHEMERAL remote container (a fresh ~/.claude every session), so the project CLAUDE.md's routing
# reference ("the global reasoning framework, ~/.claude/CLAUDE.md") resolves everywhere.
#
# THE REPO IS ALWAYS THE TRUTH (developer ruling, 2026-08-06). The three docs below are copied
# UNCONDITIONALLY on every run. Idempotent — the same bytes land every time — and deterministic,
# which is the point of the ruling. `scripts/claude-bootstrap/test-install.sh` pins the contract.
#
# This replaced `cp -u`, whose header used to claim "a hand-edited (newer) ~/.claude file on a real
# workstation is never clobbered". That claim was FALSE, and the behaviour was nondeterministic:
# `cp -u` copies when the SOURCE is newer, and a fresh `git clone` stamps every file with the clone
# time — so in this container it clobbered anyway, while after a hand-edit of the target it silently
# did nothing and the repo stopped being the truth. Neither outcome was chosen; both depended on
# mtimes nobody was tracking.
#
# The one thing unconditional copying must not do is destroy a global framework with no way back, so
# a file that predates this hook is snapshotted ONCE to <name>.pre-bootstrap.bak and never touched
# again. That is a safety net, not a second source of truth: it is never read back, and the repo
# still wins every session.
#
# Wired as a SessionStart hook in .claude/settings.json; safe to run by hand.
#
# SCOPE IS DELIBERATELY NARROW: this script copies three documentation files INTO ~/.claude and does
# nothing else. It must never copy anything OUT of ~/.claude into the repo — `~/.claude.json` holds
# the oauth account, userID and machineID, and this repo is PUBLIC and one `git add -A` away from
# history. A commented-out block doing exactly that (copy `/root/.claude{,.json}` into
# `claude-bundle/`, then `git add` + `commit` + `push --force-with-lease`) sat here until 2026-08-06
# and was deleted, not re-commented: a disabled credential-exfiltration path inside a SessionStart
# hook is one uncomment away from publishing the developer's tokens. [Verified it never ran: no
# `claude-bundle` directory and no `*.claude.json` appears anywhere in this repo's history.] All four
# sibling repos removed the same block for the same reason.
#
# The repo-native skills (.claude/skills/*) and agents (.claude/agents/*) need NO install — Claude
# Code reads them in place from the clone.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dest="${HOME}/.claude"

# Structured logging for the two fail-closed paths below (global Rule 13). Guarded: a missing helper
# must not abort a SessionStart hook, so log_obs degrades to a no-op rather than an unbound command.
# shellcheck source=/dev/null
. "$here/hooks/log-helpers.sh" 2>/dev/null || log_obs() { :; }

mkdir -p "$dest"

# install_doc <repo-source> <target-name>
#   1. If the target exists, predates this hook, and differs from what we are about to write, take a
#      one-time snapshot. "Predates this hook" is inferred from the absence of the snapshot itself —
#      once it exists we never write it again, so a later run cannot overwrite the original with our
#      own copy. That ordering is the whole trick, and test-install.sh asserts the converse: all five
#      sibling repos ship this same hook, so a rent-watch session installs ITS CLAUDE-global.md over
#      ours, and on the next phorj session the target differs from our source again. Without the
#      `! -e` guard we would then snapshot rent-watch's copy on top of the irreplaceable original.
#   2. Copy unconditionally. The repo is the truth.
#
# THREE CORRECTIONS, 2026-08-06, all found by the DEC-268 panel reviewing the first draft of this very
# function. Each was a way for "the repo is the truth" to mean "your file is gone":
#
#   (a) FAIL CLOSED ON A FAILED SNAPSHOT. The first draft ran `cp -p … 2>/dev/null && printf …` and then
#       copied unconditionally, so a snapshot that FAILED was indistinguishable from one that succeeded
#       and the original was destroyed with an empty stderr and rc=0. Reproduced three ways: ENOSPC; a
#       root-owned `~/.claude` directory with a world-writable file inside (overwriting an existing file
#       needs no directory write permission, so `cp -f` succeeds exactly where the snapshot cannot); and
#       a DANGLING SYMLINK at `$backup`, which passes `! -e` and then fails ENOENT on the copy. In the
#       second case the only diagnostic printed named a DIFFERENT file. There is no acceptable
#       `2>/dev/null` here: if we cannot preserve the original we do not touch it.
#   (b) NEVER WRITE THROUGH A SYMLINK. `cp -f` follows a symlinked destination, so a dotfiles-managed
#       `~/.claude/CLAUDE.md -> ~/dotfiles/claude-md` meant this hook wrote 62 KB into the developer's
#       dotfiles tree — outside `~/.claude` entirely, falsifying the scope promise above, every session,
#       and silently from the second run on once the `! -e "$backup"` guard suppressed the notice.
#       `--remove-destination` replaces the link instead of following it; the external file is untouched.
#   (c) The existence tests must catch a symlink, dangling or not — hence `-e … || -L …`, since `-e` is
#       false for a dangling link and `-f` is false for a link to a directory.
install_doc() {
  local src="$1" name="$2"
  local target="$dest/$name"
  local backup="$target.pre-bootstrap.bak"

  refuse() {
    log_obs ERROR install.sh "could not snapshot $name ($1) — REFUSING to overwrite it (fail-closed)"
    printf 'claude-bootstrap: could NOT back up your existing %s (%s) — leaving it UNTOUCHED.\n' \
      "$name" "$1" >&2
    printf 'claude-bootstrap:   (%s is therefore NOT the repo copy this session)\n' "$target" >&2
  }

  # Only the "target exists AND differs from what we are about to write" case can lose anything.
  if { [[ -e "$target" || -L "$target" ]]; } && ! cmp -s "$src" "$target"; then
    if [[ -e "$backup" ]]; then
      : # A usable snapshot is already there. Snapshot ONCE — never rewrite it. Proceed to copy.
    elif [[ -L "$backup" ]]; then
      # A DANGLING symlink occupying the backup slot. `-e` is false for it, so an `! -e` guard would
      # let the snapshot proceed and `cp` would fail ENOENT; but `! -L` is equally wrong, because it
      # reads a broken slot as a valid snapshot and proceeds to destroy the original. Neither test
      # alone is safe: we can neither write the snapshot nor trust what is there, so we refuse.
      refuse "backup path is a dangling symlink"
      return 0
    # `-P` preserves symlink-ness: when the target IS a link, the irreplaceable thing is the link and
    # where it pointed, not the content it happens to reach today.
    elif cp -P -p "$target" "$backup"; then
      printf 'claude-bootstrap: kept your previous %s as %s\n' "$name" "${backup##*/}" >&2
    else
      refuse "snapshot copy failed"
      return 0
    fi
  fi

  if ! cp -f --remove-destination "$src" "$target"; then
    log_obs ERROR install.sh "failed to install $name to $target"
    printf 'claude-bootstrap: failed to install %s — see %s\n' "$name" "$target" >&2
  fi
}

install_doc "$here/CLAUDE-global.md" CLAUDE.md
install_doc "$here/THINKING.md"      THINKING.md
install_doc "$here/BLAST-RADIUS.md"  BLAST-RADIUS.md

# var/claude/ is the in-repo, gitignored (`/var`) home for everything the review skills and the
# PreCompact handoff hook write. Created here so a skill never has to guess whether it exists.
#
# `|| true` is deliberate and evidenced, not a reflex (global Rule 14 / the anti-bandaid gate): this
# runs LAST, after the three docs are already in place, and a SessionStart hook that exits non-zero
# reports a failed bootstrap for a directory that is pure convenience. MEASURED, 2026-08-06: with
# CLAUDE_PROJECT_DIR pointing at a path whose parent is a regular file, `mkdir -p` fails ENOTDIR and
# `set -e` took the whole hook down with exit 1 — test-install.sh assertion 7 reproduced it, and its
# converse asserts the docs still land. (ENOTDIR rather than a permission denial because these tests
# run as uid 0 in this container, where `chmod 500` on a directory is a no-op and the mode-based
# version of this test is vacuous.)
mkdir -p "${CLAUDE_PROJECT_DIR:-$here/../..}/var/claude" 2>/dev/null || true

exit 0
