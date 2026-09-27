// A value of the model is handed to another thread. The arena it lives in is this thread's.
use model::com::example::cart::domain::UserId;
use model::{Construction, Library};

fn main() {
    let library = unsafe { Library::load(rust_cart::library()) }.unwrap();
    library
        .run(|run| {
            let Ok(Construction::Value(user)) = UserId::new(run, "u") else { return };
            std::thread::scope(|threads| {
                threads.spawn(move || user.value());
            });
        })
        .unwrap();
}
