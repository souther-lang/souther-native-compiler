//! `lowercase` and `uppercase`: Unicode 18.0.0's default case conversion, untailored (spec
//! §string-case).
//!
//! The full mapping, so one code point may become several (`ß` uppercases to `SS`), and no locale:
//! a Turkish `i` uppercases to `I`, never to `İ`. The one condition read is `Final_Sigma`, which is
//! about the text around a capital sigma and not about who is reading it. What either answers is
//! put in NFC again, since mapping case can leave text that is not.

use crate::Text;
use crate::canonical::Joined;
use crate::capacity::Capacity;
use crate::tables::{CASE_IGNORABLE, CASED, FINAL_SIGMA, LOWERCASE, UPPERCASE};
use alloc::string::String;

/// The text lowercased (`String.lowercase`), or nothing where the mapping is more text than the
/// carrier holds. Measured as each code point is mapped and not once all are, so that text no
/// carrier holds is never made.
pub fn lowercase(text: Text, capacity: Capacity) -> Option<String> {
    let text = text.as_str();
    let mut mapped = Mapped::new(capacity);
    for (at, character) in text.char_indices() {
        let point = u32::from(character);
        let table = mapping(FINAL_SIGMA, point)
            .filter(|_| final_sigma_holds(text, at, character))
            .or_else(|| mapping(LOWERCASE, point));
        mapped.put(table, point)?;
    }
    mapped.in_nfc()
}

/// The text uppercased (`String.uppercase`), as [`lowercase`] lowercases it.
pub fn uppercase(text: Text, capacity: Capacity) -> Option<String> {
    let mut mapped = Mapped::new(capacity);
    for point in text.as_str().chars().map(u32::from) {
        mapped.put(mapping(UPPERCASE, point), point)?;
    }
    mapped.in_nfc()
}

/// What a case mapping has written so far, and what it may still write.
struct Mapped {
    text: String,
    units: i64,
    capacity: Capacity,
}

impl Mapped {
    fn new(capacity: Capacity) -> Mapped {
        Mapped {
            text: String::new(),
            units: 0,
            capacity,
        }
    }

    /// What `point` maps to, or itself where the table names nothing, unless that is more than is
    /// held.
    fn put(&mut self, mapped: Option<&[u32]>, point: u32) -> Option<()> {
        for one in mapped.unwrap_or(&[point]) {
            let character = scalar(*one);
            self.units += character.len_utf16() as i64;
            if !self.capacity.holds(self.units) {
                return None;
            }
            self.text.push(character);
        }
        Some(())
    }

    /// The text in NFC, which mapping case can leave it out of, within what is held.
    fn in_nfc(self) -> Option<String> {
        let mut joined = Joined::new(self.capacity);
        joined.push_unnormalized(&self.text)?;
        Some(joined.finished())
    }
}

/// A code point a table maps to, which is a scalar value.
fn scalar(point: u32) -> char {
    char::from_u32(point).expect("a case mapping maps to scalar values")
}

/// What a table maps a code point to, where it maps it at all: a code point it does not name maps
/// to itself.
fn mapping(table: &'static [(u32, &'static [u32])], point: u32) -> Option<&'static [u32]> {
    table
        .binary_search_by_key(&point, |(it, _)| *it)
        .ok()
        .map(|at| table[at].1)
}

fn within(runs: &[(u32, u32)], point: u32) -> bool {
    match runs.binary_search_by_key(&point, |(from, _)| *from) {
        Ok(_) => true,
        Err(0) => false,
        Err(after) => point <= runs[after - 1].1,
    }
}

/// Whether the code point `character` at byte `at` stands where `Final_Sigma` holds: a cased code
/// point before it and none after it, each looked for past the case-ignorable code points between.
fn final_sigma_holds(text: &str, at: usize, character: char) -> bool {
    let cased = |point: &char| !within(CASE_IGNORABLE, u32::from(*point));
    let before = text[..at].chars().rev().find(cased);
    let after = text[at + character.len_utf8()..].chars().find(cased);
    before.is_some_and(|it| within(CASED, u32::from(it)))
        && !after.is_some_and(|it| within(CASED, u32::from(it)))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::string::String;

    const ROOMY: Capacity = Capacity::of_units(1 << 20);

    fn lower(text: &str) -> String {
        lowercase(Text::held(text), ROOMY).unwrap()
    }

    fn upper(text: &str) -> String {
        uppercase(Text::held(text), ROOMY).unwrap()
    }

    /// A mapping is measured as it is made: it is held exactly where every capacity from the units
    /// it maps to up holds it, and no capacity below does, whatever it is put in NFC as after.
    #[test]
    fn a_mapping_is_held_by_what_it_maps_to() {
        for (text, mapped_units) in [
            ("straße", 7),
            ("ﬁﬁ", 4),
            ("\u{149}\u{149}", 4),
            ("a\u{10428}", 3),
        ] {
            let mapped = upper(text);
            for capacity in 0..=mapped_units + 2 {
                let capacity = Capacity::of_units(capacity);
                let held = uppercase(Text::held(text), capacity);
                assert_eq!(
                    held.is_some(),
                    capacity.units() >= mapped_units,
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
