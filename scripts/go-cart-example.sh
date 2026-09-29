#!/usr/bin/env bash
# examples/go-cart as its README says to run it: the model built by the command line, and Go
# building the application over the binding and running its tests, the HTTP contract and what the
# runtime refuses a host of the model. The rows of cart.sou run in the build, so a model that
# stopped holding them fails here too. The application and the binding are what gofmt writes,
# go.mod and go.sum are what go mod tidy leaves, and go.mod names the Go the runtime module does.
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

"$example/bin/build"
cd "$example"
unformatted="$(gofmt -l .)"
if [ -n "$unformatted" ]; then
    echo "gofmt would change these:" >&2
    echo "$unformatted" >&2
    exit 1
fi
go mod tidy -diff
go vet ./...
go test ./...
