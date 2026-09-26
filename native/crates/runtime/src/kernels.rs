//! The `String` module's kernels, as generated code calls them.
//!
//! What each answers is `souther_text`'s. What is here is where the answer is kept: text read out of
//! the strings handed over, and the text or the pieces `souther_text` answered written into the
//! arena as a string or a list of the layout `souther_native_abi` states. Nothing here decides
//! what a kernel means, and nothing here knows why a kernel answers nothing where it does: a
//! kernel that can answer nothing says only whether it wrote its value, and what the caller makes
//! of that is the caller's contract.

use crate::{Count, List, Text, souther_alloc, string_of, text};
use souther_native_abi::{LIST_LENGTH, list_at, room_for_list};
use souther_text::Text as Held;
use souther_text::pattern;

/// A list of these, each written into its slot by `slot`.
fn list_of<T>(each: &[T], slot: impl Fn(&T) -> i64) -> *mut List {
    let elements = i64::try_from(each.len()).expect("a list holds fewer elements than an Int");
    let at = souther_alloc(Count(room_for_list(elements)));
    unsafe {
        at.offset(LIST_LENGTH as isize)
            .cast::<i64>()
            .write(elements);
        for (index, element) in each.iter().enumerate() {
            at.offset(list_at(index as i64) as isize)
                .cast::<i64>()
                .write(slot(element));
        }
    }
    at.cast()
}

/// A list of strings holding these pieces of text.
fn list_of_strings(pieces: &[Held]) -> *mut List {
    list_of(pieces, |piece| string_of(piece.as_str()) as i64)
}

/// The text of each string a list of strings holds, in order, for as long as the pointer to the
/// list is borrowed, as [`text`] is.
///
/// # Safety
///
/// `list` is a list whose every element is a string, and the mark below it still stands.
unsafe fn texts<'a>(list: &'a *const List) -> Vec<Held<'a>> {
    let at = list.cast::<u8>();
    let elements = unsafe { at.offset(LIST_LENGTH as isize).cast::<i64>().read() };
    (0..elements)
        .map(|index| {
            // Each element is a slot of the list holding a string's address, there for as long as
            // the list is.
            let slot: &'a *const u8 =
                unsafe { &*at.offset(list_at(index) as isize).cast::<*const u8>() };
            unsafe { text(slot) }
        })
        .collect()
}

/// Writes `value` through `out` where there is one, and says whether there was.
///
/// # Safety
///
/// `out` is room for one `T`.
unsafe fn answered<T>(value: Option<T>, out: *mut T) -> i8 {
    match value {
        Some(value) => {
            unsafe { out.write(value) };
            1
        }
        None => 0,
    }
}

/// `String.trim`.
///
/// # Safety
///
/// As [`crate::souther_string_compare`], for every string handed over. So for every function here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_trim(s: *const Text) -> *mut Text {
    string_of(souther_text::trim(unsafe { text(&s) }).as_str())
}

/// `String.lowercase`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_lowercase(s: *const Text) -> *mut Text {
    string_of(&souther_text::lowercase(unsafe { text(&s) }))
}

/// `String.uppercase`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_uppercase(s: *const Text) -> *mut Text {
    string_of(&souther_text::uppercase(unsafe { text(&s) }))
}

/// `String.contains`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_contains(sub: *const Text, s: *const Text) -> i8 {
    unsafe { souther_text::contains(text(&sub), text(&s)) }.into()
}

/// `String.startsWith`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_starts_with(prefix: *const Text, s: *const Text) -> i8 {
    unsafe { souther_text::starts_with(text(&prefix), text(&s)) }.into()
}

/// `String.endsWith`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_ends_with(suffix: *const Text, s: *const Text) -> i8 {
    unsafe { souther_text::ends_with(text(&suffix), text(&s)) }.into()
}

/// `String.matches`, run on the machine the pattern was compiled to.
///
/// # Safety
///
/// As [`souther_string_trim`], and `machine` is the first of the words `souther_text::pattern`
/// compiled, all of which may be read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_matches(machine: *const u32, s: *const Text) -> i8 {
    let words = unsafe { std::slice::from_raw_parts(machine, pattern::length(machine.read())) };
    pattern::matches(words, unsafe { text(&s) }).into()
}

/// `String.slice`, written through `out` where the string has the code points asked for.
///
/// # Safety
///
/// As [`souther_string_trim`], and `out` is room for the address of a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_slice(
    from: i64,
    to: i64,
    s: *const Text,
    out: *mut *mut Text,
) -> i8 {
    let sliced =
        souther_text::slice(from, to, unsafe { text(&s) }).map(|it| string_of(it.as_str()));
    unsafe { answered(sliced, out) }
}

/// `String.split`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_split(separator: *const Text, s: *const Text) -> *mut List {
    list_of_strings(&souther_text::split(unsafe { text(&separator) }, unsafe {
        text(&s)
    }))
}

/// `String.join`.
///
/// # Safety
///
/// As [`souther_string_trim`], and `xs` is a list of strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_join(separator: *const Text, xs: *const List) -> *mut Text {
    let pieces = unsafe { texts(&xs) };
    string_of(&souther_text::join(unsafe { text(&separator) }, pieces))
}

/// `String.concat`.
///
/// # Safety
///
/// As [`souther_string_join`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_concat_all(xs: *const List) -> *mut Text {
    let pieces = unsafe { texts(&xs) };
    string_of(&souther_text::join(Held::held(""), pieces))
}

/// `String.replace`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_replace(
    target: *const Text,
    replacement: *const Text,
    s: *const Text,
) -> *mut Text {
    let replaced = unsafe { souther_text::replace(text(&target), text(&replacement), text(&s)) };
    string_of(&replaced)
}

