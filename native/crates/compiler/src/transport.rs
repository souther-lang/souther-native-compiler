//! What the Java half wrote, read back.
//!
//! These types are the document's shape and not a model of Souther. Nothing here decides anything
//! about what a program means: every question was answered by the checker, and what crosses is the
//! answer. A field added here means a field the writer started writing, not a new thing to work out
//! on this side.
//!
//! The vocabularies are whole. Every operator and every primitive the language has is read here,
//! including the ones no lowering exists for, because what this side cannot read and what it cannot
//! lower are different answers and a reader that knows only what is lowered gives the first when
//! the second is true.
//!
//! A document is read strictly: a field nothing here names is a writer saying something this driver
//! has no idea it was told, and reading past it would be reading a program that means more than
//! what was understood of it.

use serde::Deserialize;

/// What this side reads. A document written to say anything else is refused rather than read as
/// much of as happens to parse.
///
/// The last of [`MOVES`], and written nowhere else on this side.
pub const TRANSPORT_VERSION: u32 = MOVES[MOVES.len() - 1].0;

/// What each version moved, since the one before it, oldest first. The versions before the first
/// here are in the history of this file.
///
/// A change to what the document means adds its line at the end, under the next number. That is
/// what the list is for, beside saying what moved: two branches that each move the document to the
/// same number each add a different line at one place, which a merge stops at. Two edits of one
/// constant to the same number merge without a word, which is how two different documents were
/// both once written as 17. That the numbers follow on from one another is held by a test.
pub const MOVES: &[(u32, &str)] = &[
    (
        17,
        "an attempted construction (`attempt`), and what another build's clauses are answered \
         under (`headers`)",
    ),
    (
        18,
        "what constructing a behavior requires injected, beside its body or its composition \
         (`requirements`)",
    ),
    (
        19,
        "a helper, and a call of one, under the reference a call reaches it by (`reached`), as the \
         route and the declaration it reaches, and a type variable a helper's body leaves open \
         (`var`)",
    ),
    (
        20,
        "an operation the checker's compiler emits for a backend to lower whole, as the member it \
         is (`emitted`), and the type of what has no value (`nothing`)",
    ),
    (
        21,
        "what a row states each dependency of its behavior answers, entry by entry and for the \
         rest (`standsIn`)",
    ),
    (
        22,
        "what constructing a behavior requires injected, on the behavior's target wherever it is \
         reached (`requirements`), and no longer beside a definition",
    ),
    (
        23,
        "what a `String.matches` pattern means as the checker read it (`meaning`), beside the text \
         it was written as (`written`), in place of the text alone",
    ),
    (
        24,
        "a type named wherever the checker names one, as which of the three it is: a type \
         reference (`ref`), what a field, a key or a boundary is named by (`named`), a unit \
         (`unit`); the type of what does not answer (`never`), and where the run ends \
         (`unreachable`)",
    ),
    (
        25,
        "a `Decimal` literal (`decimal`), as the integer and the scale the checker read it as",
    ),
];

/// A document of [`TRANSPORT_VERSION`], and no other, read through [`Program::read`] and nothing
/// else ([`crate::versioned`]).
#[derive(Debug)]
pub struct Program {
    pub transport: u32,
    pub declarations: Vec<Declaration>,
    /// Every behavior the program names, which is wider than what it emits: a body may reach a
    /// behavior a module read off the path declares, and that module is not one of these.
    pub behaviors: Vec<Target>,
    pub modules: Vec<Module>,
}

/// A [`Program`] as the document writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WrittenProgram {
    transport: u32,
    declarations: Vec<Declaration>,
    behaviors: Vec<Target>,
    modules: Vec<Module>,
}

/// What a document says it is.
#[derive(Deserialize)]
struct Says {
    transport: u32,
}

impl Program {
    /// The program `document` holds, where it is a document of [`TRANSPORT_VERSION`]. One of any
    /// other version is refused as that, before anything else in it is read.
    pub fn read(document: &str) -> anyhow::Result<Program> {
        let written: WrittenProgram = crate::versioned::read(document.as_bytes(), |says: Says| {
            if says.transport == TRANSPORT_VERSION {
                Ok(())
            } else {
                Err(format!(
                    "this driver reads transport {TRANSPORT_VERSION} and was handed {}",
                    says.transport
                ))
            }
        })?;
        let mut modules = written.modules;
        // What a local definition requires is what its target says, read off the one place the
        // document writes it. A definition no target names keeps nothing, and is refused as that
        // where the two are held together (`Coherent`).
        let required: std::collections::HashMap<String, &[Requirement]> = written
            .behaviors
            .iter()
            .map(|target| (target.declared(), target.requirements.as_slice()))
            .collect();
        for module in &mut modules {
            for definition in &mut module.definitions {
                let copied = required
                    .get(definition.declared())
                    .map(|it| it.to_vec())
                    .unwrap_or_default();
                match definition {
                    Definition::Body { requirements, .. }
                    | Definition::Composed { requirements, .. } => *requirements = copied,
                }
            }
        }
        Ok(Program {
            transport: written.transport,
            declarations: written.declarations,
            behaviors: written.behaviors,
            modules,
        })
    }
}

impl Program {
    /// Every body of `Core` the document holds, with the module it stands in and what owns it.
    ///
    /// What the document says, read whole: whether the two halves agree about a body is asked of
    /// every one of them. It is not what the object runs. A clause of a declaration no value of
    /// which is built here is a body here and is run nowhere here, so a pass asking what the object
    /// emits (the closure sites it lifts, the published values it imports, what it constructs) walks
    /// the object's own `Runs`, which narrows this, and never this. `Module` is taken apart whole,
    /// so a field it starts carrying tomorrow does not compile here until it is said whether it
    /// holds a body.
    ///
    /// A clause a declaration holds its values to is a body as much as a behavior's is. It stands
    /// in the declaration's own module, whose copy of a helper a call from it reaches. So is a rule a
    /// behavior's answer is held to, in the module that declares the behavior.
    pub fn bodies(&self) -> impl Iterator<Item = Body<'_>> {
        let clauses = self.declarations.iter().flat_map(|declaration| {
            declaration
                .clauses()
                .unwrap_or_default()
                .iter()
                .enumerate()
                .map(move |(at, clause)| {
                    Body::at(
                        declaration.module(),
                        Owner::Invariant { declaration, at },
                        &clause.condition,
                    )
                })
        });
        let modules = self.modules.iter().flat_map(|written| {
            let Module {
                name,
                // What the module publishes of its data names declarations and holds no body.
                publishes: _,
                helpers,
                values,
                entries,
                definitions,
                examples,
            } = written;
            let module = name.as_str();
            let helpers = helpers
                .iter()
                .map(move |it| Body::at(module, Owner::Helper(it), &it.body));
            let values = values
                .iter()
                .map(move |it| Body::at(module, Owner::Value(it), &it.body));
            let entries = entries
                .iter()
                .map(move |it| Body::at(module, Owner::Entry(it), &it.body));
            let definitions = definitions.iter().filter_map(move |it| match it {
                Definition::Body {
                    declared,
                    body,
                    requirements,
                    ..
                } => Some(Body {
                    environment: requirements,
                    ..Body::at(module, Owner::Definition(declared), body)
                }),
                // Stages reach other behaviors by name, and there is no `Core` of its own.
                Definition::Composed { .. } => None,
            });
            let examples = examples.iter().flat_map(move |it| {
                let stood = it
                    .stands_in
                    .iter()
                    .enumerate()
                    .flat_map(move |(at, stand_in)| {
                        stand_in.values().map(move |value| {
                            Body::at(module, Owner::StoodIn { example: it, at }, value)
                        })
                    });
                std::iter::once(Body::at(module, Owner::Example(it), &it.body)).chain(stood)
            });
            helpers
                .chain(values)
                .chain(entries)
                .chain(definitions)
                .chain(examples)
        });
        // A rule a behavior's answer is held to is a body as much as a clause is. It stands in the
        // module that declares the behavior, wherever the check is run from: the rule is that
        // module's, and a call from it reaches that module's copy of a helper.
        let rules = self.behaviors.iter().flat_map(|target| {
            target
                .ensures
                .contract()
                .map(|contract| contract.rules.as_slice())
                .unwrap_or_default()
                .iter()
                .enumerate()
                .map(move |(at, rule)| {
                    Body::at(
                        &target.module,
                        Owner::Ensures { target, at },
                        &rule.condition,
                    )
                })
        });
        clauses.chain(rules).chain(modules)
    }
}

/// One body of `Core`, where it stands, and what owns it.
///
/// Made by [`Program::bodies`] and nowhere else, which the module it stands in being private to
/// this file holds to. Where a body stands decides what a call from it reaches, and [`Coherent`]
/// holds a body's calls to what is reachable from where it stands: a body made anywhere else could
/// say it stood somewhere its calls were never held to. It is read as a [`Carrier`].
///
/// [`Coherent`]: crate::coherent::Coherent
#[derive(Clone, Copy)]
pub struct Body<'p> {
    /// The module it stands in, whose copy of a helper a call from it reaches.
    module: &'p str,
    pub owner: Owner<'p>,
    pub node: &'p Node,
    /// What the function it is lowered as is handed a capability for, one each, in order.
    environment: &'p [Requirement],
}

impl<'p> Body<'p> {
    fn at(module: &'p str, owner: Owner<'p>, node: &'p Node) -> Self {
        Body {
            module,
            owner,
            node,
            environment: &[],
        }
    }

    /// The behaviors a call from this body reaches through a capability it was handed, in the
    /// order it was handed them: what the behavior whose body it is was constructed with, and
    /// nothing for any other body. A call reaching any other behavior reaches its symbol.
    pub fn environment(&self) -> &'p [Requirement] {
        self.environment
    }

    /// Where a call from this body is resolved.
    pub fn carrier(&self) -> Carrier<'p> {
        Carrier(self.module)
    }

    /// Whether this is the body of a helper that leaves type variables open. Such a body is not
    /// lowered as it is written, and nothing in it is planned for a function of its own: what is
    /// lowered is its copies ([`crate::specialize`]).
    pub fn leaves_types_open(&self) -> bool {
        self.owner.helper().is_some_and(|held| held.variables() > 0)
    }
}

/// The module whose copy of a helper, and whose home of a value, a call reaches: the module a body
/// stands in.
///
/// Answered by a [`Body`] and by nothing else. What a call from a body reaches is held where the
/// body is read and trusted where it is lowered, so the two have to ask one statement of where it
/// stands; a module named by hand at the place a body is lowered is a second statement, which
/// agrees with the first only as long as whoever wrote it chose the same module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carrier<'p>(&'p str);

impl<'p> Carrier<'p> {
    pub fn module(self) -> &'p str {
        self.0
    }
}

