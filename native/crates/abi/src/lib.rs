//! What generated code and the native runtime agree on.
//!
//! Both sides are written here rather than each writing its own copy, because everything in this
//! crate is a two-party contract: a number one side writes and the other reads is wrong in a way
//! neither side's tests can see. Nothing about Souther's meaning belongs here — only the names,
//! layouts and encodings a target decides.

// Everything here is one half of a contract the other half reads by name, so an item whose doc has
// slid off it onto a neighbour is a contract nobody states. Refused rather than warned about.
#![deny(missing_docs)]

/// The generation of *wire contract* every function symbol below answers to — not only the
/// calling convention, but what a `status` other than `ANSWERED` means once it crosses an object
/// boundary.
///
/// Embedded in the symbol itself rather than left for a caller to somehow already know, because a
/// symbol is exactly what a linker resolves by name and nothing else: two objects agreeing on a
/// function's name while disagreeing about how a call to it works — one answering `T` directly,
/// the other `status`, with the value through a pointer neither declared — is undefined behaviour
/// a link step that only checks the name cannot see. Bumping this the day that changes turns that
/// silent mismatch into an undefined-symbol error instead: an object built before the bump and
/// one built after no longer resolve to one another's definition of a name at all.
///
/// Two different questions are both "the day that changes", not only the shape of the call:
///
/// - The calling convention itself — `souther-native-compiler#19` is `2`; `1` was every function
///   answering its value as a plain return, with no generation written into the symbol because
///   there was only ever the one.
/// - What a non-`ANSWERED` `status` *means*. This crate reserves `ANSWERED` and the
///   [`HOST_STATUSES`] — which wire number a language abort gets is `native_status` in `souther-native-driver`'s own
///   exhaustive mapping, kept apart from this crate for the reason this file's own doc gives. Two
///   objects built by drivers whose `native_status` disagrees about what `4` is are exactly as
///   incompatible as two objects with different calling conventions; they just still link, because
///   nothing about the *shape* of the call changed. A renumbering there bumps this the same as a
///   calling-convention change does — see `abort-status-abi4.json` in the driver crate's own
///   tests, named for the generation it is a fixture of.
///
/// `3` is `souther-native-compiler#46`, both at once. A behavior with no body that declares
/// nothing to depend on is answered by whatever a host registered for it on the calling thread,
/// through a function the object of the declaring build defines under the behavior's own symbol,
/// where before that symbol was left for whoever linked the object to define. And a status is no
/// longer either `ANSWERED` or a language abort: [`HOST_STATUSES`] are three a host's
/// implementation brings about, which no Souther computation answers.
///
/// `4` is `souther-native-compiler#72`. A behavior is called with the capabilities it was
/// constructed with, in the order the checker answered what it requires: every behavior's symbol
/// takes an environment first, and one reached through `depends on` is called through the
/// [`capability`](CAPABILITY_INVOKE) its caller holds for it rather than through its own symbol.
/// Nothing is registered on a thread any more: a host builds the capabilities a call is made with
/// ([`host_bind_symbol`], [`host_implement_symbol`]), and the runtime keeps nothing for it. And a
/// row's stand-in answers [`FAKE_NO_OUTPUT`] where the row states nothing for what it was asked.
///
/// Not part of [`type_symbol`]: a declared type's token is data, not a call, and nothing about how
/// a call is made or what its status means changes what a value of one looks like.
///
/// Public, because a host is a party to it too: what a binding reads off the manifest a build
/// writes beside the object says which generation the functions it names answer to, and that is
/// this number and not a copy of it.
///
/// The last of [`GENERATIONS`], and written nowhere else.
pub const ABI_GENERATION: u32 = GENERATIONS[GENERATIONS.len() - 1].0;

/// What each generation moved, oldest first, as the paragraphs above tell it at length.
///
/// A change that moves the generation adds its line at the end under the next number, and the
/// number is read off the last line. Two branches each moving to the same number add two different
/// lines at one place, which a merge stops at; two edits of one constant to the same number merge
/// without a word. That the numbers follow on from one another is held by a test.
pub const GENERATIONS: &[(u32, &str)] = &[
    (
        2,
        "a status answered and the value written through a pointer (souther-native-compiler#19)",
    ),
    (
        3,
        "a behavior with no body answered by what a host registered for it, and the statuses a \
         host's implementation brings about (souther-native-compiler#46)",
    ),
    (
        4,
        "a behavior called with the capabilities it was constructed with, and nothing registered \
         on a thread (souther-native-compiler#72)",
    ),
];

/// Whether a module's name can stand in a symbol: it carries no `$`, which is what every symbol
/// below is split on. A module's name carries dots.
///
/// Asked here and by whoever reads the names a symbol is built from, so that a name the symbols
/// cannot spell is refused where it is read and not where a symbol is being built from it.
pub fn spells_a_module(name: &str) -> bool {
    !name.is_empty() && !name.contains('$')
}

/// Whether the name a module gives a behavior, a value or a type can stand in a symbol: it
/// carries neither a dot, since the module's own name does and the last segment is this, nor a
/// `$`, which the symbols are split on.
pub fn spells_a_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('.') && !name.contains('$')
}

/// The symbol a behavior is reached by.
///
/// `souther<abi>.<module>.<behavior>`, with the module written as it is declared. Both ELF and
/// Mach-O carry a dot in a symbol name, so nothing is replaced on the way.
///
/// What makes the spelling unambiguous is that the behavior is the last segment, and that holds
/// only while a behavior's name carries no dot. That is checked here rather than stated: it is a
/// property of the names Souther admits, and a caller that ever broke it would produce a symbol
/// meaning something other than what it named, which nothing downstream could notice.
///
/// # Panics
///
/// Where the module's name or the behavior's does not stand in a symbol ([`spells_a_module`],
/// [`spells_a_name`]).
pub fn behavior_symbol(module: &str, behavior: &str) -> String {
    assert!(
        spells_a_module(module),
        "a module's name carries no dollar, and the symbol is split on one: {module}"
    );
    assert!(
        spells_a_name(behavior),
        "a behavior's name carries neither dot nor dollar, and it is the symbol's last \
         segment: {behavior}"
    );
    format!("souther{ABI_GENERATION}.{module}.{behavior}")
}

/// The symbol a definition a module holds is reached by.
///
/// The module holding it and the reference that module reaches it by: `taxed` for a helper of the
/// module's own, `pricing.taxed` for one another module declares, `List.foldFrom` for an operation
/// the standard library writes. Not the declaration it is a copy of, which the reference does not
/// spell and which two modules reach under two references. The carrier is in it because a module
/// carries every helper it reaches and two modules reaching one helper hold a copy each — which is
/// what the language says a published helper is. One name for both copies would be one of them
/// silently standing for the other.
///
/// Nothing outside the object reaches one of these, so what this has to be is unambiguous here and
/// nowhere else. The `$` is what keeps it so: a module's name carries dots and a reference carries
/// them too, and neither carries this.
///
/// # Panics
///
/// Where the carrier's name does not stand in a symbol ([`spells_a_module`]).
pub fn held_symbol(carrier: &str, reached: &str) -> String {
    assert!(
        spells_a_module(carrier),
        "a module's name carries no dollar, and the symbol is split on one: {carrier}"
    );
    format!("souther{ABI_GENERATION}.{carrier}${reached}")
}

/// The symbol a value's home is reached by, inside the object of the module that declares it.
///
/// Not [`held_symbol`], although both are a module's own and nothing outside the object reaches
/// either: a helper and a value are two kinds of definition, and one spelling for both would make
/// a helper and a value of one name one symbol. The language never names the two alike, and the
/// symbols do not rely on it.
///
/// # Panics
///
/// Where the module's name or the value's does not stand in a symbol.
pub fn home_symbol(module: &str, value: &str) -> String {
    assert!(
        spells_a_module(module),
        "a module's name carries no dollar, and the symbol is split on one: {module}"
    );
    assert!(
        spells_a_name(value),
        "a value's name carries neither dollar nor dot, and the symbol is split on both: {value}"
    );
    format!("souther{ABI_GENERATION}.{module}$home${value}")
}

/// The symbol the entry a module publishes for one of its values is reached by.
///
/// Not a value's own executable home: a value has exactly one, in the module that declares it
/// (spec ADR-0074), and nothing outside that module ever reaches it directly — a call from
/// elsewhere goes through this entry instead, which is why this alone, and not the home, needs a
/// name a linker resolves. The home is this object's own business, named by [`home_symbol`].
///
/// `souther<abi>.<module>$value$<name>`, carrying the ABI generation the same way
/// [`behavior_symbol`] does: an entry is an ordinary call across an object boundary and a
/// calling-convention change is exactly as breaking for one as it is for a behavior. The `$value$`
/// segment is what keeps this apart from `souther<abi>.<module>.<name>`, which is a behavior's own
/// spelling and not this one's to collide with.
///
/// # Panics
///
/// Where either name carries a dollar, or the value's name carries a dot, for the reason
/// [`type_symbol`] gives.
pub fn value_symbol(module: &str, value: &str) -> String {
    assert!(
        spells_a_module(module),
        "a module's name carries no dollar, and the symbol is split on one: {module}"
    );
    assert!(
        spells_a_name(value),
        "a value's name carries neither dollar nor dot, and the symbol is split on both: {value}"
    );
    format!("souther{ABI_GENERATION}.{module}$value${value}")
}

/// The symbol a value of a declared type is built through: the constructor that takes its fields,
/// runs what the type holds its values to, and lays the value out.
///
/// Defined by the object of the build that declared the type and reached by every other one, the
/// way the type's token is. What a type's clauses read and call is that build's own — a helper it
/// holds, one it keeps to itself — so a build constructing a value of a type another declared calls
/// this rather than running a copy of the clauses it could not hold the whole of.
///
/// A call between objects this compiler built, and not what a host calls to build a value: the
/// convention is the one every generated function has, the fields at their width and `status +
/// out`. `souther<abi>.<module>$construct$<name>`, carrying the ABI generation the way
/// [`value_symbol`] does, and apart from a behavior's spelling the same way.
///
/// # Panics
///
/// Where either name carries a dollar, or the type's name carries a dot, for the reason
/// [`type_symbol`] gives.
pub fn constructor_symbol(module: &str, name: &str) -> String {
    assert!(
        spells_a_module(module),
        "a module's name carries no dollar, and the symbol is split on one: {module}"
    );
    assert!(
        spells_a_name(name),
        "a declared type's name carries neither dollar nor dot, and the symbol is split on \
         both: {name}"
    );
    format!("souther{ABI_GENERATION}.{module}$construct${name}")
}

/// The symbol what decides a construction of a declared type is reached by: the fields, room for
/// the value and room for which clause did not hold, answering a status.
///
/// What [`constructor_symbol`] runs, with the one thing a status cannot say kept apart from it. A
/// clause that does not hold is not a computation that ended without a value: the status is
/// `ANSWERED`, and the clause's room holds that clause's place among the type's, counted from
/// nought in the order a construction runs them. Where every clause held, the room holds
/// [`NO_FAILED_CLAUSE`] and the value is written through its room. A clause that itself ends
/// without a value answers that status, and nothing is written. So the constructor is this with a
/// clause that did not hold answered as a status, and an attempted construction is this with the
/// clause taking an arm.
///
/// Defined by the object of the build that declared the type, beside the constructor, and reached
/// from every other object by name, so that an attempted construction of another build's type runs
/// that build's clauses and no copy of them. `souther<abi>.<module>$checked$<name>`, spelt the way
/// [`constructor_symbol`] is.
///
/// # Panics
///
/// Where either name carries a dollar, or the type's name carries a dot, for the reason
/// [`type_symbol`] gives.
pub fn checked_constructor_symbol(module: &str, name: &str) -> String {
    assert!(
        spells_a_module(module),
        "a module's name carries no dollar, and the symbol is split on one: {module}"
    );
    assert!(
        spells_a_name(name),
        "a declared type's name carries neither dollar nor dot, and the symbol is split on \
         both: {name}"
    );
    format!("souther{ABI_GENERATION}.{module}$checked${name}")
}

