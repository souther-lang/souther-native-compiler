//! What a run does to a library's arena, over a stand-in for a library: a runtime whose arena is a
//! count of what was made, and a native function that calls a host implementation back.

use souther_binding_runtime::{
    AlreadyRunning, Failure, HostError, HostFailure, RawMark, Run, Runtime, Status, Statuses, Value,
};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::panic::{self, AssertUnwindSafe};
use std::ptr::NonNull;

const ANSWERED: Status = 0;
const DIVISION_BY_ZERO: Status = 4;
const HOST_EXCEPTION: Status = 0x7fff_ffff;

/// What the stand-in numbers its statuses, as a manifest says them.
const STATUSES: &[(&str, Status)] = &[
    ("ANSWERED", ANSWERED),
    ("DIVISION_BY_ZERO", DIVISION_BY_ZERO),
    ("INJECTION_UNBOUND", 0x7fff_fffd),
    ("INJECTION_PROTOCOL_VIOLATION", 0x7fff_fffe),
    ("HOST_EXCEPTION", HOST_EXCEPTION),
];

/// A library's runtime: its own arena, and `souther_mark` and `souther_reset` over it. Each one
/// written out here is a different pair of functions, so a different runtime.
macro_rules! library {
    ($name:ident) => {
        mod $name {
            use super::*;

            thread_local! {
                pub static ARENA: Cell<i64> = const { Cell::new(0) };
            }

            pub extern "C" fn mark() -> RawMark {
                RawMark(ARENA.with(Cell::get))
            }

            pub extern "C" fn reset(mark: RawMark) {
                ARENA.with(|it| {
                    assert!(
                        mark.0 <= it.get(),
                        "a mark is never above where the arena stands"
                    );
                    it.set(mark.0);
                });
            }

            pub fn runtime() -> Runtime {
                let statuses = Statuses::new(STATUSES).unwrap();
                // SAFETY: both are this library's, and are functions of this program.
                unsafe { Runtime::new(mark, reset, statuses) }
            }

            pub fn taken() -> i64 {
                ARENA.with(Cell::get)
            }

            pub fn make<'run>(run: &mut Run<'run, Runtime>) -> Value<'run> {
                let at = ARENA.with(|it| {
                    it.set(it.get() + 1);
                    it.get()
                });
                let at = NonNull::new(at as usize as *mut u8).expect("the arena counts from one");
                // SAFETY: the arena answered it just now, after the run was opened.
                unsafe { run.value(at) }
            }
        }
    };
}

library!(orders);
library!(prices);

type Callback = extern "C" fn(*mut c_void) -> Status;

/// A native function that calls a host implementation back, as generated code does.
extern "C" fn native(callback: Callback, userdata: *mut c_void) -> Status {
    callback(userdata)
}

/// A host implementation's trampoline: the userdata is a closure over the runtime it is lent a run
/// of.
extern "C" fn trampoline(userdata: *mut c_void) -> Status {
    // SAFETY: every caller here hands a `&mut dyn FnMut() -> Status` it holds for the call.
    let implementation = unsafe { &mut *userdata.cast::<&mut dyn FnMut() -> Status>() };
    implementation()
}

fn call_back(
    run: &mut Run<'_, Runtime>,
    mut implementation: impl FnMut() -> Status,
) -> Result<(), Failure> {
    let mut implementation: &mut dyn FnMut() -> Status = &mut implementation;
    let userdata = (&raw mut implementation).cast::<c_void>();
    run.call(|| native(trampoline, userdata))
}

/// What a generated trampoline answers the library for what a host implementation came to.
fn answered<R>(host: Result<R, HostFailure>) -> Status {
    match host {
        Ok(_) => ANSWERED,
        Err(_) => HOST_EXCEPTION,
    }
}

#[test]
fn what_a_root_run_made_is_dropped_when_it_ends() {
    let runtime = orders::runtime();
    let made = runtime
        .run(|run| {
            orders::make(run);
            orders::make(run);
            orders::taken()
        })
        .unwrap();
    assert_eq!(made, 2);
    assert_eq!(orders::taken(), 0);
}

#[test]
fn a_nested_run_drops_what_it_made_and_nothing_made_outside_it() {
    let runtime = orders::runtime();
    runtime
        .run(|run| {
            let outer = orders::make(run);
            let inside = run.scope(|inner| {
                orders::make(inner);
                // A value made outside, handed to a computation inside.
                let _ = outer.address();
                orders::taken()
            });
            assert_eq!(inside, 2);
            assert_eq!(orders::taken(), 1);
        })
        .unwrap();
}

#[test]
fn a_run_that_panicked_is_dropped_and_closed() {
    let runtime = orders::runtime();
    let panicked = panic::catch_unwind(AssertUnwindSafe(|| {
        runtime.run(|run| {
            orders::make(run);
            panic!("the host gave up");
        })
    }));
    assert!(panicked.is_err());
    assert_eq!(orders::taken(), 0);
    assert_eq!(runtime.run(|_| ()), Ok(()));
}

