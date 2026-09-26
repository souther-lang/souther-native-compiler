//! A run, and what the types hold of it.

use crate::failure::{Caught, Failure, HostError, Status, Statuses};
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

/// One library's runtime: the functions that mark its arena and drop it back, and what its
/// statuses are numbered.
///
/// Which runtime this is is the address of its `souther_mark`. Whatever works on one arena has one
/// `souther_mark`, so two `Runtime`s with the same one are two handles on the same arena, however
/// the library was reached — without this crate depending on how a loader tells files apart. A way
/// of loading that let the runtime's symbols be interposed would have to look at this again.
pub struct Runtime {
    mark: MarkFn,
    reset: ResetFn,
    statuses: Statuses,
}

impl Runtime {
    /// A runtime over a library's `souther_mark` and `souther_reset`, answering the statuses
    /// `statuses` numbers.
    ///
    /// # Safety
    ///
    /// Both functions are the same library's, stay callable for as long as this `Runtime` lives,
    /// and `statuses` numbers what that library's functions answer.
    pub unsafe fn new(mark: MarkFn, reset: ResetFn, statuses: Statuses) -> Self {
        Runtime {
            mark,
            reset,
            statuses,
        }
    }

    fn identity(&self) -> usize {
        self.mark as usize
    }

    /// What each status the library answers is.
    pub fn statuses(&self) -> &Statuses {
        &self.statuses
    }

    /// A root run of this runtime alone, as [`run`] opens one over a library.
    ///
    /// # Errors
    ///
    /// As [`run`].
    pub fn run<'lib, R>(
        &'lib self,
        f: impl for<'run> FnOnce(&mut Scope<'run, 'lib, Runtime>) -> R,
    ) -> Result<R, AlreadyRunning> {
        run(self, f)
    }

    /// A host implementation run in the run it was called back from, as [`host`] runs one.
    ///
    /// # Errors
    ///
    /// As [`host`].
    pub fn host<'lib, R>(
        &'lib self,
        f: impl for<'run> FnOnce(&mut Scope<'run, 'lib, Runtime>) -> Result<R, HostError>,
    ) -> Result<R, HostFailure> {
        host(self, f)
    }
}

/// A library a binding loaded: what has a runtime, and what a run of it hands generated code.
pub trait Loaded {
    /// The runtime the library was built with.
    fn runtime(&self) -> &Runtime;
}

impl Loaded for Runtime {
    fn runtime(&self) -> &Runtime {
        self
    }
}

/// Opens a root run of `library` on this thread, hands it to `f`, and drops everything made in it
/// once `f` has answered, or has panicked.
///
/// `'run` is `f`'s own: nothing `f` answers can name it, so no value made in the run is used after
/// the arena has dropped it.
///
/// # Errors
///
/// [`AlreadyRunning`] where a root run of this library is open on this thread, through this handle
/// on it or another one. A run inside it is opened with [`Run::scope`], from the run it is inside.
pub fn run<'lib, L: Loaded, R>(
    library: &'lib L,
    f: impl for<'run> FnOnce(&mut Scope<'run, 'lib, L>) -> R,
) -> Result<R, AlreadyRunning> {
    let runtime = library.runtime();
    let _open = Open::enter(runtime.identity())?;
    let _bracket = Bracket::open(runtime);
    Ok(f(&mut Scope::over(library)))
}

/// Runs a host implementation of a behavior, which the library has just called back, in the run
/// the call it was called back from was made in.
///
/// No run is opened: what `f` makes is answered to the library, which is still in the middle of
/// the call, and is dropped with the run that call was made in. The run the call was made through
/// stays borrowed by [`Run::call`] all the while, so `f` has no way to it but the [`Scope`] it is
/// handed here.
///
/// Neither a panic in `f` nor a failure it answers unwinds into the library. Either is kept, and
/// [`Run::call`] raises the panic again or answers the failure once the library has returned.
///
/// # Errors
///
/// [`HostFailure::Failed`] where `f` panicked or answered a failure, and
/// [`HostFailure::OutsideCall`] where the innermost call open on this thread is not one made into
/// this library: nothing here called the library, so there is no run to lend and nowhere to keep
/// what went wrong.
pub fn host<'lib, L: Loaded, R>(
    library: &'lib L,
    f: impl for<'run> FnOnce(&mut Scope<'run, 'lib, L>) -> Result<R, HostError>,
) -> Result<R, HostFailure> {
    let identity = library.runtime().identity();
    let inside = CALLS.with(|calls| {
        calls
            .borrow()
            .last()
            .is_some_and(|call| call.runtime == identity)
    });
    if !inside {
        return Err(HostFailure::OutsideCall);
    }
    let mut scope = Scope::over(library);
    let caught = match panic::catch_unwind(AssertUnwindSafe(|| f(&mut scope))) {
        Ok(Ok(answer)) => return Ok(answer),
        Ok(Err(failure)) => Caught::Failed(failure),
        Err(payload) => Caught::Panicked(payload),
    };
    CALLS.with(|calls| {
        let mut calls = calls.borrow_mut();
        let call = calls
            .last_mut()
            .expect("the call a host was called back from is open until it returns");
        // The first is what the library was told of, and what it stopped for.
        call.caught.get_or_insert(caught);
    });
    Err(HostFailure::Failed)
}