/// What the clause's room of [`checked_constructor_symbol`] holds where every clause held and the
/// value was written: below nought, so that it is no clause's place.
pub const NO_FAILED_CLAUSE: i64 = -1;

/// Everything a host calls is named by a C identifier, and this is what one is made of.
///
/// A host is a third party to this backend, and the one thing every host can write is a C
/// declaration: a C compiler reads one, and so does an FFI that declares functions from C source.
/// Neither can name a symbol carrying a `.` or a `$`, so what a host reaches is spelt apart from
/// what one object built by this compiler reaches in another, and in nothing but letters, digits
/// and `_`.
///
/// `souther<abi>`, then the module, one `_m_<segment>` per segment of its dotted name, then what is
/// reached under it: `_b_<behavior>`, with `_bind`, `_implement`, `_implementation` or `_answer_case`
/// after it
/// for what is reached of the behavior, `_v_<value>`, `_t_<type>` and the operation —
/// `_construct`, `_f_<field>`, `_case`, `_decode`, `_encode` — or `_l_`, what an element crosses
/// as, and the operation on a list of those ([`host_list_symbol`]). The ABI generation is in it for
/// the reason it is in every other function symbol here.
///
/// A name is written as it is where it is ASCII letters and digits, with `_` doubled and any other
/// character as `_u<hex>_`, its code point in lower-case hexadecimal. So a name reads as itself in
/// the common case, and the spelling holds whatever the language admits in a name, which is
/// Unicode's identifier characters and not a list kept here. What keeps it unambiguous is that
/// inside a name `_` is only ever followed by `_` or `u`, and every mark between names is `_` and
/// a letter that is neither: read from the left, each `_` says which it is. No name is refused, so
/// none of this asserts what [`spells_a_name`] does.
fn host_module(module: &str) -> String {
    let mut spelt = format!("souther{ABI_GENERATION}");
    for segment in module.split('.') {
        spelt.push_str("_m_");
        host_name(&mut spelt, segment);
    }
    spelt
}

/// One name, as [`host_module`] says a name is written.
fn host_name(spelt: &mut String, name: &str) {
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            spelt.push(character);
        } else if character == '_' {
            spelt.push_str("__");
        } else {
            spelt.push_str(&format!("_u{:x}_", u32::from(character)));
        }
    }
}

/// `<module>_<mark>_<name>`, the name written as [`host_module`] says.
fn host_under(module: &str, mark: char, name: &str) -> String {
    let mut spelt = host_module(module);
    spelt.push('_');
    spelt.push(mark);
    spelt.push('_');
    host_name(&mut spelt, name);
    spelt
}

/// Where a host calls a behavior: what it takes, as a host hands each over, and `status + out`.
///
/// A function of its own beside [`behavior_symbol`], which runs it, and not that symbol under a
/// second name: that one is what another object built by this compiler calls, and how it is called
/// is between the two of them. The day it takes a value in a form a host does not hand one over in,
/// this still takes what a host hands over.
pub fn host_behavior_symbol(module: &str, behavior: &str) -> String {
    host_under(module, 'b', behavior)
}

/// Where a host makes the capability of a published behavior constructed from `requirements`:
/// `(into, requirements)`, writing into room for one capability ([`room_for_capability`]) the
/// behavior's code and the requirements as its environment.
///
/// What a host hands where something `depends on` the behavior, the way the JVM hands the instance
/// `bind` made. The requirements are one capability each, in the order the checker answered what
/// constructing the behavior requires, and are read where the behavior runs and not copied here:
/// a host keeps them, and the room, for as long as anything it made from them may be called.
///
/// Here and not left to a host to write out, because the capability holds the address of code, and
/// a host language reaching a C function's address is one that has to be told how its own FFI does
/// that; this answers it once, in the object.
pub fn host_bind_symbol(module: &str, behavior: &str) -> String {
    format!("{}_bind", host_under(module, 'b', behavior))
}

/// Where a host makes the capability of a behavior with no body out of an implementation of its
/// own: `(into, hosted)`, writing into room for one capability ([`room_for_capability`]) the code
/// that calls what `hosted` holds and `hosted` as its environment.
///
/// `hosted` is room a host laid out as [`HOSTED_IMPLEMENTATION`] and [`HOSTED_USERDATA`] say:
/// the function it wrote, of the type [`host_implementation_type`] names, and what that function is
/// handed first each time it is called. A host keeps it, and the function callable, for as long as
/// the capability may be called. The code in the capability is where what an implementation
/// answers is held to [`IMPLEMENTATION_ANSWERS`]; a capability made any other way is not.
pub fn host_implement_symbol(module: &str, behavior: &str) -> String {
    format!("{}_implement", host_under(module, 'b', behavior))
}

/// Where a host asks which case the answer of a behavior is, where the behavior answers a union no
/// declaration names: `(value) -> case`, the case's place among the ones the union descends to,
/// counted from nought, the way [`host_case_symbol`] answers for a sum.
///
/// Under the behavior and not under the union, which has no name to be spelt under and is not one
/// thing a second behavior answering the same members would share: what is asked is what this
/// behavior answered.
pub fn host_behavior_answer_case_symbol(module: &str, behavior: &str) -> String {
    format!("{}_answer_case", host_under(module, 'b', behavior))
}

/// What a host calls the type of the function it implements a behavior with no body as
/// ([`host_implement_symbol`]): what it was handed as [`HOSTED_USERDATA`], then what the behavior
/// takes as a host hands each over, and room for what it answers, as a host is handed it,
/// answering a status. The same words a host calls a published behavior with, the other way round.
///
/// A name in C and not a symbol, since nothing is defined under it: it is what a header calls the
/// pointer, spelt under the behavior the way everything a host reaches of it is.
pub fn host_implementation_type(module: &str, behavior: &str) -> String {
    format!("{}_implementation", host_under(module, 'b', behavior))
}

/// Where a host reads a value a module publishes: nothing taken, and `status + out`, running the
/// entry [`value_symbol`] names for the same reason [`host_behavior_symbol`] runs a behavior.
pub fn host_value_symbol(module: &str, value: &str) -> String {
    host_under(module, 'v', value)
}

/// Where a host builds a value of a declared type: the fields as a host hands them over, and
/// `status + out`, the way a call to the type's own constructor answers — which is what this runs.
///
/// A type with no clause answers a status too, so a clause added to it later is not a change to
/// how a host calls it.
pub fn host_constructor_symbol(module: &str, name: &str) -> String {
    format!("{}_construct", host_under(module, 't', name))
}

/// Where a host reads one field of a value of a declared type, by the name the field is declared
/// under.
///
/// The name and not the position: a field moved within its declaration is still the field a host
/// asked for, and a position would make every reordering a break the linker cannot see.
pub fn host_field_symbol(module: &str, name: &str, field: &str) -> String {
    let mut spelt = host_under(module, 't', name);
    spelt.push_str("_f_");
    host_name(&mut spelt, field);
    spelt
}

/// Where a host asks which of a sum's cases a value is, and is answered with the case's place
/// among them, counted from nought — never with what the value is tagged by, whose address stays
/// inside the objects that compare against it.
pub fn host_case_symbol(module: &str, name: &str) -> String {
    format!("{}_case", host_under(module, 't', name))
}

/// Where a host reads a value of a declared type out of the language's external form: JSON as
/// bytes it holds, `(bytes, length, out) -> status`.
///
/// The status is a Souther computation's, the way every other one is: a clause the reading runs
/// that ends without a value — dividing by nought, leaving an `Int`'s range — answers why, and
/// nothing is written through `out`. Otherwise `out` is handed a reading the host asks through the
/// `DECODED_*` symbols below: a value, the issues found, or where the bytes stopped being JSON. A
/// clause that does not hold is one of the issues and not a status: at the boundary it is what was
/// written, not a computation that could not answer.
pub fn host_decode_symbol(module: &str, name: &str) -> String {
    format!("{}_decode", host_under(module, 't', name))
}

/// Where a host reads a value of a declared type out of a value it built itself, written out with
/// every container as an object keyed as the host keyed it: `(bytes, length, out) -> status`, as
/// [`host_decode_symbol`] in every other respect.
///
/// For a host whose one container is an ordered map, as PHP's array is. To such a host a list is
/// the map keyed by its indices and an empty list is its empty object, so its value cannot say of
/// a container which of the two it is, and text written from it would have to guess. Written with
/// every container as an object, it loses nothing, and the reading takes a map keyed by its
/// indices as an array where the declaration holds one there ([`DECODE_HOST_BEGIN`]).
pub fn host_decode_host_value_symbol(module: &str, name: &str) -> String {
    format!("{}_decode_host", host_under(module, 't', name))
}

/// Where a host writes a value of a declared type in the language's external form: `(value) ->
/// string`, JSON in a string of the runtime's layout, in the arena. Writing a value ends with its
/// form whatever the value is, so this answers no status.
pub fn host_encode_symbol(module: &str, name: &str) -> String {
    format!("{}_encode", host_under(module, 't', name))
}

/// What a host does with a list, through a function the object defines for each way an element
/// crosses ([`host_list_symbol`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HostListOperation {
    /// `(count, the elements as columns) -> list`: a list of `count` elements, the one at an index
    /// being what each column holds at that index. A column is a [`HostParameter::Slice`] of one of
    /// the words the element crosses as, so an element crossing as a presence and a value is two
    /// columns. Building a list ends with the list whatever the elements are, so this answers no
    /// status; a count below nought, or one no room could be taken for, is the host's mistake and
    /// ends the process rather than being read as some other count.
    Construct,
    /// `(list) -> count`: how many elements the list holds.
    Length,
    /// `(list, index, room for each word the element crosses as) -> bool`: one where the index is
    /// inside the list, with the element written through the room as a field of its type is
    /// handed over, and nought where it is outside it, with nothing written.
    At,
}

/// Where a host builds or reads a list whose elements cross as `element`, and as a presence beside
/// it where `present`: `_l_`, then `present_` where they do, the word as [`HostWord::spelt`] spells
/// it, and the operation.
///
/// Under what an element crosses as and not under the element's type: a list of one declared type
/// and a list of another are both a list of addresses to the functions here, which put an element
/// in its slot and take one out without knowing what it is. Under a module all the same, for
/// the reason every other function a host reaches is: two builds' objects linked into one library
/// each define their own, and a symbol under no module would be defined twice. Which module's a
/// host calls makes no difference to the list it is handed.
pub fn host_list_symbol(
    module: &str,
    present: bool,
    element: HostWord,
    operation: HostListOperation,
) -> String {
    let mut spelt = host_module(module);
    spelt.push_str("_l_");
    if present {
        spelt.push_str("present_");
    }
    spelt.push_str(element.spelt());
    spelt.push_str(match operation {
        HostListOperation::Construct => "_construct",
        HostListOperation::Length => "_length",
        HostListOperation::At => "_at",
    });
    spelt
}

