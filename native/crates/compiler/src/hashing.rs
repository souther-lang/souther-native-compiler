//! What a value hashes to, and the two functions of a type's that the runtime calls where it keeps
//! a `Set` or a `Map`.
//!
//! The runtime keeps a set as a trie over hashes and knows no type (`souther_native_abi::SET_EMPTY`),
//! so what an element hashes to and what it is equal to are handed to it by the site that asks, as
//! the addresses of two functions of this object's, one pair per type: a hasher, taking a value as
//! it stands in a slot, and an equality, taking two. Neither is kept in the set. An element stands
//! as a wider type than it was put in as, and a site asking of a `Set<S>` hands `S`'s functions over
//! whatever the set was built as.
//!
//! What makes that sound is how a hash is composed, which `souther_native_abi::HASHING` states and
//! [`hash`] and [`define`] write: a value hashes as the case it is, whatever sum it is read as, so
//! the hash an element was put in under is the hash of it under every type it can stand as without
//! being rebuilt. Where standing wider rebuilds a value (a primitive carried as a case of a union),
//! the set is rebuilt with it (`restating`), and every element is hashed again under the wider
//! type.
//!
//! Each function is given an id the first time a site asks for it and written once every body is,
//! as a comparator is (`equality`): a type is reached from inside its own hash as often as not.

use std::cell::RefCell;
use std::collections::HashMap;

use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, AbiParam, Function, InstBuilder, types};
use cranelift::codegen::isa::{CallConv, TargetFrontendConfig};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift::module::{FuncId, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    CARRIED, DATE_HASH, DATETIME_HASH, DECIMAL_HASH, HASH_COMBINE, HASH_PRESENT, HASH_START, HELD,
    INSTANT_HASH, LIST_ELEMENTS, LIST_LENGTH, MAP_HASH, NOTHING, RATIONAL_HASH, SET_HASH, SLOT,
    STRING_HASH, TIME_HASH, field_at, member_at,
};

use crate::transport::{Case, Declaration, Prim, Ty};
use crate::{
    Lowered, Lowerings, POINTER, TRUSTED, Tagged, accepted, into_slot, machine_type, not_lowered,
    out_of_slot, runtime_call, token_of,
};

/// Which of a type's two functions one is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Kind {
    /// A value as it stands in a slot, and its hash.
    Hasher,
    /// Two values as they stand in slots, and whether they are equal: nought or one.
    Equality,
}

/// The hasher and the equality of each type a set or a map in this object is kept over, and the
/// ones whose body is still to be written.
#[derive(Default)]
pub(crate) struct ValueOps {
    by_type: RefCell<HashMap<(Kind, Ty), FuncId>>,
    owed: RefCell<Vec<Owed>>,
}

/// A function declared and not yet written.
pub(crate) struct Owed {
    pub(crate) id: FuncId,
    pub(crate) kind: Kind,
    pub(crate) ty: Ty,
    pub(crate) signature: ir::Signature,
}

impl ValueOps {
    /// The function of `kind` for `ty`, given an id now where none was yet and owed a body until
    /// [`ValueOps::owed`] hands it out.
    ///
    /// Declared whatever `ty` is: a set of a type no value of which is made still hands the
    /// runtime a hasher, which nothing calls.
    pub(crate) fn of(
        &self,
        module: &mut ObjectModule,
        kind: Kind,
        ty: &Ty,
        call_conv: CallConv,
    ) -> FuncId {
        let key = (kind, ty.clone());
        if let Some(id) = self.by_type.borrow().get(&key) {
            return *id;
        }
        let mut signature = ir::Signature::new(call_conv);
        signature.params.push(AbiParam::new(types::I64));
        match kind {
            Kind::Hasher => signature.returns.push(AbiParam::new(types::I64)),
            Kind::Equality => {
                signature.params.push(AbiParam::new(types::I64));
                signature.returns.push(AbiParam::new(types::I8));
            }
        }
        let id = accepted(module.declare_anonymous_function(&signature));
        crate::index::unique(&mut *self.by_type.borrow_mut(), key, id);
        self.owed.borrow_mut().push(Owed {
            id,
            kind,
            ty: ty.clone(),
            signature,
        });
        id
    }

