# souther-native-compiler

Compiles a checked Souther program to a host-native object.

A behavior becomes a symbol in an object file the system linker takes, so the output runs where
there is no virtual machine and no wasm engine.

## What it reads

`CheckedProgram` and what is reachable from it, and nothing else of the Souther compiler. Anything
this needs that the program API does not carry is a question for Souther rather than something to
reach around, so it is raised there. This project is the second reader of that boundary, and what
it finds the boundary does not answer is the most useful thing it produces.

Where the code works around something Souther does not answer yet, it names the Souther issue that
asks for it. `scripts/upstream-premises.sh`, run in CI, fails once the Souther this build pins has
the fix, so a workaround does not outlive what it worked around.

## The two halves

The Java half reads a checked program and writes it out. It decides nothing: the projection over
`Core` is exhaustive, so a node added to the language stops this compiling rather than travelling
as something else.

The Rust half reads that and lowers it to Cranelift IR, and Cranelift writes the object. The two
speak over a process boundary — what crosses is a program to compile and an object file, once per
build, so there is nothing here for an in-process call to make faster, and a panic on the Rust side
does not take a JVM with it.

## What is not shared with the wasm backend

The lowering is not, and neither is the representation. A value's layout, what a pointer is, and
how a call is made are what a target decides, and the two targets decide them differently. What the
two backends share is above them — the checked program — and, where the same computation turns out
to be written twice, below them in the Rust runtime. Nothing in the middle is shared, and no
abstraction over the two is written to make it look as though something is.

## Building

    mvn test

Cargo is what builds the Rust half; Maven runs it. The toolchain is pinned in
`rust-toolchain.toml`, so a clone needs rustup and nothing else installed by hand for it. A C and a
C++ compiler are needed too, by the tests that link what came out and run it, and PHP 8.2 or later
with the `ffi` and `intl` extensions, by the tests that read what a host is handed the way an FFI
with no preprocessor does and run a generated PHP binding. Maven also runs Composer, which has to be
installed, for what the PHP runtime in `bindings/php/runtime` depends on, as its `composer.lock`
fixes it.

## From the command line

What the API builds, the command line builds too, so an application needs no Java of its own to
build what it runs. From the root of a clone:

    mvn -q process-classes exec:java \
        -Dargs='--library build/native --php build/php --namespace Acme\Shop model'

`process-classes` builds the driver the command hands the program to; where it is built already,
`mvn -q exec:java -Dargs='...'` is enough. The command ends with what `Main` ends with: 0 where it
wrote everything, 1 where the build is refused (a compile error, what this backend does not write
yet, or a name from the model PHP will not take), and 2 where the command is refused.

    souther-native [-cp <path>] -o <object> <source>...
    souther-native [-cp <path>] --library <dir> [--with <object>]...
                   [--php <dir> --namespace <ns>] <source>...

A source is a `.sou` file or a directory holding some. `--library` writes what a host is handed
(below) into its directory, and `--php` the binding of it, from the manifest the library was written
with. A program importing another build reads that build's modules from `-cp`, the class path the
`souther` command takes, and has its object linked in with `--with`, one for each build.

The two directories are each replaced whole, and they are two: a binding refused for a name in the
model leaves the new library and the binding that was there before. A namespace PHP will not take,
or a binding directory holding what no binding wrote, is refused before the library is built.

Until the runtime is published, an application reaches it as a Composer path repository, which is
the supported way for now:

    {
        "repositories": [
            { "type": "path", "url": "<clone>/bindings/php/runtime" }
        ],
        "require": { "souther-lang/php-runtime": "@dev" },
        "autoload": { "psr-4": { "Acme\\Shop\\": "build/php/" } }
    }

The binding is mapped by the application like its own classes, or loaded with the `autoload.php`
written beside it. `scripts/php-from-the-command-line.sh` does all of this in CI, with no Java calling
the API.

`examples/php-cart` is an application built this way: the cart model of the Java
`raoh-souther` example, with its HTTP boundary decoded by raoh-php and its injected behaviors
implemented over PDO. Its README says how to build and run it, and what differs from the Java one.
`scripts/php-cart-example.sh` builds it and runs its tests in CI.

