//! What every Rust binding of a Souther library runs on.
//!
//! A library keeps what a computation makes in an arena of its own, one for each thread, and a
//! run is a mark on it that the library is told to drop back to when the run ends. A value made in
//! a run is an address into that arena, good until then. The PHP runtime checks that at run time,
//! value by value (`Expired`, `RunOnAnotherFiber`, `NotTheInnermostRun`). Here it is held in the
//! types instead, and a program that would break it does not compile:
//!
//! - A root run is opened by [`Runtime::run`] with a lifetime of its own that nothing made in it
//!   can be answered out of.
//! - A [`Run`] and everything made in it are neither `Send` nor `Sync`.
//! - What makes something in the arena takes the run mutably. A nested run is opened from a run
//!   ([`Run::scope`]), which borrows it for as long as the nested one is open, so nothing is made
//!   through a run while a run inside it is the innermost.
//! - A nested run is handed as a [`Scope`], whose type says the run outside outlives it. A
//!   [`Value`] made outside is taken as one made inside, and never the other way.
//! - A host implementation called back from the library does not open a run. It borrows the
//!   innermost one, the one the call it was called back from was made in ([`Runtime::host`]).
//!
//! The one thing the types cannot see is two [`Runtime`]s over one library: two handles on one
//! arena, each able to open a root run. That is refused when the run is opened
//! ([`AlreadyRunning`]).

use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::panic::{self, AssertUnwindSafe};
use std::ptr::NonNull;

/// Where the arena stood, as `souther_mark` answers it and `souther_reset` takes it back.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawMark(pub i64);

/// `souther_mark`, as a library exports it.
pub type MarkFn = unsafe extern "C" fn() -> RawMark;

/// `souther_reset`, as a library exports it.
pub type ResetFn = unsafe extern "C" fn(RawMark);

/// One library's runtime: the functions that mark its arena and drop it back.
///
/// Which runtime this is is the address of its `souther_mark`. Whatever works on one arena has one
/// `souther_mark`, so two `Runtime`s with the same one are two handles on the same arena, however
/// the library was reached — without this crate depending on how a loader tells files apart. A way
/// of loading that let the runtime's symbols be interposed would have to look at this again.
pub struct Runtime {
    mark: MarkFn,
    reset: ResetFn,
}

impl Runtime {
    /// A runtime over a library's `souther_mark` and `souther_reset`.
    ///
    /// # Safety
    ///
    /// Both are the same library's, and both stay callable for as long as this `Runtime` lives.
    pub unsafe fn new(mark: MarkFn, reset: ResetFn) -> Self {
        Runtime { mark, reset }
    }

    fn identity(&self) -> usize {
        self.mark as usize
    }

