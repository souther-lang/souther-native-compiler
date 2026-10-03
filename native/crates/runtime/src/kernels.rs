//! The `String` module's kernels, as generated code calls them.
//!
//! What each answers is `souther_text`'s. What is here is where the answer is kept: text read out of
//! the strings handed over, and the text or the pieces `souther_text` answered written into the
//! arena as a string or a list of the layout `souther_native_abi` states. Nothing here decides
//! what a kernel means, and nothing here knows why a kernel answers nothing where it does: a
//! kernel that can answer nothing says only whether it wrote its value, and what the caller makes
//! of that is the caller's contract.

use crate::{Bool, Count, List, STRING_HOLDS, Text, souther_alloc, string_of, text};
use notation199x::Pattern;
use souther_native_abi::{
    LIST_LENGTH, PATTERN_IMAGE, PATTERN_LENGTH, PATTERN_READ, list_at, room_for_list,
};
use souther_text::Text as Held;
use std::sync::atomic::{AtomicPtr, Ordering};

/// A list of these, each written into its slot by `slot`.
pub(crate) fn list_of<T>(each: &[T], slot: impl Fn(&T) -> i64) -> *mut List {
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
/// `list` is a list whose every element is a string, and the scope it was made in is still open.
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
pub(crate) unsafe fn answered<T>(value: Option<T>, out: *mut T) -> Bool {
    match value {
        Some(value) => {
            unsafe { out.write(value) };
            Bool::TRUE
        }
        None => Bool::FALSE,
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
pub unsafe extern "C" fn souther_string_lowercase(s: *const Text, out: *mut *mut Text) -> Bool {
    let lowered = souther_text::lowercase(unsafe { text(&s) }, STRING_HOLDS);
    unsafe { answered(lowered.as_deref().map(string_of), out) }
}

/// `String.uppercase`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_uppercase(s: *const Text, out: *mut *mut Text) -> Bool {
    let raised = souther_text::uppercase(unsafe { text(&s) }, STRING_HOLDS);
    unsafe { answered(raised.as_deref().map(string_of), out) }
}

/// `String.contains`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_contains(sub: *const Text, s: *const Text) -> Bool {
    unsafe { souther_text::contains(text(&sub), text(&s)) }.into()
}

/// `String.startsWith`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_starts_with(prefix: *const Text, s: *const Text) -> Bool {
    unsafe { souther_text::starts_with(text(&prefix), text(&s)) }.into()
}

/// `String.endsWith`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_ends_with(suffix: *const Text, s: *const Text) -> Bool {
    unsafe { souther_text::ends_with(text(&suffix), text(&s)) }.into()
}

/// A pattern as an object carries it: room the runtime keeps the pattern it read in, how long the
/// image is, and the image, at the places `souther_native_abi` states ([`PATTERN_READ`] and the
/// rest).
#[repr(C)]
pub struct HeldPattern {
    _opaque: [u8; 0],
}

/// The pattern `held` carries, read from its image the first time it is asked for and kept in its
/// room from then on, so that a pattern is read once however many times it is matched.
///
/// Two threads that both find the room empty both read the image, and the first to put what it
/// read in the room is the one kept: the other drops its own. Nothing is ever taken out of the
/// room, so what is kept lives as long as the object does.
///
/// # Safety
///
/// `held` is a pattern an object carries, laid out as `souther_native_abi` states it, with its
/// room writable.
pub(crate) unsafe fn pattern(held: *const HeldPattern) -> &'static Pattern {
    let at = held.cast::<u8>();
    // SAFETY: the room is a slot, aligned to one, which only this function writes.
    let read = unsafe { &*at.add(PATTERN_READ as usize).cast::<AtomicPtr<Pattern>>() };
    let kept = read.load(Ordering::Acquire);
    if !kept.is_null() {
        // SAFETY: what is in the room was put there below and is never taken out.
        return unsafe { &*kept };
    }
    // SAFETY: the object wrote the image's length and the image behind it.
    let image = unsafe {
        let length = at.add(PATTERN_LENGTH as usize).cast::<u64>().read();
        std::slice::from_raw_parts(at.add(PATTERN_IMAGE as usize), length as usize)
    };
    let image = std::str::from_utf8(image).expect("an image is written in ASCII");
    let made = Box::into_raw(Box::new(
        Pattern::from_image(image).expect("the compiler read every image it wrote into the object"),
    ));
    match read.compare_exchange(
        std::ptr::null_mut(),
        made,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        // SAFETY: `made` is in the room now, and is never taken out.
        Ok(_) => unsafe { &*made },
        Err(first) => {
            // SAFETY: `made` was never shared, and `first` is in the room for good.
            unsafe {
                drop(Box::from_raw(made));
                &*first
            }
        }
    }
}

