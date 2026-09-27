// A run a call is made through is not reached while the call is open, from a host implementation
// the library calls back or anything else: the call borrows it.

include!("../library.rs");

fn main() {
    let runtime = runtime();
    runtime
        .run(|run| {
            run.call(|| {
                make(run);
                0
            });
        })
        .unwrap();
}
