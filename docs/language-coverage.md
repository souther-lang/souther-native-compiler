# What of the language compiles

For whoever writes a Souther program to run through this backend, or changes how a value is
lowered: which of the language compiles today, and how a value is represented while a run holds
it. What a host builds, reads and calls is in [host-abi.md](host-abi.md), and how the compiler is
built in [development.md](development.md).

## Numbers and truths

Over `Int` and `Bool`: `+`, `-`, `*`, the six comparisons, `&&` and `||`, `if`, and a name for a
value. `/` answers the exact quotient, which is a `Rational`. Unary `-` of a literal is folded at
compile time, and of anything else ends the run where it leaves the range, the way `+`, `-` and `*`
do.

A `Rational` is numerator × 2^twos × 5^fives / denominator, held by the runtime behind an address
that only it reads, so a `Decimal` enters by its scale and nothing is built from it. It is a value
a computation holds and a case a union can be, and no host is handed one: it has no external form,
so no behavior takes or answers one and the manifest names no way to make or read one. An operator
that reads its operands at their exact values (`Int + Rational`, `Decimal < Rational`) takes each as
the `Rational` it stands for, which is the operator's reading and not a conversion any position
that asks for a `Rational` gets. `/` reads its operands so whatever it divides. A zero divisor ends
the run, and so does an answer that has no place: an exponent past sixty-four bits, or a part wider
than a `Decimal`'s integer, asked of the answer once it is in its one form and not of what was
worked with on the way to it. An order or a rounding of values at exponents nothing can be built at
is answered from what is known of them, to as many bits as it takes. Two `Rational`s whose order
needs more room than the run has end it as an arena that has run out does.

## Kernels and text

Every operation the standard library of the pinned Souther declares as a kernel (an `intrinsic`)
is lowered: those over `Int`, `String`, `Decimal`, `Rational`, `Date`, `Time`, `DateTime`, `List`,
`Option`, `Set` and `Map`. What each takes, answers and can end a run for is one contract in
[`native/crates/compiler/src/kernels.rs`](../native/crates/compiler/src/kernels.rs), and a kernel
not in that table is refused as not lowered. `Int.add`, `Int.subtract` and `Int.multiply` are the
instructions `+`, `-` and `*` are; `Int.compare` and `Int.floorMod` are emitted beside them;
`Int.truncatingDivide` and `Int.truncatingRemainder` answer `Int | DivisionByZero` and end a
quotient only on the smallest `Int` over -1.