/// The symbol a value of a declared type is read out of a document through, by another object this
/// compiler built: `(node, path, reading, out) -> status`, the three pointers being the runtime's
/// and never looked behind.
///
/// Defined by the object of the build that declared the type, beside its constructor and its token,
/// whatever the type is: how a declaration is read is the declaring build's, and every other build
/// reaching a value of it in a document calls this rather than reading one itself. For a type built
/// from fields it is also the one place that can say which clause did not hold, which the
/// constructor's status does not.
///
/// Nothing is written through `out` unless the status is `ANSWERED`, and then what is written is
/// the value, or nothing ([`NOTHING`]) where what stands there is not one and the reading was told
/// why.
///
/// # Panics
///
/// Where either name does not stand in a symbol.
pub fn reader_symbol(module: &str, name: &str) -> String {
    assert!(
        spells_a_module(module),
        "a module's name carries no dollar, and the symbol is split on one: {module}"
    );
    assert!(
        spells_a_name(name),
        "a declared type's name carries neither dollar nor dot, and the symbol is split on \
         both: {name}"
    );
    format!("souther{ABI_GENERATION}.{module}$read${name}")
}

/// The symbol the object carries for one of a behavior's `example` rows.
///
/// A row states the values to hand over, so what stands under this name takes nothing: the values
/// are written into it. That is what makes it reachable from outside whatever the module says
/// about the behavior's own name — running a row is not reaching the behavior, and a row of a name
/// a module keeps is as much a row as any other.
///
/// The `$` is what keeps the spelling unambiguous, as it is for a definition a module holds: a
/// module's name carries dots and a behavior's carries none, and neither carries this.
///
/// # Panics
///
/// Where the behavior's name carries a dot, for the reason [`behavior_symbol`] gives.
pub fn example_symbol(module: &str, behavior: &str, at: usize) -> String {
    format!("{}$example${at}", behavior_symbol(module, behavior))
}

/// The symbol whose address a value of a declared type carries as its tag.
///
/// One data object per declaration. Two objects naming one declaration reach one address because
/// the linker resolved one name, which is the property a number counted in a document does not
/// have: two documents number their declarations differently, and a value tagged with one object's
/// count compared against another's is two answers to a question neither was asked.
///
/// Not the whole of what a declared type's identity is. That is its module and its name, which
/// every declaration has and which the document carries; this is the one representation of it a
/// value can hold in a slot, and it exists for values to be told apart by.
///
/// So the agreement is not between two builds. It is between each build and the linker, which is
/// already what makes a call reach a definition.
///
/// `souther$type$<module>$<name>`. The dollar after `souther` is what keeps this out of the way of
/// a behavior's symbol, which starts `souther<abi>.`; the one before the name is what tells the
/// two segments apart, since a module's name carries dots and a type's carries none.
///
/// # Panics
///
/// Where either name carries a dollar, or the type's name carries a dot, since the spelling is
/// read on both.
pub fn type_symbol(module: &str, name: &str) -> String {
    assert!(
        spells_a_module(module),
        "a module's name carries no dollar, and the symbol is split on one: {module}"
    );
    assert!(
        spells_a_name(name),
        "a declared type's name carries neither dollar nor dot, and the symbol is split on \
         both: {name}"
    );
    format!("souther$type${module}${name}")
}

/// Where a host reaches an entry the object answers for — a published behavior, or a row — to be
/// handed its answer as the external form the language writes it in, rather than as a value laid
/// out the way this backend lays one out.
///
/// The same parameters as the entry it runs, and room for one string in place of room for the
/// answer: the JSON text, in the arena. The status is the entry's own, and nothing is written
/// through the room unless it is `ANSWERED`.
pub fn boundary_symbol(entry: &str) -> String {
    format!("{entry}$boundary")
}

/// What stands under a declared type's symbol.
///
/// One byte, whose value means nothing and which nothing ever reads. What the token is for is its
/// address, and a byte is what gives it one of its own: two symbols with no bytes between them may
/// be laid at one address, and two declarations would then be tagged the same way.
///
/// A case no declaration names stands under a [`BUILT_IN_CASES`] symbol, which is the same byte.
pub const TOKEN: &[u8] = &[0];

/// Every case no declaration names that a value can say it is: the primitives a value of a union
/// can be, and the cases the language gives. The name is what the language writes the case as.
///
/// A primitive says nothing about which type it is, and a case the language gives is declared by
/// no module, so neither has a declared type's token to carry. The runtime defines one for each of
/// these, under [`built_in_case_symbol`], and a value of a union carries its address the way a
/// value of a declared type carries its declaration's.
///
/// The runtime's and not an object's, because the runtime is the one thing every object in a
/// library shares: two objects naming one of these reach one address for the reason two naming one
/// declaration do, and no object is where a case the language gives is at home.
///
/// `Some` and `None` among them. An optional says whether it holds something by whether it is a
/// null pointer ([`NOTHING`]) and never carries either token: which case a value is and how it is
/// held are two questions, and an optional answers the second without a token. A union naming one
/// of the two as a case is held the way it holds any case the language gives, by the token alone.
pub const BUILT_IN_CASES: &[&str] = &[
    "Int",
    "Bool",
    "String",
    "Some",
    "None",
    "DivisionByZero",
    "NotANumber",
    "NotADate",
    "NotATime",
    "NotWhole",
    "NotAFiniteDecimal",
];

/// The symbol whose address a value of a case in [`BUILT_IN_CASES`] carries as its tag.
///
/// `souther$case$<name>`, apart from [`type_symbol`]'s `souther$type$` so that no declaration a
/// module makes can be spelt as one of these.
///
/// # Panics
///
/// Where the name is not one of [`BUILT_IN_CASES`]: a symbol for any other would name something
/// the runtime does not define, and the link would say so far from here.
pub fn built_in_case_symbol(name: &str) -> String {
    assert!(
        BUILT_IN_CASES.contains(&name),
        "{name} is not a case the runtime defines a token for"
    );
    format!("souther$case${name}")
}

/// How wide a slot is, and so what a value made of slots is measured in.
///
/// One width for every slot, whatever it holds. A layout that packed a `Bool` into a byte would
/// make a field's offset depend on the types of the fields before it, which is a computation both
/// sides would have to agree on for every declaration rather than a multiplication either can do.
pub const SLOT: i64 = 8;

/// Where a value of a declared type says which type it is.
///
/// The address of the declaration's token, which [`type_symbol`] names. Every constructed value
/// carries it, including one of a type no sum has a case for. A value's own type is what it is,
/// not what it is being read as, so writing it only where someone was going to match on it would
/// make the representation depend on a use rather than on the value — and a value built in one
/// behavior and matched in another has no such use to read.
///
/// One slot, as it was when it held a count. What the slot means is what moved: from where the
/// declaration stood among the ones one document brought, to what the linker resolved its name to.
pub const WHICH: i64 = 0;

/// Where a constructed value's first field is. Its fields follow in declaration order.
pub const FIRST_FIELD: i64 = SLOT;

/// The offset of a field of a declared type, by its position in the declaration.
pub const fn field_at(position: usize) -> i64 {
    FIRST_FIELD + SLOT * position as i64
}

/// The offset of a tuple's member. A tuple says which type it is nowhere: it is not a declared
/// type and nothing matches on one, so its members start where they are.
pub const fn member_at(position: usize) -> i64 {
    SLOT * position as i64
}

/// Where a value of a primitive case is, in what carries it as a value of a union.
///
/// A primitive carries no tag, so where a union holds one it holds the address of room laid out as
/// a value with one field: [`WHICH`] is the primitive's [`built_in_case_symbol`] token and this is
/// the primitive. A case the language gives has nothing in it, and is carried as a value with no
/// fields. Either way what stands at [`WHICH`] says which case the value is, as it does for a value
/// of a declared type, so a test of which case a value is reads one slot whatever the case is.
pub const CARRIED: i64 = field_at(0);

/// How much room what carries a primitive as a value of a union takes.
pub const fn room_for_carried() -> i64 {
    room_for_fields(1)
}

/// How much room a value of a declared type takes, by how many fields it has.
///
/// Here and not worked out by whoever takes the room, which is the whole point of this crate: an
/// offset and the room it has to fall inside are one fact, and a caller that added up slots for
/// itself would be holding a copy of half of it. The two have gone out of step once already.
pub const fn room_for_fields(fields: usize) -> i64 {
    FIRST_FIELD + SLOT * fields as i64
}

/// How much room a tuple of this many members takes.
pub const fn room_for_members(members: usize) -> i64 {
    member_at(members)
}

/// Where a list says how many elements it holds.
///
/// A list is its length and then its elements, one slot each and in order, through the same slot
/// every value is held in. Nothing else stands in it. No capacity, since a list is never grown in
/// place; no element type, since that is the static type's; no element width, since every element
/// is one slot. A `Set` and a `Map` are not laid out as this: how they hold their members is a
/// question the language has not settled, and a header shared with them would be answering it.
///
/// The empty list is a length of nought and no elements, never a null pointer, which is what an
/// `Option` holding nothing already is.
pub const LIST_LENGTH: i64 = 0;

/// Where a list's first element is.
pub const LIST_ELEMENTS: i64 = SLOT;

/// The offset of a list's element, by its index.
pub const fn list_at(index: i64) -> i64 {
    LIST_ELEMENTS + SLOT * index
}

/// How much room a list of this many elements takes.
pub const fn room_for_list(elements: i64) -> i64 {
    list_at(elements)
}

/// Where a capability holds its code: the function a call through it reaches, taking
/// [`CAPABILITY_ENVIRONMENT`] first and then what the behavior takes, and answering `status + out`.
///
/// A capability is what a behavior is handed for each behavior it `depends on`: the JVM's instance
/// of the behavior's interface, as the code that answers it and what that code reads. What stands
/// behind it is the capability's own and not the caller's to know: a body the object compiled, with
/// the capabilities it was constructed with as its environment; an implementation a host wrote; a
/// row's stand-in. So a call through one never recovers its code from the behavior it calls, which
/// is what lets any of those stand where the behavior is required.
///
/// Where a behavior with a body is itself called, its environment is the requirements it was
/// constructed with: an address of as many addresses of capabilities as it requires, one slot
/// each, in the order the checker answered them ([`requirement_at`]). A behavior requiring nothing
/// is handed a null one and never reads it.
pub const CAPABILITY_INVOKE: i64 = 0;

/// Where a capability holds what its code is handed first.
pub const CAPABILITY_ENVIRONMENT: i64 = SLOT;

/// How much room a capability takes.
pub const fn room_for_capability() -> i64 {
    CAPABILITY_ENVIRONMENT + SLOT
}

/// The offset of the address of the capability for a requirement, among the requirements a
/// behavior was constructed with, by where the checker answered it.
pub const fn requirement_at(position: usize) -> i64 {
    SLOT * position as i64
}

/// How much room the requirements of a behavior constructed with this many take.
pub const fn room_for_requirements(requirements: usize) -> i64 {
    requirement_at(requirements)
}

/// Where what a host lays out for an implementation of its own holds the function it wrote
/// ([`host_implement_symbol`]).
pub const HOSTED_IMPLEMENTATION: i64 = 0;

/// Where what a host lays out for an implementation of its own holds what the function is handed
/// first, which nothing but the function reads.
pub const HOSTED_USERDATA: i64 = SLOT;

/// How much room a host lays out for an implementation of its own.
pub const fn room_for_hosted() -> i64 {
    HOSTED_USERDATA + SLOT
}

/// How much room an `Option` holding a value takes.
pub const fn room_for_held() -> i64 {
    HELD + SLOT
}

/// How much room a string carrying this many bytes of text takes.
pub const fn room_for_text(bytes: i64) -> i64 {
    TEXT_BYTES + bytes
}

