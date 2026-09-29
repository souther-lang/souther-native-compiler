#!/usr/bin/env bash
# Writes the SHA-256 of everything a released compiler fetches into the file the compiler is released
# with: `native.<platform>=<sha256>` for each bundle, and `generator.<id>=<sha256>` for each
# generator's jar. What the compiler fetches from a GitHub release or a Maven repository is held to
# these, since neither can be trusted to say what it is: an asset can be replaced and a repository can
# be a mirror. The compiler on Maven Central cannot be changed, so what it carries is what was built.
#
# Which platforms and which generators there are is the compiler's own, and the build that packs this
# file refuses a release that lacks one or holds another (ReleaseChecksums); this writes what it is
# given.
#
# usage: scripts/record-release-checksums.sh <properties file> <directory of bundles> <id>=<jar>...
set -euo pipefail

if [ "$#" -lt 3 ]; then
    echo "usage: $0 <properties file> <directory of bundles> <id>=<jar>..." >&2
    exit 2
fi
into="$1"
bundles="$2"
shift 2

mkdir -p "$(dirname "$into")"
: > "$into"

for sum in "$bundles"/souther-native-*.zip.sha256; do
    [ -e "$sum" ] || continue
    name="$(basename "$sum" .zip.sha256)"
    platform="$(printf '%s' "$name" | sed -nE 's/^souther-native-.+-((linux|macos)-(x86_64|aarch64))$/\1/p')"
    if [ -z "$platform" ]; then
        echo "not a bundle's name: $name" >&2
        exit 2
    fi
    echo "native.$platform=$(tr -d '[:space:]' < "$sum")" >> "$into"
done

for generator in "$@"; do
    id="${generator%%=*}"
    jar="${generator#*=}"
    if [ "$id" = "$generator" ] || [ ! -f "$jar" ]; then
        echo "not an <id>=<jar> that is there: $generator" >&2
        exit 2
    fi
    echo "generator.$id=$(shasum -a 256 "$jar" | cut -d ' ' -f 1)" >> "$into"
done