Everything that walks text is a call into the runtime, which hands the text to `souther-text`: that
crate is what the language says text means, over bytes alone, and where the wasm runtime is to read
it from as well (#17). A string is a sequence of Unicode scalar values kept as UTF-8, so two are
ordered by comparing their bytes, which are in scalar-value order; its length and every index count
code points. What builds a string — `++`, `append`, `join`, `replace`, `reverse`, `repeat`, the pads
and the case mappings — puts it in NFC. What Unicode says of text — NFC, case, `White_Space` — and
which text is a date, a time, a date-time or an instant are
[199x-notation](https://github.com/raoh-project/199x-notation)'s, the rules Souther and Raoh share,
implemented once per language: the runtime reads them from its Rust crate. `String.matches` and a
clause's pattern run what the checker read the pattern as: the compiler writes its machine as an
image with 199x-notation's Java library, the object carries the image, and the runtime reads it with
the Rust crate the first time the pattern is matched. Nothing here reads pattern text. A kernel
that can end a run for some of what it is handed — a slice the string has no room for, a zero
divisor to `floorMod`, a count `repeat` cannot make — ends it with the reason the call names, and
the runtime says only whether it answered.

## Decimals

A `Decimal` is an address, and what it points at is the runtime's alone: generated code, another
object and a host all hand the address over and never read behind it, so how the runtime keeps a
`Decimal` can change without anything being built again. Every operation on one is a call into the
runtime — a literal is made there from the integer and the scale the checker read it as, which the
object carries — and what each answers is the runtime's `amount` module: the scale a sum, a
difference and a product answer at, each of `RoundingMode`'s seven cases, `Decimal.divide`
answering `DivisionByZero` before it looks at the scale, `String.toDecimal` reading decimal text by
the grammar the language states and `String.fromDecimal` writing plain notation at the value's
scale. Equality and order are by amount, so `1.0` and `1.00` are equal. The integer is held in a
`u128` where it fits, which is nearly every amount, and every operation on it is machine arithmetic
there; `num-bigint` works it out only past that, inside the runtime's `magnitude` module, and a test
holds every `u128` path to what `num-bigint` answers for the same operands.

A result whose scale leaves the 32-bit range, or whose integer is wider than a JVM `BigInteger`
holds, ends the run where it is computed; a value a long way below the unit it is rounded to is
rounded from how many digits it has, without the power of ten its scale names. A plain notation
longer than a string holds ends the run, measured before any of it is written. A boundary writes a
`Decimal` as its amount and not at its scale — `1.50` is written `1.5` and `100.00` is written
`100`, an exponent spelt out into at most a thousand digits — and reads one at the scale the number
was spelt at. The cases of `RoundingMode` are declared by the language and at home in no module's
object: the runtime defines their tokens, and every object naming one imports it.

## Dates and times

A `Date`, a `Time`, a `DateTime` and an `Instant` are addresses in the same way, and what they point
at is the runtime's alone. Each holds what its `java.time` counterpart holds and nothing past it: a
`Date` the years -999999999 to 999999999, a `Time` and a `DateTime` whole seconds, an `Instant`
nanoseconds from -1000000000-01-01T00:00:00Z to +1000000000-12-31T23:59:59.999999999Z. What a
temporal means and which text names one are the Souther specification's; the calendar and that
grammar are implemented here, in the runtime alone
([`native/crates/runtime/src/temporal.rs`](../native/crates/runtime/src/temporal.rs)), and nowhere
else in the runtime reads either from scratch.

A literal is carried as the count the checker's own parse read it as — a `Date`'s day, a `Time`'s
second of the day, a `DateTime`'s second, an `Instant`'s second and nanosecond — and not as the text
it was written as: what admits a spelling is `TemporalText`'s grammar, read the same way for a
literal the checker elaborates and for a text a boundary is handed, and `java.time` only builds a
value from a text that grammar already admitted, rather than deciding on its own what a program may
say. The driver holds the count to what the type holds, and the runtime makes the value from it
where it is reached. Every operation is a call into the runtime, and equality and order are by the
day, the second or the moment a value names, so two made apart are equal where they name one. A
shift (`Date.addDays`, `addMonths` and `addYears`, `DateTime.addMinutes`, `addHours` and `addDays`)
that leaves what a type holds ends the run, the one reason for a count too large to add and for a
day past the end; `Date.fromParts` and `Time.fromParts` name a case for parts that name none and
normalise nothing.

A boundary writes a temporal as `toString` of its `java.time` class does — a time without its
seconds where they are nought, an instant in UTC — and reads it by the same `TemporalText` authority
a literal is checked against: `atBoundary`'s grammar, which a literal is held to as well, plus one
condition more a source `Instant` alone answers to, that its offset is spelled `Z` and nothing else
— so the sets are not equal, only decided by the one language either way. A text held to the second
refuses a fraction of one even where it is nought, since `09:30:00.000` and `09:30:00` name one
second once the point is read past and only the text still says which was sent, and an instant's end
of day, `24:00:00`, admits none of a minute, a second or a fraction after it for the same reason. An
instant is read from an offset spelling as the moment it names, and a leap second is refused.
`ATemporalAnswersWhatTheJvmAnswersTest` holds every kernel and comparison to what `java.time`
answers over the ends of every range and a seeded run of the rest.

## Unions and a model's own types

A value of a union says which case it is by the token at the front of it. A declared case's token is
its declaration's; an `Int`, a `Bool`, a `String`, a `Decimal`, a `Rational` or a temporal standing
as a case, and a case the language gives such as `DivisionByZero`, is carried with a token the
runtime defines for it, so a value of `Int | DivisionByZero` is told apart the way a value of a sum
is, and a case keeps its token when the union it stands in widens. Every one of those tokens is
resolved by the linker, a declaration's to the object of the build that declares it and the others
to the runtime every object links, so a union means the same in every object of a library: one
object hands a value of it to another, which forks on it and reads its case back out, and a
composition routes on its cases as it does on a sum's.

A model's own types: a product, a newtype, a unit, a sum, a value that may be absent, and several
values carried as one. Built, read a field off, and forked on which case a value is. Two values of
one type compare by what they are made of, through a comparator the object holds per type.

## Calls

A helper that does not call itself arrives already written into the body that calls it; one that
does is a definition the module holds, and every module that reaches it holds a copy, which is what
the language says a published helper is. A behavior reaching a behavior is a call whether this
program answers it or not. One another build implements is a name the object leaves for that build's
object. A behavior is called with a capability for each behavior it requires, in the order the
checker answered them, which is what it was constructed with, and a call it makes through `depends
on` goes through that capability: whatever the capability holds answers it, a body, a host's
implementation or a row's stand-in, and never what the name would recover. One with no body that
declares nothing to depend on is reached only that way. The object of the build that declares it
makes the capability of what a host implements it as, so what answers a dependency is the host's to
choose when it runs the program and not something linked in. What constructing a behavior requires
is on its target wherever it is reached, one another build implements included, and a composition
hands each stage the capabilities of what the stage requires, picked out of its own.

A value a module declares runs in that module's object, which is its one home, and another build
reaches a published value through an entry that object defines for it. What a behavior declares of
its answer (`ensures`) is held where the checker placed the check: where the behavior answers, for
one whose body is here, and where the answer crosses into this object's code, for one whose answer
arrives from outside. An answer that does not keep it ends the run with `EnsuresNotHeld`.

What a call may reach is wider than what the object defines — a body may name a behavior, or a
type, that a module built before this one declares — so the document says the two apart.

What the object's symbol table carries is what the declaring module publishes, read together with
what this object is for. The object is one whole program, so a name the module keeps is reached
inside it and nowhere else, and a name it publishes may be reached by a host linking the object in
or by another Souther build's object. The two are different questions: a module's `exposing` clause
is its surface in the language, and an object that read that as its own answer would be publishing
whatever surface suited the shape it happened to be built in.

A behavior the object does not define is reached over any type the checker settled: numbers,
truths, text, a `Decimal`, a `Rational`, a temporal, a value of a model's own types, a union, an
optional, a tuple, a list, a set, a map and a function value. A function value one object made is
called by another through its header and nothing else: the code at its head, handed the value
itself and then what the function takes. What the value captured is read by that code alone, which
the object that made the value wrote, so two objects agree on a function value where they agree on
what it takes and what it answers. A value of a declared type says which type it is with the address
of a byte its declaration owns, under a name the linker resolves, so a fork in one object over a
value built in another compares what the linker resolved for both. A value of a type variable no
call settles does not cross, and the signature is where that is said.

## Constructing a value

A type that states what its values owe is built by one function per declaration, which every
construction of it calls: it takes the fields, runs the clauses in the order the type states them,
a clause a spread took in among them, and lays the value out only once all of them hold. The first
that does not hold ends the run with `InvariantNotHeld`, and a clause that itself ends without a
value, leaving an `Int`'s range, ends it for that reason. A construction with no clause to run, a
unit's value or a value of a type of this compile that states none, is laid out where it stands by
the same code the function lays a value out with, since there is nothing for a call to run and
nothing the construction can end for.

The function belongs to the build that declared the type, the way the type's token does. That
build's object defines it for every type a body there builds through it, every type its modules
publish, since another build can name and build one of those, and every type a value of a published
one is read through, kept or not, since reading one builds it. A build constructing a value of a
type another declared calls that one: what a clause reads and calls, a helper among them, is the
declaring build's own, and a copy run elsewhere would run without it. It is a call between objects
this compiler built, under `souther<n>.<module>$construct$<Name>`, where `<n>` is the ABI
generation. What a host calls to build a value is a boundary of its own and not this; it is
described in [host-abi.md](host-abi.md).

The constructor is one reading of a function under it that decides the construction and answers
which clause did not hold, as its place among the type's, beside the status. The constructor turns
that place into `InvariantNotHeld`, a reader into an issue at the value's path, and an attempted
construction into the arm naming the clause: `guard PendingItem { ... } as pending else |
withinCapacity -> CartFull` builds the value and goes on where every clause holds, and answers
`CartFull` where `withinCapacity` does not. `else e` on its own answers every clause; beside arms
naming clauses, `| _ ->` answers the clauses that have no name and no other, as the checker holds
it. A clause that itself ends without a value still ends the run. The deciding function is the
declaring build's too, reached from another build under `souther<n>.<module>$checked$<Name>`, so an
attempt of a type another build declares runs that build's clauses and takes its arm here. What
crosses to the attempting build is what each clause is answered under, in the order the clauses
run, and nothing of what they say.

A clause of a type the module keeps and nothing here builds or reads, or one whose fields have no
representation here, is read, and refused if the two halves disagree about it, and is not run: no
value of the type is built here to run it over.

## The external form

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
have a form here there is a decoder, taking JSON as bytes, and an encoder, taking a value and
answering its JSON; how a host calls them is in [host-abi.md](host-abi.md). The decoder is the
encoder walked backwards over the same shapes, one reader per declaration: a field left out is
absent where it may be and missing where it may not, `null` is absence where there is no key, a case
is told apart by the key and the name the program carries for it, and a member the declaration does
not name is not read. What was read is built by the one function every construction of the type
goes through, so a value read is one its clauses hold of, checked in the order they are declared. A
decoder answers a status where a clause ended without a value, and otherwise a reading the host asks
what it came to: a value, the bytes not being JSON and where they stopped, or every issue found in
the document — not the first — each with one of Raoh's codes, the message key a resolver words it by
(the code where Raoh gives none of its own), a JSON Pointer and its metadata as the JSON object it
is, a `Decimal` in it at its scale.

A newtype's clause the checker states as a standard constraint is reported as that constraint, with
the code, key and metadata the JVM's decoder reports for it (`too_short` with `min` and `actual`,
`out_of_range` under `out_of_range.non_negative`), each of a clause's constraints asked in the order
they are written. A clause no constraint states, the part of one a constraint does not, and a
product's clause are `invariant_violation` at the value's path, naming the type's module and name
and the clause where it has one. A value of a type another build declares is read by that build's
object, under `souther<n>.<module>$read$<Name>`, whatever kind of type it is: how a declaration is
read is the declaring build's, and for a type built from fields that build is also the only one that
can say which clause did not hold. Text read is canonicalized to NFC. What JSON is, is
`souther-json-syntax`, a crate that knows no Souther type, no arena and no runtime, written to be
what both runtimes read once #17 moves it.

## Lists, sets and maps

A `List` is laid out inside a run as its length and then its elements, one slot each. Every list
kernel the standard library declares (the `intrinsic`s in `souther/list.sou`) is lowered:
`List.length`, `List.get`, `List.find`, `List.sort`, `List.sortBy`, `List.max`, `List.min`,
`List.reverse`, `List.sum`, `List.product` and `List.rangeInclusive`. `List.fold` is not one of
them: it is an ordinary recursive helper over `List.get`, as `souther/list.sou` writes it, and so
are the combinators written over it. In the external form a list is an array of its elements, and a
mistake inside one is answered at the element's index (`/lines/2/quantity`). Two lists compare
element by element.

A `Set` and a `Map` are the runtime's: a persistent hash trie in the arena, which generated code
holds the address of and never reads behind. What a member hashes to and what it is equal to are
handed to the runtime by the site that asks, for the type it asks at, so a set of a sum's case read
as a set of the sum is asked of under the sum. Two sets are equal where their members are, whatever
order each keeps them in, which is no order the language says anything of. At a boundary a set is
an array of its members and a map an object, each written in ascending order of what it holds, as
the language fixes (spec §collections); a map's key is read as the key's own type, and two keys
that are one once read are refused at the second as `duplicate_key`.

## Where a value lives

In an arena the caller brackets. Nothing frees a Souther value on its own: what a run makes is
dropped in one go by whoever bracketed the call, so generated code takes room and never gives any
back, and nothing it emits has to know what owns what. The arena is the calling thread's, and a
host brackets its calls with a scope it opens and closes, as [host-abi.md](host-abi.md) describes.

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
the writer says so. Either way it is
[`NotLowered`](../compiler/src/main/java/souther/nativecode/NotLowered.java), which is not what a
program the language refuses gets.