/// That every offset falls inside the room its value is given.
///
/// Held here rather than by a test, because both sides of each of these are constants and a test
/// could only fail after one of them had already been changed. What it stops is a layout moved at
/// one of the two places it is read: an offset that moved past the room would be a value written
/// off the end of what was taken for it, which is not something the run would report.
const _: () = {
    let mut fields = 0;
    while fields < 16 {
        assert!(field_at(fields) + SLOT <= room_for_fields(fields + 1));
        assert!(member_at(fields) + SLOT <= room_for_members(fields + 1));
        assert!(list_at(fields as i64) + SLOT <= room_for_list(fields as i64 + 1));
        fields += 1;
    }
    assert!(WHICH + SLOT <= room_for_fields(0));
    assert!(CARRIED + SLOT <= room_for_carried());
    assert!(HELD + SLOT <= room_for_held());
    assert!(LIST_LENGTH + SLOT <= room_for_list(0));
    assert!(TEXT_LENGTH + SLOT <= room_for_text(0));
};

/// That a string's count of bytes is as wide as a slot.
///
/// Both halves write and read it as an `i64`, which is this and not something either of them
/// decided. Were a slot to be made wider, that is two readings to change and a build that stops
/// until they are.
const _: () = assert!(SLOT as usize == size_of::<i64>());

/// What an `Option` holding nothing is.
///
/// A null pointer, which no allocation answers, so the two are told apart by what the pointer is
/// rather than by a slot beside it. An `Option` holding a value is a pointer to a slot holding it,
/// and a slot even where the value would fit in a pointer, because whether it fits is a fact about
/// one type and an `Option` is one representation over every type.
///
/// A slot that stays as it is for as long as the run does, and not one taken for the `Option`
/// alone. Nothing a run holds is changed once it is written, so any slot holding a `T` is one an
/// `Option<T>` may point at: an element of a list is, and `list.get` answers the element's own
/// slot rather than a copy of it. A reader of an `Option` reads [`HELD`] through the pointer and
/// asks nothing about where the slot stands.
pub const NOTHING: i64 = 0;

/// Where an `Option`'s value is, once it is known to be holding one.
pub const HELD: i64 = 0;

/// Where a string says how many bytes of text it carries.
///
/// A count of bytes, not of code points. What the language counts a string in is code points — a
/// length, an index and a range all do — and nothing here answers any of those: they are reached
/// through a kernel. What this side needs is where the text ends, which is what a comparison and a
/// join read, and a second count would be room spent on a question nothing yet asks.
///
/// Nought, as a declared type's [`WHICH`] is, and the two are not one fact. A string is not a
/// declared type and nothing matches on one, so it carries no tag; what stands first is the only
/// thing standing before the text.
pub const TEXT_LENGTH: i64 = 0;

/// Where a string's text begins, as UTF-8 and in no other encoding.
///
/// One slot along, so the text starts aligned as everything the arena answers does, and so what
/// stands before it is read as a slot like any other.
pub const TEXT_BYTES: i64 = SLOT;

/// The symbol two strings are compared through.
///
/// Answers a number below, at or above nought, as the left one comes before, at, or after the
/// right one. One symbol for all six comparisons: the six differ in what they do with the answer
/// and not in what they ask, and a symbol each would be six chances to order text six ways.
pub const STRING_COMPARE: &str = "souther_string_compare";

/// The symbol two strings are joined through. Answers a new string and touches neither operand.
pub const STRING_CONCAT: &str = "souther_string_concat";

/// The symbol a string's length is counted through, in code points, which is what the language
/// counts a string in.
///
/// Not [`STRING_LENGTH`], which answers the bytes [`TEXT_LENGTH`] holds and is what a caller outside
/// a Souther program reads the text back by. The two agree only on ASCII.
pub const STRING_CODE_POINTS: &str = "souther_string_code_points";

/// The symbols the `String` module's kernels are computed through (spec §stdlib-string), one for
/// each, taking what the kernel takes in the order it takes it.
///
/// The text is walked in the runtime rather than in code emitted at every call, for the reason
/// [`STRING_COMPARE`] is. A kernel that answers a value for everything it is handed answers it. One
/// that answers nothing for some of what it is handed — a slice the string has no room for, a count
/// no string could hold, text that is no integer — answers whether it wrote its value through room
/// it is handed last, and says nothing of why: which reason a run ends for, or which case stands
/// in for the value, is the kernel's contract and the caller's to read, never the runtime's.
pub const STRING_TRIM: &str = "souther_string_trim";
/// `String.lowercase`.
pub const STRING_LOWERCASE: &str = "souther_string_lowercase";
/// `String.uppercase`.
pub const STRING_UPPERCASE: &str = "souther_string_uppercase";
/// `String.contains`.
pub const STRING_CONTAINS: &str = "souther_string_contains";
/// `String.startsWith`.
pub const STRING_STARTS_WITH: &str = "souther_string_starts_with";
/// `String.endsWith`.
pub const STRING_ENDS_WITH: &str = "souther_string_ends_with";
/// `String.matches`, handed the machine the pattern was compiled to in place of the pattern.
pub const STRING_MATCHES: &str = "souther_string_matches";
/// `String.slice`, into room for the string.
pub const STRING_SLICE: &str = "souther_string_slice";
/// `String.split`.
pub const STRING_SPLIT: &str = "souther_string_split";
/// `String.join`.
pub const STRING_JOIN: &str = "souther_string_join";
/// `String.concat`.
pub const STRING_CONCAT_ALL: &str = "souther_string_concat_all";
/// `String.replace`.
pub const STRING_REPLACE: &str = "souther_string_replace";
/// `String.words`.
pub const STRING_WORDS: &str = "souther_string_words";
/// `String.lines`.
pub const STRING_LINES: &str = "souther_string_lines";
/// `String.fromInt`.
pub const STRING_FROM_INT: &str = "souther_string_from_int";
/// `String.toInt`, into room for the `Int`.
pub const STRING_TO_INT: &str = "souther_string_to_int";
/// `String.reverse`.
pub const STRING_REVERSE: &str = "souther_string_reverse";
/// `String.repeat`, into room for the string.
pub const STRING_REPEAT: &str = "souther_string_repeat";
/// `String.padLeft`, into room for the string.
pub const STRING_PAD_LEFT: &str = "souther_string_pad_left";
/// `String.padRight`, into room for the string.
pub const STRING_PAD_RIGHT: &str = "souther_string_pad_right";
/// `String.characters`.
pub const STRING_CHARACTERS: &str = "souther_string_characters";
/// `String.codePoints`: the code points as a list, where [`STRING_CODE_POINTS`] counts them.
pub const STRING_CODE_POINT_VALUES: &str = "souther_string_code_point_values";

/// The symbol a caller outside a Souther program makes a string with, from bytes it holds.
///
/// Here rather than left to whoever writes such a caller, for the reason [`MARK`] is: the layout
/// above is between this crate and the runtime, and a caller that built a string from it would be
/// a third party to a two-party contract.
pub const STRING_OF_UTF8: &str = "souther_string_of_utf8";

/// The symbols such a caller reads a string back through: first how many bytes it holds.
pub const STRING_LENGTH: &str = "souther_string_length";
/// And then where those bytes start.
pub const STRING_BYTES: &str = "souther_string_bytes";

/// The symbol generated code takes room from.
///
/// It answers a pointer to `size` bytes that stay valid until the mark below them is reset. A
/// Souther value is never freed on its own: what a run makes is dropped in one go by the caller
/// that bracketed the call, so nothing generated has to know what owns what.
pub const ALLOCATE: &str = "souther_alloc";

/// The symbol a caller reads the arena's position from, to reset to afterwards.
pub const MARK: &str = "souther_mark";

/// The symbol a caller gives a mark back to, dropping everything taken since.
pub const RESET: &str = "souther_reset";

/// The runtime's external form: what a value is written as at a boundary, built as a tree by
/// generated code and written out as JSON in one step.
///
/// The tree is the runtime's own and lives on its heap, not in the arena. Every constructor hands
/// the caller a form it owns; `EXTERNAL_APPEND` and `EXTERNAL_PUT` take ownership of the item they
/// are given and leave the container with the caller; `EXTERNAL_JSON` takes the root, drops the
/// whole tree, and answers a string of the runtime's own layout (`TEXT_LENGTH`, `TEXT_BYTES`) in
/// the arena. So nothing of the tree outlives the call that writes it, and what `RESET` drops is
/// only what it always dropped.
///
/// A key and a string handed in are strings of that same layout, not NUL-terminated text: a key a
/// compile writes is a literal in the object, and a string a run worked out is in the arena.
pub const EXTERNAL_NULL: &str = "souther_external_null";
/// `(i8) -> form`: any value but 0 is true.
pub const EXTERNAL_BOOL: &str = "souther_external_bool";
/// `(i64) -> form`.
pub const EXTERNAL_INT: &str = "souther_external_int";
/// `(string) -> form`, the bytes copied.
pub const EXTERNAL_STRING: &str = "souther_external_string";
/// `() -> form`, an array with nothing in it.
pub const EXTERNAL_ARRAY: &str = "souther_external_array";
/// `(array, item)`: the item is appended and owned by the array from then on.
pub const EXTERNAL_APPEND: &str = "souther_external_append";
/// `() -> form`, an object with nothing in it.
pub const EXTERNAL_OBJECT: &str = "souther_external_object";
/// `(object, key string, item)`: the member is placed after those already there, and the item
/// is owned by the object from then on.
///
/// `object` must be an object and `EXTERNAL_APPEND`'s `array` an array; handed any other kind of
/// form, the runtime aborts the process rather than guess. Generated code only ever puts into an
/// object the same function made, so reaching that is a caller outside this contract.
pub const EXTERNAL_PUT: &str = "souther_external_put";
/// `(form) -> string`: the whole tree written as JSON, and dropped.
pub const EXTERNAL_JSON: &str = "souther_external_json";

/// Reading a document, from its bytes to what a host is answered. Generated code begins one with
/// the bytes, `(bytes, length) -> reading`; asks for its root, `(reading) -> node`, which is null
/// where the bytes were not JSON; and ends it with what it read, `(reading, value)`, the value null
/// where it read none. A reading whose clauses ended without a value is abandoned, `(reading)`, and
/// not answered. The reading, its issues and every string they hold are taken from the arena; the
/// document is not, and ending or abandoning a reading is what drops it.
pub const DECODE_BEGIN: &str = "souther_decode_begin";
/// `(bytes, length) -> reading`, as [`DECODE_BEGIN`], for a value a host built out of ordered
/// maps and wrote with every container as an object ([`host_decode_host_value_symbol`]): a map
/// keyed by its indices is read as an array wherever the reader takes one.
pub const DECODE_HOST_BEGIN: &str = "souther_decode_host_begin";
/// `(reading) -> node`.
pub const DECODE_ROOT: &str = "souther_decode_root";
/// `(reading, value)`.
pub const DECODE_END: &str = "souther_decode_end";
/// `(reading)`.
pub const DECODE_ABANDON: &str = "souther_decode_abandon";

