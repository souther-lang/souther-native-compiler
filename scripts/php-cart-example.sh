#!/usr/bin/env bash
# examples/php-cart as its README says to run it: the model built by the command line, the
# runtime and raoh-php installed by Composer, and the HTTP layer driven by PHPUnit over SQLite.
# The rows of cart.sou run in the build, so a model that stopped holding them fails here too.
#
# The example requires the runtime from Packagist, as an application does. Where this clone's
# runtime is at a version Packagist does not have, which is the case from a change to the runtime
# until it is published, the binding this clone writes calls that runtime, and the example is built
# as a copy of itself with the runtime taken from the clone as a path repository at that version:
# the lock, which names the release, is not installed from then.
#
# Needs what php-from-the-command-line.sh needs, and PHP's pdo_sqlite.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
example="$root/examples/php-cart"
runtime="$root/bindings/php/runtime"
export SOUTHER="${SOUTHER:-$root/scripts/souther}"

version="$(tr -d '[:space:]' < "$runtime/VERSION")"

# Packagist answers 404 for a package it does not have, and lists each version of one it has.
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
status="$(curl -sS -o "$work/packagist.json" -w '%{http_code}' \
    https://repo.packagist.org/p2/souther-lang/php-runtime.json)"
case "$status" in
    200) published="$(grep -c "\"version\":\"v$version\"" "$work/packagist.json" || true)" ;;
    404) published=0 ;;
    *)
        echo "Packagist answered $status for souther-lang/php-runtime" >&2
        exit 1
        ;;
esac

if [ "$published" -gt 0 ]; then
    "$example/bin/build"
    composer install --working-dir="$example" --no-interaction --no-progress --quiet
    cd "$example"
    vendor/bin/phpunit
    exit 0
fi

echo "souther-lang/php-runtime $version is not on Packagist: building examples/php-cart over $runtime" >&2
mkdir "$work/examples"
cp -R "$root/examples/cart-model" "$work/examples/"
rsync -a --exclude /vendor --exclude /build "$example/" "$work/examples/php-cart/"
copy="$work/examples/php-cart"
"$copy/bin/build"
composer config --working-dir="$copy" repositories.runtime \
    "{\"type\": \"path\", \"url\": \"$runtime\", \"options\": {\"symlink\": false, \"versions\": {\"souther-lang/php-runtime\": \"$version\"}}}"
composer update --working-dir="$copy" --no-interaction --no-progress --quiet souther-lang/php-runtime
cd "$copy"
vendor/bin/phpunit
