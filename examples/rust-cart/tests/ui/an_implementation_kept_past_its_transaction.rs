// The transaction an implementation reads through is committed while the implementation can still
// be called by the library.
use model::com::example::cart::domain::LoadCartImplementation;
use model::Library;
use rusqlite::Connection;
use rust_cart::db::SqlLoadCart;

fn main() {
    let library = unsafe { Library::load(rust_cart::library()) }.unwrap();
    let mut connection = Connection::open_in_memory().unwrap();
    let transaction = connection.transaction().unwrap();
    let load_cart = LoadCartImplementation::new(&library, SqlLoadCart(&transaction));
    transaction.commit().unwrap();
    drop(load_cart);
}
