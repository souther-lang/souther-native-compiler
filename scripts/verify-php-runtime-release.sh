#!/usr/bin/env bash
# Holds what publishing the PHP runtime does to what a host needs of it, without publishing it.
#
# The tests and the example reach the runtime as a Composer path repository, which only a clone has.
# What anybody else has is the package Packagist reads from the mirror, so whether a host can require
# it by its name and version from a mirror the script made is asked here, of a mirror that is only a
# directory:
#
#   - HEAD is published to it as a publication would be, rehearsed (--rehearse), and a host that has
#     only the mirror as a repository requires the package at its version and loads its classes;
#   - publishing it again is publishing nothing, and the same commit is not a reason to refuse;
#   - a runtime that changed and kept its version is refused, and so is a version that is not one,
#     each before anything is pushed;
#   - a publication, which a rehearsal is not, refuses a runtime requiring a development version,
#     before anything is pushed, and publishes one requiring releases only; and a rehearsal pushes to
#     nothing but a directory.
#
# HEAD is rehearsed and not published, so this holds while the runtime still requires a development
# version of raoh-php, which a publication would refuse; what a publication refuses is held with a
# runtime of its own below. Needs PHP and Composer, and reaches raoh-php on Packagist. What is
# published is HEAD, so what is not committed is not in it.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

# What this clone is, which publishing reads and does not change.
clone() {
    git rev-parse --is-shallow-repository
    git for-each-ref --format='%(refname) %(objectname)' refs/heads refs/tags
}
before="$(clone)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

directory="bindings/php/runtime"
version="$(tr -d '[:space:]' < "$directory/VERSION")"
protocol="$(sed -nE 's#^    public const PROTOCOL = ([0-9]+);$#\1#p' "$directory/src/Binding.php")"

fail() {
    echo "$*" >&2
    exit 1
}

# The publisher runs with no Git configuration but the clone's own, and with Git told not to guess an
# identity from the system (user.useConfigOnly), which it does on one system and not on another: no
# identity and no signing, as a clean environment has it. What the commits it makes are made of is
# its own to say, and is not to be found here in a configuration it happened to run under.
publish() {
    HOME="$work/home" XDG_CONFIG_HOME="$work/home" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null \
        GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=user.useConfigOnly GIT_CONFIG_VALUE_0=true \
        "$root/scripts/publish-php-runtime.sh" "$@"
}
mkdir "$work/home"
publish="publish"

# Published as a publication publishes it, rehearsed.
git init --quiet --bare "$work/mirror.git"
"$publish" --rehearse "$work/mirror.git" HEAD
[ "$(git -C "$work/mirror.git" rev-parse "refs/tags/v$version^{tree}")" = "$(git rev-parse "HEAD:$directory")" ] \
    || fail "the mirror's v$version is not $directory as HEAD has it"

# The commit is made of the commit it publishes, and of nothing of where it was published from: on
# another mirror, it is the same commit.
[ "$(git -C "$work/mirror.git" show -s --format='%an <%ae> %ad %cn <%ce> %cd' --date=raw "v$version")" \
    = "$(git show -s --format='%an <%ae> %ad %cn <%ce> %cd' --date=raw HEAD)" ] \
    || fail "the mirror's commit is not authored and committed as HEAD is"
git init --quiet --bare "$work/again.git"
"$publish" --rehearse "$work/again.git" HEAD > /dev/null
[ "$(git -C "$work/again.git" rev-parse "v$version")" = "$(git -C "$work/mirror.git" rev-parse "v$version")" ] \
    || fail "HEAD published to two mirrors made two commits"

# Publishing what is published is publishing nothing.
main="$(git -C "$work/mirror.git" rev-parse refs/heads/main)"
said="$("$publish" --rehearse "$work/mirror.git" HEAD)"
case "$said" in
    *"is published on"*"and is this runtime"*) ;;
    *) fail "published again, it said: $said" ;;
esac
[ "$(git -C "$work/mirror.git" rev-parse refs/heads/main)" = "$main" ] || fail "publishing again pushed"

