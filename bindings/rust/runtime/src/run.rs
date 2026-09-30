//! A run, and what the types hold of it.

use crate::failure::{Caught, Failure, HostError, Status, Statuses};
use crate::keep::{FunctionImplementFn, Keeper};
use crate::native::Word;
use std::cell::RefCell;
use std::ffi::c_void;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::panic::{self, AssertUnwindSafe};
use std::ptr::NonNull;

/// An open scope of the arena, as `souther_scope_open` answers it and `souther_scope_close` takes
/// it back: a token, and never where the arena stands.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawScope(pub i64);

/// `souther_scope_open`, as a library exports it.
pub type ScopeOpenFn = unsafe extern "C" fn() -> RawScope;

/// `souther_scope_close`, as a library exports it: whether the scope was the innermost the thread
/// had open, which it closes only then.
pub type ScopeCloseFn = unsafe extern "C" fn(RawScope) -> u8;

/// One library's runtime: the functions that open a scope of its arena and close it, and what its
/// statuses are numbered.
///
/// Which runtime this is is the address of its `souther_scope_open`. Whatever works on one arena
/// has one `souther_scope_open`, so two `Runtime`s with the same one are two handles on the same
/// arena, however the library was reached — without this crate depending on how a loader tells
/// files apart. A way of loading that let the runtime's symbols be interposed would have to look at
/// this again.
pub struct Runtime {
    open: ScopeOpenFn,
    close: ScopeCloseFn,
    statuses: Statuses,
}

impl Runtime {
    /// A runtime over a library's `souther_scope_open` and `souther_scope_close`, answering the
    /// statuses `statuses` numbers.
    ///
    /// # Safety
    ///
    /// Both functions are the same library's, stay callable for as long as this `Runtime` lives,
    /// and `statuses` numbers what that library's functions answer.
    pub unsafe fn new(open: ScopeOpenFn, close: ScopeCloseFn, statuses: Statuses) -> Self {
        Runtime {
            open,
            close,
            statuses,
        }
    }

    pub(crate) fn identity(&self) -> usize {
        self.open as usize
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
///
/// # Safety
///
/// [`Loaded::runtime`] is the runtime of the library loaded, always the same one: a value is told
/// to be of a library by comparing what this answers, and a `Loaded` answering another's would
/// have a value of the one handed to the other's computation. A type answering it is one a binding
/// wrote around a loaded library, and nothing a caller implements.
pub unsafe trait Loaded {
    /// The runtime the library was built with.
    fn runtime(&self) -> &Runtime;
}

// SAFETY: a runtime is its own.
unsafe impl Loaded for Runtime {
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
    // Dropped last, once the arena no longer holds a value that reads what it keeps.
    let keeper = Keeper::default();
    let _open = Open::enter(runtime.identity())?;
    let _bracket = Bracket::open(runtime);
    Ok(f(&mut Scope::over(library, NonNull::from(&keeper))))
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
    let keeper = CALLS.with(|calls| {
        calls
            .borrow()
            .last()
            .filter(|call| call.runtime == identity)
            .map(|call| call.keeper)
    });
    let Some(keeper) = keeper else {
        return Err(HostFailure::OutsideCall);
    };
    // What the implementation makes that the library reads is kept by the run the call was made
    // in, which is still open, and not by anything that ends with the implementation.
    let mut scope = Scope::over(library, keeper);
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

/// What a generated implementation's function answers the library: [`host`] run over `f`, and
/// the status saying it answered, or the one saying it failed, whichever it came to.
pub fn implemented<'lib, L: Loaded>(
    library: &'lib L,
    f: impl for<'run> FnOnce(&mut Scope<'run, 'lib, L>) -> Result<(), HostError>,
) -> Status {
    let statuses = *library.runtime().statuses();
    match host(library, f) {
        Ok(()) => statuses.answered_status(),
        Err(_) => statuses.host_exception(),
    }
}

/// A run: what a computation is made in, taken mutably by whatever makes something.
///
/// `'run` is invariant, so a run is never taken for one that ends sooner or later than it does.
pub struct Run<'run, L> {
    library: &'run L,
    keeper: NonNull<Keeper>,
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
    /// This run is borrowed until then, so nothing is made through it while the one inside is open.
    /// A [`Held`] value made in this run is still one `f` can read and hand to a computation it
    /// starts; one made inside cannot be answered out of `f`.
    pub fn scope<R>(&mut self, f: impl for<'inner> FnOnce(&mut Scope<'inner, 'run, L>) -> R) -> R {
        let keeper = Keeper::default();
        let _bracket = Bracket::open(self.library.runtime());
        f(&mut Scope::over(self.library, NonNull::from(&keeper)))
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
        let call = OpenCall::enter(runtime.identity(), self.keeper);
        let status = native();
        let caught = call.close();
        runtime.statuses.answered(status, caught)
    }

    /// The function value `implement` makes of `dispatch`, which the library calls through
    /// `implementation`, kept with the room it is until this run ends; or the one made of the same
    /// `key` in this run already, where it was made through the same `implementation`.
    ///
    /// # Safety
    ///
    /// `implement` is this library's function making a function value of the shape
    /// `implementation` is of, and `implementation` reads what it is handed first as a `D`. `key`
    /// says which host function `dispatch` calls: two dispatches of one key call one function.
    pub unsafe fn host_function<D: 'static>(
        &mut self,
        implement: FunctionImplementFn,
        implementation: *const c_void,
        key: usize,
        dispatch: D,
    ) -> Word {
        // SAFETY: the keeper is the one of the run this is, or of the run it was lent from, and
        // that run is open for as long as this is reached; and what the caller says.
        unsafe {
            self.keeper
                .as_ref()
                .function(implement, implementation, key, dispatch)
        }
    }

