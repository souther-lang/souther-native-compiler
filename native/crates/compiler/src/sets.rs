//! The kernels of the `Set` and `Map` modules, each a call into the runtime, which keeps both
//! (`souther_native_abi::SET_EMPTY`).
//!
//! What a call hands over beside the collection is what the runtime cannot know: the hasher and the
//! equality of what the members or keys are, which the checker settled for this application and
//! `Coherent` held to the kernel's contract. They are read off what the application takes, and not
//! off the collection's own type as some other site saw it, since a set built as a `Set<A>` is
//! asked of here as the `Set<S>` this call takes (`hashing`).
//!
//! `singleton` is the empty collection with one entry put in, and `isEmpty` is a size of nought:
//! neither is a function of the runtime's of its own, since each is one call of one that is.

use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, InstBuilder};
use cranelift::frontend::FunctionBuilder;
use cranelift::object::ObjectModule;
use souther_native_abi::{
    MAP_CONTAINS_KEY, MAP_EMPTY, MAP_FROM_LIST, MAP_GET, MAP_INSERT, MAP_KEYS, MAP_REMOVE,
    MAP_SIZE, MAP_TO_LIST, MAP_VALUES, SET_CONTAINS, SET_DIFFERENCE, SET_EMPTY, SET_FROM_LIST,
    SET_INSERT, SET_INTERSECTION, SET_REMOVE, SET_SIZE, SET_TO_LIST, SET_UNION,
};

use crate::kernels::LoweredKernel;
use crate::transport::{AbortKind, Ty};
use crate::{Lowerings, POINTER, TRUSTED, into_slot, out_slot, runtime_call, written_or_ended};

