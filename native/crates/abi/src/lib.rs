//! What generated code and the native runtime agree on.
//!
//! Both sides are written here rather than each writing its own copy, because everything in this
//! crate is a two-party contract: a number one side writes and the other reads is wrong in a way
//! neither side's tests can see. Nothing about Souther's meaning belongs here — only the names,
//! layouts and encodings a target decides.

/// The symbol a behavior is reached by.
///
/// `souther.<module>.<behavior>`, with the module written as it is declared. Both ELF and Mach-O
/// carry a dot in a symbol name, so nothing is replaced on the way.
///
/// What makes the spelling unambiguous is that the behavior is the last segment, and that holds
/// only while a behavior's name carries no dot. That is checked here rather than stated: it is a
/// property of the names Souther admits, and a caller that ever broke it would produce a symbol
/// meaning something other than what it named, which nothing downstream could notice.
///
/// # Panics
///
/// Where the behavior's name carries a dot.
pub fn behavior_symbol(module: &str, behavior: &str) -> String {
    assert!(
        !behavior.contains('.'),
        "a behavior's name carries no dot, and the symbol's last segment is the behavior: {behavior}"
    );
    format!("souther.{module}.{behavior}")
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
pub fn held_symbol(carrier: &str, declared: &str) -> String {
    assert!(
        !carrier.contains('$'),
        "a module's name carries no dollar, and the symbol is split on one: {carrier}"
    );
    format!("souther.{carrier}${declared}")
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

/// How wide a slot is, and so what a value made of slots is measured in.
///
/// One width for every slot, whatever it holds. A layout that packed a `Bool` into a byte would
/// make a field's offset depend on the types of the fields before it, which is a computation both
/// sides would have to agree on for every declaration rather than a multiplication either can do.
pub const SLOT: i64 = 8;

/// Where a value of a declared type says which type it is.
///
/// Every constructed value carries it, including one of a type no sum has a case for. A value's
/// own type is what it is, not what it is being read as, so writing the number only where someone
/// was going to match on it would make the representation depend on a use rather than on the
/// value — and a value built in one behavior and matched in another has no such use to read.
pub const WHICH: i64 = 0;

/// Where a constructed value's first field is. Its fields follow in declaration order.
pub const FIRST_FIELD: i64 = SLOT;

/// The offset of a field of a declared type, by its position in the declaration.
pub fn field_at(position: usize) -> i64 {
    FIRST_FIELD + SLOT * position as i64
}

/// The offset of a tuple's member. A tuple says which type it is nowhere: it is not a declared
/// type and nothing matches on one, so its members start where they are.
pub fn member_at(position: usize) -> i64 {
    SLOT * position as i64
}

/// What an `Option` holding nothing is.
///
/// A null pointer, which no allocation answers, so the two are told apart by what the pointer is
/// rather than by a slot beside it. An `Option` holding a value is a pointer to one slot holding
/// it, boxed even where the value would fit in a pointer, because whether it fits is a fact about
/// one type and an `Option` is one representation over every type.
pub const NOTHING: i64 = 0;

/// Where an `Option`'s value is, once it is known to be holding one.
pub const HELD: i64 = 0;

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

#[cfg(test)]
mod tests {
    use super::{
        FIRST_FIELD, SLOT, WHICH, behavior_symbol, example_symbol, field_at, held_symbol,
        member_at,
    };

    #[test]
    fn a_behavior_is_reached_by_its_module_and_its_name() {
        assert_eq!(behavior_symbol("calculation", "add"), "souther.calculation.add");
    }

    #[test]
    fn a_dotted_module_keeps_its_dots() {
        assert_eq!(behavior_symbol("lib.pub", "bill"), "souther.lib.pub.bill");
    }

    /// What the reading rests on. Were this admitted, `a.b` / `c` and `a` / `b.c` would be spelt
    /// the same way, and a caller reaching one would reach the other.
    #[test]
    #[should_panic(expected = "carries no dot")]
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
    fn each_row_of_a_behavior_is_its_own_symbol() {
        assert_eq!(
            example_symbol("calculation", "add", 0),
            "souther.calculation.add$example$0"
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
    fn fields_and_members_follow_one_after_another() {
        assert_eq!(field_at(1) - field_at(0), SLOT);
        assert_eq!(member_at(0), 0);
        assert_eq!(member_at(3) - member_at(2), SLOT);
    }
}
