#!/usr/bin/env bash
# Holds what a release does for the Go runtime to what a host needs of it, without a release.
#
# The tests that build a Go host put the runtime in place with a `replace`, which is what a clone
# has to do and is not what anybody else does: nobody who is not in this clone has a `replace`. So
# whether the module can be resolved by its path and its version, with the tag the release process
# makes, is asked here. The release is made to a repository that is only a directory: the commit
# is tagged by scripts/publish-go-runtime.sh, as a release tags it, into a bare repository that
# stands where https://github.com/souther-lang/souther-native-compiler does, and a host that has no
# `replace` requires the module at that tag, is built and run.
#
# Needs Go and a C compiler, and reaches Raoh, which the runtime requires, where it is: at its
# repository. The commit that is checked is HEAD: what is not committed is not in it.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

module="github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
version="0.0.1-check"

# The repository the release is made to, and Git's own word that it stands where GitHub does.
git init --quiet --bare "$work/remote.git"
# A clone made shallow by CI has no history to send, and sends what it has.
git -C "$work/remote.git" config receive.shallowUpdate true
export GIT_CONFIG_COUNT=1
export GIT_CONFIG_KEY_0="url.file://$work/remote.git.insteadOf"
export GIT_CONFIG_VALUE_0="https://github.com/souther-lang/souther-native-compiler"
export GIT_TERMINAL_PROMPT=0

scripts/publish-go-runtime.sh "$version" "$work/remote.git" HEAD

# What a host has: the module by its path and its version, and Raoh as the runtime requires it, from
# nothing local.
raoh="$(sed -nE 's#^require (github.com/raoh-project/raoh-go) (.+)$#\1 \2#p' bindings/go/runtime/go.mod)"
protocol="$(sed -nE 's#^const Protocol = ([0-9]+)$#\1#p' bindings/go/runtime/protocol.go)"
mkdir -p "$work/host"
cat > "$work/host/go.mod" <<GOMOD
module host

go 1.27

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
if [ "$said" != "$protocol" ]; then
    echo "the host printed '$said', not the protocol '$protocol' of the runtime it required" >&2
    exit 1
fi
echo "a host that requires $module v$version by its path is built, with no replace"
