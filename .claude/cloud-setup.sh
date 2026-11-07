#!/usr/bin/env bash
# phorj's toolchain hook for claude.ai cloud environments (DEC-577). The developer's global kit `claude-code-setup`
# (its cloud/setup.sh is the environment's setup script) runs this file as root, from the workspace root, before Claude
# starts, with stdout and stderr appended to /tmp/ccs-setup.log, inside ONE time budget shared by every workspace's hook
# (CCS_HOOK_BUDGET, default 120 s). It installs what phorj's gates need beyond the kit:
#   1. core.hooksPath scripts/git-hooks  — pre-commit and the FULL pre-push gate run as on the PC (D4); pre-push stays
#                                          fail-closed, so a push needs step 2's oracle;
#   2. gpg + PyYAML                      — only when absent, from the image's own archive: step 3's trust check needs gpg, and
#                                          scripts/test-cloud-ready.sh (run by pre-push) imports yaml;
#   3. PHP 8.5 + bcmath                  — the byte-identity oracle, from packages.sury.org (D2; Ubuntu ships 8.3 and the
#                                          Launchpad PPA is being merged into sury). The key is trusted only when it is
#                                          exactly ONE key (public or secret) with the pinned fingerprint; the patch
#                                          version floats (scripts/validate-infra.sh forbids patch pins);
#   4. cargo-nextest                     — a pinned release, sha256-checked (the hooks fall back to cargo test without it);
#   5. llvm-objcopy                      — checked only: tests/build.rs asserts it; a WARN names the remedy.
# Rules: a no-op off the cloud (the kit's own test, inline: this runs even when the kit's install failed); idempotent;
# one log line per step with its seconds; never fails the environment (exit 0). The PHP key and nextest pins are
# constants: no environment variable overrides them. PHORJ_CLOUD_SETUP_PREFIX only relocates the root-owned paths for
# scripts/test-cloud-setup.sh.
set -u
[[ "${CLAUDE_CODE_REMOTE:-}" == true && "$(realpath -m -- "${HOME:-}")" == /root ]] || exit 0

P="${PHORJ_CLOUD_SETUP_PREFIX:-}"
SURY_FPR=15058500A0235D97F5D10063B188E2B695BD4743   # DEB.SURY.ORG Automatic Signing Key, expires 2028-02-04
SURY_KEY_URL=https://packages.sury.org/php/apt.gpg
SURY_REPO=https://packages.sury.org/php/
KEYRING=/usr/share/keyrings/debsuryorg-archive-keyring.gpg
PHP_PKGS=(php8.5-cli php8.5-bcmath php8.5-mbstring php8.5-sqlite3)
NEXTEST_VER=0.9.149                                   # released 2026-10-20
NEXTEST_URL="https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-$NEXTEST_VER/cargo-nextest-$NEXTEST_VER-x86_64-unknown-linux-gnu.tar.gz"
NEXTEST_SHA=32d1d8c0567b7a80bee2bad8bb09125c846da96bc009b2de11ec08557290c770

say() { printf '%s | phorj-cloud-setup | %s | %s\n' "$(date -u +%FT%TZ)" "$1" "$2"; }
timed() { local name=$1 t0=$SECONDS; shift; "$@"; say INFO "$name took $((SECONDS - t0))s"; }
# stdout's module list is the verdict; a half-configured php prints startup warnings on stderr, noise for this check
has_bcmath() { command -v php8.5 >/dev/null && [[ "$(php8.5 -m 2>/dev/null | grep -cx bcmath)" != 0 ]]; }

hooks_path() {
  if [[ ! -f scripts/git-hooks/pre-push ]]; then
    say WARN "hooksPath: $(pwd) has no scripts/git-hooks/pre-push: not phorj's root, left unset"; return 0
  fi
  git config core.hooksPath scripts/git-hooks && say INFO "hooksPath: core.hooksPath = scripts/git-hooks"
}

