#!/usr/bin/env bash
# Behaviour tests for .claude/cloud-setup.sh — the toolchain hook the developer's global kit runs as root when a
# claude.ai cloud environment is set up (DEC-577). Run by scripts/git-hooks/pre-push via test-cloud-ready.sh's
# wiring check. The hook trusts a third-party apt key and installs packages, so its guards are tested by EXECUTION:
#   - off the cloud (no CLAUDE_CODE_REMOTE, or a stray export with HOME not /root) it does nothing at all;
#   - the sury key is trusted only when it is exactly ONE key with the pinned fingerprint (a wrong key, an extra key
#     riding along, a failed download: ERROR, no keyring, no apt source, apt never runs);
#   - cargo-nextest is installed only when the download matches the pinned sha256;
#   - every step logs its seconds; the hook always exits 0 (it must never fail the environment).
# Isolation: a closed PATH (fake curl/apt-get + symlinks to the real tools the hook needs; this PC's own php8.5,
# llvm-objcopy and cargo-nextest must be invisible), root-only paths under PHORJ_CLOUD_SETUP_PREFIX, a private
# TMPDIR, a throwaway git repo. Real gpg (on a temp --homedir) parses the real sury public key in
# scripts/testdata/sury-php-apt.gpg. No network, nothing written outside $TMP.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(git -C "$HERE" rev-parse --show-toplevel)"
HOOK="$ROOT/.claude/cloud-setup.sh"
KEY="$HERE/testdata/sury-php-apt.gpg"
PASS=0
FAIL=0
ok()  { printf '  ok   — %s\n' "$1"; PASS=$((PASS + 1)); }
bad() { printf '  FAIL — %s\n' "$1"; FAIL=$((FAIL + 1)); }
check() { if eval "$2"; then ok "$1"; else bad "$1 [$2]"; fi; }

