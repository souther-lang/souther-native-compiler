#!/usr/bin/env bash
# The testkit held to what it promises an author outside this repository: a project that depends on
# the API and the testkit of a release, and on nothing else of this project, builds a library and runs
# its generator's test. Run against a publication before it is published, the way it would be used
# once it is: the project resolves the release from <maven repository>, and the testkit fetches its
# driver from <bundles>, laid out as the release's assets are and held to the checksum the compiler in
# that publication carries. Nothing of this clone's build is on its path: no reactor module, no
# compiler test-jar, no driver named.
#
# usage: scripts/testkit-acceptance.sh <maven repository> <version> <bundles>
#   bundles: a directory holding souther-native-<version>-<platform>.zip for this platform
set -euo pipefail

if [ "$#" -ne 3 ]; then
    echo "usage: $0 <maven repository> <version> <bundles>" >&2
    exit 2
fi
repository="$(cd "$1" && pwd)"
version="$2"
bundles="$(cd "$3" && pwd)"
root="$(cd "$(dirname "$0")/.." && pwd)"

scratch="$(mktemp -d "${TMPDIR:-/tmp}/souther-testkit-acceptance.XXXXXX")"

# The release's assets as NativeBundle reads them: <root>/v<version>/<asset>. A file: address reads
# this layout as an https: one reads the release's, so the same lookup and the same check run.
mkdir -p "$scratch/releases/v$version"
cp "$bundles"/souther-native-"$version"-*.zip "$scratch/releases/v$version/"

# A local repository of its own, so that nothing a build of this clone installed is resolved, and a
# cache of its own, so that the driver is fetched and checked here and not found kept.
SOUTHER_HOME="$scratch/home" mvn --batch-mode \
    -f "$root/bindings/testkit/acceptance/pom.xml" \
    -Dmaven.repo.local="$scratch/m2" \
    -Dsouther.version="$version" \
    -Dsouther.publication="file://$repository" \
    -Dsouther.releases="file://$scratch/releases" \
    test

# The driver it built with is the release's, fetched into the cache and not taken from anywhere else.
if ! compgen -G "$scratch/home/native/$version/*/souther-native-driver" > /dev/null; then
    echo "the testkit built without fetching the driver of $version" >&2
    exit 1
fi
echo "the testkit of $version builds a library outside this repository"
