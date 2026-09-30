# Writing a binding

How a binding for a host's language is made: the generator the command runs to write it, the model
the generator is handed, and the runtime the code it writes calls into. The PHP, Rust and Go
bindings in [`bindings/`](../bindings/) are made this way, and each is an example of everything
below. What a library offers at the C level, which a binding's runtime follows, is
[host-abi.md](host-abi.md).

A binding has two parts. The generator is Java, written against `souther-bindings-api`
([`bindings/api`](../bindings/api/)), and writes source in the host's language from a library's
model. The runtime is a library in the host's language, published the way that language publishes
one, which the written source calls into and which loads the library and calls the C ABI. The
generator knows the runtime's surface, since it writes calls to it, and the two move together. The
command knows neither.

The command ships three generators, which its catalog names
([`KnownBindings`](../compiler/src/main/java/souther/nativecode/KnownBindings.java)) and asks for by
their own flags, and runs a generator of anyone else's that it is pointed at with `--binding`. Both
are a jar, and the command runs both the same way.

## Pointing the command at a generator

A generator of someone else's is named by its Maven coordinate and the SHA-256 of its jar:

```text
souther-native --library out/native \
    --binding com.acme:souther-binding-kotlin:1.2.0@sha256:<64 hex> out/kotlin \
    --binding-option package=com.acme.shop \
    src/
```

The digest is required, and nothing is trusted on first use: a coordinate says where to fetch the
jar from, and the digest which bytes it has to be. Other bytes under the same coordinate are another
artifact, so a fetched jar is kept under its coordinate and its digest, in
`$SOUTHER_HOME/generators/<group>/<artifact>/<version>/<sha256>.jar`, and hashed again each time it
is used. `--binding-option key=value` belongs to the `--binding` before it and may be repeated; the
command splits it at the first `=`, and refuses a key named twice for one binding.

An author's own build is named by the path of its jar, `--binding build/libs/generator.jar out/kotlin`,
and no digest is asked for. The command copies the jar and loads the copy, so a jar rebuilt while
the command runs is not what runs.

The three bindings the command ships are the same thing reached another way. A released command
fetches the jar its catalog names at its own version, held to the SHA-256 the release carries for it;
a clone's build names the jars its reactor made with `-Dsouther.generator.<id>`, as
[`scripts/souther-native`](../scripts/souther-native) does. From there
([`GeneratorSpec`](../compiler/src/main/java/souther/nativecode/GeneratorSpec.java)) nothing tells
them apart from a jar named with `--binding`.

The digest decides whether code runs. The class loader below decides only what that code can see; it
is not a sandbox, and a generator runs with the command's rights, as a Maven plugin does.

## How the command runs a generator

The command reads its whole command line before anything is run. Then, for each binding asked for,
it runs the generator's jar through a fixed order, in which each step runs nothing the one before did
not allow ([`Bindings`](../compiler/src/main/java/souther/nativecode/Bindings.java)):

```text
bytes verified → MANIFEST.MF (id, API major, ABI generations) → which id it may be
    → its own class loader → exactly one provider → constructed → preflight → generate
```

Nothing a jar says of itself is read before its digest has matched
([`GeneratorArtifacts`](../compiler/src/main/java/souther/nativecode/GeneratorArtifacts.java)). What
its manifest says is checked before any of its code runs
([`GeneratorDescriptor`](../compiler/src/main/java/souther/nativecode/GeneratorDescriptor.java)): a
jar that says it is another generator than the one asked for, one of the catalog's when it was named
with `--binding`, or that was written against another major of the API or for ABI generations that
leave out the one the command writes, is refused in one line, before the library is built. The
providers its service file names are counted before any is constructed, so a jar naming none or two
runs nothing.

The command never holds a generator itself. Every call into one goes through one type
(`Bindings.Generator`), which tells a generator's failure apart from the command's by where it was
thrown: what a generator's code throws, an `IOException` among it, is said as that generator's
failure, in one line.

Each generator's `BindingGenerator#preflight` is asked before anything is built, and the directory
each binding goes to is checked: it has to be absent, empty, or a binding the same generator wrote,
which the one file `.souther-binding` in it says
([`BindingDirectory`](../compiler/src/main/java/souther/nativecode/BindingDirectory.java)). A
directory holding anything else is refused rather than deleted. A refusal here ends the command with
2, and nothing is built.

