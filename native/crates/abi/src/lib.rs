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

#[cfg(test)]
mod tests {
    use super::behavior_symbol;

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
}
