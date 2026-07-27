#!/usr/bin/env bash
# Apply a pending .claude/settings.json change that Claude cannot write itself.
#
# WHY THIS EXISTS: Claude Code's safety classifier blocks Claude from writing `.claude/settings.json`
# (self-modification of its own permission surface). In a remote container the developer has no
# terminal either, so the hand-over has to travel through the repo: Claude commits
# `settings.json.pending`, the developer pulls, runs THIS script on their machine, and pushes the
# result back. Claude then pulls to re-sync.
#
# NOT A LASTING DUPLICATE: on success this script DELETES settings.json.pending, so the repo never
# carries two copies of the settings (Invariant 19 — no divergent artifact).
#
# Usage:  bash scripts/claude-bootstrap/apply-pending-settings.sh
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
pending="$here/settings.json.pending"
target="$repo/.claude/settings.json"

cd "$repo"

if [[ ! -f "$pending" ]]; then
  echo "Nothing to apply — $pending does not exist (already applied?)."
  exit 0
fi

# Validate BEFORE touching the live file — a malformed settings.json breaks every future session.
python3 -c "import json,sys; json.load(open(sys.argv[1])); print('pending settings: valid JSON')" "$pending"

if [[ -f "$target" ]]; then
  backup="$target.bak.$(date +%s)"
  cp -a "$target" "$backup"
  echo "backup: ${backup#"$repo"/}"
fi

mkdir -p "$repo/.claude"
cp -f "$pending" "$target"
python3 -c "import json,sys; json.load(open(sys.argv[1])); print('applied settings: valid JSON')" "$target"
rm -f "$pending"

echo
echo "--- diff vs HEAD -------------------------------------------------------"
git --no-pager diff --stat -- .claude/settings.json scripts/claude-bootstrap/settings.json.pending || true
echo "-----------------------------------------------------------------------"
echo
echo "Applied. Review the diff, then:"
echo "    git add .claude/settings.json scripts/claude-bootstrap/settings.json.pending"
echo "    git commit -m 'chore(claude): apply the DEC-354 allow-list-only settings + PreCompact hook'"
echo "    git push origin master"
echo
echo "Nothing was staged, committed or pushed by this script."
