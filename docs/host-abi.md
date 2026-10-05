# The host ABI

What a library built for a host offers, and what a host must follow to call it. This is for whoever
writes the runtime a binding runs on in a language of their own (the Rust crate, the Go module and
the PHP package in `bindings/` are three), and for a host calling a library from C directly. How a
binding is generated from a library is [writing-a-binding.md](writing-a-binding.md).

This document explains the ABI and says where each part of it is stated. It does not list what the
generation record lists: the functions, their words and the numbers are in
[`native/crates/abi/generations/`](../native/crates/abi/generations/), one file for each generation,
and in the header a build writes. Where this and the record disagree, the record is right.

## What a build writes for a host

A build for a host writes five things into a directory: the object, `souther.o`; the declarations of
every function a host calls, `souther.ffi.h`; a header a C or C++ compiler includes, `souther.h`; a
manifest, `souther.json`; and a shared library of the object and the runtime, `libsouther.dylib` or
`libsouther.so`. The driver writes them when run with `--library <directory>`, and
[`NativeCompiler.library`](../compiler/src/main/java/souther/nativecode/NativeCompiler.java) is that
from Java. Every function a host calls is put on one surface where its code is emitted, and the
declarations, the manifest and what the library exports are each written from that surface, so none
of them names a function the others do not. A test holds the three, and what the object defines, to
one set.

The declarations are C and nothing else, no directive and no guard, because a reader of C
declarations with no preprocessor, PHP's `FFI::cdef` among them, takes them as they are. They name
every function a host calls, the runtime's among them, and the numbers a status and a reading's
outcome are compared with, as enumerations rather than macros. `souther.h` is what a C or C++
compiler wants around them, the same text for every library: a guard, `<stdint.h>`, C linkage for
C++, the declarations included, and static assertions of the size and alignment of every type a host
lays out room for, where the compiler is C11 or C++11 or later.

The manifest is not part of the ABI. It is how the driver tells the command what a library offers,
and the command reads it into the model a generator is handed; its format is private to the two,
which are released together and read one version exactly. A runtime never reads it, and neither does
a generator. What it says is described with its types in
[`native/crates/compiler/src/manifest.rs`](../native/crates/compiler/src/manifest.rs).

## The ABI generation

Everything a host calls of a library belongs to one ABI generation, and the generation moves when
any of it changes. It is `ABI_GENERATION` in the `abi` crate, and the file of its number under
`generations/` records every contract the generation has offered: every function a host calls and
every function generated code calls in the runtime, with what each takes and answers; every word
and what it is on the machine; the storage a host lays out; the scope contract; the input contract;
the statuses and the range reserved for a host; how a value is hashed; and the constants generated
code and the runtime agree on. The record only grows: a host or an object can be built on any day of a
generation's life, so what is added under a generation is recorded under it, and taking it away
later is a change of generation. A test fails when a recorded line is gone or changed without a new
generation, and when the contract has a line the record does not yet; another fails where a
generation has no record.

A host asks a library which generation it answers to before it calls anything else, with `uint32_t
souther_abi_generation(void)`, and refuses one it was not written for. That function is outside the
generations: its name, its signature and how it is called are the same in every one from 9 on, and
no new generation can change them, which a test of its own holds. A library without it is of
generation 8 or earlier.

The functions a host calls for a module carry the generation in their name, so a library of another
generation has none of the names a binding looks up. The runtime's own functions carry none, since
every library exports the same ones. For the linker, the runtime defines a symbol only its
generation defines, `souther_runtime_abi_<n>`, and every object refers to the one of its own, so an
object linked with a runtime of another generation has an undefined symbol. That symbol is not
exported, and a host asks the generation with `souther_abi_generation` instead.

## Words

What a function takes and answers is said in words, and each word is a kind of thing a host holds:
an `Int`, a `Bool`, a count, a status, which case a value is, what a reading came to, text, a value,
a list, a scope and the rest. Two kinds one width wide are two words all the same: a value and a
string are both an address, and a host handed the one where the other was meant has been handed
something else. Each word says what it is on the machine, one of an unsigned byte, an unsigned or a
signed 32 bits, a signed 64 bits, or an address, and the header's C types and the driver's machine
types are both read off it, so the two cannot say different things. The Rust runtime of this
repository and the Rust binding's runtime hold every Rust type that stands for a word to the same
representation, and a C compiler holds the Go runtime's calls to the header. The words, and the
representation of each, are the record's `word` lines.

A `Bool` is a byte without a sign. The library answers one as nought or one, and reads any byte but
nought that a host hands it as true.

