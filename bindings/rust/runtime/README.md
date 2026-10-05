# souther-binding-runtime

What every Rust binding of a Souther library built by souther-native-compiler runs on: a run's arena,
the handles a value is held as, and calls into the library and back. An application does not
depend on this crate by itself. It depends on the crate `souther compile --target native --rust` wrote for its
model, which depends on this one and re-exports what a host names (`Construction`, `Reading`,
`Failure`, `Decimal`, the temporal types and `raoh`).

## Building a library and its Rust binding

The command writes the library and the Rust binding of it in one build, the binding from the
manifest the library was written with:

    souther compile --target native --library build/native --rust build/rust --crate acme model

`--crate` names the crate the binding is, and `--rust` the directory it is written to. That
directory is the binding of one manifest and nothing else: the command writes a binding beside it
and puts it in place whole, so a type the model no longer declares is not left from the build
before. It replaces only a directory a Rust binding was written to, which the file
`.souther-binding` in it says, and refuses one holding anything else rather than deleting it.

A Rust host depends on the crate `--rust` wrote by path, and that crate requires this one from
crates.io, so the host names nothing else:

    [dependencies]
    acme = { path = "build/rust" }

Work on this crate itself is tried against a host by patching it in from a clone of
souther-native-compiler (`[patch.crates-io]`, naming `<clone>/bindings/rust/runtime`), which the
repository's own tests of a generated binding do.

The generated crate asks for Rust 1.88 or newer, as this one does, since Raoh, which both depend
on, asks for that. The library is loaded by path when the host runs, not linked.
[`scripts/rust-from-the-command-line.sh`](../../../scripts/rust-from-the-command-line.sh) is the
whole of it as an application outside the repository's tests does it: a two-file model built into a
library and its binding by the command line, and a Rust program calling the library through it. How
a binding is generated from the manifest, for Rust and for the other hosts, is in
[`docs/writing-a-binding.md`](../../../docs/writing-a-binding.md).

## What the binding writes

A module of the model is a Rust module (`cart.lines` is `cart::lines`), and every name is the
model's, a keyword written raw (`r#type`); the crate allows the lints that would ask for Rust's own
case. Nothing is linked and no `build.rs` is written: `Library::load(path)` loads the library by
path and looks each function up through that handle, into a table of typed function pointers
written from the manifest. Every Souther library exports the same runtime functions, so a link could
not say which of two a call reaches, and each is reached through its own handle instead. Loading is
`unsafe`, since a library built from another program may export a function of the same name that is
something else, and it is the one `unsafe` a host writes.

Before it looks up anything else, `Library::load` asks the library which ABI generation it was built
as (`souther_abi_generation`), and refuses one of another generation, or one with no such function,
as `LoadError::Generation`, before a function of it is called as what it is not. A library with no
function the binding calls is refused as `LoadError::Missing`, and one the loader cannot open as
`LoadError::Open`. The functions of this crate's own that it calls, for text, a `Decimal`, a temporal
and what a reading came to, are one table held by this crate's tests to what the generation records
of each, so the ABI it calls is the one it says it calls. What the library exports, function by
function, is in [`docs/host-abi.md`](../../../docs/host-abi.md).

    use acme::cart::lines::{Line, total};
    use acme::cart::money::Money;
    use acme::{Construction, Library};

    // SAFETY: the library the binding was generated from.
    let library = unsafe { Library::load(&path) }.expect("the library loads");
    let said = library
        .run(|run| {
            let price = Money::new(run, 3).unwrap().into_result().unwrap();
            let line = Line::new(run, price, 4).unwrap().into_result().unwrap();
            let none = match Line::new(run, price, 0).unwrap() {
                Construction::Value(_) => "built".to_owned(),
                Construction::Rejected(issue) => issue.code().to_owned(),
            };
            format!("total {}, none {none}", total(run, line).unwrap())
        })
        .unwrap();

## What the types check of a run

What the PHP runtime checks while a host runs, the Rust types check when it is built.
`library.run(|run| ...)` opens a root run whose lifetime is its closure's own, so no value made in it
is answered out of it. A value is a handle of that lifetime (`Line<'run>`), neither `Send` nor
`Sync`, and everything that makes something in the arena takes the run mutably: a constructor,
`decode`, a behavior, a published value, calling a function value. A field's reader, `case` and
`encode` take only the value, since what they answer the value already held. `run.scope(|inner| ...)`
opens a run inside it and borrows it until that one ends, so nothing is made through the outer run
meanwhile. The `Scope` a closure is handed says the run outside outlives it, so a value made outside
is handed to a computation inside, and one made inside cannot be kept outside. This crate's tests
hold each of these to what rustc accepts and refuses, with `trybuild`, and not to what the types'
definitions say they mean.

A run is a scope of the library's arena, opened with `souther_scope_open` and closed with
`souther_scope_close` when its closure has answered or panicked, which drops everything made in it.
The library closes a scope only where it is the innermost one open on the calling thread, and the
borrows above are what keep every run this crate closes the innermost; where the library refuses one
anyway, that is this crate and the library disagreeing, and it panics rather than going on.

