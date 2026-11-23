#!/usr/bin/env bash
# Behaviour tests for scripts/php-forward-probe.sh — run by scripts/git-hooks/pre-push.
#
# The probe answers one question the 8.5 oracle cannot: does the PHP we emit stay clean on the PHP
# builds that come AFTER the oracle? (2026-11-15, measured on a real 8.6 release candidate: a user
# function named `let`, `is` or `readonly`, a class named `Let` or `Is`, and a `return` inside
# `finally` lint clean on 8.5 and print a `Deprecated:` line on 8.6.)
#
# Sections 1-4 use STUB binaries, so they pass with no newer PHP build, no release binary and no
# /stack/tools: that is CI and a cloud session. Section 5 runs the real thing when this machine has
# it and says SKIP loudly when it does not.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOL="$HERE/php-forward-probe.sh"
PASS=0
FAIL=0
SKIP=0
ok()   { printf '  ok   — %s\n' "$1"; PASS=$((PASS + 1)); }
bad()  { printf '  FAIL — %s\n' "$1"; FAIL=$((FAIL + 1)); }
skip() { printf '  SKIP — %s\n' "$1"; SKIP=$((SKIP + 1)); }

TMP=$(mktemp -d /tmp/test-php-forward-XXXXXX)
trap 'rm -rf "$TMP"' EXIT
B="$TMP/builds"

# A stub php: answers the version query, and "lints" by looking for markers in the file.
#   mkphp <path> <version> <version id> [new]   — only a `new` build knows the 8.6 deprecation.
mkphp() {
  mkdir -p "$(dirname "$1")"
  cat > "$1" <<EOF
#!/bin/sh
if [ "\$1" = "-n" ] && [ "\$2" = "-r" ]; then printf '%s %s' "$3" "$2"; exit 0; fi
for f; do :; done
echo "$1" >> "$TMP/linted"
if [ "${4:-}" = "new" ] && grep -q DEPRECATED_NAME "\$f"; then
  printf '\nDeprecated: Calling a function "let" is deprecated since 8.6 in %s on line 3\n' "\$f"
fi
if grep -q OLD_WARN "\$f"; then printf '\nDeprecated: an old thing in %s on line 2\n' "\$f"; fi
printf 'No syntax errors detected in %s\n' "\$f"
EOF
  chmod +x "$1"
}
mkphp "$B/php-old/bin/php"    8.4.27 80427 new
mkphp "$B/php-floor/bin/php"  8.5.12 80512
mkphp "$B/php-same/bin/php"   8.5.99 80599 new
mkphp "$B/php-next/bin/php"   8.6.0  80600 new
mkphp "$B/php-master/bin/php" 8.7.0-dev 80700 new
# A stub phg: `transpile <file>` prints the file behind a PHP open tag, or refuses.
mkdir -p "$TMP/bin"
cat > "$TMP/bin/phg" <<'EOF'
#!/bin/sh
[ "$1" = "transpile" ] || exit 2
if grep -q REFUSE "$2"; then echo "error[E-STUB]: not transpilable alone" >&2; exit 1; fi
printf '<?php\n'; cat "$2"
EOF
chmod +x "$TMP/bin/phg"

mkdir -p "$TMP/src dir"
printf 'function let DEPRECATED_NAME\n' > "$TMP/hit.phg"
printf 'function fine\n'                > "$TMP/clean.phg"
printf 'OLD_WARN\n'                     > "$TMP/old.phg"
printf 'REFUSE\n'                       > "$TMP/project.phg"
printf 'DEPRECATED_NAME\n'              > "$TMP/src dir/sp ace.phg"

# probe [args…] — floor and one newer build given explicitly; stdout in $out, stderr in $err.
probe() {
  out="$(PHORJ_PHG="${PHG-$TMP/bin/phg}" PHORJ_FORWARD_FLOOR="$B/php-floor/bin/php" \
         PHORJ_FORWARD_PHPS="${PHPS-$B/php-next/bin/php}" bash "$TOOL" "$@" 2>"$TMP/err")"
  rc=$?
  err="$(cat "$TMP/err")"
}

echo "php-forward-probe.sh — what a newer PHP reports and the oracle does not"

# ── 1. A finding, and only a real one ────────────────────────────────────────────────────────────
probe --quiet "$TMP/hit.phg"
if [[ $rc -eq 1 && "$out" == *'PHP 8.6.0 reports'* && "$out" == *'Calling a function "let" is deprecated since 8.6'* ]]; then
  ok "a deprecation only the newer build prints is reported, rc 1"
else
  bad "finding not reported: rc=$rc out=${out:0:300}"
fi
if [[ "$out" == *"hit.phg"* && "$out" == *"the transpiled output"* && "$out" != *"/out.php"* ]]; then
  ok "the finding names the .phg file and never the temporary PHP path"
else
  bad "finding wording: ${out:0:300}"
fi
probe --quiet "$TMP/clean.phg"
if [[ $rc -eq 0 && -z "$out" && -z "$err" ]]; then ok "a clean file is silent, rc 0"; else bad "clean file: rc=$rc out=$out err=$err"; fi
probe --quiet "$TMP/old.phg"
if [[ $rc -eq 0 && -z "$out" ]]; then ok "a line the ORACLE prints too is not a forward finding"; else bad "old warning reported: rc=$rc $out"; fi
probe --quiet "$TMP/src dir/sp ace.phg"
if [[ $rc -eq 1 && "$out" == *"sp ace.phg"* ]]; then ok "a path with spaces works"; else bad "path with spaces: rc=$rc $out"; fi