    /// The address of the function of `kind` for `ty`, which is what the runtime is handed.
    pub(crate) fn address(
        &self,
        builder: &mut FunctionBuilder,
        module: &mut ObjectModule,
        kind: Kind,
        ty: &Ty,
    ) -> ir::Value {
        let call_conv = builder.func.signature.call_conv;
        let id = self.of(module, kind, ty, call_conv);
        let reached = module.declare_func_in_func(id, builder.func);
        builder.ins().func_addr(POINTER, reached)
    }

    /// The hasher and then the equality of `ty`, as the runtime takes them.
    pub(crate) fn both(
        &self,
        builder: &mut FunctionBuilder,
        module: &mut ObjectModule,
        ty: &Ty,
    ) -> [ir::Value; 2] {
        [
            self.address(builder, module, Kind::Hasher, ty),
            self.address(builder, module, Kind::Equality, ty),
        ]
    }

    /// A function whose body is still to be written. Writing one may owe more, so this is asked
    /// until it answers nothing.
    pub(crate) fn owed(&self) -> Option<Owed> {
        self.owed.borrow_mut().pop()
    }
}

/// `word` composed into `hash`, in order.
fn combine(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    hash: ir::Value,
    word: ir::Value,
) -> ir::Value {
    let word = into_slot(builder, word);
    runtime_call(builder, lowering, module, HASH_COMBINE, &[hash, word])
}

/// How a value of a type is hashed, as far as the type decides it: what [`hash`] and a hasher's
/// body are written from, and what the one rule a hash is held to is asked of.
///
/// The rule is that a value standing as a wider type without being rebuilt keeps the hash it was
/// built with (`souther_native_abi::HASHING`): a set built as a `Set<A>` is read as a `Set<S>` with
/// the hashes it holds. Which pairs stand so is [`restatement`](crate::restating::restatement)'s
/// to say, when it answers `Same`, and [`Plan::agrees`] is the rule over two plans: every value of
/// the narrower type hashes alike under both. A test holds every such pair to it, so a plan that
/// answers one way for a type and another way for the type it stands as is caught where it is
/// written, and not by a set that misses a member.
///
/// A type no value of which is made is [`Plan::Unreached`], which agrees with anything, and it is
/// the one way a plan says so: a container of it is planned as a container, so an empty one hashes
/// as an empty one of anything does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    /// The word itself, combined once: an `Int`, a `Bool`.
    Word,
    /// The runtime's hash of it.
    Runtime(&'static str),
    /// As the case the value is, told by its token: a declared type, a union, a case the language
    /// gives. A value hashes the same as its case and as every sum it stands as.
    AsItsCase,
    /// Nothing where absent, and what it holds where present.
    Option(Box<Plan>),
    /// Each member, in order.
    Tuple(Vec<Plan>),
    /// Its length, and each element in order.
    List(Box<Plan>),
    /// The runtime's hash of it, from the hashes its members were kept under.
    Set(Box<Plan>),
    /// The runtime's hash of it, from its keys' kept hashes and its values' hashes.
    Map { key: Box<Plan>, value: Box<Plan> },
    /// No value of it is made, so nothing is hashed.
    Unreached,
}

impl Plan {
    /// How a value of `ty` is hashed.
    pub(crate) fn of(ty: &Ty) -> Lowered<Plan> {
        Ok(match ty {
            // Every primitive is named, for the reason `machine_type` names them.
            Ty::Prim { prim } => match prim {
                Prim::Int | Prim::Bool => Plan::Word,
                Prim::String => Plan::Runtime(STRING_HASH),
                Prim::Decimal => Plan::Runtime(DECIMAL_HASH),
                Prim::Rational => Plan::Runtime(RATIONAL_HASH),
                Prim::Date => Plan::Runtime(DATE_HASH),
                Prim::Time => Plan::Runtime(TIME_HASH),
                Prim::DateTime => Plan::Runtime(DATETIME_HASH),
                Prim::Instant => Plan::Runtime(INSTANT_HASH),
            },
            // The checker gives a function no equality, so nothing asks what one hashes to.
            Ty::Fn { .. } => return Err(not_lowered(format!("the hash of {}", ty.spelt()))),
            Ty::Var { var } => return Err(crate::open_type(*var)),
            Ty::Ref {
                named: Case::Primitive { .. },
            } => return crate::named_as_a_type(ty),
            Ty::Nothing { .. } | Ty::Never { .. } => Plan::Unreached,
            Ty::Ref {
                named: Case::Declared { .. } | Case::Language { .. },
            }
            | Ty::Union { .. } => Plan::AsItsCase,
            Ty::Option { option } => Plan::Option(Box::new(Plan::of(option)?)),
            Ty::List { list } => Plan::List(Box::new(Plan::of(list)?)),
            Ty::Set { set } => Plan::Set(Box::new(Plan::of(set)?)),
            Ty::Map { map } => Plan::Map {
                key: Box::new(Plan::of(&map.key)?),
                value: Box::new(Plan::of(&map.value)?),
            },
            // A tuple one of whose members has no value has none either.
            Ty::Tuple { tuple } => {
                let members = tuple.iter().map(Plan::of).collect::<Lowered<Vec<_>>>()?;
                if members.contains(&Plan::Unreached) {
                    Plan::Unreached
                } else {
                    Plan::Tuple(members)
                }
            }
        })
    }

