#!/usr/bin/env bash
# Behaviour tests of the Docker toolchain (docs/plans/2026-11-19-dockerize.plan.md): the `dev` image carries every
# tool the gates need at the version the repo names, `scripts/dk` runs as the developer (files it writes stay theirs),
# forwards git's hook environment and stdin, and fails loudly instead of falling back to host tools.
# Needs only docker on the host. Run: bash scripts/test-docker.sh
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dk="$root/scripts/dk"
pass=0 fail=0
ok()  { pass=$((pass + 1)); echo "  ok   $1"; }
bad() { fail=$((fail + 1)); echo "  FAIL $1" >&2; }
check() { local name="$1"; shift; if "$@"; then ok "$name"; else bad "$name"; fi; }

[[ -x "$dk" ]] || { echo "test-docker: FAIL scripts/dk missing or not executable" >&2; exit 1; }
PHORJ_UID=1 PHORJ_GID=1 PHORJ_ROOT="$root" docker compose -f "$root/compose.yaml" config -q || { echo "test-docker: FAIL compose.yaml invalid" >&2; exit 1; }

# 1. Every tool, read inside the container in one run (one container start, not twenty).
out="$("$dk" bash -c '
  echo "rustc=$(rustc -V | cut -d" " -f2)"
  echo "php=$(php -r "echo PHP_MAJOR_VERSION.\".\".PHP_MINOR_VERSION;")"
  echo "bcmath=$(php -n -m | grep -cx bcmath)"
  echo "pdo_pgsql=$(php -n -m | grep -cx pdo_pgsql)"
  echo "pdo_mysql=$(php -n -m | grep -cx pdo_mysql)"
  echo "phorj_php=${PHORJ_PHP:-}"
  for t in cargo-nextest cargo-zigbuild zig wasm-pack node npm python3 jq git curl flock taskset sha256sum openssl llvm-objcopy; do
    command -v "$t" >/dev/null && echo "tool:$t=yes" || echo "tool:$t=no"
  done
  rustup target list --installed | sed "s/^/target:/"
  echo "uid=$(id -u):$(id -g)"
  echo "pwd=$PWD"
')"
want_rust="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$root/rust-toolchain.toml")"
check "rustc is rust-toolchain.toml's channel ($want_rust)" grep -qx "rustc=$want_rust" <<<"$out"
check "php is the 8.5 floor" grep -qx "php=8.5" <<<"$out"
check "bcmath loads under php -n" grep -qx "bcmath=1" <<<"$out"
check "pdo_pgsql loads under php -n" grep -qx "pdo_pgsql=1" <<<"$out"
check "pdo_mysql loads under php -n" grep -qx "pdo_mysql=1" <<<"$out"
check "PHORJ_PHP names the image's php" grep -qx "phorj_php=/usr/local/bin/php" <<<"$out"
while read -r t; do check "tool $t on PATH" grep -qx "tool:$t=yes" <<<"$out"; done < <(sed -n 's/^tool:\(.*\)=.*/\1/p' <<<"$out")
for t in wasm32-unknown-unknown x86_64-unknown-linux-musl aarch64-unknown-linux-gnu aarch64-unknown-linux-musl x86_64-pc-windows-gnu; do
  check "rust target $t installed" grep -qx "target:$t" <<<"$out"
done
check "runs as the host uid:gid" grep -qx "uid=$(id -u):$(id -g)" <<<"$out"
check "cwd is the repo at its host path" grep -qx "pwd=$root" <<<"$out"

# 2. A file the container writes belongs to the developer.
probe="$root/target/.dk-owner-probe"
rm -f -- "$probe"
"$dk" touch "$probe"
check "a written file is owned by the host uid" test "$(stat -c %u "$probe")" = "$(id -u)"
rm -f -- "$probe"

# 3. stdin reaches the command (pre-push reads its refs on stdin).
check "stdin is passed through" test "$(printf 'refs/heads/master abc\n' | "$dk" cat)" = "refs/heads/master abc"

# 4. git's hook environment is forwarded: a pathspec commit hands its hook a temporary index.
check "GIT_INDEX_FILE is forwarded" test "$(GIT_INDEX_FILE=/x/idx "$dk" bash -c 'echo "$GIT_INDEX_FILE"')" = "/x/idx"
check "PHORJ_* is forwarded" test "$(PHORJ_DK_PROBE=7 "$dk" bash -c 'echo "${PHORJ_DK_PROBE:-}"')" = "7"

# The host's own toolchain locations never leak in: the image's CARGO_HOME (the cache volume) and RUSTUP_HOME win.
check "a host CARGO_HOME is not forwarded" \
  test "$(CARGO_HOME=/host/cargo RUSTUP_HOME=/host/rustup "$dk" bash -c 'echo "$CARGO_HOME $RUSTUP_HOME"')" = "/cache/cargo /usr/local/rustup"

# 5. The exit status is the command's.
set +e; "$dk" bash -c 'exit 7'; rc=$?; set -e
check "the command's exit status comes back (7)" test "$rc" = 7

# 6. An unchanged checkout does not rebuild (the label check), so a hook pays ~2 s, not a build.
check "an unchanged checkout runs without rebuilding" test -z "$("$dk" true 2>&1 | grep -F 'dk: building' || true)"

# 7. Fail loudly: with docker unreachable and no explicit host runner, dk refuses instead of using host tools.
set +e; msg="$(PATH=/usr/bin:/bin DK_DOCKER=/nonexistent/docker "$dk" true 2>&1)"; rc=$?; set -e
check "no docker and no runner switch = a loud failure" test "$rc" -ne 0
check "the failure names the switch" grep -q "phorj.runner" <<<"$msg"

# 8. What the host really needs, pinned: docker (with its compose plugin), bash, git and five coreutils, and NOTHING
#    else on PATH. dk and a hook's re-exec must work from exactly this set, so a new host dependency fails here.
#    The hook's temporary index lives in .git/, as git's own pathspec-commit index does (the container mounts the repo,
#    not the host's /tmp).
closed="$(mktemp -d)"
for t in docker git bash dirname basename env id tr; do ln -s "$(command -v "$t")" "$closed/$t"; done
set +e; env -i HOME="$HOME" PATH="$closed" "$closed/bash" "$dk" true; rc=$?; set -e
check "dk runs from a PATH of docker, git, bash and five coreutils" test "$rc" = 0
idx="$(cd "$(git -C "$root" rev-parse --git-dir)" && pwd)/test-docker-index"
cp "$(git -C "$root" rev-parse --git-path index)" "$idx"
GIT_INDEX_FILE="$idx" git -C "$root" update-index --cacheinfo "100644,$(git -C "$root" rev-parse HEAD:CONTRIBUTING.md),README.md"
set +e; out="$(cd "$root" && env -i HOME="$HOME" PATH="$closed" GIT_INDEX_FILE="$idx" "$closed/bash" scripts/git-hooks/pre-commit 2>&1)"; rc=$?; set -e
rm -f -- "$idx"; rm -rf -- "$closed"
check "a hook re-executes through dk from that PATH and passes (no-Rust path)" test "$rc" = 0
check "…and it did run in the container (its NO-RUST line)" grep -q "NO-RUST fast path" <<<"$out"

echo "test-docker: $pass passed, $fail failed"
(( fail == 0 ))