## Where it runs

Unix hosts: what the object is written as is decided by the host's format, and Mach-O and ELF are
the two anything here has been run on. Writing an object is not refused anywhere else — it is
untried, and the test that links and runs would have to say what a COFF object and its linker want
before it meant anything there. Linking a shared library for a host is refused on anything but
macOS and Linux, since what it asks of the linker is said only for those two.

## What compiles today

Over `Int` and `Bool`: `+`, `-`, `*`, the six comparisons, `&&` and `||`, `if`, and a name for a
value. `/` answers the exact quotient, which is a `Rational` and has no representation here. Unary
`-` of a literal is folded at compile time, and of anything else ends the run where it leaves the
range, the way `+`, `-` and `*` do.

An operation the language implements as a kernel: `Int.add`, reusing the same instructions `+`
does; `Int.truncatingDivide` and `Int.truncatingRemainder`, which answer `Int | DivisionByZero`
and end a quotient only on the smallest `Int` over -1; and `String.length`, which counts code points
and not the bytes a string is held in. Every other kernel is still ahead — including everything
else a program does with text beyond a literal, the six comparisons and `++`, which the language
reaches through one. What a comparison of two strings compares is neither of the two addresses and
not the bytes either — the language orders text by UTF-16 code unit, and says so of every carrier
whatever one stores a string as.

A value of a union says which case it is by the token at the front of it. A declared case's token is
its declaration's; an `Int`, a `Bool` or a `String` standing as a case, and a case the language
gives such as `DivisionByZero`, is carried with a token the runtime defines for it, so a value of
`Int | DivisionByZero` is told apart the way a value of a sum is, and a case keeps its token when
the union it stands in widens. Such a union stays in the object that made it for now: no program
yet hands one to another object, since the build publishing it would also have to write it in its
external form, and a composition does not route on such a case.

A model's own types: a product, a newtype, a unit, a sum, a value that may be absent, and several
values carried as one. Built, read a field off, and forked on which case a value is.

Calls. A helper that does not call itself arrives already written into the body that calls it; one
that does is a definition the module holds, and every module that reaches it holds a copy, which is
what the language says a published helper is. A behavior reaching a behavior is a call whether this
program answers it or not. One another build implements is a name the object leaves for that build's
object. One with no body that declares nothing to depend on is answered by the object of the build
that declares it, with whatever the host running the program registered for it, so what answers a
dependency is the host's to choose when it runs the program and not something linked in.

What a call may reach is wider than what the object defines — a body may name a behavior, or a
type, that a module built before this one declares — so the document says the two apart.

What the object's symbol table carries is what the declaring module publishes, read together with
what this object is for. The object is one whole program, so a name the module keeps is reached
inside it and nowhere else, and a name it publishes may be reached by a host linking the object in
or by another Souther build's object. The two are different questions: a module's `exposing` clause
is its surface in the language, and an object that read that as its own answer would be publishing
whatever surface suited the shape it happened to be built in.

A behavior the object does not define is reached over numbers, truths, text, and values of a
model's own types. A value of a declared type says which type it is with the address of a byte its declaration
owns, under a name the linker resolves, so a fork in one object over a value built in another
compares what the linker resolved for both. A type whose representation is still to be designed
does not cross, and the signature is where that is said.

A type that states what its values owe is built by one function per declaration, which every
construction of it calls: it takes the fields, runs the clauses in the order the type states them,
a clause a spread took in among them, and lays the value out only once all of them hold. The first
that does not hold ends the run with `InvariantNotHeld`, and a clause that itself ends without a
value, leaving an `Int`'s range, ends it for that reason. A construction with no clause to run,
a unit's value or a value of a type of this compile that states none, is laid out where it stands
by the same code the function lays a value out with, since there is nothing for a call to run and
nothing the construction can end for.

The function belongs to the build that declared the type, the way the type's token does. That
build's object defines it for every type a body there builds through it, every type its modules
publish, since another build can name and build one of those, and every type a value of a published
one is read through, kept or not, since reading one builds it. A build constructing a value of a type
another declared calls that one: what a clause reads
and calls, a helper among them, is the declaring build's own, and a copy run elsewhere would run
without it. It is a call between objects this compiler built, under a symbol carrying the ABI
generation. What a host calls to build a value is a boundary of its own and not this.

