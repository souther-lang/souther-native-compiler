// A library as far as the compiler sees one: a runtime, something that makes a value, and a
// computation that takes one. Included by every case, none of which runs.

use souther_binding_runtime::{RawMark, Run, Runtime, Value};
use std::ptr::NonNull;

extern "C" fn mark() -> RawMark {
    RawMark(0)
}

extern "C" fn reset(_: RawMark) {}

#[allow(dead_code)]
fn runtime() -> Runtime {
    unsafe { Runtime::new(mark, reset) }
}

#[allow(dead_code)]
fn make<'run>(run: &mut Run<'run>) -> Value<'run> {
    unsafe { run.value(NonNull::dangling()) }
}

#[allow(dead_code)]
fn compute<'run>(_run: &mut Run<'run>, value: Value<'run>) -> Value<'run> {
    value
}