/// What a generated reader asks of one place in a document, and records where it is not what the
/// declaration says. `path` is the place's, made with `PATH_BELOW` from the root, which is null.
/// `(path, step string) -> path`.
pub const PATH_BELOW: &str = "souther_path_below";
/// `(path, i64) -> path`: the place of an array's element, by its index.
pub const PATH_AT: &str = "souther_path_at";
/// `(node, path, reading) -> i8`: whether it is an object.
pub const READ_OBJECT: &str = "souther_read_object";
/// `(node, path, reading) -> i8`: whether it is an array.
pub const READ_ARRAY: &str = "souther_read_array";
/// `(node) -> i64`: how many elements an array holds, asked of one `READ_ARRAY` said is one.
pub const READ_ARRAY_LENGTH: &str = "souther_read_array_length";
/// `(node, i64) -> node`: an array's element at an index below its length.
pub const READ_ELEMENT: &str = "souther_read_element";
/// `(node, key string) -> node`: an object's member, null where there is none.
pub const READ_MEMBER: &str = "souther_read_member";
/// `(path, reading)`: a field every value has was not written.
pub const READ_MISSING: &str = "souther_read_missing";
/// `(node) -> i8`: whether it is `null`.
pub const READ_NULL: &str = "souther_read_null";
/// `(node, path, reading, out) -> i8`: an `Int` written through `out`.
///
/// A scalar reader writes `out` whatever it answers: the value where it read one, and nought, or
/// null for text, where it did not and recorded why. So a caller's room holds something the reader
/// wrote after every call, the same as a type's reader, which writes its value or nothing whenever
/// it answers `ANSWERED`.
pub const READ_INT: &str = "souther_read_int";
/// `(node, path, reading, out) -> i8`: a `Bool` written through `out` as one byte.
pub const READ_BOOL: &str = "souther_read_bool";
/// `(node, path, reading, out) -> i8`: a string of this crate's layout written through `out`.
pub const READ_STRING: &str = "souther_read_string";
/// `(node, path, reading) -> i8`: whether it is text naming a case.
pub const READ_CASE: &str = "souther_read_case";
/// `(node, key string, path, reading) -> node`: the text an object names its case with under a
/// key, null where it names none.
pub const READ_TAG: &str = "souther_read_tag";
/// `(node, name string) -> i8`: whether the text is that name.
pub const READ_IS: &str = "souther_read_is";
/// `(node, path, reading)`: the text names no case there is.
pub const READ_NOT_A_CASE: &str = "souther_read_not_a_case";
/// `(path, reading, module string, name string, clause string)`: a value read there breaks a
/// clause, the clause's name null where it has none.
pub const READ_INVARIANT: &str = "souther_read_invariant";

/// What a host asks a reading once a decoder has answered it. `(reading) -> i32`, one of the three
/// below.
pub const DECODED_OUTCOME: &str = "souther_decoded_outcome";
/// The document was read as a value, which `DECODED_VALUE_OF` answers.
pub const DECODED_VALUE: i32 = 0;
/// The document is JSON and not a value of the type, and the issues say why.
pub const DECODED_ISSUES: i32 = 1;
/// The bytes are not JSON, and `DECODED_MALFORMED_AT` says where they stopped being it.
pub const DECODED_MALFORMED: i32 = 2;
/// That a reading's outcomes are three numbers and not fewer: a host tells them apart by the
/// number, so two outcomes answered alike would be one it could not read.
const _: () = assert!(
    DECODED_VALUE != DECODED_ISSUES
        && DECODED_ISSUES != DECODED_MALFORMED
        && DECODED_VALUE != DECODED_MALFORMED
);
/// `(reading) -> value`.
pub const DECODED_VALUE_OF: &str = "souther_decoded_value";
/// `(reading) -> i64`: the offset of the first byte that could not be read.
pub const DECODED_MALFORMED_AT: &str = "souther_decoded_malformed_at";
/// `(reading) -> i64`.
pub const DECODED_ISSUE_COUNT: &str = "souther_decoded_issue_count";
/// `(reading, i64) -> issue`, in the order they were found.
pub const DECODED_ISSUE: &str = "souther_decoded_issue";
/// `(issue) -> string`: one of Raoh's codes.
pub const ISSUE_CODE: &str = "souther_issue_code";
/// `(issue) -> string`: a JSON Pointer, empty for the document's root.
pub const ISSUE_PATH: &str = "souther_issue_path";
/// `(issue) -> i64`: how many named entries it carries, which is what Raoh calls its metadata.
pub const ISSUE_META_COUNT: &str = "souther_issue_meta_count";
/// `(issue, i64) -> string`: an entry's name.
pub const ISSUE_META_KEY: &str = "souther_issue_meta_key";
/// `(issue, i64) -> string`: what an entry says.
pub const ISSUE_META_VALUE: &str = "souther_issue_meta_value";

/// What a host hands over and is handed, one word at a time, as a C declaration says it.
///
/// A vocabulary and not a C type: what each word is called in a header, and what it is on the
/// machine, are read off this by whoever writes the header and whoever emits the call, so the two
/// are one fact. Each is a kind of thing a host holds, and two kinds one word wide are two words
/// here all the same — a value and a string are both an address, and a host handed the one where
/// the other was meant has been handed something else.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HostWord {
    /// What a generated function answers in place of its value ([`Status`]).
    Status,
    /// An `Int`: sixty-four bits, signed.
    Int,
    /// A `Bool`, or whether an optional holds a value: one byte, nought or one.
    Bool,
    /// Which of a sum's cases a value is, as its place among them.
    Case,
    /// What a reading came to: one of the `DECODED_*` numbers.
    Outcome,
    /// How many of something there are, or where one stands among them: sixty-four bits.
    Count,
    /// Where the arena stood, to be given back to [`RESET`].
    Mark,
    /// Bytes the host holds, read and never kept.
    Bytes,
    /// The address of a value of a declared type, which a host never reads behind.
    Value,
    /// The address of text of the runtime's layout, read through [`STRING_LENGTH`] and
    /// [`STRING_BYTES`].
    String,
    /// A reading a decoder answered, asked through the `DECODED_*` functions.
    Decoded,
    /// One issue a reading found, asked through the `ISSUE_*` functions.
    Issue,
    /// The address of a list, which a host never reads behind and reaches through the functions
    /// [`host_list_symbol`] names. A word of its own and not a [`HostWord::Value`]: a host handed a
    /// list where a value of a declared type was meant has been handed something else.
    List,
    /// The capabilities a behavior is constructed with: the address of as many addresses of
    /// capabilities as it requires, in the order the checker answered them, or null where it
    /// requires nothing ([`CAPABILITY_INVOKE`]). Read for as long as anything made from them may
    /// be called, and never written.
    Requirements,
    /// One capability, which a host lays out as room of [`room_for_capability`] bytes and never
    /// reads behind: where one is written, and, as the address of one, what a host hands over in
    /// [`HostWord::Requirements`].
    Capability,
    /// What a host's own implementation is handed first each time it is called, which nothing but
    /// the implementation reads ([`host_implement_symbol`]).
    Userdata,
}

impl HostWord {
    /// The word as a symbol and a manifest spell it: its name, in lower case.
    pub const fn spelt(self) -> &'static str {
        match self {
            HostWord::Status => "status",
            HostWord::Int => "int",
            HostWord::Bool => "bool",
            HostWord::Case => "case",
            HostWord::Outcome => "outcome",
            HostWord::Count => "count",
            HostWord::Mark => "mark",
            HostWord::Bytes => "bytes",
            HostWord::Value => "value",
            HostWord::String => "string",
            HostWord::Decoded => "decoded",
            HostWord::Issue => "issue",
            HostWord::List => "list",
            HostWord::Requirements => "requirements",
            HostWord::Capability => "capability",
            HostWord::Userdata => "userdata",
        }
    }
}

/// One parameter of a function a host calls: a word handed over, or room the function writes one
/// through.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HostParameter {
    /// The word itself.
    Given(HostWord),
    /// The address of room for one, written only where the function says it writes it.
    Room(HostWord),
    /// The address of as many of the word, one after another, as another parameter counts: read
    /// for the length of the call and not kept.
    Slice(HostWord),
}

/// A function of the runtime's that a host calls, with what it takes and answers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RuntimeFunction {
    /// Its symbol, which is also its name in C.
    pub name: &'static str,
    /// What it takes, in order.
    pub takes: &'static [HostParameter],
    /// What it answers, where it answers anything.
    pub answers: Option<HostWord>,
}

/// Every function of the runtime's a host calls, and nothing else of the runtime's.
///
/// What generated code calls — taking room, comparing and joining text, building external form,
/// reading a document — is between generated code and the runtime, and a host that called it would
/// be a third party to that. So a header declares what is here and a shared library exports it,
/// and neither says anything of the rest. The runtime's own tests hold each of these to the
/// function it names.
pub const HOST_RUNTIME: &[RuntimeFunction] = {
    use HostParameter::Given;
    use HostWord::{Bytes, Count, Decoded, Issue, Mark, Outcome, String, Value};
    &[
        RuntimeFunction {
            name: MARK,
            takes: &[],
            answers: Some(Mark),
        },
        RuntimeFunction {
            name: RESET,
            takes: &[Given(Mark)],
            answers: None,
        },
        RuntimeFunction {
            name: STRING_OF_UTF8,
            takes: &[Given(Bytes), Given(Count)],
            answers: Some(String),
        },
        RuntimeFunction {
            name: STRING_LENGTH,
            takes: &[Given(String)],
            answers: Some(Count),
        },
        RuntimeFunction {
            name: STRING_BYTES,
            takes: &[Given(String)],
            answers: Some(Bytes),
        },
        RuntimeFunction {
            name: DECODED_OUTCOME,
            takes: &[Given(Decoded)],
            answers: Some(Outcome),
        },
        RuntimeFunction {
            name: DECODED_VALUE_OF,
            takes: &[Given(Decoded)],
            answers: Some(Value),
        },
        RuntimeFunction {
            name: DECODED_MALFORMED_AT,
            takes: &[Given(Decoded)],
            answers: Some(Count),
        },
        RuntimeFunction {
            name: DECODED_ISSUE_COUNT,
            takes: &[Given(Decoded)],
            answers: Some(Count),
        },
        RuntimeFunction {
            name: DECODED_ISSUE,
            takes: &[Given(Decoded), Given(Count)],
            answers: Some(Issue),
        },
        RuntimeFunction {
            name: ISSUE_CODE,
            takes: &[Given(Issue)],
            answers: Some(String),
        },
        RuntimeFunction {
            name: ISSUE_PATH,
            takes: &[Given(Issue)],
            answers: Some(String),
        },
        RuntimeFunction {
            name: ISSUE_META_COUNT,
            takes: &[Given(Issue)],
            answers: Some(Count),
        },
        RuntimeFunction {
            name: ISSUE_META_KEY,
            takes: &[Given(Issue), Given(Count)],
            answers: Some(String),
        },
        RuntimeFunction {
            name: ISSUE_META_VALUE,
            takes: &[Given(Issue), Given(Count)],
            answers: Some(String),
        },
    ]
};

/// How a host makes a value of one case in [`BUILT_IN_CASES`], as a union holds one, and reads back
/// what it holds.
///
/// A property of the case and not of any union it stands in: an `Int` carried is laid out the same
/// in every union that has it as a case, so there is one pair of functions for it and not one for
/// each union. What tells a union's cases apart is the union's ([`host_behavior_answer_case_symbol`]);
/// once that has said which case a value is, this is what a host reads it through, without being
/// told where anything is kept.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CaseCrossing {
    /// The case, as [`BUILT_IN_CASES`] names it.
    pub case: &'static str,
    /// `(what the case holds, where it holds something) -> value`.
    pub make: RuntimeFunction,
    /// `(value) -> what it holds`, for a case that holds something. Called only on a value a test
    /// of which case it is has said is this case, and nothing is asked of it again.
    pub read: Option<RuntimeFunction>,
}

