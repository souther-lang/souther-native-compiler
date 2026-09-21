//! Reads a checked program on stdin and writes an object on stdout.
//!
//! A process rather than a library the JVM calls into. A program crosses once per build, so there
//! is nothing here for an in-process call to make faster, and a panic on this side ends this
//! process rather than the one that started it.

use souther_native_driver::{NotLowered, ended, object_for};
use std::io::{Read, Write, stdin, stdout};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut document = String::new();
    if let Err(problem) = stdin().read_to_string(&mut document) {
        eprintln!("{problem}");
        return ExitCode::from(ended::BADLY);
    }

    match object_for(&document) {
        Ok(object) => match stdout().write_all(&object) {
            Ok(()) => ExitCode::from(ended::WITH_AN_OBJECT),
            Err(problem) => {
                eprintln!("{problem}");
                ExitCode::from(ended::BADLY)
            }
        },
        Err(problem) => {
            eprintln!("{problem}");
            // Which of the two it was, said as the number rather than left for the other side to
            // work out from the words.
            ExitCode::from(if problem.downcast_ref::<NotLowered>().is_some() {
                ended::NOT_LOWERED
            } else {
                ended::BADLY
            })
        }
    }
}
