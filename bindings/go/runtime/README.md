# The Go binding of a Souther library

This module, `github.com/souther-lang/souther-native-compiler/bindings/go/runtime`, is what every Go
binding of a Souther library runs on. A binding is written by `souther compile --target native --go`,
from the manifest the library was built with, as Go packages that call the library through this module. A
host imports the binding's packages and never calls this module itself, except for the types the
binding hands it (`souther.Option`, the tuples, the temporals) and the errors it answers. How a
binding is generated is in [Writing a binding](../../../docs/writing-a-binding.md), and the C ABI
the library exports is in [The host ABI](../../../docs/host-abi.md).

## Writing a binding and requiring it

The command writes the library and its Go binding in one run, with `--package` naming the import
path of the binding:

    souther compile --target native --library build/native --go build/go \
        --package example.com/acme model

A module of the model is a package of its own under that path (`cart.lines` is
`example.com/acme/cart/lines`), which the module graph allows, since modules do not depend on one
another in a cycle. The directory is one Go module, with a `go.mod` requiring this runtime and Raoh,
and what is written is what `gofmt` writes. The import path has a `.` in its first part, as Go asks
of a module that another module requires.

A host requires the binding by path, with a `replace` to where it was written, and runs `go mod
tidy` once, since the binding carries no `go.sum`:

    require example.com/acme v0.0.0
    replace example.com/acme => ./build/go

The binding requires this module at the version the runtime says it is, in
[`VERSION`](VERSION). That is the runtime's own version and not the compiler's: a compiler release
that leaves the runtime as it was requires the version already published. It is published as the
tag `bindings/go/runtime/v<version>`, since a module in a directory of a repository is versioned by
a tag that begins with that directory, so a host needs nothing more for it. A version that is not
published yet is one only a clone of this repository has, and a host reaches it by a `replace`:

    replace github.com/souther-lang/souther-native-compiler/bindings/go/runtime v<version> => <clone>/bindings/go/runtime

The library is loaded by path when the host runs, and never linked. cgo builds the binding's
packages, so a C compiler is needed. A host that builds `cart.lines` above and calls it looks like
this, and [`scripts/go-from-the-command-line.sh`](../../../scripts/go-from-the-command-line.sh)
does all of it in CI:

```go
library, err := acme.Load("build/native/libsouther.so")
if err != nil {
	return err
}
err = library.Run(func(r *acme.Run) error {
	price, err := money.NewMoney(r, 3)
	if err != nil {
		return err
	}
	line, err := lines.NewLine(r, price, 4)
	if err != nil {
		return err
	}
	total, err := lines.Total(r, line)
	if err != nil {
		return err
	}
	fmt.Println(total)
	return nil
})
```

## Loading a library

`Load(path)`, in the binding's root package, opens the library with `dlopen` and looks each function
up through that handle. Every Souther library exports the same runtime functions, so a link could
not say which of two a call reaches; each file is opened `RTLD_LOCAL`, and two libraries in one
program each keep their own arena.

`Load` first asks the file which ABI generation it answers to, by `uint32_t
souther_abi_generation(void)`, which stands outside the generations and is the same in every one
from 9 on, and refuses a library of
another generation than the one this runtime calls (`souther.ABIGeneration`) with a
`*souther.UnsupportedGeneration`. A library of generation 8 or earlier has no such function, and is
refused the same way, with `Found` nought. `Load` then looks up every function it will call: the runtime functions this
module calls, which it lists and looks up itself, and every function the binding calls, which the
binding lists. One that is missing is a `*souther.MissingSymbols`, so a file of another library is
refused at load and not where a call reaches it. That the file is the library the binding was
generated from is still the caller's to hold, as it is for `Library::load` in Rust: `Load` cannot
tell a library from another of the same generation that has the same functions under the same
names with other signatures. A library that loaded stays loaded for as long as the program runs,
since nothing says when the last value or function pointer taken from it is gone; one that failed
to load is unloaded.

A call goes through a small C function that takes the function's address, since cgo cannot call
one. Each is written as the type the manifest's words make of the function, and the C compiler
asserts (`_Static_assert` over `__builtin_types_compatible_p` and `__typeof__`) that it is the type
`souther.ffi.h`, copied beside the binding, declares. A number of another width or an address of
another type is a build that fails, and not a call that converts one into the other. The same holds
for each function Go exports for the library to call back. The calls this module makes into a
library's runtime are held by a test to what the generation's record in
[`native/crates/abi/generations`](../../../native/crates/abi/generations) says.

What Go exports is one name in the whole program, so it is an injective encoding of the binding's
import path, the module and the behavior, and never a spelling of them with what Go does not take
in a name replaced: two bindings in one program, and two modules whose names end alike, link
together.

Go says none of the C ABI itself. What a host lays out room for (a capability, what a host's
implementation is read out of, a function value of the host's, an array of the capabilities a
behavior requires) is the C compiler's `sizeof` of what `souther.ffi.h` declares, handed to this
module as a `souther.Layout`. The header declares each of these as opaque slots of `uint64_t`
(`souther_capability`, `souther_hosted`, `souther_hosted_function`), which gives the size and the
alignment and no field a host could read or write. The room is taken from C memory, since the
library keeps it for as long as it may be called and cgo does not let C keep a Go pointer, and it is
freed when the run it was made in ends.

