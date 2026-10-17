//! What a host runs a row of a behavior it implements through, against an implementation of its own:
//! one function for each row, exported, which runs the row against the capability it is handed and
//! writes the answer in the language's external form, and, where the row states an answer, whether
//! the answer keeps it.
//!
//! The rows are the contract a host's implementation is held to, and the language's to decide. So
//! whether an answer keeps a row is decided in the object, by what holds the answer to what the
//! checker says the row states ([`crate::define_keeping`]), and never by the external form: that is
//! what a host reports a row in, and comparing two of them would decide a decimal by its scale and a
//! set by its order. A row whose answer is owed states nothing for an answer to keep, so its
//! function writes the answer alone, and a row the compile could not read has nothing to run and is
//! put on the surface with why.
//!
//! What a row of a behavior with a body is run through is not here: that is this project's own
//! check of a native run against the JVM's, and a host is told nothing of it.
//!
//! That is what every named item here says, so this module denies an item without documentation.

#![deny(clippy::missing_docs_in_private_items)]

use crate::codec::Codecs;
use crate::codec::Runtime;
use crate::codec::write::Writing;
use crate::interface::{HostFunction, RowRun, Surface};
use crate::manifest::Refusal;
use crate::transport::{BoundaryOutput, RowIdentity};
use crate::{Emitting, Lowered, TRUSTED, accepted, call_reached, machine_type};
use cranelift::codegen::ir::{InstBuilder, types};
use cranelift::module::{FuncId, Linkage, Module};
use souther_native_abi::{
    ANSWERED, EXAMPLE_ENTRY, OWED_ENTRY, host_example_symbol, host_owed_symbol,
};

/// One row of a behavior a host implements, and what the object holds to run it.
pub(crate) struct Row<'a> {
    /// The module declaring the behavior.
    pub module: &'a str,
    /// The behavior.
    pub behavior: &'a str,
    /// Where the row stands among the behavior's rows.
    pub at: usize,
    /// What the row names itself.
    pub identity: &'a RowIdentity,
    /// What the behavior answers, as the checker settled it leaves: what the answer is written as.
    pub output: &'a BoundaryOutput,
    /// What there is to run.
    pub runs: Runs<'a>,
}

/// What the object holds to run a row, by the kind of row.
pub(crate) enum Runs<'a> {
    /// The row states an answer: what holds the answer to it, which the row's own entry runs
    /// inside, or why the object holds nothing that does.
    Asserted(Result<FuncId, Refusal>),
    /// The row's answer is owed: the row's own entry, which is handed the capabilities.
    Owed(FuncId),
    /// The compile could not read the row: why.
    Unreproducible(&'a str),
}

/// Defines what a host runs each of `rows` through, asking `codecs` for the writer of each
/// declaration an answer reaches, and puts every row on `surface`.
pub(crate) fn define(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    surface: &mut Surface,
    rows: &[Row],
) -> Lowered<()> {
    for row in rows {
        let run = match &row.runs {
            Runs::Asserted(Ok(keeps)) => {
                let function = HostFunction::of(
                    &EXAMPLE_ENTRY,
                    host_example_symbol(row.module, row.behavior, row.at),
                    &[],
                    None,
                );
                written(emitting, codecs, &function, *keeps, row.output, true)?;
                Ok(RowRun::Asserted(Ok(function)))
            }
            Runs::Asserted(Err(refusal)) => Ok(RowRun::Asserted(Err(refusal.clone()))),
            Runs::Owed(entry) => {
                let function = HostFunction::of(
                    &OWED_ENTRY,
                    host_owed_symbol(row.module, row.behavior, row.at),
                    &[],
                    None,
                );
                written(emitting, codecs, &function, *entry, row.output, false)?;
                Ok(RowRun::Owed(function))
            }
            Runs::Unreproducible(why) => Err(why.to_string()),
        };
        surface.example(row.module, row.behavior, row.at, row.identity, run);
    }
    Ok(())
}

/// Defines `function`, which calls `runs` with the capabilities it is handed, and where it
/// `keeps`, the room for whether the answer keeps the row as well, then writes what was answered
/// in external form into the last room. A status that is not [`ANSWERED`] goes back as it came, and
/// nothing is written.
fn written(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    function: &HostFunction,
    runs: FuncId,
    output: &BoundaryOutput,
    keeps: bool,
) -> Lowered<()> {
    let signature = function.signature(emitting.call_conv);
    let id = accepted(emitting.module.declare_function(
        &function.symbol,
        Linkage::Export,
        &signature,
    ));
    let answers = machine_type(&output.ty())?;
    let declared = emitting.declared;
    let literals = emitting.literals;
    emitting.function(id, signature, |builder, module, given| {
        let (handed, room) = given.split_at(given.len() - 1);
        let abort = builder.create_block();
        builder.append_block_param(abort, types::I32);
        let answer = call_reached(builder, module, abort, runs, answers, handed);
        debug_assert_eq!(handed.len(), 1 + usize::from(keeps));

        let json = {
            let mut writing = Writing {
                builder,
                module,
                declared,
                literals,
                codecs,
            };
            let form = writing.output(output, answer)?;
            writing.call(Runtime::ExternalJson, &[form])
        };
        builder.ins().store(TRUSTED, json, room[0], 0);
        let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
        builder.ins().return_(&[ok]);

        builder.switch_to_block(abort);
        let status = builder.block_params(abort)[0];
        builder.ins().return_(&[status]);
        Ok(())
    })
}
