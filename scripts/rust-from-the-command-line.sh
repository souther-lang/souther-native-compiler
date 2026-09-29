#!/usr/bin/env bash
# What an application outside the tests does to run a model from Rust, and nothing it does not: a
# two-file model built into a library and its Rust binding by the command line, the runtime crate
# patched in from bindings/rust/runtime, and a Rust program calling the library through the binding.
# No Java here calls the compiler's API, so a command that stopped writing what a host needs fails
# this where the tests, which call the API, would not.
#
# Needs Maven and Cargo: what `mvn test` needs.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

app="$(mktemp -d)"
trap 'rm -rf "$app"' EXIT
mkdir -p "$app/model" "$app/host/src"

cat > "$app/model/money.sou" <<'EOF'
module cart.money exposing ( Money )

data Money = Int
    invariant notNegative = value >= 0
EOF

cat > "$app/model/lines.sou" <<'EOF'
module cart.lines exposing ( Line, total )
import cart.money ( Money )

data Line = { price: Money, quantity: Int }
    invariant some = quantity > 0

behavior total : (line: Line) -> Int
let total (line) = line.price.value * line.quantity
EOF

# The line the README gives.
scripts/souther-native --library "$app/native" --rust "$app/rust" --crate shop "$app/model"

cat > "$app/host/Cargo.toml" <<EOF
[package]
name = "host"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
shop = { path = "../rust" }

[patch.crates-io]
souther-binding-runtime = { path = "$root/bindings/rust/runtime" }
EOF

cat > "$app/host/src/main.rs" <<'EOF'
use shop::cart::lines::{Line, total};
use shop::cart::money::Money;
use shop::{Construction, Library};

fn main() {
    let path = std::env::args().nth(1).expect("the library's path");
    // SAFETY: the library the binding was generated from.
    let library = unsafe { Library::load(&path) }.expect("the library loads");
    let said = library
        .run(|run| {
            let price = Money::new(run, 3).unwrap().into_result().unwrap();
            let line = Line::new(run, price, 4).unwrap().into_result().unwrap();
            let none = match Line::new(run, price, 0).unwrap() {
                Construction::Value(_) => "built".to_owned(),
                Construction::Rejected(issue) => issue.code().to_owned(),
            };
            format!("total {}, none {none}", total(run, line).unwrap())
        })
        .unwrap();
    println!("{said}");
}
EOF

library="$(ls "$app"/native/libsouther.*)"
said="$(CARGO_TARGET_DIR="$app/target" cargo run --quiet --manifest-path "$app/host/Cargo.toml" -- "$library")"
expected="total 12, none invariant_violation"
if [ "$said" != "$expected" ]; then
    echo "the host printed '$said', not '$expected'" >&2
    exit 1
fi
echo "$said"
