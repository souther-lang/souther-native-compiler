# souther-lang/php-runtime

This package is what every PHP binding of a Souther library built by souther-native-compiler runs
on. A binding is PHP generated from the library's manifest, and it calls the shared library through
PHP's FFI; this package loads the library, keeps each run's scope of its arena, holds every value to
the run it belongs to, and says what each failure is. It needs PHP 8.2 or later with `ext-ffi`, and
[raoh-php](https://github.com/kawasima/raoh-php), whose `Result`, `Decoder` and issues a binding
answers in. How the library itself is called, function by function, is in
[the host ABI](../../../docs/host-abi.md), and how a binding is generated from the manifest is in
[writing a binding](../../../docs/writing-a-binding.md).

## Generating a binding and reaching the runtime

A binding is written by the same command that builds the library, and the namespace it is written
under is the caller's to name:

    souther-native --library build/native --php build/php --namespace Acme\Shop model

`--library` writes the shared library, its C declarations and its manifest into `build/native`, and
`--php` writes the binding into `build/php`, with a copy of those declarations as `souther.ffi.h`
beside it. The [top-level README](../../../README.md) says how to run the command, through jbang or
from a clone. A namespace PHP will not take is refused before the library is built.

Until the runtime is published, an application reaches it as a Composer path repository, which is
the supported way for now. The runtime asks for a development version of raoh-php, which Composer
takes only where the application asks for it too:

    {
        "repositories": [
            { "type": "path", "url": "<clone>/bindings/php/runtime" }
        ],
        "require": {
            "raoh/raoh": "0.8.x-dev",
            "souther-lang/php-runtime": "@dev"
        },
        "autoload": { "psr-4": { "Acme\\Shop\\": "build/php/" } }
    }

The binding is mapped by the application like its own classes, or loaded with the `autoload.php`
written beside it. The application then loads the library, `libsouther.so` or `libsouther.dylib`
in the `--library` directory, and makes its calls inside a run:

    $binding = \Acme\Shop\Binding::load(__DIR__ . '/build/native/libsouther.so');
    echo $binding->run(fn () => \Acme\Shop\Cart\Lines\Behaviors::total($line));

[`scripts/php-from-the-command-line.sh`](../../../scripts/php-from-the-command-line.sh) does all of
this in CI, with no Java calling the API. [`examples/php-cart`](../../../examples/php-cart) is an
application built this way: `examples/cart-model`, the cart model every host's cart example runs,
with its HTTP boundary decoded by raoh-php and its injected behaviors implemented over PDO. Its
README says how to build and run it, and what differs from the Java `raoh-souther` example.
`scripts/php-cart-example.sh` builds it and runs its tests in CI.

## What the binding is made of

A binding is written under the namespace the command was given, so two libraries publishing a
module of the same name can stand in one application. A module is a namespace under it, each part
of its name with its first letter made capital (`shop` is `Acme\Shop\Shop`, `cart.lines` is
`Acme\Shop\Cart\Lines`). A product, a newtype and a unit are each a `final readonly` class holding
the value where the library made it, with a reader for each field, a static `of` building one and
answering a raoh-php `Result`, a static `decode` reading one out of its external form, and `encode`
writing it. `decoder` is a raoh-php `Decoder` over a PHP value, which a host composes with its own
the way a JVM host composes a type's `decoder()`: what the library finds wrong is an issue at the
path the decoder was reached at. A PHP array is an ordered map, and a list is the one keyed by its
indices, so an empty array is an empty object and an empty list at once. The value is therefore not
written as text, which would have to say which: it is written with every array as an object and read
by the library's reading of a host's value, which takes an array keyed by its indices as a list
wherever the declaration holds one. A float stays one, so `1.0` handed where an `Int` is taken is
refused rather than read as `1`.

A sum is an interface, which a sum whose cases are all its cases extends, and each case's class
implements it. `<Sum>Codec` finds which class a value is through the sum's `case` function, and
reads and writes the sum's own external form, which says which case it is, with a `decoder` of its
own. A case the model keeps, or a sum whose cases the library cannot tell apart, is `<Sum>Value`,
which is still the sum and can still be written.

A module's behaviors are static functions on `Behaviors`, and its values are static functions on
`Values`. A behavior is also a class named after it (`quote` is `Quote`), which an application holds
the way the JVM backend's does. One a host implements is abstract, with an `apply` typed as the model
says, and an application extends it. One the library defines is final: `bind` takes an instance of
the class of each behavior it `requires`, in that order, each parameter named after the behavior, or
by its place (`$dependency0`) where two of one name from two modules are both required. `of` makes
one that requires nothing. `apply` calls it with the capabilities of what it was bound to, and the
class is callable, so `($placeOrder)($orderId, $userId, $orderer)` is the same call. It answers a
value of the caller's run, as every function does, and opens no run of its own, whose values would
be gone by the time the caller held them. A missing or mistyped implementation is PHP's `TypeError`
at `bind`, not an `UnboundInjection` at the call. A behavior bound to another holds that one as it
was bound, so one behavior bound to two implementations of another at two places calls each at its
own place, as the JVM's does. A class is what the binding adds beside the model's surface, under a
name the binding makes, so it is never a reason to refuse one: a behavior whose class PHP will not
take (`clone`), or whose class would be one with another the module's binding writes (`behaviors`,
or `lookupCodec` beside `LookupCodec`), has no class, and neither has what requires it. Each stays a
function on `Behaviors`.

