// A value made in a root run is not answered out of it.

include!("../library.rs");

fn main() {
    let runtime = runtime();
    let made = runtime.run(|run| make(run)).unwrap();
    let _ = made.address();
}
