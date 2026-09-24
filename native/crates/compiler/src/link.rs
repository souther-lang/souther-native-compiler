//! Objects and the runtime linked into one shared library a host loads.
//!
//! The system's C compiler drives the system's linker, the way it does for every executable this
//! project's tests link. What is asked of it beyond that is two things, and both are about the
//! runtime being a static archive.
//!
//! An archive gives a link only what something already linked asks it for, and a function only a
//! host calls — taking a mark, asking a reading what it came to — is asked for by nothing in an
//! object. So every function a host calls is named to the linker as wanted, and what the archive
//! holds of them is taken whatever the objects call. Nothing else of the archive is forced in:
//! loading all of it would carry everything the Rust standard library put there into the library.
//!
//! And what the library exports is what a host calls and nothing else. The objects carry more:
//! every symbol one object built by this compiler reaches in another, every row's entry, and every
//! boundary. Those are still there to be called inside the library, and a host is not offered them.
//!
//! How each of those is said is the linker's, and a linker is named here only where this has been
//! run against it. Any other host is refused before anything is written, rather than handed the
//! flags of whichever linker it is not.

use anyhow::{Result, bail};
use std::fs;
use std::path::Path;
use std::process::Command;

/// A linker this knows how to ask for a shared library.
#[derive(Clone, Copy)]
pub(crate) enum Linker {
    /// Apple's, writing Mach-O.
    Darwin,
    /// GNU ld or one that reads its flags, writing ELF on Linux.
    Elf,
}

impl Linker {
    /// The linker of the host this runs on, where this knows it.
    pub(crate) fn of_this_host() -> Result<Linker> {
        if cfg!(target_os = "macos") {
            Ok(Linker::Darwin)
        } else if cfg!(target_os = "linux") {
            Ok(Linker::Elf)
        } else {
            bail!(
                "building a shared library for a host is not supported on {}: macOS and Linux \
                 are the hosts it has been run on",
                std::env::consts::OS
            )
        }
    }

    /// What the library is called.
    pub(crate) fn library(self) -> &'static str {
        match self {
            Linker::Darwin => "libsouther.dylib",
            Linker::Elf => "libsouther.so",
        }
    }

    /// What the linker calls a symbol a C declaration names: Mach-O writes an underscore before
    /// every one and ELF writes none.
    fn prefix(self) -> &'static str {
        match self {
            Linker::Darwin => "_",
            Linker::Elf => "",
        }
    }

    /// Links `objects` and `runtime` into a shared library at `into` exporting `exported`.
    pub(crate) fn shared_library(
        self,
        objects: &[&Path],
        runtime: &Path,
        exported: &[String],
        into: &Path,
    ) -> Result<()> {
        let prefix = self.prefix();
        let listed = into.with_extension("exported");
        let mut command = Command::new("cc");
        command.arg("-o").arg(into);
        match self {
            Linker::Darwin => {
                let mut list = String::new();
                for symbol in exported {
                    list.push_str(&format!("{prefix}{symbol}\n"));
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
            }
            Linker::Elf => {
                let mut script = String::from("{\n  global:\n");
                for symbol in exported {
                    script.push_str(&format!("    {symbol};\n"));
                }
                script.push_str("  local: *;\n};\n");
                fs::write(&listed, script)?;
                command
                    .arg("-shared")
                    .arg(format!("-Wl,--version-script={}", listed.display()))
                    // What the library leaves unresolved is a link that fails here, as it does for
                    // a Mach-O library, and not a load that fails in a host later.
                    .arg("-Wl,--no-undefined");
            }
        }
        for symbol in exported {
            command.arg(format!("-Wl,-u,{prefix}{symbol}"));
        }
        command.args(objects).arg(runtime);
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
}
