//! What an answer is written as where it leaves the object: an entry beside each behavior the
//! object publishes and each row, which runs it and writes what it answered in the language's
//! external form.
//!
//! What runs the program's own rows and asks what a behavior answers in the language's form, and
//! not what a host is offered: none of these is on the surface the header, the manifest and a
//! shared library's exports are written from. A host that wants a value's form asks a type's own
//! encoder for it.
//!
//! Nothing here decides a representation. What the answer leaves as — a scalar, a declared type, a
//! set of alternatives and the form it travels in — arrives on the transport, and writing it is
//! the writers' in [`codec`](crate::codec), the same ones a host reaches to write any value.
//!
//! That is what every named item here says, so this module denies an item without documentation.

#![deny(clippy::missing_docs_in_private_items)]

use crate::codec::write::Writing;
use crate::codec::{Codecs, Runtime};
use crate::transport::{BoundaryOutput, Ty};
use crate::{Emitting, Lowered, POINTER, TRUSTED, accepted, call_reached, machine_type};
use cranelift::codegen::ir::{self, AbiParam, InstBuilder, types};
use cranelift::module::{FuncId, Linkage, Module};
use souther_native_abi::ANSWERED;

/// An entry a host reaches for its answer as the language writes it: what it runs, what that
/// takes, and what the checker settled the answer leaves as.
pub(crate) struct Boundary<'a> {
    /// The symbol the entry is exported under. It is not the symbol of what it runs: a host that
    /// reaches this one is handed the answer written out, not the value.
    pub symbol: String,
    /// What the entry runs, whose answer it writes.
    pub runs: FuncId,
    /// Whether what it runs is a behavior's symbol, which takes what the behavior was constructed
    /// with first ([`crate::behavior_signature`]): the boundary takes it first too, and hands it on.
    /// A row's entry takes nothing more, since a row states what it stands in with.
    pub constructed: bool,
    /// What `runs` takes after that, in order. The entry takes the same, and then where to put
    /// what it wrote.
    pub takes: Vec<Ty>,
    /// What the checker settled the answer leaves as, which the entry writes without deciding it
    /// again.
    pub output: &'a BoundaryOutput,
}

/// Defines every entry in `boundaries`, asking `codecs` for the writer of each declaration one of
/// them reaches.
pub(crate) fn define(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    boundaries: &[Boundary],
) -> Lowered<()> {
    for boundary in boundaries {
        let mut signature = ir::Signature::new(emitting.call_conv);
        if boundary.constructed {
            signature.params.push(AbiParam::new(POINTER));
        }
        for taken in &boundary.takes {
            signature.params.push(AbiParam::new(machine_type(taken)?));
        }
        signature.params.push(AbiParam::new(POINTER));
        signature.returns.push(AbiParam::new(types::I32));
        let id = accepted(emitting.module.declare_function(
            &boundary.symbol,
            Linkage::Export,
            &signature,
        ));
        let answers = machine_type(&boundary.output.ty())?;
        let declared = emitting.declared;
        let literals = emitting.literals;
        emitting.function(id, signature, |builder, module, given| {
            let (arguments, out) =
                given.split_at(usize::from(boundary.constructed) + boundary.takes.len());

            // A status that is not `ANSWERED` goes back as it came, and nothing is written.
            let abort = builder.create_block();
            builder.append_block_param(abort, types::I32);
            let answer = call_reached(builder, module, abort, boundary.runs, answers, arguments);

            let json = {
                let mut writing = Writing {
                    builder,
                    module,
                    declared,
                    literals,
                    codecs,
                };
                let form = writing.output(boundary.output, answer)?;
                writing.call(Runtime::ExternalJson, &[form])
            };
            builder.ins().store(TRUSTED, json, out[0], 0);
            let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
            builder.ins().return_(&[ok]);

            builder.switch_to_block(abort);
            let status = builder.block_params(abort)[0];
            builder.ins().return_(&[status]);
            Ok(())
        })?;
    }
    Ok(())
}
