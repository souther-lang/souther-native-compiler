#!/usr/bin/env bash
# What an application outside the tests does to run a model from PHP, and nothing it does not: a
# two-file model built into a library and its binding by the command line, the runtime installed by
# Composer from bindings/php/runtime as a path repository, and a PHP file calling the library
# through the binding. No Java here calls the compiler's API, so a command that stopped writing what
# a host needs fails this where the tests, which call the API, would not.
#
# Needs Maven, Cargo, Composer, and PHP with the ffi and intl extensions: what `mvn test` needs.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

app="$(mktemp -d)"
trap 'rm -rf "$app"' EXIT
mkdir -p "$app/model"

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

# The line the README gives, run from the root of the clone as it says. --no-snapshot-updates keeps
# the souther-compiler CI installed from its pinned commit, as the build's own step does.
mvn --batch-mode --quiet --no-snapshot-updates process-classes exec:java \
    -Dargs="--library $app/native --php $app/php --namespace Shop $app/model"

# The binding's namespace is mapped by the application, as it would map its own classes. raoh-php is
# the version the runtime's composer.lock fixes, so this run installs what every other run does and
# not whichever release is newest today.
raoh="$(php -r 'foreach (json_decode(file_get_contents($argv[1]), true)["packages"] as $p) {
    if ($p["name"] === "raoh/raoh") { echo $p["version"]; } }' "$root/bindings/php/runtime/composer.lock")"
cat > "$app/composer.json" <<EOF
{
    "repositories": [
        { "type": "path", "url": "$root/bindings/php/runtime" }
    ],
    "require": {
        "raoh/raoh": "$raoh",
        "souther-lang/php-runtime": "@dev"
    },
    "autoload": {
        "psr-4": { "Shop\\\\": "php/" }
    }
}
EOF
composer install --working-dir="$app" --no-interaction --no-progress --quiet

cat > "$app/host.php" <<'EOF'
<?php
declare(strict_types=1);

require __DIR__ . '/vendor/autoload.php';

use Raoh\Issue;
use Shop\Binding;
use Shop\Cart\Lines\Behaviors;
use Shop\Cart\Lines\Line;
use Shop\Cart\Money\Money;

$library = glob(__DIR__ . '/native/libsouther.*')[0];
echo Binding::load($library)->run(function (): string {
    $line = Line::of(Money::of(3)->getOrThrow(), 4)->getOrThrow();
    $none = Line::of(Money::of(3)->getOrThrow(), 0)->fold(
        fn ($line) => 'built',
        fn ($issues) => implode(' ', array_map(fn (Issue $it) => $it->code, $issues->toArray())));
    return 'total ' . Behaviors::total($line) . ', none ' . $none;
}), "\n";
EOF

said="$(php -d ffi.enable=1 -d display_errors=stderr -d error_reporting=-1 "$app/host.php")"
expected="total 12, none invariant_violation"
if [ "$said" != "$expected" ]; then
    echo "host.php printed '$said', not '$expected'" >&2
    exit 1
fi
echo "$said"
