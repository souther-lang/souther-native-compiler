#!/usr/bin/env bash
# Runs each argument as a Bash command, all of them at once, and ends with failure if any of them
# did. What each prints is kept apart and printed whole once all have ended, in the order given,
# each in a group of the CI log of its own, so the log of one is not mixed with the others'.
#
# For checks that share nothing but a clone's build, which scripts/souther-native makes one at a
# time: the hosts of each language, which spent most of their time waiting on a compiler of their
# own one after another.
set -euo pipefail

logs="$(mktemp -d)"
trap 'rm -rf "$logs"' EXIT

pids=()
i=0
for command in "$@"; do
    bash -euo pipefail -c "$command" > "$logs/$i" 2>&1 &
    pids+=("$!")
    i=$((i + 1))
done

status=0
i=0
for command in "$@"; do
    if wait "${pids[$i]}"; then
        ended="passed"
    else
        ended="failed"
        status=1
    fi
    echo "::group::$ended: $command"
    cat "$logs/$i"
    echo "::endgroup::"
    if [ "$ended" = "failed" ]; then
        echo "::error::failed: $command"
    fi
    i=$((i + 1))
done
exit "$status"
