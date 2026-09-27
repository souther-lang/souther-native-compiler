//! `cargo run`, once `bin/build` has written the library and its binding, serves the cart on
//! `localhost:8080`, over the database `build/cart.sqlite` unless `CART_DATABASE` names another.

use std::path::{Path, PathBuf};

use model::Library;
use rusqlite::Connection;
use rust_cart::App;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    // SAFETY: the library bin/build wrote beside the binding this was compiled against.
    let library = unsafe { Library::load(rust_cart::library()) }?;
    let database = std::env::var_os("CART_DATABASE")
        .map_or_else(|| here.join("build/cart.sqlite"), PathBuf::from);
    let app = App::new(library, Connection::open(database)?)?;

    let listener = tokio::net::TcpListener::bind("localhost:8080").await?;
    axum::serve(listener, rust_cart::router(app)).await?;
    Ok(())
}
