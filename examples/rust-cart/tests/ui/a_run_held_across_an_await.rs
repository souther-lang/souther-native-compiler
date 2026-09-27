// A run is made asynchronous, so that it could wait on something. What the closure answers is a
// future holding the run, which would outlive it, and be resumed on whichever thread polled it.
use model::com::example::cart::domain::UserId;
use model::Library;

fn main() {
    let library = unsafe { Library::load(rust_cart::library()) }.unwrap();
    let _pending = library
        .run(|run| async move {
            let _user = UserId::new(run, "u");
        })
        .unwrap();
}
