# Developing the compiler

For whoever changes this compiler: how it is put together, how it is built and released, where it
runs, and how what it writes is known to be right. What a host is handed is in
[host-abi.md](host-abi.md), and what of the language compiles in
[language-coverage.md](language-coverage.md).

## What it reads

`CheckedProgram` and what is reachable from it, and nothing else of the Souther compiler. Anything
this needs that the program API does not carry is a question for Souther rather than something to
reach around, so it is raised there. This project is the second reader of that boundary, and what
it finds the boundary does not answer is the most useful thing it produces.

Where the code works around something Souther does not answer yet, it names the Souther issue that
asks for it. [`scripts/upstream-premises.sh`](../scripts/upstream-premises.sh), run in CI, fails
once the Souther this build pins has the fix, so a workaround does not outlive what it worked
around. The pin is the `souther.version` of the top-level `pom.xml`, and the commit it is held at
is the one the souther-compiler jar of that version says it was built from
(`Implementation-Revision`).

## The two halves

The Java half reads a checked program and writes it out. It decides nothing: the projection over
`Core` is exhaustive, so a node added to the language stops this compiling rather than travelling
as something else.

The Rust half reads that and lowers it to Cranelift IR, and Cranelift writes the object. The two
speak over a process boundary: the Java half hands the driver the program on its standard input,
once per build, and is handed back an object file, or with `--library` the names of the five files
the driver wrote into a directory for a host. There is nothing here for an in-process call to make
faster, and a panic on the Rust side does not take a JVM with it.

## What is not shared with the wasm backend

The lowering is not, and neither is the representation. A value's layout, what a pointer is, and
how a call is made are what a target decides, and the two targets decide them differently. What the
two backends share is above them — the checked program — and, where the same computation turns out
to be written twice, below them in the Rust runtime. Nothing in the middle is shared, and no
abstraction over the two is written to make it look as though something is.

## Building

    mvn verify

Cargo is what builds the Rust half; Maven runs it, and formats, lints and tests it as well. The
toolchain is pinned in [`rust-toolchain.toml`](../rust-toolchain.toml), so a clone needs rustup and
nothing else installed by hand for it. A C and a C++ compiler are needed too, by the tests that link
what came out and run it, and PHP 8.3 or later with the `ffi` and `intl` extensions, by the tests
that read what a host is handed the way an FFI with no preprocessor does and run a generated PHP
binding. A binding runs on PHP 8.2; the tests ask for 8.3 because they have one PHP lint many files,
which 8.3 is the first to do. Maven also runs Composer, which has to be installed, for what the PHP
runtime in `bindings/php/runtime` depends on, as its `composer.lock` fixes it.

The Rust runtime in `bindings/rust/runtime` is a crate of its own, which Maven formats, lints and
tests from the Rust generator's module, and the tests of a generated Rust binding build a host of it
with Cargo, fetching what the runtime depends on the first time. Go is needed too, the version the
runtime module's `go.mod` names (the one place it is said; the workflows and the scripts read it
from there), with a C compiler, since a Go binding is built with cgo: Maven formats, vets and tests
the Go runtime in `bindings/go/runtime` under the race detector, and the tests of a generated Go
binding build a host of it with the Go toolchain, fetching what the runtime depends on the first
time.

The tests are told where the driver is by Maven, through the property `souther.native.driver`,
which the top-level `pom.xml` sets to the driver Cargo built. A test run from an IDE names it the
same way (`-Dsouther.native.driver=<clone>/native/target/debug/souther-native-driver`).

## Releasing

A release is a `v<version>` tag, and
[`.github/workflows/release.yml`](../.github/workflows/release.yml) does what the tag names. A tag
can be put on any commit, and a release is not replaced once it is out, so the first thing it does
is run the build (`build.yml`, called from it) on the commit that is tagged, and nothing after it
starts unless that passes; a test holds every job of the release to waiting for it. It then builds
the driver on each of the four platforms (Linux and macOS, on x86_64 and aarch64) and packs it with
the runtime archive and the file of what linking that needs, by
[`scripts/package-native-bundle.sh`](../scripts/package-native-bundle.sh). It builds the
generators' jars, and writes the SHA-256 of every bundle and every jar into
`release-checksums.properties`
([`scripts/record-release-checksums.sh`](../scripts/record-release-checksums.sh)), which is built
into the compiler's jar. It then builds the publication, as a Maven repository, and keeps it as the
workflow's artifact `maven-repository`, with the compiler in it carrying those checksums, before it
creates the GitHub release with the bundles and the checksums file.