## Runs and scopes

A library keeps what a computation makes in an arena of its own, one for each OS thread, and a run
is a scope of that arena. `library.Run(func(r *Run) error { ... })` opens a scope with
`souther_scope_open`, hands the run to the function, and closes the scope with `souther_scope_close`
when the function returns or panics, dropping everything made in it. A value made in a run is an
address into the arena, good until the run ends.

Go moves a goroutine between threads, so a run holds its goroutine on one OS thread
(`runtime.LockOSThread`) from the scope's opening to its closing, and a callback from the library
comes back on that thread, since cgo runs it where it was called from. `r.Scope(func(inner *Run)
error { ... })` opens a run inside a run. The library closes a scope only where it is the innermost
one open on the calling thread, and runs on one locked thread nest, so this module always closes
the innermost; the library refusing it is this module and the library disagreeing, and panics.

What Rust holds in types, Go checks when a value is used. A run and its values are of the binding's
own tag type, so a run of another generated binding is another type and does not compile.

- A value used after the run it was made in ended, a run or a value from another goroutine, and
  something made through a run that has a run inside it are what Rust refuses to compile. They
  panic with a `*souther.Misuse` (`ErrExpired`, `ErrRunOnAnotherGoroutine`,
  `ErrNotTheInnermostRun`), as does the zero value of a handle, which holds no value
  (`ErrNoValue`). Being on the run's thread is being in its goroutine, since no other goroutine
  runs on a locked thread.
- A value another runtime made, or a behavior bound to one at any depth, and a second root run of
  one runtime on one thread, are what Rust also reports at run time. They are errors
  (`ErrForeignHandle`, `ErrAlreadyRunning`). A library is told apart by the address of its
  `souther_scope_open`, so two `Library` values over one file are one runtime.
- Every function of the binding that takes a run asks it first (`souther.Making(r)`), whatever it
  goes on to do: calling a function of the host's own that touches no run is asked the same as
  calling the library's.
- Reading a value and copying what a call answered leave no arena-owned value with the caller, and
  need only the first two. Whatever makes a value needs the run to be the innermost, so a value made
  outside is read and handed to a computation inside `r.Scope`, and one made inside cannot be used
  once it has ended.

## How the model's types are held

A product, a newtype and a unit are each a struct holding the value and the run it was made in,
with a method for each field. `New<Type>` answers the value, or an `invariant_violation` Raoh issue
as an error where what is handed over does not hold what the type states. `Decode<Type>` answers
the value read out of its external form, or Raoh's issues (`*raoh.Issues`), or `invalid_format`
where the text is not JSON. `Encode` answers the value's external form. `<Type>Decoder(r)` is
`Decode<Type>` as a raoh-go decoder of what a host decoded, reading in `r`, which a host composes
with its own decoders the way a JVM host composes a type's `decoder()`: the value it is handed is
written back as the JSON it is, in the order of an object's members, and what the library finds
wrong is an issue at the path it is reached at. A member that is not there is raoh's to report, as
for any field. A decoder holds `r`, so it is made for one decode inside the run and not kept past
it.

A sum has `Case`, answering the value as the type of its case (`Owed`), which is marked as one of
the sum's cases; a case the model keeps is `<Sum>Kept`, and there is a `<Sum>From<Type>` for each
case and each narrower sum. Every case of a sum is a type the model declares; one declared in
another package is held by a type of the sum's own (`<Sum><Case>`), since Go lets a package write a
method only on its own types.

An `Int`, a `Bool` and a `String` are Go's own. A `String` handed to the library has to be UTF-8,
and one that is not is `souther.ErrNotUTF8` before anything is called; the library puts text in NFC,
and text whose canonical form is longer than a `String` holds is an `*souther.Abort` of
`REQUIRED_FORM_HAS_NO_PLACE`. A `Decimal` is Raoh's `raoh.Decimal`, with the scale it was written
with: it crosses as its unscaled digits and its scale, and comes back with the scale the library
has, so `1.50` and `1.5` are equal by `Cmp` and not as Go values. Every `raoh.Decimal` names a
Decimal, so a library that answers it could not make one disagrees with this module, and handing
the value over is `souther.ErrProtocolViolation`.

A `Date`, a `Time`, a `DateTime` and an `Instant` are this module's own types, held as the numbers
they mean and handed to the library as those numbers. Each is checked where it is made: `NewDate`,
`NewTime` and `NewInstant` answer `souther.ErrNotTemporal` for numbers that name no value of the
type, such as a day the calendar does not have or a year past what a `Date` holds, and `NewDateTime`
is a `Date` and a `Time` that are both values already. The library decides the same again where it
is handed one, and answers whether the numbers name a value rather than ending the process, so a
value a Go program has is always one, and a refusal from the library is reported as
`souther.ErrProtocolViolation`, a disagreement between the library and this module. Each type's
`String` writes the text `java.time` writes for it, for a Go program to show; that text never
crosses to the library. They are not `time.Time`, since none of them is what one is: a `Date` and
a `Time` have no zone and no instant, and an `Instant` reaches a billion years either way.
`InstantOf` and `Instant.Time` convert.

