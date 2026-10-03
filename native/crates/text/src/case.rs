//! `lowercase` and `uppercase`: Unicode 18.0.0's default case conversion, untailored (spec
//! §string-case).
//!
//! The full mapping, so one code point may become several (`ß` uppercases to `SS`), and no locale:
//! a Turkish `i` uppercases to `I`, never to `İ`. The one condition read is `Final_Sigma`, which is
//! about the text around a capital sigma and not about who is reading it. The mapping is
//! 199x-notation's, and what either answers is put in NFC again, since mapping case can leave text
//! that is not.

use crate::Text;
use crate::canonical::nfc_of_input;
use crate::capacity::Capacity;
use alloc::string::String;

/// The text lowercased (`String.lowercase`), or nothing where the mapping is more text than the
/// carrier holds. Measured as the mapping is written and not once it is, so that text no carrier
/// holds is never made.
pub fn lowercase(text: Text, capacity: Capacity) -> Option<String> {
    let mapped = notation199x::lowercase_within(text.as_str(), most(capacity))?;
    nfc_of_input(&mapped, capacity)
}

/// The text uppercased (`String.uppercase`), as [`lowercase`] lowercases it.
pub fn uppercase(text: Text, capacity: Capacity) -> Option<String> {
    let mapped = notation199x::uppercase_within(text.as_str(), most(capacity))?;
    nfc_of_input(&mapped, capacity)
}

/// The most code points a budget holds, as 199x-notation's bounded rules take it.
fn most(capacity: Capacity) -> usize {
    usize::try_from(capacity.code_points()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::string::String;

    const ROOMY: Capacity = Capacity::of_code_points(1 << 20);

    fn lower(text: &str) -> String {
        lowercase(Text::held(text), ROOMY).unwrap()
    }

    fn upper(text: &str) -> String {
        uppercase(Text::held(text), ROOMY).unwrap()
    }

    /// A mapping is measured as it is made: it is held exactly where every capacity from the code
    /// points it maps to up holds it, and no capacity below does, whatever it is put in NFC as
    /// after. `a\u{10428}` uppercases to `A\u{10400}`, two code points that are four UTF-16 units
    /// (each outside the basic plane), which is why this bound is counted in code points and not
    /// units (spec §what-a-string-holds, ADR-0096).
    #[test]
    fn a_mapping_is_held_by_what_it_maps_to() {
        for (text, mapped_code_points) in [
            ("straße", 7),
            ("ﬁﬁ", 4),
            ("\u{149}\u{149}", 4),
            ("a\u{10428}", 2),
        ] {
            let mapped = upper(text);
            for capacity in 0..=mapped_code_points + 2 {
                let capacity = Capacity::of_code_points(capacity);
                let held = uppercase(Text::held(text), capacity);
                assert_eq!(
                    held.is_some(),
                    capacity.code_points() >= mapped_code_points,
                    "{text:?} at {capacity:?}"
                );
                if let Some(held) = held {
                    assert_eq!(held, mapped);
                }
            }
        }
    }

    /// The full mapping widens where Unicode says it does.
    #[test]
    fn a_code_point_may_map_to_several() {
        assert_eq!(upper("straße"), "STRASSE");
        assert_eq!(lower("STRASSE"), "strasse");
        assert_eq!(upper("ﬁ"), "FI");
        assert_eq!(lower("\u{130}"), "i\u{307}");
    }

    /// No locale narrows it: a Turkish dotless or dotted i is mapped as everyone's.
    #[test]
    fn no_locale_is_read() {
        assert_eq!(upper("i"), "I");
        assert_eq!(lower("I"), "i");
        assert_eq!(upper("ı"), "I");
    }

    /// A capital sigma is final at the end of a cased run and not inside one.
    #[test]
    fn a_final_sigma_is_read_off_the_text_around_it() {
        assert_eq!(lower("ΟΣ"), "ος");
        assert_eq!(lower("ΟΣΑ"), "οσα");
        assert_eq!(lower("Σ"), "σ");
        assert_eq!(lower("ΟΣ."), "ος.");
        assert_eq!(lower("ΟΣ'Α"), "οσ'α");
    }

    /// What neither mapping names is left as it is, and what comes out is in NFC.
    #[test]
    fn what_is_not_mapped_stays_and_what_comes_out_is_canonical() {
        assert_eq!(upper("日本 123"), "日本 123");
        assert_eq!(lower("日本 123"), "日本 123");
        assert_eq!(upper("\u{1f0}"), "J\u{30c}");
    }
}
