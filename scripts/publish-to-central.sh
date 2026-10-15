#!/usr/bin/env bash
# Gives Maven Central the publication a release built: the workflow artifact `maven-repository` of
# release.yml, downloaded with `gh run download <run> -n maven-repository -D <directory>`. Run by hand
# on the owner's machine, never by CI: it signs with the owner's key, and gpg asks for its passphrase
# on the terminal.
#
# The publication is the only copy of a compiler that carries the checksums of the release's bundles,
# so it is published as it was built, and not rebuilt here. What it adds is what Central requires and
# the workflow cannot hold: a signature beside every file, and the checksums Maven's deploy did not
# write. A checksum the deploy did write is held to the file it is of, so a download that changed a
# file is refused before anything is signed.
#
# It refuses a directory that does not hold exactly one version of this repository's artifacts, a
# snapshot, and a jar without its sources and its Javadoc beside it. It works on a copy, so the
# downloaded directory is left as it was and can be given to it again.
#
# The upload is to the Central Portal's Publisher API, as the user token of the server `central` in
# ~/.m2/settings.xml, which is the server Souther's own release publishes with. The token goes to
# curl on its standard input and nowhere else: it is not printed, not written to a file, and not on
# a command line. With `--rehearse` it does everything but the upload and leaves the bundle in the
# current directory, for trying the script with a keyring of no consequence (GNUPGHOME).
#
# usage: scripts/publish-to-central.sh [--rehearse] <maven repository>
set -euo pipefail

api="https://central.sonatype.com/api/v1/publisher"
group_path="org/souther-lang"

rehearse=false
if [ "${1:-}" = "--rehearse" ]; then
    rehearse=true
    shift
fi
if [ "$#" -ne 1 ]; then
    echo "usage: $0 [--rehearse] <maven repository>" >&2
    exit 2
fi
repository="$(cd "$1" && pwd)"

if [ ! -d "$repository/$group_path" ]; then
    echo "$repository holds no $group_path: give the directory the artifact maven-repository was downloaded into" >&2
    exit 1
fi

# One version, the release's: every artifact of the group at it, and nothing at any other.
versions="$(cd "$repository/$group_path" && find . -mindepth 2 -maxdepth 2 -type d | sed 's#^\./[^/]*/##' | sort -u)"
if [ -z "$versions" ]; then
    echo "$repository/$group_path holds no version of anything" >&2
    exit 1
fi
if [ "$(printf '%s\n' "$versions" | wc -l | tr -d ' ')" -ne 1 ]; then
    echo "$repository holds more than one version, and a release is one:" >&2
    printf '  %s\n' $versions >&2
    exit 1
fi
version="$versions"
case "$version" in
    *-SNAPSHOT)
        echo "$version is a snapshot, which Maven Central does not take: a release is built from a v<version> tag" >&2
        exit 1
        ;;
esac
artifacts="$(cd "$repository/$group_path" && find . -mindepth 1 -maxdepth 1 -type d | sed 's#^\./##' | sort)"
if ! printf '%s\n' "$artifacts" | grep -qx 'souther-native-parent'; then
    echo "$repository holds no souther-native-parent, which every other pom of it inherits from" >&2
    exit 1
fi
for artifact in $artifacts; do
    base="$repository/$group_path/$artifact/$version/$artifact-$version"
    if [ ! -f "$base.pom" ]; then
        echo "no $base.pom" >&2
        exit 1
    fi
    if [ -f "$base.jar" ]; then
        for classifier in sources javadoc; do
            if [ ! -f "$base-$classifier.jar" ]; then
                echo "no $base-$classifier.jar, which Maven Central requires beside $base.jar" >&2
                exit 1
            fi
        done
    fi
done

scratch="$(mktemp -d "${TMPDIR:-/tmp}/souther-central.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT
staging="$scratch/staging"

# The version directories and nothing else of the repository: maven-metadata.xml describes a
# repository, and Central writes its own.
for artifact in $artifacts; do
    mkdir -p "$staging/$group_path/$artifact"
    cp -R "$repository/$group_path/$artifact/$version" "$staging/$group_path/$artifact/$version"
done

