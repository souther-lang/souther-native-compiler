//! A value as the runtime's external form: generated code that walks a value and builds the tree
//! the runtime writes out as JSON.
//!
//! Nothing bounds how deep a value handed to be written is: one may have been built by another
//! object, or by a behavior the host supplies. So what is left to do in a walk is kept in a list
//! of the walk's own, and not on the native stack. Each declaration has a step, a function of this
//! object's that writes in place what of a value it can and adds to the list what has to wait for a
//! declared value it holds; one loop, [`Driver::Run`], takes work off the list until none is left.
//! A step never calls another step, so writing a value takes the same few native frames however
//! deeply its declarations nest. Splitting the writing by declaration is what a step is for; it is
//! not a native call, and nothing here makes it one.
//!
//! Only what waits on a declared value goes on the list ([`defers`], [`output_defers`]). Anything
//! else is written in place, as deep as its shape is and no deeper, and an answer that holds no
//! declared value starts no walk at all. A list whose elements wait adds one element's work at a
//! time, so what is left to do stays as much as the value is deep and not as long as its lists.
//!
//! What a piece of work leaves for the one after it is a form, in the walk's `result`, and the
//! form there is owned by nothing else. Work that leaves a form ([`Continuation::Give`], a step)
//! finds `result` empty and fills it; work that takes one ([`Continuation::Put`],
//! [`Continuation::Append`]) finds it filled, moves the form into the object or array it belongs
//! to, and empties it. Each checks that it finds `result` the way it expects, and traps if not, so
//! work added out of that order is caught where it runs and never reads a form already moved. A
//! form is put into its object only once it is whole, as it always was, so what the runtime is
//! asked stays what it was asked before. Work is added last first, so it is taken in the order the
//! fields are laid out, and an object keeps its members in the order they were put.
//!
//! The list, its records and the signature work is called by are this object's own and nothing
//! another object or the runtime knows about, the way a closure's layout is. A record names its
//! work by a [`Work`], which only a function declared with that signature is, so a record never
//! names a function the loop would call wrongly. A record is taken from the arena, and one taken
//! off the list is kept for the next push, so the room a walk takes is as much as was ever left to
//! do at once. The runtime's own walk over the tree, and its drop, take no frame per level either.

use super::{Codecs, Runtime};
use crate::literals::Literals;
use crate::transport::{
    AlternativesForm, BoundaryOutput, Case, CodecShape, Declaration, Field, LeafScalar, Prim, Ty,
};
use crate::{
    A_WALK_OUT_OF_ORDER, Declared, Emitting, Lowered, NO_ARM, POINTER, TRUSTED, Tagged, accepted,
    machine_type, not_lowered, out_of_slot,
};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, AbiParam, InstBuilder, TrapCode, types};
use cranelift::codegen::isa::CallConv;
use cranelift::frontend::FunctionBuilder;
use cranelift::module::{FuncId, Linkage, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{HELD, LIST_ELEMENTS, LIST_LENGTH, NOTHING, SLOT, field_at};

/// Where a walk keeps the first record of what is left to do, the first record it can use again,
/// and the form the last piece of work left: three words in a stack slot of the function that
/// starts the walk.
const LEFT: i32 = 0;
const FREE: i32 = 8;
const RESULT: i32 = 16;
const WALK: u32 = 24;

/// A record of work: the record under it, the function that does it, and the three words that
/// function is handed.
const NEXT: i32 = 0;
const CODE: i32 = 8;
const HANDED: [i32; 3] = [16, 24, 32];
const WORK: i64 = 40;

/// A function a record of work can name: one declared with the signature [`Driver::Run`] calls
/// work by, the walk and the record's three words. Made only by [`declare_work`], so whatever
/// declares a function under another signature has no `Work` to put in a record.
#[derive(Clone, Copy)]
pub(super) struct Work(FuncId);

impl Work {
    pub(super) fn id(self) -> FuncId {
        self.0
    }
}

/// Declares a function of this object's that a record of work can name.
pub(super) fn declare_work(module: &mut ObjectModule, call_conv: CallConv, symbol: &str) -> Work {
    Work(accepted(module.declare_function(
        symbol,
        Linkage::Local,
        &work_signature(call_conv),
    )))
}

fn work_signature(call_conv: CallConv) -> ir::Signature {
    words(call_conv, 1 + HANDED.len())
}

fn words(call_conv: CallConv, count: usize) -> ir::Signature {
    let mut signature = ir::Signature::new(call_conv);
    for _ in 0..count {
        signature.params.push(AbiParam::new(POINTER));
    }
    signature
}

/// What a function that starts a walk calls, and never a record's work.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Driver {
    /// Takes work off the list and does it until none is left.
    Run,
    /// Adds a record to the list.
    Push,
}

