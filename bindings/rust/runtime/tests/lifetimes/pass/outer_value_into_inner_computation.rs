// A value made in a run is taken by a computation in a run inside it, however deep.

include!("../library.rs");

fn main() {
    let runtime = runtime();
    runtime
        .run(|run| {
            let outer = make(run);
            run.scope(|inner| {
                let middle = compute(inner, outer);
                inner.scope(|innermost| {
                    compute(innermost, outer);
                    compute(innermost, middle);
                });
            });
            // What was made in the run is still its own once the runs inside have ended.
            compute(run, outer);
        })
        .unwrap();
}