/// `String.words`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_words(s: *const Text) -> *mut List {
    list_of_strings(&souther_text::words(unsafe { text(&s) }))
}

/// `String.lines`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_lines(s: *const Text) -> *mut List {
    list_of_strings(&souther_text::lines(unsafe { text(&s) }))
}

/// `String.fromInt`.
#[unsafe(no_mangle)]
pub extern "C" fn souther_string_from_int(n: i64) -> *mut Text {
    string_of(&souther_text::written(n))
}

/// `String.toInt`, the integer written through `out` where the text is integer text of an `Int`.
///
/// # Safety
///
/// As [`souther_string_trim`], and `out` is room for an `Int`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_to_int(s: *const Text, out: *mut i64) -> i8 {
    unsafe { answered(souther_text::integer(text(&s)), out) }
}

/// `String.reverse`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_reverse(s: *const Text) -> *mut Text {
    string_of(&souther_text::reverse(unsafe { text(&s) }))
}

/// `String.repeat`, written through `out` where the count is one a string can hold.
///
/// # Safety
///
/// As [`souther_string_slice`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_repeat(
    copies: i64,
    s: *const Text,
    out: *mut *mut Text,
) -> i8 {
    let repeated = souther_text::repeat(copies, unsafe { text(&s) });
    unsafe { answered(repeated.as_deref().map(string_of), out) }
}

/// `String.padLeft`, written through `out` where the width is one a string can hold.
///
/// # Safety
///
/// As [`souther_string_slice`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_pad_left(
    width: i64,
    pad: *const Text,
    s: *const Text,
    out: *mut *mut Text,
) -> i8 {
    let padded = unsafe { souther_text::pad_left(width, text(&pad), text(&s)) };
    unsafe { answered(padded.as_deref().map(string_of), out) }
}

/// `String.padRight`, as [`souther_string_pad_left`].
///
/// # Safety
///
/// As [`souther_string_slice`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_pad_right(
    width: i64,
    pad: *const Text,
    s: *const Text,
    out: *mut *mut Text,
) -> i8 {
    let padded = unsafe { souther_text::pad_right(width, text(&pad), text(&s)) };
    unsafe { answered(padded.as_deref().map(string_of), out) }
}

/// `String.characters`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_characters(s: *const Text) -> *mut List {
    list_of_strings(&souther_text::characters(unsafe { text(&s) }))
}

/// `String.codePoints`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_code_point_values(s: *const Text) -> *mut List {
    list_of(
        &souther_text::code_points_of(unsafe { text(&s) }),
        |point| *point,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{souther_mark, souther_reset, souther_string_concat};
    use std::ptr;

    fn made(text: &str) -> *mut Text {
        string_of(text)
    }

    fn said(at: *const Text) -> String {
        String::from(unsafe { text(&at) }.as_str())
    }

    fn strings(list: *const List) -> Vec<String> {
        unsafe { texts(&list) }
            .into_iter()
            .map(|it| String::from(it.as_str()))
            .collect()
    }

    /// A list is its length and then its elements, and the strings a split answers are read back
    /// out of it by what a join reads.
    #[test]
    fn a_list_of_strings_is_written_as_the_layout_says_and_read_back() {
        let mark = souther_mark();
        let pieces = unsafe { souther_string_split(made(","), made("a,,日")) };
        assert_eq!(strings(pieces), ["a", "", "日"]);
        let joined = unsafe { souther_string_join(made("-"), pieces) };
        assert_eq!(said(joined), "a--日");
        assert_eq!(said(unsafe { souther_string_concat_all(pieces) }), "a日");
        souther_reset(mark);
    }

    /// Two strings joined are put in NFC at the seam.
    #[test]
    fn a_join_of_two_strings_is_canonical() {
        let mark = souther_mark();
        let joined = unsafe { souther_string_concat(made("e"), made("\u{301}")) };
        assert_eq!(said(joined), "\u{e9}");
        souther_reset(mark);
    }

    /// A kernel that can answer nothing writes its value only where it has one, and says which.
    #[test]
    fn a_kernel_answering_nothing_writes_nothing() {
        let mark = souther_mark();
        let mut out: *mut Text = ptr::null_mut();
        assert_eq!(
            unsafe { souther_string_slice(1, 3, made("a𠮷b"), &mut out) },
            1
        );
        assert_eq!(said(out), "𠮷b");
        let mut untouched: *mut Text = ptr::null_mut();
        assert_eq!(
            unsafe { souther_string_slice(2, 1, made("abc"), &mut untouched) },
            0
        );
        assert!(untouched.is_null());
        let mut read = -1;
        assert_eq!(unsafe { souther_string_to_int(made("-007"), &mut read) }, 1);
        assert_eq!(read, -7);
        let mut unread = -1;
        assert_eq!(
            unsafe { souther_string_to_int(made("１２３"), &mut unread) },
            0
        );
        assert_eq!(unread, -1);
        souther_reset(mark);
    }

    /// Code points are written into a list as the numbers they are.
    #[test]
    fn code_points_are_a_list_of_numbers() {
        let mark = souther_mark();
        let list = unsafe { souther_string_code_point_values(made("a𠮷")) };
        let at = list.cast::<u8>();
        unsafe {
            assert_eq!(at.offset(LIST_LENGTH as isize).cast::<i64>().read(), 2);
            assert_eq!(at.offset(list_at(0) as isize).cast::<i64>().read(), 0x61);
            assert_eq!(at.offset(list_at(1) as isize).cast::<i64>().read(), 0x20bb7);
        }
        souther_reset(mark);
    }
}