    /// The value at `at`, as one this run's library made in this run.
    ///
    /// # Safety
    ///
    /// `at` is an address this library's arena answered after this run was opened, or one that
    /// was already good before it was and still is.
    pub unsafe fn held(&self, at: Word) -> Held<'run, L> {
        // SAFETY: what the caller says.
        unsafe { Held::new(self.library, at) }
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
    fn over(library: &'run L, keeper: NonNull<Keeper>) -> Self {
        Scope {
            run: Run {
                library,
                keeper,
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

/// A value in a library's arena: where it stands, good for as long as `'run`, and the library that
/// made it.
///
/// Covariant in `'run`: a value made in a run is good in every run inside it. A lifetime says for
/// how long a value is good and not which arena it stands in, and two libraries' runs can be
/// related by lifetimes as well as two runs of one: a value is handed to a computation only through
/// [`Held::word_in`], which refuses one another runtime made, and read only through the library
/// that made it ([`Held::own`]). There is no other way to the address.
pub struct Held<'run, L> {
    at: NonNull<u8>,
    library: &'run L,
}

impl<L> Clone for Held<'_, L> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<L> Copy for Held<'_, L> {}

impl<'run, L: Loaded> Held<'run, L> {
    /// The value at `at`, which `library` answered.
    ///
    /// # Safety
    ///
    /// `at` is an address `library` answered that stays good for as long as `'run`: made in a run
    /// open for that long, or read out of a value that is good for that long.
    ///
    /// # Panics
    ///
    /// Where `at` is null, which the library answers for no value.
    pub unsafe fn new(library: &'run L, at: Word) -> Self {
        let at = NonNull::new(at.cast_mut()).expect("the library answers a value's address");
        Held { at, library }
    }

    /// The library that made it.
    pub fn library(&self) -> &'run L {
        self.library
    }

    /// The library that made it and where the value stands, to read it through that library and
    /// no other: a field, a case, its external form.
    pub fn own(&self) -> (&'run L, Word) {
        (self.library, self.at.as_ptr().cast_const())
    }

    /// Where the value stands, to hand to a computation started in `run`.
    ///
    /// # Errors
    ///
    /// [`Failure::Foreign`] where another runtime than `run`'s made it: its address is one of
    /// another arena, which the computation would read as its own. Two handles on one library are
    /// one runtime, and a value one made is the other's.
    pub fn word_in(&self, run: &Run<'_, L>) -> Result<Word, Failure> {
        if self.library.runtime().identity() == run.library().runtime().identity() {
            Ok(self.at.as_ptr().cast_const())
        } else {
            Err(Failure::Foreign)
        }
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
    keeper: NonNull<Keeper>,
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

/// A scope of the arena, closed when this is dropped.
struct Bracket<'lib> {
    runtime: &'lib Runtime,
    scope: RawScope,
}

impl<'lib> Bracket<'lib> {
    fn open(runtime: &'lib Runtime) -> Self {
        // SAFETY: `Runtime::new` was told the function is the library's and callable while the
        // runtime lives, which it does for as long as it is borrowed here.
        let scope = unsafe { (runtime.open)() };
        Bracket { runtime, scope }
    }
}

impl Drop for Bracket<'_> {
    fn drop(&mut self) {
        // SAFETY: as in `open`.
        let closed = unsafe { (self.runtime.close)(self.scope) };
        // The scope is the innermost this thread has open, since every run inside was opened later
        // and is dropped sooner, and a run is on no other thread. The library refusing it is this
        // crate and the library disagreeing, and nothing a program did; where the thread is already
        // unwinding, a second panic would abort it, and the first one says more.
        if closed == 0 && !std::thread::panicking() {
            panic!("the library refused to close the innermost scope this crate opened");
        }
    }
}

/// A call into a library, on [`CALLS`] until it is closed or dropped.
struct OpenCall;

impl OpenCall {
    fn enter(runtime: usize, keeper: NonNull<Keeper>) -> Self {
        CALLS.with(|calls| {
            calls.borrow_mut().push(Call {
                runtime,
                keeper,
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