/// What a body is the body of.
#[derive(Clone, Copy)]
pub enum Owner<'p> {
    Helper(&'p Held),
    Value(&'p Value),
    Entry(&'p ValueEntry),
    /// A behavior's own body, by the name it defines.
    Definition(&'p str),
    Example(&'p Example),
    /// A value the row `example` states its stand-in at `at` is asked with or answers.
    StoodIn {
        example: &'p Example,
        at: usize,
    },
    /// The clause at `at` among what `declaration` holds its values to, in the order they run.
    Invariant {
        declaration: &'p Declaration,
        at: usize,
    },
    /// The rule at `at` among what `target`'s answer is held to, in the order they run.
    Ensures {
        target: &'p Target,
        at: usize,
    },
}

impl<'p> Owner<'p> {
    /// The helper this is the body of, where it is one: the one kind of body the checker leaves type
    /// variables open in, and the one lowered as copies. Every kind is named, so a kind added here
    /// says whether it is one.
    pub fn helper(self) -> Option<&'p Held> {
        match self {
            Owner::Helper(held) => Some(held),
            Owner::Value(_)
            | Owner::Entry(_)
            | Owner::Definition(_)
            | Owner::Example(_)
            | Owner::StoodIn { .. }
            | Owner::Invariant { .. }
            | Owner::Ensures { .. } => None,
        }
    }
}

/// Who declared a type, which is what decides who defines the byte its values are tagged with.
///
/// The checker's answer and not one worked out here. This side could ask whether the declaration's
/// module is one the document carries and get the same answer for two of these three, which is the
/// kind of agreement that holds until it does not: a declaration the language itself gives is in
/// no module of any compilation, and the rule would file it under the one case it is not.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum DeclaredBy {
    /// A module this compile checked. The declaration is at home in this object, which is what
    /// defines its token for whoever links it.
    AModule,
    /// A module this compile read off the path, already built. That build defined the token and
    /// this object names it, so the linker is what brings the two together.
    OnThePath,
    /// The language, in its own namespace and in no module of any compilation. Nothing here ships
    /// an implementation of one, so there is nothing to be at home in this object either.
    TheLanguage,
}

/// What a declared type is made of, as its declaration says.
///
/// The shape and not the layout: how many fields there are and what they are called. Where a field
/// sits and what a value costs to make are decided here on this side, from this.
///
/// The module and the name apart, because this is where a declared type's identity is owned: the
/// symbol its values are tagged with is built from the two, and a reference elsewhere in the
/// document carries the key that reaches this rather than a second copy of what the key stands
/// for. So nothing on this side ever splits a key back up — an identity comes out of a declaration
/// or it does not come out at all.
#[derive(Debug, Deserialize)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Declaration {
    Product {
        module: String,
        name: String,
        by: DeclaredBy,
        fields: Vec<Field>,
        /// What every value of this owes, in the order a construction runs them and stops at the
        /// first that does not hold, where this build is the one that runs them: carried for a
        /// declaration a module of this compile declares and for no other. One on the path is
        /// built by its own build's object, and a construction here calls that.
        #[serde(default)]
        invariants: Option<Vec<Invariant>>,
        /// What each of those clauses is answered under, in the same order, where another build
        /// runs them: carried for a declaration on the path and for no other. What a clause says
        /// is that build's, and which clause did not hold is what that build's object answers, so
        /// this is what an arm naming the clause is matched to here.
        #[serde(default)]
        headers: Option<Vec<Header>>,
    },
    /// One value under another name: one field, and not a list of them that happens to hold one.
    Newtype {
        module: String,
        name: String,
        by: DeclaredBy,
        field: Field,
        #[serde(default)]
        invariants: Option<Vec<Invariant>>,
        #[serde(default)]
        headers: Option<Vec<Header>>,
    },
    /// One value, and naming it is that value: no field, and no clause, since there is nothing
    /// for one to observe.
    Unit {
        module: String,
        name: String,
        by: DeclaredBy,
    },
    /// A sum is never built. What it says is which types stand as its cases, and those are the
    /// leaves it descends to: the checker descends a case that is a sum before the declaration
    /// crosses, and a sum standing as a case of another is refused when the document is read.
    ///
    /// No value is ever one, so nothing is ever tagged with a sum and no object defines a token
    /// for one. Which is not to say a sum has no identity: it has the one every declaration has,
    /// its module and its name, and that is here. What it has no need of is a byte for a value to
    /// carry the address of.
    Sum {
        module: String,
        name: String,
        by: DeclaredBy,
        cases: Cases,
        form: AlternativesForm,
    },
}

impl Declaration {
    pub fn module(&self) -> &str {
        match self {
            Declaration::Product { module, .. }
            | Declaration::Newtype { module, .. }
            | Declaration::Unit { module, .. }
            | Declaration::Sum { module, .. } => module,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Declaration::Product { name, .. }
            | Declaration::Newtype { name, .. }
            | Declaration::Unit { name, .. }
            | Declaration::Sum { name, .. } => name,
        }
    }

    pub fn by(&self) -> DeclaredBy {
        match self {
            Declaration::Product { by, .. }
            | Declaration::Newtype { by, .. }
            | Declaration::Unit { by, .. }
            | Declaration::Sum { by, .. } => *by,
        }
    }

    /// What a reference to this declaration in the document says, which is the two halves joined
    /// the one way.
    pub fn key(&self) -> String {
        format!("{}.{}", self.module(), self.name())
    }

    /// Every field a value of this holds, with what each carries, in the order they are laid
    /// out; none for a unit, and none for a sum, which is never built.
    pub fn fields(&self) -> &[Field] {
        match self {
            Declaration::Product { fields, .. } => fields,
            Declaration::Newtype { field, .. } => std::slice::from_ref(field),
            Declaration::Unit { .. } | Declaration::Sum { .. } => &[],
        }
    }

    /// Where a field of this type sits among its fields, by the name it is declared under.
    pub fn position_of(&self, field: &str) -> Option<usize> {
        self.fields().iter().position(|it| it.name == field)
    }

    pub fn field_count(&self) -> usize {
        self.fields().len()
    }

    /// What every value of this owes, in the order a construction runs them, where this build
    /// runs them; `None` for a declaration another build builds, whose clauses are that build's.
    /// None to run for a unit, which has nothing for a clause to read, and none for a sum, which is
    /// never built.
    pub fn clauses(&self) -> Option<&[Invariant]> {
        match self {
            Declaration::Product { invariants, .. } | Declaration::Newtype { invariants, .. } => {
                invariants.as_deref()
            }
            Declaration::Unit { .. } | Declaration::Sum { .. } => Some(&[]),
        }
    }

    /// What each clause a value of this owes is answered under, in the order a construction runs
    /// them, whichever build runs them: read off the clauses where this build runs them and off
    /// the headers where another does. `None` for a clause its author gave no name. None for a
    /// unit or a sum.
    ///
    /// The place of a name here is the place the object that runs the clauses answers when that
    /// clause does not hold, which is what makes it the one list an arm is matched against.
    pub fn clause_names(&self) -> Vec<Option<&str>> {
        match self {
            Declaration::Product {
                invariants,
                headers,
                ..
            }
            | Declaration::Newtype {
                invariants,
                headers,
                ..
            } => match (invariants, headers) {
                (Some(invariants), _) => invariants.iter().map(|it| it.name.as_deref()).collect(),
                (None, Some(headers)) => headers.iter().map(|it| it.name.as_deref()).collect(),
                (None, None) => Vec::new(),
            },
            Declaration::Unit { .. } | Declaration::Sum { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub name: String,
    /// The data this module publishes, by the key a reference to each says: what another build can
    /// name, and so build a value of. The module's answer about its surface and not a fact about
    /// any one declaration, so it is carried here and not beside the declarations.
    pub publishes: Vec<String>,
    pub helpers: Vec<Held>,
    /// The values this module declares: the one place each of them runs.
    pub values: Vec<Value>,
    /// The entries this module publishes, one per published value — the nullary bridge another
    /// module calls in place of holding a copy of the value (ADR-0074).
    pub entries: Vec<ValueEntry>,
    /// What this object puts under a name. A behavior that answers some other way — supplied from
    /// outside, implemented by another build, or not written — is in the table above and nowhere
    /// here.
    pub definitions: Vec<Definition>,
    /// The `example` rows of this module's behaviors that the object runs.
    pub examples: Vec<Example>,
}

/// A value this module declares: the one place it runs.
///
/// Not a `Held`. A helper is a copy a module carries because a call to it was left standing, and
/// two modules holding one hold a copy each; a value has one executable home, the module that
/// declares it (spec ADR-0074), and this is that home — an object-private definition, reached only
/// from within the declaring module ([`Reaches::Value`]) or through the [`ValueEntry`] a module
/// publishes for it. Its identity crosses split — `module` and `name` apart — not because this
/// home needs a linker-visible name built from the two (it does not: the home is free to be named
/// however this side's own local symbols are), but because [`ValueEntry`]'s `value_symbol(module,
/// name)` does, and a joined spelling here would have to be split back up to answer it.
///
/// What it takes at the language level is nothing: a value takes no argument. `handovers` are not
/// that — they are the machine parameters its generated method actually has, one per other value
/// its root region names, already built by whoever calls it. A language-level "takes none" and a
/// method with parameters are not in tension: the same is true of any other zero-argument
/// definition whose generated method still takes what its body captures.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Value {
    pub module: String,
    pub name: String,
    /// Whether this module publishes it is not carried here: souther's own `CheckedModule`
    /// constructor already holds "published ⇔ has a `ValueEntry` among [`Module::entries`]" as an
    /// invariant, so a `publication` field beside this one would be the same fact stated twice —
    /// and the two would agree only until whichever consumer reads `entries` and whichever reads
    /// `publication` were updated on different days. A value's own publication is asked by looking
    /// it up in `entries`, never by a field here.
    pub handovers: Vec<Handover>,
    pub body: Node,
}

impl Value {
    /// What it answers: its body's type, which is what the checker checked the value as.
    pub fn answers(&self) -> &Ty {
        self.body.ty()
    }

    /// What a reference to this value in the document says, which is the two halves joined the one
    /// way.
    pub fn declared(&self) -> String {
        format!("{}.{}", self.module, self.name)
    }
}

/// The entry a module publishes for one of its values: the nullary bridge another module calls in
/// place of holding a copy of the value (ADR-0074).
///
/// Not the value's own body — `body` here is a reference to the value and nothing else, a call
/// reaching [`Reaches::Value`], written the same way any other reach to it is. Present for exactly
/// the values a module publishes; a value it keeps has no entry, because nothing outside the module
/// may call through one.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueEntry {
    pub value: ValueRef,
    pub body: Node,
}

/// What the method a value runs as is handed: another value its root region names, already built.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handover {
    pub parameter: String,
    #[serde(rename = "type")]
    pub ty: Ty,
    /// The value this handover carries, split the same way [`Value`]'s own identity is.
    pub carries: ValueRef,
}

/// The module and the name of a value, apart — what a `value_symbol` is built from.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ValueRef {
    pub module: String,
    pub name: String,
}

impl ValueRef {
    /// What a reference to this value in the document says, which is the two halves joined the
    /// one way.
    pub fn declared(&self) -> String {
        format!("{}.{}", self.module, self.name)
    }
}

/// What an object defines under a behavior's name: a body of {@link Core}, or a composition of
/// other behaviors.
///
/// Both are local definitions and neither is a body the other can be read as: a composition has no
/// Core to fall back to and a body has no stages. A definition added upstream stops this
/// compiling, for the reason every closed set here does — a third way of defining a name would
/// otherwise arrive as whichever of these two it happened to resemble.
#[derive(Debug, Deserialize)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Definition {
    /// Written as a `let`: the checker's Core for it.
    Body {
        declared: String,
        parameters: Vec<String>,
        /// What the module declaring it says about the name.
        publication: Publication,
        /// What its target says constructing it requires ([`Target::requirements`]), which
        /// [`Program::read`] puts here and the document does not write a second time.
        #[serde(skip)]
        requirements: Vec<Requirement>,
        body: Node,
    },
    /// Written as `>->`: the stages, and what each is offered (spec §type-routing). Carried
    /// whole and not translated into a plan for running it — how the routing between stages is
    /// realised is this side's to decide, and none of it is written down upstream.
    ///
    /// What it answers is its own target's answer and is not carried here a second time.
    Composed {
        declared: String,
        /// What the module declaring it says about the name.
        publication: Publication,
        /// What its target says constructing it requires ([`Target::requirements`]): what its stages
        /// require, which is not what it calls.
        #[serde(skip)]
        requirements: Vec<Requirement>,
        stages: Vec<Stage>,
    },
}

impl Definition {
    /// What a call reaching this definition writes, which is the two halves joined the one way.
    pub fn declared(&self) -> &str {
        match self {
            Definition::Body { declared, .. } | Definition::Composed { declared, .. } => declared,
        }
    }

    /// What the module declaring it says about the name.
    pub fn publication(&self) -> Publication {
        match self {
            Definition::Body { publication, .. } | Definition::Composed { publication, .. } => {
                *publication
            }
        }
    }

    /// What constructing it requires injected, in the order the checker answered it: the
    /// behaviors a host binding it has to be handed, each either one a host implements or one
    /// constructed from what it requires in turn.
    pub fn requirements(&self) -> &[Requirement] {
        match self {
            Definition::Body { requirements, .. } | Definition::Composed { requirements, .. } => {
                requirements
            }
        }
    }
}

/// A behavior a definition requires injected: its module and its name, apart, since a module's
/// name carries dots.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub module: String,
    pub name: String,
}

impl Requirement {
    /// What a reference to this behavior in the document says, which is the two halves joined the
    /// one way.
    pub fn declared(&self) -> String {
        format!("{}.{}", self.module, self.name)
    }
}

