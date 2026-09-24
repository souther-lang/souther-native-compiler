//! Reading a document that says which version of itself it is.
//!
//! A document is read as a version only once it has said it is that version. Read whole first, a
//! document of another version fails on whichever member moved between the two, and says that
//! member is unknown or missing rather than that the document is of a version this does not read.
//! So what it says it is, `H`, is read on its own first, and only then the document, `W`, from the
//! same text, so that where it fails is still said by line and column.
//!
//! A versioned document has no `Deserialize` of its own, and is read through [`read`] and nothing
//! else.

use anyhow::{Result, bail};
use serde::de::DeserializeOwned;

/// `W` out of `text`, once `says` has accepted what `text` says it is.
///
/// `H` reads only what says which version a document is, and reads past everything else.
pub(crate) fn read<H, W>(text: &[u8], says: impl FnOnce(H) -> Result<(), String>) -> Result<W>
where
    H: DeserializeOwned,
    W: DeserializeOwned,
{
    let header: H = serde_json::from_slice(text)?;
    if let Err(why) = says(header) {
        bail!(why);
    }
    Ok(serde_json::from_slice(text)?)
}
