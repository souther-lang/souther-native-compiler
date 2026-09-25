#!/usr/bin/env bash
# examples/php-cart as its README says to run it: the model built by the command line, the
# runtime and raoh-php installed by Composer, and the HTTP layer driven by PHPUnit over SQLite.
# The rows of cart.sou run in the build, so a model that stopped holding them fails here too.
#
# Needs what php-from-the-command-line.sh needs, and PHP's pdo_sqlite.
set -euo pipefail

example="$(cd "$(dirname "$0")/../examples/php-cart" && pwd)"

"$example/bin/build"
composer install --working-dir="$example" --no-interaction --no-progress --quiet
cd "$example"
vendor/bin/phpunit