The constructor is one reading of a function under it that decides the construction and answers
which clause did not hold, as its place among the type's, beside the status. The constructor turns
that place into `InvariantNotHeld`, a reader into an issue at the value's path, and an attempted
construction into the arm naming the clause: `guard PendingItem { ... } as pending else |
withinCapacity -> CartFull` builds the value and goes on where every clause holds, and answers
`CartFull` where `withinCapacity` does not. `else e` on its own answers every clause; beside arms
naming clauses, `| _ ->` answers the clauses that have no name and no other, as the checker holds
it. A clause that itself ends without a value still ends the run. The deciding function
is the declaring build's too, reached from another build under a symbol of its own, so an attempt of
a type another build declares runs that build's clauses and takes its arm here. What crosses to the
attempting build is what each clause is answered under, in the order the clauses run, and nothing
of what they say.

A host builds and reads a value of a type the module publishes through functions the object
defines for it, and never through where the value keeps anything. A host holds a value as an
address it does not look behind, good until the mark taken before it was made is reset, and hands
it back to these and to the behaviors. For each published type with fields or none there is a
constructor, `souther3_m_<module>_t_<Name>_construct`, taking the fields and answering `status +
out` the way the type's own constructor does, since it is that constructor it runs: a value whose
clauses do not hold is answered `InvariantNotHeld` and nothing is written through `out`, and a type
with no clause answers a status too, so a clause added later does not change how a host calls it.
For each field there is a reader, `..._f_<field>`, answering the field itself. For a published
sum whose every case is a declared type there is `..._case`, answering which of the cases the sum
descends to the value is, as its place among them counted from nought; the address the value is
tagged with never leaves the object. A union with a primitive among its cases has no reader, since what
carries the primitive is not something a host is handed. The case answered is the concrete one the value is, and
whether a host can read that case further is its own publication's answer and not the sum's. A
behavior answering a union no declaration names has the same reader beside its call,
`souther3_m_<module>_b_<behavior>_answer_case`, counting the cases the union descends to: a member
that is a sum counts as its own cases, since a value of it is one of them. It is the behavior's and
not the union's, which has no name to be spelt under.

An `Int` crosses as 64 bits, a `Bool` as a byte, and text and a value of a declared type as an
address. An optional crosses as a presence beside the value: a constructor takes a byte and the
value, which is ignored where the byte is nought, and a reader answers the byte and writes the value
through a pointer only where there is one. How the generated code keeps an optional is not what a
host is told. What a host is handed is decided apart from what another object built by this
compiler reads the same way, because the two are different questions. A field of a type with no way
across yet has no reader, and keeps its type from having a host constructor, and nothing else: its
siblings are still read. A type the module keeps has none of these, whichever published sum it is a
case of.

A list crosses as an address too, of type `souther_list`, wherever its element crosses: as a field,
as what a behavior takes or answers, and as what a behavior a host implements takes or answers. A
host builds one and reads one through functions the object defines for each way an element crosses,
under the module: `souther3_m_<module>_l_<element>_construct`, taking a count and a column for each
word an element crosses as and answering the list, `..._length`, and `..._at`, taking the list, an
index and room for the element's words and answering one where the index is inside the list and
nought, with nothing written, where it is not. `<element>` is the word, `value` or `int` and so on,
with `present_` before it for an optional element, which is two columns, a presence and the value,
as an optional field is. So a list of one declared type is built through the same functions as a
list of another, a list of lists is a list of `list` elements, and none of it asks where the list
keeps its length. A count below nought, or one no room can be taken for, is the host's mistake and
ends the process.

A clause of a type the module keeps and nothing here builds or reads, or one whose fields have no
representation here, is read, and refused if the two halves disagree about it, and is not run: no
value of the type is built here to run it over.