    /// Whether every value hashed by this plan hashes alike by `wider`: what a value standing as a
    /// wider type without being rebuilt asks of the two. Asked by the test that holds every such
    /// pair to it, and by nothing that runs.
    #[cfg(test)]
    pub(crate) fn agrees(&self, wider: &Plan) -> bool {
        match (self, wider) {
            (Plan::Unreached, _) => true,
            (Plan::Word, Plan::Word) | (Plan::AsItsCase, Plan::AsItsCase) => true,
            (Plan::Runtime(one), Plan::Runtime(other)) => one == other,
            (Plan::Option(one), Plan::Option(other))
            | (Plan::List(one), Plan::List(other))
            | (Plan::Set(one), Plan::Set(other)) => one.agrees(other),
            (Plan::Tuple(one), Plan::Tuple(other)) => {
                one.len() == other.len() && one.iter().zip(other).all(|(it, also)| it.agrees(also))
            }
            (
                Plan::Map { key, value },
                Plan::Map {
                    key: wider_key,
                    value: wider_value,
                },
            ) => key.agrees(wider_key) && value.agrees(wider_value),
            _ => false,
        }
    }
}

/// The hash of `value`, a value of `ty` as the machine holds it, composed as its [`Plan`] says and
/// `souther_native_abi::HASHING` states.
pub(crate) fn hash(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    ty: &Ty,
    value: ir::Value,
) -> Lowered<ir::Value> {
    Ok(match Plan::of(ty)? {
        Plan::Word => {
            let start = builder.ins().iconst(types::I64, HASH_START);
            combine(builder, lowering, module, start, value)
        }
        Plan::Runtime(through) => runtime_call(builder, lowering, module, through, &[value]),
        // Nothing asks what a value of it hashes to; a hasher of it is still handed to the runtime
        // (`define`).
        Plan::Unreached => builder.ins().iconst(types::I64, HASH_START),
        Plan::Set(_) => runtime_call(builder, lowering, module, SET_HASH, &[value]),
        Plan::Map { .. } => {
            let Ty::Map { map } = ty else {
                unreachable!("a map's plan is a map's");
            };
            let values = lowering
                .value_ops
                .address(builder, module, Kind::Hasher, &map.value);
            runtime_call(builder, lowering, module, MAP_HASH, &[value, values])
        }
        Plan::AsItsCase | Plan::Option(_) | Plan::Tuple(_) | Plan::List(_) => {
            let call_conv = builder.func.signature.call_conv;
            let id = lowering.value_ops.of(module, Kind::Hasher, ty, call_conv);
            let hashing = module.declare_func_in_func(id, builder.func);
            let slot = into_slot(builder, value);
            let hashed = builder.ins().call(hashing, &[slot]);
            builder.inst_results(hashed)[0]
        }
    })
}

