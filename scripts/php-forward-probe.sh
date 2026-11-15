#!/usr/bin/env bash
# PHP forward-compat probe: does the PHP we emit stay clean on the PHP builds AFTER the oracle?
#
# The byte-identity gate runs the transpiled output on the 8.5 floor and nothing newer, so a name PHP
# takes in a later release, or a construct it starts to deprecate, is invisible to it. Measured
# 2026-11-15 on this machine's 8.6 release candidate: the output for a user function named `let`,
# `is` or `readonly`, a class named `Let` or `Is`, and a `return` inside `finally` lints clean on the
# oracle and prints a `Deprecated:` line on 8.6 - a line that would land in the program's output.
#
# What it does, per .phg file: transpile it with the release binary, lint the PHP (`php -n -l`) on the
# oracle and on every installed build NEWER than the oracle, and print each diagnostic line a newer
# build gives that the oracle does not.
#
# What it does NOT do: it never runs the program (lint only - no side effects, no hang on a program
# that reads stdin or serves), so it sees compile-time deprecations and nothing that only shows at
# run time. A file that does not transpile on its own (a project file, a concurrency example with its
# E-CONCURRENCY-NO-PHP error, a file mid-edit) is counted as skipped, not as clean.
#
# Newer builds are FOUND, never named: the folders beside the resolved oracle (`php-*/bin/php`), kept
# when the build's OWN version is a later minor. No PHP patch version is written here -
# scripts/validate-infra.sh forbids a second copy of the oracle's path, and the next release would
# make any literal stale.
#
# Usage: scripts/php-forward-probe.sh [--quiet] FILE.phg...
#   stdout   one line per finding
#   stderr   a summary, or why nothing was probed; nothing under --quiet (the write-time hook's mode)
#   exit     0 no finding (also when nothing could be probed), 1 findings, 2 usage
# Seams (tests): PHORJ_PHG, PHORJ_FORWARD_FLOOR (skips scripts/toolchain.env), PHORJ_FORWARD_PHPS
# (colon-separated candidate builds instead of the glob; empty = none).
# Guard: scripts/test-php-forward-probe.sh (run by scripts/git-hooks/pre-push).
set -uo pipefail

quiet=0
files=()
for arg in "$@"; do
  case "$arg" in
    --quiet) quiet=1 ;;
    -h | --help) sed -n '2,/^set -uo/{/^set -uo/d;s/^# \{0,1\}//;p}' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) files+=("$arg") ;;
  esac
done
note() { ((quiet)) || printf 'php-forward: %s\n' "$1" >&2; }
if ((${#files[@]} == 0)); then
  echo "usage: scripts/php-forward-probe.sh [--quiet] FILE.phg..." >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
phg="${PHORJ_PHG:-$root/target/release/phg}"
if [[ ! -x "$phg" ]]; then
  note "nothing probed - no phg binary at ${phg#"$root"/} (cargo build --release)"
  exit 0
fi

floor="${PHORJ_FORWARD_FLOOR:-}"
if [[ -z "$floor" ]]; then
  # The oracle is resolved in ONE place. Its own warnings go to stderr as they are.
  # shellcheck source=toolchain.env
  . "$root/scripts/toolchain.env"
  floor="${PHORJ_PHP:-}"
fi
if [[ -z "$floor" || ! -x "$floor" ]]; then
  note "nothing probed - no PHP oracle resolved (scripts/toolchain.env)"
  exit 0
fi

# version_of <php> -> "<PHP_VERSION_ID> <PHP_VERSION>", or nothing when the binary does not answer.
version_of() { "$1" -n -r 'echo PHP_VERSION_ID, " ", PHP_VERSION;' 2>/dev/null; }
is_id() { case "$1" in '' | *[!0123456789]*) return 1 ;; esac; ((${#1} <= 7)); }

read -r floor_id floor_ver <<<"$(version_of "$floor")"
if ! is_id "${floor_id:-}"; then
  note "nothing probed - the oracle at $floor did not report a version"
  exit 0
fi

cands=()
if [[ -n "${PHORJ_FORWARD_PHPS+set}" ]]; then
  IFS=: read -r -a cands <<<"$PHORJ_FORWARD_PHPS"
else
  base="$(dirname "$(dirname "$(dirname "$floor")")")"
  for cand in "$base"/php-*/bin/php; do
    [[ -x "$cand" ]] && cands+=("$cand")
  done
fi
newer=()
labels=()
for cand in "${cands[@]:-}"; do
  [[ -n "$cand" && -x "$cand" ]] || continue
  read -r id ver <<<"$(version_of "$cand")"
  is_id "${id:-}" || continue
  # A later MINOR than the oracle's: another patch of the oracle's own minor is not "forward".
  if ((10#$id / 100 > 10#$floor_id / 100)); then
    newer+=("$cand")
    labels+=("$ver")
  fi
done
if ((${#newer[@]} == 0)); then
  note "nothing probed - no installed PHP build is newer than the oracle ($floor_ver)"
  exit 0
fi

tmp="$(mktemp -d "${TMPDIR:-/tmp}/php-forward-XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
php_out="$tmp/out.php"

# lint <php> -> the diagnostic lines of `php -n -l`, with the temporary path replaced. The exit code of
# the lint is not used: a parse error is reported as a line, like a deprecation.
lint() {
  local line
  while IFS= read -r line; do
    case "$line" in
      Deprecated:* | Warning:* | Notice:* | "Fatal error:"* | "Parse error:"* | "PHP "*:*)
        printf '%s\n' "${line//"$php_out"/the transpiled output}"
        ;;
    esac
  done < <("$1" -n -l "$php_out" 2>&1)
}

probed=0 skipped=0 findings=0
for file in "${files[@]}"; do
  # Not transpilable alone is a skip, counted and reported in the summary: the refusal text belongs to
  # `phg transpile`, which the author runs to see it (measured: examples/guide/concurrency.phg exits 1
  # with E-CONCURRENCY-NO-PHP).
  if [[ ! -f "$file" ]] || ! "$phg" transpile "$file" >"$php_out" 2>/dev/null; then
    skipped=$((skipped + 1))
    continue
  fi
  probed=$((probed + 1))
  shown="$file"
  [[ "$file" == "$root"/* ]] && shown="${file#"$root"/}"
  floor_lines="$(lint "$floor")"
  for i in "${!newer[@]}"; do
    while IFS= read -r line; do
      [[ -n "$line" ]] || continue
      # Here-string, not a pipe: a `| grep -q` under pipefail dies of SIGPIPE on a long input.
      if ! grep -qxF -e "$line" <<<"$floor_lines"; then
        printf 'php-forward: %s: PHP %s reports what the %s oracle does not: %s\n' \
          "$shown" "${labels[$i]}" "$floor_ver" "$line"
        findings=$((findings + 1))
      fi
    done < <(lint "${newer[$i]}")
  done
done

note "probed $probed file(s), skipped $skipped (not transpilable alone), findings $findings; newer builds: ${labels[*]}; oracle $floor_ver"
((findings == 0))
