// Package souther is what every Go binding of a Souther library runs on.
//
// A library keeps what a computation makes in an arena of its own, one for each OS thread, and a
// run is a scope of it that the library is told to close, dropping what was made in it, when the
// run ends. A value made in a run is an address into that arena, good until then. The Rust runtime
// holds that in types. Go cannot, so a value keeps the run it was made in and is checked when it
// is used:
//
//   - A run holds its goroutine on one OS thread ([runtime.LockOSThread]) from the scope's
//     opening to its closing, and everything is made on that thread. No other goroutine runs on a
//     locked thread, so being on the run's thread is being in the run's goroutine.
//   - Using a value after its run ended, from another goroutine, or through a run that is not the
//     innermost one is a program that Rust does not compile. Here it panics with a [*Misuse].
//   - What Rust reports at run time is an error here as well: a value another runtime made
//     ([ErrForeignHandle]), and a second root run of one runtime on one thread
//     ([ErrAlreadyRunning]).
//   - Reading a value, and copying what a native call answered into the host's own memory, leave
//     no arena-owned value with the caller, and need the value's run to be live and on this
//     thread. Whatever makes a new value in the arena needs the run it is made in to be the
//     innermost.
//
// A binding gives each generated library a tag type B, and its [Run] and [Ref] are of that tag:
// a run of another generated binding is another type, and does not compile.
//
// A library's runtime is known by its identity, the address of its souther_scope_open. Whatever
// works on one arena has one, so two [Runtime]s with the same identity are two handles on one
// arena, however the library was reached.
package souther