/// The body of a function [`ValueOps::of`] declared.
pub(crate) fn define(
    func: &mut Function,
    shapes: &mut FunctionBuilderContext,
    owed: &Owed,
    frontend: TargetFrontendConfig,
    lowering: &Lowerings,
    module: &mut ObjectModule,
) -> Lowered<()> {
    let mut builder = FunctionBuilder::new(func, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    let given = builder.block_params(entry).to_vec();
    let ty = &owed.ty;
    match owed.kind {
        // Never called where no value of the type is made, and written all the same, since the
        // runtime is handed its address.
        Kind::Hasher if Plan::of(ty)? == Plan::Unreached => {
            let start = builder.ins().iconst(types::I64, HASH_START);
            builder.ins().return_(&[start]);
        }
        Kind::Hasher => {
            let value = out_of_slot(&mut builder, given[0], machine_type(ty)?);
            let hashed = Hashing {
                builder: &mut builder,
                lowering,
                module,
            }
            .body(ty, value)?;
            builder.ins().return_(&[hashed]);
        }
        Kind::Equality if ty.has_no_value() => {
            let same = builder.ins().iconst(types::I8, 1);
            builder.ins().return_(&[same]);
        }
        Kind::Equality => {
            let held = machine_type(ty)?;
            let a = out_of_slot(&mut builder, given[0], held);
            let b = out_of_slot(&mut builder, given[1], held);
            let same = crate::equality::equal(&mut builder, lowering, module, ty, a, b)?;
            builder.ins().return_(&[same]);
        }
    }
    builder.seal_all_blocks();
    builder.finalize(frontend);
    Ok(())
}

/// One hasher's body being written.
struct Hashing<'b, 'f, 'l, 'm> {
    builder: &'b mut FunctionBuilder<'f>,
    lowering: &'l Lowerings<'l>,
    module: &'m mut ObjectModule,
}

impl Hashing<'_, '_, '_, '_> {
    fn body(&mut self, ty: &Ty, value: ir::Value) -> Lowered<ir::Value> {
        match ty {
            Ty::Ref {
                named: named @ Case::Declared { declared },
            } => match self.lowering.declared.laid(declared) {
                Declaration::Product { fields, .. } => {
                    let types: Vec<Ty> = fields.iter().map(|it| it.codec.ty()).collect();
                    let tagged = self.tagged(named)?;
                    self.slots(tagged, &types, field_at, value)
                }
                Declaration::Newtype { field, .. } => {
                    let tagged = self.tagged(named)?;
                    self.slots(tagged, &[field.codec.ty()], field_at, value)
                }
                Declaration::Unit { .. } => self.tagged(named),
                Declaration::Sum { cases, .. } => self.cases(ty, cases, value),
            },
            Ty::Ref {
                named: named @ Case::Language { .. },
            } => self.tagged(named),
            Ty::Union { union } => self.cases(ty, union, value),
            Ty::Option { option } => self.optional(option, value),
            Ty::Tuple { tuple } => {
                let start = self.builder.ins().iconst(types::I64, HASH_START);
                self.slots(start, tuple, member_at, value)
            }
            Ty::List { list } => self.list(list, value),
            Ty::Prim { .. }
            | Ty::Ref {
                named: Case::Primitive { .. },
            }
            | Ty::Fn { .. }
            | Ty::Set { .. }
            | Ty::Map { .. }
            | Ty::Var { .. }
            | Ty::Nothing { .. }
            | Ty::Never { .. } => hash(self.builder, self.lowering, self.module, ty, value),
        }
    }

    /// Where a value carrying `case`'s token starts from: the token combined into the start.
    fn tagged(&mut self, case: &Case) -> Lowered<ir::Value> {
        let token = token_of(self.builder, self.lowering.declared, self.module, case)?;
        let start = self.builder.ins().iconst(types::I64, HASH_START);
        Ok(combine(
            self.builder,
            self.lowering,
            self.module,
            start,
            token,
        ))
    }

    /// `from` with each slot's hash combined in, in order.
    fn slots(
        &mut self,
        from: ir::Value,
        types: &[Ty],
        at: fn(usize) -> i64,
        value: ir::Value,
    ) -> Lowered<ir::Value> {
        let mut hashed = from;
        for (position, ty) in types.iter().enumerate() {
            let held = self
                .builder
                .ins()
                .load(types::I64, TRUSTED, value, at(position) as i32);
            let held = out_of_slot(self.builder, held, machine_type(ty)?);
            let one = hash(self.builder, self.lowering, self.module, ty, held)?;
            hashed = combine(self.builder, self.lowering, self.module, hashed, one);
        }
        Ok(hashed)
    }

    /// As the case the value is, by the token it carries: a declared case by its own hasher, a
    /// primitive a union carries by its token and then what it carries, and a case the language
    /// gives by its token alone. The last case is the one a value tagged by none of the others is,
    /// as a comparison reads it.
    fn cases(&mut self, ty: &Ty, members: &[Case], value: ir::Value) -> Lowered<ir::Value> {
        let which = Tagged::of(value, ty).which(self.builder);
        let leaves = self
            .lowering
            .declared
            .leaves_of(members)
            .expect("`Coherent` held every case named to be one a declaration crossed for");
        let answered = self.builder.create_block();
        self.builder.append_block_param(answered, types::I64);
        for (at, leaf) in leaves.iter().enumerate() {
            if at + 1 < leaves.len() {
                let tag = token_of(self.builder, self.lowering.declared, self.module, leaf)?;
                let is_it = self.builder.ins().icmp(IntCC::Equal, which, tag);
                let taken = self.builder.create_block();
                let next = self.builder.create_block();
                self.builder.ins().brif(is_it, taken, &[], next, &[]);
                self.builder.switch_to_block(taken);
                let hashed = self.as_the_case(leaf, value)?;
                self.builder.ins().jump(answered, &[hashed.into()]);
                self.builder.switch_to_block(next);
            } else {
                let hashed = self.as_the_case(leaf, value)?;
                self.builder.ins().jump(answered, &[hashed.into()]);
            }
        }
        self.builder.switch_to_block(answered);
        Ok(self.builder.block_params(answered)[0])
    }

    /// The hash of `value`, known to be the case `leaf`, as that case.
    fn as_the_case(&mut self, leaf: &Case, value: ir::Value) -> Lowered<ir::Value> {
        match leaf {
            Case::Declared { declared } => hash(
                self.builder,
                self.lowering,
                self.module,
                &Ty::declared(declared.clone()),
                value,
            ),
            Case::Primitive { prim } => {
                let tagged = self.tagged(leaf)?;
                let as_it = Ty::Prim { prim: *prim };
                self.slots(tagged, &[as_it], |_| CARRIED, value)
            }
            Case::Language { .. } => self.tagged(leaf),
        }
    }

    /// The start where it is absent, and what it holds combined with the mark of one present where
    /// it is not.
    fn optional(&mut self, held: &Ty, value: ir::Value) -> Lowered<ir::Value> {
        // Never present, so what it holds is never read.
        if Plan::of(held)? == Plan::Unreached {
            return Ok(self.builder.ins().iconst(types::I64, HASH_START));
        }
        let absent = self.builder.ins().icmp_imm_s(IntCC::Equal, value, NOTHING);
        let there = self.builder.create_block();
        let answered = self.builder.create_block();
        self.builder.append_block_param(answered, types::I64);
        let start = self.builder.ins().iconst(types::I64, HASH_START);
        self.builder
            .ins()
            .brif(absent, answered, &[start.into()], there, &[]);

        self.builder.switch_to_block(there);
        let present = self.builder.ins().iconst(types::I64, HASH_PRESENT);
        let hashed = self.slots(present, std::slice::from_ref(held), |_| HELD, value)?;
        self.builder.ins().jump(answered, &[hashed.into()]);

        self.builder.switch_to_block(answered);
        Ok(self.builder.block_params(answered)[0])
    }

    /// The length, and then each element in order.
    fn list(&mut self, element: &Ty, value: ir::Value) -> Lowered<ir::Value> {
        let length = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, value, LIST_LENGTH as i32);
        let start = self.builder.ins().iconst(types::I64, HASH_START);
        let from = combine(self.builder, self.lowering, self.module, start, length);
        // Empty, so no element is read.
        if Plan::of(element)? == Plan::Unreached {
            return Ok(from);
        }

        let head = self.builder.create_block();
        self.builder.append_block_param(head, types::I64);
        self.builder.append_block_param(head, types::I64);
        let step = self.builder.create_block();
        let walked = self.builder.create_block();
        self.builder.append_block_param(walked, types::I64);
        let first = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().jump(head, &[first.into(), from.into()]);

        self.builder.switch_to_block(head);
        let index = self.builder.block_params(head)[0];
        let hashed = self.builder.block_params(head)[1];
        let inside = self
            .builder
            .ins()
            .icmp(IntCC::SignedLessThan, index, length);
        self.builder
            .ins()
            .brif(inside, step, &[], walked, &[hashed.into()]);

        self.builder.switch_to_block(step);
        let along = self.builder.ins().imul_imm_s(index, SLOT);
        let at = self.builder.ins().iadd(value, along);
        let held = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, at, LIST_ELEMENTS as i32);
        let held = out_of_slot(self.builder, held, machine_type(element)?);
        let one = hash(self.builder, self.lowering, self.module, element, held)?;
        let hashed = combine(self.builder, self.lowering, self.module, hashed, one);
        let next = self.builder.ins().iadd_imm_s(index, 1);
        self.builder.ins().jump(head, &[next.into(), hashed.into()]);

        self.builder.switch_to_block(walked);
        Ok(self.builder.block_params(walked)[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::restating::{Restatement, restatement};
    use crate::transport::{Bottom, Cases, LanguageCase, MapTy};

    /// Every type of the ones below, and every one of them held in an optional, a list, a set, a
    /// map's key or value, or a pair, two deep.
    fn types() -> Vec<Ty> {
        let a = Ty::declared("m.A");
        let s = Ty::declared("m.S");
        let leaves = vec![
            Ty::Prim { prim: Prim::Int },
            Ty::Prim { prim: Prim::Bool },
            Ty::Prim { prim: Prim::String },
            Ty::Prim {
                prim: Prim::Decimal,
            },
            Ty::Nothing { nothing: Bottom {} },
            a.clone(),
            s,
            Ty::Union {
                union: Cases::one_or_more(vec![
                    Case::Declared {
                        declared: "m.A".to_string(),
                    },
                    Case::Primitive { prim: Prim::Int },
                ])
                .unwrap(),
            },
            Ty::Ref {
                named: Case::Language {
                    case: LanguageCase::NotANumber,
                },
            },
        ];
        let mut all = leaves.clone();
        let mut level = leaves;
        for _ in 0..2 {
            let mut next = Vec::new();
            for held in &level {
                let boxed = || Box::new(held.clone());
                next.push(Ty::Option { option: boxed() });
                next.push(Ty::List { list: boxed() });
                next.push(Ty::Set { set: boxed() });
                next.push(Ty::Map {
                    map: MapTy {
                        key: boxed(),
                        value: Box::new(Ty::Prim { prim: Prim::Int }),
                    },
                });
                next.push(Ty::Map {
                    map: MapTy {
                        key: Box::new(a.clone()),
                        value: boxed(),
                    },
                });
                next.push(Ty::Tuple {
                    tuple: vec![held.clone(), Ty::Prim { prim: Prim::Int }],
                });
            }
            all.extend(next.iter().cloned());
            level = next;
        }
        all
    }

    /// A value that stands as a wider type without being rebuilt hashes alike under both, for
    /// every pair of types [`restatement`] answers `Same` for: a case as its sum, and what has no
    /// value as anything, at any depth in anything that holds it. What an empty set of what has no
    /// value hashed to once differed from what an empty set of `Int` hashes to, and a set holding
    /// one missed it once widened.
    #[test]
    fn a_value_standing_as_a_wider_type_hashes_as_it_did() {
        let types = types();
        let mut asked = 0;
        for from in &types {
            for to in &types {
                if from == to || !matches!(restatement(from, to), Ok(Restatement::Same)) {
                    continue;
                }
                let (Ok(narrow), Ok(wide)) = (Plan::of(from), Plan::of(to)) else {
                    continue;
                };
                asked += 1;
                assert!(
                    narrow.agrees(&wide),
                    "{} stands as {} and hashes as {narrow:?} against {wide:?}",
                    from.spelt(),
                    to.spelt()
                );
            }
        }
        assert!(asked > 1000, "only {asked} pairs asked");
    }

    /// And the rule is one that can fail: a plan that answered where every hash starts for a
    /// container of what has no value disagrees with the container it stands as.
    #[test]
    fn a_container_planned_as_nothing_would_disagree() {
        let ints = Plan::of(&Ty::Set {
            set: Box::new(Ty::Prim { prim: Prim::Int }),
        })
        .unwrap();
        assert!(!Plan::Word.agrees(&ints));
        assert!(Plan::Set(Box::new(Plan::Unreached)).agrees(&ints));
    }
}
