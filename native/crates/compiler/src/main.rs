//! Reads a checked program on stdin and writes an object on stdout.
//!
//! A process rather than a library the JVM calls into. A program crosses once per build, so there
//! is nothing here for an in-process call to make faster, and a panic on this side ends this
//! process rather than the one that started it.

use anyhow::Result;
use souther_native_driver::object_for;
use std::io::{Read, Write, stdin, stdout};

fn main() -> Result<()> {
    let mut document = String::new();
    stdin().read_to_string(&mut document)?;
    let object = object_for(&document)?;
    stdout().write_all(&object)?;
    Ok(())
}