A status is an unsigned 32 bits. `ANSWERED`, nought, is a function that answered; a computation that
ends without a value answers the reason it ended, one of the numbers the declarations name. The top
of the range, `0x7ffffff0` to `0x7fffffff`, is reserved for what is not a reason a Souther
computation ends: a host's implementation that threw (`HOST_EXCEPTION`), a capability with nothing
bound (`INJECTION_UNBOUND`), an implementation that answered what it may not
(`INJECTION_PROTOCOL_VIOLATION`), and a row's stand-in with nothing stated (`FAKE_NO_OUTPUT`). No
language abort is numbered there.

## What a host hands over

Every word a function takes is either a datum or a handle, and the record says which of the two each
word is. The two are held to different things.

A datum is anything a host may have wrong: a number, a count, a truth, bytes. A function answers
whatever datum it is handed, and one it does not take is refused by what the function answers, never
by ending the process. So `souther_string_of_utf8` answers false for a count below nought or bytes
that are not UTF-8; making a `Decimal`, a `Date`, a `Time`, a `DateTime` or an `Instant` from its
parts answers whether the parts name one, so a month of 13 or integer text that is no integer is
answered as that; reading a carried case out of a value of another case answers false; and building
a list of a count no list has answers false with nothing written. Bytes are read for as many as the
count says, where the count is above nought. A test hands every function a host calls data it has
wrong and fails where one ends the process
([`native/crates/runtime/src/contract.rs`](../native/crates/runtime/src/contract.rs)).

A handle is something only the library makes: a value, a string, a list, a decoded reading. A host
hands back only one the library answered, of the type the function names, while the scope it was
made in is open on the calling thread. That is a precondition and not something a function checks
for the host; a runtime that hands a function anything else has a mistake of its own, and the
process may end. A runtime keeps its handles where its host cannot forge them, which is what the
three runtimes here each do in their own way.

The input contract is the record's `input` lines.

## Scopes

A value lives in an arena, one to each thread, and nothing frees one on its own. A host brackets
what it does in a scope: `souther_scope_open` answers a token, and `souther_scope_close` with that
token drops everything made on the thread since it was opened. A value is good until the scope it
was made in closes, and only on the thread that made it.

Scopes nest, and close in the order they were opened. `souther_scope_close` closes only the
innermost scope open on the calling thread. A token that is not that one, whether it was never
answered, was closed already, is out of order or was opened on another thread, is answered false and
nothing changes. No token is answered twice, on any thread. The scope contract is the record's
`scope` lines.

## Storage a host lays out

A behavior a host implements, and a function value a host makes, are reached through room the host
lays out and keeps: `souther_capability`, `souther_hosted` and `souther_hosted_function`. Each is a
number of 64-bit slots, which the record's `storage` lines give with their alignment, and none of
them has fields a host reads or writes. A host takes room of that size, hands its address to the
function that fills it, and keeps it for as long as what it stands for may be called; nothing is
copied out of it and nothing is registered anywhere.

## What a host calls for a module

A host calls a function by a C identifier. The symbols one object built here calls in another carry
`.` and `$`, which no C compiler or FFI that reads C declarations can name, so what a host calls is
spelt apart: `souther<n>`, `<n>` being the ABI generation, then the module as `_m_<segment>` per
segment of its dotted name, then `_b_<behavior>`, `_v_<value>`, `_t_<type>`, or `_l_` and the shape
a list's element crosses in, or `_fn_` and the shape of a function value, and then what is done with
it. A name is written as it is where it is ASCII letters and digits, with `_` doubled and any other
character as `_u<hex>_`, its code point. So `shop.quote` is `souther11_m_shop_b_quote` at generation
10, and a behavior named `数量` is `..._b__u6570__u91cf_`. Inside a name `_` is only ever followed by
`_` or `u`, which keeps every spelling readable back to the names it was made from. The functions
that spell each name are in [`native/crates/abi/src/lib.rs`](../native/crates/abi/src/lib.rs), each
with what the function it names takes and answers.

A published behavior is `_b_<behavior>`, and a published value `_v_<value>`. Each is an entry of its
own for a host, which converts what a host hands over and calls the symbol another object calls, so
the day one of them takes a value in a form a host does not hand one over in, a host's entry still
takes what a host hands over.

