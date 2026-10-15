#!/usr/bin/env bash
# examples/rust-cart as its README says to run it: the model built by the command line, and Cargo
# building the application over the binding and running its tests, the HTTP contract and what rustc
# refuses. The rows of cart.sou run in the build, so a model that stopped holding them fails here too.
#
# The example takes the runtime from crates.io, as an application does. Where this clone's runtime
# is at a version crates.io does not have, which is the case from a change to the runtime until it
# is published, the binding this clone writes requires that version, and the example is built as a
# copy of itself with the runtime patched in from the clone: the lock, which names the release, is
# not held to it then.
#
# Needs what rust-from-the-command-line.sh needs, and a C compiler for the SQLite rusqlite builds.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
example="$root/examples/rust-cart"
runtime="$root/bindings/rust/runtime"
export SOUTHER="${SOUTHER:-$root/scripts/souther}"

version="$(sed -nE 's/^version = "(.*)"$/\1/p' "$runtime/Cargo.toml" | head -1)"

# The sparse index answers 404 for a crate it does not have, and a line per version for one it has.
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
status="$(curl -sS -o "$work/index" -w '%{http_code}' \
    https://index.crates.io/so/ut/souther-binding-runtime)"
case "$status" in
    200) published="$(grep -c "\"vers\":\"$version\"" "$work/index" || true)" ;;
    404) published=0 ;;
    *)
        echo "crates.io's index answered $status for souther-binding-runtime" >&2
        exit 1
        ;;
esac

if [ "$published" -gt 0 ]; then
    "$example/bin/build"
    cd "$example"
    cargo test --locked
    exit 0
fi

echo "souther-binding-runtime $version is not on crates.io: building examples/rust-cart over $runtime" >&2
mkdir "$work/examples"
cp -R "$root/examples/cart-model" "$work/examples/"
rsync -a --exclude /target --exclude /build "$example/" "$work/examples/rust-cart/"
# Laid out as Cargo lays out the source of a crate from a registry, which is where trybuild looks to
# write a path into another crate as $CARGO/<crate>-$VERSION, as the expected output has it.
patched="$work/registry/src/clone-0000000000000000/souther-binding-runtime-$version"
mkdir -p "$(dirname "$patched")"
rsync -a --exclude /target "$runtime/" "$patched/"
cat >> "$work/examples/rust-cart/Cargo.toml" <<EOF

[patch.crates-io]
souther-binding-runtime = { path = "$patched" }
EOF

"$work/examples/rust-cart/bin/build"
cd "$work/examples/rust-cart"
CARGO_TARGET_DIR="$example/target" cargo test