An optional is a `souther.Option`, at every depth, since a pointer cannot tell an optional of
nothing from nothing. A tuple is a `souther.Tuple2` and its like up to eight members. A list, a set
and a map are slices, a map's entries being tuples in the order the library has them, which the
language says nothing of.

A union no declaration names is an interface named after its members in the manifest's order
(`FreeOrInt`), handed over as one of them and handed back where the library says which case it is.
A member declared in the union's own package is a value of it as it is (`Free`); any other, a
primitive or a type of another package, is held by a type of the union's (`FreeOrIntInt{Value:
n}`). A union, like a function type, belongs to the module that says it: two modules that say one
alike each have their own type, so that a package imports only what its module depends on.

Every interface a union or a sum's cases is ends its doc comment with `//sumtype:decl`, so
[go-check-sumtype](https://github.com/alecthomas/go-check-sumtype), or golangci-lint's
`gochecksumtype`, fails a type switch over it that leaves a case out. Go does not, and the model's
cases are closed, so a host that runs the check learns of a case added to the model at every switch
that does not answer it.

A function value is a type of its own (`FnIntToInt`), made by the library or by `HostFnIntToInt` of
a Go function, and called with `Call(r, ...)`, since a call makes values in the innermost run. A
function of the host's handed over again and again in one run is one function value.

## Behaviors and a host's implementations

A behavior that requires nothing is a function of its package. A behavior that requires something,
or is required, is also a type with `Bind<Name>`, taking what stands for each behavior it requires,
in order, and `Call`. A behavior a host implements is an interface with an `Apply` typed as the
model says, and `Implement<Name>(r, implementation)` makes an implementation into what stands for
it. What stands for a behavior is laid out in C memory and held until the run it was made in ends.

The library calls `Apply` in the run of the call that reached it. An error `Apply` returns comes
back out of that call as a `*souther.HostError`, which unwraps to the error, except where the
generated code met it while handing over what `Apply` answered (text that is not UTF-8, a value
another runtime made), which comes back as itself; a panic is caught
before it reaches the library and raised again where the call returns. A behavior reached through a
requirement it was handed nothing for is `souther.ErrUnbound`, and a computation the library ended
without an answer for a reason of its own is a `*souther.Abort` carrying the status and the name
the manifest gives it.

## Names

Every name is the model's with its first letter a capital, which is what Go exports. A name whose
first letter has no capital (a digit, an underscore, a letter of a script without case) is refused,
and two names that come to one are refused with both named. A parameter or a field is named as the
model names it, with an underscore after it where the name means something in Go already (a word
of the language, an identifier Go declares, what a signature calls the run and a receiver, what a
file calls an import); a caller reads a function by its types and not by these names. What a
generated function writes of its own yields to the model's names and is never refused against them.

Names ending in `__` (`Ref__`, `Word__`) are the binding's own, which one package hands another. Go
has no way to keep them from a caller, as Rust does, and a host does not use them.

## What a binding leaves out

What a host has no way to reach is not written, as in the PHP and the Rust bindings, and nothing
says so where it is missing: a behavior, a value or a field the manifest says nothing reaches; a
type Go has no way to hold here (a tuple of more than eight members, a list, a set or a map the
library gives no way to build where it is handed over or to read where it is handed back, a union
with a member the language gives, such as `NotADate`); and a type or a function a name of the
binding's own would have to be, where that name is another's in the package.

## The protocol and the ABI generation

What a generated package may call of this module is its protocol (`souther.Protocol`, now 3). The
surface of each protocol is recorded under [`protocol`](protocol), a test fails where the surface
is not what the record says, and a generated package does not compile against another protocol
than the one it was written for. A change to what a generated package may call is a new protocol,
not an edit of one.

That is separate from the library's ABI generation. This module calls a library of one generation
(`souther.ABIGeneration`) and asks each library for its generation before anything else, as above.
A function generated for a behavior or a type also carries the generation in its name
(`souther11_m_shop_b_quote`), so a binding generated against another generation finds none of its
functions.

## Platforms and versions

This module is built for Unix only (`//go:build unix`), since it loads a library with `dlopen`, and
this repository builds and tests it on Linux and macOS. Windows is not written yet
(souther-native-compiler#96).

The modules a binding writes need the Go this module's `go.mod` names, which Raoh's needs are part
of. This module and a generated `go.mod` require one release of
[raoh-go](https://github.com/raoh-project/raoh-go), which a test holds to one.

## The cart example

[`examples/go-cart`](../../../examples/go-cart) is the cart example as a Go application:
`examples/cart-model` served by `net/http`, its HTTP boundary decoded by raoh-go with the model's
constructors and decoders as steps of the same decoders, and its injected behaviors implemented
over SQLite, each row written by a raoh encoder in the form the model reads. Besides the HTTP
contract, its tests hold what the Go runtime refuses a host of the model where Rust's types do.
`scripts/go-cart-example.sh` builds it and runs its tests in CI.