/// One stage of a composition: the behavior it applies, and when it is applied to the running
/// value (spec §type-routing).
///
/// What the stage answers is the answer of the behavior it names, which the table of targets
/// carries, and is not carried here a second time.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub behavior: String,
    pub routing: Routing,
}

/// When a stage is applied to the running value (spec §type-routing).
///
/// A closed set and a tagged union for the reason `Selects` is one: `accepted = []` and no routing
/// at all are two different facts, and a `bool` or an `Option<Vec<_>>` would make one of them
/// unrepresentable while inventing a third state nothing upstream ever means.
#[derive(Debug, Deserialize)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Routing {
    /// The first stage, which takes the composition's own arguments, and any stage whose running
    /// value carries no cases to tell apart.
    Always,
    /// Only where the running value is one of `accepted`. Anything else has left the main line,
    /// and the composition answers with it rather than offering it to what follows.
    OnCases { accepted: Cases },
}

/// Whether the module that declares a behavior publishes it under that name, or keeps it.
///
/// The language's answer about the module's surface, which is not the same question as what this
/// object's symbol table carries. That one is the object's own, worked out from this together with
/// what the object is for.
///
/// Carried by a body and not by a target, because it is the declaring module's answer and a
/// target is answered for a behavior of a module this compile never checked.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Publication {
    Published,
    Kept,
}

/// One `example` row of a behavior of this module, as the object runs it.
///
/// The values the row states are written into the entry rather than handed to it, so the entry
/// takes nothing and what it does is the one call the row is. Numbered by where the row stands
/// among the behavior's rows, so a row the writer carried nothing for leaves its number unused
/// rather than moving every row after it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Example {
    pub behavior: String,
    pub at: usize,
    pub body: Node,
    /// What the row states each dependency of the behavior answers, in the order the behavior
    /// requires them, and none where it states nothing of any (upstream `CheckedRow.WithStandIns`).
    #[serde(rename = "standsIn")]
    pub stands_in: Vec<StandIn>,
}

/// What a row states one dependency answers (upstream `StandsIn`): the first of `entries` stating
/// the arguments a call arrived with answers, compared as the language compares two values, and
/// `otherwise` answers the rest, where the row states anything for the rest.
///
/// The entries in order and not a table keyed by them: which entry states a call is the
/// comparison's to say, and the first to say so answers.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandIn {
    pub module: String,
    pub name: String,
    pub entries: Vec<StoodEntry>,
    pub otherwise: Option<Node>,
}

impl StandIn {
    /// What a reference to the dependency says, which is the two halves joined the one way.
    pub fn declared(&self) -> String {
        format!("{}.{}", self.module, self.name)
    }

    /// Every value it states, each as the expression that makes it: each entry's arguments then its
    /// answer, entry after entry, then what it answers for the rest.
    pub fn values(&self) -> impl Iterator<Item = &Node> {
        self.entries
            .iter()
            .flat_map(|entry| entry.arguments.iter().chain([&entry.answer]))
            .chain(&self.otherwise)
    }
}

/// One entry of what a row states a dependency answers: the arguments, in the order the dependency
/// takes them, and the answer.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoodEntry {
    pub arguments: Vec<Node>,
    pub answer: Node,
}

/// A behavior as a caller reaches it.
///
/// The module and the name apart, because that is what a behavior's identity is made of and it is
/// what the symbol is built from. Written as one string and split back, the two halves would be
/// recovered from a spelling rather than carried, and a module's name carries dots.
#[derive(Debug, Deserialize)]
#[serde(try_from = "WrittenTarget")]
pub struct Target {
    pub module: String,
    pub name: String,
    pub is: Answers,
    /// What it takes, in order.
    pub inputs: Vec<BoundaryInput>,
    /// The names its declaration gives what it takes, one for each of `inputs`, and none for a
    /// composition, which declares no parameters. Held apart from `inputs` because every reader
    /// but one asks what arrives and not what it is called; set only by [`Target::try_from`], from
    /// a document in which each name was written beside the input it names.
    names: Option<Vec<String>>,
    pub output: BoundaryOutput,
    /// What is done about what the behavior declares of its answer, as the checker answered it.
    pub ensures: Ensures,
    /// What constructing it requires injected, in the order its constructor takes them: what a
    /// caller hands it the capabilities of, and a composition hands a stage those of, whichever
    /// build implements it. Nothing for a behavior a host implements, which Souther does not
    /// construct; that is not a behavior called with nothing handed, which how it answers says.
    pub requirements: Vec<Requirement>,
}

/// A [`Target`] as the document writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WrittenTarget {
    module: String,
    name: String,
    is: Answers,
    parameters: Parameters,
    output: BoundaryOutput,
    ensures: Ensures,
    requirements: Vec<Requirement>,
}

/// What a behavior takes: named where its declaration names them, and in order only where it is a
/// composition, which writes no parameter list.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
enum Parameters {
    Named(Vec<NamedInput>),
    Positional(Vec<BoundaryInput>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NamedInput {
    name: String,
    input: BoundaryInput,
}

impl TryFrom<WrittenTarget> for Target {
    type Error = String;

    /// Holds what a behavior takes to how it answers, as the checker pairs them
    /// (`CheckedProgramAssembler`): a behavior with a body, one a host implements and one not
    /// written are each declared, and a declaration names every parameter; a composition declares
    /// none. One another build implements was either, and the kind of answer it crosses as does not
    /// say which. Every pair is written out, so a kind of answer added to [`Answers`] is not read
    /// until it is decided here.
    ///
    /// Where the behavior's answer is held to an `ensures`, the names the clause relates are the
    /// declaration's, the same list crossed twice: they are held to be one.
    fn try_from(written: WrittenTarget) -> Result<Self, Self::Error> {
        let named = |named: Vec<NamedInput>| {
            let (names, inputs): (Vec<String>, Vec<BoundaryInput>) =
                named.into_iter().map(|it| (it.name, it.input)).unzip();
            (inputs, Some(names))
        };
        let refused = |why: &str| {
            Err(format!(
                "`{}.{}` answers as {:?} and {why}: the two halves disagree",
                written.module, written.name, written.is
            ))
        };
        let (inputs, names) = match (written.is, written.parameters) {
            (Answers::Body | Answers::Injected | Answers::Unwritten, Parameters::Named(it)) => {
                named(it)
            }
            (Answers::Body | Answers::Injected | Answers::Unwritten, Parameters::Positional(_)) => {
                return refused("is written with no parameter names, which its declaration gives");
            }
            (Answers::Composed, Parameters::Positional(inputs)) => (inputs, None),
            (Answers::Composed, Parameters::Named(_)) => {
                return refused(
                    "is written with parameter names, which a composition declares none of",
                );
            }
            (Answers::Elsewhere, Parameters::Named(it)) => named(it),
            (Answers::Elsewhere, Parameters::Positional(inputs)) => (inputs, None),
        };
        if written.is == Answers::Injected && !written.requirements.is_empty() {
            return refused("requires something to construct, and a host's is not constructed");
        }
        if let Some(contract) = written.ensures.contract() {
            match &names {
                Some(names) if *names == contract.parameters => {}
                Some(names) => {
                    return refused(&format!(
                        "takes {names:?}, and its ensures relates {:?}",
                        contract.parameters
                    ));
                }
                None => return refused("declares no parameters, and an ensures relates some"),
            }
        }
        Ok(Target {
            module: written.module,
            name: written.name,
            is: written.is,
            inputs,
            names,
            output: written.output,
            ensures: written.ensures,
            requirements: written.requirements,
        })
    }
}

impl Target {
    /// The names its declaration gives what it takes, one for each of `inputs`, or none where it
    /// is a composition.
    pub fn names(&self) -> Option<&[String]> {
        self.names.as_deref()
    }

    /// What a call reaching this behavior writes, which is the two halves joined the one way.
    pub fn declared(&self) -> String {
        format!("{}.{}", self.module, self.name)
    }

    /// What it takes, read off what each parameter can arrive as. Every boundary shape stands for
    /// a type, so this answers for all of them; whether a value of one can be laid out here is
    /// asked of the type afterwards, and is a different question.
    pub fn takes(&self) -> Vec<Ty> {
        self.inputs.iter().map(BoundaryInput::ty).collect()
    }

    /// What it answers, read off what the answer can leave as.
    pub fn answers(&self) -> Ty {
        self.output.ty()
    }
}

/// Where a behavior's answer is held to what the behavior declares of it, as the checker placed
/// the check (`EnsuresEnforcement`).
///
/// Four answers and not a pair of flags. Whether the callee checks and whether a crossing does are
/// one decision with three meaningful outcomes, and two flags could also say that nothing checks a
/// clause or that two places do. `None` and `Undecided` are apart for the same reason: one is a
/// behavior read and found to declare nothing, the other one whose clause nobody here decided where
/// to run.
#[derive(Debug, Deserialize)]
#[serde(tag = "at", rename_all = "lowercase", deny_unknown_fields)]
pub enum Ensures {
    /// Held where the behavior answers: its body is here, and every way in goes through it.
    Callee { contract: Contract },
    /// Held at every call into this object's code, because the answer arrives from outside.
    Crossing { contract: Contract },
    /// The behavior declares nothing of its answer.
    None,
    /// The behavior is another build's, and this compile did not decide what is done about it.
    Undecided,
}

impl Ensures {
    /// The rules, where something here runs them.
    pub fn contract(&self) -> Option<&Contract> {
        match self {
            Ensures::Callee { contract } | Ensures::Crossing { contract } => Some(contract),
            Ensures::None | Ensures::Undecided => None,
        }
    }
}

/// What a behavior declares of the relation between what it is given and what it answers, as the
/// checker elaborated it to run (`Contract`).
///
/// What it takes and answers is its target's, and is not carried a second time. The parameters are
/// named, the way a body's are, and bound under the number of where each stands.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub parameters: Vec<String>,
    /// Every rule the answer is held to, in the order a failure is decided in. All of those whose
    /// guard holds are held, and not the first: a declaration states a conjunction.
    pub rules: Vec<Rule>,
}

/// One rule of a contract: which answers it applies to, the binding the answer is read through,
/// and what has to hold.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub guard: Guard,
    /// The number `condition` reads the answer under.
    pub value: usize,
    pub condition: Node,
    /// Whether the rule as written refers to the answer. The checker's decision about the
    /// declaration, carried so that nothing reads it back off `condition`; nothing here runs it.
    #[serde(rename = "readsanswer")]
    pub reads_answer: bool,
    /// The name a failure of this is reported under, where the author gave one.
    pub clause: Option<String>,
}

/// Which answers a rule applies to.
#[derive(Debug, Deserialize)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Guard {
    /// Every answer, read as what the behavior answers.
    Always,
    /// An answer that is this case, read as what `binds` says: the test does not say it, since a
    /// case that is a sum is tested as the leaves it descends to.
    Case { selects: Selects, binds: Ty },
}

impl Guard {
    /// What the answer is read as where this rule applies, `answers` being what the behavior
    /// answers.
    pub fn reads_as<'t>(&'t self, answers: &'t Ty) -> &'t Ty {
        match self {
            Guard::Always => answers,
            Guard::Case { binds, .. } => binds,
        }
    }
}

/// One of the closed set of scalars a boundary writes as themselves.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum LeafScalar {
    #[serde(rename = "STRING")]
    String,
    #[serde(rename = "INT")]
    Int,
    #[serde(rename = "BOOL")]
    Bool,
    #[serde(rename = "DECIMAL")]
    Decimal,
    #[serde(rename = "DATE")]
    Date,
    #[serde(rename = "TIME")]
    Time,
    #[serde(rename = "DATETIME")]
    DateTime,
    #[serde(rename = "INSTANT")]
    Instant,
}

impl LeafScalar {
    /// The scalar `prim` is written as, where it is one a boundary writes.
    pub fn of(prim: Prim) -> Option<LeafScalar> {
        match prim {
            Prim::String => Some(LeafScalar::String),
            Prim::Int => Some(LeafScalar::Int),
            Prim::Bool => Some(LeafScalar::Bool),
            Prim::Decimal => Some(LeafScalar::Decimal),
            Prim::Date => Some(LeafScalar::Date),
            Prim::Time => Some(LeafScalar::Time),
            Prim::DateTime => Some(LeafScalar::DateTime),
            Prim::Instant => Some(LeafScalar::Instant),
            Prim::Rational | Prim::Raw => None,
        }
    }

