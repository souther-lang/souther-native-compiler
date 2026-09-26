//! What the runtime's static archive needs linked beside it, written where the archive is.
//!
//! A static archive holds no libraries, and one that carries Rust's standard library needs some:
//! which ones is the target's and the toolchain's to say, and only `rustc` says it, when it makes
//! a static library (`--print native-static-libs`). Linking the archive without them works where
//! the linker supplies them anyway (macOS) and while nothing linked in reaches them, and fails
//! with undefined references the day something does: the `libm` a big-integer crate reached for
//! was the first, and a runtime that took a thread or a `dlopen` would be the next.
//!
//! So the archive's link requirements are part of the artifact, in a file of their own beside it
//! (`libsouther_native_runtime.link`), one argument to the linker to a line, and everything that
//! links the archive — the driver, and every test that links an executable — reads that file and
//! passes what it says after the archive. Nothing writes the list of libraries by hand.
//!
//! The list is asked of `rustc` for a static library of the same target and panic strategy that
//! links the same standard library, since it is the standard library's requirements that make it
//! up: the runtime's other dependencies name no native library of their own, and the tests hold
//! the file to what `rustc` says of the runtime's own archive, so the day one does the tests fail.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo to say where its output goes"));
    // `<target dir>/<profile>/build/<package>-<hash>/out`: the profile's directory, where cargo
    // puts the archive, is three above. Refused where the layout is another, rather than writing
    // a file nothing finds.
    let profile = out
        .ancestors()
        .nth(3)
        .filter(|_| out.ancestors().nth(2).and_then(|it| it.file_name()) == Some("build".as_ref()))
        .expect("cargo's output directory to stand at <profile>/build/<package>/out");

    let probe = out.join("probe.rs");
    fs::write(&probe, "pub fn probe() {}\n").expect("the probe written");
    let mut rustc = Command::new(env::var_os("RUSTC").expect("cargo to say which rustc"));
    rustc
        .args(["--crate-type", "staticlib", "--crate-name", "souther_probe"])
        .args(["--edition", "2024", "--print", "native-static-libs"])
        .arg("--target")
        .arg(env::var_os("TARGET").expect("cargo to say the target"))
        .arg("-C")
        .arg(format!(
            "panic={}",
            env::var("CARGO_CFG_PANIC").expect("cargo to say the panic strategy")
        ))
        .arg("--out-dir")
        .arg(&out)
        .arg(&probe);
    let ran = rustc.output().expect("rustc to run");
    assert!(
        ran.status.success(),
        "rustc did not make the probe: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let said = String::from_utf8_lossy(&ran.stderr);
    let libraries = native_static_libs(&said)
        .expect("rustc to say which native libraries a static library needs");

    let mut written = String::new();
    for argument in libraries {
        written.push_str(argument);
        written.push('\n');
    }
    fs::write(profile.join("libsouther_native_runtime.link"), written)
        .expect("the link requirements written beside the archive");
}

/// The arguments the `native-static-libs` note says to link, in the order it says them.
fn native_static_libs(said: &str) -> Option<Vec<&str>> {
    let note = said
        .lines()
        .find_map(|line| line.split_once("native-static-libs:"))?;
    Some(note.1.split_whitespace().collect())
}
