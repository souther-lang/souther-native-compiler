#!/usr/bin/env bash
# Every souther issue this repository names is a premise: that souther, at the commit the build pins,
# does not yet answer what the issue asks. The code around such a reference refuses, copies or works
# around what the issue says is missing. Once the issue is fixed in the pinned souther, the premise
# is false and the workaround is a copy of an answer the program now gives.
#
# So every referenced issue is asked here. One that is closed by a commit the pinned souther
# contains fails this, naming the places that still rest on it. One that is open, or closed by a
# commit after the pin, is a premise that still holds, and is listed so a pin bump shows it.
#
# Needs `gh`, authenticated.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

property() {
    sed -n "s#^ *<$1>\\(.*\\)</$1> *\$#\\1#p" pom.xml
}
version="$(property souther.version)"
if [ -z "$version" ]; then
    echo "pom.xml needs souther.version" >&2
    exit 2
fi
# What a premise is held against is the souther this build reads, which is the jar Maven resolves
# for that version, and the commit it was built from is what the jar's manifest says it was
# (`Implementation-Revision`). A tag is not asked: it names a release and not the jar a build
# resolves, and a snapshot has none. Maven resolves it as the build does, so MAVEN_ARGS that point
# the build at another local repository point this there too.
jars="$(mktemp -d)"
trap 'rm -rf "$jars"' EXIT
# Asked anew, since a snapshot kept from an earlier resolution is a build other than the latest, and
# its commit is what every premise is held against.
mvn --batch-mode --quiet --non-recursive --update-snapshots dependency:copy \
    -Dartifact="org.souther-lang:souther-compiler:$version" -DoutputDirectory="$jars" >&2
# The one jar copied, under whatever name it has: a snapshot is named by the build it resolved to.
jar=("$jars"/souther-compiler-*.jar)
pin="$(unzip -p "${jar[0]}" META-INF/MANIFEST.MF \
    | sed -n 's/^Implementation-Revision: *\([0-9a-f]*\).*$/\1/p')"
if ! [[ "$pin" =~ ^[0-9a-f]{40}$ ]]; then
    echo "souther-compiler $version says no commit it was built from (Implementation-Revision)" >&2
    exit 2
fi
echo "souther $version is $pin"

# What is searched, and the one way of searching it. Every file the repository tracks, so that a
# reference is found wherever it is put and a directory added or moved cannot leave one outside a
# list of places that nobody updated; what a build leaves behind is not tracked, so it is not read.
# The script is left out, since it holds the pattern it looks for. git grep answers 1 for finding
# nothing, which is a repository resting on no premise and not a failure, and 2 for a search that
# did not happen. Only the first is accepted, so a search that did not happen cannot be read as one
# that found nothing and let every premise pass.
references() {
    local status=0
    git grep -E "$@" -- . ':!scripts/upstream-premises.sh' || status=$?
    if [ "$status" -gt 1 ]; then
        echo "the search for references failed with status $status" >&2
        exit "$status"
    fi
}

issues="$(references -h -o 'souther-lang/souther#[0-9]+' | sed 's/.*#//' | sort -un)"

stale=0
for number in $issues; do
    state="$(gh issue view "$number" --repo souther-lang/souther --json state --jq .state)"
    if [ "$state" = "OPEN" ]; then
        echo "#$number open"
        continue
    fi
    # What fixed it. Issues in souther are often closed by hand after a merge to a branch other
    # than the default one, so the closing event seldom names a closer, and a pull request there
    # says `Refs` as often as `Closes`. So the fix is taken to be the last change in souther that
    # names the issue and landed by the time it was closed: the closer where there is one, and
    # otherwise the latest merged pull request or commit naming it. An earlier one that names it
    # only as related is not taken for the fix.
    fix="$(gh api graphql -f query='
        query($number: Int!) {
          repository(owner: "souther-lang", name: "souther") {
            issue(number: $number) {
              closedAt
              timelineItems(itemTypes: [CLOSED_EVENT, CROSS_REFERENCED_EVENT, REFERENCED_EVENT],
                            first: 100) {
                nodes {
                  ... on ClosedEvent { closer {
                    ... on PullRequest { mergeCommit { oid } mergedAt }
                    ... on Commit { oid committedDate }
                  } }
                  ... on CrossReferencedEvent { source { ... on PullRequest {
                    repository { nameWithOwner }
                    mergeCommit { oid }
                    mergedAt
                  } } }
                  ... on ReferencedEvent {
                    commitRepository { nameWithOwner }
                    commit { oid committedDate }
                  }
                }
              }
            }
          }
        }' -F number="$number" --jq '
        .data.repository.issue as $issue
        | [ $issue.timelineItems.nodes[]
            | if .closer then
                { oid: (.closer.mergeCommit.oid // .closer.oid),
                  at: (.closer.mergedAt // .closer.committedDate), closer: true }
              elif .source then
                (if .source.repository.nameWithOwner == "souther-lang/souther"
                    and .source.mergedAt != null
                 then { oid: .source.mergeCommit.oid, at: .source.mergedAt, closer: false }
                 else empty end)
              elif .commit then
                (if .commitRepository.nameWithOwner == "souther-lang/souther"
                 then { oid: .commit.oid, at: .commit.committedDate, closer: false }
                 else empty end)
              else empty end
            | select(.oid != null and .at <= $issue.closedAt) ]
        | (map(select(.closer)) | last) // (sort_by(.at) | last)
        | .oid // ""')"
    if [ -z "$fix" ]; then
        echo "#$number closed by nothing this can place against the pin; check it by hand"
        stale=1
        continue
    fi
    status="$(gh api "repos/souther-lang/souther/compare/$fix...$pin" --jq .status)"
    # The pin contains the fix exactly when the fix is behind or at the pin.
    if [ "$status" = "ahead" ] || [ "$status" = "identical" ]; then
        echo "#$number is fixed in the pinned souther ($fix), and this still rests on it:"
        references -n "souther-lang/souther#$number([^0-9]|$)" | sed 's/^/    /'
        stale=1
    else
        echo "#$number is fixed by $fix, which the pin does not contain yet"
    fi
done

exit "$stale"