An answer at the boundary, written as the language writes it. Every behavior the object defines
and publishes, and every row, has a second entry that runs it and hands back its answer as JSON:
a number, a truth, text, a newtype as what it wraps, a product as an object of its fields with an
absent optional left out, a unit as an empty object, and a set of alternatives as a bare name or
discriminated under its tag. None of that is decided here. What each position writes is what the
checker settled for it and the program carries — the codec shape of every field, and the form a
set of alternatives travels in with both of its keys — and whether a case takes the tag into its
own object or is wrapped beside it is read off the arm it was declared in. The encoder is compiled
per declaration, so nothing about a declaration is kept for run time to interpret, and the runtime
only holds the tree it is handed and writes it out. Which case a value is comes from the token the
linker resolved, and is never what the case is written as.

A value read from that form, and any value written to it. For each published type whose values
have a form here there is `..._t_<Name>_decode`, taking JSON as bytes, and `..._encode`, taking a
value and answering its JSON. The decoder is the encoder walked backwards over the same shapes, one
reader per declaration: a field left out is absent where it may be and missing where it may not,
`null` is absence where there is no key, a case is told apart by the key and the name the program
carries for it, and a member the declaration does not name is not read. What was read is built by
the one function every construction of the type goes through, so a value read is one its clauses
hold of, checked in the order they are declared. A decoder answers a status where a clause ended
without a value, and otherwise a reading the host asks what it came to: a value, the bytes not
being JSON and where they stopped, or every issue found in the document — not the first — each with
one of Raoh's codes, a JSON Pointer and its metadata as named entries. A clause that does not hold is
`invariant_violation` at the value's path, naming the type's module and name and the clause where
it has one. A value of a type another build declares is read by that build's object, under
`souther3.<module>$read$<Name>`, whatever kind of type it is: how a declaration is read is the
declaring build's, and for a type built from fields that build is also the only one that can say
which clause did not hold. Text read is canonicalized to NFC. What JSON is, is `souther-json-syntax`, a crate that knows
no Souther type, no arena and no runtime, written to be what both runtimes read once #17 moves it.

A `List` is laid out inside a run as its length and then its elements, one slot each. Of the
list kernels the standard library declares (the `intrinsic`s in `souther/list.sou`), this backend
lowers `list.length` and `list.get`; the others are still ahead. `List.fold` is not one of them:
it is an ordinary helper over `List.get` (ADR-0051), and so are the combinators written over it.
In the external form a list is an array of its elements, and a mistake inside one is answered at
the element's index (`/lines/2/quantity`). Two values of one type compare by what they are made
of, a list element by element, through a comparator the object holds per type.

Still ahead: a `Decimal`, a `Set` and a `Map`,
every kernel but `Int.add`, `Int.truncatingDivide`, `Int.truncatingRemainder`,
`String.length`, `List.length` and `List.get`, a value
that runs in the module declaring it, and a
behavior that declares what its answer owes, which is refused rather than answered without the
rule being run. A set, a map and
a `Decimal` are read off the program whole and refused where one would be laid out or written: how
every carrier orders a set's members, spells a map's keys and writes a decimal is for the language
to state before a backend writes one.


## What a host is handed

A build for a host writes five things into a directory: the object, `souther.o`; the declarations
of every function a host calls, `souther.ffi.h`; a header a C or C++ compiler includes,
`souther.h`; a manifest, `souther.json`; and a shared library of the object and the runtime,
`libsouther.dylib` or `libsouther.so`. The driver writes them when run with `--library <directory>`,
and `NativeCompiler.library` is that from Java. Nothing on the Java side reads the program to say
what a host can call. Every function a host calls is put on one surface where its code is emitted,
and the declarations, the manifest and what the library exports are each written from that
surface, so none of them names a function the others do not. A test holds the three, and what the
object defines, to one set, reading each of them as it is.

A host calls a function by a C identifier. The symbols one object built here calls in another carry
`.` and `$`, and no C compiler or FFI that reads C declarations can name those. So what a host
calls is spelt apart: `souther3`, the ABI generation, then the module as `_m_<segment>` per segment
of its dotted name, then `_b_<behavior>`, `_v_<value>`, `_t_<type>`, or `_l_` and how a list's
element crosses, and what is done with it. A
name is written as it is where it is ASCII letters and digits, with `_` doubled and any other
character as `_u<hex>_`, its code point. So `shop.quote` is `souther3_m_shop_b_quote` and a
behavior named `数量` is `..._b__u6570__u91cf_`, and inside a name `_` is only ever followed by `_`
or `u`, which is what keeps every spelling readable back to the one set of names it was made from.