What the types cannot see is which library a value is of: a lifetime says for how long a value is
good and not which arena it stands in, and a root run of one library opened inside a root run of
another relates the two by lifetimes as a run and a run inside it are related. So every handle holds
the library that made it, its address is reached only through what checks that the run a
computation is started in is of the same runtime, and one another library made is refused before
the call as `Failure::Foreign`; so is a behavior bound, at any depth, to what another library made. A
library is told apart by the address of its `souther_scope_open`, which whatever works on one arena
shares, so two `Library` values over one file are one runtime, and a second root run of it on a
thread with one open is refused where it is opened: `library.run` answers `Err(AlreadyRunning)`, and
a run inside it is opened from it with `scope`.

## How a value is held

A product, a newtype and a unit are each a `Copy` handle, whose native value is not public either,
with a reader for each field. `new` takes the run and the fields and answers a `Construction` (the
value, or an `invariant_violation` Raoh issue where what is handed over does not hold what the type
states), and `decode` takes the run and the JSON text and answers a `Reading` (the value, or Raoh's
issues, or `invalid_format` where the text is not JSON); each answers a `Failure` outside that where
the call ended for another reason. `encode` writes the value in its external form, and `decoder` is
`decode` as a raoh decoder of a `serde_json::Value`, which a host composes with its own the way a JVM
host composes a type's `decoder()`. It reads in the run a `Decoding` lends the decoders of one decode
(`Decoding::read`), which also makes a constructor into a decoder (`decoding.of`), and what the
library finds wrong is an issue at the path it is reached at. A raoh decoder answers issues or a
value and nothing else, so a step that ends the run for a reason of its own answers no issue, and the
failure is what `Decoding::read` comes to.

A sum is a handle too, with `decode`, `decoder`, `encode` and `case`, which answers an enum of its
cases, a case the model keeps being `Kept`, and `From` each of its cases and each narrower sum.
Every case of a sum is a type the model declares.

An `Int`, a `Bool` and a `String` are Rust's own. A `String` is handed over as its UTF-8 bytes,
which is all the library asks of it, and the library puts it in NFC; one whose canonical form is
longer than a `String` holds is refused, and the call comes to `Failure::Abort` named
`REQUIRED_FORM_HAS_NO_PLACE`. A `Decimal` is this crate's integer and scale, made by `Decimal::new`
of the integer's digits or `Decimal::of` of an `i128`, and equal, ordered and hashed by amount as
Souther compares two (`1.5 == 1.50`) while it keeps the scale it was written with. A `Date`, a
`Time`, a `DateTime` and an `Instant` are this crate's types, held as their numbers: a date's year,
month and day, a time's hour, minute and second, and an instant's second from the epoch and the
nanosecond within it. They cross to the library as those numbers, not as text, and the library says
whether the numbers it is handed name a value. Each is still checked where a Rust program makes one
(`Date::new`, `Time::new` and `Instant::new` answer `NotATemporal`, and `Decimal::new` answers
`NotADecimal`), so a value a program holds is always one, and the library refusing one it is handed
is this crate and the library disagreeing, which comes back as `Failure::ProtocolViolation`. The text
`java.time` writes for each (`iso()`, and `Display`) is for a program to show, and never crosses.

An optional is an `Option` at every depth, a tuple a Rust tuple, a list a slice handed over and a
`Vec` handed back. A list is built by the library of the slice's elements, and where the library
answers that it could not build it, the call comes to `Failure::ProtocolViolation` rather than
handing over a list that is not one. A union no declaration names is an enum with a variant for each
member, named after them in the manifest's order (`FreeOrInt`), handed over by reference and handed
back where the library says its case, as a behavior's answer.

A function value is a type of its own (`FnIntToInt`), made by the library or by `FnIntToInt::host`
of a `'static` Rust closure, which is handed the run and what the function takes and answers a
`Result` of what it answers or a `HostError`; the library calls it through an entry the crate
writes. What the library made is held where only the crate reaches it: the address says nothing of
which function it is, and one put under another function type would run the code of the one with
the words of the other, so what a caller sees is a type it cannot take apart and put together
otherwise. Both are called with `call`, and both are handed over; a host's function is kept, with
the room it is made into, by the run it is handed over in until that run ends, and one handed over
again in that run is the value it was made into before.

## Behaviors and what a host implements

A behavior that requires nothing is a function of its module. A behavior is also a type named after
it: `bind` takes a reference to what stands for each behavior it requires, in order, so a missing one
does not compile, and `call` calls it in the caller's run. A behavior a host implements is a trait
with an `apply` typed as the model says, and `<Behavior>Implementation::new(&library,
implementation)` makes one into what stands for it. The library calls `apply` in the run of the call
that reached it, lent that run rather than opening one, so what it answers is made where the
caller's run holds it. A failure it answers comes back out of that call as `Failure::Host`, and a
panic is caught before it reaches the library and raised again where the call returns. Where what
`apply` answered cannot be handed back to the library, a `String` with no place as one, say, the
call comes to that failure itself, as a direct call handing the same value over would, and not to a
`Failure::Host` of it. What Rust has no way to hold is not written: a behavior the manifest says a
host cannot call, a field with no reader, a type this binding has no representation for.

## A whole application

[`examples/rust-cart`](../../../examples/rust-cart) is the cart example as a Rust application: the
model every host's cart example runs, served by axum, its HTTP boundary decoded by raoh, and its
injected behaviors implemented over SQLite and bound, for each request, to the transaction the
request runs in. Besides the HTTP contract, its tests hold what rustc refuses a host of the model.
Its README says how to build and run it.