    /// Opens a root run on this thread, hands it to `f`, and drops everything made in it once `f`
    /// has answered, or has panicked.
    ///
    /// `'run` is `f`'s own: nothing `f` answers can name it, so no value made in the run is used
    /// after the arena has dropped it.
    ///
    /// # Errors
    ///
    /// [`AlreadyRunning`] where a root run of this library is open on this thread, through this
    /// `Runtime` or another one over the same library. A run inside it is opened with
    /// [`Run::scope`], from the run it is inside.
    pub fn run<'lib, R>(
        &'lib self,
        f: impl for<'run> FnOnce(&mut Scope<'run, 'lib>) -> R,
    ) -> Result<R, AlreadyRunning> {
        let _open = Open::enter(self.identity())?;
        let _bracket = Bracket::open(self);
        Ok(f(&mut Scope::over(self)))
    }

    /// Runs a host implementation of a behavior, which the library has just called back, in the
    /// run the call it was called back from was made in.
    ///
    /// No run is opened: what `f` makes is answered to the library, which is still in the middle
    /// of the call, and is dropped with the run that call was made in. The run the call was made
    /// through stays borrowed by [`Run::call`] all the while, so `f` has no way to it but the
    /// [`Scope`] it is handed here.
    ///
    /// A panic in `f` does not unwind into the library. It is kept, and raised again where
    /// [`Run::call`] returns.
    ///
    /// # Errors
    ///
    /// [`HostFailure::Panicked`] where `f` panicked, and [`HostFailure::OutsideCall`] where the
    /// innermost call open on this thread is not one made into this library: nothing here called
    /// the library, so there is no run to lend and nowhere to keep a panic.
    pub fn host<'lib, R>(
        &'lib self,
        f: impl for<'run> FnOnce(&mut Scope<'run, 'lib>) -> R,
    ) -> Result<R, HostFailure> {
        let identity = self.identity();
        let inside = CALLS.with(|calls| {
            calls
                .borrow()
                .last()
                .is_some_and(|call| call.runtime == identity)
        });
        if !inside {
            return Err(HostFailure::OutsideCall);
        }
        let mut scope = Scope::over(self);
        panic::catch_unwind(AssertUnwindSafe(|| f(&mut scope))).map_err(|payload| {
            CALLS.with(|calls| {
                let mut calls = calls.borrow_mut();
                let call = calls
                    .last_mut()
                    .expect("the call a host was called back from is open until it returns");
                // The first is what the library was told of, and what it stopped for.
                call.caught.get_or_insert(payload);
            });
            HostFailure::Panicked
        })
    }
}

/// A run: what a computation is made in, taken mutably by whatever makes something.
///
/// `'run` is invariant, so a run is never taken for one that ends sooner or later than it does.
pub struct Run<'run> {
    runtime: &'run Runtime,
    _brand: PhantomData<fn(&'run ()) -> &'run ()>,
    _this_thread: PhantomData<*const ()>,
}

impl<'run> Run<'run> {
    /// Opens a run inside this one, hands it to `f`, and drops what was made in it once `f` has
    /// answered, or has panicked.
    ///
    /// This run is borrowed until then, so nothing is made through it while the one inside is
    /// open. A [`Value`] made in this run is still one `f` can read and hand to a computation it
    /// starts; one made inside cannot be answered out of `f`.
    pub fn scope<R>(&mut self, f: impl for<'inner> FnOnce(&mut Scope<'inner, 'run>) -> R) -> R {
        let _bracket = Bracket::open(self.runtime);
        f(&mut Scope::over(self.runtime))
    }

    /// Calls into the library: `native` is the call, made while this run is the innermost.
    ///
    /// A host implementation the library calls back meanwhile is lent this run through
    /// [`Runtime::host`], and a panic it raised is raised again here, once the library has
    /// returned.
    pub fn call<T>(&mut self, native: impl FnOnce() -> T) -> T {
        let call = OpenCall::enter(self.runtime.identity());
        let answer = native();
        if let Some(payload) = call.close() {
            panic::resume_unwind(payload);
        }
        answer
    }

    /// The value at `at`, as one made in this run.
    ///
    /// # Safety
    ///
    /// `at` is an address this library's arena answered after this run was opened, or one that
    /// was already good before it was and still is.
    pub unsafe fn value(&self, at: NonNull<u8>) -> Value<'run> {
        Value {
            at,
            _made_in: PhantomData,
        }
    }
}

/// A run as it is handed to a closure: the run, and a run outside it that it is known not to
/// outlive.
///
/// The second lifetime is what lets a value made outside be taken as one made inside. A closure
/// that is handed a `&mut Scope<'inner, 'outer>` may take `'outer` as outliving `'inner`, since the
/// type could not otherwise be formed. For a root run, what is outside is the borrow of the
/// [`Runtime`].
pub struct Scope<'run, 'outer> {
    run: Run<'run>,
    _within: PhantomData<&'run &'outer ()>,
}

impl<'run> Scope<'run, '_> {
    fn over(runtime: &'run Runtime) -> Self {
        Scope {
            run: Run {
                runtime,
                _brand: PhantomData,
                _this_thread: PhantomData,
            },
            _within: PhantomData,
        }
    }
}

