//! A value as the runtime's external form: generated code that walks a value and builds the tree
//! the runtime writes out as JSON.
//!
//! Nothing bounds how deep a value handed to be written is: one may have been built by another
//! object, or by a behavior the host supplies. So what is left to do in a walk is kept in a list
//! of the walk's own, and not on the native stack. Each declaration has a step, a function of this
//! object's that writes in place what of a value it can and adds to the list what has to wait for a
//! declared value it holds; one loop, [`Machine::Run`], takes work off the list until none is left.
//! A step never calls another step, so writing a value takes the same few native frames however
//! deeply its declarations nest. Splitting the writing by declaration is what a step is for; it is
//! not a native call, and nothing here makes it one.
//!
//! What a piece of work leaves for the one after it is a form, in the walk's `result`: the work for
//! a field's value leaves the field's form there, and the work under it puts that form into the
//! object it belongs to. A form is put into its object only once it is whole, as it always was,
//! so what the runtime is asked stays what it was asked before. Work is added last first, so it is
//! taken in the order the fields are laid out, and an object keeps its members in the order they
//! were put.
//!
//! The list, its records and a step's signature are this object's own and nothing another object
//! or the runtime knows about, the way a closure's layout is. A record is taken from the arena, and
//! one taken off the list is kept for the next push, so the room a walk takes is as much as was
//! ever left to do at once. The runtime's own walk over the tree, and its drop, take no frame per
//! level either.

use super::{Codecs, Runtime};
use crate::transport::{
    AlternativesForm, BoundaryOutput, Case, CodecShape, Declaration, Field, LeafScalar, Prim, Ty,
};
use crate::{
    Declared, Emitting, Literals, Lowered, NO_ARM, POINTER, TRUSTED, Tagged, machine_type,
    not_lowered, out_of_slot, text_in_the_object,
};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, AbiParam, InstBuilder, TrapCode, types};
use cranelift::codegen::isa::CallConv;
use cranelift::frontend::FunctionBuilder;
use cranelift::module::{FuncId, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{HELD, LIST_ELEMENTS, LIST_LENGTH, NOTHING, SLOT, field_at};

/// Where a walk keeps the first record of what is left to do, the first record it can use again,
/// and the form the last piece of work left: three words in a stack slot of the function that
/// starts the walk.
const LEFT: i32 = 0;
const FREE: i32 = 8;
const RESULT: i32 = 16;
const WALK: u32 = 24;

/// A record of work: the record under it, the function that does it, and the two words that
/// function is handed.
const NEXT: i32 = 0;
const CODE: i32 = 8;
const FIRST: i32 = 16;
const SECOND: i32 = 24;
const WORK: i64 = 32;

/// The functions a walk is made of besides each declaration's step.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Machine {
    /// Takes work off the list and does it until none is left.
    Run,
    /// Adds a record to the list.
    Push,
    /// Leaves the form it is handed as the result.
    Give,
    /// Puts the result into an object under a key.
    Put,
    /// Adds the result to the end of an array.
    Append,
}

impl Machine {
    pub(super) fn symbol(self) -> &'static str {
        match self {
            Machine::Run => "$encoding$run",
            Machine::Push => "$encoding$push",
            Machine::Give => "$encoding$give",
            Machine::Put => "$encoding$put",
            Machine::Append => "$encoding$append",
        }
    }

    pub(super) fn signature(self, call_conv: CallConv) -> ir::Signature {
        match self {
            Machine::Run => words(call_conv, 1),
            Machine::Push => words(call_conv, 4),
            Machine::Give | Machine::Put | Machine::Append => step_signature(call_conv),
        }
    }
}

/// What every piece of work is called with: the walk and the record's two words.
pub(super) fn step_signature(call_conv: CallConv) -> ir::Signature {
    words(call_conv, 3)
}

fn words(call_conv: CallConv, count: usize) -> ir::Signature {
    let mut signature = ir::Signature::new(call_conv);
    for _ in 0..count {
        signature.params.push(AbiParam::new(POINTER));
    }
    signature
}

/// Defines the step of `key`.
pub(super) fn define(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    id: FuncId,
    key: &str,
) -> Lowered<()> {
    let declared = emitting.declared;
    let literals = emitting.literals;
    let signature = step_signature(emitting.call_conv);
    emitting.function(id, signature, |builder, module, given| {
        let mut writing = Writing {
            builder,
            module,
            declared,
            literals,
            codecs,
        };
        Scheduling {
            writing: &mut writing,
            walk: given[0],
        }
        .declaration(key, given[1])?;
        writing.builder.ins().return_(&[]);
        Ok(())
    })
}

