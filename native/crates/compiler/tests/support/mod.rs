//! What more than one of these tests needs and none of them owns.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The runtime's static archive, built for these tests by these tests.
///
/// Not a path into whatever `target/` last held. The archive is the output of a different build
/// than the one running this test — `cargo test` builds the runtime as a library for Rust to link,
/// not as the archive a C linker takes — so a path to it would be linking against whatever a
/// separate `cargo build` last left there, or against nothing. This asks cargo for it, once per
/// test binary, into a directory of its own so it never waits on the build that is running it.
#[allow(dead_code)]
pub fn runtime() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let into = Path::new(env!("CARGO_TARGET_TMPDIR")).join("runtime");
        let built = Command::new(env!("CARGO"))
            .args([
                "build",
                "--quiet",
                "-p",
                "souther-native-runtime",
                "--target-dir",
            ])
            .arg(&into)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("cargo to build the runtime with");
        assert!(built.success(), "the runtime did not build");
        let archive = into.join("debug").join("libsouther_native_runtime.a");
        assert!(archive.is_file(), "no archive at {}", archive.display());
        archive
    })
}
