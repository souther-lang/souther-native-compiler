//! The kernels of `List` and `Option` that walk what they are handed, emitted here.
//!
//! Nothing here is a runtime call. A kernel handed a function calls it the way a body applies one
//! ([`call_function`]), and how a closure is laid out and called is this object's own, so a runtime
//! function handed one would need a second copy of that convention across the boundary. What a
//! sort or `max` orders by is [`ordered`], which is what `<` over the same two values is lowered
//! as: the language has one order over each type, and a kernel ordering by another would be a
//! second.
//!
//! Every element is one slot, whatever it holds, so a walk that only moves elements (`reverse`, a
//! sort's merge) moves slots and never asks what they hold. An `Option` holding a value is a pointer
//! to a slot holding it, and a slot of a list is one ([`NOTHING`]'s doc says why that holds), so
//! `find`, `max` and `min` answer the slot of the element they chose and make nothing.

use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, InstBuilder, types};
use cranelift::frontend::FunctionBuilder;
use cranelift::module::Module;
use cranelift::object::ObjectModule;
use souther_native_abi::{
    HELD, LIST_LENGTH, MOST_ELEMENTS, NOTHING, SLOT, Status, list_at, room_for_held, room_for_list,
};

use crate::ordering::ordered;
use crate::transport::{FnSignature, Op, Prim, Ty};
use crate::{
    Held, Lowered, Lowering, POINTER, TRUSTED, abort_where, call_function, into_slot, machine_type,
    not_lowered, out_of_slot, product, sum,
};

/// The first element of `list` that `predicate` holds for (`List.find`), as the slot it stands in,
/// or nothing where it holds for none. The predicate is called on the elements in order and on
/// none after the first it holds for.
pub(crate) fn find(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    abort: ir::Block,
    predicate: Held,
    list: ir::Value,
) -> Lowered<ir::Value> {
    let function = signature(predicate.ty);
    let taken = machine_type(&function.takes[0])?;
    let count = length(builder, list);

    let head = builder.create_block();
    builder.append_block_param(head, types::I64);
    let asking = builder.create_block();
    let next = builder.create_block();
    let found = builder.create_block();
    builder.append_block_param(found, POINTER);

    let nought = builder.ins().iconst(types::I64, 0);
    builder.ins().jump(head, &[nought.into()]);

    builder.switch_to_block(head);
    let at = builder.block_params(head)[0];
    let more = builder.ins().icmp(IntCC::SignedLessThan, at, count);
    let nothing = builder.ins().iconst(POINTER, NOTHING);
    builder
        .ins()
        .brif(more, asking, &[], found, &[nothing.into()]);
    builder.seal_block(asking);

    builder.switch_to_block(asking);
    let slot = element_at(builder, list, at);
    let element = read(builder, slot, taken);
    let call_conv = module.isa().default_call_conv();
    let holds = call_function(
        builder,
        abort,
        predicate.value,
        call_conv,
        function,
        &[element],
    )?;
    builder.ins().brif(holds, found, &[slot.into()], next, &[]);
    builder.seal_block(next);

    builder.switch_to_block(next);
    let after = builder.ins().iadd_imm_s(at, 1);
    builder.ins().jump(head, &[after.into()]);
    builder.seal_block(head);
    builder.seal_block(found);

    builder.switch_to_block(found);
    Ok(builder.block_params(found)[0])
}

/// What `function` answers for the value `optional` holds (`Option.map`), held in an `Option` of
/// its own, or nothing where `optional` holds nothing and the function is not called.
pub(crate) fn mapped(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    abort: ir::Block,
    function: Held,
    optional: ir::Value,
) -> Lowered<ir::Value> {
    let signature = signature(function.ty);
    let taken = machine_type(&signature.takes[0])?;

    let holding = builder.create_block();
    let done = builder.create_block();
    builder.append_block_param(done, POINTER);

    let empty = builder.ins().icmp_imm_s(IntCC::Equal, optional, NOTHING);
    builder
        .ins()
        .brif(empty, done, &[optional.into()], holding, &[]);
    builder.seal_block(holding);

    builder.switch_to_block(holding);
    let value = builder
        .ins()
        .load(types::I64, TRUSTED, optional, HELD as i32);
    let value = out_of_slot(builder, value, taken);
    let call_conv = module.isa().default_call_conv();
    let answered = call_function(
        builder,
        abort,
        function.value,
        call_conv,
        signature,
        &[value],
    )?;
    let held = some(builder, lowering, module, answered);
    builder.ins().jump(done, &[held.into()]);
    builder.seal_block(done);

    builder.switch_to_block(done);
    Ok(builder.block_params(done)[0])
}