/// Defines one of the functions a walk is made of.
pub(super) fn define_machine(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    id: FuncId,
    part: Machine,
) -> Lowered<()> {
    let allocate = emitting.allocate;
    let call_conv = emitting.call_conv;
    let signature = part.signature(call_conv);
    emitting.function(id, signature, |builder, module, given| {
        let walk = given[0];
        match part {
            Machine::Run => {
                let head = builder.create_block();
                let work = builder.create_block();
                let done = builder.create_block();
                builder.ins().jump(head, &[]);

                builder.switch_to_block(head);
                let record = builder.ins().load(POINTER, TRUSTED, walk, LEFT);
                builder.ins().brif(record, work, &[], done, &[]);

                // The record is read whole and kept for the next push before its work is done, so
                // the work may push onto the list and use it again at once.
                builder.switch_to_block(work);
                let next = builder.ins().load(POINTER, TRUSTED, record, NEXT);
                builder.ins().store(TRUSTED, next, walk, LEFT);
                let code = builder.ins().load(POINTER, TRUSTED, record, CODE);
                let first = builder.ins().load(POINTER, TRUSTED, record, FIRST);
                let second = builder.ins().load(POINTER, TRUSTED, record, SECOND);
                let free = builder.ins().load(POINTER, TRUSTED, walk, FREE);
                builder.ins().store(TRUSTED, free, record, NEXT);
                builder.ins().store(TRUSTED, record, walk, FREE);
                let doing = builder.import_signature(step_signature(call_conv));
                builder
                    .ins()
                    .call_indirect(doing, code, &[walk, first, second]);
                builder.ins().jump(head, &[]);

                builder.switch_to_block(done);
            }
            Machine::Push => {
                let (code, first, second) = (given[1], given[2], given[3]);
                let again = builder.create_block();
                let taken = builder.create_block();
                let fill = builder.create_block();
                builder.append_block_param(fill, POINTER);
                let free = builder.ins().load(POINTER, TRUSTED, walk, FREE);
                builder.ins().brif(free, again, &[], taken, &[]);

                builder.switch_to_block(again);
                let after = builder.ins().load(POINTER, TRUSTED, free, NEXT);
                builder.ins().store(TRUSTED, after, walk, FREE);
                builder.ins().jump(fill, &[free.into()]);

                builder.switch_to_block(taken);
                let taking = module.declare_func_in_func(allocate, builder.func);
                let size = builder.ins().iconst(types::I64, WORK);
                let called = builder.ins().call(taking, &[size]);
                let room = builder.inst_results(called)[0];
                builder.ins().jump(fill, &[room.into()]);

                builder.switch_to_block(fill);
                let record = builder.block_params(fill)[0];
                let left = builder.ins().load(POINTER, TRUSTED, walk, LEFT);
                builder.ins().store(TRUSTED, left, record, NEXT);
                builder.ins().store(TRUSTED, code, record, CODE);
                builder.ins().store(TRUSTED, first, record, FIRST);
                builder.ins().store(TRUSTED, second, record, SECOND);
                builder.ins().store(TRUSTED, record, walk, LEFT);
            }
            Machine::Give => {
                builder.ins().store(TRUSTED, given[1], walk, RESULT);
            }
            Machine::Put | Machine::Append => {
                let result = builder.ins().load(POINTER, TRUSTED, walk, RESULT);
                let (called, arguments) = if part == Machine::Put {
                    (Runtime::ExternalPut, vec![given[1], given[2], result])
                } else {
                    (Runtime::ExternalAppend, vec![given[1], result])
                };
                let reached = codecs.runtime(module, called);
                let reaching = module.declare_func_in_func(reached, builder.func);
                builder.ins().call(reaching, &arguments);
            }
        }
        builder.ins().return_(&[]);
        Ok(())
    })
}

/// Whether writing a value of `shape` waits on a declared value's step. What does not is written in
/// place, and is as deep as its shape is and no deeper.
fn defers(shape: &CodecShape) -> bool {
    match shape {
        CodecShape::Named { .. } => true,
        CodecShape::OptionOf { present } => defers(present.shape()),
        CodecShape::ListOf { element } => defers(element),
        CodecShape::Scalar { .. } | CodecShape::SetOf { .. } | CodecShape::MapOf { .. } => false,
    }
}

