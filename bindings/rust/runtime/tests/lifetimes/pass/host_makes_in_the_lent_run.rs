// A host implementation makes values in the run it is lent, and opens runs inside it.

include!("../library.rs");

fn main() {
    let runtime = runtime();
    let _ = runtime.host(|cx| {
        let answer = make(cx);
        cx.scope(|inner| {
            compute(inner, answer);
        });
        answer.address()
    });
}