/// An `Option` holding `value`: a slot of its own with the value in it.
pub(crate) fn some(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    value: ir::Value,
) -> ir::Value {
    let held = into_slot(builder, value);
    let holding = lowering.room(builder, module, room_for_held());
    builder.ins().store(TRUSTED, held, holding, HELD as i32);
    holding
}

/// `list` the other way round (`List.reverse`), as a new list.
pub(crate) fn reversed(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    list: ir::Value,
) -> ir::Value {
    let count = length(builder, list);
    let made = new_list(builder, lowering, module, count);
    let last = builder.ins().iadd_imm_s(count, -1);
    each(builder, count, |builder, at| {
        let from = element_at(builder, list, at);
        let slot = builder.ins().load(types::I64, TRUSTED, from, 0);
        let there = builder.ins().isub(last, at);
        let to = element_at(builder, made, there);
        builder.ins().store(TRUSTED, slot, to, 0);
        Ok(())
    })
    .expect("moving a slot lowers whatever the slot holds");
    made
}

/// Every `Int` from `from` to `to`, both included (`List.rangeInclusive`), and none where `from` is
/// above `to`. A span longer than a list is made to hold ([`MOST_ELEMENTS`]) ends the run with
/// `status` before anything is made.
///
/// The span is `to - from` read without a sign: where `from` is not above `to` that is exactly how
/// far apart they are, however far that is, where `to - from` read with one would leave the range
/// an `Int` holds for two ends far enough apart.
pub(crate) fn range_inclusive(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    abort: ir::Block,
    status: Status,
    from: ir::Value,
    to: ir::Value,
) -> ir::Value {
    let above = builder.ins().icmp(IntCC::SignedGreaterThan, from, to);
    let span = builder.ins().isub(to, from);
    let too_long = builder
        .ins()
        .icmp_imm_u(IntCC::UnsignedGreaterThanOrEqual, span, MOST_ELEMENTS);
    let spans = builder.ins().icmp(IntCC::SignedLessThanOrEqual, from, to);
    let past = builder.ins().band(spans, too_long);
    abort_where(builder, abort, status, past);

    let nought = builder.ins().iconst(types::I64, 0);
    let many = builder.ins().iadd_imm_s(span, 1);
    let count = builder.ins().select(above, nought, many);
    let made = new_list(builder, lowering, module, count);
    each(builder, count, |builder, at| {
        let value = builder.ins().iadd(from, at);
        let to = element_at(builder, made, at);
        builder.ins().store(TRUSTED, value, to, 0);
        Ok(())
    })
    .expect("an Int lowers");
    made
}

/// The sum (`Op::Add`) or the product (`Op::Mul`) of the numbers `list` holds, from nought or one:
/// `List.sum` and `List.product`, one element at a time through what `+` or `*` over the two is
/// lowered as, so a total no number of the element's type holds ends the run with `status` as the
/// operator's would.
pub(crate) fn total(
    builder: &mut FunctionBuilder,
    abort: ir::Block,
    status: Status,
    op: Op,
    element: &Ty,
    list: ir::Value,
) -> Lowered<ir::Value> {
    match element {
        Ty::Prim { prim: Prim::Int } => {}
        _ => {
            return Err(not_lowered(format!(
                "{} of a list of {}",
                match op {
                    Op::Add => "a sum",
                    _ => "a product",
                },
                element.spelt()
            )));
        }
    }
    let seed = builder.ins().iconst(
        types::I64,
        match op {
            Op::Add => 0,
            _ => 1,
        },
    );
    let running = builder.declare_var(types::I64);
    builder.def_var(running, seed);
    let count = length(builder, list);
    each(builder, count, |builder, at| {
        let slot = element_at(builder, list, at);
        let value = builder.ins().load(types::I64, TRUSTED, slot, 0);
        let so_far = builder.use_var(running);
        let now = match op {
            Op::Add => sum(builder, abort, status, so_far, value),
            _ => product(builder, abort, status, so_far, value),
        };
        builder.def_var(running, now);
        Ok(())
    })?;
    Ok(builder.use_var(running))
}

