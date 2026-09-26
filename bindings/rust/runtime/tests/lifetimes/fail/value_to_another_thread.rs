// A value stands in this thread's arena and is not handed to another thread.

include!("../library.rs");

fn main() {
    let runtime = runtime();
    runtime
        .run(|run| {
            let made = make(run);
            std::thread::scope(|threads| {
                threads.spawn(move || made.address());
            });
        })
        .unwrap();
}