/// How a host makes and reads each case in [`BUILT_IN_CASES`], in the order that names them. The
/// runtime's own tests hold each function to the one it names, and the cases to that table.
pub const HOST_CASES: &[CaseCrossing] = {
    use HostParameter::Given;
    use HostWord::{Bool, Int, String, Value};
    const fn holding(
        case: &'static str,
        make: &'static str,
        read: &'static str,
        held: &'static [HostParameter],
        word: HostWord,
    ) -> CaseCrossing {
        CaseCrossing {
            case,
            make: RuntimeFunction {
                name: make,
                takes: held,
                answers: Some(Value),
            },
            read: Some(RuntimeFunction {
                name: read,
                takes: &[Given(Value)],
                answers: Some(word),
            }),
        }
    }
    const fn empty(case: &'static str, make: &'static str) -> CaseCrossing {
        CaseCrossing {
            case,
            make: RuntimeFunction {
                name: make,
                takes: &[],
                answers: Some(Value),
            },
            read: None,
        }
    }
    &[
        holding(
            "Int",
            "souther_case_int_make",
            "souther_case_int_read",
            &[Given(Int)],
            Int,
        ),
        holding(
            "Bool",
            "souther_case_bool_make",
            "souther_case_bool_read",
            &[Given(Bool)],
            Bool,
        ),
        holding(
            "String",
            "souther_case_string_make",
            "souther_case_string_read",
            &[Given(String)],
            String,
        ),
        empty("Some", "souther_case_some_make"),
        empty("None", "souther_case_none_make"),
        empty("DivisionByZero", "souther_case_division_by_zero_make"),
        empty("NotANumber", "souther_case_not_a_number_make"),
        empty("NotADate", "souther_case_not_a_date_make"),
        empty("NotATime", "souther_case_not_a_time_make"),
        empty("NotWhole", "souther_case_not_whole_make"),
        empty(
            "NotAFiniteDecimal",
            "souther_case_not_a_finite_decimal_make",
        ),
    ]
};

/// What generated code hands the runtime and is handed back: every word a host is ([`HostWord`]),
/// and the ones that stay between generated code and the runtime.
///
/// Each is a kind of thing and not a width, for the reason [`HostWord`] is: two kinds one word wide
/// are two words here, so a function said to take one and written to take the other is caught.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Word {
    /// One a host is handed or hands over too.
    Host(HostWord),
    /// Room taken from the arena for generated code to write into.
    Memory,
    /// Which of two strings comes first: below, at or above nought.
    Comparison,
    /// A pattern's machine: the words `souther_text::pattern` compiled it to, which the object
    /// carries and the runtime runs, the first of them saying how many there are.
    Machine,
    /// A piece of the external form being built, owned by whoever [`EXTERNAL_NULL`] and the rest
    /// say.
    Form,
    /// A place in a document being read.
    Node,
    /// Where a place in a document is, as the reading records it.
    Path,
}

/// One parameter of a function of the runtime's that generated code calls.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Parameter {
    /// The word itself.
    Given(Word),
    /// The address of room for one, which the function writes.
    Room(Word),
    /// The address of as many of one as another parameter counts, which the function reads.
    Slice(Word),
}

impl From<HostParameter> for Parameter {
    fn from(parameter: HostParameter) -> Parameter {
        match parameter {
            HostParameter::Given(word) => Parameter::Given(Word::Host(word)),
            HostParameter::Room(word) => Parameter::Room(Word::Host(word)),
            HostParameter::Slice(word) => Parameter::Slice(Word::Host(word)),
        }
    }
}

/// A function of the runtime's that generated code calls, with what it takes and answers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GeneratedCall {
    /// Its symbol.
    pub name: &'static str,
    /// What it takes, in order.
    pub takes: &'static [Parameter],
    /// What it answers, where it answers anything.
    pub answers: Option<Word>,
}