What `souther compile --target native` runs is the backend jar, which the compiler's module attaches
beside its own jar with the classifier `backend`: the compiler and everything it runs with, carrying
the compiler's manifest, and so its version, which is the release it fetches at. Its descriptor,
`META-INF/souther/backend.properties`, holds the target `native` and the `souther.version` of the
top-level `pom.xml`, written in when it is packaged (`compiler/src/backend`). A class asks what it is
through its package, which the JVM answers from the manifest of the jar it was loaded from, and
shading leaves one manifest for every package; so after shading,
[`compiler/src/build/BackendManifest.java`](../compiler/src/build/BackendManifest.java) gives each
package a section of its own holding what its own jar's manifest says, and leaves the main section
saying nothing of what anything is. Souther's compiler inside the jar still says it is Souther's
release, the API beside the compiler says it is the backend's, and a package that came from no jar,
or from two jars of different releases, refuses the build. The CLI runs a backend
only where that version is exactly its own, so each Souther release needs a release of this backend
built against it, and a release of this backend leaves the Souther it names alone. A test in the
launcher's module holds the descriptor to the Souther compiler the build compiles against, holds
every package of the jar to a section of its own, and runs the jar alone as the CLI runs it; `verify-release-build.sh` holds the published jar to the
checksums, as it holds the compiler's.

The CLI that runs an installed backend is not published for the Souther snapshot a clone builds
against, so in a clone the examples' `bin/build` is run with `$SOUTHER` naming
[`scripts/souther`](../scripts/souther), which takes `souther compile --target native` and runs
`scripts/souther-native` with the rest. Given a CLI that does run installed backends,
[`scripts/with-the-souther-cli`](../scripts/with-the-souther-cli) puts this clone's backend jar where
that CLI looks and runs a command with `$SOUTHER` naming it, so
`scripts/with-the-souther-cli <souther> scripts/rust-cart-example.sh` is the example as a user runs it.

The Go runtime is a module of its own in a directory of this repository, and its version is its own:
the file `bindings/go/runtime/VERSION`, beside its `go.mod`, which the Go generator is built with
and requires. It is not the compiler's, which has no reason to move when the runtime does not, and
cannot be a module's from version 2 on, where the path of the module says its major version. A
module in a directory is versioned by a tag that begins with the directory,
`bindings/go/runtime/v<version>`, and the repository's own `v<version>` is the version of no module
in it. That tag is pushed by hand
([`scripts/publish-go-runtime.sh`](../scripts/publish-go-runtime.sh)), when it is not published
already, since the Go module proxy keeps what it has fetched of a tag and does not take it back. So
nothing is pushed that has not been asked for first, and each of these refuses: a version that is
not a semantic version; a path that does not say the major version from 2 on, or says one before it;
a runtime that is not what it was when the tag was published, which needs another version; a module
that cannot be fetched by its path and its version out of a repository that has this commit tagged
(a rehearsal, in a directory that stands where GitHub does); and, when it publishes, a runtime that
requires a pseudo-version rather than a release. What it is run for is said rather than read from
whether it pushes: `--check` pushes nothing, `--rehearse` pushes to a repository that is a directory
here and asks nothing of a release, and without either it publishes and asks all of it, so a check
and a rehearsal hold between releases too, while the runtime may require a commit of Raoh, which only
a publication refuses. Every build asks the same with `--check`, which pushes nothing, so a change to
the runtime that leaves its version alone is found in the pull request, and
[`scripts/verify-go-runtime-release.sh`](../scripts/verify-go-runtime-release.sh) holds the whole of
it, without a release: a host with no `replace` requires the module at a tag rehearsed that way and
is built, and each refusal is exercised, a publication's of a pseudo-version with a runtime of its own. The tests that build a host do use a `replace`, since they run
in a clone, so that is what holds the resolution.

