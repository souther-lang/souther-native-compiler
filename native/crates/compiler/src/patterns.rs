//! What a `String.matches` pattern or a clause's pattern is lowered to, and where an object keeps
//! it.
//!
//! The checker read the pattern, and what crossed is the machine of what it read, written as an
//! image by 199x-notation, whose formats every implementation that reads them reads alike. Nothing
//! here reads pattern text, and nothing here builds a machine: the image is held to the format it
//! says it is written in ([`check`]) and written into the object as it is, and the runtime reads it
//! with the same crate, once however many calls match against it. A pattern says the same thing
//! every run, so its image is written into the object as data, once however many calls match
//! against it, the way a string literal is.
//!
//! Every named item here says why a pattern is held the way it is, so this module denies an item
//! without documentation.

#![deny(clippy::missing_docs_in_private_items)]

use crate::{POINTER, accepted, index};
use cranelift::codegen::ir::{self, InstBuilder};
use cranelift::frontend::FunctionBuilder;
use cranelift::module::{DataDescription, DataId, Module};
use cranelift::object::ObjectModule;
use notation199x::{NotAnImage, Pattern};
use souther_native_abi::{PATTERN_IMAGE, PATTERN_LENGTH, SLOT};
use std::cell::RefCell;
use std::collections::HashMap;

/// Whether what crossed as a pattern's image is one, read as the runtime will read it: an image
/// the runtime could not read would end the run the first time the pattern is matched.
pub(crate) fn check(image: &str) -> Result<(), NotAnImage> {
    Pattern::from_image(image).map(|_| ())
}

/// Every pattern this object holds, one per image however many calls match against it.
#[derive(Default)]
pub(crate) struct Patterns {
    /// The data object each image already written in this object was written to.
    held: RefCell<HashMap<String, DataId>>,
}

impl Patterns {
    /// The address of the room holding `image` in the object, for code `builder` is emitting.
    pub(crate) fn address(
        &self,
        builder: &mut FunctionBuilder,
        module: &mut ObjectModule,
        image: &str,
    ) -> ir::Value {
        let already = self.held.borrow().get(image).copied();
        let id = match already {
            Some(id) => id,
            None => {
                let id = accepted(module.declare_anonymous_data(true, false));
                accepted(module.define_data(id, &laid_out(image)));
                index::unique(&mut *self.held.borrow_mut(), image.to_owned(), id);
                id
            }
        };
        let named = module.declare_data_in_func(id, builder.func);
        builder.ins().symbol_value(POINTER, named)
    }
}

/// The room the runtime reads a pattern from, laid out as `souther_native_abi` states it: an empty
/// slot for what the runtime reads of the image (`PATTERN_READ`, which the zeros are), the image's
/// length, and the image.
///
/// Writable, since the runtime keeps there what it read, so that the image is read once.
fn laid_out(image: &str) -> DataDescription {
    let mut written = vec![0u8; PATTERN_IMAGE as usize + image.len()];
    written[PATTERN_LENGTH as usize..PATTERN_IMAGE as usize]
        .copy_from_slice(&(image.len() as u64).to_ne_bytes());
    written[PATTERN_IMAGE as usize..].copy_from_slice(image.as_bytes());
    let mut held = DataDescription::new();
    held.define(written.into_boxed_slice());
    // The room is read as slots, and an access that says its address is aligned is not checked.
    held.set_align(SLOT as u64);
    held
}
