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

The build is one Maven reactor of six modules. `bindings/api` (`souther-bindings-api`) is what a
binding generator is written against: the interface a generator implements and the model of a
library it is handed. `compiler` (`souther-native-compiler`) is the compiler and the command, which
reads the manifest a build writes into that model and puts each binding in place.
`bindings/php/generator`, `bindings/rust/generator` and `bindings/go/generator` are the generators,
each beside the runtime in its language that the code it writes calls into. `launcher` is the
command with the three generators installed, and is not published. The Rust half of the compiler,
the driver and the runtime a library links, is the Cargo workspace in `native/`.

The generators depend on the API and on nothing else of this project, and none of them can see the
compiler, a checked program or how the manifest is written. That a binding is written from the
library's model and nothing else is therefore a fact about which classes a generator can name.

## From the command line

What the API builds, the command line builds too, so an application needs no Java of its own to
build what it runs. Only a JVM is needed to start it, through [jbang](https://www.jbang.dev/):

    jbang souther-native@souther-lang/souther-native-compiler \
        --library build/native --php build/php --namespace Acme\Shop model

    souther-native [--offline] [-cp <path>] -o <object> <source>...
    souther-native [--offline] [-cp <path>] --library <dir> [--with <object>]...
                   [--php <dir> --namespace <ns>] [--rust <dir> --crate <name>]
                   [--go <dir> --package <import path>] <source>...
    souther-native --fetch

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
from Maven Central at that version. Nothing else is ever fetched: what a flag can bring is what the
command's catalog of bindings names, and nobody who does not use the Rust binding has the Rust
generator.

What is fetched is run, and neither place it is fetched from can say what it is: an asset of a
GitHub release can be replaced, and a Maven repository can be a mirror. So the SHA-256 of each bundle
and of each generator's jar is written into the compiler's own artifact when it is released, Maven
Central does not let that artifact be changed, and anything fetched that does not match is refused
and not kept. A checksum served beside a file is never asked for. What is kept is trusted as a file
in `~/.m2` is, and is not checked again when it is used. `--offline` fetches nothing and uses only
what is kept, and `--fetch` fetches everything a command may need, so that a build that may not
reach the network later can be prepared where one can. The Maven repository can be a mirror, named
by `-Dsouther.maven.repository`.

A build from a clone has no release to fetch from, and does not try: it uses the driver Cargo built,
which `scripts/souther-native` names by `-Dsouther.native.driver`, and the generators the launcher
carries. That property is the one place a driver is looked for; a compiler does not look in the
directory it is run in, where a project of somebody else's could have an executable of the same
name. In a clone:

    scripts/souther-native --library build/native --php build/php --namespace Acme\Shop model

The script builds what the command needs and runs it in the directory it was started in, so the
paths are read from there. How an application then depends on what was written is each binding's own
README.