/// The slot of the first of `list`'s greatest elements (`Op::Gt`, `List.max`) or of its least
/// (`Op::Lt`, `List.min`), ordered as `subject`, or nothing where it is empty.
///
/// An element replaces the one chosen so far only where it is strictly greater or less, so the
/// first of equal elements is the one answered. A list of what has no value is empty, and nothing
/// is compared.
pub(crate) fn extreme(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    op: Op,
    subject: &Ty,
    list: ir::Value,
) -> Lowered<ir::Value> {
    if matches!(subject, Ty::Nothing { .. }) {
        return Ok(builder.ins().iconst(POINTER, NOTHING));
    }
    let machine = machine_type(subject)?;
    let count = length(builder, list);
    let nought = builder.ins().iconst(types::I64, 0);
    let first = element_at(builder, list, nought);
    let nothing = builder.ins().iconst(POINTER, NOTHING);
    let empty = builder.ins().icmp_imm_s(IntCC::Equal, count, 0);
    let start = builder.ins().select(empty, nothing, first);
    let chosen = builder.declare_var(POINTER);
    builder.def_var(chosen, start);
    let rest = builder.ins().iadd_imm_s(count, -1);
    let rest = builder.ins().smax(rest, nought);
    each(builder, rest, |builder, at| {
        let along = builder.ins().iadd_imm_s(at, 1);
        let slot = element_at(builder, list, along);
        let so_far = builder.use_var(chosen);
        let candidate = read(builder, slot, machine);
        let best = read(builder, so_far, machine);
        let beyond = ordered(builder, lowering, module, op, subject, candidate, best)?;
        let now = builder.ins().select(beyond, slot, so_far);
        builder.def_var(chosen, now);
        Ok(())
    })?;
    Ok(builder.use_var(chosen))
}

/// `list` ordered as `subject` (`List.sort`), equal elements in the order they came in, as a new
/// list. A list of what has no value is empty, and is answered as it is.
pub(crate) fn sorted(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    subject: &Ty,
    list: ir::Value,
) -> Lowered<ir::Value> {
    if matches!(subject, Ty::Nothing { .. }) {
        return Ok(list);
    }
    let count = length(builder, list);
    let values = copied(builder, lowering, module, list, count);
    let spare = new_list(builder, lowering, module, count);
    let [values] = merged(
        builder,
        lowering,
        module,
        subject,
        count,
        [Lane {
            from: values,
            to: spare,
        }],
    )?;
    Ok(values)
}

/// `list` ordered by what `key` answers for each element, ordered as `subject` (`List.sortBy`),
/// elements of equal keys in the order they came in, as a new list.
///
/// Each element's key is worked out once, in order, before anything is compared, and the keys are
/// ordered with the elements beside them: a key is a function the program wrote, and one worked
/// out again at each comparison would be called as many times as the sort compares. Where the key
/// answers what has no value, no key is ever answered, so a list the walk gets past is empty and
/// nothing is compared.
pub(crate) fn sorted_by(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    abort: ir::Block,
    key: Held,
    subject: &Ty,
    list: ir::Value,
) -> Lowered<ir::Value> {
    let function = signature(key.ty);
    let taken = machine_type(&function.takes[0])?;
    let count = length(builder, list);
    let keys = new_list(builder, lowering, module, count);
    let call_conv = module.isa().default_call_conv();
    each(builder, count, |builder, at| {
        let slot = element_at(builder, list, at);
        let element = read(builder, slot, taken);
        let answered = call_function(builder, abort, key.value, call_conv, function, &[element])?;
        let answered = into_slot(builder, answered);
        let to = element_at(builder, keys, at);
        builder.ins().store(TRUSTED, answered, to, 0);
        Ok(())
    })?;
    if matches!(subject, Ty::Nothing { .. }) {
        return Ok(list);
    }
    let values = copied(builder, lowering, module, list, count);
    let spare_keys = new_list(builder, lowering, module, count);
    let spare_values = new_list(builder, lowering, module, count);
    let [_, values] = merged(
        builder,
        lowering,
        module,
        subject,
        count,
        [
            Lane {
                from: keys,
                to: spare_keys,
            },
            Lane {
                from: values,
                to: spare_values,
            },
        ],
    )?;
    Ok(values)
}

/// Two lists of one length, one holding what is being ordered and the other room to merge it into.
struct Lane {
    from: ir::Value,
    to: ir::Value,
}

