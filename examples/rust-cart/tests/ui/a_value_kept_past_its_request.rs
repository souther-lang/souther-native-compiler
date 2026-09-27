// A value of the model is kept after the run it was made in has ended, as a cache of the last
// user would keep one. The run's arena has been reset by then, and the value would point into it.
use model::com::example::cart::domain::UserId;
use model::{Construction, Library};

fn main() {
    let library = unsafe { Library::load(rust_cart::library()) }.unwrap();
    let mut last = None;
    library
        .run(|run| {
            if let Ok(Construction::Value(user)) = UserId::new(run, "u") {
                last = Some(user);
            }
        })
        .unwrap();
    println!("{}", last.unwrap().value());
}