/// Every function of the runtime's that generated code calls, and nothing a host calls.
///
/// What the driver declares each of these as is lowered from here, and the runtime's own tests
/// hold each of these to the function it names, as they hold [`HOST_RUNTIME`]. Between the two
/// tables is every function the runtime defines, which those tests hold too.
pub const GENERATED_RUNTIME: &[GeneratedCall] = {
    use HostWord::{Bool, Bytes, Count, Decoded, Int, List, String, Value};
    use Parameter::{Given, Room};
    use Word::{Comparison, Form, Host, Machine, Memory, Node, Path};
    &[
        GeneratedCall {
            name: ALLOCATE,
            takes: &[Given(Host(Count))],
            answers: Some(Memory),
        },
        GeneratedCall {
            name: STRING_COMPARE,
            takes: &[Given(Host(String)), Given(Host(String))],
            answers: Some(Comparison),
        },
        GeneratedCall {
            name: STRING_CONCAT,
            takes: &[Given(Host(String)), Given(Host(String))],
            answers: Some(Host(String)),
        },
        GeneratedCall {
            name: STRING_CODE_POINTS,
            takes: &[Given(Host(String))],
            answers: Some(Host(Int)),
        },
        GeneratedCall {
            name: STRING_TRIM,
            takes: &[Given(Host(String))],
            answers: Some(Host(String)),
        },
        GeneratedCall {
            name: STRING_LOWERCASE,
            takes: &[Given(Host(String))],
            answers: Some(Host(String)),
        },
        GeneratedCall {
            name: STRING_UPPERCASE,
            takes: &[Given(Host(String))],
            answers: Some(Host(String)),
        },
        GeneratedCall {
            name: STRING_CONTAINS,
            takes: &[Given(Host(String)), Given(Host(String))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_STARTS_WITH,
            takes: &[Given(Host(String)), Given(Host(String))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_ENDS_WITH,
            takes: &[Given(Host(String)), Given(Host(String))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_MATCHES,
            takes: &[Given(Machine), Given(Host(String))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_SLICE,
            takes: &[
                Given(Host(Int)),
                Given(Host(Int)),
                Given(Host(String)),
                Room(Host(String)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_SPLIT,
            takes: &[Given(Host(String)), Given(Host(String))],
            answers: Some(Host(List)),
        },
        GeneratedCall {
            name: STRING_JOIN,
            takes: &[Given(Host(String)), Given(Host(List))],
            answers: Some(Host(String)),
        },
        GeneratedCall {
            name: STRING_CONCAT_ALL,
            takes: &[Given(Host(List))],
            answers: Some(Host(String)),
        },
        GeneratedCall {
            name: STRING_REPLACE,
            takes: &[
                Given(Host(String)),
                Given(Host(String)),
                Given(Host(String)),
            ],
            answers: Some(Host(String)),
        },
        GeneratedCall {
            name: STRING_WORDS,
            takes: &[Given(Host(String))],
            answers: Some(Host(List)),
        },
        GeneratedCall {
            name: STRING_LINES,
            takes: &[Given(Host(String))],
            answers: Some(Host(List)),
        },
        GeneratedCall {
            name: STRING_FROM_INT,
            takes: &[Given(Host(Int))],
            answers: Some(Host(String)),
        },
        GeneratedCall {
            name: STRING_TO_INT,
            takes: &[Given(Host(String)), Room(Host(Int))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_REVERSE,
            takes: &[Given(Host(String))],
            answers: Some(Host(String)),
        },
        GeneratedCall {
            name: STRING_REPEAT,
            takes: &[Given(Host(Int)), Given(Host(String)), Room(Host(String))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_PAD_LEFT,
            takes: &[
                Given(Host(Int)),
                Given(Host(String)),
                Given(Host(String)),
                Room(Host(String)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_PAD_RIGHT,
            takes: &[
                Given(Host(Int)),
                Given(Host(String)),
                Given(Host(String)),
                Room(Host(String)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_CHARACTERS,
            takes: &[Given(Host(String))],
            answers: Some(Host(List)),
        },
        GeneratedCall {
            name: STRING_CODE_POINT_VALUES,
            takes: &[Given(Host(String))],
            answers: Some(Host(List)),
        },
        GeneratedCall {
            name: EXTERNAL_NULL,
            takes: &[],
            answers: Some(Form),
        },
        GeneratedCall {
            name: EXTERNAL_BOOL,
            takes: &[Given(Host(Bool))],
            answers: Some(Form),
        },
        GeneratedCall {
            name: EXTERNAL_INT,
            takes: &[Given(Host(Int))],
            answers: Some(Form),
        },
        GeneratedCall {
            name: EXTERNAL_STRING,
            takes: &[Given(Host(String))],
            answers: Some(Form),
        },
        GeneratedCall {
            name: EXTERNAL_ARRAY,
            takes: &[],
            answers: Some(Form),
        },
        GeneratedCall {
            name: EXTERNAL_APPEND,
            takes: &[Given(Form), Given(Form)],
            answers: None,
        },
        GeneratedCall {
            name: EXTERNAL_OBJECT,
            takes: &[],
            answers: Some(Form),
        },
        GeneratedCall {
            name: EXTERNAL_PUT,
            takes: &[Given(Form), Given(Host(String)), Given(Form)],
            answers: None,
        },
        GeneratedCall {
            name: EXTERNAL_JSON,
            takes: &[Given(Form)],
            answers: Some(Host(String)),
        },
        GeneratedCall {
            name: DECODE_BEGIN,
            takes: &[Given(Host(Bytes)), Given(Host(Count))],
            answers: Some(Host(Decoded)),
        },
        GeneratedCall {
            name: DECODE_HOST_BEGIN,
            takes: &[Given(Host(Bytes)), Given(Host(Count))],
            answers: Some(Host(Decoded)),
        },
        GeneratedCall {
            name: DECODE_ROOT,
            takes: &[Given(Host(Decoded))],
            answers: Some(Node),
        },
        GeneratedCall {
            name: DECODE_END,
            takes: &[Given(Host(Decoded)), Given(Host(Value))],
            answers: None,
        },
        GeneratedCall {
            name: DECODE_ABANDON,
            takes: &[Given(Host(Decoded))],
            answers: None,
        },
        GeneratedCall {
            name: PATH_BELOW,
            takes: &[Given(Path), Given(Host(String))],
            answers: Some(Path),
        },
        GeneratedCall {
            name: PATH_AT,
            takes: &[Given(Path), Given(Host(Count))],
            answers: Some(Path),
        },
        GeneratedCall {
            name: READ_ARRAY,
            takes: &[Given(Node), Given(Path), Given(Host(Decoded))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_ARRAY_LENGTH,
            takes: &[Given(Node)],
            answers: Some(Host(Count)),
        },
        GeneratedCall {
            name: READ_ELEMENT,
            takes: &[Given(Node), Given(Host(Count))],
            answers: Some(Node),
        },
        GeneratedCall {
            name: READ_OBJECT,
            takes: &[Given(Node), Given(Path), Given(Host(Decoded))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_MEMBER,
            takes: &[Given(Node), Given(Host(String))],
            answers: Some(Node),
        },
        GeneratedCall {
            name: READ_MISSING,
            takes: &[Given(Path), Given(Host(Decoded))],
            answers: None,
        },
        GeneratedCall {
            name: READ_NULL,
            takes: &[Given(Node)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_INT,
            takes: &[
                Given(Node),
                Given(Path),
                Given(Host(Decoded)),
                Room(Host(Int)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_BOOL,
            takes: &[
                Given(Node),
                Given(Path),
                Given(Host(Decoded)),
                Room(Host(Bool)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_STRING,
            takes: &[
                Given(Node),
                Given(Path),
                Given(Host(Decoded)),
                Room(Host(String)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_CASE,
            takes: &[Given(Node), Given(Path), Given(Host(Decoded))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_TAG,
            takes: &[
                Given(Node),
                Given(Host(String)),
                Given(Path),
                Given(Host(Decoded)),
            ],
            answers: Some(Node),
        },
        GeneratedCall {
            name: READ_IS,
            takes: &[Given(Node), Given(Host(String))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_NOT_A_CASE,
            takes: &[Given(Node), Given(Path), Given(Host(Decoded))],
            answers: None,
        },
        GeneratedCall {
            name: READ_INVARIANT,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Host(String)),
                Given(Host(String)),
                Given(Host(String)),
            ],
            answers: None,
        },
    ]
};

/// What generated code declares `name` as: the one entry of [`GENERATED_RUNTIME`] naming it.
///
/// # Panics
///
/// Where no entry names it, which is generated code calling a function of the runtime's this crate
/// does not say.
pub fn generated_call(name: &str) -> &'static GeneratedCall {
    GENERATED_RUNTIME
        .iter()
        .find(|call| call.name == name)
        .unwrap_or_else(|| panic!("{name} is not a function of the runtime's generated code calls"))
}

/// What a generated function answers with instead of its value directly.
///
/// A Souther computation ends with a value or without one, and a plain return can only ever say
/// the first — which is why every generated function takes one more parameter than its signature
/// shows a caller, a pointer the value is written through, and answers this instead. `ANSWERED`
/// says the pointer holds it. Any other code says the pointer was never written, and is one of
/// two things: a language abort's wire number, or one of [`HOST_STATUSES`], which a host's
/// implementation of a behavior brings about and no Souther computation answers. Generated code
/// hands either on untouched, so what a host's call is answered with may be one a host brought
/// about further down.
///
/// What number a member of `souther_compiler`'s `AbortKind` gets is not here. This crate is the
/// wire's width and the values it reserves, facts a target decides; which reason gets which of the
/// numbers left over is `souther_native_driver`'s own exhaustive mapping, kept apart from this
/// crate for the reason this file's own doc gives — nothing about what Souther means belongs here,
/// and an abort's reason is exactly that.
pub type Status = u32;

/// The pointer holds the answer.
pub const ANSWERED: Status = 0;

/// A behavior was called through a requirement it was handed no capability for: the requirements
/// were null, or the address standing for one of them was.
///
/// Answered by the object and never by an implementation: an implementation answering it is one
/// breaking what it may answer ([`INJECTION_PROTOCOL_VIOLATION`]), and a host reading it back is told that it
/// supplied nothing, which is a different thing from an implementation saying so.
pub const INJECTION_UNBOUND: Status = 0x7fff_fffd;

/// An implementation a host wrote answered what it may not: anything but [`ANSWERED`] and
/// [`HOST_EXCEPTION`].
///
/// An implementation is outside the model, where a value that breaks a clause is a reading that
/// failed and not a computation that ended (spec §invariant-abort), so it has no way to end a
/// Souther computation with a language abort, and one that answered `INVARIANT_NOT_HELD` from a
/// host constructor has not said what it meant to. The object answers this in its place, whatever
/// the implementation answered, rather than hand a language abort on that no computation of the
/// model came to.
pub const INJECTION_PROTOCOL_VIOLATION: Status = 0x7fff_fffe;

/// An implementation a host wrote ended with what its own language throws, which it keeps
/// and throws again where the outermost call returns.
///
/// Nothing crosses generated code but a status, and a host's exception is what a platform failure
/// is (spec §java-impl-rules) and what a mistake in the implementation is alike: this says only
/// that one was thrown, and the host that kept it knows which. So it is not a Souther abort, and it
/// is not called a platform failure.
pub const HOST_EXCEPTION: Status = 0x7fff_ffff;

/// The statuses no Souther computation answers, by the names a host is told them under.
///
/// Numbered from the top of what a C `int` holds, which is what a header's enumeration is, so the
/// numbers a language abort is given, counted up from one, never reach them.
pub const HOST_STATUSES: &[(&str, Status)] = &[
    ("INJECTION_UNBOUND", INJECTION_UNBOUND),
    ("INJECTION_PROTOCOL_VIOLATION", INJECTION_PROTOCOL_VIOLATION),
    ("HOST_EXCEPTION", HOST_EXCEPTION),
];

/// A row's stand-in was asked for what the row states nothing about: none of its entries states
/// the arguments it was called with, and it states nothing for the rest (upstream's
/// `FakeMissException`, which a row's report files under fake resolution).
///
/// Not a host's: a row is run by this project's own harness and a host is told nothing of it
/// ([`EXAMPLE_STATUSES`]). Not a language abort either: no computation of the model came to it,
/// and the row that stood the fake in is what it says something about.
pub const FAKE_NO_OUTPUT: Status = 0x7fff_fffc;

/// The statuses no Souther computation answers that only running a row brings about. Reserved
/// beside [`HOST_STATUSES`], and not among them: a header tells a host the one and never the other.
pub const EXAMPLE_STATUSES: &[(&str, Status)] = &[("FAKE_NO_OUTPUT", FAKE_NO_OUTPUT)];

/// What an implementation may answer: every other status it answers is [`INJECTION_PROTOCOL_VIOLATION`].
pub const IMPLEMENTATION_ANSWERS: &[Status] = &[ANSWERED, HOST_EXCEPTION];

#[cfg(test)]
mod tests {
    use super::{
        ABI_GENERATION, EXAMPLE_STATUSES, FAKE_NO_OUTPUT, FIRST_FIELD, HOST_STATUSES,
        HostListOperation, HostWord, IMPLEMENTATION_ANSWERS, INJECTION_PROTOCOL_VIOLATION,
        INJECTION_UNBOUND, SLOT, TOKEN, WHICH, behavior_symbol, boundary_symbol,
        checked_constructor_symbol, constructor_symbol, example_symbol, field_at, held_symbol,
        home_symbol, host_behavior_answer_case_symbol, host_behavior_symbol, host_bind_symbol,
        host_case_symbol, host_constructor_symbol, host_decode_symbol, host_encode_symbol,
        host_field_symbol, host_implement_symbol, host_implementation_type, host_list_symbol,
        host_value_symbol, member_at, reader_symbol, type_symbol, value_symbol,
    };

    /// Each generation is under the number after the one before it.
    #[test]
    fn every_generation_takes_the_next_number() {
        for pair in super::GENERATIONS.windows(2) {
            assert_eq!(
                pair[1].0,
                pair[0].0 + 1,
                "{:?} after {:?}",
                pair[1],
                pair[0]
            );
        }
    }

    #[test]
    fn a_behavior_is_reached_by_its_module_and_its_name() {
        assert_eq!(
            behavior_symbol("calculation", "add"),
            "souther4.calculation.add"
        );
    }

    #[test]
    fn a_dotted_module_keeps_its_dots() {
        assert_eq!(behavior_symbol("lib.pub", "bill"), "souther4.lib.pub.bill");
    }

    /// What the reading rests on. Were this admitted, `a.b` / `c` and `a` / `b.c` would be spelt
    /// the same way, and a caller reaching one would reach the other.
    #[test]
    #[should_panic(expected = "neither dot nor dollar")]
    fn a_behavior_whose_name_carries_a_dot_is_refused() {
        let _ = behavior_symbol("a", "b.c");
    }

    /// A field never lands where the type of the value is, whatever its position.
    #[test]
    fn no_field_stands_where_a_value_says_which_type_it_is() {
        for position in 0..16 {
            assert!(field_at(position) >= FIRST_FIELD);
            assert_ne!(field_at(position), WHICH);
        }
    }

    /// Two modules holding one declaration hold a copy each, and the copies are not one symbol:
    /// the declaring module reaches it as its own, and another under the declaring module's name.
    #[test]
    fn a_definition_held_by_two_modules_is_two_symbols() {
        assert_ne!(
            held_symbol("pricing", "taxed"),
            held_symbol("order", "pricing.taxed")
        );
    }

    /// A row's entry is reached by neither the behavior's name nor another row's.
    #[test]
    fn a_helper_and_a_value_of_one_name_are_two_symbols() {
        assert_ne!(held_symbol("m", "v"), home_symbol("m", "v"));
    }

    #[test]
    fn each_row_of_a_behavior_is_its_own_symbol() {
        assert_eq!(
            example_symbol("calculation", "add", 0),
            "souther4.calculation.add$example$0"
        );
        assert_ne!(
            example_symbol("calculation", "add", 0),
            example_symbol("calculation", "add", 1)
        );
        assert_ne!(
            example_symbol("calculation", "add", 0),
            behavior_symbol("calculation", "add")
        );
    }

    #[test]
    fn an_entry_and_its_boundary_are_two_symbols() {
        let entry = behavior_symbol("shop", "quote");
        assert_eq!(boundary_symbol(&entry), "souther4.shop.quote$boundary");
        assert_ne!(boundary_symbol(&entry), entry);
        assert_ne!(
            boundary_symbol(&example_symbol("shop", "quote", 0)),
            boundary_symbol(&entry)
        );
    }

    #[test]
    fn a_declared_type_is_reached_by_its_module_and_its_name() {
        assert_eq!(
            type_symbol("lib.rates", "Rate"),
            "souther$type$lib.rates$Rate"
        );
    }

    /// Two declarations of one name in different modules are two symbols, and one declaration
    /// named from two objects is one.
    #[test]
    fn a_type_of_one_name_in_two_modules_is_two_symbols() {
        assert_ne!(
            type_symbol("pricing", "Round"),
            type_symbol("shapes", "Round")
        );
        assert_eq!(
            type_symbol("shapes", "Round"),
            type_symbol("shapes", "Round")
        );
    }

    /// A type's symbol is never a behavior's, whatever either is called. Both spellings are built
    /// here, so what keeps them apart is asserted rather than described.
    #[test]
    fn a_types_symbol_is_not_a_behaviors() {
        assert_ne!(
            type_symbol("lib.rates", "Rate"),
            behavior_symbol("lib.rates", "Rate")
        );
        assert_ne!(type_symbol("lib", "rates"), behavior_symbol("lib", "rates"));
        assert_ne!(
            type_symbol("pricing", "taxed"),
            held_symbol("pricing", "taxed")
        );
    }

    /// What the reading rests on, as it is for a behavior: were this admitted, `a.b` / `C` and
    /// `a` / `b.C` would be spelt the same way.
    #[test]
    #[should_panic(expected = "neither dollar nor dot")]
    fn a_declared_type_whose_name_carries_a_dot_is_refused() {
        let _ = type_symbol("a", "b.C");
    }

    /// A tag that is an address needs a storage location of its own, and nothing with no bytes in
    /// it has one it does not share.
    #[test]
    fn a_token_is_at_least_one_byte() {
        assert!(!TOKEN.is_empty());
    }

    #[test]
    fn fields_and_members_follow_one_after_another() {
        assert_eq!(field_at(1) - field_at(0), SLOT);
        assert_eq!(member_at(0), 0);
        assert_eq!(member_at(3) - member_at(2), SLOT);
    }

    #[test]
    fn a_published_value_is_reached_by_its_module_and_its_name() {
        assert_eq!(
            value_symbol("pricing", "standard"),
            "souther4.pricing$value$standard"
        );
    }

    /// A value's own entry is never a behavior's symbol, whatever either is called — the two share
    /// a module's dot-carrying prefix and nothing else.
    #[test]
    fn a_values_entry_is_not_a_behaviors_symbol() {
        assert_ne!(
            value_symbol("pricing", "standard"),
            behavior_symbol("pricing", "standard")
        );
        assert_ne!(
            value_symbol("pricing", "standard"),
            held_symbol("pricing", "standard")
        );
    }

    /// Two modules each publishing a value of one name publish two symbols, the same as two
    /// modules declaring a behavior of one name do.
    #[test]
    fn a_value_of_one_name_in_two_modules_is_two_symbols() {
        assert_ne!(
            value_symbol("pricing", "standard"),
            value_symbol("shipping", "standard")
        );
    }

    #[test]
    #[should_panic(expected = "neither dollar nor dot")]
    fn a_published_values_name_carrying_a_dot_is_refused() {
        let _ = value_symbol("a", "b.c");
    }

    #[test]
    fn a_type_is_built_through_its_module_and_its_name() {
        assert_eq!(
            constructor_symbol("pricing", "Amount"),
            "souther4.pricing$construct$Amount"
        );
    }

    /// A constructor is none of the other things a module's name reaches, whatever it is called:
    /// not the type's token, not a behavior, not a published value.
    #[test]
    fn a_constructor_is_not_any_other_symbol_of_one_name() {
        let built = constructor_symbol("pricing", "Amount");
        assert_ne!(built, type_symbol("pricing", "Amount"));
        assert_ne!(built, behavior_symbol("pricing", "Amount"));
        assert_ne!(built, value_symbol("pricing", "Amount"));
        assert_ne!(built, home_symbol("pricing", "Amount"));
        assert_ne!(built, checked_constructor_symbol("pricing", "Amount"));
    }

    #[test]
    fn what_decides_a_construction_is_reached_by_the_types_module_and_name() {
        assert_eq!(
            checked_constructor_symbol("pricing", "Amount"),
            "souther4.pricing$checked$Amount"
        );
    }

    /// What decides a construction is none of the other things a module's name reaches either.
    #[test]
    fn what_decides_a_construction_is_not_any_other_symbol_of_one_name() {
        let deciding = checked_constructor_symbol("pricing", "Amount");
        assert_ne!(deciding, type_symbol("pricing", "Amount"));
        assert_ne!(deciding, behavior_symbol("pricing", "Amount"));
        assert_ne!(deciding, value_symbol("pricing", "Amount"));
        assert_ne!(deciding, home_symbol("pricing", "Amount"));
    }

    #[test]
    #[should_panic(expected = "neither dollar nor dot")]
    fn a_checked_types_name_carrying_a_dot_is_refused() {
        let _ = checked_constructor_symbol("a", "b.C");
    }

    #[test]
    #[should_panic(expected = "neither dollar nor dot")]
    fn a_constructed_types_name_carrying_a_dot_is_refused() {
        let _ = constructor_symbol("a", "b.C");
    }

    #[test]
    fn a_host_reaches_a_type_under_its_module_and_its_name() {
        assert_eq!(
            host_constructor_symbol("pricing", "Amount"),
            "souther4_m_pricing_t_Amount_construct"
        );
        assert_eq!(
            host_field_symbol("pricing", "Amount", "value"),
            "souther4_m_pricing_t_Amount_f_value"
        );
        assert_eq!(
            host_case_symbol("pricing", "Result"),
            "souther4_m_pricing_t_Result_case"
        );
        assert_eq!(
            host_decode_symbol("pricing", "Amount"),
            "souther4_m_pricing_t_Amount_decode"
        );
        assert_eq!(
            host_encode_symbol("pricing", "Amount"),
            "souther4_m_pricing_t_Amount_encode"
        );
    }

    #[test]
    fn a_host_reaches_a_behavior_and_a_value_under_their_module() {
        assert_eq!(
            host_behavior_symbol("lib.shop", "quote"),
            "souther4_m_lib_m_shop_b_quote"
        );
        assert_eq!(
            host_value_symbol("lib.shop", "standard"),
            "souther4_m_lib_m_shop_v_standard"
        );
        assert_eq!(
            host_behavior_answer_case_symbol("lib.shop", "find"),
            "souther4_m_lib_m_shop_b_find_answer_case"
        );
    }

    #[test]
    fn a_host_reaches_a_list_under_its_module_and_what_an_element_crosses_as() {
        assert_eq!(
            host_list_symbol("shop", false, HostWord::Value, HostListOperation::Construct),
            "souther4_m_shop_l_value_construct"
        );
        assert_eq!(
            host_list_symbol("lib.shop", true, HostWord::Int, HostListOperation::At),
            "souther4_m_lib_m_shop_l_present_int_at"
        );
        assert_eq!(
            host_list_symbol("shop", false, HostWord::List, HostListOperation::Length),
            "souther4_m_shop_l_list_length"
        );
    }

    #[test]
    fn a_name_that_is_not_ascii_letters_and_digits_is_escaped() {
        assert_eq!(
            host_behavior_symbol("shop", "foo_bar"),
            "souther4_m_shop_b_foo__bar"
        );
        assert_eq!(
            host_behavior_symbol("shop", "数量"),
            "souther4_m_shop_b__u6570__u91cf_"
        );
    }

    /// What another object reads a value of a type through is under the type's module, apart from
    /// everything a host reaches.
    #[test]
    fn a_type_is_read_through_its_module_and_its_name() {
        assert_eq!(
            reader_symbol("pricing", "Amount"),
            "souther4.pricing$read$Amount"
        );
    }

    /// One mark or one operation a host's symbol is read back into.
    #[derive(Debug, PartialEq, Eq)]
    enum Read {
        Under(char, String),
        Operation(String),
    }

    /// A host's symbol read back from the left, the way [`super::host_module`] says it can be:
    /// what it was made of, or nothing where it is not one.
    fn read_back(symbol: &str) -> Option<Vec<Read>> {
        let mut rest = symbol.strip_prefix(&format!("souther{ABI_GENERATION}"))?;
        let mut read = Vec::new();
        while !rest.is_empty() {
            rest = rest.strip_prefix('_')?;
            let mut chars = rest.chars();
            let mark = chars.next()?;
            if "mbvtf".contains(mark) && chars.next() == Some('_') {
                let (name, after) = name_back(&rest[2..])?;
                read.push(Read::Under(mark, name));
                rest = after;
            } else {
                let end = rest.find('_').unwrap_or(rest.len());
                read.push(Read::Operation(rest[..end].to_string()));
                rest = &rest[end..];
            }
        }
        Some(read)
    }

    /// One name, up to the `_` that ends it.
    fn name_back(mut rest: &str) -> Option<(String, &str)> {
        let mut name = String::new();
        loop {
            match rest.chars().next() {
                None => return Some((name, rest)),
                Some('_') => match rest[1..].chars().next() {
                    Some('_') => {
                        name.push('_');
                        rest = &rest[2..];
                    }
                    Some('u') => {
                        let end = rest[2..].find('_')? + 2;
                        let code = u32::from_str_radix(&rest[2..end], 16).ok()?;
                        name.push(char::from_u32(code)?);
                        rest = &rest[end + 1..];
                    }
                    _ => return Some((name, rest)),
                },
                Some(character) => {
                    name.push(character);
                    rest = &rest[character.len_utf8()..];
                }
            }
        }
    }

    /// What a host's symbol for a name under this module should read back as.
    fn under(module: &str, then: Vec<Read>) -> Vec<Read> {
        let mut read: Vec<Read> = module
            .split('.')
            .map(|segment| Read::Under('m', segment.to_string()))
            .collect();
        read.extend(then);
        read
    }

    /// Every host symbol reads back as the names it was made of, so no two different sets of names
    /// are one symbol — whatever the names hold, and including names that look like a mark or an
    /// escape.
    #[test]
    fn a_hosts_symbol_reads_back_as_what_it_was_made_of() {
        let modules = ["a", "a.b", "a_b", "a__b", "a._b", "é.b", "a_m_b", "a.m"];
        let names = [
            "a",
            "b",
            "u",
            "_",
            "_u",
            "a_b",
            "a__b",
            "_u41_",
            "a_m_b",
            "a_t_b",
            "construct",
            "case",
            "_case",
            "é",
            "数量",
            "𠮷",
            "a.b",
            "a$b",
            "A",
            "x_f_y",
        ];
        let mut seen = std::collections::HashMap::new();
        let mut hold = |symbol: String, expected: Vec<Read>| {
            assert!(
                symbol.starts_with(|it: char| it.is_ascii_alphabetic())
                    && symbol
                        .chars()
                        .all(|it| it.is_ascii_alphanumeric() || it == '_'),
                "{symbol} is not a C identifier"
            );
            assert_eq!(read_back(&symbol).as_ref(), Some(&expected), "{symbol}");
            let said = format!("{expected:?}");
            let before = seen.entry(symbol.clone()).or_insert_with(|| said.clone());
            assert_eq!(*before, said, "{symbol} is two things");
        };
        let named = |mark: char, name: &str| Read::Under(mark, name.to_string());
        let operation = |it: &str| Read::Operation(it.to_string());
        for module in modules {
            for name in names {
                hold(
                    host_behavior_symbol(module, name),
                    under(module, vec![named('b', name)]),
                );
                hold(
                    host_value_symbol(module, name),
                    under(module, vec![named('v', name)]),
                );
                for (symbol, done) in [
                    (host_bind_symbol(module, name), "bind"),
                    (host_implement_symbol(module, name), "implement"),
                    (host_implementation_type(module, name), "implementation"),
                ] {
                    hold(
                        symbol,
                        under(module, vec![named('b', name), operation(done)]),
                    );
                }
                hold(
                    host_behavior_answer_case_symbol(module, name),
                    under(
                        module,
                        vec![named('b', name), operation("answer"), operation("case")],
                    ),
                );
                for (symbol, done) in [
                    (host_constructor_symbol(module, name), "construct"),
                    (host_case_symbol(module, name), "case"),
                    (host_decode_symbol(module, name), "decode"),
                    (host_encode_symbol(module, name), "encode"),
                ] {
                    hold(
                        symbol,
                        under(module, vec![named('t', name), operation(done)]),
                    );
                }
                for field in names {
                    hold(
                        host_field_symbol(module, name, field),
                        under(module, vec![named('t', name), named('f', field)]),
                    );
                }
            }
            for present in [false, true] {
                for word in [
                    HostWord::Int,
                    HostWord::Bool,
                    HostWord::String,
                    HostWord::Value,
                    HostWord::List,
                ] {
                    for (done, spelt) in [
                        (HostListOperation::Construct, "construct"),
                        (HostListOperation::Length, "length"),
                        (HostListOperation::At, "at"),
                    ] {
                        let mut read = vec![operation("l")];
                        if present {
                            read.push(operation("present"));
                        }
                        read.push(operation(word.spelt()));
                        read.push(operation(spelt));
                        hold(
                            host_list_symbol(module, present, word, done),
                            under(module, read),
                        );
                    }
                }
            }
        }
    }

    /// What a host calls is never what one object built by this compiler calls in another, so the
    /// two are free to differ in how they are called.
    #[test]
    fn a_hosts_symbol_is_never_one_objects_call_into_another() {
        let others = [
            behavior_symbol("shop", "quote"),
            value_symbol("shop", "quote"),
            constructor_symbol("shop", "Quote"),
            reader_symbol("shop", "Quote"),
        ];
        for symbol in [
            host_behavior_symbol("shop", "quote"),
            host_value_symbol("shop", "quote"),
            host_constructor_symbol("shop", "Quote"),
            host_decode_symbol("shop", "Quote"),
        ] {
            assert!(!others.contains(&symbol), "{symbol}");
        }
    }

    /// What a host brings about is told apart by number alone, from `ANSWERED` and from each
    /// other, and stays inside what a C `int` holds, which is what a header's enumeration is.
    #[test]
    fn what_a_host_brings_about_is_a_number_of_its_own() {
        let mut seen = vec![super::ANSWERED];
        for (name, number) in HOST_STATUSES {
            assert!(!seen.contains(number), "{name} answers {number} twice");
            assert!(i32::try_from(*number).is_ok(), "{name} is past a C int");
            seen.push(*number);
        }
        for (name, number) in EXAMPLE_STATUSES {
            assert!(!seen.contains(number), "{name} answers {number} twice");
            assert!(i32::try_from(*number).is_ok(), "{name} is past a C int");
            seen.push(*number);
        }
        assert!(!IMPLEMENTATION_ANSWERS.contains(&INJECTION_UNBOUND));
        assert!(!IMPLEMENTATION_ANSWERS.contains(&INJECTION_PROTOCOL_VIOLATION));
        assert!(!IMPLEMENTATION_ANSWERS.contains(&FAKE_NO_OUTPUT));
    }
}