The library is built next, and the command reads the manifest the driver wrote beside it into the
model ([`ManifestReader`](../compiler/src/main/java/souther/nativecode/ManifestReader.java)). That
model and the C declarations the driver wrote are the
[`BindingInput`](../bindings/api/src/main/java/souther/bindings/BindingInput.java) every generator
is handed. Each generator writes into an empty directory of its own beside where its binding goes,
and only once every generator asked has written its own does the command put each in place, whole,
with its mark. So a generator that refuses or fails leaves every directory a binding was asked for
as it was, and no earlier binding of the same command is left half replaced. A failure of the file
system while the bindings are put in place is the one thing that can leave some in place and not
others, since several directories are never replaced as one. A refusal from
`BindingGenerator#generate`, or a generator's failure, ends the command with 1.

The mark is JSON, with a version, and is read strictly:

```json
{
  "format" : "souther-binding",
  "version" : 1,
  "generator" : "com.acme.kotlin",
  "artifact" : {
    "kind" : "maven",
    "coordinate" : "com.acme:souther-binding-kotlin:1.3.0",
    "sha256" : "..."
  }
}
```

What owns the directory is `"generator"`, the id the jar says, so a newer version of a generator
replaces what an older one wrote. `"artifact"` records which jar wrote it and decides nothing. A jar
read from a path is recorded by its digest alone (`"kind" : "local"`): the path is where one run
found it, and would carry a user's directories into the output.

## The jar a generator is

A generator is one self-contained jar. The command resolves no POM for it, and a library it needs it
carries inside the jar, shaded. It does not carry `souther-bindings-api`.

Its `META-INF/MANIFEST.MF` says which generator it is and what it is compatible with
([`BindingApi`](../bindings/api/src/main/java/souther/bindings/BindingApi.java)):

```text
Souther-Binding-Id: com.acme.kotlin
Souther-Binding-Api: 1
Souther-Abi-Generations: 9
```

`Souther-Binding-Id` is the generator, the same across its versions: lowercase letters, digits, `.`,
`_` and `-`, and none of the ids the catalog names (`php`, `rust`, `go`). `Souther-Binding-Api` is
the major of this API the generator was compiled against, which the command runs only where it is its
own. `Souther-Abi-Generations` is every ABI generation the code the generator writes, and the host
runtime that code calls, are built for, as integers separated by commas and no ranges; the command
runs a generator only where the generation it writes is among them.

The major is a promise: a command of major N runs every generator already compiled against major N,
without recompiling it, with the same meaning. Linking is not enough for that, since a generator
switching over every case of a sealed type still links once a case is added and no longer means what
it did. So a case added to a sealed type, a constant to an enum, a record's components changed, a
member's signature or declared nullness changed, or an abstract method of an interface nothing seals
added or removed, each moves the major. The surface each major promises is recorded in
[`bindings/api/generations/`](../bindings/api/generations/), and
[`WhatAMajorPromisesStillHoldsTest`](../bindings/api/src/test/java/souther/bindings/WhatAMajorPromisesStillHoldsTest.java)
fails when a line recorded for the current major is no longer true. It sees structure and declared
nullness; a method that keeps both and comes to answer something else moves the major too, and that
part is kept by whoever changes it.

The jar names exactly one implementation of `BindingGenerator` in
`META-INF/services/souther.bindings.BindingGenerator`, with a public constructor that takes nothing.

The command loads it behind a class loader of its own
([`GeneratorLoader`](../compiler/src/main/java/souther/nativecode/GeneratorLoader.java)), whose
parent is the JDK's platform loader. What a generator can resolve is the JDK, the package
`souther.bindings`, which is always the command's own copy, and what its own jar holds. The compiler
and the libraries it is written with are not in that loader's graph, so a generator that happened to
use one fails at once and not on the day the compiler changes it. The thread a generator is called on
has its loader as the context class loader, so a library inside the jar that looks up services finds
the jar's. An annotation class the loader cannot see, such as JSpecify's, does not stop a class from
loading; reading such annotations reflectively is not part of what a generator may count on.

## The interface a generator implements

