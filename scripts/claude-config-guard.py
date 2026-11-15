#!/usr/bin/env python3
"""Guard for the Claude configuration of this repository (ground-prep plan row 7, 2026-11-15).

Every failure it looks for is silent: nothing errors, a rule just stops loading or a citation just
stops being true. Four checks, each proven able to fire by scripts/test-claude-config-guard.sh:

  C1  .claude/rules/*.md - a path-scoped file opens with frontmatter that closes, has a non-empty
      `paths:` list whose every glob matches a tracked file, and is named in CLAUDE.md; the files
      in ALWAYS_LOADED have NO frontmatter (a rules file without it loads in every session, so the
      set is a decision, listed here by name); every rules file CLAUDE.md names is tracked.
  C2  quoted anchors in CLAUDE.md, the rules files, the skills and the agents:
        file ("phrase")   file (§ "Heading")   file ("phrase"; § "Heading"; ...)
      each item resolves on EXACTLY ONE line of the target (a heading item: one line starting
      with #). An anchor may wrap in the citing file; the phrase may not wrap in the target.
  C3  line cites (file:12, file:12-20) - a shifted cite still resolves, so they are a ratchet:
      per citing file the count may shrink and never grow (scripts/claude-config-baseline.txt,
      rows `C3<TAB>count<TAB>path`); write a new citation as a quoted anchor. A cite past the
      end of a tracked target fails outright.
  C4  numbers stated in always-loaded and root files that are derivable (CLAIMS below): the
      crate total, the default-build crate count, the count of non-domain skills. A claim whose
      wording is gone FAILS: reword the claim and its row together, or the row guards nothing.

"Tracked" is read from the git index, never from the disk, so the verdict is the same on the
developer's machine, on CI and in a cloud session. A target that is outside the repository
(memory/..., ~/..., an absolute path) or gitignored is counted as skipped, never failed and never
reported as resolved.

  --only PATH       report only findings that involve PATH (the write-time hook's view)
  --emit-baseline   rewrite the C3 baseline; it lowers rows and refuses to raise one
  --root DIR        repository root (default: the parent of this script's directory)
Exit: 0 clean, 1 findings, 2 usage or not a git repository.
"""
import argparse
import os
import re
import subprocess
import sys

ALWAYS_LOADED = ("expertise-core.md", "north-star.md")
RULES = ".claude/rules/"
BASELINE = "scripts/claude-config-baseline.txt"
PATH = r"(?<![\w/.~-])((?:~/|/)?[\w.-]+(?:/[\w.-]+)*\.[A-Za-z][A-Za-z0-9]{0,5})"
OPENER = re.compile(PATH + r'\s+\((?=(?:§\s*)?")')
ITEM = re.compile(r'\s*(§\s*)?"([^"]{1,300})"\s*([;)])')
LINE_CITE = re.compile(PATH + r":(\d+)(?:-(\d+))?(?!\w)")
WORDS = "ZERO ONE TWO THREE FOUR FIVE SIX SEVEN EIGHT NINE TEN ELEVEN TWELVE".split()
# (what, source of truth, file, pattern with ONE group holding the stated number)
CLAIMS = (
    ("crate total", "Cargo.toml", "CLAUDE.md", r"(\d+) vetted, feature-gated crates"),
    ("crate total", "Cargo.toml", "VISION.md", r"(\d+) admitted, \d+ in the default build"),
    ("crate total", "Cargo.toml", "README.md", r"(\d+) crates, \d+ in the default build"),
    ("crate total", "Cargo.toml", "README.md", r"(\d+) vetted, feature-gated"),
    ("crate total", "Cargo.toml", "SECURITY.md", r"out of (\d+) admitted"),
    ("default-build crate count", "Cargo.toml", "VISION.md", r"\d+ admitted, (\d+) in the default build"),
    ("default-build crate count", "Cargo.toml", "README.md", r"\d+ crates, (\d+) in the default build"),
    ("non-domain skill count", ".claude/skills/", "CLAUDE.md", r"exactly ([A-Z]+) skills"),
)


