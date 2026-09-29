#!/usr/bin/env bash
# What a release is about to publish is what it checksummed, and is what the modules say is published.
#
# The generators' jars are built before the compiler, and their checksums written into it; the
# publication is then built once more. So each generator's jar in the publication has to be the jar
# its own checksum is of, which is to say that building it twice writes the same bytes, and the
# compiler in it has to carry the file that was written.
#
# And the publication holds the artifacts the modules publish and no others: the reactor is deployed
# whole, and what is left out is left out by the module (`maven.deploy.skip`), so the set is read from
# the poms, where a module that is not published says so, and compared with what was deployed.
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
root="$(cd "$(dirname "$0")/.." && pwd)"

# Each generator's jar is the one whose checksum is recorded under its id: swapped, two jars would be
# held to each other's, and the compiler would refuse both when it fetched them.
generators=0
while IFS='=' read -r key sum; do
    id="${key#generator.}"
    artifact="souther-binding-$id"
    jar="$repository/org/souther-lang/$artifact/$version/$artifact-$version.jar"
    if [ ! -f "$jar" ]; then
        echo "no $jar, and $key is checksummed" >&2
        exit 1
    fi
    actual="$(shasum -a 256 "$jar" | cut -d ' ' -f 1)"
    if [ "$actual" != "$sum" ]; then
        echo "$jar is $actual, and $key says $sum: the build is not reproducible, or the jars are swapped" >&2
        exit 1
    fi
    generators=$((generators + 1))
done < <(grep '^generator\.' "$checksums")
if [ "$generators" -eq 0 ]; then
    echo "no generator is checksummed in $checksums" >&2
    exit 1
fi

compiler="$repository/org/souther-lang/souther-native-compiler/$version/souther-native-compiler-$version.jar"
if ! unzip -p "$compiler" souther/nativecode/release-checksums.properties | diff - "$checksums" > /dev/null; then
    echo "$compiler does not carry the checksums that were written" >&2
    exit 1
fi

artifact_of() {
    sed -n '/<\/parent>/,$ s#^ *<artifactId>\(.*\)</artifactId> *$#\1#p' "$1" | head -1
}
published="$(sed -n 's#^    <artifactId>\(.*\)</artifactId>$#\1#p' "$root/pom.xml" | head -1)"
for module in $(sed -n 's#^ *<module>\(.*\)</module> *$#\1#p' "$root/pom.xml"); do
    if ! grep -q '<maven.deploy.skip>true</maven.deploy.skip>' "$root/$module/pom.xml"; then
        published="$published"$'\n'"$(artifact_of "$root/$module/pom.xml")"
    fi
done
expected="$(printf '%s\n' "$published" | sort)"
deployed="$(cd "$repository/org/souther-lang" && find . -mindepth 1 -maxdepth 1 -type d | sed 's#^\./##' | sort)"
if [ "$expected" != "$deployed" ]; then
    echo "the publication is not what the modules say is published" >&2
    echo "the modules publish:" >&2
    printf '  %s\n' $expected >&2
    echo "deployed:" >&2
    printf '  %s\n' $deployed >&2
    exit 1
fi
echo "the publication is what was checksummed, and holds what the modules publish"
