# souther-native-compiler

Compiles a checked Souther program to a host-native object.

A behavior becomes a symbol in an object file the system linker takes, so the output runs where
there is no virtual machine and no wasm engine. A library built for a host is a shared library with
a header, and a binding generated from it lets PHP, Rust or Go call it as that language's own code.

## Documents

- [docs/host-abi.md](docs/host-abi.md): what a library built for a host offers at the C level, and
  what a host, or the runtime a binding runs on, must follow to call it.
- [docs/writing-a-binding.md](docs/writing-a-binding.md): how a binding is generated, the interface a
  generator implements and the model it is handed.
- [bindings/php/runtime/README.md](bindings/php/runtime/README.md),
  [bindings/rust/runtime/README.md](bindings/rust/runtime/README.md) and
  [bindings/go/runtime/README.md](bindings/go/runtime/README.md): using the PHP, the Rust and the Go
  binding in an application.
- [docs/language-coverage.md](docs/language-coverage.md): what of the language compiles, and how a
  value is held.
- [docs/development.md](docs/development.md): how the compiler is put together, built, released and
  held to what the language says.

The examples under [examples/](examples/) are one cart model run by a PHP, a Rust and a Go
application, each with a README of its own.

## Modules

The build is one Maven reactor of seven modules. `bindings/api` (`souther-bindings-api`) is what a
binding generator is written against: the interface a generator implements and the model of a
library it is handed. `compiler` (`souther-native-compiler`) is the compiler and the command, which
reads the manifest a build writes into that model and puts each binding in place; it also builds the
backend jar, the command with everything it runs with, which `souther compile --target native` runs.
`bindings/testkit` (`souther-bindings-testkit`) is what a generator is tested with: sources built
into a library by the compiler and driver of its release. `bindings/php/generator`,
`bindings/rust/generator` and `bindings/go/generator` are the generators, each beside the runtime in
its language that the code it writes calls into. `launcher` is the command as a clone runs it, handed
the jars of the three generators this build made, and is not published. The Rust half of the compiler,
the driver and the runtime a library links, is the Cargo workspace in `native/`.

The generators depend on the API and on nothing else of this project, and none of them can see the
compiler, a checked program or how the manifest is written. That a binding is written from the
library's model and nothing else is therefore a fact about which classes a generator can name, and
the command holds it at run time too: it loads each generator's jar behind a class loader through
which only the JDK, the API and the jar itself resolve.

## From the command line

What the API builds, the command line builds too, so an application needs no Java of its own to
build what it runs. The command is `souther compile --target native`: the Souther CLI finds this
backend among those installed beside it, checks that it was built against the CLI's own Souther, and
runs it as a process of its own with every argument after the target. Only a JVM is needed to run
it, which the CLI needs anyway.

    souther compile --target native --library build/native --php build/php --namespace Acme\Shop model

    souther compile --target native [--offline] [-cp <path>] -o <object> <source>...
    souther compile --target native [--offline] [-cp <path>] --library <dir>
        [--with <object>]... [--php <dir> --namespace <ns>] [--rust <dir> --crate <name>]
        [--go <dir> --package <import path>] <source>...
    souther compile --target native --fetch

The backend is the jar `souther-native-compiler-<version>-backend.jar`, released on Maven Central
beside the compiler at this repository's own version. Its descriptor,
`META-INF/souther/backend.properties`, names the target `native` and the Souther it was built
against, and the CLI runs a backend only where that Souther is exactly its own, so a backend is
released again for each Souther. A package manager installs it where its `souther` looks; by hand,
it goes into `$SOUTHER_HOME/backends`, which the CLI searches first. Jars built against different
Souther versions can sit there side by side, and the CLI chooses the one built against itself.

A source is a `.sou` file or a directory holding some. `-o` writes one object file. `--library`
writes what a host is handed into its directory ([docs/host-abi.md](docs/host-abi.md)), and `--php`,
`--rust` and `--go` each write that binding of it, from the library's model. A program importing
another build reads that build's modules from `-cp`, the class path the `souther` command takes, and
has its object linked in with `--with`, one for each build. The command ends with 0 where it wrote
everything, 1 where the build is refused (a compile error, what this backend does not write yet, a
name from the model a binding's language will not take, or a generator that failed), and 2 where the
command is refused.

Each directory is replaced whole, and each is its own. A binding's directory is written beside where
it goes and put in place only once every binding asked for has been written, so a binding refused
for a name in the model leaves the new library and every binding that was there before. A namespace
PHP will not take, a crate name Cargo will not take, an import path Go will not take, or a binding
directory holding what no binding of that language wrote, is refused before the library is built.

The compiler is one artifact, and what a command needs beyond it is fetched the first time it needs
it and kept in `~/.souther` (`$SOUTHER_HOME` where it is set): the driver for the platform it runs
on, from the GitHub release of its own version, and the generator of each binding that is asked for,
from Maven Central at that version. Nothing else is fetched unless the command line names it: a
flag of the catalog brings what the catalog names, and nobody who does not use the Rust binding has
the Rust generator.

A binding of someone else's is written by their generator's jar, named by its coordinate and the
SHA-256 of the jar, or by a path to a jar for its author's own build:

    souther compile --target native --library build/native \
        --binding com.acme:souther-binding-kotlin:1.2.0@sha256:<64 hex> build/kotlin \
        --binding-option package=com.acme.shop model

How a generator is written, what its jar says of itself, and how the command runs it is
[docs/writing-a-binding.md](docs/writing-a-binding.md).

What is fetched is run, and neither place it is fetched from can say what it is: an asset of a
GitHub release can be replaced, and a Maven repository can be a mirror. So the SHA-256 of each bundle
and of each generator's jar is written into the compiler's own artifact when it is released, Maven
Central does not let that artifact be changed, and anything fetched that does not match is refused
and not kept. A checksum served beside a file is never asked for. What is kept is checked again each
time it is used, and fetched again where it no longer matches; and what runs is never the kept file,
but a copy the command writes for itself from the bytes it just checked. `--offline` fetches nothing and uses only
what is kept, and `--fetch` fetches everything a command may need, so that a build that may not
reach the network later can be prepared where one can. The Maven repository can be a mirror, named
by `-Dsouther.maven.repository`.

A build from a clone has no release to fetch from, and does not try: it uses the driver Cargo built,
which `scripts/souther-native` names by `-Dsouther.native.driver`, and the jars of the generators the
same build made, which it names by `-Dsouther.generator.<id>` and the command loads as it loads any
generator's jar. That property is the one place such a build looks for a driver, and a release never
reads it, as it reads no property naming a generator: a release runs only what its checksums name. A
compiler does not look in the directory it is run in, where a project of somebody else's could have
an executable of the same name. In a clone, the script stands where `souther compile --target native`
stands:

    scripts/souther-native --library build/native --php build/php --namespace Acme\Shop model

The script builds what the command needs and runs it in the directory it was started in, so the
paths are read from there. The jar is also run directly with `java -jar`, which is how the CLI starts
it, for working on the backend and for finding out what it does. How an application then depends on
what was written is each binding's own README.