class Guard:
    def __init__(self, root):
        self.root = root
        res = subprocess.run(["git", "-C", root, "ls-files", "-z"], capture_output=True)
        if res.returncode != 0:
            print("claude-config-guard: %s is not a git repository (the guard reads the index)" % root, file=sys.stderr)
            sys.exit(2)
        self.tracked = set(p for p in res.stdout.decode("utf-8", "replace").split("\0") if p)
        self.findings = []  # (check, where, message, involved paths)
        self.count = dict(rules=0, anchors=0, skipped=0, linecites=0, claims=0)
        self.cache = {}
        self.citing = sorted(
            p for p in self.tracked
            if p == "CLAUDE.md"
            or re.fullmatch(r"\.claude/(rules|agents)/[^/]+\.md|\.claude/skills/[^/]+/SKILL\.md", p))

    def text(self, path):
        if path not in self.cache:
            try:
                with open(os.path.join(self.root, path), encoding="utf-8", errors="replace") as fh:
                    self.cache[path] = fh.read()
            except OSError as exc:  # tracked in the index, gone or unreadable in the working tree
                self.cache[path] = ""
                self.fail("C0", path, "tracked but not readable in the working tree (%s)" % exc.strerror, path)
        return self.cache[path]

    def fail(self, check, where, message, *involved):
        self.findings.append((check, where, message, involved))

    def ignored(self, path):
        cmd = ["git", "-c", "core.excludesFile=/dev/null", "-C", self.root, "check-ignore", "-q", "--", path]
        return subprocess.run(cmd, capture_output=True).returncode == 0

    # ── C1 ────────────────────────────────────────────────────────────────────────────────────
    def rules(self):
        claude = self.text("CLAUDE.md") if "CLAUDE.md" in self.tracked else ""
        files = sorted(p for p in self.tracked if re.fullmatch(re.escape(RULES) + r"[^/]+\.md", p))
        for name in ALWAYS_LOADED:
            if RULES + name not in self.tracked:
                self.fail("C1", RULES + name, "listed as always-loaded but is not a tracked file", RULES + name)
        for path in files:
            self.count["rules"] += 1
            lines = self.text(path).split("\n")
            scoped = lines[0].strip() == "---"
            if os.path.basename(path) in ALWAYS_LOADED:
                if scoped:
                    self.fail("C1", path, "listed as always-loaded but opens with frontmatter", path)
                continue
            if not scoped:
                self.fail("C1", path, "no frontmatter on line 1 (a path-scoped rules file opens with ---; "
                          "a file meant for every session is listed in ALWAYS_LOADED)", path)
                continue
            try:
                end = next(i for i in range(1, len(lines)) if lines[i].strip() == "---")
            except StopIteration:
                self.fail("C1", path, "frontmatter never closes", path)
                continue
            globs = self.paths_of(lines[1:end])
            if not globs:
                self.fail("C1", path, "paths: is empty", path)
            for glob in globs:
                if not any(rx.fullmatch(t) for rx in self.glob_res(glob) for t in self.tracked):
                    self.fail("C1", path, 'glob "%s" matches no tracked file' % glob, path)
            if os.path.basename(path) not in claude:
                self.fail("C1", path, "not named in CLAUDE.md (a path-scoped rule needs its index line)",
                          path, "CLAUDE.md")
        for m in re.finditer(re.escape(RULES) + r"[\w.-]+\.md", claude):
            if m.group(0) not in self.tracked:
                self.fail("C1", "CLAUDE.md", "names %s, which is not a tracked file" % m.group(0),
                          "CLAUDE.md", m.group(0))

    @staticmethod
    def paths_of(front):
        globs, inside = [], False
        for line in front:
            if re.match(r"paths:\s*", line):
                inside = True
                globs += re.findall(r'"([^"]+)"|\'([^\']+)\'', line.split(":", 1)[1])
                globs = [g if isinstance(g, str) else (g[0] or g[1]) for g in globs]
            elif inside and re.match(r"\s*-\s*\S", line):
                globs.append(re.sub(r"^\s*-\s*", "", line).strip().strip("\"'"))
            elif line.strip():
                inside = False
        return globs

    @staticmethod
    def glob_res(glob):
        m = re.search(r"\{([^{}]*)\}", glob)
        alts = [glob[:m.start()] + a + glob[m.end():] for a in m.group(1).split(",")] if m else [glob]
        out = []
        for g in alts:
            rx, i = "", 0
            while i < len(g):
                if g.startswith("**/", i):
                    rx, i = rx + "(?:.*/)?", i + 3
                elif g.startswith("**", i):
                    rx, i = rx + ".*", i + 2
                elif g[i] == "*":
                    rx, i = rx + "[^/]*", i + 1
                elif g[i] == "?":
                    rx, i = rx + "[^/]", i + 1
                else:
                    rx, i = rx + re.escape(g[i]), i + 1
            out.append(re.compile(rx))
        return out

    # ── C2 ────────────────────────────────────────────────────────────────────────────────────
    def external(self, target):
        return target.startswith(("memory/", "~", "/")) or self.ignored(target)

    def anchors(self):
        for src in self.citing:
            body = self.text(src)
            for m in OPENER.finditer(body):
                target, pos = m.group(1), m.end()
                where = "%s:%d" % (src, body.count("\n", 0, m.start()) + 1)
                items = []
                while True:
                    item = ITEM.match(body, pos)
                    if not item:
                        items = None
                        break
                    items.append((bool(item.group(1)), re.sub(r"\s*\n\s*", " ", item.group(2))))
                    pos = item.end()
                    if item.group(3) == ")":
                        break
                if items is None:
                    self.fail("C2", where, "anchor not parsable after `%s (` - write file (\"phrase\"), "
                              "file (§ \"Heading\") or a ;-separated list of those" % target, src, target)
                    continue
                if target not in self.tracked:
                    if self.external(target):
                        self.count["skipped"] += len(items)
                    else:
                        self.fail("C2", where, "target %s is not a tracked file" % target, src, target)
                    continue
                lines = self.text(target).split("\n")
                for heading, phrase in items:
                    self.count["anchors"] += 1
                    hits = sum(1 for ln in lines if phrase in ln and (not heading or ln.lstrip().startswith("#")))
                    if hits != 1:
                        shown = ('§ "%s"' if heading else '"%s"') % phrase
                        self.fail("C2", where, "%s (%s) resolves on %d lines (need exactly 1)"
                                  % (target, shown, hits), src, target)

    # ── C3 ────────────────────────────────────────────────────────────────────────────────────
    def line_cites(self):
        counts = {}
        for src in self.citing:
            body = self.text(src)
            for m in LINE_CITE.finditer(body):
                counts[src] = counts.get(src, 0) + 1
                target, last = m.group(1), int(m.group(3) or m.group(2))
                if target in self.tracked:
                    have = len(self.text(target).splitlines())
                    if last > have:
                        where = "%s:%d" % (src, body.count("\n", 0, m.start()) + 1)
                        self.fail("C3", where, "%s:%s is past the end of the file (%d lines)"
                                  % (target, m.group(2) if not m.group(3) else m.group(2) + "-" + m.group(3), have),
                                  src, target)
        self.count["linecites"] = sum(counts.values())
        return counts

    def baseline(self):
        """-> (rows {path: count}, leading comment lines, ok)."""
        rows, head, ok = {}, [], True
        path = os.path.join(self.root, BASELINE)
        if not os.path.isfile(path):
            return rows, head, ok
        with open(path, encoding="utf-8", errors="replace") as fh:
            for n, raw in enumerate(fh.read().split("\n"), 1):
                if raw.startswith("#") and not rows:
                    head.append(raw)
                    continue
                if not raw.strip() or raw.startswith("#"):
                    continue
                cell = raw.split("\t")
                if len(cell) != 3 or cell[0] != "C3" or not re.fullmatch(r"[0-9]{1,6}", cell[1]) \
                        or not cell[2] or cell[2] in rows:
                    self.fail("C3", "%s:%d" % (BASELINE, n), "malformed row (need C3<TAB>count<TAB>path, "
                              "one row per path, a plain decimal count)", BASELINE)
                    ok = False
                    continue
                rows[cell[2]] = int(cell[1], 10)
        return rows, head, ok

    def ratchet(self, counts):
        rows, _, ok = self.baseline()
        if not ok:
            return
        for src in sorted(set(counts) | set(rows)):
            have, allowed = counts.get(src, 0), rows.get(src, 0)
            if have > allowed:
                self.fail("C3", src, "%d line cites, baseline %d - a line cite drifts silently: write the "
                          "new citation as a quoted anchor, file (\"phrase\")" % (have, allowed), src, BASELINE)
            elif have < allowed:
                self.fail("C3", src, "%d line cites, baseline %d - the ratchet tightens: run "
                          "scripts/claude-config-guard.py --emit-baseline" % (have, allowed), src, BASELINE)

    def emit(self, counts):
        rows, head, ok = self.baseline()
        exists = os.path.isfile(os.path.join(self.root, BASELINE))
        raised = [s for s in counts if exists and counts[s] > rows.get(s, 0)]
        if not ok or raised:
            for src in sorted(raised):
                print("claude-config-guard: refusing to raise %s from %d to %d - the ratchet only tightens; "
                      "a higher row is a hand edit the diff shows" % (src, rows.get(src, 0), counts[src]))
            return 1
        body = head + ["C3\t%d\t%s" % (counts[s], s) for s in sorted(counts) if counts[s]]
        with open(os.path.join(self.root, BASELINE), "w", encoding="utf-8") as fh:
            fh.write("\n".join(body) + "\n")
        print("claude-config-guard: wrote %s (%d rows, %d line cites)" % (BASELINE, len(body) - len(head), sum(counts.values())))
        return 0

    # ── C4 ────────────────────────────────────────────────────────────────────────────────────
    def derive(self):
        """-> {what: number or None}; None when the source could not be read reliably."""
        got = {"non-domain skill count": sum(
            1 for p in self.tracked
            if re.fullmatch(r"\.claude/skills/(?!domain-)[^/]+/SKILL\.md", p))}
        got["crate total"] = got["default-build crate count"] = None
        if "Cargo.toml" not in self.tracked:
            self.fail("C4", "Cargo.toml", "not a tracked file, so no crate claim can be derived", "Cargo.toml")
            return got
        names, optional, section = set(), set(), ""
        cargo = self.text("Cargo.toml")
        for line in cargo.split("\n"):
            t = line.strip()
            if t.startswith("["):
                section = t
            elif t and not t.startswith("#") and (
                    section == "[dependencies]"
                    or (section.startswith("[target.") and section.endswith(".dependencies]"))):
                m = re.match(r"([a-z0-9_-]+)\s*=", t)
                if m:
                    names.add(m.group(1))
                    if re.search(r"optional\s*=\s*true", t):
                        optional.add(m.group(1))
        if len(names) < 10:  # the same floor as tests/dependency_claims.rs: never pass vacuously
            self.fail("C4", "Cargo.toml", "parsed only %d dependencies - the parser broke, so no crate "
                      "claim is checked" % len(names), "Cargo.toml")
            return got
        feats = {}
        block = re.search(r"^\[features\]\n(.*?)(?=^\[|\Z)", cargo, re.S | re.M)
        for m in re.finditer(r"^([A-Za-z0-9_-]+)\s*=\s*\[(.*?)\]", re.sub(r"#.*", "", block.group(1)) if block else "", re.S | re.M):
            feats[m.group(1)] = re.findall(r'"([^"]+)"', m.group(2))
        linked, seen, todo = set(names - optional), set(), ["default"]
        while todo:
            feat = todo.pop()
            if feat in seen:
                continue
            seen.add(feat)
            for item in feats.get(feat, ()):
                if item.startswith("dep:"):
                    linked.add(item[4:])
                elif "/" in item:
                    if "?/" not in item:
                        linked.add(item.split("/")[0])
                elif item in feats:
                    todo.append(item)
                elif item in names:
                    linked.add(item)
        got["crate total"], got["default-build crate count"] = len(names), len(linked & names)
        return got

    def claims(self):
        got = self.derive()
        for what, source, path, pattern in CLAIMS:
            if got[what] is None:
                continue
            self.count["claims"] += 1
            if path not in self.tracked:
                self.fail("C4", path, "not a tracked file, yet a %s claim is expected in it" % what, path, source)
                continue
            body, found = self.text(path), False
            for m in re.finditer(pattern, body):
                found = True
                said = m.group(1)
                value = WORDS.index(said) if said in WORDS else (int(said) if said.isdigit() else -1)
                if value != got[what]:
                    where = "%s:%d" % (path, body.count("\n", 0, m.start()) + 1)
                    self.fail("C4", where, "says %s for the %s, %s gives %d"
                              % (said, what, source.rstrip("/"), got[what]), path, source)
            if not found:
                self.fail("C4", path, "the %s claim was not found (reworded? update CLAIMS in "
                          "scripts/claude-config-guard.py in the same change)" % what, path, source)


def main():
    ap = argparse.ArgumentParser(description="Guard for the Claude configuration (rules files, anchors, cites, claims).")
    ap.add_argument("--root", default=os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    ap.add_argument("--only", metavar="PATH")
    ap.add_argument("--emit-baseline", action="store_true")
    args = ap.parse_args()
    guard = Guard(os.path.abspath(args.root))
    counts = guard.line_cites()
    if args.emit_baseline:
        return guard.emit(counts)
    guard.rules()
    guard.anchors()
    guard.ratchet(counts)
    guard.claims()
    shown = guard.findings
    if args.only:
        shown = [f for f in shown if any(args.only == p or (p.endswith("/") and args.only.startswith(p)) for p in f[3])]
    for check, where, message, _ in shown:
        print("FAIL %s %s: %s" % (check, where, message))
    if not args.only:
        c = guard.count
        print("claude-config-guard: rules=%d anchors=%d skipped=%d linecites=%d claims=%d fails=%d"
              % (c["rules"], c["anchors"], c["skipped"], c["linecites"], c["claims"], len(shown)))
    return 1 if shown else 0


if __name__ == "__main__":
    sys.exit(main())
