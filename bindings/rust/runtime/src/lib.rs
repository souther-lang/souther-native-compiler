//! What every Rust binding of a Souther library runs on.
//!
//! A library keeps what a computation makes in an arena of its own, one for each thread, and a
//! run is a mark on it that the library is told to drop back to when the run ends. A value made in
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
//!   [`Value`] made outside is taken as one made inside, and never the other way.
//! - A host implementation called back from the library does not open a run. It borrows the
//!   innermost one, the one the call it was called back from was made in ([`host`]).
//!
//! The one thing the types cannot see is two handles on one library: two handles on one arena,
//! each able to open a root run. That is refused when the run is opened ([`AlreadyRunning`]).
//!
//! A library is loaded by path at run time ([`NativeLibrary`]) and every function is looked up
//! through its handle, since every Souther library exports the same runtime functions.

mod bound;
mod decimal;
mod failure;
mod keep;
mod native;
mod run;

pub use bound::{BindFn, Bound, Capability, Hosted, ImplementFn, Implemented, Made, Requirement};
pub use decimal::{Decimal, NotADecimal};
pub use failure::{Abort, Failure, HostError, Status, Statuses, UnnamedStatus};
pub use keep::{FunctionImplementFn, Held, HostedFunction};
pub use native::{Construction, LoadError, NativeLibrary, Reading, Word, Words};
pub use run::{
    AlreadyRunning, HostFailure, Loaded, MarkFn, RawMark, ResetFn, Run, Runtime, Scope, Value,
    host, implemented, run,
};

/// Raoh, whose issues a reading answers, as this runtime was built against it.
pub use raoh;
