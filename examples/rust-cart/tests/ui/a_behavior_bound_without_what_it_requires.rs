// addItemToCart depends on loadProduct, loadCart and saveItem, and is bound to two of them. The
// library would answer INJECTION_UNBOUND on the call reaching the third; here there is no call.
use model::com::example::cart::domain::{
    AddItemToCart, LoadCartImplementation, LoadProductImplementation,
};
use model::Library;
use rusqlite::Connection;
use rust_cart::db::{SqlLoadCart, SqlLoadProduct};

fn main() {
    let library = unsafe { Library::load(rust_cart::library()) }.unwrap();
    let connection = Connection::open_in_memory().unwrap();
    let load_product = LoadProductImplementation::new(&library, SqlLoadProduct(&connection));
    let load_cart = LoadCartImplementation::new(&library, SqlLoadCart(&connection));
    let _add = AddItemToCart::bind(&library, &load_product, &load_cart);
}