## How values cross

How each value crosses is the shape the manifest says for it, and the binding decides only how PHP
holds what crosses in that shape. An `Int`, a `Bool` and a `String` are PHP's own `int`, `bool` and
`string`, and a value of a declared type is an object of its class.

Text is checked to be UTF-8 before it is handed over, and a PHP string that is not is an
`\InvalidArgumentException`. The library puts what it takes in NFC by the Unicode version the
language names, whichever ICU PHP was built with, so a string read back may differ in its bytes from
the one handed over. Text whose NFC form is longer than a `String` holds is refused by the library,
and the binding throws that refusal as a `SoutherAbort` naming `REQUIRED_FORM_HAS_NO_PLACE`.

A `Decimal` is a `Souther\Runtime\Decimal` both ways: its integer as a string of digits and its
scale as an `int`, the two it is made of, since no type of PHP's own keeps a scale below nought. The
scale is kept as it was, so `1.50` is `150` at scale 2 and `1.5` is `15` at scale 1. What PHP does
with them, a `BcMath\Number`, text, a money library, is the application's. A `Date`, a `Time`, a
`DateTime` and an `Instant` are `Souther\Runtime\Date` and the classes of the other three names,
held as their numbers: a year, month and day; an hour, minute and second; a date and a time; the
second from the epoch and the nanosecond. Each is checked where PHP makes it against what the type
holds, and a constructor handed numbers that name no such value throws `\InvalidArgumentException`.
Each crosses to the library as those numbers, each a 64-bit integer, and never as text, and the
library answers whether they name a value. The runtime has already held them to that, so a refusal
there is the runtime and the library disagreeing and is a `\LogicException`; the same holds for a
`Decimal`. The text `java.time` writes for each is only for PHP to show, through `__toString`. None
of them is a `\DateTimeInterface`, a moment in a zone, which only an `Instant` is and which does not
reach every year one does; `Date::of` and `Instant::of` take one, and `Instant::toDateTime` gives
one, to the microsecond.

A list is a PHP list both ways, typed `array` for PHP and `list<T>` in the docblock for PHPStan. An
element is handed over as a value of its type is anywhere else, in the run the list is built in, so
an array with a key out of order is an `\InvalidArgumentException` and an element of another type a
`TypeError`, both before the library is called. A list read is copied into a PHP array when it is
read, each element held as a field's value is. A list is built and read through the functions the
module offers for a list of its element, and a function handing a list that the module offers no way
to build or read is not written. A tuple is a PHP list of its members, typed `array` for PHP and
`array{0: T0, 1: T1}` in the docblock. An optional is null where it holds nothing and what it holds
where it holds something, except where what it holds may itself be null: an optional of an optional
holds its value in a `Souther\Runtime\Some`, so `Int??` is null, `Some(null)` or `Some(1)`, and
holding nothing is told apart at each depth.

A function value is a `Closure`, typed `\Closure(T): R` in the docblock. One the library answered is
called in the innermost run going when it is called, and is refused, as any value is, once the run it
was answered in has ended. A closure PHP hands over where a function value is taken is made into one
through a slot the binding keeps for each function type, made once for the binding for the reason an
implementation's is, and the closure is kept for as long as the run it was handed over in: what it
throws comes back out of the call into the library that reached it.

A union no declaration names crosses in two places. PHP hands one over wherever the model takes it,
as a parameter typed with the PHP union of its members' types, a declared member as its object and a
primitive member as PHP's own type. PHP is handed one only as a behavior's answer, which says which
case the value is: the behavior answers the PHP union of its members' types (`Found|Missing`), each
value made as the class of the case the library says it is, or, for a case with no class of its own,
through the codec of the member sum it is a case of. Nothing is generated for the union itself, which
has no name in the model. A host implementing a behavior that answers one hands back a value of one
of those types as it is.

## What is left out, and what is refused

What a host has no way to reach is not written: a behavior, a value or a field the manifest says
nothing reaches, whatever the reason it gives, and what PHP has no way to hold in the shape it crosses
in. That is a value of a declared type the binding has no class for, and a union no declaration names
anywhere PHP would be handed one other than a behavior's answer, including one inside a tuple, a list,
an optional or a function value.

