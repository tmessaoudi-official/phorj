#!/usr/bin/env bash
# Behaviour tests for scripts/claude-config-guard.py — run by scripts/git-hooks/pre-push next to the
# other gates' own suites: a gate whose behaviour suite no gate runs goes dark unnoticed.
#
# Every failure this guard exists for is SILENT in production: a path-scoped rules file with a dead
# glob never loads, a quoted anchor whose phrase moved still reads as a citation, a crate count in an
# always-loaded file stays wrong until someone counts by hand (2026-11-13: 14 and 9 in three files,
# Cargo.toml gave 15 and 10). So each check is proven able to FIRE here, on a fixture repository, and
# the clean fixture is proven to pass with non-zero counts (a guard that scans nothing is green too).
#
# The fixture is a real git repository: the guard decides "tracked" from the index, never from the
# disk, so its verdict is the same on the developer's machine, on CI and in a cloud session.
set -uo pipefail
# The fixture is its own repository. Never let a GIT_* variable inherited from the hook that runs this
# suite redirect its git commands to the real one (see the isolation case below).
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOL="$HERE/claude-config-guard.py"
PASS=0
FAIL=0
ok()  { printf '  ok   — %s\n' "$1"; PASS=$((PASS + 1)); }
bad() { printf '  FAIL — %s\n' "$1"; FAIL=$((FAIL + 1)); }

TMP=$(mktemp -d /tmp/test-claude-config-guard-XXXXXX)
trap 'rm -rf "$TMP"' EXIT
R="$TMP/repo"
PACK=".claude/skills/domain-x/SKILL.md"
BASE="scripts/claude-config-baseline.txt"

mk() {
  rm -rf "$R"
  mkdir -p "$R"/.claude/rules "$R"/.claude/skills/domain-x "$R"/.claude/skills/phg-a \
           "$R"/.claude/skills/phg-b "$R"/.claude/agents "$R"/docs "$R"/src "$R"/scripts
  {
    printf '[package]\nname = "f"\n\n[dependencies]\n'
    for i in 1 2 3 4 5 6 7 8 9 10 11; do printf 'dep%s = { version = "1", optional = true }\n' "$i"; done
    printf 'plain = "1"\n\n[features]\ndefault = ["a", "b"]\na = ["dep:dep1", "dep:dep2"]\n'
    printf 'b = ["dep:dep3"]\nc = ["dep:dep4"]\n'
  } > "$R/Cargo.toml" # 12 crates; the default build links plain + dep1..dep3 = 4
  printf '# f\n**12 vetted, feature-gated crates** here.\nIt carries exactly TWO skills.\nSee `.claude/rules/scoped.md`.\n' > "$R/CLAUDE.md"
  printf 'A set (12 admitted, 4 in the default build).\n' > "$R/VISION.md"
  printf 'Deps: 12 crates, 4 in the default build.\nAlso 12 vetted, feature-gated.\n' > "$R/README.md"
  printf 'Three out of 12 admitted are optional.\n' > "$R/SECURITY.md"
  printf -- '---\npaths:\n  - "src/**"\n  - "Cargo.toml"\n---\n\n# scoped\n' > "$R/.claude/rules/scoped.md"
  printf '# north star\n' > "$R/.claude/rules/north-star.md"
  printf '# expertise\n' > "$R/.claude/rules/expertise-core.md"
  printf '# Title\n## Unique heading\nalpha unique phrase here\ndup line\ndup line\n' > "$R/docs/t.md"
  printf 'line one\nline two\n' > "$R/src/a.rs"
  {
    printf '# pack\n[Source: docs/t.md ("alpha unique phrase")]\n[Source: docs/t.md (§ "Unique heading")]\n'
    printf '[Source: docs/t.md ("alpha unique phrase"; § "Unique heading")]\n'
    printf 'wrapped in the citing file: docs/t.md ("alpha unique\n  phrase")\n'
    printf 'cites: src/a.rs:1 and src/a.rs:1-2\n'
  } > "$R/$PACK"
  printf '# a\n' > "$R/.claude/skills/phg-a/SKILL.md"
  printf '# b\n' > "$R/.claude/skills/phg-b/SKILL.md"
  printf 'C3\t2\t%s\n' "$PACK" > "$R/$BASE"
  printf 'var/\n' > "$R/.gitignore"
  git -C "$R" -c init.defaultBranch=main init -q
}
run() {
  git -C "$R" add -A >/dev/null 2>&1
  out="$(python3 "$TOOL" --root "$R" "$@" 2>&1)"
  rc=$?
}
passes() { if [[ $rc -eq 0 ]]; then ok "$1"; else bad "$1 — rc=$rc: ${out:0:400}"; fi; }
fails()  { if [[ $rc -ne 0 && "$out" == *"$2"* ]]; then ok "$1"; else bad "$1 — rc=$rc, wanted '$2': ${out:0:400}"; fi; }
add() { printf '%s\n' "$2" >> "$R/$1"; }

