//! A library is asked its ABI generation before anything else is looked up in it, and one that
//! does not answer this crate's is refused as that.

use souther_binding_runtime::{ABI_GENERATION, LoadError, NativeLibrary};

/// A shared library every machine this runs on has, and that has no generation query: the C
/// library, as a library of generation 8 or earlier would be.
fn no_query() -> &'static str {
    if cfg!(target_os = "macos") {
        "/usr/lib/libSystem.B.dylib"
    } else {
        "libc.so.6"
    }
}

#[test]
fn a_library_that_says_no_generation_is_refused_as_that() {
    // SAFETY: loading the C library runs nothing a process has not already run.
    let refused = unsafe { NativeLibrary::load(no_query()) }.err();
    assert!(
        matches!(&refused, Some(LoadError::Generation { found: None, .. })),
        "{refused:?}"
    );
    let said = refused.unwrap().to_string();
    assert!(
        said.contains(&format!("generation {ABI_GENERATION}")),
        "{said}"
    );
}
