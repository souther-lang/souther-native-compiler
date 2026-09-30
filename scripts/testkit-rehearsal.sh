#!/usr/bin/env bash
# A release rehearsed on this machine, far enough to hold the testkit to its promise before a PR is
# merged rather than on the day of a release: the commit checked out apart, at a version that is not
# a snapshot; the generators' jars built and this platform's bundle packed; their checksums written
# into the compiler; the reactor deployed into a directory; and scripts/testkit-acceptance.sh run
# against that.
#
# A release carries the checksum of every platform's bundle, and one machine builds its own, so the
# build's check that a release carries them all is skipped here, as it is in the release's first pass
# (souther.skipReleaseCheck). It is not weakened, and no checksum is made up for a platform not built:
# the compiler carries the ones built, and what the testkit fetches for this platform is held to them.
# release.yml runs the acceptance on the whole publication.
#
# usage: scripts/testkit-rehearsal.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="0.0.0-rehearsal"
scratch="$(mktemp -d "${TMPDIR:-/tmp}/souther-testkit-rehearsal.XXXXXX")"
checkout="$scratch/checkout"

git -C "$root" worktree add --quiet --detach "$checkout" HEAD
trap 'git -C "$root" worktree remove --force "$checkout"' EXIT
# Cargo's own output is shared with the clone's, so the driver's dependencies are not built again.
mkdir -p "$root/native/target"
ln -s "$root/native/target" "$checkout/native/target"

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) platform=linux-x86_64 ;;
    Linux-aarch64 | Linux-arm64) platform=linux-aarch64 ;;
    Darwin-x86_64) platform=macos-x86_64 ;;
    Darwin-arm64 | Darwin-aarch64) platform=macos-aarch64 ;;
    *)
        echo "no bundle is built for $(uname -s) on $(uname -m)" >&2
        exit 2
        ;;
esac

cd "$checkout"
mvn --batch-mode --quiet versions:set -DnewVersion="$version" -DprocessAllModules \
    -DgenerateBackupPoms=false
mvn --batch-mode --quiet -DskipTests -Dexec.skip=true -Dsouther.skipReleaseCheck=true \
    -pl bindings/php/generator,bindings/rust/generator,bindings/go/generator -am package
scripts/package-native-bundle.sh "$version" "$platform" "$scratch/dist"
scripts/record-release-checksums.sh \
    compiler/src/main/resources/souther/nativecode/release-checksums.properties "$scratch/dist" \
    "php=bindings/php/generator/target/souther-binding-php-$version.jar" \
    "rust=bindings/rust/generator/target/souther-binding-rust-$version.jar" \
    "go=bindings/go/generator/target/souther-binding-go-$version.jar"
mvn --batch-mode --quiet -DskipTests -Dexec.skip=true -Dsouther.skipReleaseCheck=true deploy \
    -DaltDeploymentRepository="rehearsal::file:$scratch/maven-repo"

"$root/scripts/testkit-acceptance.sh" "$scratch/maven-repo" "$version" "$scratch/dist"
