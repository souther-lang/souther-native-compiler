#!/usr/bin/env bash
# What an application outside the tests does to run a model from Go, and nothing it does not: a
# two-file model built into a library and its Go binding by the command line, the runtime module
# put in place from bindings/go/runtime, and a Go program calling the library through the binding.
# No Java here calls the compiler's API, so a command that stopped writing what a host needs fails
# this where the tests, which call the API, would not.
#
# Needs Maven, Go and a C compiler: what `mvn test` needs.
set -euo pipefail

. "$(dirname "$0")/require-go.sh"

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

app="$(mktemp -d)"
trap 'rm -rf "$app"' EXIT
mkdir -p "$app/model" "$app/host"

cat > "$app/model/money.sou" <<'MONEY'
module cart.money exposing ( Money )

data Money = Int
    invariant notNegative = value >= 0
MONEY

cat > "$app/model/lines.sou" <<'LINES'
module cart.lines exposing ( Line, total )
import cart.money ( Money )

data Line = { price: Money, quantity: Int }
    invariant some = quantity > 0

behavior total : (line: Line) -> Int
let total (line) = line.price.value * line.quantity
LINES

# The line the README gives.
# $SOUTHER is a CLI where scripts/with-the-souther-cli runs this, and scripts/souther otherwise.
"${SOUTHER:-scripts/souther}" compile --target native --library "$app/native" \
    --go "$app/go" --package example.com/shop "$app/model"

runtime="github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
raoh="$(sed -nE 's#^require (github.com/raoh-project/raoh-go) (.+)$#\1 \2#p' bindings/go/runtime/go.mod)"
# The version the binding requires of the runtime, which a build from a clone leaves for a replace.
required="$(sed -nE "s#^[[:space:]]*$runtime (v[^[:space:]]+)\$#\1#p" "$app/go/go.mod")"

cat > "$app/host/go.mod" <<GOMOD
module host

go $go_version

require example.com/shop v0.0.0
require $raoh

replace example.com/shop => ../go
replace $runtime $required => $root/bindings/go/runtime
GOMOD

cat > "$app/host/main.go" <<'MAIN'
package main

import (
	"errors"
	"fmt"
	"os"

	"example.com/shop"
	"example.com/shop/cart/lines"
	"example.com/shop/cart/money"

	"github.com/raoh-project/raoh-go"
)

func main() {
	library, err := shop.Load(os.Args[1])
	if err != nil {
		panic(err)
	}
	err = library.Run(func(r *shop.Run) error {
		price, err := money.NewMoney(r, 3)
		if err != nil {
			return err
		}
		line, err := lines.NewLine(r, price, 4)
		if err != nil {
			return err
		}
		none := "built"
		if _, err := lines.NewLine(r, price, 0); err != nil {
			issues, _ := errors.AsType[*raoh.Issues](err)
			none = issues.All()[0].Code()
		}
		total, err := lines.Total(r, line)
		if err != nil {
			return err
		}
		fmt.Printf("total %d, none %s\n", total, none)
		return nil
	})
	if err != nil {
		panic(err)
	}
}
MAIN

library="$(ls "$app"/native/libsouther.*)"
cd "$app/host"
go mod tidy
said="$(go run . "$library")"
expected="total 12, none invariant_violation"
if [ "$said" != "$expected" ]; then
    echo "the host printed '$said', not '$expected'" >&2
    exit 1
fi
echo "$said"