impl Driver {
    pub(super) fn symbol(self) -> &'static str {
        match self {
            Driver::Run => "$encoding$run",
            Driver::Push => "$encoding$push",
        }
    }

    pub(super) fn signature(self, call_conv: CallConv) -> ir::Signature {
        match self {
            Driver::Run => words(call_conv, 1),
            Driver::Push => words(call_conv, 2 + HANDED.len()),
        }
    }
}

/// The work a walk does besides each declaration's step and each list's elements.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Continuation {
    /// Leaves the form it is handed as the result.
    Give,
    /// Moves the result into an object under a key.
    Put,
    /// Moves the result to the end of an array.
    Append,
}

impl Continuation {
    pub(super) fn symbol(self) -> &'static str {
        match self {
            Continuation::Give => "$encoding$give",
            Continuation::Put => "$encoding$put",
            Continuation::Append => "$encoding$append",
        }
    }
}

/// What the elements of a list whose elements wait are written as: a value where it has no key of
/// its own, or an answer.
#[derive(Clone)]
pub(super) enum Element {
    Value(CodecShape),
    Output(BoundaryOutput),
}

impl Element {
    fn ty(&self) -> Ty {
        match self {
            Element::Value(shape) => shape.ty(),
            Element::Output(output) => output.ty(),
        }
    }
}

