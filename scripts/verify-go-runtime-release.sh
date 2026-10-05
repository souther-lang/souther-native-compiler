#!/usr/bin/env bash
# Holds what a release does for the Go runtime to what a host needs of it, without a release.
#
# The tests that build a Go host put the runtime in place with a `replace`, which is what a clone
# has to do and is not what anybody else does: nobody who is not in this clone has a `replace`. So
# whether the module can be resolved by its path and its version, with the tag the release process
# makes, is asked here, of a repository that is only a directory standing where
# https://github.com/souther-lang/souther-native-compiler does:
#
#   - HEAD is published as a release publishes it, rehearsed (--rehearse), and a host that has no
#     `replace` requires the module at its version, by its path, and is built and run;
#   - publishing it again is publishing nothing, and the same commit is not a reason to refuse;
#   - a runtime that changed and kept its version is refused, and so is a version that is not one, a
#     module at version 2 whose path does not say so, and one before 2 whose path does: each
#     before anything is pushed, since a tag that is pushed is not taken back;
#   - a publication, which a rehearsal is not, refuses a runtime requiring a pseudo-version, before
#     anything is pushed, and publishes one requiring releases only; and a rehearsal pushes to
#     nothing but a directory.
#
# HEAD is rehearsed and not published, so this holds while the runtime still requires a commit of
# Raoh, which a publication would refuse; what a publication refuses is held with a runtime of its
# own below, and does not wait on that.
#
# Needs Go and a C compiler, and reaches Raoh, which the runtime requires, where it is: at its
# repository. What is published is HEAD, so what is not committed is not in it.
set -euo pipefail

. "$(dirname "$0")/require-go.sh"

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

# What this clone is, which publishing reads and never changes: what it fetches goes to a
# repository of its own. A shallow fetch into this one once marked the tag's commit shallow here and
# cut the branch's history at it, so that a push of the next commit was refused.
clone() {
    git rev-parse --is-shallow-repository
    cat "$(git rev-parse --git-common-dir)/shallow" 2>/dev/null || true
    git for-each-ref --format='%(refname) %(objectname)' refs/heads refs/tags
}
before="$(clone)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

directory="bindings/go/runtime"
repository="https://github.com/souther-lang/souther-native-compiler"
publish="$root/scripts/publish-go-runtime.sh"
module="$(sed -nE 's#^module[[:space:]]+([^[:space:]]+)[[:space:]]*$#\1#p' "$directory/go.mod")"
version="$(tr -d '[:space:]' < "$directory/VERSION")"

fail() {
    echo "$*" >&2
    exit 1
}

# Git is told that the repository stands where GitHub does, for everything below.
git init --quiet --bare "$work/remote.git"
git -C "$work/remote.git" config receive.shallowUpdate true
export GIT_CONFIG_COUNT=1
export GIT_CONFIG_KEY_0="url.file://$work/remote.git.insteadOf"
export GIT_CONFIG_VALUE_0="$repository"
export GIT_TERMINAL_PROMPT=0

# Published as a release publishes it, rehearsed.
"$publish" --rehearse "$work/remote.git" HEAD
[ -n "$(git ls-remote "$work/remote.git" "refs/tags/$directory/v$version")" ] \
    || fail "the tag $directory/v$version was not published"

# Publishing what is published is publishing nothing.
said="$("$publish" --check "$work/remote.git" HEAD)"
case "$said" in
    *"is published, and is this runtime"*) ;;
    *) fail "asked again, it said: $said" ;;
esac

# What a host has: the module by its path and its version, and Raoh as the runtime requires it, from
# nothing local.
raoh="$(sed -nE 's#^require (github.com/raoh-project/raoh-go) (.+)$#\1 \2#p' "$directory/go.mod")"
protocol="$(sed -nE 's#^const Protocol = ([0-9]+)$#\1#p' "$directory/protocol.go")"
mkdir -p "$work/host"
cat > "$work/host/go.mod" <<GOMOD
module host

go $go_version

require $module v$version
require $raoh
GOMOD
cat > "$work/host/main.go" <<'MAIN'
package main

import (
	"fmt"

	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
)

func main() { fmt.Println(souther.Protocol) }
MAIN
said="$(cd "$work/host" && GOMODCACHE="$work/mod" GOFLAGS="-modcacherw -mod=mod" GOPROXY=direct GOSUMDB=off \
    go run .)"
[ "$said" = "$protocol" ] || fail "the host printed '$said', not the protocol '$protocol' of the runtime it required"
echo "a host that requires $module v$version by its path is built, with no replace"

# What is refused, of a repository that has only the runtime in it: changed in a commit each.
git_in() {
    git -C "$work/synthetic" -c user.name=check -c user.email=check@example.com "$@"
}
mkdir -p "$work/synthetic/$(dirname "$directory")"
cp -R "$directory" "$work/synthetic/$directory"
git init --quiet "$work/synthetic"
git_in add -A
git_in commit --quiet -m "the runtime"