    pub fn prim(self) -> Prim {
        match self {
            LeafScalar::String => Prim::String,
            LeafScalar::Int => Prim::Int,
            LeafScalar::Bool => Prim::Bool,
            LeafScalar::Decimal => Prim::Decimal,
            LeafScalar::Date => Prim::Date,
            LeafScalar::Time => Prim::Time,
            LeafScalar::DateTime => Prim::DateTime,
            LeafScalar::Instant => Prim::Instant,
        }
    }
}

/// What a parameter can arrive as, as the checker settled it.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum BoundaryInput {
    Scalar {
        scalar: LeafScalar,
    },
    Nominal {
        named: Case,
    },
    ListOf {
        element: Box<BoundaryInput>,
    },
    SetOf {
        element: Box<BoundaryInput>,
    },
    MapOf {
        key: MapKey,
        value: Box<BoundaryInput>,
    },
}

impl BoundaryInput {
    pub fn ty(&self) -> Ty {
        match self {
            BoundaryInput::Scalar { scalar } => Ty::Prim {
                prim: scalar.prim(),
            },
            BoundaryInput::Nominal { named } => Ty::Ref {
                named: named.clone(),
            },
            BoundaryInput::ListOf { element } => Ty::List {
                list: Box::new(element.ty()),
            },
            BoundaryInput::SetOf { element } => Ty::Set {
                set: Box::new(element.ty()),
            },
            BoundaryInput::MapOf { key, value } => Ty::Map {
                map: MapTy {
                    key: Box::new(key.ty()),
                    value: Box::new(value.ty()),
                },
            },
        }
    }
}

/// What an answer can leave as, as the checker settled it.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum BoundaryOutput {
    Scalar {
        scalar: LeafScalar,
    },
    Nominal {
        named: Case,
    },
    ListOf {
        element: Box<BoundaryOutput>,
    },
    SetOf {
        element: Box<BoundaryOutput>,
    },
    MapOf {
        key: MapKey,
        value: Box<BoundaryOutput>,
    },
    /// A union nobody named: the type exactly as its members were written, beside the cases the
    /// boundary descended to, which are not the same answer.
    Cases {
        #[serde(rename = "type")]
        ty: Ty,
        cases: Cases,
        form: AlternativesForm,
    },
}

impl BoundaryOutput {
    pub fn ty(&self) -> Ty {
        match self {
            BoundaryOutput::Scalar { scalar } => Ty::Prim {
                prim: scalar.prim(),
            },
            BoundaryOutput::Nominal { named } => Ty::Ref {
                named: named.clone(),
            },
            BoundaryOutput::ListOf { element } => Ty::List {
                list: Box::new(element.ty()),
            },
            BoundaryOutput::SetOf { element } => Ty::Set {
                set: Box::new(element.ty()),
            },
            BoundaryOutput::MapOf { key, value } => Ty::Map {
                map: MapTy {
                    key: Box::new(key.ty()),
                    value: Box::new(value.ty()),
                },
            },
            BoundaryOutput::Cases { ty, .. } => ty.clone(),
        }
    }
}

/// What a boundary map's key is written as.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum MapKey {
    Text,
    Date,
    Time,
    DateTime,
    Instant,
    NamedKey { named: Case },
}

impl MapKey {
    pub fn ty(&self) -> Ty {
        match self {
            MapKey::Text => Ty::Prim { prim: Prim::String },
            MapKey::Date => Ty::Prim { prim: Prim::Date },
            MapKey::Time => Ty::Prim { prim: Prim::Time },
            MapKey::DateTime => Ty::Prim {
                prim: Prim::DateTime,
            },
            MapKey::Instant => Ty::Prim {
                prim: Prim::Instant,
            },
            MapKey::NamedKey { named } => Ty::Ref {
                named: named.clone(),
            },
        }
    }
}

/// How a set of alternatives travels. Both keys of a discriminated form cross, so nothing on
/// this side spells either of them.
///
/// Read through [`FormOnTheWire`], so a form with one key for both the tag and a wrapped case's
/// contents is not a value here: the two stand in one object, and the checker refuses to build
/// one (`CheckedAlternativesForm.Discriminated`).
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(try_from = "FormOnTheWire")]
pub enum AlternativesForm {
    Enumeration,
    Discriminated { tag: String, contents: String },
}

#[derive(Deserialize)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
enum FormOnTheWire {
    Enumeration,
    Discriminated { tag: String, contents: String },
}

impl TryFrom<FormOnTheWire> for AlternativesForm {
    type Error = String;

    fn try_from(form: FormOnTheWire) -> Result<Self, Self::Error> {
        match form {
            FormOnTheWire::Enumeration => Ok(AlternativesForm::Enumeration),
            FormOnTheWire::Discriminated { tag, contents } if tag == contents => Err(format!(
                "a discriminated form with {tag} for both its tag and a wrapped case's contents, \
                 which stand in one object"
            )),
            FormOnTheWire::Discriminated { tag, contents } => {
                Ok(AlternativesForm::Discriminated { tag, contents })
            }
        }
    }
}

/// What a field carries across the boundary, as the check derived it for where it stands.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum CodecShape {
    Scalar {
        scalar: LeafScalar,
    },
    Named {
        named: Case,
    },
    ListOf {
        element: Box<CodecShape>,
    },
    SetOf {
        element: Box<CodecShape>,
    },
    MapOf {
        key: MapKey,
        value: Box<CodecShape>,
    },
    /// What an optional holds, which is never an optional again: absence has one form wherever it
    /// stands, so the checker has no shape for an optional of one, and neither does this.
    OptionOf {
        present: Box<Bare>,
    },
}

impl CodecShape {
    /// The type a value standing at this shape is, which is what says how it is held in a slot.
    pub fn ty(&self) -> Ty {
        match self {
            CodecShape::Scalar { scalar } => Ty::Prim {
                prim: scalar.prim(),
            },
            CodecShape::Named { named } => Ty::Ref {
                named: named.clone(),
            },
            CodecShape::ListOf { element } => Ty::List {
                list: Box::new(element.ty()),
            },
            CodecShape::SetOf { element } => Ty::Set {
                set: Box::new(element.ty()),
            },
            CodecShape::MapOf { key, value } => Ty::Map {
                map: MapTy {
                    key: Box::new(key.ty()),
                    value: Box::new(value.ty()),
                },
            },
            CodecShape::OptionOf { present } => Ty::Option {
                option: Box::new(present.shape().ty()),
            },
        }
    }
}

/// A shape that is not an optional: what an optional holds (`CheckedCodecShape.Bare`).
///
/// Read as a [`CodecShape`] and refused if it is an optional, so an optional of an optional never
/// becomes a value here — which is what keeps this side from giving one a form the language never
/// decided.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(try_from = "CodecShape")]
pub struct Bare(CodecShape);

impl Bare {
    pub fn shape(&self) -> &CodecShape {
        &self.0
    }
}

impl TryFrom<CodecShape> for Bare {
    type Error = String;

    fn try_from(shape: CodecShape) -> Result<Self, Self::Error> {
        match shape {
            CodecShape::OptionOf { .. } => Err(
                "an optional holding an optional, which the check never settles: absence has one \
                 form wherever it stands"
                    .to_string(),
            ),
            bare => Ok(Bare(bare)),
        }
    }
}

/// A field of a declaration, what it carries across the boundary, and the number a clause reads it
/// under, held together.
///
/// The number is the declaration's own, counted where its fields are bound, and it is not where the
/// field sits. A clause a spread takes in reads the field under the binding the declaration that
/// wrote it gave, so a reader putting a field's value under its position would be running the
/// clause over something it was not written about.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub binding: usize,
    pub codec: CodecShape,
}

/// One clause a declaration holds its values to: the name a failure is reported under, where the
/// author gave one, and what has to hold, as the checker elaborated it over the fields' bindings.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invariant {
    pub name: Option<String>,
    pub condition: Node,
}

/// The name one clause of a declaration another build runs is answered under, where its author
/// gave one, and nothing of what the clause says.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub name: Option<String>,
}

/// A set of alternatives: one case or more, never none.
///
/// Every place the document names cases a value may be one of — a sum's cases, a union's members,
/// the cases an answer is written by, the atoms an arm tests, the cases a stage accepts — is one of
/// these, and refused as read where it names none. The checker never writes one empty: a sum and a
/// union have cases, and an arm or a stage that tests for none is refused upstream. Everything
/// reading one tells a value apart by its token and takes the last case for what a value tagged by
/// none of the others is, which holds only of a set with a case in it; so the fact is held here,
/// once, rather than by each reader remembering to ask.
#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(try_from = "Vec<Case>")]
pub struct Cases(Vec<Case>);

impl Cases {
    /// `cases`, where there is one.
    pub fn one_or_more(cases: Vec<Case>) -> Option<Cases> {
        (!cases.is_empty()).then_some(Cases(cases))
    }
}

impl TryFrom<Vec<Case>> for Cases {
    type Error = String;

    fn try_from(cases: Vec<Case>) -> Result<Cases, String> {
        Cases::one_or_more(cases).ok_or_else(|| {
            "a set of alternatives with no case in it, which the checker never states".to_string()
        })
    }
}

impl std::ops::Deref for Cases {
    type Target = [Case];

    fn deref(&self) -> &[Case] {
        &self.0
    }
}

impl<'c> IntoIterator for &'c Cases {
    type Item = &'c Case;
    type IntoIter = std::slice::Iter<'c, Case>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Which type a name is: one a module declares, a primitive standing as one, or one the language
/// gives. Written wherever the checker names a type — a union's member, a sum's case, a type
/// reference, what a field, a key or a boundary is named by, a unit — so a primitive or a case
/// the language gives named where a declaration usually stands is read as what it is, and whether
/// it has a representation there is the lowering's to answer.
///
/// The identity only — how a case is written is read off what it reaches.
#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Case {
    Declared { declared: String },
    Primitive { prim: Prim },
    Language { case: LanguageCase },
}

impl Case {
    /// The key of the declaration this is, where a module declares it.
    pub fn declared(&self) -> Option<&str> {
        match self {
            Case::Declared { declared } => Some(declared),
            Case::Primitive { .. } | Case::Language { .. } => None,
        }
    }

    pub fn spelt(&self) -> String {
        match self {
            Case::Declared { declared } => declared.clone(),
            Case::Primitive { prim } => prim.spelt().to_string(),
            Case::Language { case } => case.spelt().to_string(),
        }
    }
}

/// The cases the language itself gives, a closed set.
#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone, Copy)]
pub enum LanguageCase {
    #[serde(rename = "SOME")]
    Some,
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "DIVISION_BY_ZERO")]
    DivisionByZero,
    #[serde(rename = "NOT_A_NUMBER")]
    NotANumber,
    #[serde(rename = "NOT_A_DATE")]
    NotADate,
    #[serde(rename = "NOT_A_TIME")]
    NotATime,
    #[serde(rename = "NOT_WHOLE")]
    NotWhole,
    #[serde(rename = "NOT_A_FINITE_DECIMAL")]
    NotAFiniteDecimal,
}

impl LanguageCase {
    pub fn spelt(self) -> &'static str {
        match self {
            LanguageCase::Some => "Some",
            LanguageCase::None => "None",
            LanguageCase::DivisionByZero => "DivisionByZero",
            LanguageCase::NotANumber => "NotANumber",
            LanguageCase::NotADate => "NotADate",
            LanguageCase::NotATime => "NotATime",
            LanguageCase::NotWhole => "NotWhole",
            LanguageCase::NotAFiniteDecimal => "NotAFiniteDecimal",
        }
    }
}

/// How a behavior comes to answer.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Answers {
    /// Code this object holds, which is emitted.
    Body,
    /// Supplied by whoever runs the program, as a capability handed to what requires it. The object
    /// of the build that declares it makes a capability of what a host implements it as; no object
    /// defines it under a symbol, since nothing reaches it but through a capability.
    Injected,
    /// Implemented by another build. The same call to whoever reaches in, and a different thing to
    /// whoever links.
    Elsewhere,
    /// By running other behaviors in an order the composition states.
    Composed,
    /// Not written, which the language admits and nothing can run.
    Unwritten,
}

