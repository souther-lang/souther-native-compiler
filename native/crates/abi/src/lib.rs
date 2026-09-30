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
/// `5` is `souther-native-compiler#95`, and is two contracts moving together. Between objects, a
/// function value is one another object may call: what stands at [`FUNCTION_INVOKE`] and how it is
/// called are stated here, so a behavior or a published value taking or answering one reaches
/// across objects. Between the object and a host, every value of the model a function hands a host
/// is written through room, in the words of its [`HostShape`], and none is answered as the
/// function's return: a field's reader returns nothing, where it answered the field. A tuple, an
/// optional at any depth and a function value cross to a host, and every function a host reaches a
/// list or a function value through is spelt under the shape it crosses in ([`host_list_symbol`],
/// [`host_function_symbol`]).
///
/// `6` is `souther-native-compiler#107`, and is the calling convention of eight functions moving.
/// A string operation whose answer can be more text than a string holds no longer answers the
/// string: it writes it through room and answers whether it wrote one, as `String.repeat` already
/// did. The symbols are the runtime's own and carry no generation, so an object built before the
/// move calls `souther_string_concat(a, b)` and reads a pointer back from a function that now
/// takes a third argument for the answer, which no linker sees; what a generation is for is that
/// such an object no longer resolves the symbols of one built after it, and that the runtime is
/// held to the same by a symbol of its own ([`runtime_generation_symbol`]). The record of the
/// contract each generation begins from is `generations/<n>.txt`, and `tests/generation.rs` holds
/// the current one to it.
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

/// The symbol the runtime defines to say which generation it answers to, and every object generated
/// code makes refers to.
///
/// A generation written into the symbols of generated code keeps two objects of different
/// generations from resolving one another, and keeps nothing from the runtime: the functions
/// generated code calls in it are the runtime's own, `souther_string_concat` among them, and a
/// linker resolves such a name to whatever defines it, however differently the call is made. So
/// the runtime states its generation in the one way a linker checks, a symbol only that generation
/// defines, and every object refers to the one its own generation names. An object linked with a
/// runtime of another generation then has an undefined symbol, and never a call made one way and
/// answered the other. The runtime writes its definition from [`ABI_GENERATION`] when it is built
/// and the driver writes the reference from it, so neither spells a number.
///
/// An object built before the symbol existed refers to none, and this cannot stop it: the guard
/// is from generation 6 on.
pub fn runtime_generation_symbol() -> String {
    format!("souther_runtime_abi_{ABI_GENERATION}")
}

/// The one function a host calls before any other, to ask which generation a library it has loaded
/// answers to: `uint32_t souther_abi_generation(void)`, answering [`ABI_GENERATION`].
///
/// Outside every generation, and not generation 0 of them: its name, what it takes and what it
/// answers are the same for every library that has it, so a host can ask it of a library of any
/// generation and refuse one it was not written for before it looks up anything else. No
/// generation moving makes a change to it legal, so it is in no record of one, and a test of its
/// own holds it (`tests/bootstrap.rs`). A library without the symbol is one of generation 8 or
/// earlier, from before it existed.
///
/// [`runtime_generation_symbol`] is another thing: the guard a linker checks between generated
/// objects and the runtime, which no library exports.
pub const GENERATION_QUERY: &str = "souther_abi_generation";

/// [`GENERATION_QUERY`] as a header declares it.
pub const GENERATION_QUERY_DECLARED: &str = "uint32_t souther_abi_generation(void);";

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
    (
        5,
        "a function value called across objects through its header, and every value of the model \
         a host is handed written through room in the words of the shape it crosses in \
         (souther-native-compiler#95)",
    ),
    (
        6,
        "a generated string operation that may have no answer writes it through room and answers \
         whether it wrote one: `souther_string_concat`, `_lowercase`, `_uppercase`, `_join`, \
         `_concat_all`, `_replace`, `_reverse` and `_from_decimal`, where each answered the string \
         itself, and the runtime says which generation it is by a symbol every object refers to \
         (souther-native-compiler#107)",
    ),
    (
        7,
        "an issue a reading found says its message key (`souther_issue_message_key`) and its \
         metadata as the JSON object it is (`souther_issue_meta`), in place of the count, name and \
         text of each entry; and a clause the checker states as a standard constraint is reported \
         as that constraint by a call of its own (`souther_read_min_length` and the rest) \
         (souther-native-compiler#97)",
    ),
    (
        8,
        "a host handing text in may have no place for it as a `String`: `souther_string_of_utf8` \
         writes the string through room and answers whether it wrote one, as a generated string \
         operation already does, in place of always answering one; and `souther_decimal_of_parts` \
         takes the integer's digits as bytes and a count in place of a `String`, since they are \
         never the value's written form and were never fallible on a String's own bound \
         (souther-native-compiler#109)",
    ),
    (
        9,
        "what a host hands in never ends the process: a temporal is made of and read as the numbers \
         it means, each an `Int` (`souther_date_of_parts`, `_parts`, and the same for `time`, \
         `datetime` and `instant`), in place of the ISO text `souther_date_of_iso` and the rest took \
         and answered and ended the process on; `souther_decimal_of_parts` writes the `Decimal` \
         through room and answers whether its parts name one; and a host brackets its calls with \
         a scope (`souther_scope_open`, `souther_scope_close`), which the runtime closes only as \
         the innermost open on the calling thread and refuses otherwise, in place of the arena \
         position `souther_mark` answered and `souther_reset` took back unchecked \
         (souther-native-compiler#137)",
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

/// What a host does with a list, through a function the object defines for each shape an element
/// crosses in ([`host_list_symbol`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HostListOperation {
    /// `(count, the elements as columns) -> list`: a list of `count` elements, the one at an index
    /// being what each column holds at that index. A column is a [`HostParameter::Slice`] of one of
    /// the words the element crosses as ([`HostShape::words`]), so an element crossing as a presence
    /// and a value is two columns. Building a list ends with the list whatever the elements are, so
    /// this answers no status; a count below nought, or one no room could be taken for, is the
    /// host's mistake and ends the process rather than being read as some other count.
    Construct,
    /// `(list) -> count`: how many elements the list holds.
    Length,
    /// `(list, index, room for each word the element crosses as) -> bool`: one where the index is
    /// inside the list, with the element written through the room as every value a host is handed
    /// is, and nought where it is outside it, with nothing written.
    At,
}

/// Where a host builds or reads a list whose elements cross as `element`: `_l_`, the shape as
/// [`HostShape::spelt`] spells it, and the operation.
///
/// Under the shape an element crosses in and not under the element's type: a list of one declared
/// type and a list of another are both a list of addresses to the functions here, which put an
/// element in its slot and take one out without knowing what it is. Under a module all the same,
/// for the reason every other function a host reaches is: two builds' objects linked into one
/// library each define their own, and a symbol under no module would be defined twice. Which
/// module's a host calls makes no difference to the list it is handed.
pub fn host_list_symbol(module: &str, element: &HostShape, operation: HostListOperation) -> String {
    let mut spelt = host_module(module);
    spelt.push_str("_l_");
    spelt.push_str(&element.spelt());
    spelt.push_str(match operation {
        HostListOperation::Construct => "_construct",
        HostListOperation::Length => "_length",
        HostListOperation::At => "_at",
    });
    spelt
}

/// What a host does with a function value, through what the object defines for each shape a
/// function crosses in ([`host_function_symbol`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HostFunctionOperation {
    /// `(function, what it takes as a host hands each over, room for each word of its answer) ->
    /// status`: the function called, its answer written through the room only where the status is
    /// `ANSWERED`. The status is the function's own: a computation that ended without a value
    /// answers why, as a behavior's call does.
    Call,
    /// `(room for a hosted function, the host's implementation, what it is handed first) ->
    /// function`: a function value of the host's own, laid out in the room
    /// ([`room_for_hosted_function`]) and answered as the address of it. Calling the value calls
    /// the implementation, a function of the type [`HostFunctionOperation::Implementation`] names,
    /// with what it was handed first, what the value was called with as a host is handed each, and
    /// room for each word of its answer; what it answers is held to [`IMPLEMENTATION_ANSWERS`], as
    /// an implementation of a behavior is. Nothing is copied out of the room, so a host keeps it,
    /// and the function callable, for as long as the value may be called.
    Implement,
    /// The type of the function a host implements a function value as, which nothing defines: a
    /// name in C, as [`host_implementation_type`] is for a behavior.
    Implementation,
}

