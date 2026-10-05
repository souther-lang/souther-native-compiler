#!/usr/bin/env bash
# examples/rust-cart as its README says to run it: the model built by the command line, and Cargo
# building the application over the binding and running its tests, the HTTP contract and what rustc
# refuses. The rows of cart.sou run in the build, so a model that stopped holding them fails here too.
#
# Needs what rust-from-the-command-line.sh needs, and a C compiler for the SQLite rusqlite builds.
set -euo pipefail

example="$(cd "$(dirname "$0")/../examples/rust-cart" && pwd)"

SOUTHER="${SOUTHER:-$(cd "$(dirname "$0")" && pwd)/souther}" "$example/bin/build"
cd "$example"
cargo test --locked
