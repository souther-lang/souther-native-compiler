// A value made in a nested run is not kept where the run outside it can reach it.

include!("../library.rs");

fn main() {
    let runtime = runtime();
    runtime
        .run(|run| {
            let mut kept = make(run);
            run.scope(|inner| {
                kept = make(inner);
            });
            compute(run, kept);
        })
        .unwrap();
}