echo "claude-config-guard.py — rules files, quoted anchors, line cites, derived claims"

# ── 0. Control: the clean fixture passes, and it scanned something ───────────────────────────────
mk; run
passes "the clean fixture passes"
if [[ "$out" == *"rules=3 anchors=5 skipped=0 linecites=2 claims=8 fails=0"* ]]; then
  ok "the summary counts what it scanned (3 rules files, 5 anchors, 2 line cites, 8 claims)"
else
  bad "summary counts differ: ${out:0:300}"
fi

mk; run; rm "$R/docs/t.md"
out="$(python3 "$TOOL" --root "$R" 2>&1)"; rc=$?
fails "a tracked file missing from the working tree is a finding, not a traceback" "docs/t.md: tracked but not readable"
if [[ "$out" != *"Traceback"* ]]; then ok "…and the guard still prints its summary"; else bad "traceback: ${out:0:300}"; fi
mk; printf 'alpha unique phrase here \377\376 broken bytes\n' >> "$R/docs/t.md"; run
fails "a cited file that is not valid UTF-8 is a finding, not a silent replacement" "docs/t.md: not valid UTF-8"
mk; printf 'C3\t2\t%s\n\377\tbad\n' "$PACK" > "$R/$BASE"; run
fails "a baseline row with a stray byte fails closed as malformed" "malformed row"

# ── Isolation: an inherited GIT_DIR must never reach the fixture's git commands ───────────────────
# git exports GIT_DIR (and friends) to a hook in some invocations - a push from a subdirectory or a
# linked worktree. Without the `unset` at the top of this file, `git -C <fixture> add -A` then acts on
# THAT repository: measured 2026-11-16 on a scratch one, whose index lost its own file and gained the
# fixture's. A child run under an exported GIT_DIR must leave that repository's index alone.
if [[ -z "${CCG_NESTED:-}" ]]; then
  git -c init.defaultBranch=main init -q "$TMP/outer"
  printf 'canary\n' > "$TMP/outer/canary.txt"; git -C "$TMP/outer" add canary.txt
  child="$(GIT_DIR="$TMP/outer/.git" CCG_NESTED=1 bash "${BASH_SOURCE[0]}" 2>&1 | tail -1)"
  index="$(git --git-dir="$TMP/outer/.git" ls-files | tr '\n' ' ')"
  if [[ "$index" == "canary.txt " ]]; then ok "a run under an exported GIT_DIR leaves that repository's index untouched"; else bad "the suite wrote into the exported GIT_DIR's index: ${index:0:160}"; fi
  if [[ "$child" == *" passed, 0 failed" ]]; then ok "…and still passes there ($child)"; else bad "child run under GIT_DIR: $child"; fi
else
  echo; echo "$PASS passed, $FAIL failed"; [[ "$FAIL" -eq 0 ]]; exit