refused() {
    local why="$1"
    local wanted="$2"
    local said
    if said="$(cd "$work/synthetic" && "$publish" --check "$work/none.git" HEAD 2>&1)"; then
        fail "$why was not refused: $said"
    fi
    case "$said" in
        *"$wanted"*) ;;
        *) fail "$why was refused for another reason than '$wanted': $said" ;;
    esac
    [ -z "$(git ls-remote "$work/none.git")" ] || fail "$why was refused after something was pushed"
    echo "refused: $why"
}
# Where nothing is pushed: a repository that stays empty.
git init --quiet --bare "$work/none.git"

printf '1.0\n' > "$work/synthetic/$directory/VERSION"
git_in commit --quiet -am "no version"
refused "a version that is not one" "is not a semantic version"

printf '2.0.0\n' > "$work/synthetic/$directory/VERSION"
git_in commit --quiet -am "version 2 without /v2"
refused "version 2 with no /v2 in the path" "has to end in /v2"

printf '1.0.0\n' > "$work/synthetic/$directory/VERSION"
sed -i.bak -E 's#^module[[:space:]]+(.*)$#module \1/v2#' "$work/synthetic/$directory/go.mod"
rm -f "$work/synthetic/$directory/go.mod.bak"
git_in commit --quiet -am "version 1 with /v2"
refused "version 1 with /v2 in the path" "has no major version"

# A version 2 that says so is one Go takes, and is only rehearsed here.
printf '2.0.0\n' > "$work/synthetic/$directory/VERSION"
git_in commit --quiet -am "version 2 with /v2"
said="$(cd "$work/synthetic" && "$publish" --check "$work/none.git" HEAD 2>&1)" \
    || fail "version 2 with /v2 in the path was refused: $said"
echo "a version 2 whose path says so is one Go fetches"

# A published runtime that changed and kept its version.
git init --quiet --bare "$work/second.git"
git -C "$work/second.git" config receive.shallowUpdate true
(cd "$work/synthetic" && GIT_CONFIG_KEY_0="url.file://$work/second.git.insteadOf" \
    "$publish" --rehearse "$work/second.git" HEAD > /dev/null)
printf '\n// changed\n' >> "$work/synthetic/$directory/run.go"
git_in commit --quiet -am "changed, and not given another version"
if said="$(cd "$work/synthetic" && "$publish" --check "$work/second.git" HEAD 2>&1)"; then
    fail "a runtime that changed and kept its version was not refused: $said"
fi
case "$said" in
    *"give it another version"*) echo "refused: a runtime that changed and kept its version" ;;
    *) fail "refused for another reason: $said" ;;
esac

# A publication requires releases only: a runtime requiring a pseudo-version is refused before
# anything is pushed, whatever Raoh the runtime requires today.
git_in reset --quiet --hard "$(git_in rev-list --max-parents=0 HEAD)"
cp "$work/synthetic/$directory/go.mod" "$work/released.go.mod"
printf '\nrequire example.com/unreleased v0.0.0-20260101000000-0123456789ab\n' \
    >> "$work/synthetic/$directory/go.mod"
git_in commit --quiet -am "requires a pseudo-version"
if said="$(cd "$work/synthetic" && "$publish" "$work/none.git" HEAD 2>&1)"; then
    fail "a publication of a runtime requiring a pseudo-version was not refused: $said"
fi
case "$said" in
    *"requires what is not released"*) echo "refused: a publication of a runtime requiring a pseudo-version" ;;
    *) fail "refused for another reason: $said" ;;
esac
[ -z "$(git ls-remote "$work/none.git")" ] || fail "a pseudo-version was refused after something was pushed"

# And one requiring releases only is published. Its requirements are dropped, which fetching the
# module by its path and version does not read.
sed -E '/^require|^\)|^[[:space:]]/d' "$work/released.go.mod" > "$work/synthetic/$directory/go.mod"
: > "$work/synthetic/$directory/go.sum"
git_in commit --quiet -am "requires releases only"
git init --quiet --bare "$work/third.git"
git -C "$work/third.git" config receive.shallowUpdate true
said="$(cd "$work/synthetic" && GIT_CONFIG_KEY_0="url.file://$work/third.git.insteadOf" \
    "$publish" "$work/third.git" HEAD 2>&1)" || fail "a publication requiring releases only was refused: $said"
[ -n "$(git ls-remote "$work/third.git" "refs/tags/$directory/v$version")" ] \
    || fail "a publication requiring releases only pushed no tag: $said"
echo "a publication requiring releases only is published"

# A rehearsal reaches nothing but a directory.
if said="$("$publish" --rehearse "$repository" HEAD 2>&1)"; then
    fail "a rehearsal to $repository was not refused: $said"
fi
echo "refused: a rehearsal to a remote that is not a directory"

[ "$(clone)" = "$before" ] || fail "publishing changed this clone: its shallow commits or its refs are not what they were"
