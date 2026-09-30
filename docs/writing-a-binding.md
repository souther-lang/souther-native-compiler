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

Today the command runs only the generators its catalog names
([`KnownBindings`](../compiler/src/main/java/souther/nativecode/KnownBindings.java)), installed with
it or fetched from Maven Central at its own version. Nothing here depends on that: a generator is
held to this interface whoever wrote it.

## How the command runs a generator

The command reads its whole command line before any generator is found. Which bindings there are,
the flag that asks for each and the options each takes are its catalog's, and nothing else's, since
a generator that is not installed yet cannot say what it takes. A generator is asked for by the id
the catalog names it by, and handed the options the catalog names for it, keyed by the option's
name without its dashes.

Then, for each binding asked for, the command finds its generator: one installed with the command,
through `ServiceLoader`, or, where there is none, the jar the catalog names, fetched and held to the
checksum the compiler was released with. It never holds a generator itself. Every call into one goes
through one type
([`Bindings.Generator`](../compiler/src/main/java/souther/nativecode/Bindings.java)), which tells a
generator's failure apart from the command's by where it was thrown: what a generator's code throws,
an `IOException` among it, is said as that generator's failure, in one line.

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

## The interface a generator implements

[`BindingGenerator`](../bindings/api/src/main/java/souther/bindings/BindingGenerator.java) has three
methods. `BindingGenerator#id` answers the id the catalog names the generator by.
`BindingGenerator#preflight` is handed the options and refuses, with
[`NotBindable`](../bindings/api/src/main/java/souther/bindings/NotBindable.java), what would be
refused whatever the model says: an option missing, or a value the language will not take, such as a
namespace PHP will not take or a crate name Cargo will not take. `BindingGenerator#generate` is
handed the input, the empty directory and the options, and writes the binding; it refuses with
`NotBindable` what only the model can say, a name in it the language will not take. The two moments
are apart because the second needs a library the first is meant to spare building.

A generator writes files into the directory it is handed and does nothing else to the file system.
Where the binding goes, what was there before and putting it in place are the command's. The input
is shared by every generator the command asks, and every part of it is immutable, so a generator
changes nothing it is handed. Anything a generator throws other than `NotBindable` is its own
failure, which the command reports as that; a generator does not need to catch its own mistakes to
be polite about them.

A generator is found through `ServiceLoader`, so its jar names its class in
`META-INF/services/souther.bindings.BindingGenerator`. It depends on `souther-bindings-api` and on
nothing else of this project, and the standard generators import nothing outside `souther.bindings`.
What the command is, what a checked program is and how the manifest is written are all out of its
reach. A test holds every member of what the API offers to being public or private and nothing
between
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

The standard generators' tests build a library from Souther source with the compiler, hand what it
wrote to the generator, and then compile and run what the generator wrote with the host's own
toolchain: a PHP linter and PHP, Cargo, and the Go toolchain with cgo. They use the compiler's test
support and the driver of a checkout, found through `-Dsouther.native.driver`, which a generator
outside this repository does not have yet.