fi

# ── C1. Rules files ──────────────────────────────────────────────────────────────────────────────
mk; printf -- '---\npaths:\n  - "nope/**"\n---\n# scoped\n' > "$R/.claude/rules/scoped.md"; run
fails "a glob that matches no tracked file fails" 'glob "nope/**" matches no tracked file'
mk; printf '# scoped, frontmatter lost\n' > "$R/.claude/rules/scoped.md"; run
fails "a scoped file that lost its frontmatter fails" "no frontmatter on line 1"
mk; printf -- '---\npaths:\n  - "src/**"\n\n# scoped\n' > "$R/.claude/rules/scoped.md"; run
fails "frontmatter that never closes fails" "frontmatter never closes"
mk; printf -- '---\npaths:\n---\n# scoped\n' > "$R/.claude/rules/scoped.md"; run
fails "an empty paths list fails" "paths: is empty"
mk; printf -- '---\npaths:\n  - "src/**"\n---\n# north star\n' > "$R/.claude/rules/north-star.md"; run
fails "an always-loaded file that gains frontmatter fails" "listed as always-loaded but opens with frontmatter"
mk; rm "$R/.claude/rules/expertise-core.md"; run
fails "an always-loaded name with no file fails" "expertise-core.md: listed as always-loaded but is not a tracked file"
mk; printf '# f\n**12 vetted, feature-gated crates** here.\nIt carries exactly TWO skills.\n' > "$R/CLAUDE.md"; run
fails "a scoped file CLAUDE.md does not name fails" "scoped.md: not named in CLAUDE.md"
mk; add CLAUDE.md 'And `.claude/rules/gone.md`.'; run
fails "a rules file CLAUDE.md names but the tree lacks fails" "names .claude/rules/gone.md, which is not a tracked file"

# ── C2. Quoted anchors ───────────────────────────────────────────────────────────────────────────
mk; add "$PACK" 'docs/t.md ("no such phrase")'; run
fails "a phrase on no line fails" 'resolves on 0 lines'
mk; add "$PACK" 'docs/t.md ("dup line")'; run
fails "a phrase on two lines fails" 'resolves on 2 lines'
mk; printf '# Title\n## Unique heading\nalpha unique\nphrase here\n' > "$R/docs/t.md"; run
fails "a phrase the TARGET wraps across two lines fails" 'resolves on 0 lines'
mk; add "$PACK" 'docs/t.md (§ "alpha unique phrase")'; run
fails "a heading anchor that only matches body text fails" 'resolves on 0 lines'
mk; add "$PACK" 'docs/t.md ("alpha unique phrase"; § "No such heading")'; run
fails "the second item of a multi-item anchor is checked" '§ "No such heading"'
mk; add "$PACK" 'docs/t.md ("never closed'; run
fails "an anchor opener that never closes fails" 'anchor not parsable'
mk; add "$PACK" 'docs/typo.md ("alpha unique phrase")'; run
fails "a target that is neither tracked nor ignored fails" 'docs/typo.md is not a tracked file'
mk; add "$PACK" 'memory/note.md ("anything") and ~/.claude/x.md ("anything") and var/r.md ("anything")'; run
passes "targets outside the repository or gitignored do not fail"
if [[ "$out" == *"skipped=3"* ]]; then ok "and they are counted as skipped (3), not as resolved"; else bad "skipped count: ${out:0:300}"; fi

