// Nothing is made through a run while a run inside it is open.

include!("../library.rs");

fn main() {
    let runtime = runtime();
    runtime
        .run(|run| {
            run.scope(|_inner| {
                make(run);
            });
        })
        .unwrap();
}