/// Where a host calls or makes a function value that crosses as `function`: `_fn_`, the shape as
/// [`HostShape::spelt`] spells it, and the operation. Under what the function crosses as and a
/// module, for the reasons [`host_list_symbol`] is.
///
/// # Panics
///
/// Where `function` is not the shape of a function value.
pub fn host_function_symbol(
    module: &str,
    function: &HostShape,
    operation: HostFunctionOperation,
) -> String {
    assert!(
        matches!(function, HostShape::Function { .. }),
        "{function:?} is not how a function value crosses"
    );
    let mut spelt = host_module(module);
    spelt.push_str("_fn_");
    spelt.push_str(&function.spelt());
    spelt.push_str(match operation {
        HostFunctionOperation::Call => "_call",
        HostFunctionOperation::Implement => "_implement",
        HostFunctionOperation::Implementation => "_implementation",
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
///
/// A case here is one objects of a library can tell apart, which is not the same as one a host can
/// cross: `Rational` is a case a value of a union can be and has no external form, so it has a
/// token and is not in [`HOST_CASES`].
pub const BUILT_IN_CASES: &[&str] = &[
    "Int",
    "Bool",
    "String",
    "Decimal",
    "Rational",
    "Date",
    "Time",
    "DateTime",
    "Instant",
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

/// Every unit the language itself declares, as the module and the name the checker writes it
/// under: the cases of `RoundingMode`, which `Decimal.divide`, `Decimal.round` and `Decimal.toInt`
/// take (spec §stdlib-decimal).
///
/// A value of one is tagged by its declaration's token, under [`type_symbol`], as a value of any
/// declared type is. What differs is who defines it. A module's declaration is at home in the
/// object of the build that checked the module, and the language's are at home in no build: the
/// runtime defines each of these, as it defines a token for each of [`BUILT_IN_CASES`], and every
/// object naming one imports it. An object defining its own would give one value two addresses, and
/// a value built in one object would not be the value another tests for.
///
/// Units and nothing else: a sum the language declares is never tagged with a token of its own,
/// since a value of it is always one of its cases, and nothing the language declares is built from
/// fields. A declaration the language gives that is not here is one no object can name, and the
/// driver refuses it as that rather than importing a symbol nothing defines.
///
/// What the language declares is the language's to say, and not this table's: `language-units.txt`
/// beside this crate is every unit the checker's library declares, which the Java half writes from
/// `CheckedProgram.languageDeclarations()` and holds to what upstream answers, and a test here
/// holds this table to that file. So a unit the language adds fails here until the runtime defines
/// its token, and the runtime's own test holds its tokens to this table.
pub const LANGUAGE_UNITS: &[(&str, &str)] = &[
    ("souther.decimal", "HALF_UP"),
    ("souther.decimal", "HALF_EVEN"),
    ("souther.decimal", "HALF_DOWN"),
    ("souther.decimal", "UP"),
    ("souther.decimal", "DOWN"),
    ("souther.decimal", "CEILING"),
    ("souther.decimal", "FLOOR"),
];

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
/// is one slot. A `Set` and a `Map` are not laid out as this, nor laid out here at all: they are
/// the runtime's ([`SET_EMPTY`]).
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

/// Where a function value holds its code: the function a call of the value reaches, handed the
/// value itself first, then what the function takes, each as the word the generated code holds it
/// in, then room for one slot, and answering `status + out` the way every generated function does.
///
/// That, and nothing more, is what a function value is to anything but the code that made it. What
/// stands after the code is the code's own to read — what a closure captured, the function a
/// restated one wraps, what a host's implementation is — so a caller hands the value to its own
/// code and never reads further into it. Two objects built by this compiler call one another's
/// function values on that alone: the code at this slot came from the object that made the value,
/// and it is the one thing that knows what stands beside it.
pub const FUNCTION_INVOKE: i64 = 0;

/// Where a function value a host made of an implementation of its own holds what its code reads
/// the implementation out of: room laid out as [`HOSTED_IMPLEMENTATION`] and [`HOSTED_USERDATA`]
/// say, as a host lays out room for an implementation of a behavior. After the code, as everything
/// a function value holds but its code is.
pub const HOSTED_FUNCTION_HOSTED: i64 = FUNCTION_INVOKE + SLOT;

/// How much room a host lays out for a function value of its own
/// ([`HostFunctionOperation::Implement`]).
pub const fn room_for_hosted_function() -> i64 {
    HOSTED_FUNCTION_HOSTED + room_for_hosted()
}

/// Room a host lays out, owns and never reads: a capability, what a host's implementation of a
/// behavior is read out of, and a function value made of a host's own function.
///
/// A host keeps each for as long as the thing it stands for may be called, which is longer than a
/// scope, so the room is the host's and not the arena's. What stands in it is the generated code's
/// own, written by what makes the capability or the value and read by nothing else: the offsets
/// above are between the generated code and itself. So a host is told the size and the alignment
/// and no field, and a declaration of it is as many slots, each a `uint64_t`, as the room takes:
/// a host that allocates the type by its name has the size and the alignment right, and nothing
/// it could write into a field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HostStorage {
    /// Its name in C, which a host allocates it by.
    pub name: &'static str,
    /// How many slots it takes, each [`SLOT`] bytes and aligned to a slot.
    pub slots: i64,
}

/// Every room a host lays out.
pub const HOST_STORAGE: &[HostStorage] = &[
    HostStorage {
        name: "souther_capability",
        slots: room_for_capability() / SLOT,
    },
    HostStorage {
        name: "souther_hosted",
        slots: room_for_hosted() / SLOT,
    },
    HostStorage {
        name: "souther_hosted_function",
        slots: room_for_hosted_function() / SLOT,
    },
];

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
    assert!(FUNCTION_INVOKE + SLOT <= HOSTED_FUNCTION_HOSTED);
    assert!(HOSTED_FUNCTION_HOSTED + HOSTED_USERDATA + SLOT <= room_for_hosted_function());
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

/// The symbol two strings are joined through. Writes a new string through room and touches neither
/// operand, and answers whether it wrote one: a join longer than a string holds writes none.
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
/// that answers nothing for some of what it is handed — a slice the string has no room for, copies
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
/// Here rather than left to whoever writes such a caller, for the reason [`SCOPE_OPEN`] is: the layout
/// above is between this crate and the runtime, and a caller that built a string from it would be
/// a third party to a two-party contract.
pub const STRING_OF_UTF8: &str = "souther_string_of_utf8";

/// The symbols such a caller reads a string back through: first how many bytes it holds.
pub const STRING_LENGTH: &str = "souther_string_length";
/// And then where those bytes start.
pub const STRING_BYTES: &str = "souther_string_bytes";

/// The symbol a caller outside a Souther program makes a `Decimal` with: its integer, as integer
/// text in bytes, and its scale, written through room where they name one, answering whether they
/// did. What does not name one is answered as that, and never ends the process: the runtime decides
/// what a `Decimal` is, so no binding has to.
///
/// A `Decimal` is an address and nothing a host reads behind: how the runtime keeps one is the
/// runtime's alone, so a host hands over and reads back the two numbers the language says a
/// `Decimal` is, and never a layout. The integer crosses as text because it has as many digits as
/// it has, which no word holds; the text is the integer and not the value's written form, so it
/// says nothing of where a point goes and nothing of how a boundary writes the amount.
pub const DECIMAL_OF_PARTS: &str = "souther_decimal_of_parts";
/// The symbols such a caller reads a `Decimal` back through: its integer, as integer text in a
/// string.
pub const DECIMAL_UNSCALED: &str = "souther_decimal_unscaled";
/// And its scale.
pub const DECIMAL_SCALE: &str = "souther_decimal_scale";

/// The symbol a `Decimal` literal is made through, from the integer the checker read it as, which
/// the object carries as a string, and its scale.
pub const DECIMAL_LITERAL: &str = "souther_decimal_literal";

/// The symbol two `Decimal`s are compared through, by amount and whatever their scales. Answers
/// below, at or above nought, as [`STRING_COMPARE`] does, and for all six comparisons for the same
/// reason.
pub const DECIMAL_COMPARE: &str = "souther_decimal_compare";

/// The symbol a division asks whether its divisor is nought through, before it divides.
pub const DECIMAL_IS_ZERO: &str = "souther_decimal_is_zero";

/// The symbols the `Decimal` module's kernels and its operators are computed through (spec
/// §stdlib-decimal), taking what each takes in the order it takes it.
///
/// What each answers is the runtime's to work out and not generated code's: the scale a result
/// has, how it is rounded, and where there is no result at all. One that answers nothing for some
/// of what it is handed answers whether it wrote its value through room it is handed last, as a
/// kernel over strings does ([`STRING_TRIM`]), and the reason the run ends for is the caller's.
/// A `RoundingMode` is handed over as the value it is, and the runtime reads which case it is off
/// the token it carries ([`LANGUAGE_UNITS`]).
pub const DECIMAL_NEGATE: &str = "souther_decimal_negate";
/// `+` and `Decimal.add`, into room for the `Decimal`.
pub const DECIMAL_ADD: &str = "souther_decimal_add";
/// `-` and `Decimal.subtract`, into room for the `Decimal`.
pub const DECIMAL_SUBTRACT: &str = "souther_decimal_subtract";
/// `*` and `Decimal.multiply`, into room for the `Decimal`.
pub const DECIMAL_MULTIPLY: &str = "souther_decimal_multiply";
/// `Decimal.fromInt`.
pub const DECIMAL_FROM_INT: &str = "souther_decimal_from_int";
/// `Decimal.toInt`, into room for the `Int`.
pub const DECIMAL_TO_INT: &str = "souther_decimal_to_int";
/// `Decimal.round`, into room for the `Decimal`.
pub const DECIMAL_ROUND: &str = "souther_decimal_round";
/// `Decimal.divide`, into room for the `Decimal`, over a divisor [`DECIMAL_IS_ZERO`] has said is
/// not nought.
pub const DECIMAL_DIVIDE: &str = "souther_decimal_divide";
/// `String.toDecimal`, into room for the `Decimal`.
pub const STRING_TO_DECIMAL: &str = "souther_string_to_decimal";
/// `String.fromDecimal`.
pub const STRING_FROM_DECIMAL: &str = "souther_string_from_decimal";

/// The symbols the `Rational` module's kernels and the exact reading an operator does of its
/// operands are computed through (spec §primitives, ADR-0116), taking what each takes in the order
/// it takes it.
///
/// A `Rational` has no external form, so nothing a host calls names one and each of these is a
/// call generated code makes: it stands between generated code and the runtime as a
/// [`Word::Rational`], and no [`HostWord`] is one. One that answers nothing for some of what it is
/// handed answers whether it wrote its value through room it is handed last, as [`DECIMAL_ADD`]
/// does, and the reason the run ends for is the caller's.
pub const RATIONAL_FROM_INT: &str = "souther_rational_from_int";
/// `Rational.fromDecimal`, and the exact reading of a `Decimal`.
pub const RATIONAL_FROM_DECIMAL: &str = "souther_rational_from_decimal";
/// The unary `-`.
pub const RATIONAL_NEGATE: &str = "souther_rational_negate";
/// Whether a division's divisor is nought, asked before it divides.
pub const RATIONAL_IS_ZERO: &str = "souther_rational_is_zero";
/// Whether a `Rational` is a whole number, asked before [`RATIONAL_TO_WHOLE`].
pub const RATIONAL_IS_WHOLE: &str = "souther_rational_is_whole";
/// Whether a `Rational` has a finite decimal spelling, asked before [`RATIONAL_TO_FINITE_DECIMAL`].
pub const RATIONAL_HAS_FINITE_DECIMAL: &str = "souther_rational_has_finite_decimal";
/// Two `Rational`s by exact value, as [`DECIMAL_COMPARE`] answers it and for all six comparisons for
/// the same reason. It says nothing of a pair whose order needs more room than the run has, which
/// ends the run as an arena that has run out does: that is the run's shortage and not a value
/// with no place, so no abort of the program stands for it.
pub const RATIONAL_COMPARE: &str = "souther_rational_compare";
/// `+` and `Rational.add`, into room for the `Rational`.
pub const RATIONAL_ADD: &str = "souther_rational_add";
/// `-` and `Rational.subtract`, into room for the `Rational`.
pub const RATIONAL_SUBTRACT: &str = "souther_rational_subtract";
/// `*` and `Rational.multiply`, into room for the `Rational`.
pub const RATIONAL_MULTIPLY: &str = "souther_rational_multiply";
/// `/` and `Rational.divide`, into room for the `Rational`, over a divisor [`RATIONAL_IS_ZERO`] has
/// said is not nought.
pub const RATIONAL_DIVIDE: &str = "souther_rational_divide";
/// `Rational.toWholeNumber` of a whole number, into room for the `Int`.
pub const RATIONAL_TO_WHOLE: &str = "souther_rational_to_whole";
/// `Rational.toFiniteDecimal` of a value that has one, into room for the `Decimal`.
pub const RATIONAL_TO_FINITE_DECIMAL: &str = "souther_rational_to_finite_decimal";
/// `Rational.toInt`, into room for the `Int`.
pub const RATIONAL_TO_INT: &str = "souther_rational_to_int";
/// `Rational.toDecimal`, into room for the `Decimal`.
pub const RATIONAL_TO_DECIMAL: &str = "souther_rational_to_decimal";

/// The symbols a caller outside a Souther program makes a temporal with, and reads one back
/// through: the numbers that are what it means, each an `Int`, and never the text it is written as.
///
/// A temporal is an address and nothing a host reads behind, for the reason a `Decimal` is: how the
/// runtime keeps one is the runtime's alone. What a host hands over is the coordinate each type is
/// defined by (spec §primitives): a `Date` its year, month and day; a `Time` its hour, minute and
/// second, since it is held to the second; a `DateTime` both; an `Instant` its second from
/// 1970-01-01T00:00:00Z and the nanosecond within it. Text is one way of writing those, which a host
/// has its own ways of doing, and taking it here would put the grammar `java.time` writes into every
/// binding.
///
/// Making one writes it through room where the numbers name one and answers whether they did, so
/// a month of 13 or the thirtieth of February is answered as that and ends nothing: the runtime
/// decides which numbers name a value, and a binding that checks first does so only to say it in
/// its own words. Every number is a whole `Int`, and none is narrowed on the way, so a wrong one
/// reaches the runtime as the number it is. Reading one writes each number through room for it.
pub const DATE_OF_PARTS: &str = "souther_date_of_parts";
/// The year, month and day of a `Date`.
pub const DATE_PARTS: &str = "souther_date_parts";
/// As [`DATE_OF_PARTS`], for a `Time`.
pub const TIME_OF_PARTS: &str = "souther_time_of_parts";
/// The hour, minute and second of a `Time`.
pub const TIME_PARTS: &str = "souther_time_parts";
/// As [`DATE_OF_PARTS`], for a `DateTime`: the parts of its date, then of its time.
pub const DATETIME_OF_PARTS: &str = "souther_datetime_of_parts";
/// The parts of the date and then of the time of a `DateTime`.
pub const DATETIME_PARTS: &str = "souther_datetime_parts";
/// As [`DATE_OF_PARTS`], for an `Instant`: its second from the epoch and the nanosecond within it.
pub const INSTANT_OF_PARTS: &str = "souther_instant_of_parts";
/// The second from the epoch and the nanosecond within it of an `Instant`.
pub const INSTANT_PARTS: &str = "souther_instant_parts";

/// What each temporal holds, as the counts a transport document and the runtime's literal functions
/// speak in: the days from 1970-01-01 that a `Date` holds, and the seconds from 1970-01-01T00:00:00
/// that a `DateTime` and an `Instant` hold, the first as though it were in UTC.
///
/// The bounds `java.time` states for `LocalDate`, `LocalDateTime` and `Instant` (spec §primitives),
/// and numbers, not a calendar: both halves of this backend hold a literal to them, and the
/// runtime's own tests hold its calendar to them, so no side takes them on trust from the other.
pub const DATE_DAYS: std::ops::RangeInclusive<i64> = -365_243_219_162..=365_241_780_471;
/// See [`DATE_DAYS`]: `LocalDateTime.MIN` to the last whole second of `LocalDateTime.MAX`.
pub const DATE_TIME_SECONDS: std::ops::RangeInclusive<i64> =
    -365_243_219_162 * 86_400..=365_241_780_471 * 86_400 + 86_399;
/// See [`DATE_DAYS`]: `Instant.MIN` to the second `Instant.MAX` falls in.
pub const INSTANT_SECONDS: std::ops::RangeInclusive<i64> =
    -31_557_014_167_219_200..=31_556_889_864_403_199;
/// The seconds in a day, which a `Time` holds fewer than.
pub const SECONDS_PER_DAY: i64 = 86_400;

/// The symbols a temporal literal is made through, from the count the checker read it as: the day
/// of a `Date` ([`DATE_DAYS`]), the second of the day of a `Time`, the second of a `DateTime`, and
/// the second and then the nanosecond of an `Instant`. Not text: the checker's parse decides what a
/// program may spell, and what it read is what crosses.
pub const DATE_LITERAL: &str = "souther_date_literal";
/// A `Time` literal.
pub const TIME_LITERAL: &str = "souther_time_literal";
/// A `DateTime` literal.
pub const DATETIME_LITERAL: &str = "souther_datetime_literal";
/// An `Instant` literal.
pub const INSTANT_LITERAL: &str = "souther_instant_literal";

/// The symbols two temporals of a type are compared through, in order: `==`, `<` and the rest
/// are read off one answer below, at or above nought, as [`STRING_COMPARE`] does. Two temporals
/// are equal where they name one moment or one day, which is not where their addresses are.
pub const DATE_COMPARE: &str = "souther_date_compare";
/// Two `Time`s, in order.
pub const TIME_COMPARE: &str = "souther_time_compare";
/// Two `DateTime`s, in order.
pub const DATETIME_COMPARE: &str = "souther_datetime_compare";
/// Two `Instant`s, in order, to the nanosecond.
pub const INSTANT_COMPARE: &str = "souther_instant_compare";

/// The symbols the `Date`, `Time` and `DateTime` modules' kernels are computed through (spec
/// §stdlib-date), taking what each takes in the order it takes it.
///
/// A shift, which ends the run where what it shifts to is off the end of what the type holds,
/// answers whether it wrote its value through room it is handed last, as [`DECIMAL_ADD`] does, and
/// the reason the run ends for is the caller's. A construction from parts answers whether they
/// name one in the same way, and the caller answers the case the language names for none.
pub const DATE_ADD_DAYS: &str = "souther_date_add_days";
/// `Date.addMonths`, as [`DATE_ADD_DAYS`].
pub const DATE_ADD_MONTHS: &str = "souther_date_add_months";
/// `Date.addYears`, as [`DATE_ADD_DAYS`].
pub const DATE_ADD_YEARS: &str = "souther_date_add_years";
/// `Date.daysBetween`.
pub const DATE_DAYS_BETWEEN: &str = "souther_date_days_between";
/// `Date.year`.
pub const DATE_YEAR: &str = "souther_date_year";
/// `Date.month`.
pub const DATE_MONTH: &str = "souther_date_month";
/// `Date.day`.
pub const DATE_DAY: &str = "souther_date_day";
/// `Date.fromParts`, into room for the `Date` where the parts name one.
pub const DATE_FROM_PARTS: &str = "souther_date_from_parts";
/// `Time.fromParts`, into room for the `Time` where the parts name one.
pub const TIME_FROM_PARTS: &str = "souther_time_from_parts";
/// `Time.hour`.
pub const TIME_HOUR: &str = "souther_time_hour";
/// `Time.minute`.
pub const TIME_MINUTE: &str = "souther_time_minute";
/// `Time.second`.
pub const TIME_SECOND: &str = "souther_time_second";
/// `DateTime.addMinutes`, as [`DATE_ADD_DAYS`].
pub const DATETIME_ADD_MINUTES: &str = "souther_datetime_add_minutes";
/// `DateTime.addHours`, as [`DATE_ADD_DAYS`].
pub const DATETIME_ADD_HOURS: &str = "souther_datetime_add_hours";
/// `DateTime.addDays`, as [`DATE_ADD_DAYS`].
pub const DATETIME_ADD_DAYS: &str = "souther_datetime_add_days";
/// `DateTime.minutesBetween`.
pub const DATETIME_MINUTES_BETWEEN: &str = "souther_datetime_minutes_between";
/// `DateTime.toDate`.
pub const DATETIME_TO_DATE: &str = "souther_datetime_to_date";
/// `DateTime.toTime`.
pub const DATETIME_TO_TIME: &str = "souther_datetime_to_time";
/// `DateTime.fromDateAndTime`.
pub const DATETIME_FROM_DATE_AND_TIME: &str = "souther_datetime_from_date_and_time";

/// The symbols a `Set` and a `Map` are built and read through.
///
/// A set and a map are an address and nothing generated code reads behind, for the reason a
/// `Decimal` is: what the runtime keeps one as is a trie of its own, and a layout stated here would
/// make every change to how it is kept a change of this contract. A list states its layout because
/// generated code walks one; nothing walks a set but the runtime, so none is stated.
///
/// What an element is equal to, and what it hashes to, is the language's and differs by type, and
/// the runtime knows no type. So generated code hands over a function for each ([`Word::Hasher`],
/// [`Word::Equality`]), of the element's type at the site asking, over a value as it stands in a
/// slot. Neither is kept in the collection: an element stands as a wider type than the one it was
/// put in as (a `Set<A>` as a `Set<S>`, spec §collections), and a value of another case of `S` is
/// then asked for under `S`'s functions. What is kept is the hash each element was put in under,
/// which [`HASHING`] holds to be the hash of it under any type it stands as.
///
/// Which member a set keeps of two equal ones, and in what order one is listed, is the runtime's
/// and nothing else says it (spec §stdlib-set): the listing is the trie's, and it is neither the
/// order the members were put in nor the one a boundary writes.
pub const SET_EMPTY: &str = "souther_set_empty";
/// A set with one more member, or the set itself where it holds one equal to it, through room for
/// it: nothing is written where one more is more members than a set holds, and the caller ends the
/// run for the reason the kernel names. So for [`SET_UNION`] and [`MAP_INSERT`].
pub const SET_INSERT: &str = "souther_set_insert";
/// A set without the member equal to this, or the set itself where it holds none.
pub const SET_REMOVE: &str = "souther_set_remove";
/// Whether a set holds a member equal to this.
pub const SET_CONTAINS: &str = "souther_set_contains";
/// Every member of either set, the larger's where both hold one.
pub const SET_UNION: &str = "souther_set_union";
/// The members both sets hold, the smaller's.
pub const SET_INTERSECTION: &str = "souther_set_intersection";
/// The first set's members the second does not hold.
pub const SET_DIFFERENCE: &str = "souther_set_difference";
/// How many members a set holds.
pub const SET_SIZE: &str = "souther_set_size";
/// A set's members as a list, in the trie's order.
pub const SET_TO_LIST: &str = "souther_set_to_list";
/// The set of a list's elements, one of each that are equal.
pub const SET_FROM_LIST: &str = "souther_set_from_list";
/// Whether two sets hold the same members, in whatever order each keeps them.
pub const SET_EQUAL: &str = "souther_set_equal";
/// A set's hash, which is its members' and not the order they are kept in ([`HASHING`]).
pub const SET_HASH: &str = "souther_set_hash";
/// The empty map.
pub const MAP_EMPTY: &str = "souther_map_empty";
/// The address of the value a map holds under a key equal to this, or [`NOTHING`]: what an
/// optional of the value is, so what it answers is `Map.get`'s answer as it stands.
pub const MAP_GET: &str = "souther_map_get";
/// Whether a map holds a key equal to this.
pub const MAP_CONTAINS_KEY: &str = "souther_map_contains_key";
/// A map's keys as a list, in the trie's order.
pub const MAP_KEYS: &str = "souther_map_keys";
/// A map's values as a list, in the order [`MAP_KEYS`] lists their keys.
pub const MAP_VALUES: &str = "souther_map_values";
/// A map with this key holding this value, in place of what a key equal to it held, which stays the
/// key the map holds.
pub const MAP_INSERT: &str = "souther_map_insert";
/// A map without the key equal to this, or the map itself where it holds none.
pub const MAP_REMOVE: &str = "souther_map_remove";
/// How many keys a map holds.
pub const MAP_SIZE: &str = "souther_map_size";
/// A map's entries as a list of pairs, each a tuple of the layout [`member_at`] states, in the
/// order [`MAP_KEYS`] lists the keys.
pub const MAP_TO_LIST: &str = "souther_map_to_list";
/// The map of a list of pairs, a later pair's value winning where two keys are equal, under the
/// earlier pair's key.
pub const MAP_FROM_LIST: &str = "souther_map_from_list";
/// Whether two maps hold equal values under the same keys.
pub const MAP_EQUAL: &str = "souther_map_equal";
/// A map's hash, from its keys' kept hashes and its values' hashes under the function handed over.
pub const MAP_HASH: &str = "souther_map_hash";

/// The symbols a value's hash is worked out through, where the runtime holds what decides it or
/// where the step is one both sides have to take alike.
///
/// A hash is kept in a set or a map, which one object builds and another reads, so what a value
/// hashes to is part of the contract between the objects of a generation and not only of one
/// object's: [`HASHING`] says how generated code composes one, and these are the steps it composes
/// with.
pub const HASH_COMBINE: &str = "souther_hash_combine";
/// A string's hash, from its bytes: two strings are equal where their bytes are.
pub const STRING_HASH: &str = "souther_string_hash";
/// A `Decimal`'s hash, from its amount and not its scale: `1.0` and `1.00` are equal.
pub const DECIMAL_HASH: &str = "souther_decimal_hash";
/// A `Rational`'s hash, from the one form each value is kept in.
pub const RATIONAL_HASH: &str = "souther_rational_hash";
/// A `Date`'s hash, from the day it names.
pub const DATE_HASH: &str = "souther_date_hash";
/// A `Time`'s hash, from the second of the day it names.
pub const TIME_HASH: &str = "souther_time_hash";
/// A `DateTime`'s hash, from the second it names.
pub const DATETIME_HASH: &str = "souther_datetime_hash";
/// An `Instant`'s hash, from the second and the nanosecond it names.
pub const INSTANT_HASH: &str = "souther_instant_hash";

/// How generated code composes a value's hash, which every object of a generation composes alike.
///
/// Each line is one kind of type, and each composes from [`HASH_START`] with [`HASH_COMBINE`]:
///
/// - `Int` and `Bool`: the number, combined once. A truth is 0 or 1.
/// - `String`, `Decimal`, `Rational` and the temporals: the runtime's hash of it (above).
/// - A value that carries its token (a declared type, a case a union carries, a case the language
///   gives): the token's address combined first, then each field in order as its own type, or what
///   a union's primitive case carries as that primitive. A sum and a union hash as the case the
///   value is, so a value hashes the same as its case and as every sum it stands as: that is what
///   lets a `Set<A>` stand as a `Set<S>` with the hashes it was built with.
/// - An optional: [`HASH_START`] where it is absent, and what it holds combined with
///   [`HASH_PRESENT`] where it is not.
/// - A tuple: each member in order. A list: its length, then each element in order.
/// - A set: [`SET_HASH`]. A map: [`MAP_HASH`], handed the values' hasher.
/// - The type of what has no value: [`HASH_START`], which nothing asks. What holds only it hashes as
///   what holds anything else does, since it stands as that without being rebuilt.
///
/// Two equal values hash alike under every line, which is what the runtime asks of a hash. A
/// change to any line is a change of what a set built by one object is to another, and moves
/// [`ABI_GENERATION`].
pub const HASHING: &[&str] = &[
    "Int, Bool: combine(START, n)",
    "String, Decimal, Rational, Date, Time, DateTime, Instant: the runtime's hash",
    "tagged: combine(START, token), then each field or what is carried",
    "sum, union: as the case the value is",
    "optional: START if absent, else combine(PRESENT, held)",
    "tuple: each member from START; list: combine(START, length), then each element",
    "set: souther_set_hash; map: souther_map_hash(values' hasher)",
    "what has no value: START; what holds only it, as what holds anything",
];

/// What a hash is composed from.
pub const HASH_START: i64 = 0;

/// What an optional holding a value combines what it holds with.
pub const HASH_PRESENT: i64 = 1;

/// The symbol generated code takes room from.
///
/// It answers a pointer to `size` bytes that stay valid until the mark below them is reset. A
/// Souther value is never freed on its own: what a run makes is dropped in one go by the caller
/// that bracketed the call, so nothing generated has to know what owns what.
pub const ALLOCATE: &str = "souther_alloc";

/// The symbol a host opens a scope with: the stretch of a run that everything the library answers
/// inside it lives for, answered as a token to close it with.
///
/// What a host brackets its calls with, and the one thing of the arena it is told. A value the
/// library answers is good on the thread that made it, until the scope it was made in is closed,
/// and on no other thread; scopes on a thread close in the order opposite to the one they opened
/// in; a token closes the scope it was answered for once. The runtime holds all of that itself
/// ([`SCOPE_CONTRACT`]): closing with a token that is not the innermost open scope of the calling
/// thread — one never answered, one already closed, one opened later than a scope still open
/// inside it, or one another thread opened — is answered as that and changes nothing. How far the
/// arena stood is the runtime's own and never crosses: a host that held a position could hand
/// back one the arena never stood at, and nothing could tell.
pub const SCOPE_OPEN: &str = "souther_scope_open";

/// The symbol a host closes a scope with, dropping everything made inside it, and answering whether
/// the token was the innermost open scope of the calling thread.
pub const SCOPE_CLOSE: &str = "souther_scope_close";

/// What a host function takes of what a host hands it, in the words a generation records.
///
/// A host hands over data and handles. Data ([`HostWord::is_datum`]) is a number, a truth, a count,
/// a scope's token, or the bytes a pointer and a count describe: anything a host can have wrong
/// while holding it rightly, and everything the runtime can tell is wrong by looking. So every
/// function a host calls, the runtime's and every one generated for a library, answers every datum,
/// and refuses one it does not take by what it answers — `false`, an absent value, a reading that
/// came to nothing — and never by ending the process. A handle is an address the library answered:
/// what it points at is the library's, and the runtime cannot tell one that is not what it was
/// answered as without reading it as that. So a handle a function takes is one the library
/// answered, of the type the function names, whose scope is still open on the calling thread; that
/// is the one thing a host holds to, as a C caller holds a pointer to what it points at.
pub const HOST_INPUT_CONTRACT: &[(&str, &str)] = &[
    (
        "datum",
        "answered whatever it is; one a function does not take is refused by what it answers, \
         and ends nothing",
    ),
    (
        "bytes",
        "read for as many as the count says where the count is above nought; a count below \
         nought is a datum refused, and what the bytes say is a datum",
    ),
    ("bool", "a byte, true where it is not nought"),
    (
        "handle",
        "one the library answered, of the type the function names, whose scope is open on the \
         calling thread",
    ),
];

/// What a scope promises and what the runtime holds of it, in the words a generation records.
///
/// Written as data so that the record of a generation holds it: a change to any of these is a
/// change to what a host's runtime was written against, however little the functions' types say
/// of it.
pub const SCOPE_CONTRACT: &[(&str, &str)] = &[
    ("arena", "one to each thread"),
    (
        "value-life",
        "until the scope it was made in closes, on the thread that made it",
    ),
    (
        "close-order",
        "the innermost open scope of the calling thread, and no other",
    ),
    ("close-refused", "answered as false, and nothing changes"),
    ("token", "never answered twice, on any thread"),
];

/// The runtime's external form: what a value is written as at a boundary, built as a tree by
/// generated code and written out as JSON in one step.
///
/// The tree is the runtime's own and lives on its heap, not in the arena. Every constructor hands
/// the caller a form it owns; `EXTERNAL_APPEND` and `EXTERNAL_PUT` take ownership of the item they
/// are given and leave the container with the caller; `EXTERNAL_JSON` takes the root, drops the
/// whole tree, and answers a string of the runtime's own layout (`TEXT_LENGTH`, `TEXT_BYTES`) in
/// the arena. So nothing of the tree outlives the call that writes it, and what closing a scope drops is
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
/// `(decimal) -> form`: the amount, written with as few digits as it is written with and not at
/// the scale it carries (spec §primitives).
pub const EXTERNAL_DECIMAL: &str = "souther_external_decimal";
/// `(date) -> form`: the text the temporal is written as at a boundary, which is what a `Date`
/// reads as (spec §primitives).
pub const EXTERNAL_DATE: &str = "souther_external_date";
/// `(time) -> form`, as [`EXTERNAL_DATE`].
pub const EXTERNAL_TIME: &str = "souther_external_time";
/// `(datetime) -> form`, as [`EXTERNAL_DATE`].
pub const EXTERNAL_DATETIME: &str = "souther_external_datetime";
/// `(instant) -> form`, as [`EXTERNAL_DATE`], in UTC.
pub const EXTERNAL_INSTANT: &str = "souther_external_instant";
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
/// `(array)`: a set's members, which the caller still owns, put in the order a boundary writes them
/// in, ascending by their own external representation (spec §collections), so two equal sets write
/// one text. Asked once every member is in the array and each is in that order itself.
pub const EXTERNAL_ORDER: &str = "souther_external_order";
/// `(array)`: a map's entries, an array the caller still owns of pairs each of a string and a
/// value, made the object a boundary writes, its members ascending by their keys. Asked once every
/// entry is in it.
pub const EXTERNAL_ENTRIES: &str = "souther_external_entries";

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
/// `(node) -> i64`: how many members an object holds, asked of one `READ_OBJECT` said is one. A
/// key written twice is two members: a map read from them says so where the two are one key
/// (`READ_DUPLICATE_KEY`).
pub const READ_MEMBERS: &str = "souther_read_members";
/// `(node, i64) -> node`: the key of an object's member at an index below its count, as a place
/// of the document holding the text it was written as: what a map's key is read from, as the key's
/// type is read anywhere else.
pub const READ_MEMBER_KEY: &str = "souther_read_member_key";
/// `(node, i64) -> node`: what an object's member at an index below its count holds.
pub const READ_MEMBER_VALUE: &str = "souther_read_member_value";
/// `(path, node, i64) -> path`: the place of an object's member at an index, by its key as it was
/// written.
pub const PATH_BELOW_MEMBER: &str = "souther_path_below_member";
/// `(path, reading)`: two of a map's keys are one key once each is read as the key's type.
pub const READ_DUPLICATE_KEY: &str = "souther_read_duplicate_key";
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
/// `(node, path, reading, out) -> i8`: a `Decimal` written through `out`, at the scale the number
/// was spelt at.
pub const READ_DECIMAL: &str = "souther_read_decimal";
/// `(node, path, reading, out) -> i8`: a `Date` written through `out`, where the node is text that
/// names one.
pub const READ_DATE: &str = "souther_read_date";
/// `(node, path, reading, out) -> i8`: a `Time`, as [`READ_DATE`].
pub const READ_TIME: &str = "souther_read_time";
/// `(node, path, reading, out) -> i8`: a `DateTime`, as [`READ_DATE`].
pub const READ_DATETIME: &str = "souther_read_datetime";
/// `(node, path, reading, out) -> i8`: an `Instant`, as [`READ_DATE`], from text written in UTC or
/// with an offset, as the moment it names.
pub const READ_INSTANT: &str = "souther_read_instant";
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
/// `(path, reading, string, i64) -> i8`: whether the text holds at least that many characters,
/// having recorded Raoh's `too_short` where it does not.
pub const READ_MIN_LENGTH: &str = "souther_read_min_length";
/// `(path, reading, string, i64) -> i8`: at most that many, `too_long`.
pub const READ_MAX_LENGTH: &str = "souther_read_max_length";
/// `(path, reading, string, i64) -> i8`: exactly that many, `invalid_length`.
pub const READ_FIXED_LENGTH: &str = "souther_read_fixed_length";
/// `(path, reading, string, machine, written string) -> i8`: whether the pattern matches the whole
/// text, `invalid_format` with the pattern as written.
pub const READ_PATTERN: &str = "souther_read_pattern";
/// `(path, reading, i64, i64) -> i8`: at least the bound, `out_of_range.minimum`.
pub const READ_INT_MIN: &str = "souther_read_int_min";
/// `(path, reading, i64, i64) -> i8`: at most the bound, `out_of_range.maximum`.
pub const READ_INT_MAX: &str = "souther_read_int_max";
/// `(path, reading, i64) -> i8`: above nought, `out_of_range.positive`.
pub const READ_INT_POSITIVE: &str = "souther_read_int_positive";
/// `(path, reading, i64) -> i8`: not below nought, `out_of_range.non_negative`.
pub const READ_INT_NON_NEGATIVE: &str = "souther_read_int_non_negative";
/// `(path, reading, decimal, decimal) -> i8`: at least the bound by amount.
pub const READ_DECIMAL_MIN: &str = "souther_read_decimal_min";
/// `(path, reading, decimal, decimal) -> i8`: at most the bound by amount.
pub const READ_DECIMAL_MAX: &str = "souther_read_decimal_max";
/// `(path, reading, decimal) -> i8`: above nought.
pub const READ_DECIMAL_POSITIVE: &str = "souther_read_decimal_positive";
/// `(path, reading, decimal) -> i8`: not below nought.
pub const READ_DECIMAL_NON_NEGATIVE: &str = "souther_read_decimal_non_negative";
/// `(path, reading, list) -> i8`: one element or more, `too_small.nonempty`.
pub const READ_LIST_NON_EMPTY: &str = "souther_read_list_non_empty";
/// `(path, reading, list, i64) -> i8`: at least that many elements, `too_small`.
pub const READ_LIST_MIN_SIZE: &str = "souther_read_list_min_size";
/// `(path, reading, list, i64) -> i8`: at most that many, `too_big`.
pub const READ_LIST_MAX_SIZE: &str = "souther_read_list_max_size";
/// `(path, reading, list, i64) -> i8`: exactly that many, `invalid_size`.
pub const READ_LIST_FIXED_SIZE: &str = "souther_read_list_fixed_size";
/// `(list, hasher, equality) -> list`: the elements the list holds more than once, each once, in
/// the order their repetition was found.
pub const LIST_DUPLICATES: &str = "souther_list_duplicates";
/// `(path, reading, form)`: a list held no element twice and does, `duplicate_element` with the
/// form of the elements it repeats, which it takes.
pub const READ_DUPLICATES: &str = "souther_read_duplicates";
/// `(path, reading, map) -> i8`: one entry or more, `too_small.nonempty`.
pub const READ_MAP_NON_EMPTY: &str = "souther_read_map_non_empty";
/// `(path, reading, map, i64) -> i8`: at least that many entries, `too_small`.
pub const READ_MAP_MIN_SIZE: &str = "souther_read_map_min_size";
/// `(path, reading, map, i64) -> i8`: at most that many, `too_big`.
pub const READ_MAP_MAX_SIZE: &str = "souther_read_map_max_size";

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
/// `(issue) -> string`: the key a resolver picks its wording by, Raoh's, and the code where Raoh
/// gives none of its own.
pub const ISSUE_MESSAGE_KEY: &str = "souther_issue_message_key";
/// `(issue) -> string`: a JSON Pointer, empty for the document's root.
pub const ISSUE_PATH: &str = "souther_issue_path";
/// `(issue) -> string`: what Raoh calls its metadata, as the JSON object it is.
pub const ISSUE_META: &str = "souther_issue_meta";

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
    /// An open scope, as [`SCOPE_OPEN`] answers one and [`SCOPE_CLOSE`] takes it back.
    Scope,
    /// Bytes the host holds, read and never kept.
    Bytes,
    /// The address of a value of a declared type, which a host never reads behind.
    Value,
    /// The address of text of the runtime's layout, read through [`STRING_LENGTH`] and
    /// [`STRING_BYTES`].
    String,
    /// The address of a `Decimal`, which a host never reads behind: made through
    /// [`DECIMAL_OF_PARTS`], and read through [`DECIMAL_UNSCALED`] and [`DECIMAL_SCALE`].
    Decimal,
    /// The address of a `Date`, which a host never reads behind: made through [`DATE_OF_PARTS`]
    /// and read through [`DATE_PARTS`].
    Date,
    /// The address of a `Time`: made through [`TIME_OF_PARTS`] and read through [`TIME_PARTS`].
    Time,
    /// The address of a `DateTime`: made through [`DATETIME_OF_PARTS`] and read through
    /// [`DATETIME_PARTS`].
    DateTime,
    /// The address of an `Instant`: made through [`INSTANT_OF_PARTS`] and read through
    /// [`INSTANT_PARTS`].
    Instant,
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
    /// The address of a function value, which a host never reads behind and calls through the
    /// function the object defines for the shape it crosses in ([`host_function_symbol`]). A word
    /// of its own for the reason [`HostWord::List`] is.
    Function,
}

/// What a word is on the machine, which the driver's machine types and the header's C types are
/// both read off, so that the two cannot say different things and a change to either is a change
/// here, which the record of a generation holds.
///
/// Unsigned and signed are said apart though a machine integer has no sign, because a host's
/// language does: a `Bool` is a byte a host reads as nought or one, and an `Outcome` a number a
/// host may be handed below nought.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Representation {
    /// Eight bits, read without a sign.
    U8,
    /// Thirty-two bits, read without a sign.
    U32,
    /// Thirty-two bits, read with a sign.
    I32,
    /// Sixty-four bits, read with a sign.
    I64,
    /// An address, as wide as the machine's.
    Address,
}

impl Representation {
    /// The type `<stdint.h>` names for it, or `void *` for an address a header does not name
    /// otherwise.
    pub const fn c(self) -> &'static str {
        match self {
            Representation::U8 => "uint8_t",
            Representation::U32 => "uint32_t",
            Representation::I32 => "int32_t",
            Representation::I64 => "int64_t",
            Representation::Address => "void *",
        }
    }
}

