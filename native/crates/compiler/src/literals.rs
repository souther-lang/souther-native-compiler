//! The string literals an object carries, and the address generated code reads one at.
//!
//! A literal says the same text every run, so it is written into the object rather than worked out
//! into the arena. What generated code gets is the address of a string like any other: a comparison
//! and a join read it the way they read one a run made, and nothing in the value says which of the
//! two it is. That is what keeps where a string is kept out of what a string means.
//!
//! Every named item here says why a literal is held the way it is, so this module denies an item
//! without documentation.

#![deny(clippy::missing_docs_in_private_items)]

use crate::{POINTER, accepted, index};
use cranelift::codegen::ir::{self, InstBuilder};
use cranelift::frontend::FunctionBuilder;
use cranelift::module::{DataDescription, DataId, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{SLOT, TEXT_BYTES, TEXT_LENGTH, room_for_text};
use std::cell::RefCell;
use std::collections::HashMap;

/// Every string literal this object holds, one per text however many places spell it.
///
/// Held for the whole object rather than asked of each site, because a site is not what a literal
/// is: two places spelling one text are one literal, and data declared per site would be the same
/// bytes written as many times as the program says them.
#[derive(Default)]
pub(crate) struct Literals {
    /// The data object each text already spelt in this object was written to.
    held: RefCell<HashMap<String, DataId>>,
}

impl Literals {
    /// The address of `text` in the object, for code `builder` is emitting.
    ///
    /// One data object per text, shared by every site that spells it, and anonymous because
    /// nothing outside this object reaches one. A text's data is declared here and nowhere else,
    /// in the same step that puts it in `held`, so no text is written twice.
    pub(crate) fn address(
        &self,
        builder: &mut FunctionBuilder,
        module: &mut ObjectModule,
        text: &str,
    ) -> ir::Value {
        let already = self.held.borrow().get(text).copied();
        let id = match already {
            Some(id) => id,
            None => {
                let id = accepted(module.declare_anonymous_data(false, false));
                accepted(module.define_data(id, &laid_out(text)));
                index::unique(&mut *self.held.borrow_mut(), text.to_string(), id);
                id
            }
        };
        let named = module.declare_data_in_func(id, builder.func);
        builder.ins().symbol_value(POINTER, named)
    }
}

/// `text`'s bytes, laid out as the runtime lays a string out.
fn laid_out(text: &str) -> DataDescription {
    let length = i64::try_from(text.len()).expect("a literal is shorter than an Int");
    let mut written = vec![0u8; room_for_text(length) as usize];
    written[TEXT_LENGTH as usize..][..SLOT as usize].copy_from_slice(&length.to_ne_bytes());
    written[TEXT_BYTES as usize..].copy_from_slice(text.as_bytes());

    let mut held = DataDescription::new();
    held.define(written.into_boxed_slice());
    // Aligned as everything the arena answers is. The count before the text is read as a slot, and
    // every access generated code makes to a string says its address is aligned rather than
    // checking that it is.
    held.set_align(SLOT as u64);
    held
}
