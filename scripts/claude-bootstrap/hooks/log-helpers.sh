#!/usr/bin/env bash
# Shared observability helper (project CLAUDE.md / global Rule 13). Source this from hooks that need
# structured logging. Never fatal — every write ends in `|| true`, because a logging failure must not
# take down the hook that is logging.
#
# Format: YYYY-MM-DDTHH:MM:SS±ZZ | LEVEL | script | message
# Destination: $OBS_LOG, else ~/.claude/logs/hooks-errors.log

umask 077  # hook-created files are owner-only

log_obs() {
  local level="$1" script="$2"; shift 2
  local dest="${OBS_LOG:-$HOME/.claude/logs/hooks-errors.log}"
  mkdir -p "$(dirname "$dest")" 2>/dev/null || true
  printf '%s | %s | %s | %s\n' "$(date -Iseconds)" "$level" "$script" "$*" >> "$dest" 2>/dev/null || true
}