/// The function being emitted, and what it reaches while it builds a form.
pub(crate) struct Writing<'w, 'f> {
    pub builder: &'w mut FunctionBuilder<'f>,
    pub module: &'w mut ObjectModule,
    pub declared: &'w Declared<'w>,
    pub literals: &'w Literals,
    pub codecs: &'w mut Codecs,
}

impl<'w, 'f> Writing<'w, 'f> {
    pub(crate) fn call(&mut self, called: Runtime, arguments: &[ir::Value]) -> ir::Value {
        let reached = self.codecs.runtime(self.module, called);
        let reaching = self.module.declare_func_in_func(reached, self.builder.func);
        let called = self.builder.ins().call(reaching, arguments);
        self.builder.inst_results(called)[0]
    }

    fn call_for_effect(&mut self, called: Runtime, arguments: &[ir::Value]) {
        let reached = self.codecs.runtime(self.module, called);
        let reaching = self.module.declare_func_in_func(reached, self.builder.func);
        self.builder.ins().call(reaching, arguments);
    }

    /// The address of a function of this object's, to be done as a piece of work.
    fn address(&mut self, id: FuncId) -> ir::Value {
        let reaching = self.module.declare_func_in_func(id, self.builder.func);
        self.builder.ins().func_addr(POINTER, reaching)
    }

    /// A string written into the object, a literal of the runtime's own layout: a key, or a
    /// case's name.
    fn literal(&mut self, text: &str) -> Lowered<ir::Value> {
        text_in_the_object(self.builder, self.module, self.literals, text)
    }

    fn object(&mut self) -> ir::Value {
        self.call(Runtime::ExternalObject, &[])
    }

    fn put(&mut self, object: ir::Value, key: &str, item: ir::Value) -> Lowered<()> {
        let key = self.literal(key)?;
        self.call_for_effect(Runtime::ExternalPut, &[object, key, item]);
        Ok(())
    }

    fn name(&mut self, name: &str) -> Lowered<ir::Value> {
        let spelt = self.literal(name)?;
        Ok(self.call(Runtime::ExternalString, &[spelt]))
    }

    /// What an answer leaves as, written by a walk this function starts and finishes.
    pub(crate) fn output(
        &mut self,
        output: &BoundaryOutput,
        answer: ir::Value,
    ) -> Lowered<ir::Value> {
        self.walked(|scheduling| scheduling.output(output, answer))
    }

    /// A value of a declared type, written by a walk this function starts at that type's step.
    pub(crate) fn declared(&mut self, declared: &str, value: ir::Value) -> ir::Value {
        self.walked(|scheduling| {
            scheduling.step(declared, value);
            Ok(())
        })
        .expect("adding a step to a walk refuses nothing")
    }