# What a host has: the package by its name and version, from the mirror and Packagist and nothing
# local. The runtime may require a development version of raoh-php until it is published, which a
# host takes only where it lowers its stability; a publication refuses that, below.
mkdir "$work/host"
cat > "$work/host/composer.json" <<JSON
{
    "repositories": [{ "type": "vcs", "url": "$work/mirror.git" }],
    "require": { "souther-lang/php-runtime": "$version" },
    "minimum-stability": "dev",
    "prefer-stable": true
}
JSON
(cd "$work/host" && COMPOSER_HOME="$work/composer" composer install --quiet --no-interaction)
said="$(cd "$work/host" && php -r 'require "vendor/autoload.php"; echo \Souther\Runtime\Binding::PROTOCOL;')"
[ "$said" = "$protocol" ] || fail "the host printed '$said', not the protocol '$protocol' of the runtime it required"
echo "a host that requires souther-lang/php-runtime $version from the mirror loads it"

# What is refused, of a repository that has only the runtime in it: changed in a commit each.
git_in() {
    git -C "$work/synthetic" -c user.name=check -c user.email=check@example.com -c commit.gpgSign=false "$@"
}
mkdir -p "$work/synthetic/$(dirname "$directory")"
git archive "HEAD:$directory" | (mkdir "$work/synthetic/$directory" && tar -x -C "$work/synthetic/$directory")
git init --quiet "$work/synthetic"
git_in add -A
git_in commit --quiet -m "the runtime"
git init --quiet --bare "$work/none.git"

refused() {
    local why="$1"
    local wanted="$2"
    shift 2
    local said
    if said="$(cd "$work/synthetic" && "$publish" "$@" 2>&1)"; then
        fail "$why was not refused: $said"
    fi
    case "$said" in
        *"$wanted"*) ;;
        *) fail "$why was refused for another reason than '$wanted': $said" ;;
    esac
    [ -z "$(git ls-remote "$work/none.git")" ] || fail "$why was refused after something was pushed"
    echo "refused: $why"
}

printf '1.0\n' > "$work/synthetic/$directory/VERSION"
git_in commit --quiet -am "no version"
refused "a version that is not one" "is not a semantic version" --check "$work/none.git" HEAD
git_in reset --quiet --hard HEAD~1

# A published runtime that changed and kept its version.
git init --quiet --bare "$work/second.git"
(cd "$work/synthetic" && "$publish" --rehearse "$work/second.git" HEAD > /dev/null)
printf '\n// changed\n' >> "$work/synthetic/$directory/src/Binding.php"
git_in commit --quiet -am "changed, and not given another version"
refused "a runtime that changed and kept its version" "give it another version" --check "$work/second.git" HEAD
git_in reset --quiet --hard HEAD~1

# A publication requires releases only, whatever raoh-php the runtime requires today.
php -r '$p = "'"$work/synthetic/$directory"'/composer.json"; $j = json_decode(file_get_contents($p), true);
    $j["require"]["example/unreleased"] = "^1.0@dev";
    file_put_contents($p, json_encode($j, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES) . "\n");'
git_in commit --quiet -am "requires a development version"
refused "a publication of a runtime requiring a development version" "requires what is not released" \
    "$work/none.git" HEAD
git_in reset --quiet --hard HEAD~1

# And one requiring releases only is published.
php -r '$p = "'"$work/synthetic/$directory"'/composer.json"; $j = json_decode(file_get_contents($p), true);
    $j["require"] = array_filter($j["require"], fn ($c) => !preg_match("/(^dev-|-dev$|@dev)/i", $c));
    file_put_contents($p, json_encode($j, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES) . "\n");'
git_in commit --quiet -am "requires releases only"
git init --quiet --bare "$work/third.git"
said="$(cd "$work/synthetic" && "$publish" "$work/third.git" HEAD 2>&1)" \
    || fail "a publication requiring releases only was refused: $said"
[ -n "$(git ls-remote "$work/third.git" "refs/tags/v$version")" ] \
    || fail "a publication requiring releases only pushed no tag: $said"
echo "a publication requiring releases only is published"

# A rehearsal reaches nothing but a directory.
if said="$("$publish" --rehearse https://github.com/souther-lang/php-runtime.git HEAD 2>&1)"; then
    fail "a rehearsal to the mirror on GitHub was not refused: $said"
fi
echo "refused: a rehearsal to a mirror that is not a directory"

[ "$(clone)" = "$before" ] || fail "publishing changed this clone's refs"