# ── 2. Nothing to probe is never a failure, and never a silent success outside --quiet ───────────
probe --quiet "$TMP/project.phg"
if [[ $rc -eq 0 && -z "$out" && -z "$err" ]]; then ok "a file phg transpile refuses is silent under --quiet"; else bad "refused file: rc=$rc out=$out err=$err"; fi
probe "$TMP/project.phg" "$TMP/hit.phg"
if [[ $rc -eq 1 && "$err" == *"probed 1 file(s), skipped 1"* && "$err" == *"findings 1"* ]]; then
  ok "without --quiet the summary counts probed, skipped and findings"
else
  bad "summary: rc=$rc err=${err:0:300}"
fi
PHPS="" probe "$TMP/hit.phg"
if [[ $rc -eq 0 && -z "$out" && "$err" == *"no installed PHP build is newer than the oracle"* ]]; then
  ok "no newer build: rc 0 and the reason is stated"
else
  bad "no newer build: rc=$rc out=$out err=${err:0:200}"
fi
PHPS="" probe --quiet "$TMP/hit.phg"
if [[ $rc -eq 0 && -z "$out" && -z "$err" ]]; then ok "…and nothing at all under --quiet"; else bad "quiet, no newer build: rc=$rc out=$out err=$err"; fi
PHG="$TMP/bin/absent" probe "$TMP/hit.phg"
if [[ $rc -eq 0 && -z "$out" && "$err" == *"no phg binary"* ]]; then ok "no phg binary: rc 0 and the reason is stated"; else bad "no binary: rc=$rc err=${err:0:200}"; fi
out="$(bash "$TOOL" 2>&1)"; rc=$?
if [[ $rc -eq 2 ]]; then ok "no file argument is a usage error (rc 2)"; else bad "no args: rc=$rc $out"; fi

# ── 3. Which builds count as newer: decided by the build's own version, never by its folder name ─
: > "$TMP/linted"
# env -u: the glob is the path under test, so an inherited explicit list (the toolchain image sets one) must not win.
out="$(env -u PHORJ_FORWARD_PHPS PHORJ_PHG="$TMP/bin/phg" PHORJ_FORWARD_FLOOR="$B/php-floor/bin/php" bash "$TOOL" --quiet "$TMP/hit.phg" 2>/dev/null)"; rc=$?
if [[ $rc -eq 1 && "$out" == *"PHP 8.6.0 reports"* && "$out" == *"PHP 8.7.0-dev reports"* ]]; then
  ok "the builds beside the oracle are found by glob: 8.6.0 and 8.7.0-dev both probed"
else
  bad "glob resolution: rc=$rc ${out:0:300}"
fi
if ! grep -q -e "php-old" -e "php-same" "$TMP/linted"; then
  ok "an older build and another patch of the oracle's own minor are never linted"
else
  bad "linted a build that is not newer: $(sort -u "$TMP/linted" | tr '\n' ' ')"
fi
if [[ "$(grep -c 'Deprecated' <<<"$out")" == 2 ]]; then ok "one finding per newer build, no more"; else bad "finding count: $out"; fi

# ── 4. The script names no PHP patch version (scripts/validate-infra.sh forbids a pinned path) ───
if ! grep -nE 'php-[0-9]+\.[0-9]+' "$TOOL" | grep -vE '^[0-9]+:[[:space:]]*#' | grep -q .; then
  ok "no versioned php path outside comments"
else
  bad "a versioned php path is written in the probe"
fi

# ── 5. The real thing, when this machine has it ─────────────────────────────────────────────────
# The only end-to-end proof that the probe sees a REAL deprecation, so it needs a shape the transpiler
# still emits and a newer PHP still deprecates. It was `function let` until DEC-594 (2026-11-23)
# renamed that one like `clamp`; it is now a `return` inside `finally`. DEC-595 (ruled 2026-11-23,
# QUEUED) makes that shape a check error, so the change that builds DEC-595 must swap this case again
# or drop it - by design, not a gate on the defect.
ROOT="$(cd "$HERE/.." && pwd)"
real="$TMP/real.phg"
printf 'package Main;\nimport Core.Output;\nimport Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\nfunction f(): int { try { return 1; } finally { return 2; } }\n#[Entry(kind: EntryKind.Cli)]\nfunction main(): void { Output.printLine("{f()}"); }\n' > "$real"
if [[ -x "$ROOT/target/release/phg" ]]; then
  err="$(bash "$TOOL" "$real" 2>&1 >/dev/null)"
  if [[ "$err" == *"nothing probed"* ]]; then
    skip "real probe: ${err#php-forward: }"
  else
    out="$(bash "$TOOL" --quiet "$real" 2>/dev/null)"; rc=$?
    if [[ $rc -eq 1 && "$out" == *'Returning from a finally block is deprecated'* ]]; then
      ok "REAL: a return inside finally is reported by an installed newer PHP"
    else
      bad "REAL probe found nothing: rc=$rc ${out:0:300}"
    fi
    sed -i 's/finally { return 2; }/finally { Output.printLine("y"); }/' "$real"
    out="$(bash "$TOOL" --quiet "$real" 2>/dev/null)"; rc=$?
    if [[ $rc -eq 0 && -z "$out" ]]; then ok "REAL: the same file with an ordinary name is silent"; else bad "REAL clean file: rc=$rc $out"; fi
  fi
else
  skip "real probe: no target/release/phg on this machine"
fi

echo
echo "$PASS passed, $FAIL failed, $SKIP skipped"
[[ "$FAIL" -eq 0 ]]
