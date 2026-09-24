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
`rust-toolchain.toml`, so a clone needs rustup and nothing else installed by hand. A C compiler is
needed too, by the test that links what came out and runs it.

## Where it runs

Unix hosts: what the object is written as is decided by the host's format, and Mach-O and ELF are
the two anything here has been run on. Windows is not refused anywhere — it is untried, and the
test that links and runs would have to say what a COFF object and its linker want before it meant
anything there.

## What compiles today

Over `Int` and `Bool`: `+`, `-`, `*`, the six comparisons, `&&` and `||`, `if`, and a name for a
value. `/` answers the exact quotient, which is a `Rational` and has no representation here. Unary
`-` of a literal is folded at compile time, and of anything else ends the run where it leaves the
range, the way `+`, `-` and `*` do.

An operation the language implements as a kernel: `Int.add` today, reusing the same instructions
`+` does. Every other kernel is still ahead — including everything a program does with text beyond
a literal, the six comparisons and `++`, which the language reaches through one. What a comparison
of two strings compares is neither of the two addresses and not the bytes either — the language
orders text by UTF-16 code unit, and says so of every carrier whatever one stores a string as.

A model's own types: a product, a newtype, a unit, a sum, a value that may be absent, and several
values carried as one. Built, read a field off, and forked on which case a value is.

Calls. A helper that does not call itself arrives already written into the body that calls it; one
that does is a definition the module holds, and every module that reaches it holds a copy, which is
what the language says a published helper is. A behavior reaching a behavior is a call whether this
program answers it or not: a behavior with no body is a name the object leaves for whoever links
it, so what answers a dependency is settled at the link and not arranged around the run.

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

A value of a declared type is built by one function per declaration, which every construction of
it calls, a unit's value among them: it takes the fields, runs the clauses in the order the type states them,
a clause a spread took in among them, and lays the value out only once all of them hold. The first
that does not hold ends the run with `InvariantNotHeld`, and a clause that itself ends without a
value, leaving an `Int`'s range, ends it for that reason.

The function belongs to the build that declared the type, the way the type's token does. That
build's object defines it for every type a body there builds and every type its modules publish,
since another build can name and build one of those, and a build constructing a value of a type
another declared calls that one: what a clause reads
and calls, a helper among them, is the declaring build's own, and a copy run elsewhere would run
without it. It is a call between objects this compiler built, under a symbol carrying the ABI
generation. What a host calls to build a value is a boundary of its own and not this.

A host builds and reads a value of a type the module publishes through functions the object
defines for it, and never through where the value keeps anything. A host holds a value as an
address it does not look behind, good until the mark taken before it was made is reset, and hands
it back to these and to the behaviors. For each published type with fields or none there is a
constructor, `souther2.<module>$type$<Name>$construct`, taking the fields and answering `status +
out` the way the type's own constructor does, since it is that constructor it runs: a value whose
clauses do not hold is answered `InvariantNotHeld` and nothing is written through `out`, and a type
with no clause answers a status too, so a clause added later does not change how a host calls it.
For each field there is a reader, `...$field$<field>`, answering the field itself. For a published
sum there is `...$case`, answering which of the cases the sum descends to the value is, as its
place among them counted from nought; the address the value is tagged with never leaves the object.

An `Int` crosses as 64 bits, a `Bool` as a byte, and text and a value of a declared type as an
address. An optional crosses as a presence beside the value: a constructor takes a byte and the
value, which is ignored where the byte is nought, and a reader answers the byte and writes the value
through a pointer only where there is one. How the generated code keeps an optional is not what a
host is told. What a host is handed is decided apart from what another object built by this
compiler reads the same way, because the two are different questions. A field of a type with no way
across yet has no reader, and keeps its type from having a host constructor, and nothing else: its
siblings are still read. A type the module keeps has none of these, whichever published sum it is a
case of.

A clause of a type the module keeps and nothing here builds, or one whose fields have no
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

Still ahead: a `Decimal`, the collections, a function value, every kernel but `Int.add`, a value
that runs in the module declaring it, an attempted
construction, which takes an arm by the clause that did not hold instead of ending the run, and a
behavior that declares what its answer owes, which is refused rather than answered without the
rule being run. A collection and
a `Decimal` are read off the program whole and refused where one would be laid out or written: how
every carrier orders a set's members, spells a map's keys and writes a decimal is for the language
to state before a backend writes one.


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