/// A kernel of the `Set` or the `Map` module, applied to `values`, which the application takes as
/// `takes`.
///
/// # Panics
///
/// Where `kernel` is of neither module, or the values are not as many as its contract takes:
/// `Coherent` held every application to the contract of the kernel it names.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    abort: ir::Block,
    kernel: LoweredKernel,
    takes: &[Ty],
    values: &[ir::Value],
    aborts: &[AbortKind],
) -> ir::Value {
    let mut call =
        |name: &str, handed: &[ir::Value]| runtime_call(builder, lowering, module, name, handed);
    match (kernel, values) {
        (LoweredKernel::SetEmpty, []) => call(SET_EMPTY, &[]),
        (LoweredKernel::MapEmpty, []) => call(MAP_EMPTY, &[]),
        (LoweredKernel::SetSingleton, [value]) => {
            let empty = call(SET_EMPTY, &[]);
            let [hasher, equality] = lowering.value_ops.both(builder, module, &takes[0]);
            let value = into_slot(builder, *value);
            put_in(
                builder,
                lowering,
                module,
                SET_INSERT,
                &[empty, value, hasher, equality],
            )
        }
        (LoweredKernel::MapSingleton, [key, value]) => {
            let empty = call(MAP_EMPTY, &[]);
            let [hasher, equality] = lowering.value_ops.both(builder, module, &takes[0]);
            let key = into_slot(builder, *key);
            let value = into_slot(builder, *value);
            put_in(
                builder,
                lowering,
                module,
                MAP_INSERT,
                &[empty, key, value, hasher, equality],
            )
        }
        (LoweredKernel::SetInsert, [value, set]) => {
            let [hasher, equality] = lowering.value_ops.both(builder, module, &takes[0]);
            let value = into_slot(builder, *value);
            written_or_ended(
                builder,
                lowering,
                module,
                abort,
                SET_INSERT,
                &[*set, value, hasher, equality],
                POINTER,
                aborts,
            )
        }
        (LoweredKernel::MapInsert, [key, value, map]) => {
            let [hasher, equality] = lowering.value_ops.both(builder, module, &takes[0]);
            let key = into_slot(builder, *key);
            let value = into_slot(builder, *value);
            written_or_ended(
                builder,
                lowering,
                module,
                abort,
                MAP_INSERT,
                &[*map, key, value, hasher, equality],
                POINTER,
                aborts,
            )
        }
        (LoweredKernel::SetUnion, [a, b]) => {
            let equality = equality_of(builder, lowering, module, &takes[0]);
            written_or_ended(
                builder,
                lowering,
                module,
                abort,
                SET_UNION,
                &[*a, *b, equality],
                POINTER,
                aborts,
            )
        }
        (
            LoweredKernel::SetRemove
            | LoweredKernel::SetContains
            | LoweredKernel::MapGet
            | LoweredKernel::MapContainsKey
            | LoweredKernel::MapRemove,
            [key, collection],
        ) => {
            let name = match kernel {
                LoweredKernel::SetRemove => SET_REMOVE,
                LoweredKernel::SetContains => SET_CONTAINS,
                LoweredKernel::MapGet => MAP_GET,
                LoweredKernel::MapContainsKey => MAP_CONTAINS_KEY,
                _ => MAP_REMOVE,
            };
            let [hasher, equality] = lowering.value_ops.both(builder, module, &takes[0]);
            let key = into_slot(builder, *key);
            runtime_call(
                builder,
                lowering,
                module,
                name,
                &[*collection, key, hasher, equality],
            )
        }
        (LoweredKernel::SetIntersection | LoweredKernel::SetDifference, [a, b]) => {
            let name = if kernel == LoweredKernel::SetIntersection {
                SET_INTERSECTION
            } else {
                SET_DIFFERENCE
            };
            let equality = equality_of(builder, lowering, module, &takes[0]);
            runtime_call(builder, lowering, module, name, &[*a, *b, equality])
        }
        (LoweredKernel::SetIsEmpty | LoweredKernel::MapIsEmpty, [collection]) => {
            let name = if kernel == LoweredKernel::SetIsEmpty {
                SET_SIZE
            } else {
                MAP_SIZE
            };
            let size = call(name, &[*collection]);
            builder.ins().icmp_imm_s(IntCC::Equal, size, 0)
        }
        (LoweredKernel::SetSize, [set]) => call(SET_SIZE, &[*set]),
        (LoweredKernel::MapSize, [map]) => call(MAP_SIZE, &[*map]),
        (LoweredKernel::SetToList, [set]) => call(SET_TO_LIST, &[*set]),
        (LoweredKernel::MapKeys, [map]) => call(MAP_KEYS, &[*map]),
        (LoweredKernel::MapValues, [map]) => call(MAP_VALUES, &[*map]),
        (LoweredKernel::MapToList, [map]) => call(MAP_TO_LIST, &[*map]),
        (LoweredKernel::SetFromList, [list]) => {
            let Ty::List { list: element } = &takes[0] else {
                unreachable!("`Coherent` held set.fromList to take a list");
            };
            let [hasher, equality] = lowering.value_ops.both(builder, module, element);
            runtime_call(
                builder,
                lowering,
                module,
                SET_FROM_LIST,
                &[*list, hasher, equality],
            )
        }
        (LoweredKernel::MapFromList, [list]) => {
            let Ty::List { list: pair } = &takes[0] else {
                unreachable!("`Coherent` held map.fromList to take a list");
            };
            let Ty::Tuple { tuple } = &**pair else {
                unreachable!("`Coherent` held map.fromList to take a list of pairs");
            };
            let [hasher, equality] = lowering.value_ops.both(builder, module, &tuple[0]);
            runtime_call(
                builder,
                lowering,
                module,
                MAP_FROM_LIST,
                &[*list, hasher, equality],
            )
        }
        _ => unreachable!(
            "`Coherent` held {kernel:?} to be a kernel of `Set` or `Map` applied to the arguments \
             it takes"
        ),
    }
}

/// The equality of a set's members, where `set` is the type of the set.
fn equality_of(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    set: &Ty,
) -> ir::Value {
    let Ty::Set { set: element } = set else {
        unreachable!("`Coherent` held a set's algebra to take sets");
    };
    lowering
        .value_ops
        .address(builder, module, crate::hashing::Kind::Equality, element)
}

/// The collection an insertion into an empty one made. One entry is never more than a set or a map
/// holds, so what the runtime answers about whether it wrote one is not read.
fn put_in(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    name: &str,
    handed: &[ir::Value],
) -> ir::Value {
    let room = out_slot(builder);
    let mut handed = handed.to_vec();
    handed.push(room);
    runtime_call(builder, lowering, module, name, &handed);
    builder.ins().load(POINTER, TRUSTED, room, 0)
}