/// The lanes ordered together by what the first holds, ordered as `subject`, and the list each is
/// left in: a merge sort from the bottom up, merging runs of one element into runs of two, of four,
/// and on until one run is the whole list.
///
/// Each pass merges every pair of runs from one list of a lane into the other, and the two swap for
/// the next pass. Where the two runs being merged hold equal elements, the one from the run on the
/// left is taken first: the left run's elements came in before the right's, so the order equal
/// elements came in is kept however many passes there are.
fn merged<const LANES: usize>(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    subject: &Ty,
    count: ir::Value,
    lanes: [Lane; LANES],
) -> Lowered<[ir::Value; LANES]> {
    let machine = machine_type(subject)?;
    let from = lanes.each_ref().map(|_| builder.declare_var(POINTER));
    let to = lanes.each_ref().map(|_| builder.declare_var(POINTER));
    for (lane, (from, to)) in lanes.iter().zip(from.iter().zip(&to)) {
        builder.def_var(*from, lane.from);
        builder.def_var(*to, lane.to);
    }
    let width = builder.declare_var(types::I64);
    let low = builder.declare_var(types::I64);
    let middle = builder.declare_var(types::I64);
    let high = builder.declare_var(types::I64);
    let left = builder.declare_var(types::I64);
    let right = builder.declare_var(types::I64);
    let into = builder.declare_var(types::I64);
    let one = builder.ins().iconst(types::I64, 1);
    builder.def_var(width, one);

    let passes = builder.create_block();
    let pass = builder.create_block();
    let runs = builder.create_block();
    let run = builder.create_block();
    let merging = builder.create_block();
    let left_holds = builder.create_block();
    let comparing = builder.create_block();
    let take_left = builder.create_block();
    let take_right = builder.create_block();
    let run_merged = builder.create_block();
    let passed = builder.create_block();
    let done = builder.create_block();

    builder.ins().jump(passes, &[]);

    // A pass for every width below the length: one run the whole list is ordered.
    builder.switch_to_block(passes);
    let wide = builder.use_var(width);
    let more = builder.ins().icmp(IntCC::SignedLessThan, wide, count);
    builder.ins().brif(more, pass, &[], done, &[]);
    builder.seal_block(pass);

    builder.switch_to_block(pass);
    let nought = builder.ins().iconst(types::I64, 0);
    builder.def_var(low, nought);
    builder.ins().jump(runs, &[]);

    // A merge for every pair of runs, the second of them shorter or empty at the end.
    builder.switch_to_block(runs);
    let at = builder.use_var(low);
    let more = builder.ins().icmp(IntCC::SignedLessThan, at, count);
    builder.ins().brif(more, run, &[], passed, &[]);
    builder.seal_block(run);
    builder.seal_block(passed);

    builder.switch_to_block(run);
    let wide = builder.use_var(width);
    let mid = builder.ins().iadd(at, wide);
    let mid = builder.ins().smin(mid, count);
    let end = builder.ins().iadd(mid, wide);
    let end = builder.ins().smin(end, count);
    builder.def_var(middle, mid);
    builder.def_var(high, end);
    builder.def_var(left, at);
    builder.def_var(right, mid);
    builder.def_var(into, at);
    builder.ins().jump(merging, &[]);

    builder.switch_to_block(merging);
    let k = builder.use_var(into);
    let end = builder.use_var(high);
    let more = builder.ins().icmp(IntCC::SignedLessThan, k, end);
    builder.ins().brif(more, left_holds, &[], run_merged, &[]);
    builder.seal_block(left_holds);
    builder.seal_block(run_merged);

    // The left run's element where the right run is used up, or where the right's is not strictly
    // before it.
    builder.switch_to_block(left_holds);
    let i = builder.use_var(left);
    let mid = builder.use_var(middle);
    let left_more = builder.ins().icmp(IntCC::SignedLessThan, i, mid);
    builder
        .ins()
        .brif(left_more, comparing, &[], take_right, &[]);
    builder.seal_block(comparing);

    builder.switch_to_block(comparing);
    let j = builder.use_var(right);
    let end = builder.use_var(high);
    let right_more = builder.ins().icmp(IntCC::SignedLessThan, j, end);
    let compared = builder.create_block();
    builder
        .ins()
        .brif(right_more, compared, &[], take_left, &[]);
    builder.seal_block(compared);

    builder.switch_to_block(compared);
    let ordering = builder.use_var(from[0]);
    let i = builder.use_var(left);
    let j = builder.use_var(right);
    let on_the_left = element_at(builder, ordering, i);
    let on_the_left = read(builder, on_the_left, machine);
    let on_the_right = element_at(builder, ordering, j);
    let on_the_right = read(builder, on_the_right, machine);
    let before = ordered(
        builder,
        lowering,
        module,
        Op::Lt,
        subject,
        on_the_right,
        on_the_left,
    )?;
    builder.ins().brif(before, take_right, &[], take_left, &[]);
    builder.seal_block(take_left);
    builder.seal_block(take_right);

    for (taking, side) in [(take_left, left), (take_right, right)] {
        builder.switch_to_block(taking);
        let at = builder.use_var(side);
        let k = builder.use_var(into);
        for (from, to) in from.iter().zip(&to) {
            let from = builder.use_var(*from);
            let to = builder.use_var(*to);
            let source = element_at(builder, from, at);
            let slot = builder.ins().load(types::I64, TRUSTED, source, 0);
            let target = element_at(builder, to, k);
            builder.ins().store(TRUSTED, slot, target, 0);
        }
        let next = builder.ins().iadd_imm_s(at, 1);
        builder.def_var(side, next);
        let next = builder.ins().iadd_imm_s(k, 1);
        builder.def_var(into, next);
        builder.ins().jump(merging, &[]);
    }
    builder.seal_block(merging);

    builder.switch_to_block(run_merged);
    let end = builder.use_var(high);
    builder.def_var(low, end);
    builder.ins().jump(runs, &[]);
    builder.seal_block(runs);

    // What was merged into is what the next pass merges from.
    builder.switch_to_block(passed);
    for (from, to) in from.iter().zip(&to) {
        let was_from = builder.use_var(*from);
        let was_to = builder.use_var(*to);
        builder.def_var(*from, was_to);
        builder.def_var(*to, was_from);
    }
    let wide = builder.use_var(width);
    let wider = builder.ins().iadd(wide, wide);
    builder.def_var(width, wider);
    builder.ins().jump(passes, &[]);
    builder.seal_block(passes);
    builder.seal_block(done);

    builder.switch_to_block(done);
    Ok(from.map(|from| builder.use_var(from)))
}