/// A run: what a computation is made in, taken mutably by whatever makes something.
///
/// `'run` is invariant, so a run is never taken for one that ends sooner or later than it does.
pub struct Run<'run, L> {
    library: &'run L,
    _brand: PhantomData<fn(&'run ()) -> &'run ()>,
    _this_thread: PhantomData<*const ()>,
}

impl<'run, L: Loaded> Run<'run, L> {
    /// The library this is a run of.
    pub fn library(&self) -> &'run L {
        self.library
    }

    /// Opens a run inside this one, hands it to `f`, and drops what was made in it once `f` has
    /// answered, or has panicked.
    ///
    /// This run is borrowed until then, so nothing is made through it while the one inside is
    /// open. A [`Value`] made in this run is still one `f` can read and hand to a computation it
    /// starts; one made inside cannot be answered out of `f`.
    pub fn scope<R>(&mut self, f: impl for<'inner> FnOnce(&mut Scope<'inner, 'run, L>) -> R) -> R {
        let _bracket = Bracket::open(self.library.runtime());
        f(&mut Scope::over(self.library))
    }

    /// Calls into the library: `native` is the call, made while this run is the innermost, and
    /// what it answers is the status the call answered.
    ///
    /// A host implementation the library calls back meanwhile is lent this run through [`host`],
    /// and a panic it raised is raised again here, once the library has returned.
    ///
    /// # Errors
    ///
    /// The [`Failure`] the status says, where it is not the one saying the call answered: a host
    /// implementation's failure where it answered one.
    pub fn call(&mut self, native: impl FnOnce() -> Status) -> Result<(), Failure> {
        let runtime = self.library.runtime();
        let call = OpenCall::enter(runtime.identity());
        let status = native();
        let caught = call.close();
        runtime.statuses.answered(status, caught)
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
/// that is handed a `&mut Scope<'inner, 'outer, L>` may take `'outer` as outliving `'inner`, since
/// the type could not otherwise be formed. For a root run, what is outside is the borrow of the
/// library, so a value may hold on to the library for as long as its run.
pub struct Scope<'run, 'outer, L> {
    run: Run<'run, L>,
    _within: PhantomData<&'run &'outer ()>,
}

impl<'run, L> Scope<'run, '_, L> {
    fn over(library: &'run L) -> Self {
        Scope {
            run: Run {
                library,
                _brand: PhantomData,
                _this_thread: PhantomData,
            },
            _within: PhantomData,
        }
    }
}

impl<'run, L> Deref for Scope<'run, '_, L> {
    type Target = Run<'run, L>;

    fn deref(&self) -> &Run<'run, L> {
        &self.run
    }
}

impl<'run, L> DerefMut for Scope<'run, '_, L> {
    fn deref_mut(&mut self) -> &mut Run<'run, L> {
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
    /// It panicked or answered a failure, which the call into the library raises again or
    /// answers where it returns.
    Failed,
    /// It was called back with no call into this library open on this thread.
    OutsideCall,
}

/// A call into a library, open on this thread, and what a host implementation it called back
/// left for it to raise or answer once the library has returned.
struct Call {
    runtime: usize,
    caught: Option<Caught>,
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

    /// Takes the call off and answers what a host implementation it called back left, if one did.
    fn close(self) -> Option<Caught> {
        let caught = Self::pop();
        std::mem::forget(self);
        caught
    }

    fn pop() -> Option<Caught> {
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

impl Caught {
    /// Raises a panic again, and answers a failure as what the call comes to.
    pub(crate) fn raised(self) -> HostError {
        match self {
            Caught::Panicked(payload) => panic::resume_unwind(payload),
            Caught::Failed(failure) => failure,
        }
    }
}