impl<'run> Deref for Scope<'run, '_> {
    type Target = Run<'run>;

    fn deref(&self) -> &Run<'run> {
        &self.run
    }
}

impl<'run> DerefMut for Scope<'run, '_> {
    fn deref_mut(&mut self) -> &mut Run<'run> {
        &mut self.run
    }
}

/// An address in a library's arena, good for as long as `'run`.
///
/// Covariant in `'run`: a value made in a run is good in every run inside it.
#[derive(Clone, Copy, Debug)]
pub struct Value<'run> {
    at: NonNull<u8>,
    _made_in: PhantomData<&'run ()>,
}

impl Value<'_> {
    /// Where the value stands, to hand to the library.
    pub fn address(self) -> NonNull<u8> {
        self.at
    }
}

/// A root run was asked for where one of the same library is already open on this thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlreadyRunning;

impl fmt::Display for AlreadyRunning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "a run of this library is already open on this thread; a run inside it is opened \
             from it with `scope`",
        )
    }
}

impl std::error::Error for AlreadyRunning {}

/// Why a host implementation answered nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostFailure {
    /// It panicked; the panic is raised again where the call into the library returns.
    Panicked,
    /// It was called back with no call into this library open on this thread.
    OutsideCall,
}

/// A call into a library, open on this thread, and a host implementation's panic it will raise
/// again once the library has returned.
struct Call {
    runtime: usize,
    caught: Option<Box<dyn Any + Send>>,
}

thread_local! {
    /// The libraries with a root run open on this thread, by runtime.
    static OPEN: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };

    /// The calls into a library open on this thread, the innermost last.
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
}

/// A root run of one library, open on this thread until dropped.
struct Open(usize);

impl Open {
    fn enter(runtime: usize) -> Result<Self, AlreadyRunning> {
        OPEN.with(|open| {
            let mut open = open.borrow_mut();
            if open.contains(&runtime) {
                return Err(AlreadyRunning);
            }
            open.push(runtime);
            Ok(Open(runtime))
        })
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        OPEN.with(|open| {
            let mut open = open.borrow_mut();
            let at = open
                .iter()
                .rposition(|it| *it == self.0)
                .expect("a root run is open until its guard drops");
            open.remove(at);
        });
    }
}

/// A mark on the arena, dropped back to when this is.
struct Bracket<'lib> {
    runtime: &'lib Runtime,
    mark: RawMark,
}

impl<'lib> Bracket<'lib> {
    fn open(runtime: &'lib Runtime) -> Self {
        // SAFETY: `Runtime::new` was told the function is the library's and callable while the
        // runtime lives, which it does for as long as it is borrowed here.
        let mark = unsafe { (runtime.mark)() };
        Bracket { runtime, mark }
    }
}

impl Drop for Bracket<'_> {
    fn drop(&mut self) {
        // SAFETY: as in `open`; and the mark is the one this arena answered there, which no run
        // inside has dropped back past, since each of theirs was taken later and dropped sooner.
        unsafe { (self.runtime.reset)(self.mark) }
    }
}

/// A call into a library, on [`CALLS`] until it is closed or dropped.
struct OpenCall;

impl OpenCall {
    fn enter(runtime: usize) -> Self {
        CALLS.with(|calls| {
            calls.borrow_mut().push(Call {
                runtime,
                caught: None,
            });
        });
        OpenCall
    }

    /// Takes the call off and answers the panic a host implementation raised in it, if one did.
    fn close(self) -> Option<Box<dyn Any + Send>> {
        let caught = Self::pop();
        std::mem::forget(self);
        caught
    }

    fn pop() -> Option<Box<dyn Any + Send>> {
        CALLS.with(|calls| {
            calls
                .borrow_mut()
                .pop()
                .expect("a call is open until it is closed")
                .caught
        })
    }
}

impl Drop for OpenCall {
    /// Only where what made the call panicked before it closed.
    fn drop(&mut self) {
        Self::pop();
    }
}