A published behavior and a published value have an entry of their own for a host, which converts
what a host hands over and calls the symbol another object calls. The two are called by different
parties, and the day one of them takes a value in a form a host does not hand one over in, the
host's entry still takes what a host hands over. An operation on a published type is called by a
host and by nothing else, so it has the one symbol.

The declarations are every function a host calls, the runtime's among them, and the numbers a
status and a reading's outcome are compared with, as enumerations rather than macros. They are C
and nothing else — no directive, no guard — because a reader of C declarations with no
preprocessor, PHP's `FFI::cdef` among them, takes them as they are, and a test hands them to it.
What a C or C++ compiler wants around them is `souther.h`, the same text for every library: a
guard, `<stdint.h>`, C linkage for C++, and the declarations included. So the surface is written
into C once, and the two readers are given what each can read.

The manifest says the same functions in the model's terms, for a binding to be written from
without reading the program: each module's behaviors with what they take and answer, its published
values, and its published types with their fields and cases, each beside the function that reaches
it, or `null` where a host has no way in yet. Apart from its behaviors, each module's `injections`
are the behaviors a host implements, published or not, each with what it takes and answers, the
function type a host implements it as, and what it registers one through. The function type is not
a function: every `name` of a function in the manifest is a symbol the library defines, and the
type's is under `type`. What a behavior takes is `named`, under the names its signature gives them,
or `positional` for a `>->` composition, which declares no parameters; the names are the
signature's and never those a `let` binds. What a behavior answers is its `type` beside `union`,
which is `null` unless the type is a union no declaration names, and then lists the `cases` the
union descends to and the `case` function answering which of them a value is. The type stays what
the model says, members and all, the same as wherever else it is written. A type is said by its
module and its name, never by the key the Java half hands this one. Each module's `lists` are the
functions a list is built and read through, one entry for each way an element of a list crosses
there (`{"whole": "value"}`, `{"present": "string"}`), apart from the type `{"kind": "list"}`, which
says only what the model says: a binding works out how a position's element crosses and finds the
entry for it. A parameter is `given`, `room`, or a `slice`, as many of a word as a count before it
says. A behavior `requires` what constructing it requires injected, each by its module and its name,
in the order the checker answered it: a behavior a host implements, or one constructed from what it
requires in turn. It is the checker's list as it crossed, and not what the body calls, since a
composition requires what its stages require. What a manifest may say is Rust types, and version 6
is `native/crates/compiler/tests/interface-v6.json`: a test holds a program's manifest to it, and
another reads it with those types and writes it back unchanged. The manifest carries its own
`version`, moved when what it says is read differently, and the `abi` its functions answer to,
which is the generation in every symbol.

A behavior with no body that declares nothing to depend on is implemented by the host. The object of
the build that declares it defines the behavior's own symbol, so every object calling it calls it the
way it calls any behavior, and that definition calls what the host registered for it on the calling
thread. The host registers through `souther3_m_<module>_b_<behavior>_register`, handing a pointer to
a function of the type `..._implementation` and handed back the one it replaced, either null for
none. The function takes what the behavior takes and room for its answer, in the words a host hands
a published behavior, and answers a status. Registration is per thread, like the arena, and handing
back what was replaced is how a binding registers an implementation around one call and puts the
previous one back after it, so a call made from inside an implementation into another still finds
its own. What is registered stays the host's, and has to stay callable while it is registered, so a
binding makes the pointer once and hands that same pointer over around each call. PHP's FFI makes a
new C entry each time a closure is handed to C and keeps it until the request ends, so a binding
that handed its closure over on every call would grow for as long as the process lives; a test
holds a PHP binding to the first shape. A call with nothing registered answers `INJECTION_UNBOUND`. An implementation may answer
`ANSWERED` or `HOST_EXCEPTION`, which says it threw and that the host kept what it threw to throw
again where the outermost call returns, since a host's exception cannot unwind through generated
code. Anything else it answers is `INJECTION_PROTOCOL_VIOLATION` by the time a caller sees it: an
implementation is outside the model, where a clause that does not hold is a failed reading and not
an abort, so it cannot end a computation with a language abort. None of these three is a reason a
Souther computation ends, and their numbers are the top of what a C `int` holds, away from the
aborts'. The answer still crosses the behavior's `ensures` where the checker placed it. What a host
can hand over is what it can for a published behavior, and a behavior with no body taking or
answering anything else is not written yet.

