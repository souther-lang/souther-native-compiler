//! What a run does to a library's arena, over a stand-in for a library: a runtime whose arena is a
//! count of what was made, and a native function that calls a host implementation back.

use souther_binding_runtime::{
    AlreadyRunning, Bound, Capability, Failure, Held, HostError, HostFailure, Hosted, Implemented,
    RawScope, Requirement, Run, Runtime, Status, Statuses,
};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::panic::{self, AssertUnwindSafe};
use std::ptr;

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

/// A library's runtime: its own arena, and `souther_scope_open` and `souther_scope_close` over it,
/// holding which scopes are open on the thread as the library's runtime does. Each one written out
/// here is a different pair of functions, so a different runtime.
macro_rules! library {
    ($name:ident) => {
        mod $name {
            use super::*;

            thread_local! {
                pub static ARENA: Cell<i64> = const { Cell::new(0) };
                pub static OPEN: RefCell<Vec<(i64, i64)>> = const { RefCell::new(Vec::new()) };
            }

            static TOKENS: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);

            pub extern "C" fn open() -> RawScope {
                let token = TOKENS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let stood = ARENA.with(Cell::get);
                OPEN.with(|it| it.borrow_mut().push((token, stood)));
                RawScope(token)
            }

            pub extern "C" fn close(scope: RawScope) -> i8 {
                let stood = OPEN.with(|it| {
                    let mut open = it.borrow_mut();
                    match open.last() {
                        Some(&(token, stood)) if token == scope.0 => {
                            open.pop();
                            Some(stood)
                        }
                        _ => None,
                    }
                });
                match stood {
                    Some(stood) => {
                        ARENA.with(|it| it.set(stood));
                        1
                    }
                    None => 0,
                }
            }

            pub fn runtime() -> Runtime {
                let statuses = Statuses::new(STATUSES).unwrap();
                // SAFETY: both are this library's, and are functions of this program.
                unsafe { Runtime::new(open, close, statuses) }
            }

            pub fn taken() -> i64 {
                ARENA.with(Cell::get)
            }

            pub fn make<'run>(run: &mut Run<'run, Runtime>) -> Held<'run, Runtime> {
                let at = ARENA.with(|it| {
                    it.set(it.get() + 1);
                    it.get()
                });
                // SAFETY: the arena answered it just now, after the run was opened.
                unsafe { run.held(at as usize as *const u8) }
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
                assert!(outer.word_in(inner).is_ok());
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

#[test]
fn a_value_another_runtime_made_is_handed_to_no_computation() {
    let orders = orders::runtime();
    let again = orders::runtime();
    let prices = prices::runtime();
    orders
        .run(|outer| {
            let made = orders::make(outer);
            again
                .run(|_| unreachable!("a second root run of one library is refused"))
                .unwrap_err();
            prices
                .run(|run| {
                    assert!(matches!(made.word_in(run), Err(Failure::Foreign)));
                })
                .unwrap();
            // Another handle on the same library is the same runtime.
            let (library, word) = made.own();
            assert!(std::ptr::eq(library, &orders));
            assert_eq!(made.word_in(outer).unwrap(), word);
        })
        .unwrap();
    let _ = again;
}

unsafe extern "C" fn bind(into: *mut Capability, _requirements: *const *const Capability) {
    // What a library writes is its own; a stand-in writes nothing, and nothing reads it.
    let _ = into;
}

unsafe extern "C" fn implement(
    _into: *mut Capability,
    _hosted: *mut Hosted,
    _implementation: *const c_void,
    _userdata: *mut c_void,
) {
}

#[test]
fn a_behavior_bound_to_what_another_runtime_made_is_refused_at_any_depth() {
    let orders = orders::runtime();
    let prices = prices::runtime();
    // SAFETY: the stand-ins read nothing and write nothing.
    unsafe {
        let own = Implemented::new(&orders, implement, ptr::null(), ());
        let foreign = Implemented::new(&prices, implement, ptr::null(), ());
        let fine = Bound::new(&orders, Some(bind), &[&own]);
        assert!(fine.requirements(&orders).is_ok());
        assert!(matches!(fine.requirements(&prices), Err(Failure::Foreign)));

        let mixed = Bound::new(&orders, Some(bind), &[&own, &foreign]);
        assert!(matches!(mixed.requirements(&orders), Err(Failure::Foreign)));
        // Bound in turn to the one bound to what another runtime made.
        let above = Bound::new(&orders, None, &[&mixed as &dyn Requirement]);
        assert!(matches!(above.requirements(&orders), Err(Failure::Foreign)));
        let clean = Bound::new(&orders, None, &[&fine as &dyn Requirement]);
        assert!(clean.requirements(&orders).is_ok());
    }
}