impl HostWord {
    /// Every word, in the order they are declared.
    pub const ALL: &[HostWord] = &[
        HostWord::Status,
        HostWord::Int,
        HostWord::Bool,
        HostWord::Case,
        HostWord::Outcome,
        HostWord::Count,
        HostWord::Scope,
        HostWord::Bytes,
        HostWord::Value,
        HostWord::String,
        HostWord::Decimal,
        HostWord::Date,
        HostWord::Time,
        HostWord::DateTime,
        HostWord::Instant,
        HostWord::Decoded,
        HostWord::Issue,
        HostWord::List,
        HostWord::Requirements,
        HostWord::Capability,
        HostWord::Userdata,
        HostWord::Function,
    ];

    /// Whether a host hands this word over as data, which every host function answers whatever
    /// it is ([`HOST_INPUT_CONTRACT`]), and not as a handle to what the library holds.
    pub const fn is_datum(self) -> bool {
        match self {
            HostWord::Status
            | HostWord::Int
            | HostWord::Bool
            | HostWord::Case
            | HostWord::Outcome
            | HostWord::Count
            | HostWord::Scope
            | HostWord::Bytes => true,
            HostWord::Value
            | HostWord::String
            | HostWord::Decimal
            | HostWord::Date
            | HostWord::Time
            | HostWord::DateTime
            | HostWord::Instant
            | HostWord::Decoded
            | HostWord::Issue
            | HostWord::List
            | HostWord::Requirements
            | HostWord::Capability
            | HostWord::Userdata
            | HostWord::Function => false,
        }
    }

