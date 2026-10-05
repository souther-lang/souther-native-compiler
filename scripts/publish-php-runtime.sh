#!/usr/bin/env bash
# Publishes the PHP runtime to its mirror at the version it says it is, if that is not published, and
# asks before it does whether it could be. Run by hand, from a clean clone; nothing in CI publishes.
#
# Packagist reads a package from the root of a repository, and the runtime is a directory of this
# one, so what Packagist reads is a mirror (souther-lang/php-runtime) that holds the runtime and
# nothing else. Each version published there is one commit whose tree is bindings/php/runtime as it
# was at the commit published, on top of the version before it, tagged v<version>. Its version is the
# file bindings/php/runtime/VERSION, the runtime's own and not the compiler's, as the Go runtime's is.
#
# A tag Packagist has read is a version a host may have locked, so it is not moved, and each of
# these is asked first, and refuses:
#
#   - the version is a semantic version;
#   - where the tag stands already, it stands at the runtime as it is now: a runtime that has changed
#     since it was published at this version needs another version;
#   - Composer takes the package as it would be published;
#   - when it is published, the runtime requires only releases, since a development version is one a
#     host can install only by lowering its own minimum stability.
#
# What it is run for is said, as scripts/publish-go-runtime.sh has it said: a check pushes nothing, a
# rehearsal pushes to a mirror that is a directory here and asks nothing of a release, and a
# publication pushes to where Packagist reads and asks all of it. scripts/verify-php-runtime-release.sh
# rehearses it in every build.
#
# usage: scripts/publish-php-runtime.sh [--check | --rehearse] [<mirror> [<commit>]]
#   --check     asks all of it and pushes nothing
#   --rehearse  pushes, to a mirror that is a local directory, and asks nothing of a release
#   mirror      where the mirror is (default https://github.com/souther-lang/php-runtime.git)
#   commit      the runtime as of which commit (default HEAD)
set -euo pipefail

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

mode=publish
case "${1:-}" in
    --check) mode=check; shift ;;
    --rehearse) mode=rehearse; shift ;;
esac
if [ "$#" -gt 2 ]; then
    echo "usage: $0 [--check | --rehearse] [<mirror> [<commit>]]" >&2
    exit 2
fi
mirror="${1:-https://github.com/souther-lang/php-runtime.git}"
commit="${2:-HEAD}"
if [ "$mode" = rehearse ] && [ ! -d "$mirror" ]; then
    echo "a rehearsal pushes to a mirror that is a directory here, and $mirror is not one" >&2
    exit 2
fi

directory="bindings/php/runtime"
sha="$(git rev-parse "$commit^{commit}")"
version="$(git show "$sha:$directory/VERSION" | tr -d '[:space:]')"
tag="v$version"
tree="$(git rev-parse "$sha:$directory")"

refuse() {
    echo "$*" >&2
    exit 1
}

if ! printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'; then
    refuse "the version '$version' of the runtime is not a semantic version"
fi

# The package as it would be published, which Composer takes as a library it would read from Packagist.
package="$scratch/package"
mkdir "$package"
git archive "$tree" | tar -x -C "$package"
(cd "$package" && composer validate --no-check-lock --strict --quiet) \
    || refuse "Composer does not take $directory as a package: composer validate --strict"

# Where the tag stands already, it stands at this runtime.
looked="$scratch/looked"
git init --quiet --bare "$looked"
published=false
if git -C "$looked" fetch --quiet --depth=1 "$mirror" "refs/tags/$tag" 2> /dev/null; then
    if [ "$(git -C "$looked" rev-parse "FETCH_HEAD^{tree}")" != "$tree" ]; then
        refuse "the runtime is not what it was when $tag was published, and a published tag is not moved: give it another version in $directory/VERSION"
    fi
    published=true
fi

if $published; then
    echo "$tag is published on $mirror, and is this runtime"
    exit 0
fi

# What a publication is held to: the runtime requires releases only.
if [ "$mode" = publish ]; then
    unreleased="$(cd "$package" && php -r '
        $require = json_decode(file_get_contents("composer.json"), true)["require"] ?? [];
        foreach ($require as $name => $constraint) {
            if (preg_match("/(^dev-|-dev$|@dev|@alpha|@beta|@RC)/i", $constraint)) {
                echo "$name $constraint\n";
            }
        }')"
    if [ -n "$unreleased" ]; then
        refuse "the runtime requires what is not released, and is published requiring releases only:
$unreleased"
    fi
fi

if [ "$mode" = check ]; then
    echo "$tag is not published on $mirror"
    exit 0
fi

# On top of what the mirror holds, where it holds anything yet.
parents=()
if git -C "$looked" fetch --quiet "$mirror" refs/heads/main 2> /dev/null; then
    parents=(-p "$(git -C "$looked" rev-parse FETCH_HEAD)")
    git fetch --quiet "$mirror" refs/heads/main
fi
published_commit="$(git commit-tree "$tree" ${parents[@]+"${parents[@]}"} \
    -m "souther-lang/php-runtime $version" \
    -m "bindings/php/runtime of souther-lang/souther-native-compiler at $sha")"
git push "$mirror" "$published_commit:refs/heads/main" "$published_commit:refs/tags/$tag"
if [ "$mode" = publish ]; then
    echo "$tag is published on $mirror: Packagist reads it from there"
else
    echo "$tag is published on $mirror, rehearsed"
fi