    /// The form a walk leaves once `start` has added its first work and the walk has done all of
    /// it. The walk is this function's, in a stack slot of its own.
    fn walked(
        &mut self,
        start: impl FnOnce(&mut Scheduling<'_, 'w, 'f>) -> Lowered<()>,
    ) -> Lowered<ir::Value> {
        let slot = self.builder.create_sized_stack_slot(ir::StackSlotData::new(
            ir::StackSlotKind::ExplicitSlot,
            WALK,
            3,
        ));
        let walk = self.builder.ins().stack_addr(POINTER, slot, 0);
        let none = self.builder.ins().iconst(POINTER, 0);
        for at in [LEFT, FREE, RESULT] {
            self.builder.ins().store(TRUSTED, none, walk, at);
        }
        start(&mut Scheduling {
            writing: &mut *self,
            walk,
        })?;
        let run = self.codecs.machine(self.module, Machine::Run);
        let reaching = self.module.declare_func_in_func(run, self.builder.func);
        self.builder.ins().call(reaching, &[walk]);
        Ok(self.builder.ins().load(POINTER, TRUSTED, walk, RESULT))
    }

    fn scalar(&mut self, scalar: LeafScalar, value: ir::Value) -> Lowered<ir::Value> {
        match scalar.prim() {
            Prim::Int => Ok(self.call(Runtime::ExternalInt, &[value])),
            Prim::Bool => Ok(self.call(Runtime::ExternalBool, &[value])),
            Prim::String => Ok(self.call(Runtime::ExternalString, &[value])),
            other => Err(not_lowered(format!(
                "a {} written at a boundary",
                other.spelt()
            ))),
        }
    }

    /// A value standing where it has no key of its own, written in place: an absent one is written
    /// `null`. Only for what [`defers`] holds holds no declared value.
    fn value(&mut self, shape: &CodecShape, value: ir::Value) -> Lowered<ir::Value> {
        match shape {
            CodecShape::Scalar { scalar } => self.scalar(*scalar, value),
            CodecShape::Named { declared } => unreachable!(
                "a value of {declared} is written by its step, and `defers` keeps it from being \
                 written in place"
            ),
            CodecShape::OptionOf { present } => {
                let absent = self.builder.create_block();
                let held = self.builder.create_block();
                let written = self.builder.create_block();
                self.builder.append_block_param(written, POINTER);
                let nothing = self.builder.ins().icmp_imm_s(IntCC::Equal, value, NOTHING);
                self.builder.ins().brif(nothing, absent, &[], held, &[]);

                self.builder.switch_to_block(absent);
                let null = self.call(Runtime::ExternalNull, &[]);
                self.builder.ins().jump(written, &[null.into()]);

                self.builder.switch_to_block(held);
                let inner = self.held(present.shape(), value)?;
                let form = self.value(present.shape(), inner)?;
                self.builder.ins().jump(written, &[form.into()]);

                self.builder.switch_to_block(written);
                Ok(self.builder.block_params(written)[0])
            }
            // A list, as an array of its elements in the order it holds them.
            CodecShape::ListOf { element } => {
                let array = self.call(Runtime::ExternalArray, &[]);
                self.elements(&element.ty(), value, false, |writing, value| {
                    let form = writing.value(element, value)?;
                    writing.call_for_effect(Runtime::ExternalAppend, &[array, form]);
                    Ok(())
                })?;
                Ok(array)
            }
            CodecShape::SetOf { .. } | CodecShape::MapOf { .. } => Err(not_lowered(format!(
                "{} written at a boundary",
                shape.ty().spelt()
            ))),
        }
    }

    /// Emits `each` over every element of a list of `element`s, first to last, or last to first
    /// where `backwards`.
    fn elements(
        &mut self,
        element: &Ty,
        list: ir::Value,
        backwards: bool,
        mut each: impl FnMut(&mut Self, ir::Value) -> Lowered<()>,
    ) -> Lowered<()> {
        let length = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, list, LIST_LENGTH as i32);
        let head = self.builder.create_block();
        self.builder.append_block_param(head, types::I64);
        let step = self.builder.create_block();
        let walked = self.builder.create_block();
        // Counted up from nought to the length, or down from the length to nought, the index
        // reached being one below the count in the second.
        let start = if backwards {
            length
        } else {
            self.builder.ins().iconst(types::I64, 0)
        };
        self.builder.ins().jump(head, &[start.into()]);

        self.builder.switch_to_block(head);
        let count = self.builder.block_params(head)[0];
        let inside = if backwards {
            self.builder
                .ins()
                .icmp_imm_s(IntCC::SignedGreaterThan, count, 0)
        } else {
            self.builder
                .ins()
                .icmp(IntCC::SignedLessThan, count, length)
        };
        self.builder.ins().brif(inside, step, &[], walked, &[]);

        self.builder.switch_to_block(step);
        let (index, next) = if backwards {
            let below = self.builder.ins().iadd_imm_s(count, -1);
            (below, below)
        } else {
            (count, self.builder.ins().iadd_imm_s(count, 1))
        };
        let along = self.builder.ins().imul_imm_s(index, SLOT);
        let at = self.builder.ins().iadd(list, along);
        let slot = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, at, LIST_ELEMENTS as i32);
        let value = out_of_slot(self.builder, slot, machine_type(element)?);
        each(self, value)?;
        self.builder.ins().jump(head, &[next.into()]);

