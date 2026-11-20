# syntax=docker/dockerfile:1
# phorj's whole toolchain in one image (docs/plans/2026-11-19-dockerize.plan.md): the host needs docker, bash and git.
# Every gate, git hook, CI job and script runs here through `scripts/dk` (or `docker compose run --rm dev …`).
#
# THIS FILE IS THE ONE PLACE A TOOL VERSION LIVES (the ARGs below), apart from two that have their own SSOT:
#   - Rust: `rust-toolchain.toml` (copied in and installed as written: no second version entry here);
#   - PHP: the minor only. The tag floats the patch, as `scripts/toolchain.env`'s `php-8.5.*` glob does, because the
#     oracle is never pinned to a patch (CLAUDE.md, "Toolchain & quality gate"). PHP_MINOR=8.6 builds the image of
#     CI's `gate-php86` job. `.claude/cloud-setup.sh` reads PHP_MINOR from here for the cloud fallback.
ARG PHP_MINOR=8.5
ARG NODE_MAJOR=26
# The newest PHP after the floor, for scripts/php-forward-probe.sh (lint only).
ARG FORWARD_PHP_MINOR=8.6

FROM node:${NODE_MAJOR}-trixie-slim AS node
FROM php:${FORWARD_PHP_MINOR}-cli-trixie AS forward-php

FROM php:${PHP_MINOR}-cli-trixie AS dev
ARG NEXTEST_VERSION=0.9.152
ARG ZIGBUILD_VERSION=0.24.1
ARG ZIG_VERSION=0.17.1
ARG ZIG_SHA256=3581863e3cbcfd0fce0efc682ce7e9e93da96668a977791d356a26d2a7e9b17a
ARG WASM_PACK_VERSION=v0.15.1
ARG WASM_PACK_SHA256=410101009739b9d01c7d5affaaad6e9c9a24b4cba6791a4805db8e89b9bf7af2

