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
around. The pin is the `souther.version` and `souther.commit` of the top-level `pom.xml`.

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

    mvn test

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

The Go runtime is a module of its own in a directory of this repository, and its version is its own:
the file `bindings/go/runtime/VERSION`, beside its `go.mod`, which the Go generator is built with and
requires. It is not the compiler's, which has no reason to move when the runtime does not, and cannot
be a module's from version 2 on, where the path of the module says its major version. A module in a
directory is versioned by a tag that begins with the directory, `bindings/go/runtime/v<version>`, and
the repository's own `v<version>` is the version of no module in it. A release publishes that tag
([`scripts/publish-go-runtime.sh`](../scripts/publish-go-runtime.sh)) as the last thing before the
GitHub release, when it is not published already, since the Go module proxy keeps what it has
fetched of a tag and does not take it back. So nothing is pushed that has not been asked for first,
and each of these refuses: a version that is not a semantic version; a path that does not say the
major version from 2 on, or says one before it; a runtime that is not what it was when the tag was
published, which needs another version; and a module that cannot be fetched by its path and its
version out of a repository that has this commit tagged (a rehearsal, in a directory that stands
where GitHub does). Every build asks the same with `--check`, which pushes nothing, so a change to
the runtime that leaves its version alone is found in the pull request, and
[`scripts/verify-go-runtime-release.sh`](../scripts/verify-go-runtime-release.sh) holds the whole of
it, without a release: a host with no `replace` requires the module at a tag made that way and is
built, and each refusal is exercised. The tests that build a host do use a `replace`, since they run
in a clone, so that is what holds the resolution.

The checksums are a fact about builds that follow the commit, so the file is not committed, and a
build of a release version that does not have every one of them fails ([`ReleaseChecksums`](../compiler/src/main/java/souther/nativecode/ReleaseChecksums.java), checked
when the compiler is packaged). The check has a skip of its own, `souther.skipReleaseCheck`, so
`-Dexec.skip`, which leaves Cargo out, does not leave it out; the release sets that skip only where
it builds the generators, whose compiler is thrown away. A clone cannot rebuild a release into a
compiler that fetches nothing it can check. The generators' jars are built twice, before their
checksums are written and after, and have to be the same bytes (`project.build.outputTimestamp` is
fixed for it), which [`scripts/verify-release-build.sh`](../scripts/verify-release-build.sh) checks
of what is about to be published, together with that the publication holds the artifacts the
modules publish and no others. Tests hold the places that name the platforms, and the path of the
file, to one answer.

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