prereqs() {
  local need=()
  command -v gpg >/dev/null || need+=(gnupg)
  # the import's exit status is the answer; the ModuleNotFoundError traceback it prints when absent is not news
  python3 -c 'import yaml' >/dev/null 2>&1 || need+=(python3-yaml)
  if [[ ${#need[@]} -eq 0 ]]; then say INFO "prereqs: gpg and PyYAML present"; return 0; fi
  if apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "${need[@]}"; then
    say INFO "prereqs: installed ${need[*]} from the image archive"
  else
    say ERROR "prereqs: apt-get failed for ${need[*]} (output above)"
  fi
}

php_oracle() {
  if has_bcmath; then say INFO "php: php8.5 with bcmath already present ($(command -v php8.5))"; return 0; fi
  if ! command -v gpg >/dev/null; then
    say ERROR "php: gpg is not installed (see prereqs), the sury key cannot be checked: PHP skipped, pre-push will refuse every push"
    return 0
  fi
  # One line per PRIMARY key, public (`pub`) or secret (`sec`): a file with any extra key prints two lines and can never
  # equal the single pin. Never stop at the first fingerprint (`exit` in the awk): that would trust a key riding along
  # behind the pinned one. gpg's stderr goes to a file and is quoted only on failure (a fresh homedir also prints
  # "keybox created" there, noise on success).
  local t fprs codename
  t="$(mktemp -d)"
  mkdir -m 700 "$t/gnupg"
  if ! curl -fsSL --max-time 30 -o "$t/apt.gpg" "$SURY_KEY_URL"; then
    say ERROR "php: sury key download failed ($SURY_KEY_URL): PHP skipped, pre-push will refuse every push"
  elif ! gpg --homedir "$t/gnupg" --batch --dearmor <"$t/apt.gpg" >"$t/key.gpg" 2>"$t/gpg.err"; then
    say ERROR "php: sury key is not an OpenPGP key (gpg: $(tr '\n' ' ' <"$t/gpg.err" | cut -c1-300)): PHP skipped"
  elif fprs="$(gpg --homedir "$t/gnupg" --batch --show-keys --with-colons "$t/key.gpg" 2>"$t/gpg.err" \
                | awk -F: '$1=="pub"||$1=="sec"{want=1} $1=="fpr"&&want{print $10; want=0}')" \
       && [[ "$fprs" != "$SURY_FPR" ]]; then
    say ERROR "php: sury key is not exactly the pinned key $SURY_FPR (got: $(tr '\n' ' ' <<<"$fprs"); gpg: $(tr '\n' ' ' <"$t/gpg.err" | cut -c1-300)): PHP skipped"
  # a missing os-release is reported by the codename test itself, so sed's own complaint is not needed
  elif codename="$(sed -n 's/^VERSION_CODENAME=//p' "$P/etc/os-release" 2>/dev/null | tr -d '"')"
       [[ ! "$codename" =~ ^[a-z]+$ ]]; then
    say ERROR "php: no VERSION_CODENAME in $P/etc/os-release: PHP skipped"
  elif ! install -D -m 0644 "$t/key.gpg" "$P$KEYRING"; then
    say ERROR "php: cannot write the keyring $KEYRING (output above): PHP skipped, no apt source written"
  elif ! { mkdir -p "$P/etc/apt/sources.list.d" \
           && printf 'deb [signed-by=%s] %s %s main\n' "$KEYRING" "$SURY_REPO" "$codename" >"$P/etc/apt/sources.list.d/php.list"; }; then
    say ERROR "php: cannot write the apt source (output above): PHP skipped"
  elif apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "${PHP_PKGS[@]}"; then
    if has_bcmath; then say INFO "php: $(command -v php8.5) with bcmath installed from sury ($codename)"
    else say ERROR "php: apt succeeded but php8.5 -m lists no bcmath"; fi
  else
    say ERROR "php: apt-get failed for ${PHP_PKGS[*]} (output above): pre-push will refuse every push"
  fi
  rm -rf -- "$t"
}

nextest() {
  if command -v cargo-nextest >/dev/null; then say INFO "nextest: already present ($(command -v cargo-nextest))"; return 0; fi
  local t; t="$(mktemp -d)"
  if curl -fsSL --max-time 60 -o "$t/n.tgz" "$NEXTEST_URL" \
     && printf '%s  %s\n' "$NEXTEST_SHA" "$t/n.tgz" | sha256sum -c --status; then
    if tar -xzf "$t/n.tgz" -C "$t" cargo-nextest && install -D -m 0755 "$t/cargo-nextest" "$P/usr/local/bin/cargo-nextest"; then
      say INFO "nextest: cargo-nextest $NEXTEST_VER installed (sha256 checked)"
    else
      say ERROR "nextest: extract or install failed"
    fi
  else
    say ERROR "nextest: download failed or sha256 is not the pinned $NEXTEST_SHA: not installed (hooks fall back to cargo test)"
  fi
  rm -rf -- "$t"
}

objcopy() {
  if command -v llvm-objcopy >/dev/null; then say INFO "llvm-objcopy: present ($(command -v llvm-objcopy))"; return 0; fi
  say WARN "llvm-objcopy: absent, tests/build.rs will fail; remedy: rustup component add llvm-tools, then link \$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-objcopy into /usr/local/bin"
}

say INFO "start in $(pwd)"
timed hooksPath hooks_path
timed prereqs prereqs
timed php php_oracle
timed nextest nextest
timed llvm-objcopy objcopy
exit 0
