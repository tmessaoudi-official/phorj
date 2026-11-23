#!/usr/bin/env bash
# Every pre-push leg is classified in scripts/gate-legs.tsv (DEC-597), so a new leg cannot go uncovered in a cloud
# session without anyone deciding it: checked both ways (a leg with no row, a row with no leg), each class is valid,
# an `owed` leg's script records an OWED entry (scripts/owed-legs.sh), and a `pc-only` leg runs before the hook's
# dk re-exec, guarded by the runner check. Static (no build, no network); the fixture cases prove each check fires.
set -uo pipefail
export LC_ALL=C   # sort and comm must collate alike
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
PASS=0; FAIL=0
ok()  { printf '  ok   — %s\n' "$1"; PASS=$((PASS + 1)); }
bad() { printf '  FAIL — %s\n' "$1"; FAIL=$((FAIL + 1)); }

# legs <hook>: the key of each leg line (stdout echoes only; stderr lines are messages), one per line.
legs() { grep -E '^\s*echo "\[pre-push\] [^"]*"\s*$' "$1" | sed -E 's/^\s*echo "\[pre-push\] //; s/"\s*$//; s/ \(.*$//'; }
rows() { grep -vE '^\s*(#|$)' "$1" | cut -f1; }
# verify <hook> <tsv> <root>: prints one line per problem; silent when the two agree.
verify() {
  local hook="$1" tsv="$2" root="$3" k cls note exec_line line
  comm -23 <(legs "$hook" | sort -u) <(rows "$tsv" | sort -u) | sed 's/^/leg with no row: /'
  comm -13 <(legs "$hook" | sort -u) <(rows "$tsv" | sort -u) | sed 's/^/row with no leg: /'
  exec_line="$(grep -nE '^\s*exec .*/scripts/dk' "$hook" | head -1 | cut -d: -f1)"
  while IFS=$'\t' read -r k cls note; do
    [[ -z "$k" || "$k" == \#* ]] && continue
    case "$cls" in
      everywhere|info) ;;
      owed) [[ -f "$root/$note" ]] && grep -q 'owed-legs.sh' "$root/$note" && grep -qE '(owed|owed-legs\.sh) record ' "$root/$note" \
              || echo "owed leg '$k' does not record OWED in its script '$note'" ;;
      pc-only)
        line="$(grep -nF "echo \"[pre-push] $k" "$hook" | head -1 | cut -d: -f1)"
        if [[ -z "$line" || -z "$exec_line" ]] || (( line > exec_line )); then echo "pc-only leg '$k' is not before the dk re-exec"
        elif ! sed -n "1,${line}p" "$hook" | grep -q 'phorj.runner'; then echo "pc-only leg '$k' is not guarded by the runner check"; fi ;;
      *) echo "row '$k' has an unknown class '$cls'" ;;
    esac
  done <"$tsv"
}

echo "gate-legs — $ROOT/scripts/gate-legs.tsv against $ROOT/scripts/git-hooks/pre-push"
out="$(verify "$ROOT/scripts/git-hooks/pre-push" "$ROOT/scripts/gate-legs.tsv" "$ROOT")"
[[ -z "$out" ]] && ok "every pre-push leg is classified, and every row names a leg" || bad "the table and the hook disagree: $out"
(( $(legs "$ROOT/scripts/git-hooks/pre-push" | wc -l) >= 25 )) && ok "the leg parser finds the hook's legs (25 or more)" \
  || bad "the leg parser found only $(legs "$ROOT/scripts/git-hooks/pre-push" | wc -l) legs"

# Fixtures: each check fires.
TMP=$(mktemp -d /tmp/test-gate-legs-XXXXXX); trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/scripts"
cat >"$TMP/hook" <<'EOF'
if [[ "$(git config --get phorj.runner || true)" != host ]]; then
  echo "[pre-push] early (a pc-only leg)"
  exec "$root/scripts/dk" "$0"
fi
echo "[pre-push] REFUSED: a message" >&2
echo "[pre-push] alpha (a leg)"
echo "[pre-push] bench (an owed leg)"
echo "[pre-push] late (pc-only, but after the re-exec)"
EOF
printf 'bash scripts/owed-legs.sh record bench pc x\n' >"$TMP/scripts/records.sh"; : >"$TMP/scripts/silent.sh"
f() { printf '%s\n' "$@" >"$TMP/t.tsv"; verify "$TMP/hook" "$TMP/t.tsv" "$TMP"; }
T=$'\t'
good=("early${T}pc-only${T}x" "alpha${T}everywhere${T}-" "bench${T}owed${T}scripts/records.sh" "late${T}everywhere${T}-")
[[ -z "$(f "${good[@]}")" ]] && ok "fixture: a consistent table passes" || bad "fixture: consistent table: $(f "${good[@]}")"
[[ "$(f "${good[@]:1}")" == *"leg with no row: early"* ]] && ok "fixture: a leg with no row fails" || bad "fixture: missing row not caught"
[[ "$(f "${good[@]}" "ghost${T}everywhere${T}-")" == *"row with no leg: ghost"* ]] && ok "fixture: a row with no leg fails" || bad "fixture: ghost row not caught"
[[ "$(f "early${T}pc-only${T}x" "alpha${T}sometimes${T}-" "bench${T}owed${T}scripts/records.sh" "late${T}everywhere${T}-")" == *"unknown class"* ]] \
  && ok "fixture: an unknown class fails" || bad "fixture: unknown class not caught"
[[ "$(f "early${T}pc-only${T}x" "alpha${T}everywhere${T}-" "bench${T}owed${T}scripts/silent.sh" "late${T}everywhere${T}-")" == *"does not record OWED"* ]] \
  && ok "fixture: an owed leg whose script records nothing fails" || bad "fixture: silent owed script not caught"
[[ "$(f "early${T}pc-only${T}x" "alpha${T}everywhere${T}-" "bench${T}owed${T}scripts/records.sh" "late${T}pc-only${T}x")" == *"not before the dk re-exec"* ]] \
  && ok "fixture: a pc-only leg after the re-exec fails" || bad "fixture: late pc-only not caught"
[[ "$(legs "$TMP/hook" | grep -c REFUSED)" == 0 ]] && ok "fixture: a stderr message is not a leg" || bad "fixture: a stderr line was read as a leg"

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 && $PASS -gt 0 ]]
