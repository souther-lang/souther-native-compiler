# souther-native-compiler

Compiles a checked Souther program to a host-native object.

A behavior becomes a symbol in an object file the system linker takes, so the output runs where
there is no virtual machine and no wasm engine.

## What it reads

`CheckedProgram` and what is reachable from it, and nothing else of the Souther compiler. Anything
this needs that the program API does not carry is a question for Souther rather than something to
reach around, so it is raised there. This project is the second reader of that boundary, and what
it finds the boundary does not answer is the most useful thing it produces.

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

Over `Int` and `Bool`: `+`, `-`, `*`, unary `-`, the six comparisons, `&&` and `||`, `if`, and a
name for a value. `/` answers the exact quotient, which is a `Rational` and has no representation
here.

A model's own types: a product, a newtype, a unit, a sum, a value that may be absent, and several
values carried as one. Built, read a field off, and forked on which case a value is. A helper
arrives already written into the body that calls it, so a model that factors its arithmetic out
compiles without anything here knowing what a call is.

A type that states what its values owe is not built: a construction runs those clauses and stops at
the first that does not hold, and nothing here runs one.

Still ahead: a `String`, a `Decimal`, the collections, a function value, a kernel, and reaching a
behavior another build implements.

## Where a value lives

In an arena the caller brackets. Nothing frees a Souther value on its own: what a run makes is
dropped in one go by whoever bracketed the call, so generated code takes room and never gives any
back, and nothing it emits has to know what owns what.

A value of a declared type carries which type it is, so a fork on what a value is reads a number
rather than asking where the value came from. Everything a value is made of sits in a slot of one
width, which keeps a field's offset a fact about its position rather than about the types of the
fields before it.

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

This is not the two carriers compared against each other. Holding both to one statement is not
running both and comparing what came back, and running a program on every carrier and comparing the
answers is still ahead.
