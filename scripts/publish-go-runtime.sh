#!/usr/bin/env bash
# Publishes the Go runtime module at the version it says it is, if that is not published, and asks
# before it does whether it could be.
#
# The runtime is a module of its own in bindings/go/runtime, and its version is its own, in the file
# bindings/go/runtime/VERSION beside its go.mod: not the compiler's, which has no reason to move when
# the runtime does not, and cannot be a module's from version 2 on, where the path of the module says
# its major version. A module that lives in a directory of a repository is versioned by a tag that
# begins with that directory: version 1.2.3 of this one is the tag bindings/go/runtime/v1.2.3, and
# the repository's own v1.2.3 is the version of no module in it. The packages the Go generator writes
# require this module at the version the generator was built with, so a release without it published
# is a release whose packages do not build.
#
# A published tag is not taken back: the Go module proxy keeps what it has fetched. So nothing is
# pushed that has not been shown to work, and each of these is asked first, and refuses:
#
#   - the version is a semantic version, and the module's path says its major version from 2 on and
#     not before, which Go refuses after the fact and cannot be put right;
#   - where the tag stands already, it stands at the runtime as it is now: a runtime that has changed
#     since it was published at this version needs another version, which is what the file says;
#   - the module can be fetched by its path and its version, from nothing local, out of a repository
#     that has this commit tagged: a rehearsal, in a directory that stands where GitHub does.
#
# Only then is the tag pushed, if it is not standing, and the module fetched from where it went.
#
# usage: scripts/publish-go-runtime.sh [--check] [<remote> [<commit>]]
#   --check  asks all of it and pushes nothing
#   remote   where the tag is (default origin)
#   commit   the runtime as of which commit (default HEAD)
set -euo pipefail

. "$(dirname "$0")/require-go.sh"

check=false
if [ "${1:-}" = "--check" ]; then
    check=true
    shift
fi
if [ "$#" -gt 2 ]; then
    echo "usage: $0 [--check] [<remote> [<commit>]]" >&2
    exit 2
fi
remote="${1:-origin}"
commit="${2:-HEAD}"

directory="bindings/go/runtime"
repository="https://github.com/souther-lang/souther-native-compiler"
sha="$(git rev-parse "$commit^{commit}")"
version="$(git show "$sha:$directory/VERSION" | tr -d '[:space:]')"
module="$(git show "$sha:$directory/go.mod" | sed -nE 's#^module[[:space:]]+([^[:space:]]+)[[:space:]]*$#\1#p')"
tag="$directory/v$version"
now="$(git rev-parse "$sha:$directory")"

refuse() {
    echo "$*" >&2
    exit 1
}

# What the version and the path say of one another.
if ! printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'; then
    refuse "the version '$version' of the runtime is not a semantic version"
fi
major="${version%%.*}"
if [ "$major" -ge 2 ]; then
    case "$module" in
        */v"$major") ;;
        *) refuse "the runtime is at version $version, so its path has to end in /v$major, and is $module" ;;
    esac
else
    case "$module" in
        */v[0-9]*) refuse "the runtime is at version $version, so its path has no major version, and is $module" ;;
    esac
fi

# Where the tag stands already, it stands at this runtime.
standing="$(git ls-remote "$remote" "refs/tags/$tag" | cut -f1)"
published=false
if [ -n "$standing" ]; then
    # Read in a repository of its own and never in this one: a shallow fetch here would mark the
    # tag's commit shallow in this clone, and cut its history there for everything that runs after.
    looked="$(mktemp -d)"
    git init --quiet --bare "$looked"
    git -C "$looked" fetch --quiet --depth=1 "$(git remote get-url "$remote" 2>/dev/null || printf '%s' "$remote")" \
        "refs/tags/$tag"
    published_tree="$(git -C "$looked" rev-parse "FETCH_HEAD:$directory")"
    rm -rf "$looked"
    if [ "$published_tree" != "$now" ]; then
        refuse "the runtime is not what it was when $tag was published, and a published tag is not moved: give it another version in $directory/VERSION"
    fi
    published=true
fi

# Whether the module can be fetched by its path and version: from nothing local.
fetches() {
    local asked
    asked="$(mktemp -d)"
    printf 'module asked\n\ngo %s\n' "$go_version" > "$asked/go.mod"
    local fetched
    fetched="$(cd "$asked" && GOMODCACHE="$asked/mod" GOFLAGS=-modcacherw GOPROXY=direct GOSUMDB=off \
        go mod download -json "$module@v$version" 2>&1 || true)"
    rm -rf "$asked"
    if printf '%s' "$fetched" | grep -q '"Error"\|^go: '; then
        echo "$module@v$version cannot be fetched:" >&2
        printf '%s\n' "$fetched" >&2
        return 1
    fi
}

# A rehearsal, in a repository that is only a directory, which Git is told stands where GitHub does.
if ! $published; then
    rehearsal="$(mktemp -d)"
    trap 'rm -rf "$rehearsal"' EXIT
    git init --quiet --bare "$rehearsal/remote.git"
    # A clone that is shallow has no history to send, and sends what it has.
    git -C "$rehearsal/remote.git" config receive.shallowUpdate true
    git push --quiet "$rehearsal/remote.git" "$sha:refs/tags/$tag"
    if ! GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0="url.file://$rehearsal/remote.git.insteadOf" \
        GIT_CONFIG_VALUE_0="$repository" GIT_TERMINAL_PROMPT=0 fetches; then
        refuse "$tag would not be a version of $module: nothing was published"
    fi
    echo "$module@v$version can be fetched from a tag of this commit"
fi

if $check; then
    if $published; then
        echo "$tag is published, and is this runtime"
    else
        echo "$tag is not published: a release publishes it"
    fi
    exit 0
fi

if ! $published; then
    git push "$remote" "$sha:refs/tags/$tag"
fi
GIT_TERMINAL_PROMPT=0 fetches || refuse "$tag is published and $module@v$version cannot be fetched from it"
echo "$module@v$version can be fetched"