/// Defines the step of `key`.
pub(super) fn define(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    work: Work,
    key: &str,
) -> Lowered<()> {
    let declared = emitting.declared;
    let literals = emitting.literals;
    let signature = work_signature(emitting.call_conv);
    emitting.function(work.id(), signature, |builder, module, given| {
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

/// Defines the work that writes the element of a list at an index, handed the list, the array its
/// elements go into, and the index. It adds, under that element's work, the moving of what the
/// element leaves into the array and then itself at the next index, so one element's work is on
/// the list at a time.
pub(super) fn define_each(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    work: Work,
    element: &Element,
) -> Lowered<()> {
    let declared = emitting.declared;
    let literals = emitting.literals;
    let signature = work_signature(emitting.call_conv);
    emitting.function(work.id(), signature, |builder, module, given| {
        let (walk, list, array, index) = (given[0], given[1], given[2], given[3]);
        let mut writing = Writing {
            builder,
            module,
            declared,
            literals,
            codecs,
        };
        let length = writing.length(list);
        let more = writing.builder.create_block();
        let done = writing.builder.create_block();
        let inside = writing
            .builder
            .ins()
            .icmp(IntCC::SignedLessThan, index, length);
        writing.builder.ins().brif(inside, more, &[], done, &[]);

        writing.builder.switch_to_block(more);
        let next = writing.builder.ins().iadd_imm_s(index, 1);
        let value = writing.element_at(list, index, &element.ty())?;
        let mut scheduling = Scheduling {
            writing: &mut writing,
            walk,
        };
        scheduling.push(work, &[list, array, next]);
        scheduling.push_continuation(Continuation::Append, &[array]);
        match element {
            Element::Value(shape) => scheduling.value(shape, value)?,
            Element::Output(output) => scheduling.output(output, value)?,
        }
        writing.builder.ins().jump(done, &[]);

        writing.builder.switch_to_block(done);
        writing.builder.ins().return_(&[]);
        Ok(())
    })
}

/// Defines one of the continuations.
pub(super) fn define_continuation(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    work: Work,
    part: Continuation,
) -> Lowered<()> {
    let signature = work_signature(emitting.call_conv);
    emitting.function(work.id(), signature, |builder, module, given| {
        let walk = given[0];
        let result = builder.ins().load(POINTER, TRUSTED, walk, RESULT);
        let out_of_order = TrapCode::user(A_WALK_OUT_OF_ORDER).expect("a trap code of its own");
        match part {
            Continuation::Give => {
                builder.ins().trapnz(result, out_of_order);
                builder.ins().store(TRUSTED, given[1], walk, RESULT);
            }
            Continuation::Put | Continuation::Append => {
                builder.ins().trapz(result, out_of_order);
                let (called, arguments) = if part == Continuation::Put {
                    (Runtime::ExternalPut, vec![given[1], given[2], result])
                } else {
                    (Runtime::ExternalAppend, vec![given[1], result])
                };
                let reached = codecs.runtime(module, called);
                let reaching = module.declare_func_in_func(reached, builder.func);
                builder.ins().call(reaching, &arguments);
                // The form is the object's or the array's now, and nothing is left for the next.
                let none = builder.ins().iconst(POINTER, 0);
                builder.ins().store(TRUSTED, none, walk, RESULT);
            }
        }
        builder.ins().return_(&[]);
        Ok(())
    })
}

/// Defines one of the functions a walk is driven by.
pub(super) fn define_driver(emitting: &mut Emitting, id: FuncId, part: Driver) -> Lowered<()> {
    let allocate = emitting.allocate;
    let call_conv = emitting.call_conv;
    let signature = part.signature(call_conv);
    emitting.function(id, signature, |builder, module, given| {
        let walk = given[0];
        match part {
            Driver::Run => {
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
                let mut handed = vec![walk];
                for at in HANDED {
                    handed.push(builder.ins().load(POINTER, TRUSTED, record, at));
                }
                let free = builder.ins().load(POINTER, TRUSTED, walk, FREE);
                builder.ins().store(TRUSTED, free, record, NEXT);
                builder.ins().store(TRUSTED, record, walk, FREE);
                let doing = builder.import_signature(work_signature(call_conv));
                builder.ins().call_indirect(doing, code, &handed);
                builder.ins().jump(head, &[]);

                builder.switch_to_block(done);
            }
            Driver::Push => {
                let code = given[1];
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
                for (at, &word) in HANDED.iter().zip(&given[2..]) {
                    builder.ins().store(TRUSTED, word, record, *at);
                }
                builder.ins().store(TRUSTED, record, walk, LEFT);
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

/// [`defers`] for an answer. A set of alternatives waits, since each of its cases is a declared
/// type.
fn output_defers(output: &BoundaryOutput) -> bool {
    match output {
        BoundaryOutput::Nominal { .. } | BoundaryOutput::Cases { .. } => true,
        BoundaryOutput::ListOf { element } => output_defers(element),
        BoundaryOutput::Scalar { .. }
        | BoundaryOutput::SetOf { .. }
        | BoundaryOutput::MapOf { .. } => false,
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

    /// A string written into the object, a literal of the runtime's own layout: a key, or a
    /// case's name.
    fn literal(&mut self, text: &str) -> Lowered<ir::Value> {
        self.literals.address(self.builder, self.module, text)
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

    /// What an answer leaves as. One that holds no declared value is written in place, and one
    /// that does by a walk this function starts and finishes.
    pub(crate) fn output(
        &mut self,
        output: &BoundaryOutput,
        answer: ir::Value,
    ) -> Lowered<ir::Value> {
        if !output_defers(output) {
            return self.output_in_place(output, answer);
        }
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
        let run = self.codecs.driver(self.module, Driver::Run);
        let reaching = self.module.declare_func_in_func(run, self.builder.func);
        self.builder.ins().call(reaching, &[walk]);
        let result = self.builder.ins().load(POINTER, TRUSTED, walk, RESULT);
        self.builder.ins().trapz(
            result,
            TrapCode::user(A_WALK_OUT_OF_ORDER).expect("a trap code of its own"),
        );
        Ok(result)
    }

    /// An answer that holds no declared value, written in place.
    fn output_in_place(
        &mut self,
        output: &BoundaryOutput,
        answer: ir::Value,
    ) -> Lowered<ir::Value> {
        match output {
            BoundaryOutput::Scalar { scalar } => self.scalar(*scalar, answer),
            // Each element as an answer of the element's shape is written, which is how a value of
            // the element's type is written anywhere else.
            BoundaryOutput::ListOf { element } => {
                self.array(&element.ty(), answer, |writing, value| {
                    writing.output_in_place(element, value)
                })
            }
            BoundaryOutput::Nominal { .. } | BoundaryOutput::Cases { .. } => unreachable!(
                "`output_defers` keeps what holds a declared value from being written in place"
            ),
            BoundaryOutput::SetOf { .. } | BoundaryOutput::MapOf { .. } => Err(not_lowered(
                format!("an answer written as {}", output.ty().spelt()),
            )),
        }
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
            CodecShape::ListOf { element } => self.array(&element.ty(), value, |writing, value| {
                writing.value(element, value)
            }),
            CodecShape::SetOf { .. } | CodecShape::MapOf { .. } => Err(not_lowered(format!(
                "{} written at a boundary",
                shape.ty().spelt()
            ))),
        }
    }

    /// A list of `element`s written in place, as an array of what `write` writes each as, in the
    /// order it holds them.
    fn array(
        &mut self,
        element: &Ty,
        list: ir::Value,
        mut write: impl FnMut(&mut Self, ir::Value) -> Lowered<ir::Value>,
    ) -> Lowered<ir::Value> {
        let array = self.call(Runtime::ExternalArray, &[]);
        let length = self.length(list);
        let head = self.builder.create_block();
        self.builder.append_block_param(head, types::I64);
        let step = self.builder.create_block();
        let written = self.builder.create_block();
        let start = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().jump(head, &[start.into()]);

        self.builder.switch_to_block(head);
        let index = self.builder.block_params(head)[0];
        let inside = self
            .builder
            .ins()
            .icmp(IntCC::SignedLessThan, index, length);
        self.builder.ins().brif(inside, step, &[], written, &[]);

        self.builder.switch_to_block(step);
        let value = self.element_at(list, index, element)?;
        let form = write(self, value)?;
        self.call_for_effect(Runtime::ExternalAppend, &[array, form]);
        let next = self.builder.ins().iadd_imm_s(index, 1);
        self.builder.ins().jump(head, &[next.into()]);

        self.builder.switch_to_block(written);
        Ok(array)
    }

    fn length(&mut self, list: ir::Value) -> ir::Value {
        self.builder
            .ins()
            .load(types::I64, TRUSTED, list, LIST_LENGTH as i32)
    }

    /// The element of a list of `element`s at `index`, read out of its slot the way any value is.
    fn element_at(
        &mut self,
        list: ir::Value,
        index: ir::Value,
        element: &Ty,
    ) -> Lowered<ir::Value> {
        let along = self.builder.ins().imul_imm_s(index, SLOT);
        let at = self.builder.ins().iadd(list, along);
        let slot = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, at, LIST_ELEMENTS as i32);
        Ok(out_of_slot(self.builder, slot, machine_type(element)?))
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

/// A function adding work to a walk: a piece of work itself, or the function that starts the walk.
/// Each of its methods adds work that, once done, leaves one form as the walk's result.
struct Scheduling<'s, 'w, 'f> {
    writing: &'s mut Writing<'w, 'f>,
    walk: ir::Value,
}

impl<'w, 'f> Scheduling<'_, 'w, 'f> {
    /// Adds a record naming `work`, handed `handed` and noughts for the words it is not handed.
    fn push(&mut self, work: Work, handed: &[ir::Value]) {
        assert!(
            handed.len() <= HANDED.len(),
            "a record hands its work {} words",
            HANDED.len()
        );
        let writing = &mut *self.writing;
        let reaching = writing
            .module
            .declare_func_in_func(work.id(), writing.builder.func);
        let code = writing.builder.ins().func_addr(POINTER, reaching);
        let mut given = vec![self.walk, code];
        given.extend_from_slice(handed);
        while given.len() < 2 + HANDED.len() {
            given.push(writing.builder.ins().iconst(POINTER, 0));
        }
        let push = writing.codecs.driver(writing.module, Driver::Push);
        let reaching = writing
            .module
            .declare_func_in_func(push, writing.builder.func);
        writing.builder.ins().call(reaching, &given);
    }

    fn push_continuation(&mut self, part: Continuation, handed: &[ir::Value]) {
        let work = self.writing.codecs.continuation(self.writing.module, part);
        self.push(work, handed);
    }

    /// `form`, as it stands.
    fn give(&mut self, form: ir::Value) {
        self.push_continuation(Continuation::Give, &[form]);
    }

    /// A value of `declared`, written by that type's step.
    fn step(&mut self, declared: &str, value: ir::Value) {
        let work = self.writing.codecs.step(self.writing.module, declared);
        self.push(work, &[value]);
    }

    /// Adds the moving of what the work pushed after this leaves into `object` under `key`.
    fn put_later(&mut self, object: ir::Value, key: &str) -> Lowered<()> {
        let key = self.writing.literal(key)?;
        self.push_continuation(Continuation::Put, &[object, key]);
        Ok(())
    }

    /// What an answer leaves as.
    fn output(&mut self, output: &BoundaryOutput, answer: ir::Value) -> Lowered<()> {
        if !output_defers(output) {
            let form = self.writing.output_in_place(output, answer)?;
            self.give(form);
            return Ok(());
        }
        match output {
            BoundaryOutput::Nominal { declared } => {
                self.step(declared, answer);
                Ok(())
            }
            BoundaryOutput::Cases { ty, cases, form } => {
                self.alternatives(cases, form, Tagged::of(answer, ty))
            }
            BoundaryOutput::ListOf { element } => {
                self.array(Element::Output((**element).clone()), answer);
                Ok(())
            }
            BoundaryOutput::Scalar { .. }
            | BoundaryOutput::SetOf { .. }
            | BoundaryOutput::MapOf { .. } => {
                unreachable!("`output_defers` holds only what holds a declared value to wait")
            }
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
                self.array(Element::Value((**element).clone()), value);
                Ok(())
            }
            CodecShape::Scalar { .. } | CodecShape::SetOf { .. } | CodecShape::MapOf { .. } => {
                unreachable!("`defers` holds only what holds a declared value to wait")
            }
        }
    }

    /// A list whose elements wait, as an array of what each element leaves, in the order the list
    /// holds them. What is added is the array, given once every element is in it, and the work
    /// that writes the first element, which adds the next one's only once it is done.
    fn array(&mut self, element: Element, list: ir::Value) {
        let array = self.writing.call(Runtime::ExternalArray, &[]);
        self.give(array);
        let each = self.writing.codecs.each(self.writing.module, element);
        let first = self.writing.builder.ins().iconst(types::I64, 0);
        self.push(each, &[list, array, first]);
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
