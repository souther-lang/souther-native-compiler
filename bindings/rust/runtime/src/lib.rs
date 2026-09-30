//! What every Rust binding of a Souther library runs on.
//!
//! A library keeps what a computation makes in an arena of its own, one for each thread, and a
//! run is a scope of it that the library is told to close, dropping what was made in it, when the
//! run ends. A value made in
//! a run is an address into that arena, good until then. The PHP runtime checks that at run time,
//! value by value (`Expired`, `RunOnAnotherFiber`, `NotTheInnermostRun`). Here it is held in the
//! types instead, and a program that would break it does not compile:
//!
//! - A root run is opened by [`run`] with a lifetime of its own that nothing made in it can be
//!   answered out of.
//! - A [`Run`] and everything made in it are neither `Send` nor `Sync`.
//! - What makes something in the arena takes the run mutably. A nested run is opened from a run
//!   ([`Run::scope`]), which borrows it for as long as the nested one is open, so nothing is made
//!   through a run while a run inside it is the innermost.
//! - A nested run is handed as a [`Scope`], whose type says the run outside outlives it. A
//!   [`Held`] value made outside is taken as one made inside, and never the other way.
//! - A host implementation called back from the library does not open a run. It borrows the
//!   innermost one, the one the call it was called back from was made in ([`host`]).
//!
//! What the types cannot see is which library a value is of: a lifetime says for how long a value
//! is good and not which arena it stands in, and runs of two libraries can be related by lifetimes
//! as two runs of one can. So a value holds the library that made it, and handing one to a
//! computation of another runtime is refused before the call ([`Failure::Foreign`]), as handing it
//! what stands for a behavior bound by another is. Two handles on one library are one runtime,
//! and a second root run of it on a thread is refused where it is opened ([`AlreadyRunning`]).
//!
//! A library is loaded by path at run time ([`NativeLibrary`]) and every function is looked up
//! through its handle, since every Souther library exports the same runtime functions.

mod bound;
mod decimal;
mod decoding;
mod failure;
mod keep;
mod native;
mod run;
mod temporal;

pub use bound::{BindFn, Bound, Capability, Hosted, ImplementFn, Implemented, Made, Requirement};
pub use decimal::{Decimal, NotADecimal};
pub use decoding::{Answered, Decoding};
pub use failure::{
    Abort, CallbackFailure, Failure, HostError, Status, Statuses, UnnamedStatus, crossing,
};
pub use keep::{FunctionImplementFn, HostedFunction};
pub use native::{
    ABI_GENERATION, Construction, LoadError, NativeLibrary, Reading, Word, Words, made,
};
pub use run::{
    AlreadyRunning, Held, HostFailure, Loaded, RawScope, Run, Runtime, Scope, ScopeCloseFn,
    ScopeOpenFn, host, implemented, run,
};
pub use temporal::{Date, DateTime, Instant, NotATemporal, Time};

/// Raoh, whose issues a reading answers, as this runtime was built against it.
pub use raoh;