What the library exports is what the header declares, and nothing else. A row's entry and a
boundary stay in the object, since running the program's own rows is what they are for, and so does
every symbol one object built here calls in another; the library keeps them and does not offer them.
The runtime is a static archive, which gives a link only what something asks it for, and nothing in
the object asks for a function only a host calls. So the link names each function a host calls as
wanted, and the rest of the archive comes in only where the object reaches it. By hand, what the
driver runs is:

    # macOS
    cc -dynamiclib -o libsouther.dylib -Wl,-install_name,@rpath/libsouther.dylib \
        -Wl,-exported_symbols_list,<list> -Wl,-u,_<symbol> ... souther.o libsouther_native_runtime.a

    # Linux
    cc -shared -o libsouther.so -Wl,--version-script=<script> -Wl,--no-undefined \
        -Wl,-u,<symbol> ... souther.o libsouther_native_runtime.a

where the list and the script name every function the header declares.

A library is one program, so it holds every build the program reaches: a build's object defines
what reads and builds a value of a type it declares, and another build calls that. Those objects are
handed to the driver with `--with <object>`, or to `NativeCompiler.library` beside the program, the
same objects an executable of it is linked with. A behavior with no body is among them: the object
of the build that declares it is where it is defined, so a program calling one another build
declares is linked with that build's object like any other, and nothing besides Souther objects and
the runtime goes into a library. What the library then offers a host is what each of its objects
offers, and each says that itself: an object carries its own surface, in a section
of its own, so an object another build wrote is described by the build that wrote it and not by a
second reading of a program this one does not have. The declarations, the manifest and the export
list are written from what the objects carry. A module two of them carry is refused, and so is an
object that carries none, and a program missing a build it reaches is refused when it is linked
rather than when a host loads it.

What a behavior takes is said with the names its signature declares, which a binding writes its
function's parameters under. A `>->` composition declares no parameters, so what it takes is said by
type and in order, and a binding names each by its place.

## A PHP binding

