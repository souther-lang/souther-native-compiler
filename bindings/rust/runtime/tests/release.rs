//! A library is released before it is unloaded: dropping the last `NativeLibrary` holding it calls
//! its `souther_release`, which drops what the library keeps beyond any scope, and then unloads it.

use souther_binding_runtime::{ABI_GENERATION, LoadError, NativeLibrary};
use std::path::{Path, PathBuf};
use std::process::Command;

/// `tests/release/released.c` built as a shared library in `into`, answering `generation`, and
/// with a `souther_release` or without one.
fn built(into: &Path, generation: u32, releases: bool) -> PathBuf {
    let library = into.join(if cfg!(target_os = "macos") {
        "released.dylib"
    } else {
        "released.so"
    });
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/release/released.c");
    let mut cc = Command::new("cc");
    cc.args(["-shared", "-fPIC", "-o"])
        .arg(&library)
        .arg(format!("-DABI={generation}"))
        .arg(&source);
    if !releases {
        cc.arg("-Dsouther_release=souther_release_is_not_here");
    }
    let status = cc.status().expect("a C compiler is there");
    assert!(status.success(), "{source:?} did not build");
    library
}

/// A directory of the test's own, which nothing else writes.
fn scratch(name: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!("souther-release-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&at).unwrap();
    at
}

#[test]
fn dropping_a_library_releases_it_once() {
    let at = scratch("drop");
    let released = at.join("released");
    // SAFETY: the variable is read by the stand-in only, and no other test of this binary sets it.
    unsafe { std::env::set_var("SOUTHER_RELEASED", &released) };
    let path = built(&at, ABI_GENERATION, true);

    // SAFETY: the stand-in runs nothing when it is loaded, and only its two functions are called.
    let library = unsafe { NativeLibrary::load(&path) }.expect("the stand-in loads");
    assert!(!released.exists(), "released before it was dropped");
    drop(library);
    assert_eq!(std::fs::read_to_string(&released).unwrap(), "released\n");

    // Two loads of one file are one library, which the loader unloads when the last is dropped:
    // released then and not before, since what one release drops a call through the other may be
    // reading.
    std::fs::remove_file(&released).unwrap();
    // SAFETY: as above.
    let one = unsafe { NativeLibrary::load(&path) }.expect("the stand-in loads");
    // SAFETY: as above.
    let other = unsafe { NativeLibrary::load(&path) }.expect("the stand-in loads again");
    drop(one);
    assert!(
        !released.exists(),
        "released while another load of it held it"
    );
    drop(other);
    assert_eq!(std::fs::read_to_string(&released).unwrap(), "released\n");
}

#[test]
fn a_library_without_release_is_refused_as_missing_it() {
    let at = scratch("missing");
    let path = built(&at, ABI_GENERATION, false);
    // SAFETY: as above.
    let refused = unsafe { NativeLibrary::load(&path) }.err();
    assert!(
        matches!(&refused, Some(LoadError::Missing { name, .. }) if name == "souther_release"),
        "{refused:?}"
    );
}
