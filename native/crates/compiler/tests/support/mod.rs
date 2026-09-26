//! What more than one of these tests needs and none of them owns.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// What the linker on this platform calls a symbol an object names: Mach-O writes an underscore
/// before every one and ELF writes none. The object carries whichever its format takes, so what
/// needs saying is only what a C declaration has to be written with to reach it.
///
/// Said for the two hosts these tests have been run on, and refused at compile time on any other,
/// rather than one of the two standing for every host that is not the other.
#[allow(dead_code)]
pub const PREFIX: &str = if cfg!(target_os = "macos") {
    "_"
} else if cfg!(target_os = "linux") {
    ""
} else {
    panic!("these tests link on macOS and Linux, and have not been run anywhere else")
};

/// `text`, a harness these tests compile, with what it cannot know written out: `PREFIX` as
/// [`PREFIX`], and `souther@` as `souther` and the ABI generation the `abi` crate says, so no
/// harness spells a generation and none has to change when one moves. A harness handed on without
/// this keeps an `@` in a name, which no C compiler takes.
#[allow(dead_code)]
pub fn harness(text: &str) -> String {
    text.replace("PREFIX", PREFIX).replace(
        "souther@",
        &format!("souther{}", souther_native_abi::ABI_GENERATION),
    )
}

/// The runtime's static archive, built for these tests by these tests.
///
/// Not a path into whatever `target/` last held. The archive is the output of a different build
/// than the one running this test — `cargo test` builds the runtime as a library for Rust to link,
/// not as the archive a C linker takes — so a path to it would be linking against whatever a
/// separate `cargo build` last left there, or against nothing. This asks cargo for it, once per
/// test binary, into a directory of its own so it never waits on the build that is running it.
///
/// Built with `--print native-static-libs`, and the file the runtime's build wrote beside the
/// archive is held to what `rustc` says of this very archive: that file is what every link passes
/// after the archive ([`runtime_arguments`]), so it has to be what the archive needs, whatever
/// the standard library or a dependency of the runtime came to ask of the system.
#[allow(dead_code)]
pub fn runtime() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let into = Path::new(env!("CARGO_TARGET_TMPDIR")).join("runtime");
        let built = Command::new(env!("CARGO"))
            .args([
                "rustc",
                "--quiet",
                "-p",
                "souther-native-runtime",
                "--lib",
                "--crate-type",
                "staticlib",
                "--target-dir",
            ])
            .arg(&into)
            .args(["--", "--print", "native-static-libs"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("cargo to build the runtime with");
        let said = String::from_utf8_lossy(&built.stderr);
        assert!(built.status.success(), "the runtime did not build: {said}");
        let archive = into.join("debug").join("libsouther_native_runtime.a");
        assert!(archive.is_file(), "no archive at {}", archive.display());
        let asked: Vec<&str> = said
            .lines()
            .find_map(|line| line.split_once("native-static-libs:"))
            .expect("rustc to say what the archive needs")
            .1
            .split_whitespace()
            .collect();
        let written = souther_native_driver::runtime_arguments(&archive)
            .expect("the requirements written beside the archive");
        let written: Vec<String> = written[1..]
            .iter()
            .map(|it| it.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            written, asked,
            "what the runtime's build wrote beside the archive is not what rustc says the archive \
             needs"
        );
        archive
    })
}

/// The archive, and after it what it needs linked with it: what every link of a test's executable
/// is handed in place of the archive's path alone, which is a link that works where the linker
/// happens to add the rest.
#[allow(dead_code)]
pub fn runtime_arguments() -> Vec<std::ffi::OsString> {
    souther_native_driver::runtime_arguments(runtime())
        .expect("the requirements written beside the archive")
}
