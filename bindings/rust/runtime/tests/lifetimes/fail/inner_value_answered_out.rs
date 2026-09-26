// A value made in a nested run is not answered out of it.

include!("../library.rs");

fn main() {
    let runtime = runtime();
    runtime
        .run(|run| {
            let made = run.scope(|inner| make(inner));
            compute(run, made);
        })
        .unwrap();
}
