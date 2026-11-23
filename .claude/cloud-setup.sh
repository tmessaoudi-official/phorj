#!/usr/bin/env bash
# phorj's toolchain hook for claude.ai cloud environments (DEC-577). The developer's global kit `claude-code-setup`
# (its cloud/setup.sh is the environment's setup script) runs this file as root, from the workspace root, before Claude
# starts, with stdout and stderr appended to /tmp/ccs-setup.log, inside ONE time budget shared by every workspace's hook
# (CCS_HOOK_BUDGET, default 120 s). It installs what phorj's gates need beyond the kit:
#   1. core.hooksPath scripts/git-hooks  — pre-commit and the FULL pre-push gate run as on the PC (D4); pre-push stays
#                                          fail-closed, so a push needs step 3's oracle;
#   2. gpg + PyYAML                      — only when absent, from the image's own archive: step 3's trust check needs gpg, and
#                                          scripts/test-cloud-ready.sh (run by pre-push) imports yaml;
#   3. PHP 8.5 + bcmath                  — the byte-identity oracle, from packages.sury.org (D2; Ubuntu ships 8.3 and the
#                                          Launchpad PPA is being merged into sury). The key file is trusted only when gpg
#                                          parses it cleanly and it holds exactly ONE key (public or secret) with the
#                                          pinned fingerprint, and the keyring apt gets is `gpg --export` of that one
#                                          fingerprint, never the downloaded file; the patch version floats
#                                          (scripts/validate-infra.sh forbids patch pins);
#   3b. PHP <FORWARD_PHP_MINOR>          — the forward build (8.6), step 3's package set, from the same sury source, in its own apt call: what
#                                          `scripts/dk --php 8.6` and scripts/php-forward-probe.sh use here (B2). Not a
#                                          pre-push need yet, so a failure is a WARN, and it runs LAST (after steps 4-5) so
#                                          it never spends a needed step's share of the budget; plan step 1c makes 8.6 the oracle;
#   4. cargo-nextest                     — a pinned release, sha256-checked (the hooks fall back to cargo test without it);
#   5. llvm-objcopy                      — checked only: tests/build.rs asserts it; a WARN names the remedy.
# Rules: a no-op off the cloud (the kit's own test, inline: this runs even when the kit's install failed); idempotent;
# one log line per step with its seconds; never fails the environment (exit 0). The PHP key and nextest pins are
# constants: no environment variable overrides them. PHORJ_CLOUD_SETUP_PREFIX only relocates the root-owned paths for
# scripts/test-cloud-setup.sh.
# Not installed: the tools no pre-push leg needs, which run in CI on every cloud branch (developer ruling D1, 2026-11-23:
# zig + cargo-zigbuild for `phg build --target`, wasm-pack + node for the playground and the VS Code extension, and the
# pdo_mysql/pdo_pgsql extensions). scripts/test-cloud-setup.sh pins that every docker/Dockerfile ARG is either read
# below or named on this line:
# CI-only in a cloud session (D1): ZIG_VERSION ZIGBUILD_VERSION WASM_PACK_VERSION NODE_MAJOR
set -u
[[ "${CLAUDE_CODE_REMOTE:-}" == true && "$(realpath -m -- "${HOME:-}")" == /root ]] || exit 0

P="${PHORJ_CLOUD_SETUP_PREFIX:-}"
SURY_FPR=15058500A0235D97F5D10063B188E2B695BD4743   # DEB.SURY.ORG Automatic Signing Key, expires 2028-02-04
SURY_KEY_URL=https://packages.sury.org/php/apt.gpg
SURY_REPO=https://packages.sury.org/php/
KEYRING=/usr/share/keyrings/debsuryorg-archive-keyring.gpg
# Versions come from docker/Dockerfile, the one place a tool version lives (docs/plans/2026-11-19-dockerize.plan.md):
# this script provisions the host runner a cloud session uses instead of the image (D2). Read beside the script, not from the cwd.
DOCKERFILE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/docker/Dockerfile"
dockerfile_arg() { sed -n "s/^ARG $1=//p" "$DOCKERFILE" 2>/dev/null | head -n1; }
PHP_MINOR="$(dockerfile_arg PHP_MINOR)"
PHP_PKGS=("php$PHP_MINOR-cli" "php$PHP_MINOR-bcmath" "php$PHP_MINOR-mbstring" "php$PHP_MINOR-sqlite3")
FORWARD_PHP_MINOR="$(dockerfile_arg FORWARD_PHP_MINOR)"
NEXTEST_VER="$(dockerfile_arg NEXTEST_VERSION)"
NEXTEST_SHA_FOR=0.9.152                               # the version the sha256 below was computed for, 2026-11-19
NEXTEST_URL="https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-$NEXTEST_VER/cargo-nextest-$NEXTEST_VER-x86_64-unknown-linux-gnu.tar.gz"
NEXTEST_SHA=d039b43c3e0958441b6ad0a39c8884d018b3d2e8fb9928f209074b1100b5e17a