For a published type, `_t_<Name>_construct` takes the fields and answers a status the way the type's
own constructor does, since it is that constructor it runs: a value whose clauses do not hold is
answered `InvariantNotHeld` and nothing is written. A type with no clause answers a status too, so a
clause added later does not change how a host calls it. `_f_<field>` writes a field through room and
answers nothing. A sum has `_case`, answering which of the cases the sum descends to the value is,
as its place among them counted from nought. `_decode` reads a value out of its external form as
bytes, and `_decode_host` out of a value a host built of ordered maps and wrote with every container
as an object. Each answers a status, which is not `ANSWERED` only where a clause the reading runs
ended without a value, and otherwise writes through room a reading the host asks what it came to
through the runtime's `souther_decoded_*` functions: a value, bytes that are not JSON and where they
stopped, or every issue found. A clause that does not hold is one of the issues. An issue says its
code, its message key, its path and its metadata (`souther_issue_meta`), a JSON object with a
member for each name Raoh gives the issue. Each value in it is written as the type it is in Raoh's
value model, an object of one member named for the type: `{"int":5}`, `{"decimal":"1.50"}` (the
decimal at its scale, as Java's `BigDecimal.toString` writes it), `{"string":"…"}` and
`{"list":[…]}`, and a value of the model an element `duplicates` lists as a boundary writes it, with
`{"bool":…}`, `{"record":{…}}` and `{"none":null}` beside those. A host makes each value that type
in its own Raoh and guesses nothing from the JSON, since plain JSON writes the `Decimal` 5 and the
`Int` 5 alike and a JSON reader takes `1.50` as the float 1.5. `_encode` writes a value in its
external form. A behavior answering a union no declaration names has
`_b_<behavior>_answer_case` beside its call, which says which of the union's cases the value is.

A primitive among the cases of a union is carried by the runtime, and a host makes one and reads it
back through the runtime's functions for that case, `souther_case_int_make` and
`souther_case_int_read` for an `Int` and the same for the other primitives; a case the language
gives holds nothing and is only made, as `souther_case_division_by_zero_make`. A read writes through
room and answers whether the value was that case. Those functions are the record's `case` lines.

## How a value crosses

How a value crosses is decided once, by the driver, as the shape it crosses in. An `Int` crosses as
64 bits, a `Bool` as a byte, and text and a value of a declared type or of a union as an address. A
`Decimal` crosses as an address of type `souther_decimal`, which a host makes with
`souther_decimal_of_parts`, handing its integer as integer text in bytes and its scale, and reads
back with `souther_decimal_unscaled` and `souther_decimal_scale`: the two numbers the language says
a `Decimal` is, and not its text, which would be one spelling among several. A `Date`, a `Time`, a
`DateTime` and an `Instant` cross each as an address of a type of its own (`souther_date`,
`souther_time`, `souther_datetime`, `souther_instant`), which a host makes of the numbers the value
means, each an `int64_t`: `souther_date_of_parts` takes a year, a month and a day,
`souther_time_of_parts` an hour, a minute and a second, `souther_datetime_of_parts` all six, and
`souther_instant_of_parts` the second from the epoch and the nanosecond within it. Each is read back
as those numbers (`souther_date_parts` and the same for the others). The runtime decides what a
value is; a binding that checks first does so only to say it in its own words.

An optional crosses as a presence and then what it holds: a function takes a byte and the words of
the value, which are ignored where the byte is nought, and a reader writes the byte, and the value
only where there is one. Each optional says so of itself, so an optional of an optional is two
presences, and absence at one depth is not absence at another. A tuple crosses as its members, one
after another. Every value a function hands a host is written through room, a room for each word,
and none is answered as the function's return: what a function answers is a status, a count, whether
an index is inside a list, whether a value was made, or which case a value is.

A list crosses as an address of type `souther_list`. A host builds one and reads one through
functions the object defines for each shape an element crosses in: `_l_<element>_construct`, taking
a count, a column of each word the element crosses as, and room for the list, and answering whether
it made one; `_l_<element>_length`; and `_l_<element>_at`, taking the list, an index and room for
the element and answering whether the index is inside the list. `<element>` is the shape spelt as a
word (`value`, `int`) or a mark and what it is made of: `o` for an optional, `t` and the count of
members for a tuple, `l` for a list, `f` and the count of what it takes for a function value, its
answer last. So a list of optional strings is `o_string`, two columns, and a list of pairs
`t2_int_string`. A list of one declared type is built through the same functions as a list of
another. What builds a list is there where something takes one from a host, and what reads one where
something hands one to a host, and not otherwise.

A function value crosses as an address of type `souther_function`. A host calls one through
`_fn_<shape>_call`, handing the value, what it takes and room for its answer, and is answered the
status the function answered. A host makes one of its own through `_fn_<shape>_implement`, handing
room laid out as `souther_hosted_function`, a function of the type `_fn_<shape>_implementation`, and
what that function is handed first, and is answered the value. As with a list, what calls one is
there where one is handed to a host, and what makes one where one is taken from a host.

Where a host has no way to a value, the library does not offer the function, and the model a
generator is handed says why and where in the value it stands: a type with no representation for a
host yet, a type with no value, or a union a host would be handed with nothing to say which case it
is.

## Behaviors a host implements

A behavior with no body that declares nothing to depend on is implemented by the host, and reached
through a capability of the host's implementation and nothing else. The host makes one through
`_b_<behavior>_implement`, handing room for a `souther_capability`, room for a `souther_hosted`, a
function of the type `_b_<behavior>_implementation`, and what that function is to be handed first.
The function takes that, then what the behavior takes and room for its answer, in the words a host
hands a published behavior, and answers a status. A behavior with a body that something may require
is made one through `_b_<behavior>_bind`, out of the capabilities of what it requires, and a
published behavior's call takes those capabilities first, null where it requires nothing. Two
implementations of one behavior are two capabilities, each reached by what it was handed, on any
thread.

An implementation answers `ANSWERED`, or `HOST_EXCEPTION` to say it threw and that the host kept
what it threw to throw again where the outermost call returns, since a host's exception cannot
unwind through generated code. Anything else it answers is `INJECTION_PROTOCOL_VIOLATION` by the
time a caller sees it. A call through a requirement bound to null answers `INJECTION_UNBOUND`.

PHP's FFI makes a new C entry each time a closure is handed to C and keeps it until the request
ends, so a runtime there makes the function pointer once for a behavior and tells its
implementations apart by what each is handed first. The layout above allows it everywhere.

## What a library exports, and how it is linked

What the library exports is what the header declares, and nothing else. A row's entry and a boundary
stay in the object, and so does every symbol one object built here calls in another. The runtime is
a static archive, which gives a link only what something asks it for, so the link names each
function a host calls as wanted. By hand, what the driver runs is:

    # macOS
    cc -dynamiclib -o libsouther.dylib -Wl,-install_name,@rpath/libsouther.dylib \
        -Wl,-exported_symbols_list,<list> -Wl,-u,_<symbol> ... souther.o libsouther_native_runtime.a \
        <what the archive needs>

    # Linux
    cc -shared -o libsouther.so -Wl,--version-script=<script> -Wl,--no-undefined \
        -Wl,-u,<symbol> ... souther.o libsouther_native_runtime.a <what the archive needs>

where the list and the script name every function the header declares. What the archive needs is the
system libraries Rust's standard library reaches on the target, which the runtime's `build.rs` asks
`rustc` for and writes beside the archive as `libsouther_native_runtime.link`, one argument to a
line. An archive with no such file beside it is refused, so ship the two together.

A library is one program, so it holds every build the program reaches: a build's object defines what
reads and builds a value of a type it declares, and another build calls that. Those objects are
handed to the driver with `--with <object>`, or to `NativeCompiler.library` beside the program. Each
object carries its own surface in a section of its own, and the declarations, the manifest and the
export list are written from what the objects carry. A module two of them carry is refused, and so
is an object that carries none, and a program missing a build it reaches is refused when it is
linked rather than when a host loads it.

## Loading a library

Every Souther library exports the same runtime functions, so a runtime cannot link one in: a link
could not say which of two libraries a call reaches. The three runtimes here each load a library by
path and look each function up through that handle, so two libraries in one process each keep their
own arena. A runtime tells two libraries apart by the address of their `souther_scope_open`, which
whatever works on one arena shares, so two loads of one file are one runtime.

A library is loaded, asked `souther_abi_generation`, and only then looked up further. Everything
else a runtime does with it follows the words, the input contract and the scope contract above.

A library keeps one thing beyond any scope: a pattern it read from the image the object carries,
so that the image is read once however often the pattern is matched. That is kept on the runtime's
heap and not in the object, so unloading the library does not drop it. A host that unloads a library
calls `souther_release` first, with no call into the library in flight on any thread; it drops what
the library kept and leaves it usable, and calling it again drops nothing. A host that keeps a
library loaded for as long as the process runs never has to call it. The release contract is the
record's `release` lines.

A loader hands the same library back for every load of one file, and unloads it only when the last
of them is closed, so "unloads" is the last close and not each. Two loads of one file share what it
keeps: released when one of them is closed, it drops a pattern a call through the other may be
matching on another thread. A host that loads one file more than once counts its loads and releases
the library only when it closes the last. The Rust runtime counts each `NativeLibrary` of a library
and calls it when the last is dropped, which unloads the library; the Go runtime unloads a library
only where loading it failed and nothing was called, and releases it then only where no other load
holds it; and the PHP runtime keeps a library for as long as the process runs.
