//! The object and the runtime linked into one shared library a host loads.
//!
//! The system's C compiler drives the system's linker, the way it does for every executable this
//! project's tests link. What is asked of it beyond that is two things, and both are about the
//! runtime being a static archive.
//!
//! An archive gives a link only what something already linked asks it for, and a function only a
//! host calls — taking a mark, asking a reading what it came to — is asked for by nothing in the
//! object. So every function a host calls is named to the linker as wanted, and what the archive
//! holds of them is taken whatever the object calls. Nothing else of the archive is forced in: loading all
//! of it would carry everything the Rust standard library put there into the library.
//!
//! And what the library exports is what a host calls and nothing else. The object carries more:
//! every symbol another object built by this compiler reaches, every row's entry, and every
//! boundary. Those are still there to be called inside the library, and a host is not offered them.

use anyhow::{Result, bail};
use std::fs;
use std::path::Path;
use std::process::Command;

/// What the library is called on this host.
pub(crate) const LIBRARY: &str = if cfg!(target_vendor = "apple") {
    "libsouther.dylib"
} else {
    "libsouther.so"
};

/// What the linker on this host calls a symbol a C declaration names: Mach-O writes an underscore
/// before every one and ELF writes none.
const PREFIX: &str = if cfg!(target_vendor = "apple") {
    "_"
} else {
    ""
};

/// Links `object` and `runtime` into a shared library at `into` exporting `exported`.
pub(crate) fn shared_library(
    object: &Path,
    runtime: &Path,
    exported: &[String],
    into: &Path,
) -> Result<()> {
    let listed = into.with_extension("exported");
    let mut command = Command::new("cc");
    command.arg("-o").arg(into);
    if cfg!(target_vendor = "apple") {
        let mut list = String::new();
        for symbol in exported {
            list.push_str(&format!("{PREFIX}{symbol}\n"));
        }
        fs::write(&listed, list)?;
        command
            .arg("-dynamiclib")
            .arg(format!(
                "-Wl,-install_name,@rpath/{}",
                into.file_name()
                    .expect("a library's path names a file")
                    .to_string_lossy()
            ))
            .arg(format!("-Wl,-exported_symbols_list,{}", listed.display()));
    } else {
        let mut script = String::from("{\n  global:\n");
        for symbol in exported {
            script.push_str(&format!("    {symbol};\n"));
        }
        script.push_str("  local: *;\n};\n");
        fs::write(&listed, script)?;
        command
            .arg("-shared")
            .arg(format!("-Wl,--version-script={}", listed.display()))
            // What the library leaves unresolved is a link that fails here, as it does for a
            // Mach-O library, and not a load that fails in a host later.
            .arg("-Wl,--no-undefined");
    }
    for symbol in exported {
        command.arg(format!("-Wl,-u,{PREFIX}{symbol}"));
    }
    command.arg(object).arg(runtime);
    let ran = command.output();
    let _ = fs::remove_file(&listed);
    let ran = ran?;
    if !ran.status.success() {
        bail!(
            "the linker did not make {}: {}",
            into.display(),
            String::from_utf8_lossy(&ran.stderr).trim()
        );
    }
    Ok(())
}
