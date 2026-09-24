//! Reads a checked program on stdin and writes an object on stdout.
//!
//! A process rather than a library the JVM calls into. A program crosses once per build, so there
//! is nothing here for an in-process call to make faster, and a panic on this side ends this
//! process rather than the one that started it.
//!
//! `--library <directory>` writes, into the directory, the object, a C header, the declarations it
//! includes, a manifest and a shared library, and on stdout where it wrote each of them, one to a
//! line and in that order, so what a library is called on this host is said by the one that named
//! it. The library is the object; every object another Souther build wrote that `--with <object>`
//! names, once for each; and the runtime: the static archive the same `cargo build` put beside this executable, or the one
//! `--runtime <archive>` names.

use souther_native_driver::{Linking, NotLowered, ended, library_for, object_for};
use std::env;
use std::io::{Read, Write, stdin, stdout};
use std::path::PathBuf;
use std::process::ExitCode;

/// What the command line asked for.
enum Asked {
    /// The object, on stdout.
    Object,
    /// Everything a host needs, in a directory.
    Library { into: PathBuf, linking: Linking },
}

fn asked() -> Result<Asked, String> {
    let mut into = None;
    let mut runtime = None;
    let mut builds = Vec::new();
    let mut arguments = env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        let Some(value) = arguments.next() else {
            return Err(format!("{argument:?} names a path, and none followed it"));
        };
        let value = PathBuf::from(value);
        match argument.to_str() {
            Some("--library") => into = Some(value),
            Some("--runtime") => runtime = Some(value),
            Some("--with") => builds.push(value),
            _ => {
                return Err(format!(
                    "an argument this driver does not read: {argument:?}"
                ));
            }
        }
    }
    let Some(into) = into else {
        return if runtime.is_none() && builds.is_empty() {
            Ok(Asked::Object)
        } else {
            Err("--runtime and --with are read only with --library".to_string())
        };
    };
    let runtime = match runtime {
        Some(runtime) => runtime,
        None => env::current_exe()
            .map_err(|it| format!("where this driver is: {it}"))?
            .with_file_name("libsouther_native_runtime.a"),
    };
    Ok(Asked::Library {
        into,
        linking: Linking { builds, runtime },
    })
}

fn main() -> ExitCode {
    let asked = match asked() {
        Ok(asked) => asked,
        Err(problem) => {
            eprintln!("{problem}");
            return ExitCode::from(ended::BADLY);
        }
    };
    let mut document = String::new();
    if let Err(problem) = stdin().read_to_string(&mut document) {
        eprintln!("{problem}");
        return ExitCode::from(ended::BADLY);
    }

    let done = match asked {
        Asked::Object => {
            object_for(&document).and_then(|object| stdout().write_all(&object).map_err(Into::into))
        }
        Asked::Library { into, linking } => {
            library_for(&document, &linking, &into).and_then(|written| {
                let mut said = String::new();
                for path in [
                    &written.object,
                    &written.header,
                    &written.declarations,
                    &written.manifest,
                    &written.library,
                ] {
                    said.push_str(&format!("{}\n", path.display()));
                }
                stdout().write_all(said.as_bytes()).map_err(Into::into)
            })
        }
    };
    match done {
        Ok(()) => ExitCode::from(ended::WITH_AN_OBJECT),
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