/// What a function value handed to a kernel is: the contract gave it a function's shape, and
/// `Coherent` held the argument to it.
fn signature(ty: &Ty) -> &FnSignature {
    let Ty::Fn { fn_ } = ty else {
        unreachable!("`Coherent` held every function a kernel takes to a function's type");
    };
    fn_
}

/// How many elements `list` holds.
fn length(builder: &mut FunctionBuilder, list: ir::Value) -> ir::Value {
    builder
        .ins()
        .load(types::I64, TRUSTED, list, LIST_LENGTH as i32)
}

/// Where `list`'s element at `index` is.
fn element_at(builder: &mut FunctionBuilder, list: ir::Value, index: ir::Value) -> ir::Value {
    let along = builder.ins().imul_imm_s(index, SLOT);
    let at = builder.ins().iadd(list, along);
    builder.ins().iadd_imm_s(at, list_at(0))
}

/// What the slot at `slot` holds, as a value of the machine type `wanted`.
fn read(builder: &mut FunctionBuilder, slot: ir::Value, wanted: types::Type) -> ir::Value {
    let held = builder.ins().load(types::I64, TRUSTED, slot, 0);
    out_of_slot(builder, held, wanted)
}

/// A list of `count` elements, with its length written and its elements not yet.
pub(crate) fn new_list(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    count: ir::Value,
) -> ir::Value {
    let slots = builder.ins().imul_imm_s(count, SLOT);
    let bytes = builder.ins().iadd_imm_s(slots, room_for_list(0));
    let made = lowering.room_of(builder, module, bytes);
    builder
        .ins()
        .store(TRUSTED, count, made, LIST_LENGTH as i32);
    made
}

/// A new list holding what `list`, `count` long, holds.
fn copied(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    list: ir::Value,
    count: ir::Value,
) -> ir::Value {
    let made = new_list(builder, lowering, module, count);
    let into = builder.ins().iadd_imm_s(made, list_at(0));
    let from = builder.ins().iadd_imm_s(list, list_at(0));
    crate::copy_slots(builder, from, into, count);
    made
}

/// `body` run once for each index from nought up to `count`, in order, and none where `count` is
/// nought or below.
fn each(
    builder: &mut FunctionBuilder,
    count: ir::Value,
    mut body: impl FnMut(&mut FunctionBuilder, ir::Value) -> Lowered<()>,
) -> Lowered<()> {
    let head = builder.create_block();
    builder.append_block_param(head, types::I64);
    let walking = builder.create_block();
    let done = builder.create_block();

    let nought = builder.ins().iconst(types::I64, 0);
    builder.ins().jump(head, &[nought.into()]);

    builder.switch_to_block(head);
    let at = builder.block_params(head)[0];
    let more = builder.ins().icmp(IntCC::SignedLessThan, at, count);
    builder.ins().brif(more, walking, &[], done, &[]);
    builder.seal_block(walking);
    builder.seal_block(done);

    builder.switch_to_block(walking);
    body(builder, at)?;
    let next = builder.ins().iadd_imm_s(at, 1);
    builder.ins().jump(head, &[next.into()]);
    builder.seal_block(head);

    builder.switch_to_block(done);
    Ok(())
}
