#!/usr/bin/env bash
# What a release is about to publish is what it checksummed. The generators' jars are built before the
# compiler, and their checksums written into it; the publication is then built once more. So the jars
# in the publication have to be the ones that were checksummed, which is to say that building them
# twice writes the same bytes, and the compiler in it has to carry the file that was written.
#
# usage: scripts/verify-release-build.sh <maven repository> <version> <checksums file>
set -euo pipefail

if [ "$#" -ne 3 ]; then
    echo "usage: $0 <maven repository> <version> <checksums file>" >&2
    exit 2
fi
repository="$1"
version="$2"
checksums="$3"

found=0
for jar in "$repository"/org/souther-lang/souther-binding-*/"$version"/souther-binding-*-"$version".jar; do
    [ -e "$jar" ] || continue
    sum="$(shasum -a 256 "$jar" | cut -d ' ' -f 1)"
    if ! grep -q "^generator\..*=$sum\$" "$checksums"; then
        echo "$jar is not the jar that was checksummed ($sum): the build is not reproducible" >&2
        exit 1
    fi
    found=$((found + 1))
done
recorded="$(grep -c '^generator\.' "$checksums")"
if [ "$found" -ne "$recorded" ]; then
    echo "the publication holds $found generators, and $recorded were checksummed" >&2
    exit 1
fi

compiler="$repository/org/souther-lang/souther-native-compiler/$version/souther-native-compiler-$version.jar"
if ! unzip -p "$compiler" souther/nativecode/release-checksums.properties | diff - "$checksums" > /dev/null; then
    echo "$compiler does not carry the checksums that were written" >&2
    exit 1
fi
echo "the publication carries the checksums of $found generators and is what was checksummed"