[`BindingGenerator`](../bindings/api/src/main/java/souther/bindings/BindingGenerator.java) has two
methods. `BindingGenerator#preflight` is handed the options and refuses, with
[`NotBindable`](../bindings/api/src/main/java/souther/bindings/NotBindable.java), what would be
refused whatever the model says: a key it does not take, an option missing, or a value the language
will not take, such as a namespace PHP will not take or a crate name Cargo will not take.
`BindingGenerator#generate` is handed the input, the empty directory and the options, and writes the
binding; it refuses with `NotBindable` what only the model can say, a name in it the language will
not take. The two moments are apart because the second needs a library the first is meant to spare
building. Which generator it is, the jar says, and not the code.

A generator writes files into the directory it is handed and does nothing else to the file system.
Where the binding goes, what was there before and putting it in place are the command's. The input
is shared by every generator the command asks, and every part of it is immutable, so a generator
changes nothing it is handed. Anything a generator throws other than `NotBindable` is its own
failure, which the command reports as that; a generator does not need to catch its own mistakes to
be polite about them.

A generator depends on `souther-bindings-api` and on nothing else of this project. What the command
is, what a checked program is and how the manifest is written are all out of its reach. A test holds
every member of what the API offers to being public or private and nothing between
([`WhatAGeneratorReachesIsPublicTest`](../bindings/api/src/test/java/souther/bindings/WhatAGeneratorReachesIsPublicTest.java)).

## The model

`BindingInput` holds the [`Manifest`](../bindings/api/src/main/java/souther/bindings/Manifest.java)
and the [`Declarations`](../bindings/api/src/main/java/souther/bindings/Declarations.java). The
declarations are the C the driver wrote for an FFI to read (`souther.ffi.h`), which a binding may
carry beside itself and load the library through: a generator can copy them (`Declarations.copyTo`)
and can do nothing else with them, since what they say about the ABI is the driver's answer, and a
generator that read it would be giving a second one. The PHP and the Go bindings copy them; the Rust
binding needs none.

A `Manifest` is what a library offers a host and asks of one, for each `Manifest.Module`. A module
has its behaviors, the values it publishes, its declarations (products, newtypes, units and sums,
with their fields and cases), the behaviors a host implements (`Manifest.Injection`), what a host
builds the capabilities a behavior is called with out of (`Manifest.Construction`), and the
functions a list or a function value of each shape is reached through (`Manifest.ListCrossing`,
`Manifest.FunctionCrossing`). The manifest also names the statuses and a reading's outcomes by
number, and the runtime's functions for each case a union may carry (`Manifest.CaseCrossing`). It is
made only by `Manifest.of`, which holds it to what no part can hold alone: a module says the
functions for every list and function value it hands across, each the way it crosses, and what
constructing a behavior requires is closed over the whole.

Everything a host reaches is a `Manifest.Reach`: `Manifest.Reach.Available` with what reaches it, or
`Manifest.Reach.Unavailable`, where the model has the thing and a host has no way to it. A generator
writes nothing for what is unavailable, and says nothing where it is missing.

A value that crosses, wherever it crosses, is a
[`ValueCrossing`](../bindings/api/src/main/java/souther/bindings/ValueCrossing.java): what a
behavior, a published value or an implemented behavior takes and answers (`Manifest.Crossings`),
what a constructor takes, a field read, a list's element, and what a function value takes and
answers. It is the value's type and the shape it crosses in taken apart together, once, by the
command, and a generator folds it and never pairs a type with a shape itself. The tree is closed.
Its leaves are a `ValueCrossing.Primitive`, a declared type's `ValueCrossing.Handle` and a
`ValueCrossing.Union`, each crossing as one `Manifest.Word`; the rest are a
`ValueCrossing.Optional`, a `ValueCrossing.Tuple`, a `ValueCrossing.Listed` list, set or map, and a
`ValueCrossing.FunctionValue`. Each record is held to agreeing where it is made, so one that says a
type crosses in a shape it cannot is one nobody can make. A leaf keeps the pair of its type and its
word and not a rule of which word a type crosses as, since that is the driver's to choose: a
generator asks of the pair whether its language can hold it, and does not bind a leaf it has no way
to hold. A `ValueCrossing.Listed` and a `ValueCrossing.FunctionValue` say whether the module offers
a way for one to cross each way (`ValueCrossing.Listed#crosses(Way)`,
`ValueCrossing.FunctionValue#crosses(Way)`): built where a host hands one over and read where it is
handed one, called where a host is handed one and made where it hands one over.