say() { printf '%s | phorj-cloud-setup | %s | %s\n' "$(date -u +%FT%TZ)" "$1" "$2"; }
timed() { local name=$1 t0=$SECONDS; shift; "$@"; say INFO "$name took $((SECONDS - t0))s"; }
# stdout's module list is the verdict; a half-configured php prints startup warnings on stderr, noise for this check
has_bcmath() { command -v "php$PHP_MINOR" >/dev/null && [[ "$("php$PHP_MINOR" -m 2>/dev/null | grep -cx bcmath)" != 0 ]]; }

hooks_path() {
  if [[ ! -f scripts/git-hooks/pre-push ]]; then
    say WARN "hooksPath: $(pwd) has no scripts/git-hooks/pre-push: not phorj's root, left unset"; return 0
  fi
  git config core.hooksPath scripts/git-hooks && say INFO "hooksPath: core.hooksPath = scripts/git-hooks"
  # The hooks and scripts/dk run on this machine, provisioned by this script (D2: docker may run in a cloud session,
  # but the host runner is proven and needs no Docker Hub pulls or cold image build). Said explicitly, because dk never
  # infers the runner.
  git config phorj.runner host && say INFO "runner: phorj.runner = host (the host runner, D2)"
}

prereqs() {
  local need=()
  command -v gpg >/dev/null || need+=(gnupg)
  # the import's exit status is the answer; the ModuleNotFoundError traceback it prints when absent is not news
  python3 -c 'import yaml' >/dev/null 2>&1 || need+=(python3-yaml)
  if [[ ${#need[@]} -eq 0 ]]; then say INFO "prereqs: gpg and PyYAML present"; return 0; fi
  if ! { apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "${need[@]}"; }; then
    say ERROR "prereqs: apt-get failed for ${need[*]} (output above)"
  fi
  # judged by what is there afterwards, not by apt's exit status
  local still=()
  command -v gpg >/dev/null || still+=(gnupg)
  python3 -c 'import yaml' >/dev/null 2>&1 || still+=(python3-yaml)
  if [[ ${#still[@]} -eq 0 ]]; then say INFO "prereqs: installed ${need[*]} from the image archive"
  else say ERROR "prereqs: still missing: ${still[*]}: pre-push will refuse every push"; fi
}

php_oracle() {
  if [[ -z "$PHP_MINOR" ]]; then say ERROR "php: no ARG PHP_MINOR in $DOCKERFILE: PHP skipped, pre-push will refuse every push"; return 0; fi
  if has_bcmath; then say INFO "php: php$PHP_MINOR with bcmath already present ($(command -v "php$PHP_MINOR"))"; return 0; fi
  if ! command -v gpg >/dev/null; then
    say ERROR "php: gpg is not installed (see prereqs), the sury key cannot be checked: PHP skipped, pre-push will refuse every push"
    return 0
  fi
  # Four refusals, then a re-export: (1) gpg must parse the whole file (trailing junk made gpg exit 2 yet still list only
  # the pinned key, with a foreign key in the file); (2) awk must run: joined to the comparison with `&&`, a missing awk
  # (status 127) once made the test false and fell through to TRUST the key; (3) one fingerprint line per PRIMARY key,
  # public (`pub`) or secret (`sec`), so any extra key gives two lines and never equals the pin (never `exit` the awk at
  # the first one); (4) the keyring apt gets is `gpg --export` of the pinned fingerprint, so nothing else the file held
  # can reach apt. gpg's stderr goes to a file and is quoted only on failure (a fresh homedir prints "keybox created").
  local t fprs codename
  t="$(mktemp -d)"
  mkdir -m 700 "$t/gnupg"
  if ! curl -fsSL --max-time 30 -o "$t/apt.gpg" "$SURY_KEY_URL"; then
    say ERROR "php: sury key download failed ($SURY_KEY_URL): PHP skipped, pre-push will refuse every push"
  elif ! gpg --homedir "$t/gnupg" --batch --dearmor <"$t/apt.gpg" >"$t/key.gpg" 2>"$t/gpg.err"; then
    say ERROR "php: sury key is not an OpenPGP key (gpg: $(tr '\n' ' ' <"$t/gpg.err" | cut -c1-300)): PHP skipped"
  elif ! gpg --homedir "$t/gnupg" --batch --show-keys --with-colons "$t/key.gpg" >"$t/keys.txt" 2>"$t/gpg.err"; then
    say ERROR "php: gpg rejects the sury key file (gpg: $(tr '\n' ' ' <"$t/gpg.err" | cut -c1-300)): PHP skipped"
  elif ! fprs="$(awk -F: '$1=="pub"||$1=="sec"{want=1} $1=="fpr"&&want{print $10; want=0}' "$t/keys.txt")"; then
    say ERROR "php: awk failed listing the sury key's fingerprints: PHP skipped"
  elif [[ "$fprs" != "$SURY_FPR" ]]; then
    say ERROR "php: sury key is not exactly the pinned key $SURY_FPR (got: $(tr '\n' ' ' <<<"$fprs")): PHP skipped"
  elif ! { gpg --homedir "$t/gnupg" --batch --import "$t/key.gpg" 2>"$t/gpg.err" \
           && gpg --homedir "$t/gnupg" --batch --export "$SURY_FPR" >"$t/pinned.gpg" 2>>"$t/gpg.err" \
           && [[ -s "$t/pinned.gpg" ]]; }; then
    say ERROR "php: the pinned key cannot be exported (gpg: $(tr '\n' ' ' <"$t/gpg.err" | cut -c1-300)): PHP skipped"
  # a missing os-release is reported by the codename test itself, so sed's own complaint is not needed
  elif codename="$(sed -n 's/^VERSION_CODENAME=//p' "$P/etc/os-release" 2>/dev/null | tr -d '"')"
       [[ ! "$codename" =~ ^[a-z]+$ ]]; then
    say ERROR "php: no VERSION_CODENAME in $P/etc/os-release: PHP skipped"
  elif ! install -D -m 0644 "$t/pinned.gpg" "$P$KEYRING"; then
    say ERROR "php: cannot write the keyring $KEYRING (output above): PHP skipped, no apt source written"
  elif ! { mkdir -p "$P/etc/apt/sources.list.d" \
           && printf 'deb [signed-by=%s] %s %s main\n' "$KEYRING" "$SURY_REPO" "$codename" >"$P/etc/apt/sources.list.d/php.list"; }; then
    say ERROR "php: cannot write the apt source (output above): PHP skipped"
  elif apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "${PHP_PKGS[@]}"; then
    if has_bcmath; then say INFO "php: $(command -v "php$PHP_MINOR") with bcmath installed from sury ($codename)"
    else say ERROR "php: apt succeeded but php$PHP_MINOR -m lists no bcmath"; fi
  else
    say ERROR "php: apt-get failed for ${PHP_PKGS[*]} (output above): pre-push will refuse every push"
  fi
  rm -rf -- "$t"
}

php_forward() {
  local v="$FORWARD_PHP_MINOR" p
  if [[ -z "$v" ]]; then say WARN "php-forward: no ARG FORWARD_PHP_MINOR in $DOCKERFILE: skipped"; return 0; fi
  if p="$(command -v "php$v")" && [[ "$("$p" -m 2>/dev/null | grep -cx bcmath)" != 0 ]]; then
    say INFO "php-forward: php$v with bcmath already present ($p)"; return 0
  fi
  if [[ ! -f "$P/etc/apt/sources.list.d/php.list" ]]; then
    say WARN "php-forward: no sury apt source (the php step wrote none): php$v not installed, scripts/dk --php $v refuses"
    return 0
  fi
  if apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "${PHP_PKGS[@]//php$PHP_MINOR-/php$v-}" \
     && p="$(command -v "php$v")" && [[ "$("$p" -m 2>/dev/null | grep -cx bcmath)" != 0 ]]; then
    say INFO "php-forward: $p with bcmath installed from sury"
  else
    say WARN "php-forward: ${PHP_PKGS[*]//php$PHP_MINOR-/php$v-} not installed (output above): scripts/dk --php $v refuses; the oracle is unaffected"
  fi
}

nextest() {
  if command -v cargo-nextest >/dev/null; then say INFO "nextest: already present ($(command -v cargo-nextest))"; return 0; fi
  if [[ "$NEXTEST_VER" != "$NEXTEST_SHA_FOR" ]]; then
    say ERROR "nextest: docker/Dockerfile names ${NEXTEST_VER:-no NEXTEST_VERSION}, the sha256 here is for $NEXTEST_SHA_FOR: recompute it; not installed"
    return 0
  fi
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
timed php-forward php_forward   # last: the one optional step, so it never costs a needed one the shared time budget
exit 0