A name the model gives that PHP will not take is refused with the name, rather than spelt some other
way, and the binding is not written: a reserved word, `this` or a superglobal for a parameter, two
parameters of one function under one name, a field named as a method the binding writes, and two
names that are one where they are looked up. A name the binding makes for what it adds, a behavior's
class or what `bind` and an implementation's `apply` take, is never refused: it is made another way,
or the class is left out. Two methods are one where they differ in the case of ASCII letters, as PHP
compares them. Two classes or namespaces are one where they differ in the case of any letter, since
each is also a file or a directory, and the file systems macOS and Windows use by default do not tell
those apart.

The binding's directory is replaced whole on every build. The command writes it beside where it goes
and puts it there once every binding asked for has been written, so it is the binding of one
manifest: a class the model no longer declares does not survive a build, and a refused build leaves
the directory as it was. The directory carries a `.souther-binding` file naming the generator that
wrote it. A directory without one, holding anything, is refused rather than replaced, but one with it
is replaced whole, so a file an application puts in it is gone after the next build.

## Runs and the values in them

A host calls `$binding->run(fn () => ...)`: the run opens a scope of the library's arena, and when it
ends it expires the run's session and closes the scope. The library closes a scope only as the
innermost one open on the calling thread, which runs always are, since they nest as calls do. No
function of a binding takes a session. Each finds the innermost run going on this fiber of a library
the binding was loaded for, and one called outside any run throws `OutsideAnyRun`. There is nothing
for a caller to choose there: a computation belongs to the innermost run of its library, and a
library's runs are on one fiber at a time. A decoder holds no run, so it can be made once and kept,
and it finds the run it reads in when it is used.

Every value holds a handle to the session it was made in, and every read of one goes through the
handle, which refuses a value whose run has ended (`Expired`) or that another library made
(`ForeignHandle`) before anything reads the memory. A value that has to outlive its run leaves it as
its external form, through `encode`. Runs nest, and a value from an outer run may be handed to a call
in an inner one. What a computation (a construction, a reading, a behavior, a published value)
answers is made in the scope of the innermost run going, and belongs to it. What a field reader
answers is a value the one read already held, made no later, so it belongs to that value's run,
whichever run it is read in. What an implementation is handed belongs to the innermost run, which is
no longer than it lives.

## Failures

A construction that does not hold its type's invariants is an `Err` with `invariant_violation`, and
a reading answers the issues the library found, their codes being Raoh's already, or
`invalid_format` where the text is not JSON. A Souther computation that ends without a value throws
`SoutherAbort`, naming the status. What the runtime throws about runs, handles and the library's
statuses implements `Souther\Runtime\SoutherFailure`; a host's own exception coming back out of a
call does not, and neither do the refusals of a value PHP made (`\InvalidArgumentException`,
`TypeError`) or of a library at loading, described below.

A behavior a host implements is handed to a run as its module's `Injections::of(name: fn (...) =>
...)`, which is what a behavior called through `Behaviors` in that run is constructed from, or bound
to a behavior class as an instance of its own. Each behavior a host implements is one C function
pointer, made once for each library a binding is loaded for, and each implementation handed over is a
capability of it and a number of its own, so a worker does not grow with every request. What turns
what the library hands an implementation into the binding's classes is the binding's, and an
implementation keeps the binding it was written against: one library loaded by bindings generated
under two namespaces calls each implementation through its own binding's, and the library itself
holds nothing of either. A call reaching what nothing was handed for throws `UnboundInjection`. An
exception an implementation throws is the one that comes back out of the call that reached it, and
an implementation answering a value of another type than the behavior answers comes back as a
`TypeError`.

## Loading the library

The runtime loads a library once per process, told apart by device and inode rather than by the path
it was loaded through, since two instances over one file would be two stacks of runs over one arena.
`Binding::load($library)` reads the declarations copied beside the binding with `FFI::cdef`, and a
second path to them may be passed where they stand elsewhere. Under `ffi.enable=preload`, where a
request cannot declare a library itself, a preload script hands `FFI::load()` what
`Binding::preloadHeader($scope, $library)` answers, and a request calls
`Binding::preloaded($scope, $library)`, given the library's path again so that it is the same
library a load of that file would be.

Before anything else, the runtime asks the library `souther_abi_generation()` and refuses one that
answers another generation than the one it calls, or none, with `UnsupportedGeneration`. A binding
says which version of the runtime's surface it was generated for, and refuses to load over a runtime
that says another (`Souther\Runtime\Binding::PROTOCOL`, 11 at present) with a `\LogicException`
naming both. A binding whose numbering of the library's statuses differs from the declarations it
loads, or from another binding's that loaded the same library first, is refused the same way.

The arena is per thread, and a handle is PHP's, which a ZTS runtime such as FrankenPHP does not hand
from one thread to another; nothing here checks for one that was. A fiber is checked for: runs are one
stack, ended in the order they nest, so while a run is going on one fiber, another fiber can neither
start one nor use a value of it (`RunOnAnotherFiber`), a call made there finds no run of its own
(`OutsideAnyRun`), and one suspended in a run holds the library until it ends that run.
