// A value good for longer is one good for less long, with no conversion written.

include!("../library.rs");

fn shorten<'long: 'short, 'short>(value: Held<'long, Runtime>) -> Held<'short, Runtime> {
    value
}

fn main() {
    let runtime = runtime();
    runtime
        .run(|run| {
            let outer = make(run);
            run.scope(|inner| {
                let _: Held<'_, Runtime> = shorten(outer);
                let _ = inner;
            });
        })
        .unwrap();
}