/// `String.matches`, run on the pattern the object carries.
///
/// # Safety
///
/// As [`souther_string_trim`], and as [`pattern`] for `held`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_matches(held: *const HeldPattern, s: *const Text) -> Bool {
    unsafe { pattern(held).matches(text(&s).as_str()) }.into()
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
) -> Bool {
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
pub unsafe extern "C" fn souther_string_join(
    separator: *const Text,
    xs: *const List,
    out: *mut *mut Text,
) -> Bool {
    let pieces = unsafe { texts(&xs) };
    let joined = souther_text::join(unsafe { text(&separator) }, pieces, STRING_HOLDS);
    unsafe { answered(joined.as_deref().map(string_of), out) }
}

/// `String.concat`.
///
/// # Safety
///
/// As [`souther_string_join`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_concat_all(xs: *const List, out: *mut *mut Text) -> Bool {
    let pieces = unsafe { texts(&xs) };
    let joined = souther_text::join(Held::held(""), pieces, STRING_HOLDS);
    unsafe { answered(joined.as_deref().map(string_of), out) }
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
    out: *mut *mut Text,
) -> Bool {
    let replaced =
        unsafe { souther_text::replace(text(&target), text(&replacement), text(&s), STRING_HOLDS) };
    unsafe { answered(replaced.as_deref().map(string_of), out) }
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
pub unsafe extern "C" fn souther_string_to_int(s: *const Text, out: *mut i64) -> Bool {
    unsafe { answered(souther_text::integer(text(&s)), out) }
}

/// `String.reverse`.
///
/// # Safety
///
/// As [`souther_string_trim`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_reverse(s: *const Text, out: *mut *mut Text) -> Bool {
    let reversed = souther_text::reverse(unsafe { text(&s) }, STRING_HOLDS);
    unsafe { answered(reversed.as_deref().map(string_of), out) }
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
) -> Bool {
    let repeated = souther_text::repeat(copies, unsafe { text(&s) }, STRING_HOLDS);
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
) -> Bool {
    let padded = unsafe { souther_text::pad_left(width, text(&pad), text(&s), STRING_HOLDS) };
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
) -> Bool {
    let padded = unsafe { souther_text::pad_right(width, text(&pad), text(&s), STRING_HOLDS) };
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
    use crate::{souther_scope_close, souther_scope_open, souther_string_concat};
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
        let scope = souther_scope_open();
        let pieces = unsafe { souther_string_split(made(","), made("a,,日")) };
        assert_eq!(strings(pieces), ["a", "", "日"]);
        let mut joined = ptr::null_mut();
        assert_eq!(
            unsafe { souther_string_join(made("-"), pieces, &mut joined) },
            Bool::TRUE
        );
        assert_eq!(said(joined), "a--日");
        assert_eq!(
            unsafe { souther_string_concat_all(pieces, &mut joined) },
            Bool::TRUE
        );
        assert_eq!(said(joined), "a日");
        souther_scope_close(scope);
    }

    /// A pattern's room as an object lays it out (`souther_native_abi`): nothing read yet, the
    /// image's length, and the image, in slots so that the room is aligned as an object aligns it.
    fn room(image: &str) -> Vec<u64> {
        let slots = PATTERN_IMAGE as usize / 8 + image.len().div_ceil(8);
        let mut room = vec![0u64; slots];
        let bytes: &mut [u8] =
            unsafe { std::slice::from_raw_parts_mut(room.as_mut_ptr().cast(), slots * 8) };
        bytes[PATTERN_LENGTH as usize..PATTERN_IMAGE as usize]
            .copy_from_slice(&(image.len() as u64).to_ne_bytes());
        bytes[PATTERN_IMAGE as usize..][..image.len()].copy_from_slice(image.as_bytes());
        room
    }

    /// A pattern is read from its image the first time it is matched and kept in its room, so that
    /// every match after it runs on the pattern read then.
    #[test]
    fn a_pattern_is_read_from_its_image_once_and_kept() {
        let scope = souther_scope_open();
        // `a+`, as P1 writes it.
        let mut held = room("P1,0,1,1,97,97,2,0,1,0,1,0,1,1,0,1,0");
        let at = held.as_mut_ptr().cast::<HeldPattern>();
        assert_eq!(held[0], 0);
        assert_eq!(
            unsafe { souther_string_matches(at, made("aaa")) },
            Bool::TRUE
        );
        let kept = held[0];
        assert_ne!(kept, 0);
        assert_eq!(unsafe { souther_string_matches(at, made("")) }, Bool::FALSE);
        assert_eq!(
            unsafe { souther_string_matches(at, made("ab")) },
            Bool::FALSE
        );
        assert_eq!(held[0], kept);
        souther_scope_close(scope);
    }

    /// Two strings joined are put in NFC at the seam.
    #[test]
    fn a_join_of_two_strings_is_canonical() {
        let scope = souther_scope_open();
        let mut joined = ptr::null_mut();
        assert_eq!(
            unsafe { souther_string_concat(made("e"), made("\u{301}"), &mut joined) },
            Bool::TRUE
        );
        assert_eq!(said(joined), "\u{e9}");
        souther_scope_close(scope);
    }

    /// A kernel that can answer nothing writes its value only where it has one, and says which.
    #[test]
    fn a_kernel_answering_nothing_writes_nothing() {
        let scope = souther_scope_open();
        let mut out: *mut Text = ptr::null_mut();
        assert_eq!(
            unsafe { souther_string_slice(1, 3, made("a𠮷b"), &mut out) },
            Bool::TRUE
        );
        assert_eq!(said(out), "𠮷b");
        let mut untouched: *mut Text = ptr::null_mut();
        assert_eq!(
            unsafe { souther_string_slice(2, 1, made("abc"), &mut untouched) },
            Bool::FALSE
        );
        assert!(untouched.is_null());
        let mut read = -1;
        assert_eq!(
            unsafe { souther_string_to_int(made("-007"), &mut read) },
            Bool::TRUE
        );
        assert_eq!(read, -7);
        let mut unread = -1;
        assert_eq!(
            unsafe { souther_string_to_int(made("１２３"), &mut unread) },
            Bool::FALSE
        );
        assert_eq!(unread, -1);
        souther_scope_close(scope);
    }

    /// Code points are written into a list as the numbers they are.
    #[test]
    fn code_points_are_a_list_of_numbers() {
        let scope = souther_scope_open();
        let list = unsafe { souther_string_code_point_values(made("a𠮷")) };
        let at = list.cast::<u8>();
        unsafe {
            assert_eq!(at.offset(LIST_LENGTH as isize).cast::<i64>().read(), 2);
            assert_eq!(at.offset(list_at(0) as isize).cast::<i64>().read(), 0x61);
            assert_eq!(at.offset(list_at(1) as isize).cast::<i64>().read(), 0x20bb7);
        }
        souther_scope_close(scope);
    }
}