        self.builder.switch_to_block(walked);
        Ok(())
    }

    /// A field of an object written in place, which has a second way of holding nothing: not being
    /// there.
    fn field(
        &mut self,
        object: ir::Value,
        name: &str,
        shape: &CodecShape,
        slot: ir::Value,
    ) -> Lowered<()> {
        let CodecShape::OptionOf { present } = shape else {
            let value = out_of_slot(self.builder, slot, machine_type(&shape.ty())?);
            let form = self.value(shape, value)?;
            return self.put(object, name, form);
        };
        let held = self.builder.create_block();
        let done = self.builder.create_block();
        let nothing = self.builder.ins().icmp_imm_s(IntCC::Equal, slot, NOTHING);
        self.builder.ins().brif(nothing, done, &[], held, &[]);

        self.builder.switch_to_block(held);
        let inner = self.held(present.shape(), slot)?;
        let form = self.value(present.shape(), inner)?;
        self.put(object, name, form)?;
        self.builder.ins().jump(done, &[]);

        self.builder.switch_to_block(done);
        Ok(())
    }

    /// What a present optional holds, read out of its slot the way any value is.
    fn held(&mut self, present: &CodecShape, holding: ir::Value) -> Lowered<ir::Value> {
        let slot = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, holding, HELD as i32);
        Ok(out_of_slot(
            self.builder,
            slot,
            machine_type(&present.ty())?,
        ))
    }

    fn slot_of(&mut self, value: ir::Value, at: usize) -> ir::Value {
        self.builder
            .ins()
            .load(types::I64, TRUSTED, value, field_at(at) as i32)
    }
}

/// A function adding work to a walk: a declaration's step, or the function that starts the walk.
/// Each of its methods leaves, once the work it adds is done, one form as the walk's result.
struct Scheduling<'s, 'w, 'f> {
    writing: &'s mut Writing<'w, 'f>,
    walk: ir::Value,
}

