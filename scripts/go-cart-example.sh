#!/usr/bin/env bash
# examples/go-cart as its README says to run it: the model built by the command line, and Go
# building the application over the binding and running its tests, the HTTP contract and what the
# runtime refuses a host of the model. The rows of cart.sou run in the build, so a model that
# stopped holding them fails here too. The application and the binding are what gofmt writes,
# go.mod and go.sum are what go mod tidy leaves, go.mod names the Go the runtime module does, and
# no type switch over a union or a sum's cases leaves one out.
#
# Needs what go-from-the-command-line.sh needs. SQLite is modernc.org/sqlite, which is Go.
set -euo pipefail

. "$(dirname "$0")/require-go.sh"

example="$(cd "$(dirname "$0")/../examples/go-cart" && pwd)"

# A go.mod has to say a Go, and the example's is the runtime module's, where the version is stated.
said="$(sed -nE 's/^go[[:space:]]+([0-9][0-9.]*)[[:space:]]*$/\1/p' "$example/go.mod")"
if [ "$said" != "$go_version" ]; then
    echo "examples/go-cart/go.mod says Go $said, and the runtime module Go $go_version" >&2
    exit 1
fi

SOUTHER="${SOUTHER:-$(cd "$(dirname "$0")" && pwd)/souther}" "$example/bin/build"
cd "$example"
unformatted="$(gofmt -l .)"
if [ -n "$unformatted" ]; then
    echo "gofmt would change these:" >&2
    echo "$unformatted" >&2
    exit 1
fi
go mod tidy -diff
go vet ./...
# Every type switch over a union or a sum's cases names every case, a default notwithstanding: Go
# does not check that, and the binding declares each such interface a sum type for this check.
# Run at a fixed version rather than as a tool of the module, whose go line it would raise past the
# runtime's. v0.4.0 does not see a sum type declared in another module, which the binding is.
go run github.com/alecthomas/go-check-sumtype/cmd/go-check-sumtype@v0.5.0 \
    -default-signifies-exhaustive=false ./...
go test ./...
