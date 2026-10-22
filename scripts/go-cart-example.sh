#!/usr/bin/env bash
# examples/go-cart as its README says to run it: the model built by the command line, and Go
# building the application over the binding and running its tests, the HTTP contract and what the
# runtime refuses a host of the model. The rows of cart.sou run in the build, so a model that
# stopped holding them fails here too. The application and the binding are what gofmt writes,
# go.mod and go.sum are what go mod tidy leaves, go.mod names the Go the runtime module does, and
# no type switch over a union or a sum's cases leaves one out.
#
# The example requires the runtime from the Go module proxy, as an application does. Where this
# clone's runtime is at a version the proxy does not have, which is the case from a change to the
# runtime until its tag is published, the binding this clone writes requires that version, and the
# example is built as a copy of itself with the runtime replaced by the clone's: go.mod and go.sum,
# which name the release, are not held to what go mod tidy leaves then.
#
# Needs what go-from-the-command-line.sh needs. SQLite is modernc.org/sqlite, which is Go.
set -euo pipefail

. "$(dirname "$0")/require-go.sh"

root="$(cd "$(dirname "$0")/.." && pwd)"
example="$root/examples/go-cart"
runtime="$root/bindings/go/runtime"
export SOUTHER="${SOUTHER:-$root/scripts/souther}"

# A go.mod has to say a Go, and the example's is the runtime module's, where the version is stated.
said="$(sed -nE 's/^go[[:space:]]+([0-9][0-9.]*)[[:space:]]*$/\1/p' "$example/go.mod")"
if [ "$said" != "$go_version" ]; then
    echo "examples/go-cart/go.mod says Go $said, and the runtime module Go $go_version" >&2
    exit 1
fi

module="$(sed -nE 's#^module[[:space:]]+([^[:space:]]+)[[:space:]]*$#\1#p' "$runtime/go.mod")"
version="$(tr -d '[:space:]' < "$runtime/VERSION")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if (cd "$work" && go list -m "$module@v$version" > /dev/null 2>&1); then
    "$example/bin/build"
    cd "$example"
    go mod tidy -diff
else
    echo "$module v$version is not on the module proxy: building examples/go-cart over $runtime" >&2
    mkdir "$work/examples"
    cp -R "$root/examples/cart-model" "$work/examples/"
    rsync -a --exclude /build "$example/" "$work/examples/go-cart/"
    "$work/examples/go-cart/bin/build"
    cd "$work/examples/go-cart"
    go mod edit -replace "$module=$runtime"
    go mod tidy
fi

unformatted="$(gofmt -l .)"
if [ -n "$unformatted" ]; then
    echo "gofmt would change these:" >&2
    echo "$unformatted" >&2
    exit 1
fi
go vet ./...
# Every type switch over a union or a sum's cases names every case, a default notwithstanding: Go
# does not check that, and the binding declares each such interface a sum type for this check.
# Run at a fixed version rather than as a tool of the module, whose go line it would raise past the
# runtime's. v0.4.0 does not see a sum type declared in another module, which the binding is.
go run github.com/alecthomas/go-check-sumtype/cmd/go-check-sumtype@v0.5.0 \
    -default-signifies-exhaustive=false ./...
go test ./...
