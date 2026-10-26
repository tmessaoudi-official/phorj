#!/usr/bin/env bash
# Package the memory written in THIS cloud session for the developer's PC (DEC-570): every memory file that is
# new or changed since the last baseline, renamed back to its PC slug (e.g. -stack-projects-phorj), tarred and
# age-encrypted to the developer's public key. Then the baseline moves forward, so the next delta holds only
# what changed after this one.
#   memory-delta.sh [--out DIR]     default DIR: tools/cloud-kit/memory-deltas
# On the PC:  age -d -i ~/.config/phorj-cloud-kit.key DELTA.age | tar -x -C ~/.claude/projects
# Recipient: $CLOUD_KIT_RECIPIENT, else tools/cloud-kit/recipient.txt. Cloud only, like install.sh.
set -euo pipefail
KIT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="$KIT_DIR/memory-deltas"
[[ "${1:-}" == "--out" ]] && out="$2"
[[ "${CLAUDE_CODE_REMOTE:-}" == "true" && -d "${CLOUD_KIT_CCR_DIR:-/root/.ccr}" ]] \
  || { echo "memory-delta: refusing — not a claude.ai cloud session; nothing was touched." >&2; exit 3; }
recip="${CLOUD_KIT_RECIPIENT:-$(cat "$KIT_DIR/recipient.txt" 2>/dev/null || true)}"
[[ "$recip" == age1* ]] || { echo "memory-delta: no age public key (CLOUD_KIT_RECIPIENT or tools/cloud-kit/recipient.txt)" >&2; exit 2; }
CH="$HOME/.claude"
[[ -f "$CH/.cloud-kit/map.tsv" ]] || { echo "memory-delta: the cloud kit is not installed here" >&2; exit 5; }

W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
python3 - "$CH" "$W/stage" <<'PY'
import hashlib, os, shutil, sys
CH, stage = sys.argv[1], sys.argv[2]
n = 0
for line in open(os.path.join(CH, ".cloud-kit", "map.tsv")):
    if not line.strip(): continue
    cloud, pc = line.rstrip("\n").split("\t")
    base = os.path.join(CH, "projects", cloud)
    manp = os.path.join(base, ".cloud-kit-manifest")
    old = {}
    if os.path.exists(manp):
        for l in open(manp):
            if l.strip(): h, p = l.rstrip("\n").split("  ", 1); old[p] = h
    new = {}
    for dirpath, _, files in os.walk(os.path.join(base, "memory")):
        for f in files:
            p = os.path.join(dirpath, f); rel = os.path.relpath(p, base)
            new[rel] = hashlib.sha256(open(p, "rb").read()).hexdigest()
            if old.get(rel) != new[rel]:
                d = os.path.join(stage, pc or cloud, rel); os.makedirs(os.path.dirname(d), exist_ok=True)
                shutil.copy2(p, d); n += 1
    with open(manp + ".next", "w") as f: f.write("\n".join("%s  %s" % (new[k], k) for k in sorted(new)) + "\n")
print(n)
PY
if [[ ! -d "$W/stage" ]]; then
  echo "memory-delta: no memory changed since the last baseline; nothing written."
  find "$CH/projects" -name '.cloud-kit-manifest.next' -delete
  exit 0
fi
mkdir -p "$out"
f="$out/$(date -u +%Y%m%dT%H%M%SZ).age"
tar -cf "$W/delta.tar" -C "$W/stage" .
age -r "$recip" -o "$f" "$W/delta.tar"
# Move each baseline forward only once the encrypted delta exists.
find "$CH/projects" -name '.cloud-kit-manifest.next' | while read -r n; do mv "$n" "${n%.next}"; done
echo "memory-delta: wrote $f — commit it on the session branch."
