//! The cart of `cart.sou`, served over HTTP.
//!
//! The rules are the model's and nowhere here. What is written in Rust is the two boundaries around
//! it: [`http`] reads a request into a behavior's arguments and writes back what it answered, and
//! [`db`] implements over SQLite the behaviors the model asks a host for. [`app`] binds the one to
//! the other for each request.

pub mod app;
pub mod db;
pub mod http;

pub use app::App;
pub use http::router;

use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::path::PathBuf;

/// The shared library bin/build wrote, under the name the platform gives it.
pub fn library() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("build/native")
        .join(format!("{DLL_PREFIX}souther{DLL_SUFFIX}"))
}