Nothing is published from CI. The release workflow keeps the publication and creates the GitHub
release, and what goes to a registry is published by hand, from a clean clone at the tag, once the
Souther the backend names is on Maven Central: the publication, which holds the backend jar, to Maven
Central from the workflow's `maven-repository` artifact; the Rust runtime with `cargo publish` in
`bindings/rust/runtime`; the Go runtime's tag with `scripts/publish-go-runtime.sh`; and the PHP
runtime with [`scripts/publish-php-runtime.sh`](../scripts/publish-php-runtime.sh). Packagist reads
a package from the root of a repository, so the PHP runtime is published to a mirror,
`souther-lang/php-runtime`, which holds the runtime and nothing else: each version is one commit whose
tree is `bindings/php/runtime` at the commit published, tagged `v<version>`, the version being the
runtime's own, in `bindings/php/runtime/VERSION`. The script has the Go one's three modes and refuses as it
does: a version that is not a semantic version, a tag already standing at another runtime, a package
Composer does not validate, and, when it publishes, a runtime requiring a development version.
[`scripts/verify-php-runtime-release.sh`](../scripts/verify-php-runtime-release.sh) rehearses it in
every build: a host requires the package by its name and version from a mirror that is a directory,
and each refusal is exercised. Each runtime
requires only released versions of Raoh when it is published; Cargo refuses a git dependency on its
own, and the two scripts refuse the rest.

The checksums are a fact about builds that follow the commit, so the file is not committed, and a
build of a release version that does not have every one of them fails
([`ReleaseChecksums`](../compiler/src/main/java/souther/nativecode/ReleaseChecksums.java), checked
when the compiler is packaged). The check has a skip of its own, `souther.skipReleaseCheck`, so
`-Dexec.skip`, which leaves Cargo out, does not leave it out; the release sets that skip only where
it builds the generators, whose compiler is thrown away. A clone cannot rebuild a release into a
compiler that fetches nothing it can check. The generators' jars are built twice, before their
checksums are written and after, and have to be the same bytes (`project.build.outputTimestamp` is
fixed for it), which [`scripts/verify-release-build.sh`](../scripts/verify-release-build.sh) checks
of what is about to be published, together with that the publication holds the artifacts the modules
publish and no others. Tests hold the places that name the platforms, and the path of the file, to
one answer.

## Where it runs

Unix hosts: what the object is written as is decided by the host's format, and Mach-O and ELF are
the two anything here has been run on. Writing an object is not refused anywhere else — it is
untried, and the test that links and runs would have to say what a COFF object and its linker want
before it meant anything there. Linking a shared library for a host is refused on anything but
macOS and Linux, since what it asks of the linker is said only for those two, and a release builds
the driver only for those two, on x86_64 and aarch64.

## How it is known to be right

By the program's own `example` rows, and by what a row's arrival says about it. A row the compile
ran arrives as one an output can put to its own emission; a row whose answer did not keep it
refuses the program. So for such a row the JVM answered and the answer kept the row, and putting
the native run to the same row holds both carriers to one statement without this project writing
down what either of them should say. Whether an answer keeps a row is asked of the row, so there is
no second reading of what a row means either.

A row the compile did not run arrives saying so and carrying why. Those are not skipped: skipping
them is how a check goes on being green over fewer and fewer rows. A row whose answer is owed, one
that states no answer, is run as any other and holds nothing, and the test that puts rows to the
native run refuses one as having nothing to hold the run to.

The object carries an entry for every row whose values the compile read, which is every row but one
the compile did not run, and that entry is what runs it. The entry calls the behavior with what
computes each of the row's inputs: the definition the checked program names for it, which the
module holds as one of its helpers, whose body is the operand as the row writes it, elaborated by
the checker at the parameter it is handed to. So running a row is the object doing something with
the row and not the behavior being reached with values from outside — which is what lets a row of a
name the module keeps be run at all — and how a value stands where it is handed over, a case where
its sum is taken or a value given to an optional field, is the checker's to say and not this
backend's. A row whose operand this backend has no expression for refuses the build, as any body
does, rather than being left out of the object: an object missing an entry would link and answer
every row it did carry.

This is not the two carriers compared against each other. Holding both to one statement is not
running both and comparing what came back, and running a program on every carrier and comparing the
answers is still ahead. Two places do ask the JVM directly: the calendar, which
`ATemporalAnswersWhatTheJvmAnswersTest` holds to what `java.time` answers over the ends of every
range and a seeded run of the rest, and a clause a decoder reports as broken, which
`ABrokenClauseIsReportedAsTheJvmReportsItTest` holds to what the JVM's generated decoder reports for
the same module.