`PhpBindings.generate(library, into, namespace)` writes the PHP a host calls a library through, from
the manifest and nothing else, under a namespace the caller names: two libraries publishing a module
of the same name can then stand in one application. A module is a namespace under it (`shop` is
`Acme\Billing\Shop`). A product, a newtype and a unit are each a `final readonly` class holding the
value where the library made it, with a reader for each field, a static `of` building one and
answering a raoh-php `Result`, a static `decode` reading one out of its external form, and `encode`.
`decoder` is `decode` as a raoh-php `Decoder` over a PHP value, which a host composes with its own
the way a JVM host composes a type's `decoder()`: what the library finds wrong is an issue at the
path the decoder was reached at.
A sum is an interface, which a sum whose cases are all its cases extends, and each case's class
implements it. `<Sum>Codec` finds which class a value is through the sum's `case` function, and
reads and writes the sum's own external form, which says which case it is, with a `decoder` of its
own. A case the model keeps,
or a sum whose cases the library cannot tell apart, is `<Sum>Value`, which is still the sum and can
still be written. A module's behaviors are static functions on `Behaviors`, its values on `Values`.
A behavior is also a class named after it (`quote` is `Quote`), which an application holds the
way the JVM backend's does. One a host implements is abstract, with an `apply` typed as the model
says, and an application extends it. One the library defines is final: `bind` takes an instance of
the class of each behavior it `requires`, in that order, each named after the behavior, or by its
place (`$dependency0`) where two of one name from two modules are both required. `of` makes one that
requires nothing.
`apply` calls it with what it was bound to registered for the length of the call, and the class is
callable, so `($placeOrder)($orderId, $userId, $orderer)` is the same call. It answers a value of the
caller's run, as every function does, and opens no run of its own, whose values would be gone by
the time the caller held them. A missing or mistyped
implementation is PHP's `TypeError` at `bind`, not an `UnboundInjection` at the call. A behavior
bound to another brings what that one was bound to, and one bound to two implementations of one
behavior is refused at `bind`, since the library calls one implementation of a behavior at a time
(#72). A class is what the binding adds beside the model's surface, under a name the binding makes,
so it is never a reason to refuse one: a behavior whose class PHP will not take (`clone`), or whose
class is one with another the module's binding writes (`behaviors`, or `lookupCodec` beside
`LookupCodec`), has no class, and neither has what requires it. Each stays a function on
`Behaviors`.
A behavior answering a union no declaration names answers the PHP union of its members' classes
(`Found|Missing`), each value made as the class of the case the behavior's `case` function says it
is, or, for a case with no class of its own, through the codec of the member sum it is a case of.
Nothing is generated for the union itself, which has no name in the model. A host implementing a
behavior that answers one hands back an object of one of those classes as it is. A list is a PHP
list both ways, typed `array` for PHP and `list<T>` in the docblock for PHPStan: an element is handed
over as a value of its type is anywhere else, in the run the list is built in, so an array
with a key out of order or an element of another type is refused before the library is called. A
list read is copied into a PHP array when it is read, each element held as a field's value is.
A module's classes build and read a list through that module's own functions and no other
module's, every list a module's manifest entry says is held to what a list of its element is built
and read through, and a module with a function handing a list across and nothing to build one
through is refused rather than written without the function.
The FFI declarations are the build's own, copied beside the binding as `souther.ffi.h`, and
`autoload.php` loads the binding's classes for a host that does not map the namespace itself. The
directory is written beside where it goes and put there whole, so it is the binding of one manifest:
a class the model no longer declares does not survive a generation, a refused generation leaves the
last one as it was, and a directory holding anything a generation did not write is refused rather
than replaced. The driver writes a library's directory the same way.

What a host has no way to reach is not written: a behavior with no `call`, a field with no `read`, a
behavior taking or answering a type with no representation for a host, and a union no declaration
names that PHP would be handed other than as a behavior's answer, since nothing else says which case
a value of it is.
A name the model gives that PHP will not take is refused with the name, rather than spelt some
other way: a reserved word, `this` or a superglobal for a parameter, two parameters of one function
under one name, a field named as a method the binding writes, and two names that are one where they
are looked up. A name the binding makes for what it adds, a behavior's class or what `bind` and an
implementation's `apply` take, is never refused: it is made another way, or the class is left out. Two
methods are one where they differ in the case of ASCII letters, as PHP compares them. Two classes or
namespaces are one where they differ in the case of any letter, since each is also a file or a
directory, and the file systems macOS and Windows use by default do not tell those apart. What PHP
refuses is held to PHP itself by a test that asks it. A manifest
is read as a version only once it has said it is that one, so one of another version is refused as
that and not as whichever member moved since, as the driver reads a transport and what an object
carries.

Everything else is in `bindings/php/runtime`, one Composer package every generated binding runs on.
A host calls `$binding->run(fn () => ...)`: the run marks the arena, and when it ends it expires the
run's session and resets the arena to the mark. No function of a binding takes a session. Each
finds the innermost run going on this fiber of a library the binding was loaded for, and one called
outside any run throws `OutsideAnyRun`. There is nothing for a caller to choose there: a
computation belongs to the innermost run of its library, and a library's runs are on one fiber at a
time. A decoder holds no run, so it can be made once and kept. Every value holds a handle to the session it
was made in, and every read of one goes through the handle, which refuses a value whose run has ended
(`Expired`) or that another library made (`ForeignHandle`) before anything reads the memory. A value
that has to outlive its run leaves it as its external form. Runs nest, and a value from an outer run
may be handed to a call in an inner one. A value belongs to the run its memory is dropped with,
which a binding knows by where the value came from. What a computation (a construction, a reading, a
behavior, a published value) answers is made after the mark of the innermost run going, which is
the run a function finds, and its answer belongs to it. What a field reader answers is a value the one read already held, made no
later, so it belongs to that value's run, whichever run it is read in. What an implementation is
handed belongs to the innermost run, which is no longer than it lives. A library is
one per file, told apart by device and inode rather than by the path it was loaded through, since
two instances over one file would be two stacks of runs over one arena. Text is checked to be UTF-8 and put in NFC before the
library takes it, which the library itself does not do.

