#!/usr/bin/env bash
# Run by the claude.ai cloud environment's SETUP SCRIPT (never by anything in phorj): registers ONE SessionStart
# hook in this container's ~/.claude/settings.json that runs install.sh --hook (DEC-570). The setup script cannot
# read the environment's secrets, so decryption happens later, inside the session, where PHORJ_CLOUD_KIT_KEY is
# visible. Setup-script line:  bash phorj/tools/cloud-kit/arm.sh
# Refuses (writes nothing) unless the cloud proxy dir exists. Re-running adds nothing.
set -euo pipefail
KIT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ -d "${CLOUD_KIT_CCR_DIR:-/root/.ccr}" ]] \
  || { echo "arm.sh: refusing — not a claude.ai cloud container; nothing was touched." >&2; exit 3; }
mkdir -p "$HOME/.claude"
python3 - "$HOME/.claude/settings.json" "$KIT_DIR/install.sh" <<'PY'
import json, os, sys
sp, inst = sys.argv[1], sys.argv[2]
s = json.load(open(sp)) if os.path.exists(sp) else {}
cmd = 'bash "%s" --hook' % inst
groups = s.setdefault("hooks", {}).setdefault("SessionStart", [])
if not any("tools/cloud-kit/install.sh" in h.get("command", "") for g in groups for h in g.get("hooks", [])):
    groups.append({"matcher": "", "hooks": [{"type": "command", "command": cmd, "timeout": 120}]})
    with open(sp, "w") as f: json.dump(s, f, indent=2); f.write("\n")
PY
echo "arm.sh: SessionStart hook registered — the kit installs when the session starts."
