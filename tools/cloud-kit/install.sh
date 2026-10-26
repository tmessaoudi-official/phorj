#!/usr/bin/env bash
# Install the developer's global Claude framework + project memories into THIS container's ~/.claude —
# claude.ai CLOUD sessions only (plan docs/plans/2026-10-26-cloud-kit.plan.md, DEC-570).
#
# On the developer's PC this file is inert: nothing in phorj references it, and it refuses to run unless
# every cloud marker holds (CLAUDE_CODE_REMOTE=true AND the cloud proxy dir /root/.ccr exists; the
# encrypted kit also needs PHORJ_CLOUD_KIT_KEY). A refusal writes nothing.
#
#   install.sh [--kit FILE.age] [--no-firewall]      decrypt tools/cloud-kit/kit.tar.age (default) and install
#   install.sh --from-plain GLOBAL.tgz MEMORIES.tgz  install from the developer's two plaintext exports
#   install.sh --hook                                 SessionStart mode: never fails the session (exit 0, logs)
#
# Re-runnable: a second run with the same inputs changes nothing. Test seams (tests only):
# CLOUD_KIT_CCR_DIR (default /root/.ccr), CLOUD_KIT_PROJECTS_ROOT (default /home/user).
set -euo pipefail

KIT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CCR_DIR="${CLOUD_KIT_CCR_DIR:-/root/.ccr}"
PROJECTS_ROOT="${CLOUD_KIT_PROJECTS_ROOT:-/home/user}"
CH="$HOME/.claude"

hook=0; firewall=1; kit="$KIT_DIR/kit.tar.age"; plain_g=""; plain_m=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --hook) hook=1 ;;
    --no-firewall) firewall=0 ;;
    --kit) kit="$2"; shift ;;
    --from-plain) plain_g="$2"; plain_m="$3"; shift 2 ;;
    *) echo "cloud-kit: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done

if [[ $hook -eq 1 ]]; then
  # A SessionStart hook must never block or fail the session. Off-cloud it does nothing at all — no log,
  # no directory — so even a stray hook entry on a non-cloud machine leaves no trace.
  [[ "${CLAUDE_CODE_REMOTE:-}" == "true" && -d "$CCR_DIR" ]] || exit 0
  mkdir -p "$CH/logs"
  if out="$(bash "${BASH_SOURCE[0]}" 2>&1)"; then
    echo "cloud-kit: global framework + memories installed for this cloud session."
  else
    printf '%s | ERROR | cloud-kit | %s\n' "$(date '+%Y-%m-%dT%H:%M:%S')" "$(tail -1 <<<"$out")" \
      >>"$CH/logs/hooks-errors.log" 2>/dev/null || true
  fi
  exit 0
fi

refuse() { echo "cloud-kit: refusing — $1; not a claude.ai cloud session, nothing was touched." >&2; exit 3; }
[[ "${CLAUDE_CODE_REMOTE:-}" == "true" ]] || refuse "CLAUDE_CODE_REMOTE is not 'true'"
[[ -d "$CCR_DIR" ]] || refuse "the cloud proxy dir $CCR_DIR is absent"
command -v python3 >/dev/null || { echo "cloud-kit: python3 is required" >&2; exit 4; }

W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
if [[ -n "$plain_g" ]]; then
  cp "$plain_g" "$W/global.tar.gz"; cp "$plain_m" "$W/memories.tar.gz"
else
  [[ -n "${PHORJ_CLOUD_KIT_KEY:-}" ]] || { echo "cloud-kit: PHORJ_CLOUD_KIT_KEY is not set — add the age private key in the cloud environment's settings; nothing was touched." >&2; exit 3; }
  [[ -f "$kit" ]] || { echo "cloud-kit: no kit at $kit; nothing was touched." >&2; exit 5; }
  command -v age >/dev/null || { echo "cloud-kit: age is not installed; nothing was touched." >&2; exit 4; }
  (umask 077; printf '%s\n' "$PHORJ_CLOUD_KIT_KEY" >"$W/key")
  age -d -i "$W/key" -o "$W/kit.tar" "$kit"
  rm -f "$W/key"
  tar -xf "$W/kit.tar" -C "$W" global.tar.gz memories.tar.gz
fi