#[test]
fn a_second_root_run_of_one_library_is_refused() {
    let runtime = orders::runtime();
    let again = orders::runtime();
    runtime
        .run(|_| {
            assert_eq!(runtime.run(|_| ()), Err(AlreadyRunning));
            assert_eq!(again.run(|_| ()), Err(AlreadyRunning));
        })
        .unwrap();
    assert_eq!(again.run(|_| ()), Ok(()));
}

#[test]
fn a_root_run_of_another_library_is_its_own() {
    let orders = orders::runtime();
    let prices = prices::runtime();
    orders
        .run(|run| {
            orders::make(run);
            prices
                .run(|run| {
                    prices::make(run);
                    assert_eq!((orders::taken(), prices::taken()), (1, 1));
                })
                .unwrap();
            assert_eq!((orders::taken(), prices::taken()), (1, 0));
        })
        .unwrap();
}

#[test]
fn a_status_is_answered_as_what_the_library_names_it() {
    let runtime = orders::runtime();
    runtime
        .run(|run| {
            assert!(run.call(|| ANSWERED).is_ok());
            match run.call(|| DIVISION_BY_ZERO) {
                Err(Failure::Abort(abort)) => {
                    assert_eq!(abort.name(), Some("DIVISION_BY_ZERO"));
                }
                other => panic!("answered {other:?}"),
            }
            match run.call(|| 77) {
                Err(Failure::Abort(abort)) => {
                    assert_eq!((abort.status(), abort.name()), (77, None));
                }
                other => panic!("answered {other:?}"),
            }
            // The library says a host implementation failed, and none did here.
            assert!(matches!(
                run.call(|| HOST_EXCEPTION),
                Err(Failure::ProtocolViolation)
            ));
        })
        .unwrap();
}

#[test]
fn a_host_implementation_makes_what_it_answers_in_the_run_it_was_called_from() {
    let runtime = orders::runtime();
    runtime
        .run(|run| {
            orders::make(run);
            let called = call_back(run, || {
                answered(runtime.host(|cx| {
                    orders::make(cx);
                    Ok(())
                }))
            });
            assert!(called.is_ok());
            // Not dropped when the implementation returned: the library still holds it.
            assert_eq!(orders::taken(), 2);
        })
        .unwrap();
    assert_eq!(orders::taken(), 0);
}

#[test]
fn a_host_implementation_opens_no_root_run_of_the_library_that_called_it() {
    let runtime = orders::runtime();
    let refused = RefCell::new(None);
    runtime
        .run(|run| {
            call_back(run, || {
                *refused.borrow_mut() = Some(runtime.run(|_| ()));
                ANSWERED
            })
            .unwrap();
        })
        .unwrap();
    assert_eq!(refused.into_inner(), Some(Err(AlreadyRunning)));
}

#[test]
fn a_host_implementations_failure_is_what_the_call_answers() {
    let runtime = orders::runtime();
    runtime
        .run(|run| {
            let called = call_back(run, || {
                answered(runtime.host(|_| -> Result<(), HostError> { Err("no price".into()) }))
            });
            match called {
                Err(Failure::Host(failure)) => assert_eq!(failure.to_string(), "no price"),
                other => panic!("answered {other:?}"),
            }
        })
        .unwrap();
}

#[test]
fn a_host_implementations_panic_is_raised_where_the_call_returns() {
    let runtime = orders::runtime();
    let status = Cell::new(None);
    let panicked = panic::catch_unwind(AssertUnwindSafe(|| {
        runtime.run(|run| {
            call_back(run, || {
                let host = runtime
                    .host(|_| -> Result<(), HostError> { panic!("the implementation gave up") });
                assert_eq!(host, Err(HostFailure::Failed));
                status.set(Some(answered(host)));
                HOST_EXCEPTION
            })
        })
    }));
    let payload = panicked.expect_err("the panic is raised again once the library returned");
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"the implementation gave up")
    );
    assert_eq!(status.get(), Some(HOST_EXCEPTION));
    assert_eq!(orders::taken(), 0);
}

#[test]
fn a_host_implementation_outside_any_call_is_lent_no_run() {
    let orders = orders::runtime();
    let prices = prices::runtime();
    assert_eq!(orders.host(|_| Ok(())), Err(HostFailure::OutsideCall));
    orders
        .run(|run| {
            call_back(run, || {
                // The call open is into `orders`, and `prices` has none to lend.
                assert_eq!(prices.host(|_| Ok(())), Err(HostFailure::OutsideCall));
                ANSWERED
            })
            .unwrap();
        })
        .unwrap();
}
