//! What a `String.matches` pattern is lowered to, and where an object keeps it.
//!
//! The checker read the pattern and what crossed is what it means ([`PatternPart`]); nothing here
//! reads pattern text. That meaning is compiled to a machine by `souther_text::pattern`, which is
//! also what runs it, so what a word of the machine means is written in one place for both the
//! object that carries it and the runtime that reads it. A pattern says the same thing every run,
//! so its machine is written into the object as data, once however many calls match against it,
//! the way a string literal is.
//!
//! Every named item here says why a machine is held the way it is, so this module denies an item
//! without documentation.

#![deny(clippy::missing_docs_in_private_items)]

use crate::transport::PatternPart;
use crate::{POINTER, accepted, index};
use cranelift::codegen::ir::{self, InstBuilder};
use cranelift::frontend::FunctionBuilder;
use cranelift::module::{DataDescription, DataId, Module};
use cranelift::object::ObjectModule;
use souther_text::pattern::{self, Part, Refused};
use std::cell::RefCell;
use std::collections::HashMap;

/// The machine what a pattern means compiles to.
pub(crate) fn machine(meaning: &[PatternPart]) -> Result<Vec<u32>, Refused> {
    pattern::compile(&parts(meaning))
}

/// What [`machine`] would refuse, found without building the machine.
pub(crate) fn check(meaning: &[PatternPart]) -> Result<usize, Refused> {
    pattern::check(&parts(meaning))
}

/// The parts of what a pattern means, as `souther_text` reads them.
fn parts(meaning: &[PatternPart]) -> Vec<Part> {
    meaning
        .iter()
        .map(|part| match part {
            PatternPart::Nothing => Part::Nothing,
            PatternPart::Never => Part::Never,
            PatternPart::Symbols { ranges } => Part::Symbols(ranges.clone()),
            PatternPart::InTurn { parts } => Part::InTurn(parts.clone()),
            PatternPart::EitherOf { arms } => Part::EitherOf(arms.clone()),
            PatternPart::Repeated { what, least, most } => Part::Repeated {
                what: *what,
                least: *least,
                most: *most,
            },
        })
        .collect()
}

/// Every machine this object holds, one per machine however many calls match against it.
#[derive(Default)]
pub(crate) struct Machines {
    /// The data object each machine already written in this object was written to.
    held: RefCell<HashMap<Vec<u32>, DataId>>,
}

impl Machines {
    /// The address of the first word of `machine` in the object, for code `builder` is emitting.
    pub(crate) fn address(
        &self,
        builder: &mut FunctionBuilder,
        module: &mut ObjectModule,
        machine: &[u32],
    ) -> ir::Value {
        let already = self.held.borrow().get(machine).copied();
        let id = match already {
            Some(id) => id,
            None => {
                let id = accepted(module.declare_anonymous_data(false, false));
                accepted(module.define_data(id, &laid_out(machine)));
                index::unique(&mut *self.held.borrow_mut(), machine.to_vec(), id);
                id
            }
        };
        let named = module.declare_data_in_func(id, builder.func);
        builder.ins().symbol_value(POINTER, named)
    }
}

/// The words, each in the order this machine holds its bytes, as the runtime reads them.
fn laid_out(machine: &[u32]) -> DataDescription {
    let written: Vec<u8> = machine.iter().flat_map(|word| word.to_ne_bytes()).collect();
    let mut held = DataDescription::new();
    held.define(written.into_boxed_slice());
    // Read as words, and an access that says its address is aligned is not checked.
    held.set_align(u64::from(u32::BITS / 8));
    held
}