A `Manifest.Word` says what it is on the machine (`Manifest.Word#representation()`), as the ABI
generation records it, and a generator writes the type its language holds a word in from that and
from nothing else. Every `Manifest.Function` in the model is a symbol the library defines, with the
words it takes (given, room, or a slice of as many as a count says) and the word it answers.

Names a generator writes are claimed in the scope its language reads them in
([`Claimed`](../bindings/api/src/main/java/souther/bindings/Claimed.java)), so two things given one
name are refused with both named, rather than written as two a compiler refuses. A name the model
gives that the language will not take is refused, with the name, rather than spelt some other way. A
name the generator makes for what it adds beside the model is never a reason to refuse: it is made
another way.

## The runtime a binding calls

What the written code calls, the runtime calls in the library, and it follows the host ABI: it asks
`souther_abi_generation` before any other symbol and refuses a library of another generation; it
brackets what it does in scopes and closes them innermost first; it keeps the handles the library
answers where its host cannot forge them; it lays out the storage a behavior or a function value of
the host's own needs; and it loads the library by path rather than linking it. All of it is
[host-abi.md](host-abi.md).

The runtime's own surface, what the written code may call of it, is a protocol between the generator
and the runtime, apart from the library's ABI generation. The PHP runtime records its surface under
[`bindings/php/generator/src/test/resources/souther/bindings/php-runtime-surface/`](../bindings/php/generator/src/test/resources/souther/bindings/php-runtime-surface/)
and the Go runtime under [`bindings/go/runtime/protocol/`](../bindings/go/runtime/protocol/), and a
test fails where the surface is not what the record says. Written code says which protocol it was
written for: a PHP binding refuses to load over a runtime of another, and a Go package does not
compile against one.

How each of the three runtimes holds values, runs and failures in its language is its own
documentation: [PHP](../bindings/php/runtime/README.md), [Rust](../bindings/rust/runtime/README.md)
and [Go](../bindings/go/runtime/README.md).

## Testing a generator

`souther-bindings-testkit` builds a library from Souther source with the compiler and the driver of
the release it belongs to, and hands over what the command would hand a generator
([`SoutherBindingTest`](../bindings/testkit/src/main/java/souther/bindings/testkit/SoutherBindingTest.java)).
It writes where the test tells it, and keeps no directory of its own:

```java
@Test
void theBindingIsWritten(@TempDir Path into) throws Exception {
    TestLibrary library = SoutherBindingTest.compile(into.resolve("native"), """
            module shop exposing ( total )
            ...
            """);
    Path binding = Files.createDirectories(into.resolve("binding"));

    new KotlinBindingGenerator().generate(library.bindingInput(), binding, Map.of("package", "shop"));

    // compile and run what was written with the host's own toolchain, loading library.library()
}
```

A released testkit fetches the driver of its release for the platform it runs on, and holds it to the
checksum that release's compiler carries, as the command does; an author needs no driver of their own
and sets no property. `compile` takes several sources, one module each.

The standard generators' tests build their libraries through the testkit too, and then compile and
run what the generator wrote with the host's own toolchain: a PHP linter and PHP, Cargo, and the Go
toolchain with cgo. In this repository the testkit is not a release, and is handed the driver the
same build made. A few of those tests need what no Souther source writes, a manifest changed by hand
or a document the compiler does not write yet, and build with the compiler's own test support.

That the testkit is enough on its own is held by a project outside the reactor,
[`bindings/testkit/acceptance`](../bindings/testkit/acceptance/), which depends on the API and the
testkit and on nothing else of this project. [`scripts/testkit-acceptance.sh`](../scripts/testkit-acceptance.sh)
runs its test against a publication before it is published: the release runs it on what it is about
to release, so a testkit that cannot build a library outside this repository is never released. A
change to the testkit, or to what it depends on, can be rehearsed before then on one's own machine
with [`scripts/testkit-rehearsal.sh`](../scripts/testkit-rehearsal.sh), which rehearses a release
for the platform it runs on; the build does not run it, since what it alone reaches is rarely broken
and the release stops before publishing where it is.
