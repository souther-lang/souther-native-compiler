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
/// - What a non-`ANSWERED` `status` *means*. This crate reserves `ANSWERED` and nothing else —
///   which wire number a language abort gets is `native_status` in `souther-native-driver`'s own
///   exhaustive mapping, kept apart from this crate for the reason this file's own doc gives. Two
///   objects built by drivers whose `native_status` disagrees about what `4` is are exactly as
///   incompatible as two objects with different calling conventions; they just still link, because
///   nothing about the *shape* of the call changed. A renumbering there bumps this the same as a
///   calling-convention change does — see `abort-status-abi2.json` in the driver crate's own
///   tests, named for the generation it is a fixture of.
///
/// Not part of [`type_symbol`]: a declared type's token is data, not a call, and nothing about how
/// a call is made or what its status means changes what a value of one looks like.
const ABI: &str = "2";

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
    format!("souther{ABI}.{module}.{behavior}")
}

/// The symbol a definition a module holds is reached by.
///
/// Both the module holding it and the module that declared it, because a module carries every
/// helper it reaches and two modules reaching one helper hold a copy each — which is what the
/// language says a published helper is. One name for both copies would be one of them silently
/// standing for the other.
///
/// Nothing outside the object reaches one of these, so what this has to be is unambiguous here and
/// nowhere else. The `$` is what keeps it so: a module's name carries dots and a declaration's
/// carries them too, and neither carries this.
///
/// # Panics
///
/// Where the carrier's name does not stand in a symbol ([`spells_a_module`]).
pub fn held_symbol(carrier: &str, declared: &str) -> String {
    assert!(
        spells_a_module(carrier),
        "a module's name carries no dollar, and the symbol is split on one: {carrier}"
    );
    format!("souther{ABI}.{carrier}${declared}")
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
    format!("souther{ABI}.{module}$home${value}")
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
    format!("souther{ABI}.{module}$value${value}")
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
    format!("souther{ABI}.{module}$construct${name}")
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
pub const TOKEN: &[u8] = &[0];

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
        fields += 1;
    }
    assert!(WHICH + SLOT <= room_for_fields(0));
    assert!(HELD + SLOT <= room_for_held());
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
/// rather than by a slot beside it. An `Option` holding a value is a pointer to one slot holding
/// it, boxed even where the value would fit in a pointer, because whether it fits is a fact about
/// one type and an `Option` is one representation over every type.
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

/// What a generated function answers with instead of its value directly.
///
/// A Souther computation ends with a value or without one, and a plain return can only ever say
/// the first — which is why every generated function takes one more parameter than its signature
/// shows a caller, a pointer the value is written through, and answers this instead. `ANSWERED`
/// says the pointer holds it; any other code is a language abort's wire number and the pointer was
/// never written.
///
/// What number a member of `souther_compiler`'s `AbortKind` gets is not here. This crate is the
/// wire's width and its one reserved value, both facts a target decides; which reason gets which
/// of the numbers left over is `souther_native_driver`'s own exhaustive mapping, kept apart from
/// this crate for the reason this file's own doc gives — nothing about what Souther means belongs
/// here, and an abort's reason is exactly that.
pub type Status = u32;

/// The one code this crate reserves: the pointer holds the answer.
///
/// Every other value of [`Status`] is a language abort, and which is which is
/// `souther_native_driver`'s to say — this crate answers only for the one case that is not one.
pub const ANSWERED: Status = 0;

#[cfg(test)]
mod tests {
    use super::{
        FIRST_FIELD, SLOT, TOKEN, WHICH, behavior_symbol, boundary_symbol, constructor_symbol,
        example_symbol, field_at, held_symbol, home_symbol, member_at, type_symbol, value_symbol,
    };

    #[test]
    fn a_behavior_is_reached_by_its_module_and_its_name() {
        assert_eq!(
            behavior_symbol("calculation", "add"),
            "souther2.calculation.add"
        );
    }

    #[test]
    fn a_dotted_module_keeps_its_dots() {
        assert_eq!(behavior_symbol("lib.pub", "bill"), "souther2.lib.pub.bill");
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

    /// Two modules holding one declaration hold a copy each, and the copies are not one symbol.
    #[test]
    fn a_definition_held_by_two_modules_is_two_symbols() {
        assert_ne!(
            held_symbol("pricing", "pricing.taxed"),
            held_symbol("order", "pricing.taxed")
        );
    }

    /// A row's entry is reached by neither the behavior's name nor another row's.
    #[test]
    fn a_helper_and_a_value_of_one_name_are_two_symbols() {
        assert_ne!(held_symbol("m", "m.v"), home_symbol("m", "v"));
    }

    #[test]
    fn each_row_of_a_behavior_is_its_own_symbol() {
        assert_eq!(
            example_symbol("calculation", "add", 0),
            "souther2.calculation.add$example$0"
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
        assert_eq!(boundary_symbol(&entry), "souther2.shop.quote$boundary");
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
            held_symbol("pricing", "pricing.taxed")
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
            "souther2.pricing$value$standard"
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
            held_symbol("pricing", "pricing.standard")
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
            "souther2.pricing$construct$Amount"
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
    }

    #[test]
    #[should_panic(expected = "neither dollar nor dot")]
    fn a_constructed_types_name_carrying_a_dot_is_refused() {
        let _ = constructor_symbol("a", "b.C");
    }
}
