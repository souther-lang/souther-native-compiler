// A run is this thread's arena and is not lent to another thread, even for a while.

include!("../library.rs");

fn main() {
    let runtime = runtime();
    runtime
        .run(|run| {
            std::thread::scope(|threads| {
                threads.spawn(|| {
                    make(run);
                });
            });
        })
        .unwrap();
}
