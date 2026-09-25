#!/usr/bin/env bash
# Every Composer project in the repository installs what its composer.lock fixes, and nothing newer:
# a lock beside each composer.json, and one that still matches it. Without the lock, a clean checkout
# resolves whatever versions are newest that day, and CI can go red over a release that has nothing
# to do with this repository. `composer validate --strict` is what refuses a lock that has fallen
# behind its composer.json.
#
# Needs Composer.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

status=0
while IFS= read -r manifest; do
    dir="$(dirname "$manifest")"
    if [ ! -f "$dir/composer.lock" ]; then
        echo "$manifest has no composer.lock beside it" >&2
        status=1
        continue
    fi
    if ! composer validate --strict --no-check-publish --no-interaction --quiet --working-dir="$dir"; then
        echo "$manifest does not validate, or its composer.lock does not match it" >&2
        status=1
    fi
done < <(git ls-files '*composer.json')
exit "$status"