    /// What the word is on the machine.
    pub const fn representation(self) -> Representation {
        match self {
            HostWord::Bool => Representation::U8,
            HostWord::Status | HostWord::Case => Representation::U32,
            HostWord::Outcome => Representation::I32,
            HostWord::Int | HostWord::Count | HostWord::Scope => Representation::I64,
            HostWord::Bytes
            | HostWord::Value
            | HostWord::String
            | HostWord::Decimal
            | HostWord::Date
            | HostWord::Time
            | HostWord::DateTime
            | HostWord::Instant
            | HostWord::Decoded
            | HostWord::Issue
            | HostWord::List
            | HostWord::Requirements
            | HostWord::Capability
            | HostWord::Userdata
            | HostWord::Function => Representation::Address,
        }
    }

    /// The word as a symbol and a manifest spell it: its name, in lower case.
    pub const fn spelt(self) -> &'static str {
        match self {
            HostWord::Status => "status",
            HostWord::Int => "int",
            HostWord::Bool => "bool",
            HostWord::Case => "case",
            HostWord::Outcome => "outcome",
            HostWord::Count => "count",
            HostWord::Scope => "scope",
            HostWord::Bytes => "bytes",
            HostWord::Value => "value",
            HostWord::String => "string",
            HostWord::Decimal => "decimal",
            HostWord::Date => "date",
            HostWord::Time => "time",
            HostWord::DateTime => "datetime",
            HostWord::Instant => "instant",
            HostWord::Decoded => "decoded",
            HostWord::Issue => "issue",
            HostWord::List => "list",
            HostWord::Requirements => "requirements",
            HostWord::Capability => "capability",
            HostWord::Userdata => "userdata",
            HostWord::Function => "function",
        }
    }
}