# System tools the gates and tests call (git, jq, python3, flock/taskset, sha256sum, openssl, curl) and the libraries
# the PHP extensions build against. build-essential: cargo build scripts (cc) and rusqlite's bundled SQLite.
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      build-essential pkg-config git jq python3 python3-yaml util-linux coreutils openssl curl ca-certificates xz-utils \
      libpq-dev libssl-dev \
 && rm -rf /var/lib/apt/lists/*

# bcmath is the oracle's one hard requirement (scripts/toolchain.env); the PDO drivers serve the live database legs
# (they self-skip without a DSN). Built as shared extensions with their .ini, so they load under plain `php`; the
# oracle runs `php -n`, hence PHP_INI_SCAN_DIR is not relied on: see the `-n` note in the php wrapper below.
RUN docker-php-ext-install -j"$(nproc)" bcmath pdo_mysql pdo_pgsql

# `php -n` (how every test runs the oracle) skips the ini files, so shared extensions would not load. The wrapper adds
# them back explicitly for `-n` runs only, so `-n` still means "no user ini, deterministic", as on the local oracle
# whose bcmath was compiled in.
RUN mv /usr/local/bin/php /usr/local/bin/php.real \
 && extdir="$(/usr/local/bin/php.real -r 'echo ini_get("extension_dir");')" \
 && printf '%s\n' '#!/bin/sh' \
      '# phorj: `php -n` loads no ini, so the shared extensions docker-php-ext-install built are named here.' \
      'for a in "$@"; do' \
      '  if [ "$a" = "-n" ]; then' \
      "    exec /usr/local/bin/php.real -d extension_dir=$extdir -d extension=bcmath -d extension=pdo_mysql -d extension=pdo_pgsql \"\$@\"" \
      '  fi' \
      'done' \
      'exec /usr/local/bin/php.real "$@"' > /usr/local/bin/php \
 && chmod 0755 /usr/local/bin/php \
 && php -n -m | grep -qx bcmath

# Node (playground tests, the editor extension's npm) from the official image of the same Debian release.
COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
 && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx \
 && node --version && npm --version

# Rust: rustup, then exactly the toolchain rust-toolchain.toml names (its components and wasm target included),
# llvm-tools for llvm-objcopy (`phg build`, tests/build.rs), and the cross targets of `phg build --target`.
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:/opt/zig:$PATH
COPY rust-toolchain.toml /tmp/toolchain/rust-toolchain.toml
RUN curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o /tmp/rustup-init.sh \
 && sh /tmp/rustup-init.sh -y --no-modify-path --profile minimal --default-toolchain none \
 && rm /tmp/rustup-init.sh \
 && cd /tmp/toolchain \
 && rustup show \
 && rustup component add llvm-tools \
 && rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-gnu aarch64-unknown-linux-musl x86_64-pc-windows-gnu \
 && ln -s "$(find "$(rustc --print sysroot)" -name llvm-objcopy -type f | head -n1)" /usr/local/bin/llvm-objcopy \
 && cargo install --locked --version "$NEXTEST_VERSION" cargo-nextest \
 && cargo install --locked --version "$ZIGBUILD_VERSION" cargo-zigbuild \
 && rm -rf /usr/local/cargo/registry /usr/local/cargo/git \
 && chmod -R a+rX /usr/local/rustup /usr/local/cargo

# zig (cargo-zigbuild's cross linker) and wasm-pack: official release archives, checksum-verified.
RUN curl -fsSL "https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-linux-${ZIG_VERSION}.tar.xz" -o /tmp/zig.tar.xz \
 && echo "${ZIG_SHA256}  /tmp/zig.tar.xz" | sha256sum -c - \
 && mkdir -p /opt/zig && tar -xf /tmp/zig.tar.xz -C /opt/zig --strip-components=1 && rm /tmp/zig.tar.xz \
 && name="wasm-pack-${WASM_PACK_VERSION}-x86_64-unknown-linux-musl" \
 && curl -fsSL -o /tmp/wasm-pack.tar.gz \
      "https://github.com/wasm-bindgen/wasm-pack/releases/download/${WASM_PACK_VERSION}/${name}.tar.gz" \
 && echo "${WASM_PACK_SHA256}  /tmp/wasm-pack.tar.gz" | sha256sum -c - \
 && tar -xzf /tmp/wasm-pack.tar.gz -C /tmp && install -m 0755 "/tmp/${name}/wasm-pack" /usr/local/bin/wasm-pack \
 && rm -rf /tmp/wasm-pack.tar.gz "/tmp/${name}" \
 && zig version && wasm-pack --version

# Tools only the gates' own behaviour suites use: gnupg for scripts/test-cloud-setup.sh's key fixtures. A layer of its
# own, below the Rust layers, so adding one never rebuilds the toolchain.
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends gnupg \
 && rm -rf /var/lib/apt/lists/*

# scripts/php-forward-probe.sh lints transpiled output on every PHP newer than the oracle: the next minor's CLI binary,
# from the official image of the same Debian release (its shared libraries are a subset of this image's, checked
# with ldd 2026-11-19). `php -l` needs no extension. In the PHP_MINOR=8.6 image it equals the oracle and is ignored.
ARG FORWARD_PHP_MINOR
COPY --from=forward-php /usr/local/bin/php /opt/php-forward/php-${FORWARD_PHP_MINOR}/bin/php
RUN /opt/php-forward/php-${FORWARD_PHP_MINOR}/bin/php -v | head -n1
ENV PHORJ_FORWARD_PHPS=/opt/php-forward/php-${FORWARD_PHP_MINOR}/bin/php

# The container runs as the developer's uid (compose.yaml), who owns nothing in the image. Writable state therefore
# lives in two places: the cargo registry/git cache in the named volume mounted at /cache/cargo (a new volume copies
# this directory's mode, so it starts world-writable), and HOME, a scratch directory per container.
RUN mkdir -p /cache/cargo /home/dev && chmod 1777 /cache/cargo /home/dev
# PHORJ_PHP_SOURCE: php.real is the official php image's own binary, so scripts/microbench-gate.sh measures the same
# comparator as `docker php:<minor>-cli` (the baseline's recorded source) and keeps gating instead of skipping.
ARG PHP_MINOR
ENV CARGO_HOME=/cache/cargo \
    HOME=/home/dev \
    PHORJ_PHP=/usr/local/bin/php \
    PHORJ_PHP_SOURCE="docker php:${PHP_MINOR}-cli" \
    PHORJ_IN_DOCKER=1

# Last, so it never invalidates a layer above: the git blob hashes of this file, its .dockerignore and
# rust-toolchain.toml, which scripts/dk compares with its checkout to rebuild only when an input changed (a compose
# `run --build` cache check costs ~30 s per call on this box; reading a label costs nothing).
ARG PHORJ_INPUTS=unset
LABEL org.phorj.inputs=$PHORJ_INPUTS
