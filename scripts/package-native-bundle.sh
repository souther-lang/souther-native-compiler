#!/usr/bin/env bash
# The driver, with the runtime archive and the file of what linking it needs, packed as a release
# publishes them for one platform: souther-native-<version>-<platform>.zip, and beside it the
# SHA-256 of that file in souther-native-<version>-<platform>.zip.sha256.
#
# The driver reads the other two from beside itself, so they are one bundle and hold nothing else;
# the compiler that fetches it refuses a bundle that holds anything but these three files.
#
# usage: scripts/package-native-bundle.sh <version> <platform> <directory>
#   platform: linux-x86_64, linux-aarch64, macos-x86_64 or macos-aarch64, as NativeBundle names it
set -euo pipefail

if [ "$#" -ne 3 ]; then
    echo "usage: $0 <version> <platform> <directory>" >&2
    exit 2
fi
version="$1"
platform="$2"
into="$3"

case "$platform" in
    linux-x86_64 | linux-aarch64 | macos-x86_64 | macos-aarch64) ;;
    *)
        echo "no such platform: $platform" >&2
        exit 2
        ;;
esac

root="$(cd "$(dirname "$0")/.." && pwd)"
(cd "$root/native" && cargo build --release --locked)

built="$root/native/target/release"
mkdir -p "$into"
bundle="$(cd "$into" && pwd)/souther-native-$version-$platform.zip"
# A bundle written earlier is replaced, and not added to.
rm -f "$bundle"
# -q quiet, -j no directories, -X no platform attributes: the flags macOS's zip and Linux's share.
zip -q -j -X "$bundle" \
    "$built/souther-native-driver" \
    "$built/libsouther_native_runtime.a" \
    "$built/libsouther_native_runtime.link"

shasum -a 256 "$bundle" | cut -d ' ' -f 1 > "$bundle.sha256"
echo "$bundle $(cat "$bundle.sha256")"
