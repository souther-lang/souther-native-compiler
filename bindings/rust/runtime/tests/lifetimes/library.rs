// A library as far as the compiler sees one: a runtime, something that makes a value, and a
// computation that takes one. Included by every case, none of which runs.

use souther_binding_runtime::{Held, RawMark, Run, Runtime, Statuses};

extern "C" fn mark() -> RawMark {
    RawMark(0)
}

extern "C" fn reset(_: RawMark) {}

#[allow(dead_code)]
fn runtime() -> Runtime {
    const STATUSES: &[(&str, u32)] = &[
        ("ANSWERED", 0),
        ("INJECTION_UNBOUND", 0x7fff_fffd),
        ("INJECTION_PROTOCOL_VIOLATION", 0x7fff_fffe),
        ("HOST_EXCEPTION", 0x7fff_ffff),
    ];
    unsafe { Runtime::new(mark, reset, Statuses::new(STATUSES).unwrap()) }
}

#[allow(dead_code)]
fn make<'run>(run: &mut Run<'run, Runtime>) -> Held<'run, Runtime> {
    unsafe { run.held(std::ptr::NonNull::dangling().as_ptr()) }
}

#[allow(dead_code)]
fn compute<'run>(_run: &mut Run<'run, Runtime>, value: Held<'run, Runtime>) -> Held<'run, Runtime> {
    value
}