TMP=$(mktemp -d /tmp/test-cloud-setup-XXXXXX)
trap 'rm -rf "$TMP"' EXIT
echo "cloud-setup.sh — $HOOK"
[[ -f "$HOOK" ]] || { bad "the hook exists"; printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"; exit 1; }

# ── fixtures ────────────────────────────────────────────────────────────────────────────────────────────────
G="$TMP/gnupg"; mkdir -m 700 "$G"
gpg --homedir "$G" --batch --passphrase '' --quick-gen-key 'not sury <x@example.invalid>' ed25519 sign never >/dev/null 2>&1
gpg --homedir "$G" --batch --export >"$TMP/other.gpg"
cat "$KEY" "$TMP/other.gpg" >"$TMP/both.gpg"
[[ -s "$TMP/other.gpg" ]] || bad "fixture: a throwaway key could be generated"
mkdir -p "$TMP/nx"; printf '#!/bin/sh\necho fake\n' >"$TMP/nx/cargo-nextest"; chmod +x "$TMP/nx/cargo-nextest"
tar -czf "$TMP/nextest.tgz" -C "$TMP/nx" cargo-nextest   # NOT the pinned tarball: its sha must be refused

# A closed PATH: fakes first, then only the real tools the hook is allowed to use.
TOOLS="$TMP/tools"; FAKE="$TMP/fake"; mkdir -p "$TOOLS" "$FAKE"
for t in bash sh env awk sed grep cat mkdir mktemp rm chmod install tar gzip sha256sum gpg git realpath dirname \
         basename head tail tr cut cp mv date sleep printf wc timeout; do
  p="$(command -v "$t" 2>/dev/null)" && [[ "$p" == /* ]] && ln -sf "$p" "$TOOLS/$t"
done
# fake curl: serves a fixture chosen per run (CS_KEY = file for the sury key URL, CS_FAIL=1 = every download fails)
cat >"$FAKE/curl" <<'EOF'
#!/bin/bash
echo "curl $*" >>"$CS_CALLS"
[[ "${CS_FAIL:-}" == 1 ]] && exit 22
out=""; url=""
while [[ $# -gt 0 ]]; do case "$1" in -o) out="$2"; shift 2 ;; -*) shift ;; *) url="$1"; shift ;; esac; done
case "$url" in
  https://packages.sury.org/php/apt.gpg) cp "$CS_KEY" "$out" ;;
  https://github.com/nextest-rs/*) cp "$CS_NEXTEST" "$out" ;;
  *) exit 22 ;;
esac
EOF
# fake apt-get: records; `install` puts a fake php8.5 (with bcmath) where the hook will find it
cat >"$FAKE/apt-get" <<'EOF'
#!/bin/bash
echo "apt-get $*" >>"$CS_CALLS"
if [[ " $* " == *" install "* ]]; then
  printf '#!/bin/sh\n[ "$1" = -m ] && printf "Core\\nbcmath\\n"\n' >"$CS_FAKE/php8.5"; chmod +x "$CS_FAKE/php8.5"
fi
EOF
chmod +x "$FAKE/curl" "$FAKE/apt-get"

# One run: a fresh prefix + repo per case unless KEEP=1; prints the hook's output into $OUT, rc into $RC.
fresh() {
  rm -rf "$TMP/p" "$TMP/repo" "$TMP/tmp" "$TMP/calls" "$FAKE/php8.5"
  mkdir -p "$TMP/p/etc" "$TMP/tmp" "$TMP/repo/scripts/git-hooks"
  printf 'NAME="Ubuntu"\nVERSION_CODENAME=noble\n' >"$TMP/p/etc/os-release"
  git -C "$TMP/repo" init -q; : >"$TMP/repo/scripts/git-hooks/pre-push"; : >"$TMP/calls"
}
run() {   # $1 = HOME, $2 = CLAUDE_CODE_REMOTE ("" = unset)
  OUT="$(cd "$TMP/repo" && env -i PATH="$FAKE:$TOOLS" HOME="$1" ${2:+CLAUDE_CODE_REMOTE="$2"} TMPDIR="$TMP/tmp" \
         PHORJ_CLOUD_SETUP_PREFIX="$TMP/p" CS_CALLS="$TMP/calls" CS_FAKE="$FAKE" CS_KEY="${CS_KEY:-$KEY}" \
         CS_NEXTEST="$TMP/nextest.tgz" ${CS_FAIL:+CS_FAIL=1} bash "$HOOK" 2>&1)"; RC=$?
}
KR="$TMP/p/usr/share/keyrings/debsuryorg-archive-keyring.gpg"
SRC="$TMP/p/etc/apt/sources.list.d/php.list"
hooks_path() { git -C "$TMP/repo" config --get core.hooksPath; }

# ── 1. Off the cloud: nothing happens ───────────────────────────────────────────────────────────────────────
fresh; run "$TMP/home" ""
check "PC (no CLAUDE_CODE_REMOTE): exit 0" '[[ $RC -eq 0 ]]'
check "PC: no output, no download, no apt, no hooksPath" '[[ -z "$OUT" && ! -s $TMP/calls && -z "$(hooks_path)" && ! -e $KR ]]'
fresh; run "$TMP/home" true
check "stray CLAUDE_CODE_REMOTE=true with HOME not /root: no-op" '[[ $RC -eq 0 && -z "$OUT" && ! -s $TMP/calls && -z "$(hooks_path)" ]]'

# ── 2. Cloud, the real sury key ─────────────────────────────────────────────────────────────────────────────
fresh; run /root true
check "cloud: exit 0" '[[ $RC -eq 0 ]]'
check "the keyring is installed, mode 0644" '[[ -f $KR && "$(stat -c %a "$KR")" == 644 ]]'
check "the keyring holds exactly the pinned key" \
  '[[ "$(gpg --homedir "$G" --batch --show-keys --with-colons "$KR" 2>/dev/null | awk -F: '"'"'$1=="pub"{n++} $1=="fpr"&&!f{f=$10} END{print n" "f}'"'"')" == "1 15058500A0235D97F5D10063B188E2B695BD4743" ]]'
check "the apt source names the real keyring path and the os-release codename" \
  '[[ "$(cat "$SRC" 2>/dev/null)" == "deb [signed-by=/usr/share/keyrings/debsuryorg-archive-keyring.gpg] https://packages.sury.org/php/ noble main" ]]'
check "apt-get update runs, then the four php8.5 packages are installed" \
  'grep -q "^apt-get update" $TMP/calls && grep -qE "^apt-get .*install .*php8\.5-cli php8\.5-bcmath php8\.5-mbstring php8\.5-sqlite3" $TMP/calls'
check "php8.5 with bcmath is reported present after the install" 'grep -qE "INFO \| php: .*bcmath" <<<"$OUT"'
check "core.hooksPath = scripts/git-hooks" '[[ "$(hooks_path)" == scripts/git-hooks ]]'
check "a nextest download that is not the pinned tarball is refused" \
  '[[ ! -e $TMP/p/usr/local/bin/cargo-nextest ]] && grep -qE "ERROR \| nextest: .*sha256" <<<"$OUT"'
check "a missing llvm-objcopy is a WARN with the remedy" 'grep -qE "WARN \| llvm-objcopy: .*llvm-tools" <<<"$OUT"'
check "every step logs its seconds" '[[ $(grep -cE "INFO \| (hooksPath|php|nextest|llvm-objcopy) took [0-9]+s" <<<"$OUT") -eq 4 ]]'

# ── 3. Idempotent: a second run reinstalls nothing ──────────────────────────────────────────────────────────
: >"$TMP/calls"; run /root true
check "second run: exit 0, no apt-get, no key download" '[[ $RC -eq 0 ]] && ! grep -qE "^apt-get|apt\.gpg" $TMP/calls'

# ── 4. The key is refused unless it is exactly the pinned one ───────────────────────────────────────────────
for c in "other.gpg|a key with another fingerprint" "both.gpg|the pinned key with an extra key riding along"; do
  f="${c%%|*}"; what="${c#*|}"
  fresh; CS_KEY="$TMP/$f" run /root true
  check "$what: ERROR, no keyring, no apt source, apt never runs, exit 0" \
    '[[ $RC -eq 0 && ! -e $KR && ! -e $SRC ]] && ! grep -q "^apt-get" $TMP/calls && grep -qE "ERROR \| php: .*key" <<<"$OUT"'
done
fresh; CS_FAIL=1 run /root true
check "a failed key download: ERROR, apt never runs, exit 0" \
  '[[ $RC -eq 0 && ! -e $KR ]] && ! grep -q "^apt-get" $TMP/calls && grep -qE "ERROR \| php: .*download" <<<"$OUT"'
check "a failed download still sets hooksPath (the steps are independent)" '[[ "$(hooks_path)" == scripts/git-hooks ]]'

# ── 5. Nothing leaks: the hook's temp dirs are gone ─────────────────────────────────────────────────────────
check "no temp dir left in TMPDIR" '[[ -z "$(ls -A "$TMP/tmp")" ]]'

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 && $PASS -gt 0 ]]