impl<'w, 'f> Scheduling<'_, 'w, 'f> {
    fn push(&mut self, code: ir::Value, first: ir::Value, second: ir::Value) {
        let push = self
            .writing
            .codecs
            .machine(self.writing.module, Machine::Push);
        let reaching = self
            .writing
            .module
            .declare_func_in_func(push, self.writing.builder.func);
        self.writing
            .builder
            .ins()
            .call(reaching, &[self.walk, code, first, second]);
    }

    fn push_machine(&mut self, part: Machine, first: ir::Value, second: ir::Value) {
        let id = self.writing.codecs.machine(self.writing.module, part);
        let code = self.writing.address(id);
        self.push(code, first, second);
    }

    fn none(&mut self) -> ir::Value {
        self.writing.builder.ins().iconst(POINTER, 0)
    }

    /// `form`, as it stands.
    fn give(&mut self, form: ir::Value) {
        let none = self.none();
        self.push_machine(Machine::Give, form, none);
    }

    /// A value of `declared`, written by that type's step.
    fn step(&mut self, declared: &str, value: ir::Value) {
        let id = self.writing.codecs.step(self.writing.module, declared);
        let code = self.writing.address(id);
        let none = self.none();
        self.push(code, value, none);
    }

    /// Adds the putting of what the work pushed after this leaves into `object` under `key`.
    fn put_later(&mut self, object: ir::Value, key: &str) -> Lowered<()> {
        let key = self.writing.literal(key)?;
        self.push_machine(Machine::Put, object, key);
        Ok(())
    }

    /// What an answer leaves as.
    fn output(&mut self, output: &BoundaryOutput, answer: ir::Value) -> Lowered<()> {
        match output {
            BoundaryOutput::Scalar { scalar } => {
                let form = self.writing.scalar(*scalar, answer)?;
                self.give(form);
                Ok(())
            }
            BoundaryOutput::Nominal { declared } => {
                self.step(declared, answer);
                Ok(())
            }
            BoundaryOutput::Cases { ty, cases, form } => {
                self.alternatives(cases, form, Tagged::of(answer, ty))
            }
            // Each element as an answer of the element's shape is written, which is how a value of
            // the element's type is written anywhere else.
            BoundaryOutput::ListOf { element } => {
                self.array(&element.ty(), answer, |scheduling, value| {
                    scheduling.output(element, value)
                })
            }
            BoundaryOutput::SetOf { .. } | BoundaryOutput::MapOf { .. } => Err(not_lowered(
                format!("an answer written as {}", output.ty().spelt()),
            )),
        }
    }

    /// A value standing where it has no key of its own: an absent one is written `null`.
    fn value(&mut self, shape: &CodecShape, value: ir::Value) -> Lowered<()> {
        if !defers(shape) {
            let form = self.writing.value(shape, value)?;
            self.give(form);
            return Ok(());
        }
        match shape {
            CodecShape::Named { declared } => {
                self.step(declared, value);
                Ok(())
            }
            CodecShape::OptionOf { present } => {
                let absent = self.writing.builder.create_block();
                let held = self.writing.builder.create_block();
                let done = self.writing.builder.create_block();
                let nothing = self
                    .writing
                    .builder
                    .ins()
                    .icmp_imm_s(IntCC::Equal, value, NOTHING);
                self.writing
                    .builder
                    .ins()
                    .brif(nothing, absent, &[], held, &[]);

                self.writing.builder.switch_to_block(absent);
                let null = self.writing.call(Runtime::ExternalNull, &[]);
                self.give(null);
                self.writing.builder.ins().jump(done, &[]);

                self.writing.builder.switch_to_block(held);
                let inner = self.writing.held(present.shape(), value)?;
                self.value(present.shape(), inner)?;
                self.writing.builder.ins().jump(done, &[]);

                self.writing.builder.switch_to_block(done);
                Ok(())
            }
            CodecShape::ListOf { element } => {
                self.array(&element.ty(), value, |scheduling, value| {
                    scheduling.value(element, value)
                })
            }
            CodecShape::Scalar { .. } | CodecShape::SetOf { .. } | CodecShape::MapOf { .. } => {
                unreachable!("`defers` holds only what holds a declared value to wait")
            }
        }
    }

    /// A list of `element`s, as an array of what `each` leaves for each, in the order it holds
    /// them. Every element's work is added at once, last first, each with the adding of what it
    /// leaves to the array under it.
    fn array(
        &mut self,
        element: &Ty,
        list: ir::Value,
        mut each: impl FnMut(&mut Scheduling<'_, 'w, 'f>, ir::Value) -> Lowered<()>,
    ) -> Lowered<()> {
        let array = self.writing.call(Runtime::ExternalArray, &[]);
        self.give(array);
        let walk = self.walk;
        self.writing
            .elements(element, list, true, |writing, value| {
                let mut scheduling = Scheduling { writing, walk };
                let none = scheduling.none();
                scheduling.push_machine(Machine::Append, array, none);
                each(&mut scheduling, value)
            })
    }

    /// A field of an object, which has a second way of holding nothing: not being there.
    fn field(
        &mut self,
        object: ir::Value,
        name: &str,
        shape: &CodecShape,
        slot: ir::Value,
    ) -> Lowered<()> {
        let CodecShape::OptionOf { present } = shape else {
            let value = out_of_slot(self.writing.builder, slot, machine_type(&shape.ty())?);
            self.put_later(object, name)?;
            return self.value(shape, value);
        };
        let held = self.writing.builder.create_block();
        let done = self.writing.builder.create_block();
        let nothing = self
            .writing
            .builder
            .ins()
            .icmp_imm_s(IntCC::Equal, slot, NOTHING);
        self.writing
            .builder
            .ins()
            .brif(nothing, done, &[], held, &[]);

        self.writing.builder.switch_to_block(held);
        let inner = self.writing.held(present.shape(), slot)?;
        self.put_later(object, name)?;
        self.value(present.shape(), inner)?;
        self.writing.builder.ins().jump(done, &[]);

        self.writing.builder.switch_to_block(done);
        Ok(())
    }

    /// Every field of a value put into an object this function made, in the order they are laid
    /// out, leaving the object. The fields before the first that waits on a declared value are put
    /// in place; that one and every one after it wait, so that none is put before it.
    fn fields(&mut self, object: ir::Value, fields: &[Field], value: ir::Value) -> Lowered<()> {
        let waits = fields
            .iter()
            .position(|field| defers(&field.codec))
            .unwrap_or(fields.len());
        for (at, field) in fields.iter().enumerate().take(waits) {
            let slot = self.writing.slot_of(value, at);
            self.writing
                .field(object, &field.name, &field.codec, slot)?;
        }
        self.give(object);
        for (at, field) in fields.iter().enumerate().skip(waits).rev() {
            let slot = self.writing.slot_of(value, at);
            self.field(object, &field.name, &field.codec, slot)?;
        }
        Ok(())
    }

    /// What a value of a declaration is written as on its own, wherever it stands.
    fn declaration(&mut self, key: &str, value: ir::Value) -> Lowered<()> {
        match self.writing.declared.laid(key) {
            Declaration::Product { fields, .. } => {
                let object = self.writing.object();
                self.fields(object, fields, value)
            }
            Declaration::Newtype { field, .. } => {
                let slot = self.writing.slot_of(value, 0);
                let inner =
                    out_of_slot(self.writing.builder, slot, machine_type(&field.codec.ty())?);
                self.value(&field.codec, inner)
            }
            Declaration::Unit { .. } => {
                let object = self.writing.object();
                self.give(object);
                Ok(())
            }
            Declaration::Sum { cases, form, .. } => {
                let sum = Ty::Declared {
                    declared: key.to_string(),
                };
                self.alternatives(cases, form, Tagged::of(value, &sum))
            }
        }
    }

    /// One of a set of alternatives, told apart by the token at the front of the value and written
    /// in the form the set travels in.
    fn alternatives(
        &mut self,
        cases: &[Case],
        form: &AlternativesForm,
        tagged: Tagged,
    ) -> Lowered<()> {
        let which = tagged.which(self.writing.builder);
        let written = self.writing.builder.create_block();

        for case in cases {
            let Case::Declared { declared: key } = case else {
                return Err(not_lowered(format!(
                    "the case {}, which this backend does not write as one of a set of \
                     alternatives yet",
                    case.spelt()
                )));
            };
            let token = self.writing.declared.tag(self.writing.module, key)?;
            let token = self
                .writing
                .module
                .declare_data_in_func(token, self.writing.builder.func);
            let expected = self.writing.builder.ins().symbol_value(POINTER, token);
            let same = self
                .writing
                .builder
                .ins()
                .icmp(IntCC::Equal, which, expected);
            let this = self.writing.builder.create_block();
            let next = self.writing.builder.create_block();
            self.writing.builder.ins().brif(same, this, &[], next, &[]);

            self.writing.builder.switch_to_block(this);
            let shape = self.writing.declared.laid(key);
            self.case(key, shape, form, tagged.value())?;
            self.writing.builder.ins().jump(written, &[]);
            self.writing.builder.switch_to_block(next);
        }
        self.writing
            .builder
            .ins()
            .trap(TrapCode::user(NO_ARM).expect("a trap code of its own"));

        self.writing.builder.switch_to_block(written);
        Ok(())
    }

    /// A case, written with what membership adds. Whether the case takes the tag into its own
    /// object or is wrapped beside it is read off its declaration's arm, never off what its own
    /// form turns out to be: a newtype over a product writes an object and is still wrapped.
    ///
    /// Every object a member is put into is one made here. A case's own fields are laid into the
    /// object that carries its tag, rather than the tag put into whatever the case's step left, so
    /// what a `put` is given is never a form this function has to take on trust. The tag is put
    /// first, before any field.
    ///
    /// [`read`](super::read) reads each arm here back, and the two are held arm for arm.
    fn case(
        &mut self,
        key: &str,
        shape: &Declaration,
        form: &AlternativesForm,
        value: ir::Value,
    ) -> Lowered<()> {
        match (form, shape) {
            (_, Declaration::Sum { .. }) => {
                unreachable!("`Declared::settled` refused a sum standing as a case of {key}")
            }
            (AlternativesForm::Enumeration, Declaration::Unit { .. }) => {
                let name = self.writing.name(shape.name())?;
                self.give(name);
                Ok(())
            }
            (
                AlternativesForm::Enumeration,
                Declaration::Product { .. } | Declaration::Newtype { .. },
            ) => unreachable!(
                "`Declared::settled` refused {key}, which has fields, in an enumeration"
            ),
            (AlternativesForm::Discriminated { tag, .. }, Declaration::Product { fields, .. }) => {
                let object = self.tagged_object(tag, shape.name())?;
                self.fields(object, fields, value)
            }
            (AlternativesForm::Discriminated { tag, .. }, Declaration::Unit { .. }) => {
                let object = self.tagged_object(tag, shape.name())?;
                self.give(object);
                Ok(())
            }
            (AlternativesForm::Discriminated { tag, contents }, Declaration::Newtype { .. }) => {
                let object = self.tagged_object(tag, shape.name())?;
                self.give(object);
                self.put_later(object, contents)?;
                self.step(key, value);
                Ok(())
            }
        }
    }

    /// An object made here with a case's name under `tag`.
    fn tagged_object(&mut self, tag: &str, name: &str) -> Lowered<ir::Value> {
        let object = self.writing.object();
        let name = self.writing.name(name)?;
        self.writing.put(object, tag, name)?;
        Ok(object)
    }
}