# ── C3. Line cites: a ratchet that only tightens ─────────────────────────────────────────────────
mk; add "$PACK" 'one more: src/a.rs:2'; run
fails "a new line cite fails" '3 line cites, baseline 2'
mk; add ".claude/skills/phg-a/SKILL.md" 'cite: src/a.rs:1'; run
fails "a line cite in a file with no baseline row fails" '1 line cites, baseline 0'
mk; add "$PACK" 'far: src/a.rs:99'; printf 'C3\t3\t%s\n' "$PACK" > "$R/$BASE"; run
fails "a line cite past the end of its target fails" 'src/a.rs:99 is past the end of the file (2 lines)'
mk; printf 'C3\t5\t%s\n' "$PACK" > "$R/$BASE"; run
fails "a baseline above the count fails (the ratchet tightens)" '2 line cites, baseline 5'
run --emit-baseline; run
passes "--emit-baseline lowers the row and the guard passes again"
if [[ "$(cat "$R/$BASE")" == "C3"$'\t'"2"$'\t'"$PACK" ]]; then ok "the lowered row reads <C3><TAB>2<TAB>path"; else bad "baseline after emit: $(cat "$R/$BASE")"; fi
mk; add "$PACK" 'one more: src/a.rs:2'; run --emit-baseline
fails "--emit-baseline refuses to RAISE a row" 'refusing to raise'
if [[ "$(cut -f2 "$R/$BASE")" == "2" ]]; then ok "and leaves the baseline untouched"; else bad "baseline was raised: $(cat "$R/$BASE")"; fi
for row in $'C3\tabc\t'"$PACK" $'C3\t2' "C3 2 $PACK" $'C3\t-1\t'"$PACK"; do
  mk; printf '%s\n' "$row" > "$R/$BASE"; run
  fails "a malformed baseline row fails closed (${row:0:14}…)" 'malformed row'
done

# ── C4. Derived claims ───────────────────────────────────────────────────────────────────────────
mk; printf 'Deps: 14 crates, 4 in the default build.\nAlso 12 vetted, feature-gated.\n' > "$R/README.md"; run
fails "a stale crate total fails, naming both numbers" 'says 14 for the crate total, Cargo.toml gives 12'
mk; printf 'A set (12 admitted, 9 in the default build).\n' > "$R/VISION.md"; run
fails "a stale default-build count fails" 'says 9 for the default-build crate count, Cargo.toml gives 4'
mk; printf 'Nothing about counts any more.\n' > "$R/SECURITY.md"; run
fails "a claim whose wording vanished fails instead of passing" 'SECURITY.md: the crate total claim was not found'
mk; mkdir -p "$R/.claude/skills/phg-c"; printf '# c\n' > "$R/.claude/skills/phg-c/SKILL.md"; run
fails "a third non-domain skill contradicts 'exactly TWO skills'" 'says TWO for the non-domain skill count, .claude/skills gives 3'
mk; printf '[package]\nname = "f"\n[dependencies]\nplain = "1"\n' > "$R/Cargo.toml"; run
fails "a Cargo.toml the parser cannot read fails instead of passing vacuously" 'parsed only 1 dependencies'

# ── --only: the write-time view reports findings that involve ONE file ───────────────────────────
mk; add "$PACK" 'docs/t.md ("no such phrase")'
printf 'Deps: 14 crates, 4 in the default build.\nAlso 12 vetted, feature-gated.\n' > "$R/README.md"; run --only README.md
if [[ $rc -ne 0 && "$out" == *"README.md"* && "$out" != *"no such phrase"* ]]; then
  ok "--only README.md shows the README finding and not the pack's"
else
  bad "--only README.md: rc=$rc ${out:0:300}"
fi
run --only docs/t.md
if [[ $rc -ne 0 && "$out" == *"no such phrase"* && "$out" != *"says 14"* ]]; then
  ok "--only <an anchor TARGET> shows the anchors that point into it"
else
  bad "--only docs/t.md: rc=$rc ${out:0:300}"
fi
run --only Cargo.toml
if [[ $rc -ne 0 && "$out" == *"says 14"* ]]; then ok "--only Cargo.toml shows the claims derived from it"; else bad "--only Cargo.toml: rc=$rc ${out:0:300}"; fi
run --only src/a.rs
passes "--only <a file no finding involves> passes"

echo
echo "$PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
