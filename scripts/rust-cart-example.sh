#!/usr/bin/env bash
# examples/rust-cart as its README says to run it: the model built by the command line, and Cargo
# building the application over the binding and running its tests, the HTTP contract and what rustc
# refuses. The rows of cart.sou run in the build, so a model that stopped holding them fails here too.
#
# Every host's cart example runs one model, so the cart.sou of each is held to be the same file.
#
# Needs what rust-from-the-command-line.sh needs, and a C compiler for the SQLite rusqlite builds.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
example="$root/examples/rust-cart"

models=("$root"/examples/*/model/cart.sou)
for model in "${models[@]}"; do
    if ! cmp -s "${models[0]}" "$model"; then
        echo "${model#"$root"/} is not ${models[0]#"$root"/}; every host's cart runs one model" >&2
        diff "${models[0]}" "$model" >&2 || true
        exit 1
    fi
done

"$example/bin/build"
cd "$example"
cargo test --locked