/// One word a value of the model is itself handed over as: the leaves of a [`HostShape`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HostLeaf {
    /// An `Int`.
    Int,
    /// A `Bool`.
    Bool,
    /// A `String`, as the address of text of the runtime's layout.
    String,
    /// A `Decimal`, as the address of what the runtime keeps one as, which a host never reads
    /// behind ([`HostWord::Decimal`]).
    Decimal,
    /// A `Date`, as the address of what the runtime keeps one as ([`HostWord::Date`]).
    Date,
    /// A `Time` ([`HostWord::Time`]).
    Time,
    /// A `DateTime` ([`HostWord::DateTime`]).
    DateTime,
    /// An `Instant` ([`HostWord::Instant`]).
    Instant,
    /// The address of a value of a declared type or of a union, which a host never reads behind.
    Value,
}

impl HostLeaf {
    /// The word it is handed over as.
    pub const fn word(self) -> HostWord {
        match self {
            HostLeaf::Int => HostWord::Int,
            HostLeaf::Bool => HostWord::Bool,
            HostLeaf::String => HostWord::String,
            HostLeaf::Decimal => HostWord::Decimal,
            HostLeaf::Date => HostWord::Date,
            HostLeaf::Time => HostWord::Time,
            HostLeaf::DateTime => HostWord::DateTime,
            HostLeaf::Instant => HostWord::Instant,
            HostLeaf::Value => HostWord::Value,
        }
    }
}

/// The shape a value of the model crosses between a host and the object in: the words it is handed
/// over as, and what those words are made of.
///
/// Not a type of the model. A tuple and a pair of fields a type is written as elsewhere would both
/// be a [`HostShape::Product`], and a value of a declared type and one of a union are both one
/// [`HostLeaf::Value`]: what is said here is how a value is handed over and taken back, and what it
/// is, is the model's to say. So one shape serves every type that crosses in it, and a function a
/// host reaches a list or a function value through is one for each shape and not for each type.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum HostShape {
    /// One word that is the value itself.
    Leaf(HostLeaf),
    /// An optional: whether there is a value, as a [`HostWord::Bool`] that is nought or one, and
    /// then the words of the value, which are read only where there is one and written only where
    /// there is one. Each optional says so of itself, so an optional of an optional is two
    /// presences, and absence at one depth is not absence at another.
    Option(Box<HostShape>),
    /// A value made of others, handed over as each of them in order, one after another. A tuple
    /// crosses as one.
    Product(Vec<HostShape>),
    /// A list, as the address of it ([`HostWord::List`]), built and read through the functions for
    /// the shape its element crosses in ([`host_list_symbol`]).
    List(Box<HostShape>),
    /// A function value, as the address of it ([`HostWord::Function`]), called and made through
    /// the functions for its shape ([`host_function_symbol`]): what it takes, each as a host hands
    /// it over, and what it answers, as a host is handed it.
    Function {
        /// What it takes, in order.
        takes: Vec<HostShape>,
        /// What it answers.
        answers: Box<HostShape>,
    },
}

impl HostShape {
    /// The words a value of this is handed over as, in order.
    pub fn words(&self) -> Vec<HostWord> {
        let mut words = Vec::new();
        self.words_into(&mut words);
        words
    }

    fn words_into(&self, words: &mut Vec<HostWord>) {
        match self {
            HostShape::Leaf(leaf) => words.push(leaf.word()),
            HostShape::Option(of) => {
                words.push(HostWord::Bool);
                of.words_into(words);
            }
            HostShape::Product(members) => {
                for member in members {
                    member.words_into(words);
                }
            }
            HostShape::List(_) => words.push(HostWord::List),
            HostShape::Function { .. } => words.push(HostWord::Function),
        }
    }

    /// The shape as a symbol spells it: a leaf as its word ([`HostWord::spelt`]), and anything
    /// else as a mark and then what it is made of, each after a `_` — `o` for an optional, `t` and
    /// the count of members for a product, `l` for a list, `f` and the count of what it takes for a
    /// function, its answer last. Read from the left each mark says how many shapes follow it, so
    /// two shapes are never spelt alike, and every mark is a letter no word is spelt as.
    pub fn spelt(&self) -> String {
        let mut spelt = String::new();
        self.spelt_into(&mut spelt);
        spelt
    }

