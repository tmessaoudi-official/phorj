#!/usr/bin/env bash
# phorj Claude-container bootstrap — restores the developer's global reasoning framework into the
# EPHEMERAL remote container (fresh ~/.claude every session), so the project CLAUDE.md's routing
# reference ("the global reasoning framework, ~/.claude/CLAUDE.md") resolves everywhere.
#
# Idempotent + conservative: `cp -u` copies only when the repo copy is NEWER than the target, so a
# user's hand-edited (newer) ~/.claude file is never clobbered. Silent no-op when already current.
# Wired as a SessionStart hook in .claude/settings.json; safe to run by hand any time.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dest="${HOME}/.claude"
mkdir -p "$dest"

cp -u "$here/CLAUDE-global.md" "$dest/CLAUDE.md"
cp -u "$here/THINKING.md" "$dest/THINKING.md"
cp -u "$here/BLAST-RADIUS.md" "$dest/BLAST-RADIUS.md"

# The repo-native skills (.claude/skills/*) need no install — Claude Code reads them in place.
exit 0
