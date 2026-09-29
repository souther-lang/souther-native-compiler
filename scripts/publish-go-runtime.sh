#!/usr/bin/env bash
# Publishes the Go runtime module at a release, and asks whether it can be fetched.
#
# A Go module that lives in a directory of a repository is versioned by a tag that begins with that
# directory: the runtime, in bindings/go/runtime, at release 1.2.3 is the tag
# bindings/go/runtime/v1.2.3, and the repository's own v1.2.3 is the version of no module in it. The
# packages a release of the Go generator writes require this module at that release, so the tag is
# what makes them buildable, and a release is not complete without it.
#
# A tag is published once. The Go module proxy keeps what it has fetched for good, so a tag that
# already stands at another commit is refused and nothing is moved. Whether the version can be fetched
# is then asked of the module the way a host would fetch it, by its path and its version and from
# nothing local; a tag that does not name a module is found here and not by a user.
#
# usage: scripts/publish-go-runtime.sh <version> [<remote> [<commit>]]
#   version  the release, without the v: 1.2.3
#   remote   where the tag is pushed (default origin)
#   commit   what is tagged (default HEAD)
set -euo pipefail

if [ "$#" -lt 1 ] || [ "$#" -gt 3 ]; then
    echo "usage: $0 <version> [<remote> [<commit>]]" >&2
    exit 2
fi
version="$1"
remote="${2:-origin}"
commit="${3:-HEAD}"

module="github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
tag="bindings/go/runtime/v$version"
sha="$(git rev-parse "$commit^{commit}")"

standing="$(git ls-remote "$remote" "refs/tags/$tag" | cut -f1)"
if [ -n "$standing" ]; then
    if [ "$standing" != "$sha" ]; then
        echo "the tag $tag stands at $standing, not at $sha, and a published tag is not moved" >&2
        exit 1
    fi
    echo "the tag $tag stands at $sha already"
else
    # Pushed by what it names, so that nothing is left behind in the repository this is run in.
    git push "$remote" "$sha:refs/tags/$tag"
fi

# What a host asks: this module, at this version, by path, with no module of its own beside it and
# nothing of this clone in the way.
asked="$(mktemp -d)"
trap 'rm -rf "$asked"' EXIT
printf 'module asked\n\ngo 1.27\n' > "$asked/go.mod"
fetched="$(cd "$asked" && GOMODCACHE="$asked/mod" GOFLAGS=-modcacherw GOPROXY=direct GOSUMDB=off \
    go mod download -json "$module@v$version")"
if printf '%s' "$fetched" | grep -q '"Error"'; then
    echo "$module@v$version cannot be fetched:" >&2
    printf '%s\n' "$fetched" >&2
    exit 1
fi
echo "$module@v$version can be fetched"