/// A definition the module holds as one of its own.
///
/// Named by the reference a call in the holding module reaches it by, which is what a call to it
/// writes ([`Reaches::Helper`]); not by where it was declared, which for an operation of the
/// standard library is a module the reference does not name. Two modules reaching one definition
/// hold a copy each.
///
/// What it takes is its parameters, each a name and a type together, and what it answers is its
/// body's type. Neither is carried a second time, so the two cannot disagree.
///
/// A type in it may be a variable its body leaves open ([`Ty::Var`]), numbered within this
/// definition alone. Such a definition is not a function yet: what each variable comes to is what a
/// call hands it, and what is lowered is one copy of it for each set of types a call needs
/// ([`crate::specialize`]).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Held {
    /// The reference a call in the holding module reaches it by, which is also what it is a copy
    /// of: the declaration is the half of the reference the route reaches.
    pub reached: Reference,
    pub parameters: Vec<HeldParameter>,
    pub body: Node,
}

impl Held {
    /// What it takes, in the order its parameters are bound.
    pub fn takes(&self) -> Vec<Ty> {
        self.parameters.iter().map(|it| it.ty.clone()).collect()
    }

    /// What it answers: its body's type.
    pub fn answers(&self) -> &Ty {
        self.body.ty()
    }

    /// How many type variables it leaves open: one more than the largest number any of its types
    /// writes, and none where no type of it writes one. Every number below that is one of them,
    /// which [`Coherent`](crate::coherent::Coherent) holds ([`Held::numbers`]).
    pub fn variables(&self) -> usize {
        self.numbers().last().map_or(0, |largest| largest + 1)
    }

    /// Every number a type variable is written under in it, in its parameters or its body.
    pub fn numbers(&self) -> std::collections::BTreeSet<usize> {
        let mut numbers = std::collections::BTreeSet::new();
        for parameter in &self.parameters {
            parameter.ty.numbers(&mut numbers);
        }
        // Every variable the helper writes, whether or not what writes it runs: which variables a
        // helper numbers is what its copies are made over.
        self.body.each_written(&mut |node| {
            for ty in node.types() {
                ty.numbers(&mut numbers);
            }
        });
        numbers
    }
}

/// A reference to a helper, as the checker settled it: the route the holding module reaches it by,
/// and the declaration the route reaches, in one value (`ReachName.Declaration`).
///
/// Not its spelling. A helper and a call of it carry this same value, so a call finds its helper by
/// the value and not by a spelling both sides would have to render alike; and what a module is held
/// to about its helpers is asked of the declaration inside it ([`Reference::declaration`]). The
/// spelling is worked out from it ([`Reference::rendered`]) the one way the checker renders one, for
/// a symbol and for a message, and read for nothing else.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Reference {
    /// A declaration of the module doing the reading, reached as it stands.
    Own { module: String, name: String },
    /// A declaration of another module, reached under that module's name.
    OfModule { module: String, name: String },
    /// An operation the standard library writes, reached under the alias it publishes it as.
    Library { alias: String, name: String },
}

/// What a [`Reference`] reaches: a declaration a module declares, or an operation the standard
/// library writes. Two references reaching one of these reach one declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reaching<'r> {
    Module { module: &'r str, name: &'r str },
    Library { alias: &'r str, name: &'r str },
}

impl Reference {
    /// The declaration this reaches.
    pub fn declaration(&self) -> Reaching<'_> {
        match self {
            Reference::Own { module, name } | Reference::OfModule { module, name } => {
                Reaching::Module { module, name }
            }
            Reference::Library { alias, name } => Reaching::Library { alias, name },
        }
    }

    /// The module whose declaration this reaches by a route of a module's, where it does.
    pub fn declaring_module(&self) -> Option<&str> {
        match self {
            Reference::Own { module, .. } | Reference::OfModule { module, .. } => Some(module),
            Reference::Library { .. } => None,
        }
    }

    /// How the checker spells it (`ReachName::rendered`): its own declaration bare, another
    /// module's under that module's name, and a library operation under its alias.
    pub fn rendered(&self) -> String {
        match self {
            Reference::Own { module: _, name } => name.clone(),
            Reference::OfModule { module, name } => format!("{module}.{name}"),
            Reference::Library { alias, name } => format!("{alias}.{name}"),
        }
    }
}

/// One parameter of a [`Held`], bound under the number its position says.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeldParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Ty,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Prim {
    #[serde(rename = "INT")]
    Int,
    #[serde(rename = "STRING")]
    String,
    #[serde(rename = "BOOL")]
    Bool,
    #[serde(rename = "DECIMAL")]
    Decimal,
    #[serde(rename = "RATIONAL")]
    Rational,
    #[serde(rename = "DATE")]
    Date,
    #[serde(rename = "TIME")]
    Time,
    #[serde(rename = "DATETIME")]
    DateTime,
    #[serde(rename = "INSTANT")]
    Instant,
    #[serde(rename = "RAW")]
    Raw,
}

impl Prim {
    /// What a reader of a refusal is told this was.
    pub fn spelt(self) -> &'static str {
        match self {
            Prim::Int => "Int",
            Prim::String => "String",
            Prim::Bool => "Bool",
            Prim::Decimal => "Decimal",
            Prim::Rational => "Rational",
            Prim::Date => "Date",
            Prim::Time => "Time",
            Prim::DateTime => "DateTime",
            Prim::Instant => "Instant",
            Prim::Raw => "Raw",
        }
    }
}

/// The type the checker decided for something, as much of one as crosses.
///
/// Told apart by which key is written rather than by a word beside it, since each of these is a
/// different shape and no two of them are ever both readable.
#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(untagged, deny_unknown_fields)]
pub enum Ty {
    Prim {
        prim: Prim,
    },
    /// A type by its name: a declaration of the document, by the key that reaches one, or a
    /// primitive or a case the language gives, named as a type. The key is what a reference says
    /// and not what a declaration is made of: the module and the name apart are carried by the
    /// declaration, and this finds it.
    Ref {
        #[serde(rename = "ref")]
        named: Case,
    },
    /// Several cases, any one of which a value here may be. A declared one says which it is, so a
    /// union of those is written nowhere at run time: what holds it is what holds one of them. A
    /// primitive or a case the language gives says nothing of the kind, and a union with one
    /// among its members is read and not laid out.
    Union {
        union: Cases,
    },
    Option {
        option: Box<Ty>,
    },
    Tuple {
        tuple: Vec<Ty>,
    },
    /// A function value: what it takes and what it answers, nothing about what a value of it is
    /// made of. That is a representation question and this side's own — see `machine_type` and
    /// `means_the_same_elsewhere` in the crate root — not a fact the checker states, so no field
    /// here ever names a capture.
    Fn {
        #[serde(rename = "fn")]
        fn_: FnSignature,
    },
    List {
        list: Box<Ty>,
    },
    Set {
        set: Box<Ty>,
    },
    Map {
        map: MapTy,
    },
    /// A type a helper's body leaves open, by the number it has within that helper ([`Held`]). It
    /// stands nowhere else, and means nothing outside the helper that numbers it: two helpers' `0`
    /// are two variables.
    Var {
        var: usize,
    },
    /// The type of what has no value: the element of an empty list literal, and so the accumulator
    /// a walk seeded with `[]` starts from. A type like any other on the wire; that no value of it
    /// is ever made is this side's to act on, and a list of it is a list like any other.
    Nothing {
        nothing: Bottom,
    },
    /// The type of what does not answer: a computation that ends the run. Not [`Ty::Nothing`],
    /// which is a type inference had not filled in; this one is settled. No value of it is ever
    /// made, so nothing is laid out for one — and that is not a width chosen for it.
    Never {
        never: Bottom,
    },
}

/// What [`Ty::Nothing`] is written with, which is nothing: an object with no field, read strictly
/// so that one naming a field is not read as the bottom.
#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(deny_unknown_fields)]
pub struct Bottom {}

#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(deny_unknown_fields)]
pub struct MapTy {
    pub key: Box<Ty>,
    pub value: Box<Ty>,
}

/// What a function type takes and what it answers, nested under `"fn"` rather than written as
/// `takes`/`answers` siblings of it — the same reason [`Reaches`]'s own shape is nested: a reader
/// telling a function type apart from every other [`Ty`] shape by which key is present must not
/// also have to notice a document naming `fn` beside `option` or `tuple` on the same object.
#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(deny_unknown_fields)]
pub struct FnSignature {
    pub takes: Vec<Ty>,
    pub answers: Box<Ty>,
}

impl Ty {
    /// A declaration of the document, as a type, by the key that reaches it.
    pub fn declared(key: impl Into<String>) -> Ty {
        Ty::Ref {
            named: Case::Declared {
                declared: key.into(),
            },
        }
    }

    pub fn spelt(&self) -> String {
        match self {
            Ty::Prim { prim } => prim.spelt().to_string(),
            Ty::Ref { named } => named.spelt(),
            Ty::Union { union } => union
                .iter()
                .map(Case::spelt)
                .collect::<Vec<_>>()
                .join(" | "),
            Ty::Option { option } => format!("an Option of {}", option.spelt()),
            Ty::Tuple { tuple } => format!("a tuple of {} members", tuple.len()),
            Ty::Fn { fn_ } => format!(
                "a function taking {} and answering {}",
                fn_.takes
                    .iter()
                    .map(Ty::spelt)
                    .collect::<Vec<_>>()
                    .join(", "),
                fn_.answers.spelt()
            ),
            Ty::List { list } => format!("a List of {}", list.spelt()),
            Ty::Set { set } => format!("a Set of {}", set.spelt()),
            Ty::Map { map } => format!("a Map from {} to {}", map.key.spelt(), map.value.spelt()),
            Ty::Var { var } => format!("the type variable {var}"),
            Ty::Nothing { .. } => "Nothing".to_string(),
            Ty::Never { .. } => "Never".to_string(),
        }
    }

    /// Every type directly inside this one, in the order it is written.
    ///
    /// No arm standing for the rest, so a type added here is one every walk over types stops
    /// compiling over until it says what it holds.
    pub fn members(&self) -> Vec<&Ty> {
        match self {
            Ty::Prim { .. }
            | Ty::Ref { .. }
            | Ty::Union { .. }
            | Ty::Var { .. }
            | Ty::Nothing { .. }
            | Ty::Never { .. } => Vec::new(),
            Ty::Option { option: held } | Ty::List { list: held } | Ty::Set { set: held } => {
                vec![held]
            }
            Ty::Tuple { tuple } => tuple.iter().collect(),
            Ty::Fn { fn_ } => fn_.takes.iter().chain([fn_.answers.as_ref()]).collect(),
            Ty::Map { map } => vec![&map.key, &map.value],
        }
    }

    /// Whether this type writes the type of what has no value anywhere in it.
    pub fn writes_nothing(&self) -> bool {
        matches!(self, Ty::Nothing { .. }) || self.members().into_iter().any(Ty::writes_nothing)
    }

    /// Whether this type writes, anywhere in it, a type no source writes: the type of what has no
    /// value, or of what does not answer. The checker gives these and the model has no name for
    /// either, so nothing that describes a type in the model's terms can describe one.
    pub fn writes_what_no_source_writes(&self) -> bool {
        matches!(self, Ty::Nothing { .. } | Ty::Never { .. })
            || self
                .members()
                .into_iter()
                .any(Ty::writes_what_no_source_writes)
    }

    /// Whether this type writes a type variable anywhere in it.
    pub fn is_open(&self) -> bool {
        let mut numbers = std::collections::BTreeSet::new();
        self.numbers(&mut numbers);
        !numbers.is_empty()
    }

