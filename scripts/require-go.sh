# Sourced by a script that runs Go: what Go it needs, said once, and whether the Go it has is that.
#
# The version is the `go` line of the runtime module's go.mod, which is where it is stated: the
# workflows install the Go it names (setup-go reads the same file), the generator writes it into
# the modules it makes, and the scripts read it here. Nothing else in this repository says it.
#
# The toolchain Go runs is the one that is installed. Go can fetch another, newer one on its own
# when the module asks for it, which would make a job that never installed Go seem to have it, and
# make what is run there a different Go than the one the build is tested with; so it is told not to
# (GOTOOLCHAIN=local), and a Go that is too old, or not there, is a job that says so.
#
# Sets go_version for what the script writes into a go.mod of its own.

go_mod="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/bindings/go/runtime/go.mod"
go_version="$(sed -nE 's/^go[[:space:]]+([0-9][0-9.]*)[[:space:]]*$/\1/p' "$go_mod")"
if [ -z "$go_version" ]; then
    echo "$go_mod says no Go version" >&2
    exit 1
fi
export GOTOOLCHAIN=local

if ! command -v go > /dev/null 2>&1; then
    echo "Go $go_version is needed, and there is no go here: the job has to install it" >&2
    exit 1
fi
go_have="$(go env GOVERSION | sed 's/^go//')"
if [ "$(printf '%s\n%s\n' "$go_version" "$go_have" | sort -V | head -n1)" != "$go_version" ]; then
    echo "Go $go_version is needed and this is Go $go_have: the job has to install the Go the runtime module names" >&2
    exit 1
fi
