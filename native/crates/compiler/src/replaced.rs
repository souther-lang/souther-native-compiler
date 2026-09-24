//! A directory that is what one build wrote, and nothing else.
//!
//! What a build hands a host is five files read together: the header and the declarations name the
//! functions the library exports, and the manifest describes them. Written one after another into
//! where the last build's stand, a build that failed part of the way (a module carried twice, a
//! link refused) would leave this build's object and manifest beside the last one's library. So a
//! build is written beside where it goes and put in place whole once everything in it is written,
//! and what it replaces has to be a build's own: a directory holding anything else is refused
//! rather than deleted.

use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Where a build bound for `target` is written before it is put there.
pub(crate) struct Replacing {
    target: PathBuf,
    staging: PathBuf,
}

impl Replacing {
    /// Staging beside `target`, which may hold nothing but `owned`, the files a build writes.
    pub(crate) fn beside(target: &Path, owned: &[&str]) -> Result<Replacing> {
        if target.exists() {
            for entry in fs::read_dir(target)? {
                let name = entry?.file_name();
                if !owned.iter().any(|it| name == *it) {
                    bail!(
                        "{} holds {}, which no build writes, and a build replaces the directory \
                         it is written to whole",
                        target.display(),
                        name.to_string_lossy()
                    );
                }
            }
        }
        let parent = match target.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        };
        fs::create_dir_all(&parent)?;
        let staging = parent.join(format!(
            ".{}.writing-{}-{}",
            target
                .file_name()
                .context("a build is written to a directory with a name")?
                .to_string_lossy(),
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |it| it.as_nanos())
        ));
        fs::create_dir(&staging)?;
        Ok(Replacing {
            target: target.to_path_buf(),
            staging,
        })
    }

    /// Where what is written goes until it is put in place.
    pub(crate) fn staging(&self) -> &Path {
        &self.staging
    }

    /// Puts what was written in place of what was there, answering `written` as it then stands.
    pub(crate) fn commit(self, written: &[&Path]) -> Result<Vec<PathBuf>> {
        let placed = written
            .iter()
            .map(|it| {
                it.strip_prefix(&self.staging)
                    .map(|name| self.target.join(name))
                    .context("what a build writes is written where it is staged")
            })
            .collect::<Result<Vec<_>>>()?;
        if self.target.exists() {
            let former = self.staging.with_extension("former");
            fs::rename(&self.target, &former)?;
            if let Err(problem) = fs::rename(&self.staging, &self.target) {
                fs::rename(&former, &self.target)?;
                return Err(problem.into());
            }
            fs::remove_dir_all(&former)?;
        } else {
            fs::rename(&self.staging, &self.target)?;
        }
        Ok(placed)
    }

    /// Drops what was written, where it is not to be put in place.
    pub(crate) fn abandon(self) {
        let _ = fs::remove_dir_all(&self.staging);
    }
}
