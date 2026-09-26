// A value a host implementation made is not kept past the implementation.

include!("../library.rs");

fn main() {
    let runtime = runtime();
    let made = runtime.host(|cx| make(cx)).unwrap();
    let _ = made.address();
}