# What is published, as opposed to what is said about it: the files Central wants a signature and
# checksums beside.
published="$(cd "$staging" && find "$group_path" -type f \
    ! -name '*.md5' ! -name '*.sha1' ! -name '*.sha256' ! -name '*.sha512' ! -name '*.asc' | sort)"

# Signatures are the owner's to make, so any that came with the download are dropped and made again.
find "$staging" -type f -name '*.asc' -delete

digest() {
    case "$1" in
        md5) md5 -q "$2" ;;
        sha1) shasum -a 1 "$2" | cut -d ' ' -f 1 ;;
        sha256) shasum -a 256 "$2" | cut -d ' ' -f 1 ;;
        sha512) shasum -a 512 "$2" | cut -d ' ' -f 1 ;;
    esac
}

for file in $published; do
    for algorithm in md5 sha1 sha256 sha512; do
        sum="$staging/$file.$algorithm"
        actual="$(digest "$algorithm" "$staging/$file")"
        if [ -f "$sum" ]; then
            recorded="$(tr -d '[:space:]' < "$sum")"
            if [ "$recorded" != "$actual" ]; then
                echo "$file is not the file its $algorithm says it is: download the artifact again" >&2
                exit 1
            fi
        else
            printf '%s' "$actual" > "$sum"
        fi
    done
done

# gpg's own default key unless SOUTHER_GPG_KEY names one.
signer=()
if [ -n "${SOUTHER_GPG_KEY:-}" ]; then
    signer=(--local-user "$SOUTHER_GPG_KEY")
fi
echo "signing the publication of $version with gpg"
for file in $published; do
    gpg --armor --detach-sign ${signer[@]+"${signer[@]}"} --output "$staging/$file.asc" "$staging/$file"
    if ! gpg --verify "$staging/$file.asc" "$staging/$file" 2> /dev/null; then
        echo "the signature gpg wrote for $file does not verify" >&2
        exit 1
    fi
done

bundle="$scratch/souther-native-$version.zip"
(cd "$staging" && zip -qr "$bundle" "$group_path")

if [ "$rehearse" = true ]; then
    cp "$bundle" .
    echo "rehearsed: the bundle is $(pwd)/$(basename "$bundle"), and nothing was uploaded"
    exit 0
fi

# The user token, as the Portal takes it: base64 of username:password, the pair Maven's settings
# hold for the server.
settings="$HOME/.m2/settings.xml"
if [ ! -f "$settings" ]; then
    echo "no $settings, which holds the token of the server central" >&2
    exit 1
fi
server_field() {
    xmllint --xpath "string(//*[local-name()='server'][*[local-name()='id']='central']/*[local-name()='$1'])" "$settings"
}
username="$(server_field username)"
password="$(server_field password)"
if [ -z "$username" ] || [ -z "$password" ]; then
    echo "$settings has no username and password for the server central" >&2
    exit 1
fi
case "$password" in
    \{*\} | *'${'*)
        echo "the password of the server central in $settings is encrypted or interpolated, and this reads it only as written" >&2
        exit 1
        ;;
esac
token="$(printf '%s:%s' "$username" "$password" | base64 | tr -d '\n')"
unset username password

# curl reads the header from its standard input (-K -), so the token is on no command line.
central() {
    printf 'header = "Authorization: Bearer %s"\n' "$token" | curl --silent --show-error --fail-with-body -K - "$@"
}

echo "uploading $(basename "$bundle")"
deployment="$(central -X POST -F "bundle=@$bundle" "$api/upload?publishingType=AUTOMATIC&name=souther-native-$version")"
echo "deployment: $deployment"

# Until Central says it is published or has failed. Interrupting this leaves the deployment going;
# its state is on https://central.sonatype.com/publishing/deployments.
last=""
while true; do
    status="$(central -X POST "$api/status?id=$deployment")"
    state="$(printf '%s' "$status" | sed -nE 's/.*"deploymentState"[[:space:]]*:[[:space:]]*"([A-Z]+)".*/\1/p')"
    if [ "$state" != "$last" ]; then
        echo "state: ${state:-unknown}"
        last="$state"
    fi
    case "$state" in
        PUBLISHED)
            exit 0
            ;;
        FAILED)
            echo "$status" >&2
            exit 1
            ;;
    esac
    sleep 10
done