# Everything below lives in one python program so that the safety checks run before any write.
python3 - "$W" "$CH" "$PROJECTS_ROOT" "$firewall" "$KIT_DIR" <<'PY'
import hashlib, json, os, re, shutil, sys, tarfile

W, CH, PROOT, FIREWALL, KIT_DIR = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4] == "1", sys.argv[5]

def safe_extract(tgz, dest):
    with tarfile.open(tgz) as t:
        for m in t.getmembers():
            n = os.path.normpath(m.name)
            if n.startswith("/") or n == ".." or n.startswith("../") or not (m.isfile() or m.isdir()):
                sys.exit("cloud-kit: unsafe archive member %r in %s; nothing was touched." % (m.name, os.path.basename(tgz)))
        t.extractall(dest, filter="data")

g, m = os.path.join(W, "g"), os.path.join(W, "m")
safe_extract(os.path.join(W, "global.tar.gz"), g)
safe_extract(os.path.join(W, "memories.tar.gz"), m)
roots = [d for d in os.listdir(g) if os.path.isdir(os.path.join(g, d, "global"))]
if len(roots) != 1: sys.exit("cloud-kit: the global export must hold exactly one <bundle>/global dir")
G = os.path.join(g, roots[0], "global")

def write_if_changed(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if os.path.exists(path) and open(path, "rb").read() == data: return
    with open(path, "wb") as f: f.write(data)

def copy_tree(src, dst, skip=()):
    for dirpath, dirs, files in os.walk(src):
        rel = os.path.relpath(dirpath, src)
        dirs[:] = [d for d in dirs if os.path.normpath(os.path.join(rel, d)) not in skip]
        for f in files:
            s = os.path.join(dirpath, f); d = os.path.normpath(os.path.join(dst, rel, f))
            write_if_changed(d, open(s, "rb").read())
            if os.access(s, os.X_OK): os.chmod(d, 0o755)

# ── framework files (never the harness's own: settings.json, launcher-settings.json, stop-hook-*, uploads) ──
for sub, skip in (("skills", ("loop",)), ("bin", ()), ("hooks", ()), ("refs", ()), ("rules", ()),
                  ("templates", ()), (".claude", ())):
    if os.path.isdir(os.path.join(G, sub)): copy_tree(os.path.join(G, sub), os.path.join(CH, sub), skip)
for f in ("THINKING.md", "BLAST-RADIUS.md"):
    if os.path.exists(os.path.join(G, f)): write_if_changed(os.path.join(CH, f), open(os.path.join(G, f), "rb").read())

CLOUD_BLOCK = """# Cloud adaptation — this container only (installed by phorj tools/cloud-kit, DEC-570)

- This copy of the framework runs in a claude.ai cloud container; the developer's PC holds the original.
  On any conflict the project's own CLAUDE.md wins (for phorj: § Git autonomy — cloud work goes to the
  session branch only, the developer's identity, unsigned, no trailers).
- Paths differ: no /stack; projects sit at /home/user/<name>, slug -home-user-<name> (PC: -stack-projects-<name>).
- Absent here: rtk, the session-remember pipeline, headless `claude -p` handoffs, the statusline, MCP servers,
  plugins (superpowers: apply TDD directly), the docker daemon, the permission engine. permissions.deny is
  empty in the cloud (developer ruling 2026-10-26).
- Memory written here dies with the container. At milestones and before stopping, run
  `bash /home/user/phorj/tools/cloud-kit/memory-delta.sh` and commit the encrypted delta it prints.

"""
body = open(os.path.join(G, "CLAUDE.md")).read()
body = "".join(l for l in body.splitlines(True) if not re.match(r"@RTK(-local)?\.md\s*$", l))
write_if_changed(os.path.join(CH, "CLAUDE.md"), (CLOUD_BLOCK + body).encode())

# The firewall treats HOME as host-critical; in the cloud HOME=/root sits in HOST_DIRS, which would deny every
# write under ~ — including the framework's own safe zones. Patch the cloud copy only. The patched copy was
# payload-tested in auto mode (2026-10-26: scratchpad, memory, branch push, cargo allowed; force-push and
# key reads denied; settings.json asks), so it is wired by default; --no-firewall leaves it out.
fw = os.path.join(CH, "hooks", "ask-bash-firewall.sh")
if os.path.exists(fw):
    src = open(fw).read()
    patch = "HOST_DIRS = [d for d in HOST_DIRS if d != HOME]   # cloud-kit: HOME=/root here (DEC-570)\n"
    if patch not in src:
        src = re.sub(r"(?m)^(HOST_DIRS = \[.*\]\n)", lambda mo: mo.group(1) + patch, src, count=1)
        write_if_changed(fw, src.encode())

for d in ("logs", "run", "state"): os.makedirs(os.path.join(CH, d), exist_ok=True)

# ── settings.json: merge the cloud subset of the template into what the harness already wrote ──
ENABLE = ("session-start-banner.sh", "plan-write-check.sh", "edit-log.sh", "ask-human-gate-track.sh",
          "ask-human-question-guard.sh", "keep-going-guard.sh", "advisor-completion-guard.sh",
          "subagent-stop-status.sh", "backup-before-write.sh", "ask-human-gate-block.sh",
          "advisor-prework-guard.sh", "skill-invocation-log.sh", "skill-model-notice.sh",
          "agent-model-override.sh", "expertise-gate.sh", "ask-human-gate-arm.sh", "remote-bang.sh",
          "subagent-start-context.sh") + (("ask-bash-firewall.sh",) if FIREWALL else ())
tpl = json.load(open(os.path.join(G, "settings.json.template")))
sp = os.path.join(CH, "settings.json")
cur = json.load(open(sp)) if os.path.exists(sp) else {}
hooks = {ev: g for ev, g in cur.get("hooks", {}).items()}
for ev, groups in tpl.get("hooks", {}).items():
    for grp in groups:
        keep = [h for h in grp.get("hooks", []) if any(n in h.get("command", "") for n in ENABLE)]
        if not keep: continue
        new = dict(grp, hooks=keep)
        if new not in hooks.setdefault(ev, []): hooks[ev].append(new)
cur["hooks"] = hooks
perms = cur.get("permissions", {}); perms.pop("deny", None)
if perms: cur["permissions"] = perms
else: cur.pop("permissions", None)
write_if_changed(sp, (json.dumps(cur, indent=2) + "\n").encode())

# ── project memories: entries/<key>/ → projects/<cloud slug>/ + a manifest for memory-delta.sh ──
mp = json.load(open(os.path.join(m, "map.json")))
kitmap = []
for e in mp.get("entries", []):
    src = os.path.join(m, "entries", e["key"])
    if not os.path.isdir(src): continue
    slug = os.path.join(PROOT, e["name"]).replace("/", "-")
    dst = os.path.join(CH, "projects", slug)
    copy_tree(src, dst)
    man = []
    mem = os.path.join(dst, "memory")
    for dirpath, _, files in os.walk(mem):
        for f in sorted(files):
            p = os.path.join(dirpath, f)
            man.append("%s  %s" % (hashlib.sha256(open(p, "rb").read()).hexdigest(), os.path.relpath(p, dst)))
    manp = os.path.join(dst, ".cloud-kit-manifest")
    if not os.path.exists(manp):   # the baseline is the FIRST install; memory-delta.sh re-baselines after each delta
        write_if_changed(manp, ("\n".join(sorted(man)) + "\n").encode())
    kitmap.append("%s\t%s" % (slug, e.get("slug", "")))
write_if_changed(os.path.join(CH, ".cloud-kit", "map.tsv"), ("\n".join(kitmap) + "\n").encode())
print("cloud-kit: installed (%d project memory sets, firewall %s)" % (len(kitmap), "on" if FIREWALL else "off"))
PY

# Autonomous mode for phorj, through the framework's own tool (never a hand-written rules file).
# A failure here is reported, not fatal: the framework is installed either way, and the session then
# simply starts in spec mode (every gate armed), which is the framework's own safe default.
if [[ -x "$CH/bin/gates-bypass.sh" && -d "$PROJECTS_ROOT/phorj" ]]; then
  if ! mode_out="$(cd "$PROJECTS_ROOT/phorj" && bash "$CH/bin/gates-bypass.sh" --run --yes 2>&1)"; then
    echo "cloud-kit: WARNING — autonomous mode not set, the session stays in spec mode: $mode_out" >&2
  fi
fi
