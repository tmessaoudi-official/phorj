#!/usr/bin/env bash
# Build the encrypted cloud kit from the developer's two exports (DEC-570). Only ciphertext leaves this script.
#   pack.sh AGE_RECIPIENT GLOBAL.tgz MEMORIES.tgz OUT.age
# AGE_RECIPIENT is the PUBLIC key (age1…); the private key never touches the repo.
set -euo pipefail
[[ $# -eq 4 ]] || { echo "usage: pack.sh AGE_RECIPIENT GLOBAL.tgz MEMORIES.tgz OUT.age" >&2; exit 2; }
recip="$1" g="$2" m="$3" out="$4"
[[ "$recip" == age1* ]] || { echo "pack.sh: the recipient must be an age PUBLIC key (age1…), never a secret key" >&2; exit 2; }
command -v age >/dev/null || { echo "pack.sh: age is not installed" >&2; exit 4; }
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
cp "$g" "$W/global.tar.gz"; cp "$m" "$W/memories.tar.gz"
tar -cf "$W/kit.tar" -C "$W" global.tar.gz memories.tar.gz
age -r "$recip" -o "$out" "$W/kit.tar"
echo "pack.sh: wrote $out ($(wc -c <"$out") bytes, encrypted to $recip)"
