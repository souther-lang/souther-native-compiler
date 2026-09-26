//! How the runtime's archive is linked: the one way.
//!
//! An archive carries Rust's standard library, and which system libraries that needs is the target's
//! and the toolchain's to say. Linking it as a path and nothing more works where the linker adds
//! them anyway and fails on the first host that does not — the `libm` a big-integer crate reached
//! for was the first — and every place that links it and writes its own command line is one more
//! place that can. So the archive is linked through `runtime_arguments`, which reads what the
//! runtime's build wrote beside it, and this holds every place a Rust test links it to that. The
//! Java half's tests have their own (`TheJavaTestsLinkTheArchiveThroughItsRequirementsTest`), which
//! reads a Java source tree this crate is not built beside.

use std::fs;
use std::path::Path;

/// Every Rust file of a directory, and every file below it.
fn sources(directory: &Path, suffix: &str, into: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, suffix, into);
        } else if path.to_string_lossy().ends_with(suffix) {
            into.push(path);
        }
    }
}

/// A test that links a C executable hands the linker `runtime_arguments()`, and none hands it the
/// archive's path alone. A test that needs the path — to give the driver, which reads the file
/// itself — asks `runtime()` and does not pass it to `cc`.
#[test]
fn no_test_links_the_archive_as_a_path_alone() {
    let mut files = Vec::new();
    sources(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .as_path(),
        ".rs",
        &mut files,
    );
    let mut alone = Vec::new();
    for file in &files {
        if file.file_name().is_some_and(|it| it == "linking.rs") {
            continue;
        }
        for (at, line) in fs::read_to_string(file).unwrap().lines().enumerate() {
            let passed = line.contains(".arg(") || line.contains(".args([");
            if passed
                && (line.contains("runtime()") || line.contains("support::runtime"))
                && !line.contains("runtime_arguments")
            {
                alone.push(format!("{}:{}: {}", file.display(), at + 1, line.trim()));
            }
        }
    }
    assert!(
        alone.is_empty(),
        "linked as a path alone, and not through support::runtime_arguments(): {alone:#?}"
    );
}
