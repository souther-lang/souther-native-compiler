#!/usr/bin/env bash
# Writes the SHA-256 of each bundle a release built into the file the compiler is released with, as
# `<platform>=<sha256>` a line, so that what the compiler fetches from a GitHub release is held to
# what the release built: an asset there can be replaced, and the compiler on Maven Central cannot.
#
# usage: scripts/record-bundle-checksums.sh <directory of bundles> <properties file>
set -euo pipefail

if [ "$#" -ne 2 ]; then
    echo "usage: $0 <directory of bundles> <properties file>" >&2
    exit 2
fi
bundles="$1"
into="$2"

found=0
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
    echo "$platform=$(tr -d '[:space:]' < "$sum")" >> "$into"
    found=$((found + 1))
done

if [ "$found" -ne 4 ]; then
    echo "expected the four platforms' bundles in $bundles, found $found" >&2
    exit 1
fi