    fn spelt_into(&self, spelt: &mut String) {
        let then = |spelt: &mut String, shape: &HostShape| {
            spelt.push('_');
            shape.spelt_into(spelt);
        };
        match self {
            HostShape::Leaf(leaf) => spelt.push_str(leaf.word().spelt()),
            HostShape::Option(of) => {
                spelt.push('o');
                then(spelt, of);
            }
            HostShape::Product(members) => {
                spelt.push_str(&format!("t{}", members.len()));
                for member in members {
                    then(spelt, member);
                }
            }
            HostShape::List(element) => {
                spelt.push('l');
                then(spelt, element);
            }
            HostShape::Function { takes, answers } => {
                spelt.push_str(&format!("f{}", takes.len()));
                for taken in takes {
                    then(spelt, taken);
                }
                then(spelt, answers);
            }
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
    use HostParameter::{Given, Room};
    use HostWord::{
        Bool, Bytes, Count, Date, DateTime, Decimal, Decoded, Instant, Int, Issue, Outcome, Scope,
        String, Time, Value,
    };
    &[
        RuntimeFunction {
            name: SCOPE_OPEN,
            takes: &[],
            answers: Some(Scope),
        },
        RuntimeFunction {
            name: SCOPE_CLOSE,
            takes: &[Given(Scope)],
            answers: Some(Bool),
        },
        RuntimeFunction {
            name: STRING_OF_UTF8,
            takes: &[Given(Bytes), Given(Count), Room(String)],
            answers: Some(Bool),
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
            name: DECIMAL_OF_PARTS,
            takes: &[Given(Bytes), Given(Count), Given(Int), Room(Decimal)],
            answers: Some(Bool),
        },
        RuntimeFunction {
            name: DECIMAL_UNSCALED,
            takes: &[Given(Decimal)],
            answers: Some(String),
        },
        RuntimeFunction {
            name: DECIMAL_SCALE,
            takes: &[Given(Decimal)],
            answers: Some(Int),
        },
        RuntimeFunction {
            name: DATE_OF_PARTS,
            takes: &[Given(Int), Given(Int), Given(Int), Room(Date)],
            answers: Some(Bool),
        },
        RuntimeFunction {
            name: DATE_PARTS,
            takes: &[Given(Date), Room(Int), Room(Int), Room(Int)],
            answers: None,
        },
        RuntimeFunction {
            name: TIME_OF_PARTS,
            takes: &[Given(Int), Given(Int), Given(Int), Room(Time)],
            answers: Some(Bool),
        },
        RuntimeFunction {
            name: TIME_PARTS,
            takes: &[Given(Time), Room(Int), Room(Int), Room(Int)],
            answers: None,
        },
        RuntimeFunction {
            name: DATETIME_OF_PARTS,
            takes: &[
                Given(Int),
                Given(Int),
                Given(Int),
                Given(Int),
                Given(Int),
                Given(Int),
                Room(DateTime),
            ],
            answers: Some(Bool),
        },
        RuntimeFunction {
            name: DATETIME_PARTS,
            takes: &[
                Given(DateTime),
                Room(Int),
                Room(Int),
                Room(Int),
                Room(Int),
                Room(Int),
                Room(Int),
            ],
            answers: None,
        },
        RuntimeFunction {
            name: INSTANT_OF_PARTS,
            takes: &[Given(Int), Given(Int), Room(Instant)],
            answers: Some(Bool),
        },
        RuntimeFunction {
            name: INSTANT_PARTS,
            takes: &[Given(Instant), Room(Int), Room(Int)],
            answers: None,
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
            name: ISSUE_MESSAGE_KEY,
            takes: &[Given(Issue)],
            answers: Some(String),
        },
        RuntimeFunction {
            name: ISSUE_PATH,
            takes: &[Given(Issue)],
            answers: Some(String),
        },
        RuntimeFunction {
            name: ISSUE_META,
            takes: &[Given(Issue)],
            answers: Some(String),
        },
    ]
};

/// How a host makes a value of one case in [`BUILT_IN_CASES`] that a host can cross, as a union holds one, and reads back
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
    /// `(value, room for what it holds) -> bool`, for a case that holds something: what the value
    /// holds is written where the value is this case, and whether it is is answered. Which case a
    /// value is, is read off the value, so a value of another case is answered as that.
    pub read: Option<RuntimeFunction>,
}

/// How a host makes and reads each case in [`BUILT_IN_CASES`] that has an external form, in the
/// order that names them. The runtime's own tests hold each function to the one it names, and the
/// cases to that table.
pub const HOST_CASES: &[CaseCrossing] = {
    use HostParameter::{Given, Room};
    use HostWord::{Bool, Date, DateTime, Decimal, Instant, Int, String, Time, Value};
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
                takes: match word {
                    Int => &[Given(Value), Room(Int)],
                    Bool => &[Given(Value), Room(Bool)],
                    String => &[Given(Value), Room(String)],
                    Decimal => &[Given(Value), Room(Decimal)],
                    Date => &[Given(Value), Room(Date)],
                    Time => &[Given(Value), Room(Time)],
                    DateTime => &[Given(Value), Room(DateTime)],
                    Instant => &[Given(Value), Room(Instant)],
                    _ => panic!("a case holds a primitive"),
                },
                answers: Some(Bool),
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
        holding(
            "Decimal",
            "souther_case_decimal_make",
            "souther_case_decimal_read",
            &[Given(Decimal)],
            Decimal,
        ),
        holding(
            "Date",
            "souther_case_date_make",
            "souther_case_date_read",
            &[Given(Date)],
            Date,
        ),
        holding(
            "Time",
            "souther_case_time_make",
            "souther_case_time_read",
            &[Given(Time)],
            Time,
        ),
        holding(
            "DateTime",
            "souther_case_datetime_make",
            "souther_case_datetime_read",
            &[Given(DateTime)],
            DateTime,
        ),
        holding(
            "Instant",
            "souther_case_instant_make",
            "souther_case_instant_read",
            &[Given(Instant)],
            Instant,
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
    /// An exact rational, which only generated code and the runtime ever hold: it has no external
    /// form, so it is no [`HostWord`] and no function a host calls takes or answers one.
    Rational,
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
    /// The address of a set, which only the runtime reads behind ([`SET_EMPTY`]).
    Set,
    /// The address of a map, which only the runtime reads behind ([`MAP_EMPTY`]).
    Map,
    /// A value's hash, as [`HASHING`] composes one: sixty-four bits.
    Hash,
    /// The address of the slot a value is held at, or [`NOTHING`]: an optional of that value.
    Held,
    /// The address of a function of generated code's taking a value as it stands in a slot and
    /// answering its [`Word::Hash`], which the runtime calls.
    Hasher,
    /// The address of a function of generated code's taking two values as they stand in slots and
    /// answering whether they are equal, one byte, which the runtime calls.
    Equality,
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
    use HostWord::{
        Bool, Bytes, Count, Date, DateTime, Decimal, Decoded, Instant, Int, List, String, Time,
        Value,
    };
    use Parameter::{Given, Room};
    use Word::{
        Comparison, Equality, Form, Hash, Hasher, Held, Host, Machine, Map, Memory, Node, Path,
        Rational, Set,
    };
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
            takes: &[Given(Host(String)), Given(Host(String)), Room(Host(String))],
            answers: Some(Host(Bool)),
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
            takes: &[Given(Host(String)), Room(Host(String))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_UPPERCASE,
            takes: &[Given(Host(String)), Room(Host(String))],
            answers: Some(Host(Bool)),
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
            takes: &[Given(Host(String)), Given(Host(List)), Room(Host(String))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_CONCAT_ALL,
            takes: &[Given(Host(List)), Room(Host(String))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_REPLACE,
            takes: &[
                Given(Host(String)),
                Given(Host(String)),
                Given(Host(String)),
                Room(Host(String)),
            ],
            answers: Some(Host(Bool)),
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
            takes: &[Given(Host(String)), Room(Host(String))],
            answers: Some(Host(Bool)),
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
            name: STRING_TO_DECIMAL,
            takes: &[Given(Host(String)), Room(Host(Decimal))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: STRING_FROM_DECIMAL,
            takes: &[Given(Host(Decimal)), Room(Host(String))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DECIMAL_LITERAL,
            takes: &[Given(Host(String)), Given(Host(Int))],
            answers: Some(Host(Decimal)),
        },
        GeneratedCall {
            name: DECIMAL_COMPARE,
            takes: &[Given(Host(Decimal)), Given(Host(Decimal))],
            answers: Some(Comparison),
        },
        GeneratedCall {
            name: DECIMAL_IS_ZERO,
            takes: &[Given(Host(Decimal))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DECIMAL_NEGATE,
            takes: &[Given(Host(Decimal))],
            answers: Some(Host(Decimal)),
        },
        GeneratedCall {
            name: DECIMAL_ADD,
            takes: &[
                Given(Host(Decimal)),
                Given(Host(Decimal)),
                Room(Host(Decimal)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DECIMAL_SUBTRACT,
            takes: &[
                Given(Host(Decimal)),
                Given(Host(Decimal)),
                Room(Host(Decimal)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DECIMAL_MULTIPLY,
            takes: &[
                Given(Host(Decimal)),
                Given(Host(Decimal)),
                Room(Host(Decimal)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DECIMAL_FROM_INT,
            takes: &[Given(Host(Int))],
            answers: Some(Host(Decimal)),
        },
        GeneratedCall {
            name: DECIMAL_TO_INT,
            takes: &[Given(Host(Value)), Given(Host(Decimal)), Room(Host(Int))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DECIMAL_ROUND,
            takes: &[
                Given(Host(Int)),
                Given(Host(Value)),
                Given(Host(Decimal)),
                Room(Host(Decimal)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DECIMAL_DIVIDE,
            takes: &[
                Given(Host(Decimal)),
                Given(Host(Decimal)),
                Given(Host(Int)),
                Given(Host(Value)),
                Room(Host(Decimal)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: RATIONAL_FROM_INT,
            takes: &[Given(Host(Int))],
            answers: Some(Rational),
        },
        GeneratedCall {
            name: RATIONAL_FROM_DECIMAL,
            takes: &[Given(Host(Decimal))],
            answers: Some(Rational),
        },
        GeneratedCall {
            name: RATIONAL_NEGATE,
            takes: &[Given(Rational)],
            answers: Some(Rational),
        },
        GeneratedCall {
            name: RATIONAL_IS_ZERO,
            takes: &[Given(Rational)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: RATIONAL_IS_WHOLE,
            takes: &[Given(Rational)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: RATIONAL_HAS_FINITE_DECIMAL,
            takes: &[Given(Rational)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: RATIONAL_COMPARE,
            takes: &[Given(Rational), Given(Rational)],
            answers: Some(Comparison),
        },
        GeneratedCall {
            name: RATIONAL_ADD,
            takes: &[Given(Rational), Given(Rational), Room(Rational)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: RATIONAL_SUBTRACT,
            takes: &[Given(Rational), Given(Rational), Room(Rational)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: RATIONAL_MULTIPLY,
            takes: &[Given(Rational), Given(Rational), Room(Rational)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: RATIONAL_DIVIDE,
            takes: &[Given(Rational), Given(Rational), Room(Rational)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: RATIONAL_TO_WHOLE,
            takes: &[Given(Rational), Room(Host(Int))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: RATIONAL_TO_FINITE_DECIMAL,
            takes: &[Given(Rational), Room(Host(Decimal))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: RATIONAL_TO_INT,
            takes: &[Given(Host(Value)), Given(Rational), Room(Host(Int))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: RATIONAL_TO_DECIMAL,
            takes: &[
                Given(Host(Int)),
                Given(Host(Value)),
                Given(Rational),
                Room(Host(Decimal)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DATE_LITERAL,
            takes: &[Given(Host(Int))],
            answers: Some(Host(Date)),
        },
        GeneratedCall {
            name: DATE_COMPARE,
            takes: &[Given(Host(Date)), Given(Host(Date))],
            answers: Some(Comparison),
        },
        GeneratedCall {
            name: TIME_LITERAL,
            takes: &[Given(Host(Int))],
            answers: Some(Host(Time)),
        },
        GeneratedCall {
            name: TIME_COMPARE,
            takes: &[Given(Host(Time)), Given(Host(Time))],
            answers: Some(Comparison),
        },
        GeneratedCall {
            name: DATETIME_LITERAL,
            takes: &[Given(Host(Int))],
            answers: Some(Host(DateTime)),
        },
        GeneratedCall {
            name: DATETIME_COMPARE,
            takes: &[Given(Host(DateTime)), Given(Host(DateTime))],
            answers: Some(Comparison),
        },
        GeneratedCall {
            name: INSTANT_LITERAL,
            takes: &[Given(Host(Int)), Given(Host(Int))],
            answers: Some(Host(Instant)),
        },
        GeneratedCall {
            name: INSTANT_COMPARE,
            takes: &[Given(Host(Instant)), Given(Host(Instant))],
            answers: Some(Comparison),
        },
        GeneratedCall {
            name: DATE_ADD_DAYS,
            takes: &[Given(Host(Int)), Given(Host(Date)), Room(Host(Date))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DATE_ADD_MONTHS,
            takes: &[Given(Host(Int)), Given(Host(Date)), Room(Host(Date))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DATE_ADD_YEARS,
            takes: &[Given(Host(Int)), Given(Host(Date)), Room(Host(Date))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DATETIME_ADD_MINUTES,
            takes: &[
                Given(Host(Int)),
                Given(Host(DateTime)),
                Room(Host(DateTime)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DATETIME_ADD_HOURS,
            takes: &[
                Given(Host(Int)),
                Given(Host(DateTime)),
                Room(Host(DateTime)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DATETIME_ADD_DAYS,
            takes: &[
                Given(Host(Int)),
                Given(Host(DateTime)),
                Room(Host(DateTime)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: DATE_DAYS_BETWEEN,
            takes: &[Given(Host(Date)), Given(Host(Date))],
            answers: Some(Host(Int)),
        },
        GeneratedCall {
            name: DATE_YEAR,
            takes: &[Given(Host(Date))],
            answers: Some(Host(Int)),
        },
        GeneratedCall {
            name: DATE_MONTH,
            takes: &[Given(Host(Date))],
            answers: Some(Host(Int)),
        },
        GeneratedCall {
            name: DATE_DAY,
            takes: &[Given(Host(Date))],
            answers: Some(Host(Int)),
        },
        GeneratedCall {
            name: DATE_FROM_PARTS,
            takes: &[
                Given(Host(Int)),
                Given(Host(Int)),
                Given(Host(Int)),
                Room(Host(Date)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: TIME_FROM_PARTS,
            takes: &[
                Given(Host(Int)),
                Given(Host(Int)),
                Given(Host(Int)),
                Room(Host(Time)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: TIME_HOUR,
            takes: &[Given(Host(Time))],
            answers: Some(Host(Int)),
        },
        GeneratedCall {
            name: TIME_MINUTE,
            takes: &[Given(Host(Time))],
            answers: Some(Host(Int)),
        },
        GeneratedCall {
            name: TIME_SECOND,
            takes: &[Given(Host(Time))],
            answers: Some(Host(Int)),
        },
        GeneratedCall {
            name: DATETIME_MINUTES_BETWEEN,
            takes: &[Given(Host(DateTime)), Given(Host(DateTime))],
            answers: Some(Host(Int)),
        },
        GeneratedCall {
            name: DATETIME_TO_DATE,
            takes: &[Given(Host(DateTime))],
            answers: Some(Host(Date)),
        },
        GeneratedCall {
            name: DATETIME_TO_TIME,
            takes: &[Given(Host(DateTime))],
            answers: Some(Host(Time)),
        },
        GeneratedCall {
            name: DATETIME_FROM_DATE_AND_TIME,
            takes: &[Given(Host(Date)), Given(Host(Time))],
            answers: Some(Host(DateTime)),
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
            name: EXTERNAL_DECIMAL,
            takes: &[Given(Host(Decimal))],
            answers: Some(Form),
        },
        GeneratedCall {
            name: EXTERNAL_DATE,
            takes: &[Given(Host(Date))],
            answers: Some(Form),
        },
        GeneratedCall {
            name: EXTERNAL_TIME,
            takes: &[Given(Host(Time))],
            answers: Some(Form),
        },
        GeneratedCall {
            name: EXTERNAL_DATETIME,
            takes: &[Given(Host(DateTime))],
            answers: Some(Form),
        },
        GeneratedCall {
            name: EXTERNAL_INSTANT,
            takes: &[Given(Host(Instant))],
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
            name: EXTERNAL_ORDER,
            takes: &[Given(Form)],
            answers: None,
        },
        GeneratedCall {
            name: EXTERNAL_ENTRIES,
            takes: &[Given(Form)],
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
            name: READ_MEMBERS,
            takes: &[Given(Node)],
            answers: Some(Host(Count)),
        },
        GeneratedCall {
            name: READ_MEMBER_KEY,
            takes: &[Given(Node), Given(Host(Count))],
            answers: Some(Node),
        },
        GeneratedCall {
            name: READ_MEMBER_VALUE,
            takes: &[Given(Node), Given(Host(Count))],
            answers: Some(Node),
        },
        GeneratedCall {
            name: PATH_BELOW_MEMBER,
            takes: &[Given(Path), Given(Node), Given(Host(Count))],
            answers: Some(Path),
        },
        GeneratedCall {
            name: READ_DUPLICATE_KEY,
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
            name: READ_DECIMAL,
            takes: &[
                Given(Node),
                Given(Path),
                Given(Host(Decoded)),
                Room(Host(Decimal)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_DATE,
            takes: &[
                Given(Node),
                Given(Path),
                Given(Host(Decoded)),
                Room(Host(Date)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_TIME,
            takes: &[
                Given(Node),
                Given(Path),
                Given(Host(Decoded)),
                Room(Host(Time)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_DATETIME,
            takes: &[
                Given(Node),
                Given(Path),
                Given(Host(Decoded)),
                Room(Host(DateTime)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_INSTANT,
            takes: &[
                Given(Node),
                Given(Path),
                Given(Host(Decoded)),
                Room(Host(Instant)),
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
        GeneratedCall {
            name: READ_MIN_LENGTH,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Host(String)),
                Given(Host(Int)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_MAX_LENGTH,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Host(String)),
                Given(Host(Int)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_FIXED_LENGTH,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Host(String)),
                Given(Host(Int)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_PATTERN,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Host(String)),
                Given(Machine),
                Given(Host(String)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_INT_MIN,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Host(Int)),
                Given(Host(Int)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_INT_MAX,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Host(Int)),
                Given(Host(Int)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_INT_POSITIVE,
            takes: &[Given(Path), Given(Host(Decoded)), Given(Host(Int))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_INT_NON_NEGATIVE,
            takes: &[Given(Path), Given(Host(Decoded)), Given(Host(Int))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_DECIMAL_MIN,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Host(Decimal)),
                Given(Host(Decimal)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_DECIMAL_MAX,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Host(Decimal)),
                Given(Host(Decimal)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_DECIMAL_POSITIVE,
            takes: &[Given(Path), Given(Host(Decoded)), Given(Host(Decimal))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_DECIMAL_NON_NEGATIVE,
            takes: &[Given(Path), Given(Host(Decoded)), Given(Host(Decimal))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_LIST_NON_EMPTY,
            takes: &[Given(Path), Given(Host(Decoded)), Given(Host(List))],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_LIST_MIN_SIZE,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Host(List)),
                Given(Host(Int)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_LIST_MAX_SIZE,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Host(List)),
                Given(Host(Int)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_LIST_FIXED_SIZE,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Host(List)),
                Given(Host(Int)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_MAP_NON_EMPTY,
            takes: &[Given(Path), Given(Host(Decoded)), Given(Map)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_MAP_MIN_SIZE,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Map),
                Given(Host(Int)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: READ_MAP_MAX_SIZE,
            takes: &[
                Given(Path),
                Given(Host(Decoded)),
                Given(Map),
                Given(Host(Int)),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: LIST_DUPLICATES,
            takes: &[Given(Host(List)), Given(Hasher), Given(Equality)],
            answers: Some(Host(List)),
        },
        GeneratedCall {
            name: READ_DUPLICATES,
            takes: &[Given(Path), Given(Host(Decoded)), Given(Form)],
            answers: None,
        },
        GeneratedCall {
            name: SET_EMPTY,
            takes: &[],
            answers: Some(Set),
        },
        GeneratedCall {
            name: SET_INSERT,
            takes: &[
                Given(Set),
                Given(Host(Int)),
                Given(Hasher),
                Given(Equality),
                Room(Set),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: SET_REMOVE,
            takes: &[Given(Set), Given(Host(Int)), Given(Hasher), Given(Equality)],
            answers: Some(Set),
        },
        GeneratedCall {
            name: SET_CONTAINS,
            takes: &[Given(Set), Given(Host(Int)), Given(Hasher), Given(Equality)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: SET_UNION,
            takes: &[Given(Set), Given(Set), Given(Equality), Room(Set)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: SET_INTERSECTION,
            takes: &[Given(Set), Given(Set), Given(Equality)],
            answers: Some(Set),
        },
        GeneratedCall {
            name: SET_DIFFERENCE,
            takes: &[Given(Set), Given(Set), Given(Equality)],
            answers: Some(Set),
        },
        GeneratedCall {
            name: SET_SIZE,
            takes: &[Given(Set)],
            answers: Some(Host(Int)),
        },
        GeneratedCall {
            name: SET_TO_LIST,
            takes: &[Given(Set)],
            answers: Some(Host(List)),
        },
        GeneratedCall {
            name: SET_FROM_LIST,
            takes: &[Given(Host(List)), Given(Hasher), Given(Equality)],
            answers: Some(Set),
        },
        GeneratedCall {
            name: SET_EQUAL,
            takes: &[Given(Set), Given(Set), Given(Equality)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: SET_HASH,
            takes: &[Given(Set)],
            answers: Some(Hash),
        },
        GeneratedCall {
            name: MAP_EMPTY,
            takes: &[],
            answers: Some(Map),
        },
        GeneratedCall {
            name: MAP_GET,
            takes: &[Given(Map), Given(Host(Int)), Given(Hasher), Given(Equality)],
            answers: Some(Held),
        },
        GeneratedCall {
            name: MAP_CONTAINS_KEY,
            takes: &[Given(Map), Given(Host(Int)), Given(Hasher), Given(Equality)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: MAP_KEYS,
            takes: &[Given(Map)],
            answers: Some(Host(List)),
        },
        GeneratedCall {
            name: MAP_VALUES,
            takes: &[Given(Map)],
            answers: Some(Host(List)),
        },
        GeneratedCall {
            name: MAP_INSERT,
            takes: &[
                Given(Map),
                Given(Host(Int)),
                Given(Host(Int)),
                Given(Hasher),
                Given(Equality),
                Room(Map),
            ],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: MAP_REMOVE,
            takes: &[Given(Map), Given(Host(Int)), Given(Hasher), Given(Equality)],
            answers: Some(Map),
        },
        GeneratedCall {
            name: MAP_SIZE,
            takes: &[Given(Map)],
            answers: Some(Host(Int)),
        },
        GeneratedCall {
            name: MAP_TO_LIST,
            takes: &[Given(Map)],
            answers: Some(Host(List)),
        },
        GeneratedCall {
            name: MAP_FROM_LIST,
            takes: &[Given(Host(List)), Given(Hasher), Given(Equality)],
            answers: Some(Map),
        },
        GeneratedCall {
            name: MAP_EQUAL,
            takes: &[Given(Map), Given(Map), Given(Equality), Given(Equality)],
            answers: Some(Host(Bool)),
        },
        GeneratedCall {
            name: MAP_HASH,
            takes: &[Given(Map), Given(Hasher)],
            answers: Some(Hash),
        },
        GeneratedCall {
            name: HASH_COMBINE,
            takes: &[Given(Hash), Given(Host(Int))],
            answers: Some(Hash),
        },
        GeneratedCall {
            name: STRING_HASH,
            takes: &[Given(Host(String))],
            answers: Some(Hash),
        },
        GeneratedCall {
            name: DECIMAL_HASH,
            takes: &[Given(Host(Decimal))],
            answers: Some(Hash),
        },
        GeneratedCall {
            name: RATIONAL_HASH,
            takes: &[Given(Rational)],
            answers: Some(Hash),
        },
        GeneratedCall {
            name: DATE_HASH,
            takes: &[Given(Host(Date))],
            answers: Some(Hash),
        },
        GeneratedCall {
            name: TIME_HASH,
            takes: &[Given(Host(Time))],
            answers: Some(Hash),
        },
        GeneratedCall {
            name: DATETIME_HASH,
            takes: &[Given(Host(DateTime))],
            answers: Some(Hash),
        },
        GeneratedCall {
            name: INSTANT_HASH,
            takes: &[Given(Host(Instant))],
            answers: Some(Hash),
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

/// The statuses this crate keeps for what no Souther computation answers: the top sixteen numbers
/// below 2³¹. Every host status and every status only a row brings about is one of these, and
/// every number a language abort is given is above [`ANSWERED`] and below them.
///
/// A status is a `u32`, and the numbers stop at 2³¹ − 1 all the same: a header names each status
/// as an enumerator, and C gives an enumerator the range of an `int`, so a status past it is one a
/// header could not name.
pub const RESERVED: std::ops::RangeInclusive<Status> = 0x7fff_fff0..=0x7fff_ffff;

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

/// The statuses no Souther computation answers, by the names a host is told them under: each in
/// [`RESERVED`].
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
    /// Every unit the language declares, as the Java half holds it to what upstream's library
    /// declares, and this table names each of them and nothing else.
    #[test]
    fn the_runtime_has_a_token_for_every_unit_the_language_declares() {
        let declared: std::collections::BTreeSet<String> = include_str!("../language-units.txt")
            .lines()
            .map(str::to_string)
            .collect();
        let tabled: std::collections::BTreeSet<String> = super::LANGUAGE_UNITS
            .iter()
            .map(|(module, name)| format!("{module}.{name}"))
            .collect();
        assert_eq!(tabled, declared);
        assert_eq!(super::LANGUAGE_UNITS.len(), declared.len());
    }

    use super::{
        ABI_GENERATION, EXAMPLE_STATUSES, FAKE_NO_OUTPUT, FIRST_FIELD, HOST_STATUSES,
        HostFunctionOperation, HostLeaf, HostListOperation, HostShape, IMPLEMENTATION_ANSWERS,
        INJECTION_PROTOCOL_VIOLATION, INJECTION_UNBOUND, SLOT, TOKEN, WHICH, behavior_symbol,
        boundary_symbol, checked_constructor_symbol, constructor_symbol, example_symbol, field_at,
        held_symbol, home_symbol, host_behavior_answer_case_symbol, host_behavior_symbol,
        host_bind_symbol, host_case_symbol, host_constructor_symbol, host_decode_symbol,
        host_encode_symbol, host_field_symbol, host_function_symbol, host_implement_symbol,
        host_implementation_type, host_list_symbol, host_value_symbol, member_at, reader_symbol,
        type_symbol, value_symbol,
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
            "souther9.calculation.add"
        );
    }

    #[test]
    fn a_dotted_module_keeps_its_dots() {
        assert_eq!(behavior_symbol("lib.pub", "bill"), "souther9.lib.pub.bill");
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
            "souther9.calculation.add$example$0"
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
        assert_eq!(boundary_symbol(&entry), "souther9.shop.quote$boundary");
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
            "souther9.pricing$value$standard"
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
            "souther9.pricing$construct$Amount"
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
            "souther9.pricing$checked$Amount"
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
            "souther9_m_pricing_t_Amount_construct"
        );
        assert_eq!(
            host_field_symbol("pricing", "Amount", "value"),
            "souther9_m_pricing_t_Amount_f_value"
        );
        assert_eq!(
            host_case_symbol("pricing", "Result"),
            "souther9_m_pricing_t_Result_case"
        );
        assert_eq!(
            host_decode_symbol("pricing", "Amount"),
            "souther9_m_pricing_t_Amount_decode"
        );
        assert_eq!(
            host_encode_symbol("pricing", "Amount"),
            "souther9_m_pricing_t_Amount_encode"
        );
    }

    #[test]
    fn a_host_reaches_a_behavior_and_a_value_under_their_module() {
        assert_eq!(
            host_behavior_symbol("lib.shop", "quote"),
            "souther9_m_lib_m_shop_b_quote"
        );
        assert_eq!(
            host_value_symbol("lib.shop", "standard"),
            "souther9_m_lib_m_shop_v_standard"
        );
        assert_eq!(
            host_behavior_answer_case_symbol("lib.shop", "find"),
            "souther9_m_lib_m_shop_b_find_answer_case"
        );
    }

    #[test]
    fn a_host_reaches_a_list_under_its_module_and_the_shape_an_element_crosses_in() {
        use HostLeaf::{Bool, Int, Value};
        assert_eq!(
            host_list_symbol(
                "shop",
                &HostShape::Leaf(Value),
                HostListOperation::Construct
            ),
            "souther9_m_shop_l_value_construct"
        );
        assert_eq!(
            host_list_symbol(
                "lib.shop",
                &HostShape::Option(Box::new(HostShape::Leaf(Int))),
                HostListOperation::At
            ),
            "souther9_m_lib_m_shop_l_o_int_at"
        );
        assert_eq!(
            host_list_symbol(
                "shop",
                &HostShape::List(Box::new(HostShape::Product(vec![
                    HostShape::Leaf(Int),
                    HostShape::Option(Box::new(HostShape::Leaf(Bool))),
                ]))),
                HostListOperation::Length
            ),
            "souther9_m_shop_l_l_t2_int_o_bool_length"
        );
    }

    #[test]
    fn a_host_reaches_a_function_value_under_its_module_and_its_shape() {
        use HostLeaf::{Int, String};
        let function = HostShape::Function {
            takes: vec![HostShape::Leaf(Int), HostShape::Leaf(String)],
            answers: Box::new(HostShape::Option(Box::new(HostShape::Leaf(Int)))),
        };
        assert_eq!(
            host_function_symbol("shop", &function, HostFunctionOperation::Call),
            "souther9_m_shop_fn_f2_int_string_o_int_call"
        );
        assert_eq!(
            host_function_symbol("shop", &function, HostFunctionOperation::Implement),
            "souther9_m_shop_fn_f2_int_string_o_int_implement"
        );
    }

    /// Every shape up to a depth, each spelt once: no two are spelt alike, and each is read back
    /// from its spelling alone, by the count every mark says follows it.
    #[test]
    fn no_two_shapes_are_spelt_alike() {
        fn every(depth: usize) -> Vec<HostShape> {
            let mut shapes: Vec<HostShape> = [
                HostLeaf::Int,
                HostLeaf::Bool,
                HostLeaf::String,
                HostLeaf::Decimal,
                HostLeaf::Value,
            ]
            .into_iter()
            .map(HostShape::Leaf)
            .collect();
            if depth == 0 {
                return shapes;
            }
            let smaller = every(depth - 1);
            for one in &smaller {
                shapes.push(HostShape::Option(Box::new(one.clone())));
                shapes.push(HostShape::List(Box::new(one.clone())));
                shapes.push(HostShape::Function {
                    takes: vec![],
                    answers: Box::new(one.clone()),
                });
                for other in &smaller {
                    shapes.push(HostShape::Product(vec![one.clone(), other.clone()]));
                    shapes.push(HostShape::Function {
                        takes: vec![one.clone()],
                        answers: Box::new(other.clone()),
                    });
                }
            }
            shapes
        }
        fn read(tokens: &mut std::slice::Iter<&str>) -> Option<HostShape> {
            let token = *tokens.next()?;
            let count = |mark: char| token.strip_prefix(mark)?.parse::<usize>().ok();
            Some(match token {
                "int" => HostShape::Leaf(HostLeaf::Int),
                "bool" => HostShape::Leaf(HostLeaf::Bool),
                "string" => HostShape::Leaf(HostLeaf::String),
                "decimal" => HostShape::Leaf(HostLeaf::Decimal),
                "value" => HostShape::Leaf(HostLeaf::Value),
                "o" => HostShape::Option(Box::new(read(tokens)?)),
                "l" => HostShape::List(Box::new(read(tokens)?)),
                _ if count('t').is_some() => HostShape::Product(
                    (0..count('t')?)
                        .map(|_| read(tokens))
                        .collect::<Option<_>>()?,
                ),
                _ if count('f').is_some() => HostShape::Function {
                    takes: (0..count('f')?)
                        .map(|_| read(tokens))
                        .collect::<Option<_>>()?,
                    answers: Box::new(read(tokens)?),
                },
                _ => return None,
            })
        }
        let mut seen = std::collections::HashMap::new();
        for shape in every(2) {
            let spelt = shape.spelt();
            let tokens: Vec<&str> = spelt.split('_').collect();
            let mut tokens = tokens.iter();
            assert_eq!(read(&mut tokens).as_ref(), Some(&shape), "{spelt}");
            assert!(
                tokens.next().is_none(),
                "{spelt} spells more than one shape"
            );
            let before = seen.entry(spelt.clone()).or_insert_with(|| shape.clone());
            assert_eq!(*before, shape, "{spelt} is two shapes");
        }
    }

    #[test]
    fn a_name_that_is_not_ascii_letters_and_digits_is_escaped() {
        assert_eq!(
            host_behavior_symbol("shop", "foo_bar"),
            "souther9_m_shop_b_foo__bar"
        );
        assert_eq!(
            host_behavior_symbol("shop", "数量"),
            "souther9_m_shop_b__u6570__u91cf_"
        );
    }

    /// What another object reads a value of a type through is under the type's module, apart from
    /// everything a host reaches.
    #[test]
    fn a_type_is_read_through_its_module_and_its_name() {
        assert_eq!(
            reader_symbol("pricing", "Amount"),
            "souther9.pricing$read$Amount"
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
            let int = HostShape::Leaf(HostLeaf::Int);
            for shape in [
                HostShape::Leaf(HostLeaf::Value),
                HostShape::Option(Box::new(int.clone())),
                HostShape::Product(vec![int.clone(), HostShape::List(Box::new(int.clone()))]),
                HostShape::Function {
                    takes: vec![int.clone()],
                    answers: Box::new(int.clone()),
                },
            ] {
                for (done, word) in [
                    (HostListOperation::Construct, "construct"),
                    (HostListOperation::Length, "length"),
                    (HostListOperation::At, "at"),
                ] {
                    let mut read = vec![operation("l")];
                    read.extend(shape.spelt().split('_').map(operation));
                    read.push(operation(word));
                    hold(host_list_symbol(module, &shape, done), under(module, read));
                }
                if matches!(shape, HostShape::Function { .. }) {
                    for (done, word) in [
                        (HostFunctionOperation::Call, "call"),
                        (HostFunctionOperation::Implement, "implement"),
                        (HostFunctionOperation::Implementation, "implementation"),
                    ] {
                        let mut read = vec![operation("fn")];
                        read.extend(shape.spelt().split('_').map(operation));
                        read.push(operation(word));
                        hold(
                            host_function_symbol(module, &shape, done),
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
    /// other, and is one of the reserved numbers, which all stay inside what a header's
    /// enumeration holds.
    #[test]
    fn what_a_host_brings_about_is_a_number_of_its_own() {
        assert!(i32::try_from(*super::RESERVED.end()).is_ok());
        assert!(!super::RESERVED.contains(&super::ANSWERED));
        let mut seen = vec![super::ANSWERED];
        for (name, number) in HOST_STATUSES.iter().chain(EXAMPLE_STATUSES) {
            assert!(!seen.contains(number), "{name} answers {number} twice");
            assert!(
                super::RESERVED.contains(number),
                "{name} is not a reserved number"
            );
            seen.push(*number);
        }
        assert!(!IMPLEMENTATION_ANSWERS.contains(&INJECTION_UNBOUND));
        assert!(!IMPLEMENTATION_ANSWERS.contains(&INJECTION_PROTOCOL_VIOLATION));
        assert!(!IMPLEMENTATION_ANSWERS.contains(&FAKE_NO_OUTPUT));
    }
}