    /// Every number a type variable is written under in this type, added to `numbers`.
    pub fn numbers(&self, numbers: &mut std::collections::BTreeSet<usize>) {
        if let Ty::Var { var } = self {
            numbers.insert(*var);
        }
        for member in self.members() {
            member.numbers(numbers);
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "core", rename_all = "lowercase", deny_unknown_fields)]
pub enum Node {
    Int {
        value: i64,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// The number the document knows a binding by, counted where the binder was written.
    Read {
        binding: usize,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Bool {
        value: bool,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// The text a literal spells, already in the form the language keeps it in.
    ///
    /// Normalized to NFC on the way in, by the compiler that read the source. Two canonically
    /// equivalent spellings are one text by Unicode's definition and two by a comparison of code
    /// units, and which of them an editor wrote is not something the author chose — so the folding
    /// belongs where text arrives from outside, and that is not here. Nothing on this side
    /// normalizes, and a driver that did would be folding a second time whatever had already
    /// crossed.
    #[serde(rename = "string")]
    Str {
        value: String,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A `Decimal` literal, as the checker read it: its integer, as integer text since it has as
    /// many digits as it has, and its scale. `1.50m` is `150` at scale 2.
    ///
    /// The two numbers and not the text it was written as. How a literal is spelt is the source's
    /// grammar, which the checker has already read, and a writer that handed this side the
    /// spelling would be asking it to read the grammar a second time.
    Decimal {
        unscaled: String,
        scale: i32,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Binary {
        op: Op,
        /// What the operator reads its operands as, which the checker settled and the operands'
        /// types do not say.
        reading: Reading,
        left: Box<Node>,
        right: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Neg {
        operand: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A name for a value, and what is written under it. The number is the document's, given where
    /// the binder is written.
    ///
    /// `binds` is what the name is in force at, which every read of it is typed as. It is not
    /// `value`'s type: an annotation, or a sum the value is one case of, binds the name wider than
    /// the value it is given.
    Let {
        binding: usize,
        binds: Ty,
        value: Box<Node>,
        body: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    If {
        cond: Box<Node>,
        then: Box<Node>,
        #[serde(rename = "else")]
        els: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A value of a type with nothing in it. It still says which type it is: that is what it is.
    Unit {
        unit: Case,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// Every declared field, in declaration order, which is also the order they are worked out in.
    Construct {
        declared: String,
        values: Vec<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// An attempted construction: the fields worked out as a construction's are, and which way the
    /// run goes decided by the declaration's clauses. Where every clause holds, the value is bound
    /// under `binding` at `binds` and `then` answers; where one does not, what `departures` says
    /// answers that clause does.
    ///
    /// Its own node and not a [`Node::Construct`] under a fork: a construction ends the run where a
    /// clause does not hold, and this never does, so a walk that met a construction here would be
    /// told something this is not. `type` is what the branches join at, and not what is built.
    Attempt {
        declared: String,
        values: Vec<Node>,
        binding: usize,
        binds: Ty,
        then: Box<Node>,
        departures: Departures,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Field {
        target: Box<Node>,
        field: String,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Match {
        subject: Box<Node>,
        arms: Vec<Arm>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Some {
        value: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    None {
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Tuple {
        members: Vec<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Member {
        tuple: Box<Node>,
        at: usize,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A list written out element by element, in the order it holds them. Its type is the list's,
    /// and says what every element is.
    List {
        elements: Vec<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A call, and what the checker settled it reaches.
    Call {
        reaches: Reaches,
        arguments: Vec<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A function value: its own parameters, and the body they are bound in — written whole and
    /// not closure-converted on the wire. What of the body's free bindings this side has to carry
    /// forward as runtime state, and how, is this driver's own representation question; see
    /// `closures` in the crate root.
    ///
    /// `site` is this document's own number for where the block stands, minted by `ProgramWriter`
    /// so a lifted function can be declared under it before this side has decided anything about
    /// what that function closes over — the same role a binding's number plays for a read, and
    /// counted the same way: document-wide, where the block is written.
    Block {
        site: usize,
        parameters: Vec<Parameter>,
        body: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A function value applied to arguments — a value the body holds, and not a call to something
    /// declared elsewhere ([`Node::Call`]'s own `reaches`). `function` is a [`Node::Read`] every
    /// time souther's checker builds one (`Core.Apply`'s own contract), read the same way any
    /// other operand's is rather than reduced to a binding number bare beside `arguments`.
    Apply {
        function: Box<Node>,
        arguments: Vec<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A value standing as a type other than its own, where the checker decided that it may.
    ///
    /// `value` is what is evaluated, at the type it was worked out at, and `ty` is what the position
    /// it stands in takes it as. Written only where the two differ, so every other position holds a
    /// value of exactly the type it takes. Why the checker let it stand there is not carried, and
    /// nothing here works it out again.
    Widen {
        value: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// Where the run ends, with the reason the author wrote. `type` is what the position it stands
    /// in takes where the position states one, and `Never` where it states none and a branch
    /// beside it gives the fork its type. Nothing is made of either here, since nothing past this
    /// point runs. The reason is carried though the status a run ends with has no room for it yet.
    Unreachable {
        reason: String,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
}

/// What a [`Node::Attempt`] answers where a clause does not hold, in the one of the two forms the
/// language has that it was written in.
///
/// Two forms and not one list with a catch-all, because the arm naming no clause means a different
/// thing in each: on its own it answers every clause, and beside arms naming clauses it answers the
/// clauses that have no name and nothing else. A list read one way for both is how an arm naming
/// no clause came to answer a named clause no arm named.
///
/// The document writes the arms as the checker keeps them, a list, and which form the list is in
/// is told here, where it is read, the way the checker tells it (`mapsClauses`): one arm naming no
/// clause is the first form, and anything else the second.
#[derive(Debug, Deserialize, Clone)]
#[serde(try_from = "Vec<Departure>")]
pub enum Departures {
    /// `else e`, or `| _ -> e` on its own: one value for whichever clause did not hold.
    Any(Box<Node>),
    /// One arm per clause: an arm for each clause with a name, by that name, and the arm naming
    /// none for the clauses with no name.
    ByClause {
        named: Vec<(String, Node)>,
        unnamed: Option<Box<Node>>,
    },
}

/// One departure of a [`Node::Attempt`] as the document writes it: the clause it answers, by the
/// name the clause is answered under, or none, and what the run answers there.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Departure {
    pub clause: Option<String>,
    pub body: Node,
}

impl TryFrom<Vec<Departure>> for Departures {
    type Error = String;

    fn try_from(written: Vec<Departure>) -> Result<Self, Self::Error> {
        if let [
            Departure {
                clause: None,
                body: _,
            },
        ] = written.as_slice()
        {
            let only = written.into_iter().next().expect("one departure");
            return Ok(Departures::Any(Box::new(only.body)));
        }
        if written.is_empty() {
            return Err("an attempted construction departs nowhere".to_string());
        }
        let mut named = Vec::new();
        let mut unnamed = None;
        for departure in written {
            match departure.clause {
                Some(name) => named.push((name, departure.body)),
                None => {
                    if unnamed.replace(Box::new(departure.body)).is_some() {
                        return Err(
                            "two departures answer the clauses that have no name".to_string()
                        );
                    }
                }
            }
        }
        Ok(Departures::ByClause { named, unnamed })
    }
}

impl Departures {
    /// What each departure answers, in the order [`Node::children`] lists them: the arms naming a
    /// clause as they were written, and then the one naming none.
    pub fn bodies(&self) -> Vec<&Node> {
        match self {
            Departures::Any(body) => vec![body],
            Departures::ByClause { named, unnamed } => named
                .iter()
                .map(|(_, body)| body)
                .chain(unnamed.as_deref())
                .collect(),
        }
    }

    /// The same bodies, in the same order, to be rewritten in place.
    pub fn bodies_mut(&mut self) -> Vec<&mut Node> {
        match self {
            Departures::Any(body) => vec![body],
            Departures::ByClause { named, unnamed } => named
                .iter_mut()
                .map(|(_, body)| body)
                .chain(unnamed.as_deref_mut())
                .collect(),
        }
    }
}

/// One parameter of a [`Node::Block`], numbered the way any other binder on the wire is: where it
/// is written, by `ProgramWriter`'s own counter.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub binding: usize,
    pub name: String,
}

/// What a call reaches, which the checker decided and nothing here works out again.
///
/// A nested object — `{"is":"behavior","declared":"..."}` — and not `declared`/`kernel` beside
/// `reaches` as siblings of `Node::Call`'s own fields: a document naming `kernel` beside
/// `is:"behavior"`, or naming `is:"kernel"` with no `kernel` at all, does not parse as one of
/// these rather than parsing and leaving `lower` to find out with an `.expect()` at the one place
/// it is read. (An adjacently-nested object rather than `#[serde(flatten)]` on a sibling of
/// `Node::Call`'s own fields, deliberately — `flatten` inside a `Node` whose own variants are
/// chosen by an internal tag (`"core"`) asks serde to buffer the same map twice over, which it
/// does not support and fails at every call site rather than only the ambiguous ones.) `kernel`
/// itself crosses as the key the standard library declares it under (`"int.add"`) and not as a
/// closed enum here: which kernels exist is the language's question and this side's only question
/// is which of them it can lower, answered by `NotLowered` at the one place that tries, not by a
/// vocabulary this file would have to keep in step with every kernel the language ever adds.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Reaches {
    /// A definition the calling module holds, which is a copy of its own, by the reference the
    /// call reaches it by: the same value the definition is written under ([`Held::reached`]).
    Helper { reached: Reference },
    /// A value that runs where it is declared, and this module is that module: an ordinary call to
    /// the method this object runs the value as, the same call a helper's own reach is (souther's
    /// JVM backend calls it through the identical path a recursive helper's is — `BodyGen`'s
    /// `recursiveHelperCall`). "Runs once" is `ADR-0074`'s checker-level guarantee that the region
    /// reading a value's reference builds each of its dependencies once, threaded through
    /// [`Handover`] — not a runtime cache this side has to keep; nothing here memoizes a call's
    /// answer. Split identity, and not `declared` joined the way [`Helper`](Reaches::Helper)'s and
    /// [`Behavior`](Reaches::Behavior)'s are, because the *entry* a module publishes for this value
    /// needs its module and name apart to build `value_symbol` from — carried apart here too, so a
    /// lowering never has to split a joined spelling back up to answer "which module declares
    /// this."
    Value { module: String, name: String },
    /// A value another module declares, reached through the entry that module publishes for it —
    /// never a method of the emitting module. Its own tag and not [`Value`](Reaches::Value): what
    /// answers the call is `value_symbol(module, name)`, an entry across an object boundary, where
    /// [`Value`](Reaches::Value) is a call within this object to a method reached the way a helper
    /// is — a caller emitting one must not have to tell them apart by re-deriving whether `module`
    /// is its own.
    PublishedValue { module: String, name: String },
    /// A behavior, whether this object answers it or another object does.
    Behavior { declared: String },
    /// An operation the checker's compiler mints after everything is resolved, for a shape a
    /// backend lowers whole: no source names one and no module declares one. By the member it is,
    /// and not by what it renders as, which is for a report to quote.
    ///
    /// A closed set, unlike a kernel's key: the checker's compiler emits these and nothing else
    /// does, so a member it adds is one this side has to say something about before a document
    /// holding it reads — which one it lowers is still [`NotLowered`](crate::NotLowered)'s to say.
    Emitted { operation: Emitted },
    /// An operation the language itself implements, with what this application of it takes each
    /// argument as and what else the checker settled about it. The kernel's own signature has type
    /// variables, and what they came to for this call is the checker's answer.
    Kernel {
        kernel: String,
        takes: Vec<Ty>,
        fact: KernelFact,
    },
}

/// An operation the checker's compiler emits (`Core.Emitted`), each for a fold it rewrote so that
/// the collection the fold only grows is built rather than rebuilt at every step.
///
/// The order is upstream's, which `vocabularies.rs` holds the spelling of each to.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Emitted {
    /// `$build(step, xs, from)`: the walk of `xs` from `from` that grows a list and hands it over
    /// once the walk ends.
    BuildList,
    /// `$grow(acc, rhs)`, inside the step of a [`BuildList`](Emitted::BuildList): `rhs` added to
    /// the list the walk grows, and that list answered.
    GrowList,
    /// The same walk for a fold accumulating a map.
    BuildMap,
    /// The step's write into the map such a walk grows.
    PutMap,
}

impl Emitted {
    /// What a report calls it: what the checker's compiler renders it as.
    pub fn spelt(self) -> &'static str {
        match self {
            Emitted::BuildList => "List.$build",
            Emitted::GrowList => "List.$grow",
            Emitted::BuildMap => "Map.$build",
            Emitted::PutMap => "Map.$put",
        }
    }
}

/// A fact the checker settled about one application of a kernel, beside what it takes.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum KernelFact {
    None,
    /// What the pattern `String.matches`'s first argument folds to means, and the text it was
    /// written as.
    StringMatches {
        /// The pattern as its author wrote it: said in a message, and never read as a pattern.
        written: String,
        /// Which strings it accepts, as the checker read the text: each part after the parts it
        /// is made of, the whole last.
        meaning: Vec<PatternPart>,
    },
    /// The type an ordering was checked against.
    OrderingSubject {
        #[serde(rename = "type")]
        ty: Ty,
    },
}

/// One part of what a pattern means (`PatternMeaning`), naming the parts it is made of by where
/// they stand in the list it is written in, which is always before it.
///
/// A list and not a tree because a pattern nests as deep as the checker reads one, and a document
/// nesting as deep would be refused by the reader for its depth.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum PatternPart {
    /// The one string of no characters.
    Nothing,
    /// No string at all.
    Never,
    /// One character out of these runs, both ends in each, sorted and apart.
    Symbols { ranges: Vec<(u32, u32)> },
    /// One after another.
    InTurn { parts: Vec<usize> },
    /// Any one of them.
    EitherOf { arms: Vec<usize> },
    /// The same thing between `least` and `most` times, with no ceiling where `most` is absent.
    Repeated {
        what: usize,
        least: u32,
        most: Option<u32>,
    },
}

/// What an operator reads its two operands as, as the checker settled it. Not a place either
/// operand stands: a literal beside a newtype is read as the newtype by this operator and by
/// nothing else.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Reading {
    /// Each operand as the type it has, which is one type for both.
    AsTheyStand,
    /// The pair as values of this type, for this operator only.
    In {
        #[serde(rename = "type")]
        ty: Ty,
    },
    /// Each operand at its exact mathematical value, which one of them already being a `Rational`
    /// makes of the pair. No type of the language stands for it.
    ExactNumbers,
}

/// One arm of a fork on what a value is.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Arm {
    pub selects: Vec<Selects>,
    /// The number the body reads the value under, where the arm binds it at all.
    pub binding: Option<usize>,
    /// What the value is read as inside the arm.
    ///
    /// Carried rather than worked out from what the arm tests, because the test does not say it:
    /// an optional's present carrier is tested the same way whatever it holds, so a reader that
    /// took the type from the test would read every optional's value at one width.
    pub binds: Option<Ty>,
    pub body: Node,
}

/// What one case of an arm tests for.
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "tests", rename_all = "lowercase", deny_unknown_fields)]
pub enum Selects {
    /// The value's own type is one of these. The atoms are the leaves the checker resolved the
    /// case to, so a case that is a sum arrives as the several types it stands for — each as the
    /// case identity it is, and a declared one by the key that reaches its declaration.
    Which {
        atoms: Cases,
    },
    Held,
    Nothing,
}

impl Reaches {
    /// Every type this reach writes.
    ///
    /// Every field named, so a type-bearing field added to a reach is one this stops compiling
    /// over until it is listed.
    pub fn types(&self) -> Vec<&Ty> {
        match self {
            Reaches::Helper { reached: _ }
            | Reaches::Value { module: _, name: _ }
            | Reaches::PublishedValue { module: _, name: _ }
            | Reaches::Behavior { declared: _ }
            | Reaches::Emitted { operation: _ } => Vec::new(),
            Reaches::Kernel {
                kernel: _,
                takes,
                fact,
            } => takes.iter().chain(fact.types()).collect(),
        }
    }

    /// The same types, to be rewritten in place.
    pub fn types_mut(&mut self) -> Vec<&mut Ty> {
        match self {
            Reaches::Helper { reached: _ }
            | Reaches::Value { module: _, name: _ }
            | Reaches::PublishedValue { module: _, name: _ }
            | Reaches::Behavior { declared: _ }
            | Reaches::Emitted { operation: _ } => Vec::new(),
            Reaches::Kernel {
                kernel: _,
                takes,
                fact,
            } => takes.iter_mut().chain(fact.types_mut()).collect(),
        }
    }
}

impl KernelFact {
    /// Every type this fact writes.
    pub fn types(&self) -> Vec<&Ty> {
        match self {
            KernelFact::None
            | KernelFact::StringMatches {
                written: _,
                meaning: _,
            } => Vec::new(),
            KernelFact::OrderingSubject { ty } => vec![ty],
        }
    }

    /// The same types, to be rewritten in place.
    pub fn types_mut(&mut self) -> Vec<&mut Ty> {
        match self {
            KernelFact::None
            | KernelFact::StringMatches {
                written: _,
                meaning: _,
            } => Vec::new(),
            KernelFact::OrderingSubject { ty } => vec![ty],
        }
    }
}

impl Reading {
    /// Every type this reading writes.
    pub fn types(&self) -> Vec<&Ty> {
        match self {
            Reading::AsTheyStand | Reading::ExactNumbers => Vec::new(),
            Reading::In { ty } => vec![ty],
        }
    }

    /// The same types, to be rewritten in place.
    pub fn types_mut(&mut self) -> Vec<&mut Ty> {
        match self {
            Reading::AsTheyStand | Reading::ExactNumbers => Vec::new(),
            Reading::In { ty } => vec![ty],
        }
    }

    /// What a refusal says this reading is.
    pub fn spelt(&self) -> String {
        match self {
            Reading::AsTheyStand => "as they stand".to_string(),
            Reading::ExactNumbers => "at their exact values".to_string(),
            Reading::In { ty } => format!("in {}", ty.spelt()),
        }
    }
}

impl Node {
    /// Every type this node itself writes: its own, and each one it carries beside it (what a let
    /// binds, what an arm reads a value as, what an operator reads its operands in, what a kernel's
    /// application takes and what it was settled against).
    ///
    /// The one enumeration of them, so that "every type the document writes is one it declares" is
    /// held of all of them at one place. Every field of every node is named, and none is left to
    /// `..`, so a type-bearing field added to a node is one this stops compiling over until it is
    /// listed here.
    pub fn types(&self) -> Vec<&Ty> {
        match self {
            Node::Int {
                value: _,
                ty,
                aborts: _,
            }
            | Node::Read {
                binding: _,
                ty,
                aborts: _,
            }
            | Node::Bool {
                value: _,
                ty,
                aborts: _,
            }
            | Node::Str {
                value: _,
                ty,
                aborts: _,
            }
            | Node::Decimal {
                unscaled: _,
                scale: _,
                ty,
                aborts: _,
            }
            | Node::Neg {
                operand: _,
                ty,
                aborts: _,
            }
            | Node::If {
                cond: _,
                then: _,
                els: _,
                ty,
                aborts: _,
            }
            | Node::Unit {
                unit: _,
                ty,
                aborts: _,
            }
            | Node::Unreachable {
                reason: _,
                ty,
                aborts: _,
            }
            | Node::Construct {
                declared: _,
                values: _,
                ty,
                aborts: _,
            }
            | Node::Field {
                target: _,
                field: _,
                ty,
                aborts: _,
            }
            | Node::Some {
                value: _,
                ty,
                aborts: _,
            }
            | Node::None { ty, aborts: _ }
            | Node::Tuple {
                members: _,
                ty,
                aborts: _,
            }
            | Node::Member {
                tuple: _,
                at: _,
                ty,
                aborts: _,
            }
            | Node::List {
                elements: _,
                ty,
                aborts: _,
            }
            | Node::Block {
                site: _,
                parameters: _,
                body: _,
                ty,
                aborts: _,
            }
            | Node::Widen {
                value: _,
                ty,
                aborts: _,
            }
            | Node::Apply {
                function: _,
                arguments: _,
                ty,
                aborts: _,
            } => vec![ty],
            Node::Binary {
                op: _,
                reading,
                left: _,
                right: _,
                ty,
                aborts: _,
            } => std::iter::once(ty).chain(reading.types()).collect(),
            Node::Let {
                binding: _,
                binds,
                value: _,
                body: _,
                ty,
                aborts: _,
            } => vec![ty, binds],
            Node::Attempt {
                declared: _,
                values: _,
                binding: _,
                binds,
                then: _,
                // A departure carries no type of its own: its body stands at the attempt's.
                departures: _,
                ty,
                aborts: _,
            } => vec![ty, binds],
            Node::Match {
                subject: _,
                arms,
                ty,
                aborts: _,
            } => std::iter::once(ty)
                .chain(arms.iter().filter_map(|arm| {
                    let Arm {
                        selects: _,
                        binding: _,
                        binds,
                        body: _,
                    } = arm;
                    binds.as_ref()
                }))
                .collect(),
            Node::Call {
                reaches,
                arguments: _,
                ty,
                aborts: _,
            } => std::iter::once(ty).chain(reaches.types()).collect(),
        }
    }

    /// Every type this node itself writes, as [`Node::types`] lists them, to be rewritten in place.
    ///
    /// The same fields, named the same way, so that a type [`Node::types`] reads is one this
    /// rewrites: a copy of a body with its variables settled ([`crate::specialize`]) that settled
    /// fewer types than are read would leave a variable standing where a lowering reads a type.
    pub fn types_mut(&mut self) -> Vec<&mut Ty> {
        match self {
            Node::Int { ty, .. }
            | Node::Read { ty, .. }
            | Node::Bool { ty, .. }
            | Node::Str { ty, .. }
            | Node::Decimal { ty, .. }
            | Node::Neg { ty, .. }
            | Node::If { ty, .. }
            | Node::Unit { ty, .. }
            | Node::Construct { ty, .. }
            | Node::Field { ty, .. }
            | Node::Some { ty, .. }
            | Node::None { ty, .. }
            | Node::Tuple { ty, .. }
            | Node::Member { ty, .. }
            | Node::List { ty, .. }
            | Node::Block { ty, .. }
            | Node::Widen { ty, .. }
            | Node::Unreachable { ty, .. }
            | Node::Apply { ty, .. } => vec![ty],
            Node::Binary { reading, ty, .. } => {
                std::iter::once(ty).chain(reading.types_mut()).collect()
            }
            Node::Let { binds, ty, .. } | Node::Attempt { binds, ty, .. } => vec![ty, binds],
            Node::Match { arms, ty, .. } => std::iter::once(ty)
                .chain(arms.iter_mut().filter_map(|arm| arm.binds.as_mut()))
                .collect(),
            Node::Call { reaches, ty, .. } => {
                std::iter::once(ty).chain(reaches.types_mut()).collect()
            }
        }
    }

    /// The nodes directly under this one, as [`Node::children`] lists them, to be rewritten in
    /// place.
    pub fn children_mut(&mut self) -> Vec<&mut Node> {
        match self {
            Node::Binary { left, right, .. } => vec![left, right],
            Node::Neg { operand, .. } => vec![operand],
            Node::Let { value, body, .. } => vec![value, body],
            Node::If {
                cond, then, els, ..
            } => vec![cond, then, els],
            Node::Construct { values, .. } => values.iter_mut().collect(),
            Node::Attempt {
                values,
                then,
                departures,
                ..
            } => values
                .iter_mut()
                .chain(std::iter::once(then.as_mut()))
                .chain(departures.bodies_mut())
                .collect(),
            Node::Field { target, .. } => vec![target],
            Node::Match { subject, arms, .. } => std::iter::once(subject.as_mut())
                .chain(arms.iter_mut().map(|arm| &mut arm.body))
                .collect(),
            Node::Some { value, .. } | Node::Widen { value, .. } => vec![value],
            Node::Tuple { members, .. } => members.iter_mut().collect(),
            Node::Member { tuple, .. } => vec![tuple],
            Node::List { elements, .. } => elements.iter_mut().collect(),
            Node::Call { arguments, .. } => arguments.iter_mut().collect(),
            Node::Block { body, .. } => vec![body],
            Node::Apply {
                function,
                arguments,
                ..
            } => std::iter::once(function.as_mut())
                .chain(arguments.iter_mut())
                .collect(),
            Node::Int { .. }
            | Node::Read { .. }
            | Node::Bool { .. }
            | Node::Str { .. }
            | Node::Decimal { .. }
            | Node::Unit { .. }
            | Node::Unreachable { .. }
            | Node::None { .. } => Vec::new(),
        }
    }

    /// What the checker says this node can end a run without a value for.
    pub fn aborts(&self) -> &[AbortKind] {
        match self {
            Node::Int { aborts, .. }
            | Node::Read { aborts, .. }
            | Node::Bool { aborts, .. }
            | Node::Str { aborts, .. }
            | Node::Decimal { aborts, .. }
            | Node::Binary { aborts, .. }
            | Node::Neg { aborts, .. }
            | Node::Let { aborts, .. }
            | Node::If { aborts, .. }
            | Node::Unit { aborts, .. }
            | Node::Construct { aborts, .. }
            | Node::Attempt { aborts, .. }
            | Node::Field { aborts, .. }
            | Node::Match { aborts, .. }
            | Node::Some { aborts, .. }
            | Node::None { aborts, .. }
            | Node::Tuple { aborts, .. }
            | Node::Member { aborts, .. }
            | Node::List { aborts, .. }
            | Node::Call { aborts, .. }
            | Node::Block { aborts, .. }
            | Node::Widen { aborts, .. }
            | Node::Unreachable { aborts, .. }
            | Node::Apply { aborts, .. } => aborts,
        }
    }

    /// Whether a node of this kind is one the checker ever gives a reason to end a run without a
    /// value: arithmetic and negation over a number, a construction of a type that states a
    /// clause, and a call to a kernel. Every other kind is total in itself, and what a call to a
    /// behavior, a helper or a value ends with is the callee's own. An attempted construction is
    /// total too: a clause that does not hold takes a departure, and what a clause ends with where
    /// it does not answer is that clause's own, as a callee's is.
    ///
    /// Asked of what decides it and not of the kind alone: a binary operator by which operator it
    /// is, since a comparison, a truth operator and a join end no run and arithmetic may, and a call
    /// by what it reaches. Named for every kind and every operator, with no arm standing for the
    /// rest, so one added to the document has to be said to be one or the other.
    pub fn can_end_without_a_value(&self) -> bool {
        match self {
            Node::Binary { op, .. } => match op {
                Op::Add | Op::Sub | Op::Mul | Op::Div => true,
                Op::Eq
                | Op::Ne
                | Op::Lt
                | Op::Le
                | Op::Gt
                | Op::Ge
                | Op::And
                | Op::Or
                | Op::Concat => false,
            },
            // An `unreachable` is where the run ends, and ending it is all it does.
            Node::Neg { .. } | Node::Construct { .. } | Node::Unreachable { .. } => true,
            Node::Call { reaches, .. } => matches!(reaches, Reaches::Kernel { .. }),
            Node::Int { .. }
            | Node::Read { .. }
            | Node::Bool { .. }
            | Node::Str { .. }
            | Node::Decimal { .. }
            | Node::Let { .. }
            | Node::If { .. }
            | Node::Unit { .. }
            | Node::Attempt { .. }
            | Node::Field { .. }
            | Node::Match { .. }
            | Node::Some { .. }
            | Node::None { .. }
            | Node::Tuple { .. }
            | Node::Member { .. }
            | Node::List { .. }
            | Node::Block { .. }
            | Node::Widen { .. }
            | Node::Apply { .. } => false,
        }
    }

    /// The nodes directly under this one, in the order they are written.
    ///
    /// No arm standing for the rest: a node added to the document is one whose children every walk
    /// over this would otherwise silently never reach.
    pub fn children(&self) -> Vec<&Node> {
        match self {
            Node::Binary { left, right, .. } => vec![left, right],
            Node::Neg { operand, .. } => vec![operand],
            Node::Let { value, body, .. } => vec![value, body],
            Node::If {
                cond, then, els, ..
            } => vec![cond, then, els],
            Node::Construct { values, .. } => values.iter().collect(),
            Node::Attempt {
                values,
                then,
                departures,
                ..
            } => values
                .iter()
                .chain(std::iter::once(then.as_ref()))
                .chain(departures.bodies())
                .collect(),
            Node::Field { target, .. } => vec![target],
            Node::Match { subject, arms, .. } => std::iter::once(subject.as_ref())
                .chain(arms.iter().map(|arm| &arm.body))
                .collect(),
            Node::Some { value, .. } | Node::Widen { value, .. } => vec![value],
            Node::Tuple { members, .. } => members.iter().collect(),
            Node::Member { tuple, .. } => vec![tuple],
            Node::List { elements, .. } => elements.iter().collect(),
            Node::Call { arguments, .. } => arguments.iter().collect(),
            Node::Block { body, .. } => vec![body],
            Node::Apply {
                function,
                arguments,
                ..
            } => std::iter::once(function.as_ref())
                .chain(arguments.iter())
                .collect(),
            Node::Int { .. }
            | Node::Read { .. }
            | Node::Bool { .. }
            | Node::Str { .. }
            | Node::Decimal { .. }
            | Node::Unit { .. }
            | Node::Unreachable { .. }
            | Node::None { .. } => Vec::new(),
        }
    }

    /// The declaration this node builds a value of, where it builds one: a construction from
    /// fields, and a unit's value, which is a construction from none.
    ///
    /// Asked here, once, by everything that has to know what a body builds — which constructors an
    /// object defines and which it reaches — so that what counts as building a value is not a list
    /// of kinds each of them keeps.
    ///
    /// Not an attempted construction, which reaches what decides a construction and never the
    /// constructor ([`Node::attempts`]).
    pub fn builds(&self) -> Option<&str> {
        match self {
            Node::Construct { declared, .. } => Some(declared),
            // A unit a module declares is built by its constructor. One the language gives is
            // the runtime's token and is built by nothing here.
            Node::Unit { unit, .. } => unit.declared(),
            Node::Attempt { .. }
            | Node::Int { .. }
            | Node::Read { .. }
            | Node::Bool { .. }
            | Node::Str { .. }
            | Node::Decimal { .. }
            | Node::Binary { .. }
            | Node::Neg { .. }
            | Node::Let { .. }
            | Node::If { .. }
            | Node::Field { .. }
            | Node::Match { .. }
            | Node::Some { .. }
            | Node::None { .. }
            | Node::Tuple { .. }
            | Node::Member { .. }
            | Node::List { .. }
            | Node::Call { .. }
            | Node::Block { .. }
            | Node::Apply { .. }
            | Node::Widen { .. }
            | Node::Unreachable { .. } => None,
        }
    }

    /// The declaration this node attempts to build a value of, where it attempts one.
    ///
    /// Apart from [`Node::builds`] because the two reach different functions of the declaring
    /// object: a construction its constructor, which ends the run where a clause does not hold,
    /// and an attempt what decides a construction, which answers which clause did not.
    pub fn attempts(&self) -> Option<&str> {
        match self {
            Node::Attempt { declared, .. } => Some(declared),
            Node::Int { .. }
            | Node::Read { .. }
            | Node::Bool { .. }
            | Node::Str { .. }
            | Node::Decimal { .. }
            | Node::Binary { .. }
            | Node::Neg { .. }
            | Node::Let { .. }
            | Node::If { .. }
            | Node::Unit { .. }
            | Node::Construct { .. }
            | Node::Field { .. }
            | Node::Match { .. }
            | Node::Some { .. }
            | Node::None { .. }
            | Node::Tuple { .. }
            | Node::Member { .. }
            | Node::List { .. }
            | Node::Call { .. }
            | Node::Block { .. }
            | Node::Apply { .. }
            | Node::Widen { .. }
            | Node::Unreachable { .. } => None,
        }
    }

    /// Every node the document writes under this one, this one first, depth first and in the order
    /// they are written, whether or not anything runs it.
    ///
    /// For a question about what the document says. What an object runs, reaches or has to lower is
    /// a narrower question, since the step of a walk that never runs is written and never lowered,
    /// and it is walked with `unrun::each_lowered`. There is no walk called plain `each`, so a
    /// pass walking a body says which of the two it asks.
    pub fn each_written<'n>(&'n self, visit: &mut impl FnMut(&'n Node)) {
        visit(self);
        for child in self.children() {
            child.each_written(visit);
        }
    }

    /// The type the checker decided for this expression.
    ///
    /// Read off the node rather than worked out from where it sits: what a comparison compares is
    /// not what a comparison answers, and a lowering that took the second for the first would
    /// compare two values at the width of the answer.
    pub fn ty(&self) -> &Ty {
        match self {
            Node::Int { ty, .. }
            | Node::Read { ty, .. }
            | Node::Bool { ty, .. }
            | Node::Str { ty, .. }
            | Node::Decimal { ty, .. }
            | Node::Binary { ty, .. }
            | Node::Neg { ty, .. }
            | Node::Let { ty, .. }
            | Node::If { ty, .. }
            | Node::Unit { ty, .. }
            | Node::Construct { ty, .. }
            | Node::Attempt { ty, .. }
            | Node::Field { ty, .. }
            | Node::Match { ty, .. }
            | Node::Some { ty, .. }
            | Node::None { ty, .. }
            | Node::Tuple { ty, .. }
            | Node::Member { ty, .. }
            | Node::List { ty, .. }
            | Node::Call { ty, .. }
            | Node::Block { ty, .. }
            | Node::Apply { ty, .. }
            | Node::Widen { ty, .. }
            | Node::Unreachable { ty, .. } => ty,
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum Op {
    #[serde(rename = "EQ")]
    Eq,
    #[serde(rename = "NE")]
    Ne,
    #[serde(rename = "LT")]
    Lt,
    #[serde(rename = "LE")]
    Le,
    #[serde(rename = "GT")]
    Gt,
    #[serde(rename = "GE")]
    Ge,
    #[serde(rename = "AND")]
    And,
    #[serde(rename = "OR")]
    Or,
    #[serde(rename = "ADD")]
    Add,
    #[serde(rename = "SUB")]
    Sub,
    #[serde(rename = "MUL")]
    Mul,
    #[serde(rename = "DIV")]
    Div,
    #[serde(rename = "CONCAT")]
    Concat,
}

impl Op {
    /// What a reader of a refusal is told this was.
    pub fn spelt(self) -> &'static str {
        match self {
            Op::Eq => "==",
            Op::Ne => "/=",
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::And => "&&",
            Op::Or => "||",
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "/",
            Op::Concat => "++",
        }
    }
}

/// A way a Souther computation ends without a value, named by the reason the language gives for
/// it — the checker's own closed set, spelt on the wire the same way `Op` and `Prim` are.
///
/// This is not this side's to reclassify. A site's `Node::aborts` is `program.abortsAt(site)`
/// read off the checker, so which member goes with which machine condition is answered once, by
/// whoever maps a member of this to `souther_native_abi`'s own encoding — never re-derived from
/// what a node looks like here.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum AbortKind {
    #[serde(rename = "INVARIANT_NOT_HELD")]
    InvariantNotHeld,
    #[serde(rename = "ENSURES_NOT_HELD")]
    EnsuresNotHeld,
    #[serde(rename = "UNREACHABLE_REACHED")]
    UnreachableReached,
    #[serde(rename = "DIVISION_BY_ZERO")]
    DivisionByZero,
    #[serde(rename = "REQUIRED_FORM_HAS_NO_PLACE")]
    RequiredFormHasNoPlace,
    #[serde(rename = "INVALID_BOUNDS")]
    InvalidBounds,
}

impl AbortKind {
    /// Every member, in the order the Java half's enumeration declares them.
    ///
    /// Written by hand, since Rust has no way to ask an enum for its members. What keeps it whole
    /// is [`AbortKind::spelt`] and `native_status` beside it: a member added to the enum stops both
    /// compiling until it is answered for, and whoever answers it there is in the one place this
    /// list is also asked to keep up.
    pub const ALL: [AbortKind; 6] = [
        AbortKind::InvariantNotHeld,
        AbortKind::EnsuresNotHeld,
        AbortKind::UnreachableReached,
        AbortKind::DivisionByZero,
        AbortKind::RequiredFormHasNoPlace,
        AbortKind::InvalidBounds,
    ];

    /// How the member is spelt on the wire, which is also what a host is told a status means.
    pub fn spelt(self) -> &'static str {
        match self {
            AbortKind::InvariantNotHeld => "INVARIANT_NOT_HELD",
            AbortKind::EnsuresNotHeld => "ENSURES_NOT_HELD",
            AbortKind::UnreachableReached => "UNREACHABLE_REACHED",
            AbortKind::DivisionByZero => "DIVISION_BY_ZERO",
            AbortKind::RequiredFormHasNoPlace => "REQUIRED_FORM_HAS_NO_PLACE",
            AbortKind::InvalidBounds => "INVALID_BOUNDS",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each move is under the number after the one before it, so a version is never taken twice
    /// and never skipped.
    #[test]
    fn every_move_takes_the_next_version() {
        for pair in MOVES.windows(2) {
            assert_eq!(
                pair[1].0,
                pair[0].0 + 1,
                "{:?} after {:?}",
                pair[1],
                pair[0]
            );
        }
    }
}