A status crosses as one of three things. A construction that does not hold its type's invariants is
an `Err` with `invariant_violation`, and a reading answers the issues the library found, their codes
being Raoh's already, or `invalid_format` where the text is not JSON. A Souther computation that
ends without a value throws `SoutherAbort`, naming the status. A behavior the host implements is
handed to a run as `Injections::of(name: fn (...) => ...)`, or bound to a behavior
class as an instance of its own. Each behavior a host implements is one C function pointer, made
once per binding, and what is registered through it is registered around each run or bound call
and put back after, so a worker does not grow with every request. A call with nothing registered
throws `UnboundInjection`, and an exception an implementation throws is the one that comes back out
of the call that reached it.

A binding says which version of the runtime's surface it was generated for, and refuses to load
over a runtime that says another (`Binding::PROTOCOL`). The runtime loads a library once per
process, with `FFI::cdef` or, under `ffi.enable=preload`, from the scope a preload script declared
with `Binding::preloadHeader`, given the library's path again so that it is the same library a load
of that file would be. The arena and what is registered
are per thread, and a handle is PHP's, which a ZTS runtime such as FrankenPHP does not hand from one
thread to another; nothing here checks for one that was. A fiber is checked for: runs are one stack,
ended in the order they nest, so while a run is going on one fiber, another fiber can neither start
one nor use a value of it (`RunOnAnotherFiber`), a call made there finds no run of its own
(`OutsideAnyRun`), and one suspended in a run holds the library until it ends that run.

## Where a value lives

In an arena the caller brackets. Nothing frees a Souther value on its own: what a run makes is
dropped in one go by whoever bracketed the call, so generated code takes room and never gives any
back, and nothing it emits has to know what owns what.

A string a literal spells lives in the object rather than the arena, because it says the same text
every run and nothing about it is worked out. What a value holds either way is an address, and
nothing at it says which of the two it is: a join reads a literal the way it reads what a run made,
and a comparison never asks.

A value of a declared type carries which type it is, so a fork on what a value is reads a slot
rather than asking where the value came from. Everything a value is made of sits in a slot of one
width, which keeps a field's offset a fact about its position rather than about the types of the
fields before it.

What the slot holds is an address, and what is at it is one byte the declaration owns. A
declaration is at home in the object of the build that checked its module, which is what defines
that byte; every other object naming the type leaves the name for whoever links it. So two objects
naming one declaration reach one address, and the agreement is between each build and the linker —
which is already what makes a call reach a definition, and is the thing a count of the declarations
one document happened to bring could never be.

Everything else the language admits says so rather than being written as whatever it resembles. A
closed set crosses whole — every operator and every primitive — and the driver answers whether it
can write one; a node whose shape on the wire has not been designed cannot be written at all, and
the writer says so. Either way it is `NotLowered`, which is not what a program the language refuses
gets.

## How it is known to be right

By the program's own `example` rows, and by what a row's arrival says about it. A row the compile
ran arrives as one an output can put to its own emission; a row whose answer did not keep it
refuses the program. So for such a row the JVM answered and the answer kept the row, and putting
the native run to the same row holds both carriers to one statement without this project writing
down what either of them should say. Whether an answer keeps a row is asked of the row, so there is
no second reading of what a row means either.

A row the compile did not run arrives saying so and carrying why. Those are not skipped: skipping
them is how a check goes on being green over fewer and fewer rows.

The object carries an entry per row, which is what runs one. The values the row states are written
into the entry when the program crosses, so running a row is the object doing something with the
row and not the behavior being reached with values from outside — which is what lets a row of a
name the module keeps be run at all. A row stating a value this backend has no expression for
refuses the build rather than being left out of the object, for the same reason: an object missing
an entry would link and answer every row it did carry.

This is not the two carriers compared against each other. Holding both to one statement is not
running both and comparing what came back, and running a program on every carrier and comparing the
answers is still ahead.
